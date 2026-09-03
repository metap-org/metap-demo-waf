//! Custom (non-generic-CRUD) HTTP surface for `scanning-service`.
//!
//! Scope boundary first, because it is easy to get wrong: **this service does not scan.** Running
//! a DAST engine against a customer's site is a separate execution concern, the same way
//! `edge-plane` is separate from the portal — `docs/13-screen-api-map.md` corrected exactly this
//! misreading on 2026-08-30. What lives here is the portal-side half:
//!
//! - **`/api/waf.scan_jobs/{id}/run`** — hands a queued job to whatever scanner backend is
//!   configured (`SCANNER_URL`) and moves the job's workflow along. With no scanner configured it
//!   queues the job and says so, rather than pretending a scan happened.
//! - **`/internal/scan-jobs/{id}/findings`** — the callback a scanner posts results to. Creates
//!   `ScanFinding` records and completes (or fails) the job in one call, so a scanner integration
//!   needs exactly one endpoint and no knowledge of the workflow's state names.
//!
//! Both go through `CrudService` (never SQL) so findings get the same validation, permission and
//! outbox treatment any portal-created record gets.

use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use metap::crud::{JsonObject, RecordDto, ServiceResult};
use metap::http::error::{internal_error_response, service_error_response};
use metap::permission::RequestContext;
use metap::prelude::{AppState, AuthContext};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

/// Where a scan request is handed off. Unset (the default) means "no scanner deployed" — the job
/// still queues, which is the honest state: something asked for a scan and nothing is going to
/// run it yet.
fn scanner_url() -> Option<String> {
    std::env::var("SCANNER_URL").ok().filter(|s| !s.is_empty())
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

/// Applies one workflow transition, mapping a service-level failure into `anyhow`. Transition
/// names come from `entities/scan_job_entity.rs`'s workflow (`run`/`start`/`complete`/`fail`) —
/// this module never writes `status` directly, so guards, audit rows and outbox events all still
/// happen.
async fn transition(
    state: &AppState,
    job: &RecordDto,
    action: &str,
    context: &RequestContext,
) -> anyhow::Result<RecordDto> {
    match state
        .crud
        .transition("waf.scan_jobs", job.id, action, job.version, None, context)
        .await?
    {
        ServiceResult::Ok { data, .. } => Ok(data),
        ServiceResult::Err { error, message, .. } => Err(anyhow::anyhow!(
            "transition {action}: {error}{}",
            message.map(|m| format!(" ({m})")).unwrap_or_default()
        )),
    }
}

async fn load_job(state: &AppState, id: Uuid, context: &RequestContext) -> Result<RecordDto, Response> {
    match state.crud.get("waf.scan_jobs", id, context).await {
        Ok(ServiceResult::Ok { data: (record, _), .. }) => Ok(record),
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => Err(service_error_response(
            status,
            &error,
            message.as_deref(),
            field_errors,
        )),
        Err(e) => Err(internal_error_response(e)),
    }
}

/// `POST /api/waf.scan_jobs/{id}/run` — the portal's "Run scan now" button
/// (`docs/07-portal-features.md` module 5).
async fn run_scan_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
    AuthContext(context): AuthContext,
) -> Response {
    let job = match load_job(&state, job_id, &context).await {
        Ok(job) => job,
        Err(response) => return response,
    };

    // `run` is valid from `idle`/`completed`/`failed` — a job already `queued` or `running` has
    // nothing to do, and saying so beats a workflow error the UI would have to translate.
    let status = job.status.clone().unwrap_or_default();
    if status == "queued" || status == "running" {
        return service_error_response(
            409,
            "scan_already_in_progress",
            Some(&format!("This scan job is already {status}.")),
            None,
        );
    }

    let queued = match transition(&state, &job, "run", &context).await {
        Ok(record) => record,
        Err(e) => return internal_error_response(e),
    };

    let Some(scanner) = scanner_url() else {
        return Json(json!({
            "data": {
                "job": queued,
                "dispatched": false,
                "detail": "Queued. No scanner backend is configured (SCANNER_URL unset), so nothing will pick this up yet.",
            }
        }))
        .into_response();
    };

    // Fire-and-forget by design: the scanner reports back through the findings callback below,
    // so this request doesn't wait out a scan that can take minutes.
    let dispatch = http_client()
        .post(format!("{scanner}/scans"))
        .json(&json!({
            "scanJobId": queued.id,
            "zoneId": queued.data.get("zoneId"),
            "scanType": queued.data.get("scanType"),
            "callbackPath": format!("/internal/scan-jobs/{}/findings", queued.id),
        }))
        .send()
        .await;

    match dispatch {
        Ok(response) if response.status().is_success() => Json(json!({
            "data": { "job": queued, "dispatched": true, "detail": "Handed to scanner." }
        }))
        .into_response(),
        Ok(response) => {
            let detail = format!("Scanner rejected the request ({})", response.status());
            match transition(&state, &queued, "start", &context).await {
                Ok(running) => match transition(&state, &running, "fail", &context).await {
                    Ok(failed) => {
                        Json(json!({ "data": { "job": failed, "dispatched": false, "detail": detail } }))
                            .into_response()
                    }
                    Err(e) => internal_error_response(e),
                },
                Err(e) => internal_error_response(e),
            }
        }
        Err(e) => service_error_response(
            502,
            "scanner_unavailable",
            Some(&format!("Could not reach the scanner: {e}")),
            None,
        ),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindingInput {
    severity: String,
    category: String,
    endpoint: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindingsBody {
    #[serde(default)]
    findings: Vec<FindingInput>,
    /// `false` completes the job as failed instead — a scanner that crashed halfway still reports,
    /// and a job stuck in `running` forever is worse than a job marked failed.
    #[serde(default = "default_true")]
    succeeded: bool,
}

fn default_true() -> bool {
    true
}

/// `POST /internal/scan-jobs/{id}/findings` — scanner callback.
///
/// Findings are created before the job is completed, so a portal user who sees `completed` always
/// sees the findings that go with it (the reverse order would show a completed scan with an empty
/// findings list for as long as the writes took).
async fn submit_findings(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
    AuthContext(context): AuthContext,
    Json(body): Json<FindingsBody>,
) -> Response {
    let job = match load_job(&state, job_id, &context).await {
        Ok(job) => job,
        Err(response) => return response,
    };

    let now = chrono::Utc::now().to_rfc3339();
    let mut created = Vec::new();
    for finding in &body.findings {
        let mut data = JsonObject::new();
        data.insert("scanJobId".to_string(), json!(job.id.to_string()));
        data.insert("severity".to_string(), json!(finding.severity));
        data.insert("category".to_string(), json!(finding.category));
        data.insert("endpoint".to_string(), json!(finding.endpoint));
        if let Some(description) = &finding.description {
            data.insert("description".to_string(), json!(description));
        }
        data.insert("firstSeenAt".to_string(), json!(now));
        data.insert("lastSeenAt".to_string(), json!(now));
        match state.crud.create("waf.scan_findings", &data, &context).await {
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

    // The job may still be `queued` if the scanner was quick — walk it through `start` first so
    // the transition graph is respected rather than jumped over.
    let mut current = job;
    if current.status.as_deref() == Some("queued") {
        current = match transition(&state, &current, "start", &context).await {
            Ok(record) => record,
            Err(e) => return internal_error_response(e),
        };
    }
    let finish = if body.succeeded { "complete" } else { "fail" };
    let finished = match transition(&state, &current, finish, &context).await {
        Ok(record) => record,
        Err(e) => return internal_error_response(e),
    };

    // `lastRunAt` isn't part of any transition's `set_fields`, so it's written here — the one
    // field this flow owns that the workflow itself doesn't.
    let mut patch = JsonObject::new();
    patch.insert("lastRunAt".to_string(), json!(now));
    let job_after = match state
        .crud
        .update("waf.scan_jobs", finished.id, finished.version, &patch, &context)
        .await
    {
        Ok(ServiceResult::Ok { data, .. }) => Value::from(serde_json::to_value(&data.data).unwrap_or(Value::Null)),
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => return service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => return internal_error_response(e),
    };

    Json(json!({
        "data": { "jobId": finished.id, "job": job_after, "createdFindings": created }
    }))
    .into_response()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/waf.scan_jobs/{id}/run", post(run_scan_job))
        .route("/internal/scan-jobs/{id}/findings", post(submit_findings))
}
