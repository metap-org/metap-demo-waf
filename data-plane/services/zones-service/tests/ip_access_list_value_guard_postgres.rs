//! Real Postgres, no HTTP server — proves `GuardedZonesBackend::create`/`update` actually blocks
//! a malformed `value` on `waf.ip_access_lists` and lets a valid IP or CIDR through.
//!
//! Rewritten 2026-09-27 (`../../../../metap-docs/docs/roadmap/99-zones-service-guard-reachability-fix.md`)
//! off the old REST-server-plus-axum-middleware shape — same pattern as
//! `firewall_rule_match_condition_guard_postgres.rs`, see that file's own doc comment for why.
//! `#[ignore]`d: needs `DATABASE_URL` pointed at a running Postgres.

use std::sync::Arc;

use arc_swap::ArcSwap;
use metap::crud::{RecordBackend, ServiceResult};
use metap::prelude::*;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;
use zones_service::guarded_backend::GuardedZonesBackend;

fn field(name: &str, kind: FieldKind, required: bool) -> EntityField {
    EntityField {
        name: name.to_string(),
        label: name.to_string(),
        kind,
        required: Some(required),
        indexed: None,
        unique: None,
        enum_values: None,
        ref_entity: None,
        ref_display_field: None,
        searchable: None,
        search_mode: None,
        sortable: None,
        storage: None,
        min: None,
        max: None,
        min_length: None,
        max_length: None,
        computed: None,
    }
}

const TEST_TABLE: &str = "entities.test_waf_ip_access_lists_guarded_backend";

fn ip_access_list_test_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.ip_access_lists".to_string(),
        label: "IP Access List".to_string(),
        table_name: TEST_TABLE.to_string(),
        fields: vec![
            field("type", FieldKind::String, true),
            field("value", FieldKind::String, true),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec!["type".to_string(), "value".to_string()],
            filters: vec![],
            required_fields: vec![],
            default_sort: None,
            max_limit: 50,
        }],
        workflow: None,
        unique_constraints: vec![],
        audit: None,
    }
}

async fn connect() -> PgPool {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL required for this e2e test");
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
async fn guarded_backend_blocks_invalid_and_allows_valid_ip_values() {
    let pool = connect().await;
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {TEST_TABLE} (
            id uuid PRIMARY KEY DEFAULT gen_random_uuid() NOT NULL,
            tenant_id uuid NOT NULL,
            code varchar(120),
            status varchar(80),
            data jsonb DEFAULT '{{}}'::jsonb NOT NULL,
            version integer DEFAULT 1 NOT NULL,
            deleted boolean DEFAULT false NOT NULL,
            created_at timestamp with time zone DEFAULT now() NOT NULL,
            updated_at timestamp with time zone DEFAULT now() NOT NULL,
            created_by uuid,
            updated_by uuid
        )"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let ctx = admin_context(tenant_id, user_id);

    let mut registry = MetadataRegistry::new();
    registry.register(ip_access_list_test_entity()).unwrap();
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

    // 1. Garbage value is rejected before it ever reaches CrudService.
    let mut bad = metap::crud::JsonObject::new();
    bad.insert("type".to_string(), serde_json::json!("whitelist"));
    bad.insert("value".to_string(), serde_json::json!("not-an-ip"));
    let rejected = backend
        .create("waf.ip_access_lists", &bad, &ctx, None)
        .await
        .unwrap();
    let ServiceResult::Err {
        status,
        error,
        field_errors,
        ..
    } = rejected
    else {
        panic!("expected Err, got {rejected:?}");
    };
    assert_eq!(status, 422);
    assert_eq!(error, "validation_failed");
    assert!(field_errors.unwrap().contains_key("value"));

    // 2. A bare IP is accepted.
    let mut bare_ip = metap::crud::JsonObject::new();
    bare_ip.insert("type".to_string(), serde_json::json!("whitelist"));
    bare_ip.insert("value".to_string(), serde_json::json!("10.0.0.5"));
    let created = backend
        .create("waf.ip_access_lists", &bare_ip, &ctx, None)
        .await
        .unwrap();
    let ServiceResult::Ok {
        data: created_dto, ..
    } = created
    else {
        panic!("expected Ok, got {created:?}");
    };

    // 3. A real CIDR is also accepted.
    let mut cidr = metap::crud::JsonObject::new();
    cidr.insert("type".to_string(), serde_json::json!("blacklist"));
    cidr.insert("value".to_string(), serde_json::json!("10.0.0.0/24"));
    let created_cidr = backend
        .create("waf.ip_access_lists", &cidr, &ctx, None)
        .await
        .unwrap();
    assert!(matches!(created_cidr, ServiceResult::Ok { .. }));

    // 4. A field-only update (no `value` at all) is untouched by the guard.
    let mut retype = metap::crud::JsonObject::new();
    retype.insert("type".to_string(), serde_json::json!("blacklist"));
    let updated = backend
        .update(
            "waf.ip_access_lists",
            created_dto.id,
            created_dto.version,
            &retype,
            &ctx,
            None,
        )
        .await
        .unwrap();
    let ServiceResult::Ok {
        data: updated_dto, ..
    } = updated
    else {
        panic!("expected Ok, got {updated:?}");
    };

    // 5. Updating to a malformed value is rejected the same way create is.
    let mut bad_update = metap::crud::JsonObject::new();
    bad_update.insert("value".to_string(), serde_json::json!("10.0.0.0/999"));
    let rejected_update = backend
        .update(
            "waf.ip_access_lists",
            updated_dto.id,
            updated_dto.version,
            &bad_update,
            &ctx,
            None,
        )
        .await
        .unwrap();
    assert!(matches!(
        rejected_update,
        ServiceResult::Err { status: 422, .. }
    ));

    sqlx::query(&format!("DELETE FROM {TEST_TABLE} WHERE tenant_id = $1"))
        .bind(tenant_id)
        .execute(&pool)
        .await
        .ok();
}
