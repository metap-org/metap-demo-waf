//! Telemetry going **up**: `edge-plane` → here → `data-plane`.
//!
//! # The architecture decision this implements
//!
//! `data-plane/docs/04-architecture-boundary.md` left this explicitly unresolved, with two
//! candidates: (1) the edge calls `metap-grpc`'s generic `RecordService.Create` on `data-plane`
//! directly, or (2) the edge batches to `control-plane`, which writes onward. **This implements
//! (2)**, which is also the option that doc leaned toward, for the reason it gave: option (1)
//! breaks the rule stated everywhere else in the design — *`edge-plane` never talks to
//! `data-plane` directly* — and it would put N edge nodes × real traffic worth of small writes
//! straight onto the portal's database.
//!
//! This was decided in-session rather than by the project owner, so it is called out in the PR
//! and in the roadmap entry rather than buried here. It is reversible: nothing in `edge-plane`
//! knows where its telemetry goes beyond one URL.
//!
//! # What this is not
//!
//! It is not a bypass of `CrudService`. Every event still goes through `data-plane`'s ordinary
//! `POST /api/waf.security_events`, so validation, permission and the outbox all apply exactly as
//! they do for a portal write. What batching buys is fewer, larger requests — not a shortcut past
//! the platform's rules.
//!
//! # Known limitation
//!
//! The buffer is in memory and unacknowledged: if this process dies with events queued, those
//! events are lost. That is an accepted trade for v1 (telemetry, not billing data), and the honest
//! fix — a durable queue between edge and portal — is a real piece of work, not a tweak. Flagged
//! rather than hidden.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

use crate::dataplane::DataPlane;
use crate::distribute::Distributor;

/// One event as the edge reports it. Field names match `waf.security_events`' own metadata so
/// this layer stays a pass-through rather than a second schema to keep in sync.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeEvent {
    pub zone_id: String,
    /// `ddosPolicy` or `firewallRule`.
    pub triggered_by: String,
    pub triggered_by_id: String,
    #[serde(default)]
    pub triggered_by_name: Option<String>,
    /// `logged` / `challenged` / `blocked`.
    pub action: String,
    pub source_ip: String,
    pub request_path: String,
    pub occurred_at: String,
}

#[derive(Debug, Deserialize)]
pub struct IngestBody {
    pub events: Vec<EdgeEvent>,
}

#[derive(Clone)]
pub struct IngestState {
    pub sender: mpsc::Sender<EdgeEvent>,
    pub distributor: Arc<Distributor>,
    pub max_request_events: usize,
    pub ingest_token: Option<String>,
}

async fn ingest(State(state): State<IngestState>, headers: HeaderMap, body: Option<Json<IngestBody>>) -> Response {
    if let Some(expected) = &state.ingest_token {
        let presented = headers
            .get("x-waf-ingest-token")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        // Length-independent comparison isn't worth reaching for here (this is a coarse
        // deployment boundary, not a per-user credential), but an absent token must never pass.
        if presented.is_empty() || presented != expected {
            return (StatusCode::UNAUTHORIZED, Json(json!({ "error": "invalid ingest token" }))).into_response();
        }
    }

    let Some(Json(body)) = body else {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "malformed body" }))).into_response();
    };
    if body.events.len() > state.max_request_events {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({ "error": "too many events in one request", "max": state.max_request_events })),
        )
            .into_response();
    }

    let mut accepted = 0usize;
    let mut dropped = 0usize;
    for event in body.events {
        // `try_send`, not `send`: blocking the edge's ingest request until the writer drains
        // would turn a slow `data-plane` into back-pressure on request handling at the edge,
        // which is the one place in this system that must never wait on the portal.
        match state.sender.try_send(event) {
            Ok(()) => accepted += 1,
            Err(_) => dropped += 1,
        }
    }
    if dropped > 0 {
        tracing::warn!(dropped, "ingest buffer full, dropped events");
    }

    (
        StatusCode::ACCEPTED,
        Json(json!({ "data": { "accepted": accepted, "dropped": dropped } })),
    )
        .into_response()
}

async fn health(State(state): State<IngestState>) -> Response {
    let redis_ok = state.distributor.ping().await.is_ok();
    let status = if redis_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (status, Json(json!({ "data": { "self": "ok", "redis": redis_ok } }))).into_response()
}

pub fn router(state: IngestState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ingest/events", post(ingest))
        .with_state(state)
}

/// Drains the buffer into `data-plane`, one record per event.
///
/// The batching here is of *requests in flight*, not of a bulk endpoint: `metap`'s generic CRUD
/// has no batch-create route, and inventing one for this would be a much larger change to core
/// than this feature warrants. So the win is that the edge makes one call per batch instead of one
/// per request, and this worker absorbs the fan-out where a slow write costs nobody latency.
pub async fn run_writer(
    data_plane: Arc<DataPlane>,
    mut receiver: mpsc::Receiver<EdgeEvent>,
    max_batch: usize,
    shutdown: impl std::future::Future<Output = ()>,
) {
    let mut shutdown = std::pin::pin!(shutdown);
    loop {
        let mut batch: Vec<EdgeEvent> = Vec::new();
        tokio::select! {
            biased;
            _ = &mut shutdown => {
                tracing::info!("ingest writer shutting down");
                return;
            }
            received = receiver.recv() => {
                match received {
                    Some(event) => batch.push(event),
                    None => return,
                }
            }
        }
        // Opportunistically take whatever else is already queued, up to the cap — this is what
        // turns a burst into one drain pass instead of one pass per event.
        while batch.len() < max_batch {
            match receiver.try_recv() {
                Ok(event) => batch.push(event),
                Err(_) => break,
            }
        }

        let mut written = 0usize;
        for event in &batch {
            let payload = json!({
                "zoneId": event.zone_id,
                "triggeredBy": event.triggered_by,
                "triggeredById": event.triggered_by_id,
                "triggeredByName": event.triggered_by_name,
                "action": event.action,
                "sourceIp": event.source_ip,
                "requestPath": event.request_path,
                "occurredAt": event.occurred_at,
            });
            match data_plane.create_security_event(&payload).await {
                Ok(()) => written += 1,
                Err(err) => {
                    // Logged and dropped, not retried: a retry loop here would stall the whole
                    // buffer behind one bad event, and the buffer's own doc comment already owns
                    // the "telemetry is lossy under failure" trade.
                    tracing::warn!(error = %err, "failed to write security event");
                }
            }
        }
        tracing::debug!(batch = batch.len(), written, "ingest batch written");
    }
}
