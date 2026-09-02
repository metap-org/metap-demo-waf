//! Boot sequence for `alerting-service` — 1 of 3 WAF Customer Portal backend services
//! (`zones-service`/`scanning-service`/`alerting-service`), split from the single `data-plane`
//! binary this repo used to be (2026-09-01 — see `../../docs/05-metap-technical-mapping.md` for
//! the entity/workflow spec, and the plan doc referenced from the roadmap entry for why the
//! split happened and how the 3 services are bounded).
//!
//! Owns `waf.security_events` + `waf.incidents` + `waf.alert_policies` +
//! `waf.alert_notifications` — kept together because `Incident` correlation logic
//! (`OnRecordEvent` trigger on `waf.security_events.created`) calls straight into `CrudService`
//! in-process; splitting `SecurityEvent`/`Incident` across services would turn that into a
//! cross-service call for no real benefit. `SecurityEvent.zoneId`/`Incident.zoneId` are plain
//! `String`, not `Reference` — see `entities/security_event_entity.rs`'s own doc comment for
//! why (registering `zone_entity()` here just to satisfy `validate_references()` would also
//! expose CRUD for `waf.zones` on this service's `/api/:entity*` route). All 3 services point
//! at the SAME tenant database (`Router::pool_for`) — split by compute/deploy/ownership, not by
//! data layer, so `Reference` fields within this service's own entities
//! (`AlertNotification.alertPolicyId`) keep a real Postgres FK.
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

    let state = AppState::new(
        pool,
        metadata_base,
        metadata,
        permissions,
        decoding_key,
        private_key_pem,
        router,
    );

    // gRPC opt-in (`GRPC_ENABLED`/`GRPC_PORT`) — lets a `graphql-gateway` instance aggregate
    // this service alongside `zones-service`/`scanning-service` for the WAF Customer Portal's
    // cross-service, read-only views (e.g. a Zone overview page). Read `state` before it's
    // moved into `build_router` below.
    let grpc_handle = metap::grpc::optional_serve(
        &config.host,
        3021,
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
    fn owns_exactly_its_own_four_entities() {
        let mut registry = MetadataRegistry::new();
        registry.register_all_submitted().unwrap();
        registry.validate_references().unwrap();

        let names: Vec<String> = registry
            .list_entities()
            .into_iter()
            .map(|e| e.name)
            .collect();
        for expected in [
            "waf.security_events",
            "waf.incidents",
            "waf.alert_policies",
            "waf.alert_notifications",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "missing entity: {expected}"
            );
        }
        assert_eq!(names.len(), 4, "unexpected entity count: {names:?}");
    }
}
