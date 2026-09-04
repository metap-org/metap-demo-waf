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
mod routes;

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

    // JWKS trust root (opt-in via `JWKS_PRIVATE_KEY_PATH`/`JWKS_KID_PATH`) — replaces the static
    // RSA keypair above for both mint (`state.token_signer`) and verify (`state.token_verifier`)
    // once set, same "extra field, post-construction opt-in" shape `cookie_secure` above already
    // uses. All 3 WAF services share one Ed25519 key (`dev-tools gen-jwks-key`) — `zones-service`
    // is the one that PUBLISHES it below (`/.well-known/jwks.json`); every service (including
    // this one) VERIFIES via `JWKS_URL`, so rotation (`JwksKeyStore::add_key`/`promote`/
    // `remove_key`) only ever needs updating here, not resent piecemeal. See `../../README.md`'s
    // auth section for the rationale (no private key copied to a process that only ever
    // verifies — not true yet for the 2 sibling services below, since all 3 still hold the same
    // signing key today; see that section for the topology this stops short of).
    let jwks_key_store = match (std::env::var("JWKS_PRIVATE_KEY_PATH"), std::env::var("JWKS_KID_PATH")) {
        (Ok(private_key_path), Ok(kid_path)) => {
            let kid = std::fs::read_to_string(&kid_path)?.trim().to_string();
            let private_pkcs8 = std::fs::read(&private_key_path)?;
            let signing_key = metap::jwks::JwksKeyPair::from_pkcs8(kid.clone(), private_pkcs8.clone())?;
            state.token_signer = Some(Arc::new(metap::jwks::TokenSigner::Jwks {
                key: Arc::new(signing_key),
            }));
            let jwks_url = metap::runtime::env::env_or(
                "JWKS_URL",
                "http://localhost:3000/.well-known/jwks.json".to_string(),
            );
            state.token_verifier = Some(Arc::new(metap::jwks::TokenVerifier::Jwks {
                client: Arc::new(metap::jwks::JwksClient::new(jwks_url, Duration::from_secs(300))),
                leeway: 20,
            }));
            // A second, independently-owned key (same bytes) for the `JwksKeyStore` this
            // process publishes — `JwksKeyStore::new` takes ownership, and `token_signer` above
            // already claimed the first one.
            let published_key = metap::jwks::JwksKeyPair::from_pkcs8(kid, private_pkcs8)?;
            Some(Arc::new(tokio::sync::RwLock::new(metap::jwks::JwksKeyStore::new(
                published_key,
            ))))
        }
        _ => None,
    };

    // gRPC opt-in (`GRPC_ENABLED`/`GRPC_PORT`) — lets a `graphql-gateway` instance aggregate
    // this service alongside `scanning-service`/`alerting-service` for the WAF Customer Portal's
    // cross-service, read-only views (e.g. a Zone overview page). Read `state` before it's
    // moved into `build_router` below. Bypasses `metap::grpc::optional_serve` (which only ever
    // builds `TokenVerifier::Static`, see that fn's doc comment) so gRPC verifies against the
    // same JWKS trust root as REST above when configured, falling back to the static keypair
    // identically to `optional_serve`'s own behavior otherwise.
    let grpc_verifier = state.token_verifier.clone().unwrap_or_else(|| {
        Arc::new(metap::jwks::TokenVerifier::Static {
            decoding_key: (*state.jwt_decoding_key).clone(),
            leeway: 20,
        })
    });
    let grpc_handle = if metap::runtime::env::flag_enabled("GRPC_ENABLED") {
        let grpc_port: u16 = metap::runtime::env::env_or("GRPC_PORT", 3001);
        let grpc_addr: std::net::SocketAddr = format!("{}:{grpc_port}", config.host).parse()?;
        let auth = metap::grpc::AuthConfig {
            verifier: (*grpc_verifier).clone(),
            router: state.router.clone(),
            auth_context_entity: state.auth_context_entity.as_deref().map(str::to_string),
            context_attributes_cache: state.context_attributes_cache.clone(),
        };
        let service = metap::grpc::GrpcRecordService::new(state.crud.clone(), auth);
        tracing::info!(%grpc_addr, "gRPC listening");
        Some(tokio::spawn(async move {
            if let Err(err) = metap::grpc::serve(grpc_addr, service, None).await {
                tracing::error!(error = %err, "gRPC server exited with error");
            }
        }))
    } else {
        None
    };

    // `routes::router()` goes through `extra_routes` so the custom onboarding/ops endpoints get
    // the same CORS/rate-limit/tracing/security-header layers as every generic route.
    // `zone_delete_guard` is a middleware rather than a route override on purpose — see its own
    // doc comment for why overriding `DELETE /api/waf.zones/{id}` would break `GET`/`PATCH` on
    // the same path.
    let guard_state = state.clone();
    let mut router = build_router(state, &config.cors_origins, routes::router());
    // `fallback_service`, not `route_service`/`nest_service("/", ...)` — axum 0.8 refuses both
    // for mounting a whole `Router`-typed service at/under this router's own root ("cannot be
    // used with Routers" / "Nesting at the root is no longer supported"; found live, 2026-09-04
    // — the crate's own module doc comment illustrates `route_service`, written against an axum
    // version that allowed it). Safe here specifically because `build_router`'s own output
    // never sets its own fallback (checked): this router tries its normal routes first, and only
    // an unmatched request reaches `metap_jwks_http::router`'s single registered path
    // (`/.well-known/jwks.json`) — anything else still 404s from that inner router's own default
    // fallback, identical to today's behavior. Also sidesteps the state-type mismatch (`AppState`
    // here vs that crate's own `SharedJwksKeyStore` state) `.merge()` can't cross. Only mounted
    // when this process is the JWKS publisher (above).
    if let Some(jwks_key_store) = jwks_key_store {
        router = router.fallback_service(metap::jwks_http::router(jwks_key_store));
    }
    let router = router.layer(axum::middleware::from_fn_with_state(guard_state, routes::zone_delete_guard));

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
