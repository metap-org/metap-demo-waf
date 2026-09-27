//! Custom (non-generic-CRUD) HTTP surface for `zones-service`.
//!
//! Everything here is a case `docs/13-screen-api-map.md` already flagged as "phải tự code" —
//! work that isn't reading or writing one record and therefore isn't something `metap`'s generic
//! `/api/{entity}` routes can generate:
//!
//! - **`verify-dns`** / **`test-origin`** — call *out* (DNS resolver, the customer's own origin),
//!   which no CRUD route does.
//! - **`sync-config-state`** — recomputes `Zone.hasConfig` by counting the zone's related
//!   `DdosPolicy`/`FirewallRule` records. `PolicyCondition` (the grammar a workflow guard is
//!   written in) has no count operator over related records, which is exactly why
//!   `entities/zone_entity.rs` documents `hasConfig` as a technical field "the app layer flips"
//!   — this is that app layer. Until this existed the `activate` guard could never pass, so no
//!   zone could leave `pending` at all.
//!
//! All of it is mounted by `main.rs` through `build_router`'s `extra_routes` parameter, so it
//! gets the same CORS/rate-limit/tracing/security-header treatment as every core route.
//!
//! **The 4 create/update/delete guards that used to live in this file
//! (`zone_domain_guard`/`firewall_rule_match_condition_guard`/`ip_access_list_value_guard`/
//! `zone_delete_guard`) are gone from here as of 2026-09-27** — see `../guarded_backend.rs`'s doc
//! comment for where 3 of them live now and why, and for the 1 (`zone_delete_guard`) that's a
//! known, deliberately unclosed gap.

use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use metap::crud::ServiceResult;
use metap::http::error::{internal_error_response, service_error_response};
use metap::prelude::{AppState, AuthContext};
use metap::query::ListInput;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

/// Where the sibling pillar services live. Same env-var shape `web/vite.config.ts` uses for its
/// dev-server routing, so one deployment's addresses are spelled the same way everywhere.
fn scanning_url() -> String {
    std::env::var("SCANNING_URL").unwrap_or_else(|_| "http://localhost:3010".to_string())
}

fn alerting_url() -> String {
    std::env::var("ALERTING_URL").unwrap_or_else(|_| "http://localhost:3020".to_string())
}

/// Public DNS-over-HTTPS resolver used by `verify-dns`. Deliberately HTTP-based rather than a
/// DNS client library: this service already has an HTTP client, a DoH endpoint needs no new
/// dependency or UDP egress, and swapping resolvers (or pointing at an internal one) is then an
/// env change rather than a code change.
fn doh_url() -> String {
    std::env::var("DOH_RESOLVER_URL").unwrap_or_else(|_| "https://dns.google/resolve".to_string())
}

/// The hostname a verified zone's DNS is expected to point at once the customer has actually
/// routed traffic through the edge (`docs/11-onboarding-dns-resolution.md`'s `dnsRoutingStatus`).
fn edge_cname_target() -> String {
    std::env::var("EDGE_CNAME_TARGET").unwrap_or_else(|_| "edge.waf.local".to_string())
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        // An origin under test is attacker-adjacent input (a customer types the address) and a
        // redirect chain is not what "is this origin reachable" is asking about.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_default()
}

// `zone_delete_guard`/`zone_domain_guard`/`firewall_rule_match_condition_guard`/
// `ip_access_list_value_guard` used to live here as axum middleware, gated on the REST paths
// (`/api/waf.zones`, `/api/waf.firewall_rules`, `/api/waf.ip_access_lists`) `metap` core removed
// entirely 2026-09-21 — dead code ever since, since this service never mounted GraphQL either
// (see `../guarded_backend.rs`'s own doc comment for the full story, found live 2026-09-27,
// `../../../metap-docs/docs/roadmap/99-zones-service-guard-reachability-fix.md`). 3 of the 4 now
// live in `GuardedZonesBackend` instead, wrapping the `RecordBackend` gRPC actually serves;
// `zone_delete_guard`'s cross-service reference check is a known, deliberately unclosed gap
// (that module's doc comment explains why porting it isn't a one-line fix).

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerifyDnsBody {
    /// Optional override for local/manual testing — normally the check reads the zone's own
    /// `verificationToken` and never trusts a caller-supplied value.
    #[serde(default)]
    expected_token: Option<String>,
}

/// Answers one DoH question, returning every answer record's data string.
async fn dns_lookup(
    client: &reqwest::Client,
    name: &str,
    record_type: &str,
) -> Result<Vec<String>, String> {
    let response = client
        .get(doh_url())
        .query(&[("name", name), ("type", record_type)])
        .header("accept", "application/dns-json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    Ok(body
        .get("Answer")
        .and_then(Value::as_array)
        .map(|answers| {
            answers
                .iter()
                .filter_map(|a| a.get("data").and_then(Value::as_str))
                // DoH returns TXT strings wrapped in quotes; a CNAME comes back with a trailing
                // dot. Normalising both here keeps the comparison below about content.
                .map(|d| d.trim_matches('"').trim_end_matches('.').to_string())
                .collect()
        })
        .unwrap_or_default())
}

/// `POST /api/waf.zones/{id}/verify-dns` — the informational DNS **routing** check (`docs/11`):
/// is this zone's own hostname actually pointed at edge-plane yet? Never gates activation, purely
/// informational.
///
/// **Ownership verification moved to the parent `Domain`** (`verify_domain_dns` below,
/// `domain_entity.rs`'s doc comment) — a zone's own hostname can still have its own CNAME target
/// even when subdomains share one apex domain, so routing stays a per-zone check, unlike
/// ownership (proven once for the whole apex domain).
async fn verify_dns(
    State(state): State<AppState>,
    Path(zone_id): Path<Uuid>,
    AuthContext(context): AuthContext,
) -> Response {
    let zone = match state.crud.get("waf.zones", zone_id, &context).await {
        Ok(ServiceResult::Ok {
            data: (record, _), ..
        }) => record,
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => return service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => return internal_error_response(e),
    };

    let hostname = zone
        .data
        .get("hostname")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if hostname.is_empty() {
        return service_error_response(
            400,
            "validation_failed",
            Some("Zone has no hostname."),
            None,
        );
    }

    let client = http_client();
    let cname = dns_lookup(&client, &hostname, "CNAME")
        .await
        .unwrap_or_default();

    let target = edge_cname_target();
    let routed = cname.iter().any(|record| record.ends_with(&target));

    let mut patch = metap::crud::JsonObject::new();
    patch.insert(
        "dnsRoutingStatus".to_string(),
        json!(if routed { "routed" } else { "notRouted" }),
    );
    patch.insert(
        "lastDnsCheckAt".to_string(),
        json!(chrono::Utc::now().to_rfc3339()),
    );

    match state
        .crud
        .update("waf.zones", zone_id, zone.version, &patch, &context, None)
        .await
    {
        Ok(ServiceResult::Ok { data, .. }) => Json(json!({
            "data": {
                "zone": data,
                "dnsRouted": routed,
                "checked": { "cname": cname, "expectedTarget": target },
            }
        }))
        .into_response(),
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => internal_error_response(e),
    }
}

/// `POST /api/waf.domains/{id}/verify-dns` — domain-**ownership** check (`docs/06`), proven once
/// per apex domain rather than once per `Zone` (`domain_entity.rs`'s doc comment explains why this
/// moved off `Zone`).
///
/// On a real match of the domain's own `verificationToken` in a `_waf-verify.<apexDomain>` TXT
/// record, this both marks the `Domain` itself verified **and cascades** that onto every `Zone`
/// under it — writing each zone's own `verificationStatus` mirror field directly, the same
/// "app layer keeps a technical field in sync" pattern `hasConfig`/`sync_config_state` already
/// use, since a workflow guard (`zone_entity.rs`'s `activate` transition) cannot read a related
/// entity's field itself.
async fn verify_domain_dns(
    State(state): State<AppState>,
    Path(domain_id): Path<Uuid>,
    AuthContext(context): AuthContext,
    body: Option<Json<VerifyDnsBody>>,
) -> Response {
    let domain = match state.crud.get("waf.domains", domain_id, &context).await {
        Ok(ServiceResult::Ok {
            data: (record, _), ..
        }) => record,
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => return service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => return internal_error_response(e),
    };

    let apex_domain = domain
        .data
        .get("apexDomain")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if apex_domain.is_empty() {
        return service_error_response(
            400,
            "validation_failed",
            Some("Domain has no apexDomain."),
            None,
        );
    }
    let expected_token = body
        .and_then(|Json(b)| b.expected_token)
        .or_else(|| {
            domain
                .data
                .get("verificationToken")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();

    let client = http_client();
    let txt = dns_lookup(&client, &format!("_waf-verify.{apex_domain}"), "TXT")
        .await
        .unwrap_or_default();
    let ownership_ok =
        !expected_token.is_empty() && txt.iter().any(|record| record == &expected_token);

    if !ownership_ok {
        return Json(json!({
            "data": {
                "domain": domain,
                "ownershipVerified": false,
                "checked": { "txt": txt, "expectedToken": expected_token },
            }
        }))
        .into_response();
    }

    let mut patch = metap::crud::JsonObject::new();
    patch.insert("verificationStatus".to_string(), json!("verified"));
    let updated_domain = match state
        .crud
        .update(
            "waf.domains",
            domain_id,
            domain.version,
            &patch,
            &context,
            None,
        )
        .await
    {
        Ok(ServiceResult::Ok { data, .. }) => data,
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => return service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => return internal_error_response(e),
    };

    // Cascade onto every zone under this domain — best-effort per zone (one failing zone must not
    // stop the others, and must not undo the domain's own now-verified status).
    let list_input = ListInput {
        limit: 100,
        filters: vec![("domainId".to_string(), domain_id.to_string())],
        ..Default::default()
    };
    if let Ok(ServiceResult::Ok { data: zones, .. }) =
        state.crud.list("waf.zones", &list_input, &context).await
    {
        for zone in zones {
            let mut zone_patch = metap::crud::JsonObject::new();
            zone_patch.insert("verificationStatus".to_string(), json!("verified"));
            if let Err(e) = state
                .crud
                .update(
                    "waf.zones",
                    zone.id,
                    zone.version,
                    &zone_patch,
                    &context,
                    None,
                )
                .await
            {
                tracing::error!(
                    zone_id = %zone.id,
                    domain_id = %domain_id,
                    error = %e,
                    "domain verified but cascading verificationStatus onto this zone failed"
                );
            }
        }
    }

    Json(json!({
        "data": {
            "domain": updated_domain,
            "ownershipVerified": true,
            "checked": { "txt": txt, "expectedToken": expected_token },
        }
    }))
    .into_response()
}

/// `POST /api/waf.zones/{id}/test-origin` — "can we actually reach the origin the customer gave
/// us", the one-shot connectivity check `docs/11-onboarding-dns-resolution.md` describes. Not
/// stored on the zone: it is a point-in-time probe, and continuous origin health monitoring is a
/// separate (still unbuilt) feature — see `docs/14-cloudflare-gap-analysis.md`.
async fn test_origin(
    State(state): State<AppState>,
    Path(zone_id): Path<Uuid>,
    AuthContext(context): AuthContext,
) -> Response {
    let zone = match state.crud.get("waf.zones", zone_id, &context).await {
        Ok(ServiceResult::Ok {
            data: (record, _), ..
        }) => record,
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => return service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => return internal_error_response(e),
    };

    let origin = zone
        .data
        .get("originAddress")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if origin.is_empty() {
        return service_error_response(
            400,
            "validation_failed",
            Some("Zone has no origin address."),
            None,
        );
    }
    // A customer types `1.2.3.4` or `origin.example.com` as often as a full URL.
    let url = if origin.starts_with("http://") || origin.starts_with("https://") {
        origin.clone()
    } else {
        format!("https://{origin}")
    };

    let started = Instant::now();
    let result = http_client().get(&url).send().await;
    let elapsed_ms = started.elapsed().as_millis() as u64;

    let payload = match result {
        Ok(response) => json!({
            "reachable": true,
            "status": response.status().as_u16(),
            "latencyMs": elapsed_ms,
            "url": url,
        }),
        Err(e) => json!({
            "reachable": false,
            "error": e.to_string(),
            "latencyMs": elapsed_ms,
            "url": url,
        }),
    };
    Json(json!({ "data": payload })).into_response()
}

/// `POST /api/waf.zones/{id}/sync-config-state` — recomputes `hasConfig` from the zone's actual
/// `DdosPolicy`/`FirewallRule` records and writes it back.
///
/// The portal calls this right after creating or deleting a policy/rule. It is idempotent and
/// derives everything it writes, so calling it at any other time (or twice) is harmless — which
/// is the property that lets the portal fire it optimistically rather than tracking whether a
/// given mutation was the first or last one for that zone.
async fn sync_config_state(
    State(state): State<AppState>,
    Path(zone_id): Path<Uuid>,
    AuthContext(context): AuthContext,
) -> Response {
    async fn any_for_zone(
        state: &AppState,
        entity: &str,
        zone_id: Uuid,
        context: &metap::permission::RequestContext,
    ) -> anyhow::Result<bool> {
        let input = ListInput {
            limit: 1,
            filters: vec![("zoneId".to_string(), zone_id.to_string())],
            ..Default::default()
        };
        match state.crud.list(entity, &input, context).await? {
            ServiceResult::Ok { data, .. } => Ok(!data.is_empty()),
            // A caller who cannot list rules cannot be told a zone has none either — surfacing
            // "false" here would silently flip `hasConfig` off on a permission error.
            ServiceResult::Err { error, .. } => Err(anyhow::anyhow!("{entity}: {error}")),
        }
    }

    let has_config = match (
        any_for_zone(&state, "waf.ddos_policies", zone_id, &context).await,
        any_for_zone(&state, "waf.firewall_rules", zone_id, &context).await,
    ) {
        (Ok(a), Ok(b)) => a || b,
        (Err(e), _) | (_, Err(e)) => return internal_error_response(e),
    };

    let zone = match state.crud.get("waf.zones", zone_id, &context).await {
        Ok(ServiceResult::Ok {
            data: (record, _), ..
        }) => record,
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => return service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => return internal_error_response(e),
    };

    if zone.data.get("hasConfig").and_then(Value::as_bool) == Some(has_config) {
        // Already correct — skip the write so this doesn't bump `version`/`updatedAt` on every
        // call and turn an idempotent sync into a source of version conflicts for the portal.
        return Json(json!({ "data": { "hasConfig": has_config, "changed": false } }))
            .into_response();
    }

    let mut patch = metap::crud::JsonObject::new();
    patch.insert("hasConfig".to_string(), json!(has_config));
    match state
        .crud
        .update("waf.zones", zone_id, zone.version, &patch, &context, None)
        .await
    {
        Ok(ServiceResult::Ok { .. }) => {
            Json(json!({ "data": { "hasConfig": has_config, "changed": true } })).into_response()
        }
        Ok(ServiceResult::Err {
            status,
            error,
            message,
            field_errors,
        }) => service_error_response(status, &error, message.as_deref(), field_errors),
        Err(e) => internal_error_response(e),
    }
}

/// `GET /internal/health/deep` — liveness of this service *plus* the two siblings it now depends
/// on for the delete guard. Plain `/health` (generic, from `metap-http`) stays what a load
/// balancer polls; this is for an operator asking why a zone delete just returned `503`.
async fn deep_health() -> Response {
    let client = http_client();
    let mut checks = serde_json::Map::new();
    for (name, base) in [("scanning", scanning_url()), ("alerting", alerting_url())] {
        let ok = client
            .get(format!("{base}/health"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        checks.insert(name.to_string(), json!({ "reachable": ok, "url": base }));
    }
    (
        StatusCode::OK,
        Json(json!({ "data": { "self": "ok", "upstreams": checks } })),
    )
        .into_response()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/waf.zones/{id}/verify-dns", post(verify_dns))
        .route("/api/waf.domains/{id}/verify-dns", post(verify_domain_dns))
        .route("/api/waf.zones/{id}/test-origin", post(test_origin))
        .route(
            "/api/waf.zones/{id}/sync-config-state",
            post(sync_config_state),
        )
        .route("/internal/health/deep", axum::routing::get(deep_health))
}
