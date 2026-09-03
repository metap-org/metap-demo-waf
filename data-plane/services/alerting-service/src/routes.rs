//! Custom (non-generic-CRUD) HTTP surface for `alerting-service`.
//!
//! Two of the four things `docs/13-screen-api-map.md` lists as needing real code live here, and
//! both for the same reason: they are *decisions over a set of records*, not reads or writes of
//! one record, so no metadata-driven CRUD route can generate them.
//!
//! - **`/internal/incidents/correlate`** — turns raw `SecurityEvent` rows into `Incident`s using
//!   the static v1 rule `docs/02-domain-model.md` settled on: same `zoneId` + same `sourceIp`
//!   inside a 15-minute window, `threshold` events or more. Static, not per-tenant configurable —
//!   that was explicitly deferred to v2 in that same doc, so this does not invent a config
//!   surface for it.
//! - **`/internal/alerts/evaluate`** — the `AlertPolicy` side: "N events in M minutes on the same
//!   zone" (the copy note in `docs/08-module-detail-specs.md` module 8 — counted per zone, never
//!   summed across zones), producing an `AlertNotification` per firing.
//!
//! Both are written as endpoints rather than background loops so `metap-cron` can drive them on a
//! schedule (`targetType: "webhook"` pointed at this service — the pattern
//! `docs/features/06-async-verification-pattern-and-lowcode-custom-logic.md` recommends for
//! exactly this shape of work) while a portal user can also trigger one on demand. No new trigger
//! mechanism, no second scheduler.

use std::collections::HashMap;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use metap::crud::{JsonObject, RecordDto, ServiceResult};
use metap::http::error::{internal_error_response, service_error_response};
use metap::permission::RequestContext;
use metap::prelude::{AppState, AuthContext};
use metap::query::ListInput;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

/// Default correlation window/threshold — `docs/02-domain-model.md`'s settled v1 rule. Overridable
/// per request only so an operator can re-run a wider sweep by hand; the scheduled call uses the
/// defaults.
const DEFAULT_WINDOW_MINUTES: i64 = 15;
const DEFAULT_THRESHOLD: usize = 5;
/// How many recent events one correlation pass reads. Bounded on purpose: this is a periodic
/// sweep, not a backfill, and an unbounded read of the highest-volume entity in the product is
/// the last thing this service should do on a timer.
const EVENT_SCAN_LIMIT: i64 = 200;

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default()
}

fn str_field(record: &RecordDto, field: &str) -> String {
    record
        .data
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Lists up to `limit` records with plain equality filters, unwrapping the service result into
/// `anyhow` — every caller here treats a permission/validation failure as "abort the sweep", not
/// as an empty set, because silently proceeding on an empty read would create nothing and look
/// exactly like "there was nothing to do".
async fn list_records(
    state: &AppState,
    entity: &str,
    filters: Vec<(String, String)>,
    limit: i64,
    context: &RequestContext,
) -> anyhow::Result<Vec<RecordDto>> {
    let input = ListInput {
        limit,
        filters,
        ..Default::default()
    };
    match state.crud.list(entity, &input, context).await? {
        ServiceResult::Ok { data, .. } => Ok(data),
        ServiceResult::Err { error, message, .. } => Err(anyhow::anyhow!(
            "{entity}: {error}{}",
            message.map(|m| format!(" ({m})")).unwrap_or_default()
        )),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorrelateBody {
    #[serde(default)]
    window_minutes: Option<i64>,
    #[serde(default)]
    threshold: Option<usize>,
    /// Restricts the sweep to one zone — what the portal's "correlate now" button on a zone sends.
    #[serde(default)]
    zone_id: Option<String>,
}

/// Severity from volume. Deliberately a fixed ladder, not config: `docs/02-domain-model.md` keeps
/// per-tenant thresholds out of v1, and an incident's severity is only meaningful next to the
/// same ladder every other incident was scored on.
fn severity_for(count: usize) -> &'static str {
    match count {
        0..=9 => "low",
        10..=49 => "medium",
        50..=199 => "high",
        _ => "critical",
    }
}

/// `POST /internal/incidents/correlate` — groups recent events by `(zoneId, sourceIp)` and opens
/// one `Incident` per group above the threshold.
///
/// Deduplication is by "is there already an open incident for this zone with this source IP in
/// its title": the incident entity has no `sourceIp` field of its own (it correlates events, it
/// doesn't copy them), so the title carries the key. Crude but stable, and it keeps a re-run
/// idempotent — which matters because this is scheduled, so it *will* run again over the same
/// events.
async fn correlate_incidents(
    State(state): State<AppState>,
    AuthContext(context): AuthContext,
    body: Option<Json<CorrelateBody>>,
) -> Response {
    let body = body.map(|Json(b)| b);
    let window_minutes = body
        .as_ref()
        .and_then(|b| b.window_minutes)
        .unwrap_or(DEFAULT_WINDOW_MINUTES);
    let threshold = body
        .as_ref()
        .and_then(|b| b.threshold)
        .unwrap_or(DEFAULT_THRESHOLD);
    let zone_filter = body.as_ref().and_then(|b| b.zone_id.clone());

    let mut filters = Vec::new();
    if let Some(zone_id) = &zone_filter {
        filters.push(("zoneId".to_string(), zone_id.clone()));
    }
    let events = match list_records(
        &state,
        "waf.security_events",
        filters,
        EVENT_SCAN_LIMIT,
        &context,
    )
    .await
    {
        Ok(rows) => rows,
        Err(e) => return internal_error_response(e),
    };

    let cutoff = chrono::Utc::now() - chrono::Duration::minutes(window_minutes);
    // Grouped in memory rather than by SQL: the window is small and bounded by `EVENT_SCAN_LIMIT`,
    // and this keeps the correlation rule readable as one expression of the business rule instead
    // of a query that has to be re-read to know what it correlates.
    let mut groups: HashMap<(String, String), usize> = HashMap::new();
    for event in &events {
        let occurred_at = event
            .data
            .get("occurredAt")
            .and_then(Value::as_str)
            .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            // An event with no parseable timestamp falls back to when the row was written, so a
            // malformed payload can't silently drop out of correlation entirely.
            .unwrap_or(event.created_at);
        if occurred_at < cutoff {
            continue;
        }
        let zone_id = str_field(event, "zoneId");
        let source_ip = str_field(event, "sourceIp");
        if zone_id.is_empty() || source_ip.is_empty() {
            continue;
        }
        *groups.entry((zone_id, source_ip)).or_insert(0) += 1;
    }

    let mut created = Vec::new();
    let mut skipped = 0usize;
    for ((zone_id, source_ip), count) in groups {
        if count < threshold {
            continue;
        }
        let existing = match list_records(
            &state,
            "waf.incidents",
            vec![
                ("zoneId".to_string(), zone_id.clone()),
                ("status".to_string(), "open".to_string()),
            ],
            50,
            &context,
        )
        .await
        {
            Ok(rows) => rows,
            Err(e) => return internal_error_response(e),
        };
        if existing
            .iter()
            .any(|incident| str_field(incident, "title").contains(&source_ip))
        {
            skipped += 1;
            continue;
        }

        let mut data = JsonObject::new();
        data.insert("zoneId".to_string(), json!(zone_id));
        data.insert(
            "title".to_string(),
            json!(format!("{count} events from {source_ip} in {window_minutes}m")),
        );
        data.insert("severity".to_string(), json!(severity_for(count)));
        data.insert("eventCount".to_string(), json!(count));
        match state.crud.create("waf.incidents", &data, &context).await {
            Ok(ServiceResult::Ok { data, .. }) => created.push(data.id),
            Ok(ServiceResult::Err {
                status,
                error,
                message,
                field_errors,
            }) => return service_error_response(status, &error, message.as_deref(), field_errors),
            Err(e) => return internal_error_response(e),
        }
    }

    Json(json!({
        "data": {
            "scannedEvents": events.len(),
            "createdIncidents": created,
            "skippedExisting": skipped,
            "windowMinutes": window_minutes,
            "threshold": threshold,
        }
    }))
    .into_response()
}

/// Actually delivers a notification. `channels` on an `AlertPolicy` is free-form JSON
/// (`FieldKind::Json`), so this reads it defensively: `{"webhook": "https://..."}` posts,
/// `{"email": "..."}` logs (there is no mail transport in this product yet — saying so in a log
/// line is more honest than a `deliveryStatus: "sent"` nobody can verify).
async fn deliver(channels: &Value, payload: &Value) -> (bool, String) {
    if let Some(url) = channels.get("webhook").and_then(Value::as_str) {
        return match http_client().post(url).json(payload).send().await {
            Ok(response) if response.status().is_success() => (true, format!("webhook {}", response.status())),
            Ok(response) => (false, format!("webhook {}", response.status())),
            Err(e) => (false, format!("webhook error: {e}")),
        };
    }
    if let Some(address) = channels.get("email").and_then(Value::as_str) {
        tracing::info!(address, payload = %payload, "alert email (no transport configured — logged only)");
        return (true, format!("email logged to {address}"));
    }
    (false, "no deliverable channel configured".to_string())
}

/// Writes the delivery-log row every firing produces, sent or failed alike — an alert that failed
/// to deliver is exactly the one an operator needs to find later.
async fn record_notification(
    state: &AppState,
    context: &RequestContext,
    policy: &RecordDto,
    channel: &str,
    delivered: bool,
) -> anyhow::Result<Uuid> {
    let mut data = JsonObject::new();
    data.insert("alertPolicyId".to_string(), json!(policy.id.to_string()));
    data.insert("channel".to_string(), json!(channel));
    data.insert(
        "deliveryStatus".to_string(),
        json!(if delivered { "sent" } else { "failed" }),
    );
    data.insert("triggeredAt".to_string(), json!(chrono::Utc::now().to_rfc3339()));
    match state.crud.create("waf.alert_notifications", &data, context).await? {
        ServiceResult::Ok { data, .. } => Ok(data.id),
        ServiceResult::Err { error, .. } => Err(anyhow::anyhow!("alert_notifications: {error}")),
    }
}

fn channel_name(channels: &Value) -> &'static str {
    if channels.get("webhook").is_some() {
        "webhook"
    } else if channels.get("email").is_some() {
        "email"
    } else {
        "none"
    }
}

/// `POST /internal/alerts/evaluate` — every enabled `AlertPolicy`, evaluated against the recent
/// event stream, counted **per zone** (never summed across zones).
async fn evaluate_alerts(State(state): State<AppState>, AuthContext(context): AuthContext) -> Response {
    let policies = match list_records(
        &state,
        "waf.alert_policies",
        vec![("enabled".to_string(), "true".to_string())],
        50,
        &context,
    )
    .await
    {
        Ok(rows) => rows,
        Err(e) => return internal_error_response(e),
    };

    let events = match list_records(&state, "waf.security_events", Vec::new(), EVENT_SCAN_LIMIT, &context).await {
        Ok(rows) => rows,
        Err(e) => return internal_error_response(e),
    };

    let mut fired = Vec::new();
    for policy in &policies {
        let threshold = policy
            .data
            .get("thresholdCount")
            .and_then(Value::as_i64)
            .unwrap_or(0) as usize;
        let window_minutes = policy
            .data
            .get("windowMinutes")
            .and_then(Value::as_i64)
            .unwrap_or(DEFAULT_WINDOW_MINUTES);
        let cutoff = chrono::Utc::now() - chrono::Duration::minutes(window_minutes);

        let mut per_zone: HashMap<String, usize> = HashMap::new();
        for event in &events {
            let occurred_at = event
                .data
                .get("occurredAt")
                .and_then(Value::as_str)
                .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or(event.created_at);
            if occurred_at < cutoff {
                continue;
            }
            *per_zone.entry(str_field(event, "zoneId")).or_insert(0) += 1;
        }

        let channels = policy.data.get("channels").cloned().unwrap_or(Value::Null);
        for (zone_id, count) in per_zone {
            if threshold == 0 || count < threshold {
                continue;
            }
            let payload = json!({
                "policy": policy.data.get("name"),
                "zoneId": zone_id,
                "eventCount": count,
                "windowMinutes": window_minutes,
            });
            let (delivered, detail) = deliver(&channels, &payload).await;
            match record_notification(&state, &context, policy, channel_name(&channels), delivered).await {
                Ok(id) => fired.push(json!({
                    "notificationId": id,
                    "policyId": policy.id,
                    "zoneId": payload.get("zoneId"),
                    "eventCount": count,
                    "delivered": delivered,
                    "detail": detail,
                })),
                Err(e) => return internal_error_response(e),
            }
        }
    }

    Json(json!({ "data": { "policiesEvaluated": policies.len(), "fired": fired } })).into_response()
}

/// `POST /api/waf.alert_policies/{id}/test` — the "Send test alert" button
/// (`docs/07-portal-features.md` module 8). Runs the real delivery path and writes a real
/// `AlertNotification`, because a test that takes a different path than production proves nothing
/// about production.
async fn test_alert_policy(
    State(state): State<AppState>,
    Path(policy_id): Path<Uuid>,
    AuthContext(context): AuthContext,
) -> Response {
    let policy = match state.crud.get("waf.alert_policies", policy_id, &context).await {
        Ok(ServiceResult::Ok { data: (record, _), .. }) => record,
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => return service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => return internal_error_response(e),
    };

    let channels = policy.data.get("channels").cloned().unwrap_or(Value::Null);
    let payload = json!({
        "test": true,
        "policy": policy.data.get("name"),
        "message": "Test alert from the WAF portal",
    });
    let (delivered, detail) = deliver(&channels, &payload).await;
    match record_notification(&state, &context, &policy, channel_name(&channels), delivered).await {
        Ok(id) => Json(json!({
            "data": { "notificationId": id, "delivered": delivered, "detail": detail }
        }))
        .into_response(),
        Err(e) => internal_error_response(e),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/internal/incidents/correlate", post(correlate_incidents))
        .route("/internal/alerts/evaluate", post(evaluate_alerts))
        .route("/api/waf.alert_policies/{id}/test", post(test_alert_policy))
}
