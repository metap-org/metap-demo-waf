//! Boot sequence for `zones-service` — 1 of 3 WAF Customer Portal backend services
//! (`zones-service`/`scanning-service`/`alerting-service`), split from the single `data-plane`
//! binary this repo used to be (2026-09-01 — see `../../docs/05-metap-technical-mapping.md` for
//! the entity/workflow spec, and the plan doc referenced from the roadmap entry for why the
//! split happened and how the 3 services are bounded).
//!
//! Owns `waf.zones` + `waf.ddos_policies` + `waf.firewall_rules` — kept together because
//! `Zone`'s workflow guard `activate` depends on the technical field `hasConfig`, updated by
//! app logic whenever a `DdosPolicy`/`FirewallRule` is created/deleted for that zone; keeping
//! that hook in-process (not a cross-service call) matters most at exactly this sensitive a
//! spot (a workflow guard). All 3 services point at the SAME tenant database
//! (`Router::pool_for`) — split by compute/deploy/ownership, not by data layer, so `Reference`
//! fields within this service's own entities (`DdosPolicy.zoneId`/`FirewallRule.zoneId`) keep a
//! real Postgres FK. This service deliberately does **not** register any other service's
//! entities (`waf.scan_jobs`, `waf.security_events`, ...) — doing so would expose CRUD for them
//! on this service's own `/api/:entity*` route too, defeating the point of the split.
//!
//! Entity registration is auto-discovered via `submit_entity!`/`register_all_submitted()` — see
//! `src/entities/mod.rs`. Reads config from the environment (or a `.env` file in this
//! directory — see `.env.example`). Run from this directory so that resolves as expected.

mod entities;

use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::Router;
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
    // Dev binary serves plain `http://localhost:3000` — a `Secure` session cookie (the
    // `AppState::new` default) is silently dropped by the browser over non-HTTPS, which looks
    // exactly like "login succeeds but nothing stays logged in" (`GET /auth/me` never sees the
    // cookie). See `docs/roadmap/64-cookie-session-persistence.md` in `../../metap-docs`.
    state.cookie_secure = false;

    // gRPC opt-in (`GRPC_ENABLED`/`GRPC_PORT`) — lets a `graphql-gateway` instance aggregate
    // this service alongside `scanning-service`/`alerting-service` for the WAF Customer Portal's
    // cross-service, read-only views (e.g. a Zone overview page). Read `state` before it's
    // moved into `build_router` below.
    let grpc_handle = metap::grpc::optional_serve(
        &config.host,
        3001,
        metap::grpc::OptionalServeConfig {
            crud: state.crud.clone(),
            router: state.router.clone(),
            jwt_decoding_key: state.jwt_decoding_key.clone(),
            auth_context_entity: state.auth_context_entity.as_deref().map(str::to_string),
            context_attributes_cache: state.context_attributes_cache.clone(),
        },
    )
    .await?;

    let router = build_router(state, &config.cors_origins, Router::new());

    let addr = format!("{}:{}", config.host, config.port);

    // `build_router`'s rate-limit layer keys on peer IP via `ConnectInfo<SocketAddr>` — plain
    // `into_make_service()` wouldn't populate that extension and every request would fail
    // rate-limit key extraction.
    metap::runtime::serve::run(
        &addr,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    // `grpc_handle` deliberately isn't joined — no in-flight-publish state to drain, and
    // `metap_grpc::serve` has no shutdown-signal parameter to wire one in with (see that fn's
    // doc comment). Dropping a still-running task here is fine: the process is exiting anyway.
    drop(grpc_handle);

    Ok(())
}

#[cfg(test)]
mod tests {
    use metap::prelude::MetadataRegistry;

    // Regression guard for `register_all_submitted()` — catches a `src/entities/*.rs` module
    // that forgot its `submit_entity!` call, a name collision, or (the failure mode this split
    // specifically introduced) another service's entity accidentally ending up registered here.
    #[test]
    fn owns_exactly_its_own_three_entities() {
        let mut registry = MetadataRegistry::new();
        registry.register_all_submitted().unwrap();
        registry.validate_references().unwrap();

        let names: Vec<String> = registry
            .list_entities()
            .into_iter()
            .map(|e| e.name)
            .collect();
        for expected in ["waf.zones", "waf.ddos_policies", "waf.firewall_rules"] {
            assert!(
                names.contains(&expected.to_string()),
                "missing entity: {expected}"
            );
        }
        assert_eq!(names.len(), 3, "unexpected entity count: {names:?}");
    }
}
