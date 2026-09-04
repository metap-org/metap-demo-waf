//! Edge node configuration, read once at boot from the environment.
//!
//! Hand-rolled rather than reusing `metap_runtime::env`: `edge-plane` deliberately does not depend
//! on `metap` at all (`../../data-plane/docs/04-architecture-boundary.md`). Ten lines of `env::var`
//! is a much smaller cost than the dependency that helper would drag in.

use std::time::Duration;

pub struct Config {
    pub listen_addr: String,
    /// Where compiled rule-sets are read from. The only thing this process reads config from —
    /// never `data-plane`.
    pub redis_url: String,
    /// How often the snapshot is refreshed. This value *is* the config-propagation SLA
    /// (10-30s in the architecture doc).
    pub refresh_interval: Duration,
    /// `control-plane`'s ingest endpoint. Telemetry goes here, never to `data-plane`.
    pub ingest_url: String,
    pub ingest_token: Option<String>,
    pub telemetry_buffer: usize,
    pub telemetry_max_batch: usize,
    pub telemetry_flush_interval: Duration,
    /// Header carrying the client's real IP when this node sits behind another proxy or an L4
    /// load balancer. **Unset by default, and that is the safe default**: trusting a
    /// client-supplied header would let anyone spoof their source IP past every IP rule and every
    /// rate limit. Only set this when something in front is guaranteed to overwrite it.
    pub client_ip_header: Option<String>,
    /// Header carrying an already-resolved ISO-3166 country code. There is no GeoIP database in
    /// this build; country rules simply never match unless something upstream provides this.
    pub geo_country_header: Option<String>,
    /// Cookie name for the challenge clearance grant.
    pub clearance_cookie: String,
    pub clearance_ttl: Duration,
    /// Secret the clearance cookie is keyed with. A per-node random default means a restart
    /// invalidates outstanding clearances (a re-challenge, not an outage); set it explicitly to
    /// share clearances across a fleet.
    pub clearance_secret: String,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn parsed<T: std::str::FromStr>(name: &str, default: T) -> T {
    var(name).and_then(|value| value.parse().ok()).unwrap_or(default)
}

pub fn load() -> Config {
    Config {
        listen_addr: var("LISTEN_ADDR").unwrap_or_else(|| "0.0.0.0:8080".to_string()),
        redis_url: var("REDIS_URL").unwrap_or_else(|| "redis://localhost:6379".to_string()),
        refresh_interval: Duration::from_secs(parsed("REFRESH_INTERVAL_SECONDS", 10u64)),
        ingest_url: var("INGEST_URL").unwrap_or_else(|| "http://localhost:4100/ingest/events".to_string()),
        ingest_token: var("INGEST_TOKEN"),
        telemetry_buffer: parsed("TELEMETRY_BUFFER", 10_000usize),
        telemetry_max_batch: parsed("TELEMETRY_MAX_BATCH", 100usize),
        telemetry_flush_interval: Duration::from_millis(parsed("TELEMETRY_FLUSH_MS", 2_000u64)),
        client_ip_header: var("CLIENT_IP_HEADER"),
        geo_country_header: var("GEO_COUNTRY_HEADER"),
        clearance_cookie: var("CLEARANCE_COOKIE").unwrap_or_else(|| "waf_clearance".to_string()),
        clearance_ttl: Duration::from_secs(parsed("CLEARANCE_TTL_SECONDS", 1_800u64)),
        clearance_secret: var("CLEARANCE_SECRET").unwrap_or_else(random_secret),
    }
}

/// Node-local secret when none is configured. Not cryptographically strong entropy — it is a
/// startup nonce, and its only job is that two nodes (or two runs) don't accept each other's
/// clearance cookies by accident.
fn random_secret() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("edge-{nanos:x}")
}
