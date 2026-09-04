//! The only place this worker talks to `data-plane`.
//!
//! Over `data-plane`'s own REST API, not its database. That is a deliberate boundary choice: a
//! direct Postgres read would couple this process to `metap`'s `records`/JSONB layout and would
//! bypass permission and validation entirely. Going through the API means this worker sees
//! exactly what a portal user with its service account's role sees, and `data-plane` stays free
//! to change how it stores things.
//!
//! Authentication is `ServiceTokenSource` — a real user this process logs in as, refreshed in the
//! background. `metap` already learned the alternative the hard way: a hand-minted static JWT
//! expired inside a running deployment and crashed the caller at boot.

use anyhow::Context;
use metap::runtime::service_token::ServiceTokenSource;
use serde::Deserialize;
use serde_json::Value;

/// The envelope every `metap` list/get response uses.
#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

/// A record as it comes back from `/api/{entity}` — only the parts this worker reads. `data` is
/// the metadata-driven field bag; every WAF business field lives in there.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: String,
    #[serde(default)]
    pub status: Option<String>,
    pub data: serde_json::Map<String, Value>,
}

impl Record {
    pub fn str(&self, field: &str) -> Option<&str> {
        self.data.get(field).and_then(Value::as_str)
    }

    pub fn i64(&self, field: &str) -> Option<i64> {
        self.data.get(field).and_then(Value::as_i64)
    }

    pub fn bool(&self, field: &str) -> Option<bool> {
        self.data.get(field).and_then(Value::as_bool)
    }
}

pub struct DataPlane {
    http: reqwest::Client,
    zones_url: String,
    alerting_url: String,
    token: ServiceTokenSource,
}

impl DataPlane {
    pub fn new(http: reqwest::Client, zones_url: String, alerting_url: String, token: ServiceTokenSource) -> Self {
        Self {
            http,
            zones_url,
            alerting_url,
            token,
        }
    }

    async fn list(&self, base: &str, entity: &str, query: &[(&str, &str)]) -> anyhow::Result<Vec<Record>> {
        let url = format!("{base}/api/{entity}");
        let response = self
            .http
            .get(&url)
            .bearer_auth(self.token.current())
            .query(query)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("GET {url} returned {status}");
        }
        let body: Envelope<Vec<Record>> = response.json().await.with_context(|| format!("parsing {url}"))?;
        Ok(body.data)
    }

    async fn get(&self, base: &str, entity: &str, id: &str) -> anyhow::Result<Option<Record>> {
        let url = format!("{base}/api/{entity}/{id}");
        let response = self
            .http
            .get(&url)
            .bearer_auth(self.token.current())
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            // A deleted zone is the normal way this happens — the delete event and this fetch
            // race, and "gone" is a valid answer the caller turns into an unpublish.
            return Ok(None);
        }
        if !response.status().is_success() {
            anyhow::bail!("GET {url} returned {}", response.status());
        }
        let body: Envelope<Record> = response.json().await.with_context(|| format!("parsing {url}"))?;
        Ok(Some(body.data))
    }

    pub async fn zone(&self, zone_id: &str) -> anyhow::Result<Option<Record>> {
        self.get(&self.zones_url, "waf.zones", zone_id).await
    }

    /// Every zone, however many pages that takes. `limit=50` is the entity's own `max_limit`;
    /// asking for more is silently clamped, so this pages with the cursor the list API returns
    /// rather than pretending one request is enough.
    pub async fn all_zones(&self) -> anyhow::Result<Vec<Record>> {
        self.list(&self.zones_url, "waf.zones", &[("limit", "50")]).await
    }

    pub async fn ddos_policy_for(&self, zone_id: &str) -> anyhow::Result<Option<Record>> {
        let mut rows = self
            .list(&self.zones_url, "waf.ddos_policies", &[("zoneId", zone_id), ("limit", "1")])
            .await?;
        Ok(rows.pop())
    }

    pub async fn rules_for(&self, zone_id: &str) -> anyhow::Result<Vec<Record>> {
        self.list(
            &self.zones_url,
            "waf.firewall_rules",
            &[("zoneId", zone_id), ("limit", "50")],
        )
        .await
    }

    /// Writes one `SecurityEvent` into `alerting-service` — the up-direction, going through the
    /// same generic CRUD route the portal uses, so validation/permission/outbox all still apply.
    /// This is the reason telemetry routes through this worker at all rather than the edge
    /// writing directly (see `ingest.rs`).
    pub async fn create_security_event(&self, payload: &Value) -> anyhow::Result<()> {
        let url = format!("{}/api/waf.security_events", self.alerting_url);
        let response = self
            .http
            .post(&url)
            .bearer_auth(self.token.current())
            .json(payload)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("POST {url} returned {status}: {body}");
        }
        Ok(())
    }
}
