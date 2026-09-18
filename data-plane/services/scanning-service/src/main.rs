//! Boot sequence for `scanning-service` — 1 of 3 WAF Customer Portal backend services
//! (`zones-service`/`scanning-service`/`alerting-service`), split from the single `data-plane`
//! binary this repo used to be (2026-09-01 — see `../../docs/05-metap-technical-mapping.md` for
//! the entity/workflow spec, and the plan doc referenced from the roadmap entry for why the
//! split happened and how the 3 services are bounded).
//!
//! Owns `waf.scan_jobs` + `waf.scan_findings`. `ScanJob.zoneId`/`SecurityEvent.zoneId`-shaped
//! fields that point at `zones-service`'s `waf.zones` are plain `String`, not `Reference` — see
//! `entities/scan_job_entity.rs`'s own doc comment for why (registering `zone_entity()` here
//! just to satisfy `validate_references()` would also expose CRUD for `waf.zones` on this
//! service's `/api/:entity*` route). All 3 services point at the SAME tenant database
//! (`Router::pool_for`) — split by compute/deploy/ownership, not by data layer, so `Reference`
//! fields within this service's own entities (`ScanFinding.scanJobId`) keep a real Postgres FK.
//!
//! Entity registration is auto-discovered via `submit_entity!`/`register_all_submitted()` — see
//! `src/entities/mod.rs`. Reads config from the environment (or a `.env` file in this
//! directory — see `.env.example`). Run from this directory so that resolves as expected.

mod entities;
mod routes;

use entities::{scan_finding_entity::scan_finding_entity, scan_job_entity::scan_job_entity};

use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use metap::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    metap::infra::init_tracing();
    let config = load_config()?;

    let PlatformParts {
        pool,
        router,
        permissions,
        decoding_key,
        private_key_pem,
    } = bootstrap_platform(&config).await?;

    let mut registry = MetadataRegistry::new();
    registry.register_all_submitted()?;
    registry.validate_references()?;
    let metadata_base = Arc::new(registry);

    // Order is load-bearing: `ScanFinding.scanJobId` builds a real FK straight into
    // `waf.scan_jobs`'s table at DDL time, so `waf.scan_jobs` must be reconciled first. See
    // `zones-service/src/main.rs`'s reconcile loop for the `PLATFORM_TENANT_ID` sentinel +
    // `BackfillScope::AllTenants` reasoning — this boot step's DDL is tenant-agnostic, but a
    // backfill it triggers is not: `waf.scan_jobs`/`waf.scan_findings` hold every real tenant's
    // rows in one physical table.
    for entity in [scan_job_entity(), scan_finding_entity()] {
        let outcome = metap_reconciler::reconcile_with_scope(
            &pool,
            metap::control::PLATFORM_TENANT_ID,
            &entity,
            &[],
            metap_reconciler::BackfillScope::AllTenants,
        )
        .await?;
        tracing::info!(
            entity = entity.name,
            table = outcome.table,
            ops_applied = outcome.ops_applied,
            "reconciled dedicated table"
        );
    }

    let entities = metadata_base.list_entities();
    check_metadata_drift(&pool, &entities).await;
    reconcile_indexes(&pool, &entities).await;

    let metadata = Arc::new(ArcSwap::new(metadata_base.clone()));

    let mut state = AppState::new(
        pool,
        metadata_base,
        metadata,
        permissions,
        decoding_key,
        private_key_pem,
        router,
    );
    // General-purpose audit trail (`metap-audit`, audit finding 05) — every write on both
    // entities this service owns now lands a row in `metadata.audit_trail_entries`. Same
    // `Schema`-strategy shared `pool` reasoning as `zones-service`'s own wiring.
    state.crud = Arc::new(CrudService::with_audit(
        state.router.clone(),
        state.metadata.clone(),
        state.permissions.clone(),
        Arc::new(PostgresAuditTrailStore::new(state.pool.clone())),
    ));
    // Dev binary serves plain `http://localhost:3010` — a `Secure` session cookie (the
    // `AppState::new` default) is silently dropped by the browser over non-HTTPS. See
    // `docs/roadmap/64-cookie-session-persistence.md` in `../../metap-docs`.
    state.cookie_secure = false;

    // JWKS trust root (opt-in via `JWKS_PRIVATE_KEY_PATH`/`JWKS_KID_PATH`) — see
    // `../zones-service/src/main.rs`'s fuller comment on this block; that service is the one
    // that PUBLISHES the key (`/.well-known/jwks.json`), this one only verifies against it
    // (`JWKS_URL`, defaulting there) — this process still holds the same private key locally
    // too (mints via `state.token_signer`), matching today's shared-RSA-keypair topology, not a
    // single-issuer one.
    if let (Ok(private_key_path), Ok(kid_path)) = (
        std::env::var("JWKS_PRIVATE_KEY_PATH"),
        std::env::var("JWKS_KID_PATH"),
    ) {
        let kid = std::fs::read_to_string(&kid_path)?.trim().to_string();
        let private_pkcs8 = std::fs::read(&private_key_path)?;
        let signing_key = metap::jwks::JwksKeyPair::from_pkcs8(kid, private_pkcs8)?;
        state.token_signer = Some(Arc::new(metap::jwks::TokenSigner::Jwks {
            key: Arc::new(signing_key),
        }));
        let jwks_url = metap::runtime::env::env_or(
            "JWKS_URL",
            "http://localhost:3000/.well-known/jwks.json".to_string(),
        );
        state.token_verifier = Some(Arc::new(metap::jwks::TokenVerifier::Jwks {
            client: Arc::new(metap::jwks::JwksClient::new(
                jwks_url,
                Duration::from_secs(300),
            )),
            leeway: 20,
        }));
    }

    // gRPC opt-in (`GRPC_ENABLED`/`GRPC_PORT`) — lets a `graphql-gateway` instance aggregate
    // this service alongside `zones-service`/`alerting-service` for the WAF Customer Portal's
    // cross-service, read-only views (e.g. a Zone overview page). Read `state` before it's moved
    // into `build_router` below. `token_verifier_override` (audit 04 finding A#10, fixed
    // 2026-09-13 in `../../../../metap` — this call site used to hand-reimplement all of
    // `optional_serve`'s body just to pass this through) makes gRPC verify against the same JWKS
    // trust root as REST above when configured, falling back to `optional_serve`'s own
    // `TokenVerifier::Static` default otherwise.
    let grpc_handle = metap::grpc::optional_serve(
        &config.host,
        3011,
        metap::grpc::OptionalServeConfig {
            crud: state.crud.clone(),
            router: state.router.clone(),
            jwt_decoding_key: state.jwt_decoding_key.clone(),
            auth_context_entity: state.auth_context_entity.as_deref().map(str::to_string),
            metadata: state.metadata.clone(),
            context_attributes_cache: state.context_attributes_cache.clone(),
            token_verifier_override: state.token_verifier.clone(),
        },
    )
    .await?;

    // `attachments`/`dashboards` trimmed (2026-09-15, see `zones-service/src/main.rs`'s own
    // comment for the usage survey this is based on) — same 2 groups, same reasoning, applies
    // uniformly across all 3 WAF services.
    let router = build_router_with_groups(
        state,
        &config.cors_origins,
        routes::router(),
        RouteGroups {
            attachments: false,
            dashboards: false,
            ..RouteGroups::all()
        },
    );

    let addr = format!("{}:{}", config.host, config.port);

    metap::runtime::serve::run(
        &addr,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    drop(grpc_handle);

    Ok(())
}

#[cfg(test)]
mod tests {
    use metap::prelude::MetadataRegistry;

    #[test]
    fn owns_exactly_its_own_two_entities() {
        let mut registry = MetadataRegistry::new();
        registry.register_all_submitted().unwrap();
        registry.validate_references().unwrap();

        let names: Vec<String> = registry
            .list_entities()
            .into_iter()
            .map(|e| e.name)
            .collect();
        for expected in ["waf.scan_jobs", "waf.scan_findings"] {
            assert!(
                names.contains(&expected.to_string()),
                "missing entity: {expected}"
            );
        }
        assert_eq!(names.len(), 2, "unexpected entity count: {names:?}");
    }
}
