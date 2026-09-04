//! Reporting what happened, without slowing down what happens next.
//!
//! Every non-allow decision produces a `SecurityEvent`. Two rules shape this module:
//!
//! 1. **Request handling never waits on telemetry.** Events go onto a bounded channel with
//!    `try_send`; if the channel is full they are dropped and counted. An edge that stalls
//!    requests because a reporting endpoint is slow has failed at its only job.
//! 2. **The edge posts to `control-plane`, never to `data-plane`.** That is the boundary rule
//!    stated throughout `../../data-plane/docs/04-architecture-boundary.md`, and the reason that
//!    doc leaned toward this option: N edge nodes writing straight into the portal's database
//!    would put real traffic volume onto it. The batching worker on the other side is what turns
//!    this into sane write rates.
//!
//! Dropping events under pressure is a deliberate trade, not an oversight: security events are
//! *evidence*, and losing some evidence during a flood is strictly better than letting the flood
//! through because the edge was busy reporting it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityEvent {
    pub zone_id: String,
    pub triggered_by: String,
    pub triggered_by_id: String,
    pub triggered_by_name: String,
    pub action: String,
    pub source_ip: String,
    pub request_path: String,
    pub occurred_at: String,
}

#[derive(Serialize)]
struct Batch<'a> {
    events: &'a [SecurityEvent],
}

pub struct Telemetry {
    sender: mpsc::Sender<SecurityEvent>,
    dropped: AtomicU64,
    sent: AtomicU64,
}

impl Telemetry {
    pub fn new(capacity: usize) -> (Arc<Self>, mpsc::Receiver<SecurityEvent>) {
        let (sender, receiver) = mpsc::channel(capacity);
        (
            Arc::new(Self {
                sender,
                dropped: AtomicU64::new(0),
                sent: AtomicU64::new(0),
            }),
            receiver,
        )
    }

    /// Non-blocking by construction — called from the request path.
    pub fn record(&self, event: SecurityEvent) {
        if self.sender.try_send(event).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.sent.load(Ordering::Relaxed),
            self.dropped.load(Ordering::Relaxed),
        )
    }

    fn mark_sent(&self, count: u64) {
        self.sent.fetch_add(count, Ordering::Relaxed);
    }
}

/// Drains the channel into `control-plane`'s ingest endpoint, batching by size or by time —
/// whichever comes first, so a quiet zone still reports promptly and a busy one still batches.
pub async fn run_shipper(
    telemetry: Arc<Telemetry>,
    mut receiver: mpsc::Receiver<SecurityEvent>,
    ingest_url: String,
    ingest_token: Option<String>,
    max_batch: usize,
    flush_interval: Duration,
    shutdown: impl std::future::Future<Output = ()>,
) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let mut shutdown = std::pin::pin!(shutdown);
    let mut batch: Vec<SecurityEvent> = Vec::with_capacity(max_batch);
    let mut ticker = tokio::time::interval(flush_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => {
                // Best-effort final flush: a clean shutdown shouldn't throw away evidence that is
                // already in hand.
                if !batch.is_empty() {
                    ship(&client, &ingest_url, ingest_token.as_deref(), &batch, &telemetry).await;
                }
                tracing::info!("telemetry shipper shutting down");
                return;
            }
            received = receiver.recv() => {
                match received {
                    Some(event) => {
                        batch.push(event);
                        if batch.len() >= max_batch {
                            ship(&client, &ingest_url, ingest_token.as_deref(), &batch, &telemetry).await;
                            batch.clear();
                        }
                    }
                    None => return,
                }
            }
            _ = ticker.tick() => {
                if !batch.is_empty() {
                    ship(&client, &ingest_url, ingest_token.as_deref(), &batch, &telemetry).await;
                    batch.clear();
                }
            }
        }
    }
}

/// One POST. Failures are logged and the batch is dropped rather than retried: a retry queue here
/// would grow without bound under exactly the conditions that caused the failure, and the module
/// doc already owns the "telemetry is lossy under pressure" trade.
async fn ship(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
    batch: &[SecurityEvent],
    telemetry: &Telemetry,
) {
    let mut request = client.post(url).json(&Batch { events: batch });
    if let Some(token) = token {
        request = request.header("X-WAF-Ingest-Token", token);
    }
    match request.send().await {
        Ok(response) if response.status().is_success() => {
            telemetry.mark_sent(batch.len() as u64);
        }
        Ok(response) => {
            tracing::warn!(status = %response.status(), count = batch.len(), "ingest rejected batch");
        }
        Err(err) => {
            tracing::warn!(error = %err, count = batch.len(), "failed to ship telemetry batch");
        }
    }
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}
