//! Real Postgres, no HTTP server — proves `GuardedZonesBackend::create`/`update` actually blocks
//! an unrepresentable `matchCondition` on `waf.firewall_rules` and lets a valid one (including the
//! `regex` operator) through.
//!
//! Rewritten 2026-09-27 (`../../../../metap-docs/docs/roadmap/99-zones-service-guard-reachability-fix.md`)
//! off the old REST-server-plus-axum-middleware shape: `firewall_rule_match_condition_guard` moved
//! into `GuardedZonesBackend`, a `RecordBackend` decorator called by gRPC now, not an axum
//! middleware — so this test calls it the same way, in-process, with a hand-built
//! `RequestContext` (`admin` role bypasses policy checks via `RequestContext::is_admin()`, no
//! JWT/`user_roles` row needed at all). Same minimal-local-entity-on-a-throwaway-table pattern the
//! old test used (this crate is a binary, not a library an integration test can import `src/`
//! entity definitions from beyond what `lib.rs` exposes) — only needs to be named exactly
//! `"waf.firewall_rules"`, since `GuardedZonesBackend` matches on entity name, not shape.
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

const TEST_TABLE: &str = "entities.test_waf_firewall_rules_guarded_backend";

fn firewall_rule_test_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.firewall_rules".to_string(),
        label: "Firewall Rule".to_string(),
        table_name: TEST_TABLE.to_string(),
        fields: vec![
            field("name", FieldKind::String, true),
            field("matchCondition", FieldKind::Json, false),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec!["name".to_string()],
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
async fn guarded_backend_blocks_invalid_and_allows_valid_match_conditions() {
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
    registry.register(firewall_rule_test_entity()).unwrap();
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

    // 1. An unknown field/op is rejected before it ever reaches CrudService.
    let mut bad = metap::crud::JsonObject::new();
    bad.insert("name".to_string(), serde_json::json!("bad rule"));
    bad.insert(
        "matchCondition".to_string(),
        serde_json::json!({ "field": "bogus", "op": "eq", "value": "x" }),
    );
    let rejected = backend
        .create("waf.firewall_rules", &bad, &ctx, None)
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
    assert!(field_errors.unwrap().contains_key("matchCondition"));

    // 2. An invalid regex pattern is rejected too (the operator this whole feature added).
    let mut bad_regex = metap::crud::JsonObject::new();
    bad_regex.insert("name".to_string(), serde_json::json!("bad regex rule"));
    bad_regex.insert(
        "matchCondition".to_string(),
        serde_json::json!({ "field": "uri.path", "op": "regex", "value": "[invalid(regex" }),
    );
    let rejected_regex = backend
        .create("waf.firewall_rules", &bad_regex, &ctx, None)
        .await
        .unwrap();
    assert!(matches!(
        rejected_regex,
        ServiceResult::Err { status: 422, .. }
    ));

    // 3. A valid condition (including a real, compiling regex) is created successfully.
    let mut good = metap::crud::JsonObject::new();
    good.insert("name".to_string(), serde_json::json!("good rule"));
    good.insert(
        "matchCondition".to_string(),
        serde_json::json!({ "field": "uri.path", "op": "regex", "value": r"^/admin/\d+$" }),
    );
    let created = backend
        .create("waf.firewall_rules", &good, &ctx, None)
        .await
        .unwrap();
    let ServiceResult::Ok {
        data: created_dto, ..
    } = created
    else {
        panic!("expected Ok, got {created:?}");
    };

    // 4. A field-only update (no matchCondition at all) is untouched by the guard.
    let mut rename = metap::crud::JsonObject::new();
    rename.insert("name".to_string(), serde_json::json!("renamed"));
    let renamed = backend
        .update(
            "waf.firewall_rules",
            created_dto.id,
            created_dto.version,
            &rename,
            &ctx,
            None,
        )
        .await
        .unwrap();
    let ServiceResult::Ok {
        data: renamed_dto, ..
    } = renamed
    else {
        panic!("expected Ok, got {renamed:?}");
    };

    // 5. Updating to an unrepresentable condition is rejected the same way create is.
    let mut bad_update = metap::crud::JsonObject::new();
    bad_update.insert(
        "matchCondition".to_string(),
        serde_json::json!({ "field": "header", "op": "eq", "value": "1" }),
    );
    let rejected_update = backend
        .update(
            "waf.firewall_rules",
            renamed_dto.id,
            renamed_dto.version,
            &bad_update,
            &ctx,
            None,
        )
        .await
        .unwrap();
    assert!(
        matches!(rejected_update, ServiceResult::Err { status: 422, .. }),
        "header field with no param name is unrepresentable"
    );

    sqlx::query(&format!("DELETE FROM {TEST_TABLE} WHERE tenant_id = $1"))
        .bind(tenant_id)
        .execute(&pool)
        .await
        .ok();
}
