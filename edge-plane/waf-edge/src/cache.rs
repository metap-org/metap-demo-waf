//! The edge's view of configuration: a hostname → rule-set snapshot, refreshed from Redis.
//!
//! Three properties this is built around, in priority order:
//!
//! 1. **A request never touches Redis.** Lookups hit an in-memory map behind `ArcSwap`; the
//!    refresh happens on a timer in the background. A config store being slow or down must never
//!    become request latency.
//! 2. **The last good snapshot survives everything.** If Redis is unreachable, if a payload fails
//!    to parse, if the schema is newer than this binary — the previous snapshot keeps serving.
//!    A config-distribution problem must degrade to staleness, never to an outage or (worse) to
//!    an unprotected zone.
//! 3. **Refresh is cheap.** The ticker reads the version key per zone (a small integer) and only
//!    re-fetches the rule-sets whose `configVersion` actually moved. That is what
//!    `Zone.configVersion` exists for (`../../data-plane/docs/04-architecture-boundary.md`) —
//!    the alternative, re-downloading every zone every few seconds, scales with fleet size for no
//!    reason.
//!
//! The refresh interval is what the 10-30s config-propagation SLA in that same doc actually is,
//! in code.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use redis::AsyncCommands;

use crate::ruleset::{zone_key, zone_version_key, CompiledZone, EPOCH_KEY, SCHEMA_VERSION, ZONE_INDEX_KEY};

pub struct Snapshot {
    pub zones: HashMap<String, Arc<CompiledZone>>,
    pub epoch: u64,
}

impl Snapshot {
    fn empty() -> Self {
        Self {
            zones: HashMap::new(),
            epoch: 0,
        }
    }
}

pub struct RuleSetCache {
    client: redis::Client,
    current: ArcSwap<Snapshot>,
}

impl RuleSetCache {
    pub fn new(redis_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            client: redis::Client::open(redis_url)?,
            current: ArcSwap::from_pointee(Snapshot::empty()),
        })
    }

    /// The hot-path lookup: exact hostname match, no allocation, no I/O.
    ///
    /// Unknown hostname → `None` → the request is refused. An edge that passed traffic for a
    /// hostname it has no configuration for would be an open proxy.
    pub fn zone_for(&self, hostname: &str) -> Option<Arc<CompiledZone>> {
        self.current.load().zones.get(hostname).cloned()
    }

    pub fn zone_count(&self) -> usize {
        self.current.load().zones.len()
    }

    pub fn epoch(&self) -> u64 {
        self.current.load().epoch
    }

    /// One refresh pass. Returns the number of zones re-fetched, for the log line.
    async fn refresh_once(&self) -> anyhow::Result<usize> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let hostnames: Vec<String> = conn.smembers(ZONE_INDEX_KEY).await?;
        let epoch: u64 = conn.get(EPOCH_KEY).await.unwrap_or(0);
        let previous = self.current.load();

        let mut zones: HashMap<String, Arc<CompiledZone>> = HashMap::with_capacity(hostnames.len());
        let mut fetched = 0usize;

        for hostname in &hostnames {
            let version: Option<i64> = conn.get(zone_version_key(hostname)).await.unwrap_or(None);
            if let (Some(version), Some(existing)) = (version, previous.zones.get(hostname)) {
                if existing.config_version == version {
                    // Unchanged — carry the parsed value forward rather than re-downloading and
                    // re-parsing it. This is the case for almost every zone on almost every tick.
                    zones.insert(hostname.clone(), existing.clone());
                    continue;
                }
            }

            let raw: Option<String> = conn.get(zone_key(hostname)).await.unwrap_or(None);
            let Some(raw) = raw else {
                // Indexed but no rule-set: the control-plane is mid-write, or mid-delete. Keep
                // whatever we already had for this hostname and let the next tick settle it.
                if let Some(existing) = previous.zones.get(hostname) {
                    zones.insert(hostname.clone(), existing.clone());
                }
                continue;
            };
            match serde_json::from_str::<CompiledZone>(&raw) {
                Ok(zone) if zone.schema_version <= SCHEMA_VERSION => {
                    fetched += 1;
                    zones.insert(hostname.clone(), Arc::new(zone));
                }
                Ok(zone) => {
                    // A newer control-plane wrote a contract this binary doesn't understand.
                    // Keeping the old snapshot for this zone is the only safe answer: parsing it
                    // partially could drop a rule the operator is relying on.
                    tracing::error!(
                        hostname,
                        schema_version = zone.schema_version,
                        supported = SCHEMA_VERSION,
                        "rule-set schema is newer than this build — keeping the previous snapshot for this zone"
                    );
                    if let Some(existing) = previous.zones.get(hostname) {
                        zones.insert(hostname.clone(), existing.clone());
                    }
                }
                Err(err) => {
                    tracing::error!(hostname, error = %err, "failed to parse rule-set, keeping previous");
                    if let Some(existing) = previous.zones.get(hostname) {
                        zones.insert(hostname.clone(), existing.clone());
                    }
                }
            }
        }

        // Hostnames that left the index are simply absent from the new map — that is how a
        // deleted or suspended zone stops being served, without a separate eviction path.
        self.current.store(Arc::new(Snapshot { zones, epoch }));
        Ok(fetched)
    }

    /// Loads once at startup, failing loudly. Unlike the refresh loop, this one is fatal: an edge
    /// node that starts with no configuration would answer every request with "unknown host",
    /// which looks exactly like a total outage to every zone it should be serving.
    pub async fn load_initial(&self) -> anyhow::Result<()> {
        let fetched = self.refresh_once().await?;
        tracing::info!(zones = self.zone_count(), fetched, epoch = self.epoch(), "initial rule-sets loaded");
        Ok(())
    }

    pub async fn run_refresh_loop(self: Arc<Self>, interval: Duration, shutdown: impl std::future::Future<Output = ()>) {
        let mut shutdown = std::pin::pin!(shutdown);
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown => {
                    tracing::info!("rule-set refresh loop shutting down");
                    return;
                }
                _ = ticker.tick() => {
                    match self.refresh_once().await {
                        Ok(fetched) if fetched > 0 => {
                            tracing::info!(zones = self.zone_count(), fetched, "rule-sets refreshed");
                        }
                        Ok(_) => {}
                        Err(err) => {
                            // Never fatal, and deliberately not escalating: the previous snapshot
                            // is still serving traffic correctly. Redis being down degrades
                            // config freshness, not availability.
                            tracing::warn!(error = %err, "rule-set refresh failed, serving previous snapshot");
                        }
                    }
                }
            }
        }
    }
}
