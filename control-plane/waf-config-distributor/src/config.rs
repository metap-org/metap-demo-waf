//! Process configuration, read once at boot.
//!
//! Reuses `metap_runtime::env` (`require_env`/`env_or`/`optional`) rather than hand-rolling the
//! `env::var(...).ok().and_then(parse).unwrap_or(default)` idiom — that helper exists precisely
//! because the same shape kept being reimplemented per binary.

use std::time::Duration;

use metap::runtime::env::{env_or, optional, require_env};

pub struct Config {
    /// Where `zones-service` lives — the only `data-plane` service this worker reads config from.
    pub zones_url: String,
    /// Where `alerting-service` lives — telemetry coming up from the edge is written here.
    pub alerting_url: String,
    /// A real user this process logs in as, exactly like `cron-scheduler` does. Not a hand-minted
    /// static JWT: that pattern already caused a live outage in `metap` when the token's TTL
    /// expired in a running deployment.
    pub login_url: String,
    pub service_email: String,
    pub service_password: String,

    pub redis_url: String,
    pub amqp_url: String,
    /// Durable queue name. Fixed (not per-instance) so several replicas of this worker share the
    /// queue and compete for messages instead of each compiling the same change.
    pub queue: String,

    /// Full reconcile interval. Events can be missed — a message lands in the DLQ, a replica dies
    /// mid-handle, someone edits the database directly — so the incremental path is an
    /// optimisation and this sweep is what actually guarantees convergence.
    pub resync_interval: Duration,
    /// HTTP port for `/health` and the telemetry ingest endpoint.
    pub port: u16,
    pub host: String,
    /// Ingest batching: flush when either bound is hit.
    pub ingest_max_batch: usize,
    /// Reject an ingest request larger than this many events outright — the edge is trusted, but
    /// an unbounded body on the highest-volume path in the system is how one bad node takes the
    /// worker down.
    pub ingest_max_request_events: usize,
    /// Shared secret the edge presents on `/ingest/events`. Optional in dev; when set, a request
    /// without it is rejected.
    pub ingest_token: Option<String>,
}

pub fn load() -> anyhow::Result<Config> {
    Ok(Config {
        zones_url: env_or("ZONES_URL", "http://localhost:3000".to_string()),
        alerting_url: env_or("ALERTING_URL", "http://localhost:3020".to_string()),
        login_url: env_or("CONTROL_LOGIN_URL", "http://localhost:3000/auth/login".to_string()),
        service_email: require_env("CONTROL_SERVICE_EMAIL")?,
        service_password: require_env("CONTROL_SERVICE_PASSWORD")?,
        redis_url: env_or("REDIS_URL", "redis://localhost:6379".to_string()),
        amqp_url: env_or("AMQP_URL", "amqp://guest:guest@localhost:5672/%2f".to_string()),
        queue: env_or("CONFIG_DISTRIBUTOR_QUEUE", "waf.config-distributor".to_string()),
        resync_interval: Duration::from_secs(env_or("RESYNC_INTERVAL_SECONDS", 60u64)),
        port: env_or("PORT", 4100u16),
        host: env_or("HOST", "0.0.0.0".to_string()),
        ingest_max_batch: env_or("INGEST_MAX_BATCH", 200usize),
        ingest_max_request_events: env_or("INGEST_MAX_REQUEST_EVENTS", 1000usize),
        ingest_token: optional("INGEST_TOKEN"),
    })
}
