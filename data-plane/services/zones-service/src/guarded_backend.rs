//! `GuardedZonesBackend` — wraps this service's own `CrudService` (`state.crud`) so the
//! create/update/delete-time guards that used to be axum middleware (`routes::zone_domain_guard`/
//! `firewall_rule_match_condition_guard`/`ip_access_list_value_guard`/`zone_delete_guard`) still
//! run for the *only* mutation path that reaches this service today: gRPC, called by
//! `waf-graphql-gateway`'s `CompositeBackend` on behalf of the real Customer Portal (and by
//! anything else that talks GraphQL through that gateway).
//!
//! **Found live 2026-09-27**
//! (`../../../metap-docs/docs/roadmap/99-zones-service-guard-reachability-fix.md`): every one of
//! those axum middlewares matched only a literal REST path (`/api/waf.zones`, `POST
//! /api/waf.firewall_rules`, ...) that has not existed in this service's router at all since
//! `metap` core removed generic REST entity CRUD (2026-09-21) — this service never mounted
//! `metap-graphql-http::router()` either, so gRPC has been the *only* way to create/update/delete
//! `waf.zones`/`waf.ddos_policies`/`waf.firewall_rules`/`waf.ip_access_lists` for a while, and none
//! of it ever reached these middlewares. The clearest live consequence: the real Onboarding page's
//! "Create Zone" button had been hard-failing with `validation_failed: domainId required` this
//! whole time, since nothing else ever injects `domainId` — `web/src/pages/OnboardingPage.tsx` has
//! relied on `zone_domain_guard` doing that automatically since Increment 3, and still does.
//!
//! This wraps `Arc<dyn RecordBackend>` (the same seam `metap-graphql`'s resolvers already use,
//! and — since 2026-09-27 — the same one `metap-grpc::GrpcRecordService` now accepts instead of a
//! hardcoded `Arc<CrudService>`) rather than reaching back into axum, so the fix lives at the one
//! layer every real mutation path actually goes through.
//!
//! ## `zone_delete_guard`'s cross-service reference check (2026-09-27, ported same day)
//!
//! The old REST implementation called `GET {scanning,alerting}-service/api/{entity}` — also long
//! gone. The new one dials each sibling's own gRPC port directly (`SCANNING_GRPC_ADDR`/
//! `ALERTING_GRPC_ADDR`, defaulting to their standard `3011`/`3021`) via `metap_grpc::GrpcBackend`,
//! **with no service-account credential of its own** — instead of logging in as a fixed user (which
//! would only ever see one tenant's rows in this `Schema`-strategy shared table, wrong for every
//! other tenant), it mints a fresh, short-lived token for the *same* `tenant_id`/`user_id` already
//! on the incoming `RequestContext`, via the same `token_signer`/`jwt_encoding_key_pem` dispatch
//! `AppState::mint_token` uses internally (JWKS trust root when configured, the static keypair
//! otherwise, so this can't drift from whichever one this deployment actually verifies with) —
//! this struct takes just those 2 fields rather than the whole `AppState`, since minting is all it
//! needs. That token rides as `RequestContext::forwarded_bearer_token`, which `GrpcBackend`
//! already prefers over any of its own stored credentials — functionally identical to
//! `waf-graphql-gateway` forwarding a caller's real bearer token onward, just minted fresh instead
//! of relayed verbatim (this process never sees the caller's original raw JWT string). The
//! sibling's own `GrpcRecordService` then resolves that token's real roles independently, so the
//! check runs with the real caller's own permissions, not a blanket service identity.
//!
//! **Lazy, cached, self-healing connection** (`CrossServiceLink`) — the first `waf.zones` delete
//! after boot triggers a connection attempt to each sibling; a successful one is cached for reuse,
//! a failed one is *not* cached, so the very next delete retries rather than requiring a restart
//! once the sibling comes back up. This is why boot itself never depends on `scanning-service`/
//! `alerting-service` being up already — the 3 services stay independently deployable exactly as
//! this app's docs describe, at the cost of every zone delete failing closed (`503
//! reference_check_unavailable`) while a sibling is down, matching the old REST guard's own
//! "unreachable = block, never silently let an orphan through" philosophy exactly.

use std::collections::HashMap;
use std::sync::Arc;

use metap::crud::{JsonObject, RecordBackend, RecordCapabilities, RecordDto, ServiceResult};
use metap::grpc::{GrpcBackend, ServiceTokenSource};
use metap::jwks::TokenSigner;
use metap::permission::RequestContext;
use metap::query::{AggregateSpec, ListInput};
use serde_json::json;
use tokio::sync::RwLock;
use uuid::Uuid;

/// One sibling service's gRPC connection, established lazily and cached only on success — see
/// this module's own doc comment for why.
struct CrossServiceLink {
    addr: String,
    cached: RwLock<Option<Arc<dyn RecordBackend>>>,
}

impl CrossServiceLink {
    fn new(addr: String) -> Self {
        Self {
            addr,
            cached: RwLock::new(None),
        }
    }

    async fn get(&self) -> Option<Arc<dyn RecordBackend>> {
        if let Some(backend) = self.cached.read().await.as_ref() {
            return Some(backend.clone());
        }
        // The token this placeholder carries is never actually sent — every real call below
        // always sets `RequestContext::forwarded_bearer_token`, which `GrpcBackend::signed_request`
        // prefers unconditionally over this source. It exists only because `GrpcBackend::connect`
        // requires a `ServiceTokenSource` to construct one at all.
        let placeholder = ServiceTokenSource::from_static("unused-forwarded-token-always-wins");
        match GrpcBackend::connect(self.addr.clone(), placeholder).await {
            Ok(backend) => {
                let backend: Arc<dyn RecordBackend> = Arc::new(backend);
                *self.cached.write().await = Some(backend.clone());
                Some(backend)
            }
            Err(err) => {
                tracing::warn!(
                    addr = self.addr,
                    error = %err,
                    "zone-delete cross-service reference check: could not connect, will retry next delete"
                );
                None
            }
        }
    }
}

pub struct GuardedZonesBackend {
    inner: Arc<dyn RecordBackend>,
    /// Same 2 fields `AppState::mint_token` dispatches on — see this module's doc comment for
    /// why this struct takes just these instead of the whole `AppState`.
    token_signer: Option<Arc<TokenSigner>>,
    jwt_encoding_key_pem: Arc<str>,
    scanning: CrossServiceLink,
    alerting: CrossServiceLink,
}

impl GuardedZonesBackend {
    pub fn new(
        inner: Arc<dyn RecordBackend>,
        token_signer: Option<Arc<TokenSigner>>,
        jwt_encoding_key_pem: Arc<str>,
        scanning_grpc_addr: String,
        alerting_grpc_addr: String,
    ) -> Self {
        Self {
            inner,
            token_signer,
            jwt_encoding_key_pem,
            scanning: CrossServiceLink::new(scanning_grpc_addr),
            alerting: CrossServiceLink::new(alerting_grpc_addr),
        }
    }

    /// Same dispatch as `AppState::mint_token` (`crates/metap-http/src/state.rs`) — kept in sync
    /// by hand since this struct deliberately doesn't hold an `AppState` to call it on.
    fn mint_token(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        function_id: Option<String>,
        ttl_seconds: u64,
    ) -> anyhow::Result<String> {
        match &self.token_signer {
            Some(signer) => {
                metap::jwks::mint_with_signer(signer, tenant_id, user_id, function_id, ttl_seconds)
            }
            None => metap::peripherals::mint_jwt(
                &self.jwt_encoding_key_pem,
                tenant_id,
                user_id,
                ttl_seconds,
            ),
        }
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

    /// Ports `routes::zone_delete_guard`'s cross-service check — see this module's own doc
    /// comment for the identity-forwarding mechanism. `Ok(Some(entity))` names the blocking
    /// entity, `Ok(None)` means clear to delete, `Err(_)` is a `ServiceResult::Err` the caller
    /// should return as-is (either `record_referenced` or `reference_check_unavailable`).
    async fn zone_referenced_by(
        &self,
        zone_id: Uuid,
        ctx: &RequestContext,
    ) -> anyhow::Result<Result<Option<&'static str>, ServiceResult<RecordDto>>> {
        let tenant_id = Uuid::parse_str(&ctx.tenant_id)
            .map_err(|e| anyhow::anyhow!("RequestContext.tenant_id is not a UUID: {e}"))?;
        let user_id = match ctx.user_id.as_deref() {
            Some(raw) => Uuid::parse_str(raw)
                .map_err(|e| anyhow::anyhow!("RequestContext.user_id is not a UUID: {e}"))?,
            None => Uuid::nil(),
        };
        // Short-lived on purpose — this token exists only to make the next 1-3 gRPC calls below,
        // never stored, never returned to any caller.
        let forwarded_token = self.mint_token(tenant_id, user_id, ctx.function_id.clone(), 30)?;
        let mut forwarded_ctx = ctx.clone();
        forwarded_ctx.forwarded_bearer_token = Some(forwarded_token);

        let checks: [(&CrossServiceLink, &'static str); 3] = [
            (&self.scanning, "waf.scan_jobs"),
            (&self.alerting, "waf.incidents"),
            (&self.alerting, "waf.security_events"),
        ];
        for (link, entity) in checks {
            let Some(backend) = link.get().await else {
                return Ok(Err(ServiceResult::err_with_message(
                    503,
                    "reference_check_unavailable",
                    format!(
                        "Could not verify cross-service references ({entity} unreachable); refusing to delete."
                    ),
                )));
            };
            let result = backend
                .list(
                    entity,
                    &ListInput {
                        limit: 1,
                        filters: vec![("zoneId".to_string(), zone_id.to_string())],
                        ..Default::default()
                    },
                    &forwarded_ctx,
                )
                .await?;
            match result {
                ServiceResult::Ok { data, .. } if !data.is_empty() => {
                    tracing::warn!(
                        zone_id = %zone_id,
                        referencing_entity = entity,
                        "zone delete rejected: still referenced by another service"
                    );
                    return Ok(Ok(Some(entity)));
                }
                ServiceResult::Ok { .. } => {}
                ServiceResult::Err {
                    status,
                    error,
                    message,
                    field_errors,
                } => {
                    return Ok(Err(ServiceResult::Err {
                        status,
                        error,
                        message,
                        field_errors,
                    }))
                }
            }
        }
        Ok(Ok(None))
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
        if entity == "waf.zones" {
            match self.zone_referenced_by(id, ctx).await? {
                Ok(Some(blocking_entity)) => {
                    return Ok(ServiceResult::err_with_message(
                        409,
                        "record_referenced",
                        format!(
                            "This zone is still referenced by \"{blocking_entity}\" and cannot be deleted."
                        ),
                    ));
                }
                Ok(None) => {}
                Err(err) => return Ok(err),
            }
        }
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
