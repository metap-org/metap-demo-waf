//! `waf-edge` — the mitigation engine. The part of this product that actually blocks a request.
//!
//! # Why this is not built on `metap`
//!
//! Everything else in this repo is: the portal is metadata-driven CRUD, and that is the right
//! tool for config that changes shape as the business learns. This is the opposite problem —
//! fixed shape, enormous volume, latency measured per request — so it is a plain hyper server
//! with a short dependency list and no framework between it and the socket
//! (`../../data-plane/docs/04-architecture-boundary.md`).
//!
//! # What it is allowed to know
//!
//! Only what `control-plane` compiled into Redis (`ruleset.rs`). It never reads `data-plane`'s
//! database, never calls its API, and never learns an entity name. Telemetry goes back up through
//! `control-plane` for the same reason.
//!
//! # Request path
//!
//! ```text
//! request → Host → zone snapshot (in-memory, no I/O)
//!         → clearance cookie? → skip challenge
//!         → evaluate: DDoS budget, then rules by priority (first match wins)
//!         → block / challenge / log
//!         → allowed: proxy to origin
//!         → non-allow: queue a SecurityEvent (never blocking)
//! ```
//!
//! # Scope
//!
//! Reverse proxy adequate for a demo, not a CDN: HTTP only (TLS termination is expected in front),
//! buffered request bodies, no origin health checking or failover, no GeoIP database of its own.
//! Each of those is called out where it lives rather than implied to work.

mod body;
mod cache;
mod clearance;
mod config;
mod evaluate;
mod pages;
mod proxy;
mod ratelimit;
mod ruleset;
mod telemetry;

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use hyper::header::{HOST, USER_AGENT};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use crate::body::{full, BoxBody};
use crate::cache::RuleSetCache;
use crate::evaluate::{evaluate, RequestContext};
use crate::ratelimit::RateLimiter;
use crate::ruleset::Action;
use crate::telemetry::{SecurityEvent, Telemetry};

struct Edge {
    config: config::Config,
    cache: Arc<RuleSetCache>,
    limiter: Arc<RateLimiter>,
    telemetry: Arc<Telemetry>,
    client: proxy::ProxyClient,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    dotenvy::dotenv().ok();
    let config = config::load();

    let cache = Arc::new(RuleSetCache::new(&config.redis_url)?);
    // Fatal on purpose: a node that starts with no rule-sets answers every request with "unknown
    // host", which to every zone it should be serving is indistinguishable from a total outage.
    cache.load_initial().await?;

    let limiter = Arc::new(RateLimiter::new());
    let (telemetry, receiver) = Telemetry::new(config.telemetry_buffer);

    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let signal_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = signal_tx.send(());
    });
    let wait_for_shutdown = |mut rx: tokio::sync::broadcast::Receiver<()>| async move {
        let _ = rx.recv().await;
    };

    tokio::spawn(
        cache
            .clone()
            .run_refresh_loop(config.refresh_interval, wait_for_shutdown(shutdown_tx.subscribe())),
    );
    tokio::spawn(telemetry::run_shipper(
        telemetry.clone(),
        receiver,
        config.ingest_url.clone(),
        config.ingest_token.clone(),
        config.telemetry_max_batch,
        config.telemetry_flush_interval,
        wait_for_shutdown(shutdown_tx.subscribe()),
    ));
    // Rate-limit counters are pruned on a timer rather than during `check` so the request path
    // never pays for cleanup.
    {
        let limiter = limiter.clone();
        let mut shutdown = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown.recv() => return,
                    _ = ticker.tick() => limiter.prune(),
                }
            }
        });
    }

    let listen_addr: SocketAddr = config.listen_addr.parse()?;
    let edge = Arc::new(Edge {
        config,
        cache,
        limiter,
        telemetry,
        client: proxy::build_client(),
    });

    let listener = TcpListener::bind(listen_addr).await?;
    tracing::info!(
        %listen_addr,
        zones = edge.cache.zone_count(),
        "waf-edge listening"
    );

    let mut shutdown = shutdown_tx.subscribe();
    loop {
        tokio::select! {
            biased;
            _ = shutdown.recv() => {
                tracing::info!("shutting down");
                return Ok(());
            }
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(pair) => pair,
                    Err(err) => {
                        // A single failed accept (fd exhaustion, a client gone before the
                        // handshake) must not take the listener down.
                        tracing::warn!(error = %err, "accept failed");
                        continue;
                    }
                };
                let edge = edge.clone();
                tokio::spawn(async move {
                    let io = TokioIo::new(stream);
                    let service = service_fn(move |request| {
                        let edge = edge.clone();
                        async move { handle(edge, request, peer).await }
                    });
                    if let Err(err) = hyper::server::conn::http1::Builder::new()
                        .serve_connection(io, service)
                        .await
                    {
                        tracing::debug!(error = %err, "connection closed with error");
                    }
                });
            }
        }
    }
}

/// The client's IP.
///
/// The socket peer by default. `CLIENT_IP_HEADER` overrides it only when explicitly configured,
/// because trusting a client-supplied header by default would let anyone spoof their source
/// address past every IP rule and every rate limit in the product.
fn client_ip(edge: &Edge, request: &Request<hyper::body::Incoming>, peer: SocketAddr) -> IpAddr {
    let Some(header) = edge.config.client_ip_header.as_deref() else {
        return peer.ip();
    };
    request
        .headers()
        .get(header)
        .and_then(|value| value.to_str().ok())
        // `X-Forwarded-For` is a list; the first entry is the original client.
        .and_then(|value| value.split(',').next())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
        .unwrap_or_else(|| peer.ip())
}

async fn handle(
    edge: Arc<Edge>,
    request: Request<hyper::body::Incoming>,
    peer: SocketAddr,
) -> Result<Response<BoxBody>, Infallible> {
    // Operational endpoints, answered before any zone lookup so they work even with no config
    // loaded at all.
    if request.uri().path() == "/__edge/health" {
        let (sent, dropped) = edge.telemetry.stats();
        let payload = format!(
            r#"{{"zones":{},"epoch":{},"rateLimitKeys":{},"eventsSent":{},"eventsDropped":{}}}"#,
            edge.cache.zone_count(),
            edge.cache.epoch(),
            edge.limiter.tracked_keys(),
            sent,
            dropped
        );
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(full(payload))
            .expect("static response builds"));
    }

    let host = request
        .headers()
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        // Strip the port: a zone is keyed by hostname, and `example.com:8080` is the same zone.
        .map(|value| value.split(':').next().unwrap_or(value).to_string())
        .unwrap_or_default();

    let Some(zone) = edge.cache.zone_for(&host) else {
        return Ok(pages::unknown_host());
    };

    let ip = client_ip(&edge, &request, peer);
    let ip_text = ip.to_string();
    let path = request.uri().path().to_string();
    let query = request.uri().query().unwrap_or("").to_string();
    let method = request.method().as_str().to_string();
    let user_agent = request
        .headers()
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let country = edge
        .config
        .geo_country_header
        .as_deref()
        .and_then(|name| request.headers().get(name))
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let cookie_header = request
        .headers()
        .get(hyper::header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();

    let context = RequestContext {
        method: &method,
        path: &path,
        query: &query,
        client_ip: ip,
        client_ip_text: ip_text.clone(),
        user_agent: &user_agent,
        headers: request.headers(),
        country: &country,
    };

    let decision = evaluate(&zone, &context, &edge.limiter);

    let Some(decision) = decision else {
        return Ok(pass_through(&edge, request, &zone, &ip_text, &host).await);
    };

    let action = decision.effective_action(&zone);
    let ray = pages::ray_id();

    // Reported before the response is built, so an event exists even for a request the origin
    // never sees. `record` never blocks — a full buffer drops the event rather than the request.
    edge.telemetry.record(SecurityEvent {
        zone_id: zone.zone_id.clone(),
        triggered_by: decision.triggered_by.to_string(),
        triggered_by_id: decision.triggered_by_id.clone(),
        triggered_by_name: decision.triggered_by_name.clone(),
        // The *real* verdict, not the monitor-mode downgrade — seeing what enforcing would have
        // done is the entire point of monitor mode.
        action: decision.action.event_name().to_string(),
        source_ip: ip_text.clone(),
        request_path: path.clone(),
        occurred_at: telemetry::now_rfc3339(),
    });

    match action {
        Action::Block => Ok(pages::blocked(&ray)),
        Action::Challenge => {
            // A client that already passed a challenge for this zone and IP is let through
            // rather than looped forever on the interstitial.
            if clearance::verify(
                &cookie_header,
                &edge.config.clearance_cookie,
                &edge.config.clearance_secret,
                &zone.zone_id,
                &ip_text,
            ) {
                return Ok(pass_through(&edge, request, &zone, &ip_text, &host).await);
            }
            let cookie = clearance::issue(
                &edge.config.clearance_cookie,
                &edge.config.clearance_secret,
                &zone.zone_id,
                &ip_text,
                edge.config.clearance_ttl,
            );
            Ok(pages::challenge(&cookie, &ray))
        }
        // `Log` and `Allow` both pass the request on: the event above is the whole effect of a
        // log rule, and this is also the branch every decision takes in monitor mode.
        Action::Log | Action::Allow => Ok(pass_through(&edge, request, &zone, &ip_text, &host).await),
    }
}

async fn pass_through(
    edge: &Edge,
    request: Request<hyper::body::Incoming>,
    zone: &ruleset::CompiledZone,
    client_ip: &str,
    host: &str,
) -> Response<BoxBody> {
    match proxy::forward(&edge.client, request, &zone.origin_address, client_ip, host).await {
        Ok(response) => response,
        Err(err) => {
            tracing::warn!(
                hostname = zone.hostname,
                origin = zone.origin_address,
                ?err,
                "origin request failed"
            );
            pages::origin_unreachable(&pages::ray_id())
        }
    }
}
