//! Real Postgres, no HTTP server — proves `GuardedZonesBackend::create` auto-attaches a new
//! `Zone` to the right `Domain` (creating one on first apex, reusing it for a second subdomain
//! under the same apex). `#[ignore]`d: needs `DATABASE_URL`/a running Postgres with `waf.waf_zones`/
//! `waf.waf_domains` already reconciled (this test only inserts/deletes rows, it doesn't create
//! tables — `cargo run -p zones-service` once against the same database first).
//!
//! Rewritten 2026-09-27 (`../../../../metap-docs/docs/roadmap/99-zones-service-guard-reachability-fix.md`)
//! off the old REST-server-plus-axum-middleware shape: `zone_domain_guard` moved into
//! `GuardedZonesBackend`, a `RecordBackend` decorator called by gRPC now, not an axum middleware —
//! so this test calls it the same way, in-process, with a hand-built `RequestContext` (`admin` role
//! bypasses policy checks via `RequestContext::is_admin()`, no JWT/`user_roles` row needed at all).

use std::sync::Arc;

use arc_swap::ArcSwap;
use metap::crud::{RecordBackend, ServiceResult};
use metap::prelude::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;
use zones_service::entities::domain_entity::domain_entity;
use zones_service::entities::zone_entity::zone_entity;
use zones_service::guarded_backend::GuardedZonesBackend;

async fn connect() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap()
}

fn admin_context(tenant_id: Uuid, user_id: Uuid) -> metap::permission::RequestContext {
    metap::permission::RequestContext {
        tenant_id: tenant_id.to_string(),
        user_id: Some(user_id.to_string()),
        roles: Some(vec!["admin".to_string()]),
        function_id: None,
        context_attributes: None,
        forwarded_bearer_token: None,
    }
}

#[tokio::test]
#[ignore = "e2e: requires DATABASE_URL / a running Postgres"]
async fn guarded_backend_create_auto_attaches_zone_and_dedupes_domain_by_apex() {
    let pool = connect().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let ctx = admin_context(tenant_id, user_id);

    let mut registry = MetadataRegistry::new();
    registry.register(zone_entity()).unwrap();
    registry.register(domain_entity()).unwrap();
    let registry = Arc::new(registry);
    let tenant_registry = Arc::new(metap::control::PostgresTenantRegistry::new(pool.clone()));
    let router = metap::control::Router::new(
        pool.clone(),
        metap::control::RegistryCache::new(tenant_registry),
        Arc::new(metap::control::EnvStore),
    );
    let permissions = Arc::new(PermissionService::new(Box::new(PostgresPolicyStore::new(
        router.clone(),
    ))));
    let crud: Arc<dyn metap::crud::RecordBackend> = Arc::new(CrudService::new(
        router,
        Arc::new(ArcSwap::new(registry)),
        permissions,
    ));
    let backend = GuardedZonesBackend::new(crud);

    // 2 labels only, so `apex_domain()` returns this exact string unchanged and never collides
    // with a real apex like "example.com" — a 3-label value here (e.g. ending "example.com")
    // would silently collide with real data from an earlier real-DB migration.
    let apex = format!("guard-test-{}.com", Uuid::new_v4().simple());

    // 1. Creating the first zone under a brand-new apex creates a Domain for it.
    let mut first_data = metap::crud::JsonObject::new();
    first_data.insert(
        "hostname".to_string(),
        serde_json::json!(format!("shop.{apex}")),
    );
    first_data.insert("originAddress".to_string(), serde_json::json!("10.0.0.1"));
    first_data.insert("protectionMode".to_string(), serde_json::json!("monitor"));
    let first = backend
        .create("waf.zones", &first_data, &ctx, None)
        .await
        .unwrap();
    let ServiceResult::Ok {
        data: first_dto, ..
    } = first
    else {
        panic!("expected Ok, got {first:?}");
    };
    let first_domain_id = first_dto
        .data
        .get("domainId")
        .and_then(|v| v.as_str())
        .expect("domainId must be set automatically")
        .to_string();

    // 2. A second zone under the same apex reuses the same Domain — no duplicate created.
    let mut second_data = metap::crud::JsonObject::new();
    second_data.insert(
        "hostname".to_string(),
        serde_json::json!(format!("api.{apex}")),
    );
    second_data.insert("originAddress".to_string(), serde_json::json!("10.0.0.2"));
    second_data.insert("protectionMode".to_string(), serde_json::json!("monitor"));
    let second = backend
        .create("waf.zones", &second_data, &ctx, None)
        .await
        .unwrap();
    let ServiceResult::Ok {
        data: second_dto, ..
    } = second
    else {
        panic!("expected Ok, got {second:?}");
    };
    let second_domain_id = second_dto
        .data
        .get("domainId")
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(
        second_domain_id, first_domain_id,
        "a second subdomain under the same apex must reuse the same Domain"
    );

    let domain_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM waf.waf_domains WHERE deleted = false AND data->>'apexDomain' = $1",
    )
    .bind(&apex)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        domain_count.0, 1,
        "exactly one Domain must exist for this apex"
    );

    sqlx::query("DELETE FROM waf.waf_zones WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM waf.waf_domains WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .ok();
}

#[tokio::test]
#[ignore = "e2e: requires DATABASE_URL / a running Postgres"]
async fn guarded_backend_create_without_hostname_falls_through_to_the_real_validation_error() {
    let pool = connect().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let ctx = admin_context(tenant_id, user_id);

    let mut registry = MetadataRegistry::new();
    registry.register(zone_entity()).unwrap();
    registry.register(domain_entity()).unwrap();
    let registry = Arc::new(registry);
    let tenant_registry = Arc::new(metap::control::PostgresTenantRegistry::new(pool.clone()));
    let router = metap::control::Router::new(
        pool.clone(),
        metap::control::RegistryCache::new(tenant_registry),
        Arc::new(metap::control::EnvStore),
    );
    let permissions = Arc::new(PermissionService::new(Box::new(PostgresPolicyStore::new(
        router.clone(),
    ))));
    let crud: Arc<dyn metap::crud::RecordBackend> = Arc::new(CrudService::new(
        router,
        Arc::new(ArcSwap::new(registry)),
        permissions,
    ));
    let backend = GuardedZonesBackend::new(crud);

    // No `hostname` at all — the wrapper must not itself invent an error, it defers to the real
    // `create()`'s own required-field validation.
    let mut data = metap::crud::JsonObject::new();
    data.insert("originAddress".to_string(), serde_json::json!("10.0.0.1"));
    let result = backend
        .create("waf.zones", &data, &ctx, None)
        .await
        .unwrap();
    let ServiceResult::Err { status, error, .. } = result else {
        panic!("expected Err, got {result:?}");
    };
    assert_eq!(status, 400);
    assert_eq!(error, "validation_failed");
}
