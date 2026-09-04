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
//! - **`zone_delete_guard`** — the cross-service reference check `CrudService`'s own
//!   `find_referencing_record` structurally cannot do (see that function's note below).
//!
//! All of it is mounted by `main.rs` through `build_router`'s `extra_routes` parameter, so it
//! gets the same CORS/rate-limit/tracing/security-header treatment as every core route.

use std::time::{Duration, Instant};

use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
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

/// Forwards the caller's own credentials to a sibling service.
///
/// The three WAF services share one JWT keypair (found the hard way — see the roadmap entry for
/// Phase 61's T6: a gateway with its own keypair couldn't decode a real login token), so the
/// caller's token is already valid at `scanning-service`/`alerting-service`. That means no
/// service account, no `ServiceTokenSource`, and — more importantly — the sibling call runs as
/// the *real* caller, so it can never see records that caller couldn't have listed itself.
///
/// Both `authorization` and `cookie` are forwarded because either can carry the session since
/// the cookie-session migration (`metap`'s Phase 64): a browser-driven delete authenticates by
/// cookie, a script-driven one by bearer.
fn forward_auth(headers: &HeaderMap) -> Vec<(String, String)> {
    ["authorization", "cookie"]
        .iter()
        .filter_map(|name| {
            headers
                .get(*name)
                .and_then(|v| v.to_str().ok())
                .map(|v| ((*name).to_string(), v.to_string()))
        })
        .collect()
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

/// Does any record of `entity` at `base_url` still point at `zone_id`?
///
/// One `limit=1` list call per sibling entity — existence, not a count, is all the guard needs.
/// A transport failure returns `Err`: the guard treats "I could not check" as blocking, since
/// letting a delete through because a sibling service was down is the exact silent orphan this
/// whole check exists to prevent.
async fn has_references(
    client: &reqwest::Client,
    base_url: &str,
    entity: &str,
    zone_id: Uuid,
    auth: &[(String, String)],
) -> Result<bool, String> {
    let url = format!("{base_url}/api/{entity}?zoneId={zone_id}&limit=1");
    let mut request = client.get(&url);
    for (name, value) in auth {
        request = request.header(name, value);
    }
    let response = request
        .send()
        .await
        .map_err(|e| format!("{entity}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("{entity}: upstream returned {}", response.status()));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("{entity}: malformed response ({e})"))?;
    Ok(body
        .get("data")
        .and_then(Value::as_array)
        .is_some_and(|rows| !rows.is_empty()))
}

/// Blocks `DELETE /api/waf.zones/{id}` while another *service* still holds records pointing at
/// that zone.
///
/// Why a middleware rather than an overriding route: `metap-http` registers `GET`/`PATCH`/
/// `DELETE` together on `/api/{entity}/{id}`. Registering a static `/api/waf.zones/{id}` with
/// only `DELETE` would win the path match for all three methods and turn `GET`/`PATCH` on a zone
/// into `405`s — axum matches path first, then method, with no fallthrough to a less specific
/// path. A middleware adds the check without touching the routing table at all.
///
/// Why the check is needed: `CrudService::delete`'s own reference guard
/// (`find_referencing_record`) walks the *running process's* `MetadataRegistry`. Since the pillar
/// split this service registers only its own three entities, so that guard has never been able to
/// see `waf.scan_jobs`/`waf.incidents`/`waf.security_events` — deleting a zone silently orphaned
/// every one of them.
///
/// Known limitation, accepted deliberately: the check and the delete are not one transaction
/// across two services, so a scan job created in the gap between them still orphans. Zone
/// deletion is a rare admin action and the window is milliseconds; closing it properly needs a
/// distributed lock or two-phase delete, which is not worth it here — see this repo's roadmap
/// entry for the discussion.
pub async fn zone_delete_guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let is_zone_delete = request.method() == Method::DELETE
        && path.starts_with("/api/waf.zones/")
        && path.matches('/').count() == 3;
    if !is_zone_delete {
        return next.run(request).await;
    }
    let Some(zone_id) = path
        .rsplit('/')
        .next()
        .and_then(|raw| Uuid::parse_str(raw).ok())
    else {
        // Not a well-formed id — let the real route produce its own 400/404 rather than
        // inventing a different error here.
        return next.run(request).await;
    };

    let auth = forward_auth(request.headers());
    let client = http_client();
    let checks = [
        (scanning_url(), "waf.scan_jobs"),
        (alerting_url(), "waf.incidents"),
        (alerting_url(), "waf.security_events"),
    ];
    for (base_url, entity) in checks {
        match has_references(&client, &base_url, entity, zone_id, &auth).await {
            Ok(true) => {
                tracing::warn!(
                    zone_id = %zone_id,
                    referencing_entity = entity,
                    "zone delete rejected: still referenced by another service"
                );
                return service_error_response(
                    409,
                    "record_referenced",
                    Some(&format!(
                        "This zone is still referenced by \"{entity}\" and cannot be deleted."
                    )),
                    None,
                );
            }
            Ok(false) => {}
            Err(reason) => {
                tracing::error!(zone_id = %zone_id, reason, "zone delete blocked: reference check failed");
                return service_error_response(
                    503,
                    "reference_check_unavailable",
                    Some(&format!(
                        "Could not verify cross-service references ({reason}); refusing to delete."
                    )),
                    None,
                );
            }
        }
    }
    let _ = state;
    next.run(request).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerifyDnsBody {
    /// Optional override for local/manual testing — normally the check reads the zone's own
    /// `verificationToken` and never trusts a caller-supplied value.
    #[serde(default)]
    expected_token: Option<String>,
}

/// Answers one DoH question, returning every answer record's data string.
async fn dns_lookup(client: &reqwest::Client, name: &str, record_type: &str) -> Result<Vec<String>, String> {
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

/// `POST /api/waf.zones/{id}/verify-dns` — domain-ownership check (`docs/06`) plus the
/// informational routing check (`docs/11`), in one call because both read the same zone's DNS.
///
/// Ownership sets `verificationStatus` to `verified` only on a real match of the zone's own
/// `verificationToken` in a `_waf-verify.<hostname>` TXT record. Routing sets `dnsRoutingStatus`
/// independently — it never gates activation, it only tells the customer whether traffic is
/// actually reaching the edge yet.
async fn verify_dns(
    State(state): State<AppState>,
    Path(zone_id): Path<Uuid>,
    AuthContext(context): AuthContext,
    body: Option<Json<VerifyDnsBody>>,
) -> Response {
    let zone = match state.crud.get("waf.zones", zone_id, &context).await {
        Ok(ServiceResult::Ok { data: (record, _), .. }) => record,
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
        return service_error_response(400, "validation_failed", Some("Zone has no hostname."), None);
    }
    let expected_token = body
        .and_then(|Json(b)| b.expected_token)
        .or_else(|| {
            zone.data
                .get("verificationToken")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();

    let client = http_client();
    let txt = dns_lookup(&client, &format!("_waf-verify.{hostname}"), "TXT")
        .await
        .unwrap_or_default();
    let cname = dns_lookup(&client, &hostname, "CNAME").await.unwrap_or_default();

    let ownership_ok = !expected_token.is_empty() && txt.iter().any(|record| record == &expected_token);
    let target = edge_cname_target();
    let routed = cname.iter().any(|record| record.ends_with(&target));

    let mut patch = metap::crud::JsonObject::new();
    if ownership_ok {
        patch.insert("verificationStatus".to_string(), json!("verified"));
    }
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
        .update("waf.zones", zone_id, zone.version, &patch, &context)
        .await
    {
        Ok(ServiceResult::Ok { data, .. }) => Json(json!({
            "data": {
                "zone": data,
                "ownershipVerified": ownership_ok,
                "dnsRouted": routed,
                "checked": { "txt": txt, "cname": cname, "expectedTarget": target },
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
        Ok(ServiceResult::Ok { data: (record, _), .. }) => record,
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
        return service_error_response(400, "validation_failed", Some("Zone has no origin address."), None);
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
    async fn any_for_zone(state: &AppState, entity: &str, zone_id: Uuid, context: &metap::permission::RequestContext) -> anyhow::Result<bool> {
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
        Ok(ServiceResult::Ok { data: (record, _), .. }) => record,
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
        return Json(json!({ "data": { "hasConfig": has_config, "changed": false } })).into_response();
    }

    let mut patch = metap::crud::JsonObject::new();
    patch.insert("hasConfig".to_string(), json!(has_config));
    match state
        .crud
        .update("waf.zones", zone_id, zone.version, &patch, &context)
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
    (StatusCode::OK, Json(json!({ "data": { "self": "ok", "upstreams": checks } }))).into_response()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/waf.zones/{id}/verify-dns", post(verify_dns))
        .route("/api/waf.zones/{id}/test-origin", post(test_origin))
        .route("/api/waf.zones/{id}/sync-config-state", post(sync_config_state))
        .route("/internal/health/deep", axum::routing::get(deep_health))
}
