//! The periodic full sweep — what actually guarantees the edge converges.
//!
//! Runs once at boot (so a cold start doesn't wait for the first change event before the edge has
//! any config at all) and then on `RESYNC_INTERVAL_SECONDS`. It is deliberately the *simple*
//! path: read every zone, recompile, publish, and delete anything published that no longer
//! belongs. No diffing against what was published before — republishing an unchanged zone writes
//! the same bytes, and the cost of a wrong diff is a zone silently serving stale rules.
//!
//! This is also the only thing that removes a zone whose delete event went missing: the index in
//! Redis is compared against the set of hostnames `data-plane` currently says should be served.

use std::collections::HashSet;
use std::sync::Arc;

use crate::compile::{compile_zone, publishable};
use crate::dataplane::DataPlane;
use crate::distribute::Distributor;

pub async fn run_once(data_plane: &DataPlane, distributor: &Distributor) -> anyhow::Result<()> {
    let zones = data_plane.all_zones().await?;
    let mut expected: HashSet<String> = HashSet::new();

    for zone in &zones {
        let Some(hostname) = zone.str("hostname").map(str::to_string) else {
            continue;
        };
        if !publishable(zone) {
            continue;
        }
        // `tenantId` is not part of a record's own field bag — it is the tenant the service
        // account is scoped to, which for this worker is the tenant it logged into. Reading it
        // from the record would be wrong anyway: the API only ever returns this tenant's rows.
        let tenant_id = zone.str("tenantId").unwrap_or_default().to_string();

        let ddos = data_plane.ddos_policy_for(&zone.id).await?;
        let rules = data_plane.rules_for(&zone.id).await?;
        let Some(compiled) = compile_zone(zone, &tenant_id, ddos.as_ref(), &rules) else {
            continue;
        };
        distributor.publish(&compiled).await?;
        expected.insert(hostname);
    }

    // Anything published that `data-plane` no longer considers servable: a deleted zone, one
    // suspended or paused back to `pending`, or a hostname that was renamed.
    for hostname in distributor.published_hostnames().await? {
        if !expected.contains(&hostname) {
            tracing::info!(hostname, "resync found an orphaned rule-set, removing");
            distributor.unpublish(&hostname).await?;
        }
    }

    let epoch = distributor.bump_epoch().await?;
    tracing::info!(zones = expected.len(), epoch, "full resync complete");
    Ok(())
}

pub async fn run_loop(
    data_plane: Arc<DataPlane>,
    distributor: Arc<Distributor>,
    interval: std::time::Duration,
    shutdown: impl std::future::Future<Output = ()>,
) {
    let mut shutdown = std::pin::pin!(shutdown);
    let mut ticker = tokio::time::interval(interval);
    // Without this, a slow sweep (a large fleet, a sluggish `data-plane`) makes `interval` fire
    // repeatedly in a burst to "catch up" — several full sweeps back to back against the same
    // API this worker is trying not to overload.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => {
                tracing::info!("resync loop shutting down");
                return;
            }
            _ = ticker.tick() => {
                if let Err(err) = run_once(&data_plane, &distributor).await {
                    // Never fatal: the next tick retries, and the incremental path is still
                    // running. A resync failure degrades freshness, it does not stop the worker.
                    tracing::error!(error = %err, "resync failed");
                }
            }
        }
    }
}
