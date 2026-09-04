//! `waf-config-distributor` — the whole of `control-plane` today.
//!
//! Three concurrent jobs in one process, all shut down together:
//!
//! 1. **subscribe** — outbox events from `data-plane` → recompile the affected zone (fast path).
//! 2. **resync** — full sweep on a timer → guarantees convergence when an event is missed.
//! 3. **ingest** — an HTTP endpoint the edge posts telemetry batches to, plus `/health`.
//!
//! One process rather than three because they share the same `data-plane` client (and therefore
//! one service-account session) and the same Redis connection pool; splitting them would triple
//! the login traffic and the connection count to serve the same work. If the ingest path ever
//! needs to scale separately from config distribution, it is the obvious seam to cut.
//!
//! No UI, no CRUD, no database of its own — everything it knows comes from `data-plane`'s API and
//! everything it produces lands in Redis or back in `data-plane`.

mod compile;
mod config;
mod dataplane;
mod distribute;
mod ingest;
mod resync;
mod ruleset;
mod subscribe;
mod sync;

use std::sync::Arc;

use metap::runtime::service_token::ServiceTokenSource;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    metap::infra::init_tracing();
    dotenvy::dotenv().ok();
    let config = config::load()?;

    let http = metap::runtime::http_client::default_client();
    // Fails boot if it can't log in — unlike `cron-scheduler`, which degrades because some of its
    // job types need no credential. Here every single thing this process does needs `data-plane`,
    // so starting up without a session would just be a process that logs errors forever.
    let token = ServiceTokenSource::start(
        http.clone(),
        config.login_url.clone(),
        config.service_email.clone(),
        config.service_password.clone(),
    )
    .await?;

    let data_plane = Arc::new(dataplane::DataPlane::new(
        http,
        config.zones_url.clone(),
        config.alerting_url.clone(),
        token,
    ));
    let distributor = Arc::new(distribute::Distributor::connect(&config.redis_url)?);
    distributor.ping().await?;

    // Once at boot, before anything else starts: a cold edge otherwise has no config at all until
    // either the first change event or the first timer tick, and "no config" at the edge means
    // unprotected hostnames.
    if let Err(err) = resync::run_once(&data_plane, &distributor).await {
        // Not fatal — the timer will retry, and refusing to boot would take the ingest endpoint
        // down with it for a problem that may well be a slow `data-plane` still starting up.
        tracing::error!(error = %err, "initial resync failed, continuing");
    }

    let (sender, receiver) = mpsc::channel::<ingest::EdgeEvent>(10_000);
    let ingest_state = ingest::IngestState {
        sender,
        distributor: distributor.clone(),
        max_request_events: config.ingest_max_request_events,
        ingest_token: config.ingest_token.clone(),
    };

    // One shutdown signal, three listeners — `metap_runtime::shutdown::signal` handles both Ctrl+C
    // and SIGTERM (the container case), and a broadcast channel is how one future reaches three
    // tasks that each need their own.
    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        metap::runtime::shutdown::signal().await;
        let _ = signal_tx.send(());
    });
    let subscriber = |mut rx: tokio::sync::broadcast::Receiver<()>| async move {
        let _ = rx.recv().await;
    };

    let subscribe_task = tokio::spawn(subscribe::run(
        config.amqp_url.clone(),
        config.queue.clone(),
        data_plane.clone(),
        distributor.clone(),
        subscriber(shutdown_tx.subscribe()),
    ));
    let resync_task = tokio::spawn(resync::run_loop(
        data_plane.clone(),
        distributor.clone(),
        config.resync_interval,
        subscriber(shutdown_tx.subscribe()),
    ));
    let writer_task = tokio::spawn(ingest::run_writer(
        data_plane.clone(),
        receiver,
        config.ingest_max_batch,
        subscriber(shutdown_tx.subscribe()),
    ));

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!(
        addr,
        zones_url = config.zones_url,
        redis = config.redis_url,
        resync_seconds = config.resync_interval.as_secs(),
        "waf-config-distributor starting"
    );
    metap::runtime::serve::run(&addr, ingest::router(ingest_state).into_make_service()).await?;

    // `serve::run` returns on the same shutdown signal, so by here the other three are stopping
    // too — join them so an in-flight compile or write finishes rather than being cut off.
    let _ = tokio::join!(subscribe_task, resync_task, writer_task);
    Ok(())
}
