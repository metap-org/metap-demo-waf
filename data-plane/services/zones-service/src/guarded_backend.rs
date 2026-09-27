//! `GuardedZonesBackend` — wraps this service's own `CrudService` (`state.crud`) so the
//! create/update-time guards that used to be axum middleware (`routes::zone_domain_guard`/
//! `firewall_rule_match_condition_guard`/`ip_access_list_value_guard`) still run for the *only*
//! mutation path that reaches this service today: gRPC, called by `waf-graphql-gateway`'s
//! `CompositeBackend` on behalf of the real Customer Portal (and by anything else that talks
//! GraphQL through that gateway).
//!
//! **Found live 2026-09-27**
//! (`../../../metap-docs/docs/roadmap/99-zones-service-guard-reachability-fix.md`): every one of
//! those axum middlewares matched only a literal REST path (`/api/waf.zones`, `POST
//! /api/waf.firewall_rules`, ...) that has not existed in this service's router at all since
//! `metap` core removed generic REST entity CRUD (2026-09-21) — this service never mounted
//! `metap-graphql-http::router()` either, so gRPC has been the *only* way to create/update/delete
//! `waf.zones`/`waf.ddos_policies`/`waf.firewall_rules`/`waf.ip_access_lists` for a while, and none
//! of it ever reached these middlewares. The clearest live consequence: the real Onboarding page's
//! "Create Zone" button has been hard-failing with `validation_failed: domainId required` this
//! whole time, since nothing else ever injects `domainId` — `web/src/pages/OnboardingPage.tsx` has
//! relied on `zone_domain_guard` doing that automatically since Increment 3, and still does.
//!
//! This wraps `Arc<dyn RecordBackend>` (the same seam `metap-graphql`'s resolvers already use,
//! and — since 2026-09-27 — the same one `metap-grpc::GrpcRecordService` now accepts instead of a
//! hardcoded `Arc<CrudService>`) rather than reaching back into axum, so the fix lives at the one
//! layer every real mutation path actually goes through.
//!
//! **Known gap, deliberately not closed by this file**: `routes::zone_delete_guard`'s
//! cross-service reference check (does `scanning-service`/`alerting-service` still hold a record
//! pointing at this zone?) is *not* ported here. Its old REST implementation called
//! `GET {scanning,alerting}-service/api/{entity}` — also long gone — and porting it properly means
//! this service dialing its 2 siblings' own gRPC ports with a service-account login, which is a
//! real new boot-time dependency between 3 services this app's own docs describe as independently
//! deployable. That's a deployment-shape decision, not a one-line fix — flagged here rather than
//! guessed at. Until it's decided, deleting a `Zone` that still has a live `ScanJob`/`Incident`/
//! `SecurityEvent` pointing at it silently orphans those rows (no error, no block) — a real
//! regression from what the old REST-era middleware did, worth knowing about, not hidden.

use std::collections::HashMap;
use std::sync::Arc;

use metap::crud::{JsonObject, RecordBackend, RecordCapabilities, RecordDto, ServiceResult};
use metap::permission::RequestContext;
use metap::query::{AggregateSpec, ListInput};
use serde_json::json;
use uuid::Uuid;

pub struct GuardedZonesBackend {
    inner: Arc<dyn RecordBackend>,
}

impl GuardedZonesBackend {
    pub fn new(inner: Arc<dyn RecordBackend>) -> Self {
        Self { inner }
    }

    /// Ports `routes::zone_domain_guard`'s logic verbatim, just reading/writing a `JsonObject`
    /// directly instead of an axum request body — looks up (or lazily creates) the `Domain` row
    /// for `hostname`'s apex and returns its id to inject into the create payload.
    async fn resolve_domain_id(
        &self,
        hostname: &str,
        ctx: &RequestContext,
    ) -> anyhow::Result<Result<Uuid, ServiceResult<RecordDto>>> {
        let apex = crate::apex_domain::apex_domain(hostname);
        let existing = self
            .inner
            .list(
                "waf.domains",
                &ListInput {
                    limit: 1,
                    filters: vec![("apexDomain".to_string(), apex.clone())],
                    ..Default::default()
                },
                ctx,
            )
            .await?;
        match existing {
            ServiceResult::Ok { data, .. } if !data.is_empty() => Ok(Ok(data[0].id)),
            ServiceResult::Ok { .. } => {
                let mut new_domain = JsonObject::new();
                new_domain.insert("apexDomain".to_string(), json!(apex));
                new_domain.insert(
                    "verificationToken".to_string(),
                    json!(format!("waf-verify-{}", Uuid::new_v4())),
                );
                new_domain.insert("verificationMethod".to_string(), json!("dnsTxt"));
                new_domain.insert("verificationStatus".to_string(), json!("unverified"));
                match self
                    .inner
                    .create("waf.domains", &new_domain, ctx, None)
                    .await?
                {
                    ServiceResult::Ok { data, .. } => Ok(Ok(data.id)),
                    err @ ServiceResult::Err { .. } => Ok(Err(err)),
                }
            }
            ServiceResult::Err {
                status,
                error,
                message,
                field_errors,
            } => Ok(Err(ServiceResult::Err {
                status,
                error,
                message,
                field_errors,
            })),
        }
    }

    /// Ports `routes::firewall_rule_match_condition_guard`'s validation verbatim.
    fn validate_firewall_rule(data: &JsonObject) -> Option<ServiceResult<RecordDto>> {
        let condition = data.get("matchCondition");
        if condition.is_some() && !crate::match_condition::is_valid_match_condition(condition) {
            return Some(ServiceResult::err_with_field_errors(
                422,
                "validation_failed",
                HashMap::from([(
                    "matchCondition".to_string(),
                    vec!["not a recognized rule expression".to_string()],
                )]),
            ));
        }
        None
    }

    /// Ports `routes::ip_access_list_value_guard`'s validation verbatim.
    fn validate_ip_access_list(data: &JsonObject) -> Option<ServiceResult<RecordDto>> {
        let value = data.get("value").and_then(serde_json::Value::as_str);
        if let Some(value) = value {
            if !crate::ip_format::is_valid_ip_or_cidr(value) {
                return Some(ServiceResult::err_with_field_errors(
                    422,
                    "validation_failed",
                    HashMap::from([(
                        "value".to_string(),
                        vec!["not a valid IP address or CIDR".to_string()],
                    )]),
                ));
            }
        }
        None
    }
}

#[async_trait::async_trait]
impl RecordBackend for GuardedZonesBackend {
    async fn list(
        &self,
        entity: &str,
        input: &ListInput,
        ctx: &RequestContext,
    ) -> anyhow::Result<ServiceResult<Vec<RecordDto>>> {
        self.inner.list(entity, input, ctx).await
    }

    async fn get(
        &self,
        entity: &str,
        id: Uuid,
        ctx: &RequestContext,
    ) -> anyhow::Result<ServiceResult<(RecordDto, RecordCapabilities)>> {
        self.inner.get(entity, id, ctx).await
    }

    async fn get_many(
        &self,
        entity: &str,
        ids: &[Uuid],
        ctx: &RequestContext,
    ) -> anyhow::Result<ServiceResult<Vec<(Uuid, RecordDto, RecordCapabilities)>>> {
        self.inner.get_many(entity, ids, ctx).await
    }

    async fn create(
        &self,
        entity: &str,
        data: &JsonObject,
        ctx: &RequestContext,
        reason: Option<&str>,
    ) -> anyhow::Result<ServiceResult<RecordDto>> {
        match entity {
            "waf.zones" => {
                let Some(hostname) = data.get("hostname").and_then(serde_json::Value::as_str)
                else {
                    // No hostname at all — let the real create reject it with its own "required
                    // field" validation error rather than this wrapper inventing a different one.
                    return self.inner.create(entity, data, ctx, reason).await;
                };
                let domain_id = match self.resolve_domain_id(hostname, ctx).await? {
                    Ok(id) => id,
                    Err(err) => return Ok(err),
                };
                let mut data = data.clone();
                data.insert("domainId".to_string(), json!(domain_id));
                self.inner.create(entity, &data, ctx, reason).await
            }
            "waf.firewall_rules" => {
                if let Some(err) = Self::validate_firewall_rule(data) {
                    return Ok(err);
                }
                self.inner.create(entity, data, ctx, reason).await
            }
            "waf.ip_access_lists" => {
                if let Some(err) = Self::validate_ip_access_list(data) {
                    return Ok(err);
                }
                self.inner.create(entity, data, ctx, reason).await
            }
            _ => self.inner.create(entity, data, ctx, reason).await,
        }
    }

    async fn update(
        &self,
        entity: &str,
        id: Uuid,
        expected_version: i32,
        data: &JsonObject,
        ctx: &RequestContext,
        reason: Option<&str>,
    ) -> anyhow::Result<ServiceResult<RecordDto>> {
        // `domainId` is create-only (matching the old `zone_domain_guard`, which only ever
        // intercepted `POST`) — a `Zone`'s domain never changes once it's assigned.
        match entity {
            "waf.firewall_rules" => {
                if let Some(err) = Self::validate_firewall_rule(data) {
                    return Ok(err);
                }
            }
            "waf.ip_access_lists" => {
                if let Some(err) = Self::validate_ip_access_list(data) {
                    return Ok(err);
                }
            }
            _ => {}
        }
        self.inner
            .update(entity, id, expected_version, data, ctx, reason)
            .await
    }

    async fn transition(
        &self,
        entity: &str,
        id: Uuid,
        action: &str,
        expected_version: i32,
        data: Option<&JsonObject>,
        ctx: &RequestContext,
        reason: Option<&str>,
    ) -> anyhow::Result<ServiceResult<RecordDto>> {
        self.inner
            .transition(entity, id, action, expected_version, data, ctx, reason)
            .await
    }

    async fn delete(
        &self,
        entity: &str,
        id: Uuid,
        expected_version: i32,
        ctx: &RequestContext,
        reason: Option<&str>,
    ) -> anyhow::Result<ServiceResult<RecordDto>> {
        // See this module's own doc comment — the cross-service reference check that used to run
        // here (`routes::zone_delete_guard`) is a known, deliberately unclosed gap.
        self.inner
            .delete(entity, id, expected_version, ctx, reason)
            .await
    }

    async fn aggregate(
        &self,
        entity: &str,
        spec: &AggregateSpec,
        ctx: &RequestContext,
    ) -> anyhow::Result<ServiceResult<Vec<serde_json::Value>>> {
        self.inner.aggregate(entity, spec, ctx).await
    }
}
