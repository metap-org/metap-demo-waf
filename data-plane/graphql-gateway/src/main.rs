//! WAF's own GraphQL BFF — a thin wrapper around `metap`'s generic `metap-graphql-gateway`
//! library (same boot sequence, same 3 upstreams: zones-service/scanning-service/
//! alerting-service, see `../graphql-gateway/README.md` for the env config) that additionally
//! exposes the 7 custom, non-CRUD REST endpoints and the `aggregate` read every service has
//! (`docs/roadmap/70-aggregate-api.md`) as GraphQL fields, so the Customer Portal frontend
//! (`data-plane/web`) can reach 100% of its data needs through one GraphQL endpoint instead of a
//! REST/GraphQL split.
//!
//! **This binary, not `metap`'s generic gateway, is where this logic has to live.** None of these
//! 8 fields are `EntityDefinition` CRUD operations `metap-graphql`'s schema builder can synthesize
//! from metadata — they're real WAF business actions (DNS verification, scan dispatch, incident
//! correlation, ...) — and `metap-graphql`/`graphql-gateway` must stay entity-agnostic, the same
//! "no `metap-*` library crate gets business-entity knowledge" rule that keeps every REST route
//! for these same endpoints in each service's own `src/routes.rs`, not in `metap-http`.
//!
//! Every resolver here is a thin proxy: parse GraphQL args, forward the caller's own bearer token
//! (the same identity propagation `metap-grpc::GrpcBackend` already does for generic CRUD/list —
//! see `metap`'s `graphql-gateway/src/server.rs` doc comment) to the exact REST endpoint that
//! already implements the real logic, and hand back its JSON response verbatim as the schema's
//! `Json` scalar. No business logic is duplicated here.

use metap_graphql::{
    Field, FieldFuture, FieldValue, GqlError, GqlValue, InputValue, Object, ResolverContext,
    TypeRef, JSON_SCALAR,
};
use metap_graphql_gateway::config::{GatewayConfig, UpstreamConfig};
use metap_graphql_gateway::{schema_builder, server};
use metap_permission::RequestContext;
use serde_json::Value;

/// An upstream's REST base URL, derived from its `metadata_url` (`{base}/metadata/entities`) —
/// this gateway is never given a REST base directly, only the gRPC/metadata/login URLs the
/// generic boot sequence needs (see `metap`'s `graphql-gateway/src/config.rs`), so the custom
/// resolvers below derive it rather than requiring 3 more env vars that would just repeat
/// information already present in `UPSTREAM_N_METADATA_URL`.
fn rest_base_url<'a>(upstreams: &'a [UpstreamConfig], name: &str) -> &'a str {
    let upstream = upstreams
        .iter()
        .find(|u| u.name == name)
        .unwrap_or_else(|| panic!("no upstream named '{name}' configured — set UPSTREAM_N_NAME={name} (see .env.example)"));
    upstream
        .metadata_url
        .strip_suffix("/metadata/entities")
        .unwrap_or_else(|| {
            panic!(
                "upstream '{name}'s metadata_url must end in /metadata/entities, got '{}'",
                upstream.metadata_url
            )
        })
}

/// Which upstream owns a given `waf.*` entity, for the `aggregate` resolver's routing — hardcoded
/// rather than discovered at runtime (this binary already carries full WAF business knowledge by
/// design, see the module doc comment; the 9 entities across 3 services don't change without a
/// code change to the owning service anyway).
fn rest_base_for_entity<'a>(
    entity: &str,
    zones: &'a str,
    scanning: &'a str,
    alerting: &'a str,
) -> Result<&'a str, GqlError> {
    match entity {
        "waf.zones" | "waf.ddos_policies" | "waf.firewall_rules" => Ok(zones),
        "waf.scan_jobs" | "waf.scan_findings" => Ok(scanning),
        "waf.alert_policies"
        | "waf.alert_notifications"
        | "waf.incidents"
        | "waf.security_events" => Ok(alerting),
        other => Err(GqlError::new(format!("unknown entity '{other}'"))),
    }
}

/// Forwards the caller's own bearer token to a `POST` REST endpoint and returns its JSON body
/// verbatim as a GraphQL value. A missing token means the gateway's own `authenticate` middleware
/// (`metap`'s `graphql-gateway/src/server.rs`) somehow let an unauthenticated request through —
/// not expected to happen (every `/graphql` request is already decoded against this gateway's
/// keypair before a resolver ever runs), but a clear error beats a confusing downstream 401.
async fn forward_post(
    ctx: &ResolverContext<'_>,
    url: String,
    body: Value,
) -> Result<GqlValue, GqlError> {
    let token = ctx
        .data::<RequestContext>()?
        .forwarded_bearer_token
        .clone()
        .ok_or_else(|| GqlError::new("no caller token to forward — this should be unreachable"))?;
    let http = metap_runtime::http_client::default_client();
    let response = http
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| GqlError::new(format!("calling {url}: {e}")))?;
    let status = response.status();
    let json: Value = response
        .json()
        .await
        .map_err(|e| GqlError::new(format!("parsing response from {url}: {e}")))?;
    if !status.is_success() {
        let message = json
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("request failed")
            .to_string();
        return Err(GqlError::new(message));
    }
    GqlValue::from_json(json).map_err(|e| GqlError::new(e.to_string()))
}

fn id_arg(ctx: &ResolverContext<'_>, name: &str) -> Result<String, GqlError> {
    Ok(ctx.args.try_get(name)?.string()?.to_string())
}

/// Adds the 7 custom action mutations + the `aggregate` query — see the module doc comment for
/// why these live here rather than in `metap-graphql`/`graphql-gateway`. Takes only the 3 derived
/// REST base URLs (owned `String`s, cheap to clone per closure), not the upstream config list
/// itself — `UpstreamConfig` carries login credentials and deliberately isn't `Clone`.
fn add_custom_fields(
    mut query: Object,
    mut mutation: Object,
    zones: String,
    scanning: String,
    alerting: String,
) -> (Object, Object) {
    {
        let zones = zones.clone();
        mutation = mutation.field(
            Field::new("verifyZoneDns", TypeRef::named(JSON_SCALAR), move |ctx| {
                let zones = zones.clone();
                FieldFuture::new(async move {
                    let zone_id = id_arg(&ctx, "zoneId")?;
                    let url = format!("{zones}/api/waf.zones/{zone_id}/verify-dns");
                    Ok(Some(FieldValue::value(
                        forward_post(&ctx, url, serde_json::json!({})).await?,
                    )))
                })
            })
            .argument(InputValue::new("zoneId", TypeRef::named_nn(TypeRef::ID))),
        );
    }

    {
        let zones = zones.clone();
        mutation = mutation.field(
            Field::new("testZoneOrigin", TypeRef::named(JSON_SCALAR), move |ctx| {
                let zones = zones.clone();
                FieldFuture::new(async move {
                    let zone_id = id_arg(&ctx, "zoneId")?;
                    let url = format!("{zones}/api/waf.zones/{zone_id}/test-origin");
                    Ok(Some(FieldValue::value(
                        forward_post(&ctx, url, serde_json::json!({})).await?,
                    )))
                })
            })
            .argument(InputValue::new("zoneId", TypeRef::named_nn(TypeRef::ID))),
        );
    }

    {
        let zones = zones.clone();
        mutation = mutation.field(
            Field::new(
                "syncZoneConfigState",
                TypeRef::named(JSON_SCALAR),
                move |ctx| {
                    let zones = zones.clone();
                    FieldFuture::new(async move {
                        let zone_id = id_arg(&ctx, "zoneId")?;
                        let url = format!("{zones}/api/waf.zones/{zone_id}/sync-config-state");
                        Ok(Some(FieldValue::value(
                            forward_post(&ctx, url, serde_json::json!({})).await?,
                        )))
                    })
                },
            )
            .argument(InputValue::new("zoneId", TypeRef::named_nn(TypeRef::ID))),
        );
    }

    {
        let scanning = scanning.clone();
        mutation = mutation.field(
            Field::new("runScanJob", TypeRef::named(JSON_SCALAR), move |ctx| {
                let scanning = scanning.clone();
                FieldFuture::new(async move {
                    let job_id = id_arg(&ctx, "jobId")?;
                    let url = format!("{scanning}/api/waf.scan_jobs/{job_id}/run");
                    Ok(Some(FieldValue::value(
                        forward_post(&ctx, url, serde_json::json!({})).await?,
                    )))
                })
            })
            .argument(InputValue::new("jobId", TypeRef::named_nn(TypeRef::ID))),
        );
    }

    {
        let alerting = alerting.clone();
        mutation = mutation.field(
            Field::new("testAlertPolicy", TypeRef::named(JSON_SCALAR), move |ctx| {
                let alerting = alerting.clone();
                FieldFuture::new(async move {
                    let policy_id = id_arg(&ctx, "policyId")?;
                    let url = format!("{alerting}/api/waf.alert_policies/{policy_id}/test");
                    Ok(Some(FieldValue::value(
                        forward_post(&ctx, url, serde_json::json!({})).await?,
                    )))
                })
            })
            .argument(InputValue::new("policyId", TypeRef::named_nn(TypeRef::ID))),
        );
    }

    {
        let alerting = alerting.clone();
        mutation = mutation.field(
            Field::new(
                "correlateIncidents",
                TypeRef::named(JSON_SCALAR),
                move |ctx| {
                    let alerting = alerting.clone();
                    FieldFuture::new(async move {
                        let zone_id = match ctx.args.get("zoneId") {
                            Some(v) if !v.is_null() => Some(v.string()?.to_string()),
                            _ => None,
                        };
                        let url = format!("{alerting}/internal/incidents/correlate");
                        let body = match zone_id {
                            Some(zone_id) => serde_json::json!({ "zoneId": zone_id }),
                            None => serde_json::json!({}),
                        };
                        Ok(Some(FieldValue::value(
                            forward_post(&ctx, url, body).await?,
                        )))
                    })
                },
            )
            .argument(InputValue::new("zoneId", TypeRef::named(TypeRef::STRING))),
        );
    }

    {
        let alerting = alerting.clone();
        mutation = mutation.field(Field::new(
            "evaluateAlerts",
            TypeRef::named(JSON_SCALAR),
            move |ctx| {
                let alerting = alerting.clone();
                FieldFuture::new(async move {
                    let url = format!("{alerting}/internal/alerts/evaluate");
                    Ok(Some(FieldValue::value(
                        forward_post(&ctx, url, serde_json::json!({})).await?,
                    )))
                })
            },
        ));
    }

    // `aggregate` is semantically a read (same reasoning `metap-http` mounts `POST
    // /api/{entity}/aggregate` under `AuthContext`, the same read gate as `list` — see
    // `docs/roadmap/70-aggregate-api.md`) despite being a `POST` at the REST layer, so it's a
    // Query field here, not a Mutation.
    query = query.field(
        Field::new("aggregate", TypeRef::named(JSON_SCALAR), move |ctx| {
            let zones = zones.clone();
            let scanning = scanning.clone();
            let alerting = alerting.clone();
            FieldFuture::new(async move {
                let entity = id_arg(&ctx, "entity")?;
                let base = rest_base_for_entity(&entity, &zones, &scanning, &alerting)?.to_string();
                let spec = ctx
                    .args
                    .try_get("spec")?
                    .as_value()
                    .clone()
                    .into_json()
                    .map_err(|e| GqlError::new(e.to_string()))?;
                let url = format!("{base}/api/{entity}/aggregate");
                Ok(Some(FieldValue::value(
                    forward_post(&ctx, url, spec).await?,
                )))
            })
        })
        .argument(InputValue::new(
            "entity",
            TypeRef::named_nn(TypeRef::STRING),
        ))
        .argument(InputValue::new("spec", TypeRef::named_nn(JSON_SCALAR))),
    );

    (query, mutation)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    metap_infra::init_tracing();
    let config = GatewayConfig::from_env()?;

    tracing::info!(
        upstreams = config.upstreams.len(),
        "discovering upstream schemas..."
    );
    let zones = rest_base_url(&config.upstreams, "zones").to_string();
    let scanning = rest_base_url(&config.upstreams, "scanning").to_string();
    let alerting = rest_base_url(&config.upstreams, "alerting").to_string();
    let limits = metap_graphql::SchemaLimits {
        depth: config.graphql_max_depth,
        complexity: config.graphql_max_complexity,
    };
    let built =
        schema_builder::build_with_extensions(&config.upstreams, limits, move |query, mutation| {
            add_custom_fields(query, mutation, zones, scanning, alerting)
        })
        .await?;
    tracing::info!(
        entities = built.entity_count,
        "schema built, starting server"
    );

    server::serve(config, built).await
}
