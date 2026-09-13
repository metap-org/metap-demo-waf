//! Real server, real Postgres, real HTTP — proves `routes::ip_access_list_value_guard` actually
//! blocks a malformed `value` at `POST`/`PATCH /api/waf.ip_access_lists` and lets a valid IP or
//! CIDR through.
//!
//! Same pattern as `firewall_rule_match_condition_guard_postgres.rs` — a minimal local entity, not
//! the real `entities::ip_access_list_entity()`, registered on the generic `records` table so this
//! test needs no `metap_reconciler::reconcile()` call. It only needs to be named exactly
//! `"waf.ip_access_lists"`, since the guard matches on the literal request path, not on entity
//! shape. `#[ignore]`d: needs `DATABASE_URL` pointed at a running Postgres.

use std::process::Command;
use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::Router;
use jsonwebtoken::{encode, EncodingKey, Header};
use metap::prelude::*;
use serde::Serialize;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

fn openssl_genrsa(dir: &std::path::Path) -> (String, String) {
    let private_path = dir.join("private.pem");
    let public_path = dir.join("public.pem");
    let status = Command::new("openssl")
        .args(["genrsa", "-out"])
        .arg(&private_path)
        .arg("2048")
        .status()
        .expect("openssl genrsa must run for this e2e test");
    assert!(status.success());
    let status = Command::new("openssl")
        .args(["rsa", "-in"])
        .arg(&private_path)
        .args(["-pubout", "-out"])
        .arg(&public_path)
        .status()
        .expect("openssl rsa -pubout must run for this e2e test");
    assert!(status.success());
    (
        std::fs::read_to_string(private_path).unwrap(),
        std::fs::read_to_string(public_path).unwrap(),
    )
}

#[derive(Serialize)]
struct Claims {
    sub: String,
    #[serde(rename = "tenantId")]
    tenant_id: String,
    exp: usize,
}

fn mint_token(private_pem: &str, tenant_id: Uuid, user_id: Uuid) -> String {
    let claims = Claims {
        sub: user_id.to_string(),
        tenant_id: tenant_id.to_string(),
        exp: (chrono::Utc::now().timestamp() + 3600) as usize,
    };
    let key = EncodingKey::from_rsa_pem(private_pem.as_bytes()).unwrap();
    encode(&Header::new(jsonwebtoken::Algorithm::RS256), &claims, &key).unwrap()
}

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

/// Minimal stand-in for `entities::ip_access_list_entity()` — only what this test needs
/// (`type`/`value`), registered at the exact entity name the guard's path check hardcodes.
fn ip_access_list_test_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.ip_access_lists".to_string(),
        label: "IP Access List".to_string(),
        table_name: "records".to_string(),
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

#[tokio::test]
#[ignore = "e2e: requires DATABASE_URL / a running Postgres"]
async fn ip_access_list_value_guard_blocks_invalid_and_allows_valid_values() {
    let pool = connect().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let keydir = std::env::temp_dir().join(format!("waf-ip-guard-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&keydir).unwrap();
    let (private_pem, public_pem) = openssl_genrsa(&keydir);
    let token = mint_token(&private_pem, tenant_id, user_id);
    sqlx::query("INSERT INTO user_roles (tenant_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(tenant_id)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut registry = MetadataRegistry::new();
    registry.register(ip_access_list_test_entity()).unwrap();
    let registry = Arc::new(registry);
    let tenant_registry = Arc::new(metap::control::PostgresTenantRegistry::new(pool.clone()));
    let test_router = metap::control::Router::new(
        pool.clone(),
        metap::control::RegistryCache::new(tenant_registry),
        Arc::new(metap::control::EnvStore),
    );
    let permissions =
        PermissionService::new(Box::new(PostgresPolicyStore::new(test_router.clone())));
    let decoding_key = jsonwebtoken::DecodingKey::from_rsa_pem(public_pem.as_bytes()).unwrap();
    let state = AppState::new(
        pool.clone(),
        registry.clone(),
        Arc::new(ArcSwap::new(registry)),
        Arc::new(permissions),
        decoding_key,
        private_pem.clone(),
        test_router,
    );
    let router = build_router(state, &[], Router::new()).layer(axum::middleware::from_fn(
        zones_service::routes::ip_access_list_value_guard,
    ));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    // 1. Garbage value is rejected before it ever reaches CrudService.
    let rejected = client
        .post(format!("{base}/api/waf.ip_access_lists"))
        .bearer_auth(&token)
        .json(&json!({ "data": { "type": "whitelist", "value": "not-an-ip" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 422);
    let body: serde_json::Value = rejected.json().await.unwrap();
    assert_eq!(body["error"]["code"], "validation_failed");
    assert!(body["error"]["fieldErrors"]["value"].is_array());

    // 2. A bare IP is accepted.
    let created = client
        .post(format!("{base}/api/waf.ip_access_lists"))
        .bearer_auth(&token)
        .json(&json!({ "data": { "type": "whitelist", "value": "10.0.0.5" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let created_body: serde_json::Value = created.json().await.unwrap();
    let id = created_body["data"]["id"].as_str().unwrap().to_string();
    let version = created_body["data"]["version"].as_i64().unwrap();

    // 3. A real CIDR is also accepted.
    let created_cidr = client
        .post(format!("{base}/api/waf.ip_access_lists"))
        .bearer_auth(&token)
        .json(&json!({ "data": { "type": "blacklist", "value": "10.0.0.0/24" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(created_cidr.status(), 201);

    // 4. A field-only patch (no `value` at all) is untouched by the guard.
    let patched_unrelated = client
        .patch(format!("{base}/api/waf.ip_access_lists/{id}"))
        .bearer_auth(&token)
        .json(&json!({ "version": version, "data": { "type": "blacklist" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(patched_unrelated.status(), 200);

    // 5. Updating to a malformed value is rejected the same way create is.
    let patched_body: serde_json::Value = patched_unrelated.json().await.unwrap();
    let version_after = patched_body["data"]["version"].as_i64().unwrap();
    let rejected_update = client
        .patch(format!("{base}/api/waf.ip_access_lists/{id}"))
        .bearer_auth(&token)
        .json(&json!({ "version": version_after, "data": { "value": "10.0.0.0/999" } }))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected_update.status(), 422);

    sqlx::query("DELETE FROM records WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM user_roles WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .ok();
    std::fs::remove_dir_all(&keydir).ok();
}
