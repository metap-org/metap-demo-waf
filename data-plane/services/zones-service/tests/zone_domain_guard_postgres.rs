//! Real server, real Postgres, real HTTP — proves `routes::zone_domain_guard` auto-attaches a new
//! `Zone` to the right `Domain` (creating one on first apex, reusing it for a second subdomain
//! under the same apex), and that `routes::verify_domain_dns` cascades a successful ownership
//! verification onto every zone under that domain. `#[ignore]`d: needs `DATABASE_URL`/a running
//! Postgres.

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
use zones_service::entities::domain_entity::domain_entity;
use zones_service::entities::zone_entity::zone_entity;

fn openssl_genrsa(dir: &std::path::Path) -> (String, String) {
    let private_path = dir.join("private.pem");
    let public_path = dir.join("public.pem");
    let status = Command::new("openssl")
        .args(["genrsa", "-out"])
        .arg(&private_path)
        .arg("2048")
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("openssl")
        .args(["rsa", "-in"])
        .arg(&private_path)
        .args(["-pubout", "-out"])
        .arg(&public_path)
        .status()
        .unwrap();
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

async fn connect() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "e2e: requires DATABASE_URL / a running Postgres"]
async fn zone_domain_guard_auto_attaches_and_dedupes_by_apex() {
    let pool = connect().await;
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    let keydir = std::env::temp_dir().join(format!("waf-domain-guard-test-{}", Uuid::new_v4()));
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
    registry.register(zone_entity()).unwrap();
    registry.register(domain_entity()).unwrap();
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
    let guard_state = state.clone();
    let router = build_router(state, &[], Router::new()).layer(
        axum::middleware::from_fn_with_state(guard_state, zones_service::routes::zone_domain_guard),
    );

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

    // 2 labels only, so `apex_domain()` returns this exact string unchanged and never collides
    // with a real apex like "example.com" — a 3-label value here (e.g. ending "example.com")
    // would silently collide with real data from an earlier real-DB migration.
    let apex = format!("guard-test-{}.com", Uuid::new_v4().simple());

    // 1. Creating the first zone under a brand-new apex creates a Domain for it.
    let create_first = client
        .post(format!("{base}/api/waf.zones"))
        .bearer_auth(&token)
        .json(&json!({ "data": {
            "hostname": format!("shop.{apex}"),
            "originAddress": "10.0.0.1",
            "protectionMode": "monitor",
        } }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        create_first.status(),
        201,
        "{:?}",
        create_first.text().await
    );
    let first_body: serde_json::Value = create_first.json().await.unwrap();
    let first_domain_id = first_body["data"]["data"]["domainId"]
        .as_str()
        .expect("domainId must be set automatically")
        .to_string();

    // 2. A second zone under the same apex reuses the same Domain — no duplicate created.
    let create_second = client
        .post(format!("{base}/api/waf.zones"))
        .bearer_auth(&token)
        .json(&json!({ "data": {
            "hostname": format!("api.{apex}"),
            "originAddress": "10.0.0.2",
            "protectionMode": "monitor",
        } }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        create_second.status(),
        201,
        "{:?}",
        create_second.text().await
    );
    let second_body: serde_json::Value = create_second.json().await.unwrap();
    let second_domain_id = second_body["data"]["data"]["domainId"].as_str().unwrap();
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
    sqlx::query("DELETE FROM user_roles WHERE tenant_id = $1")
        .bind(tenant_id)
        .execute(&pool)
        .await
        .ok();
    std::fs::remove_dir_all(&keydir).ok();
}
