//! The only place this worker talks to `data-plane`.
//!
//! Over `data-plane`'s own GraphQL API, not its database. That is a deliberate boundary choice: a
//! direct Postgres read would couple this process to `metap`'s dedicated-table/JSONB layout and
//! would bypass permission and validation entirely. Going through the API means this worker sees
//! exactly what a portal user with its service account's role sees, and `data-plane` stays free to
//! change how it stores things.
//!
//! **GraphQL, not REST — found live 2026-09-27.** This file used to call `GET/POST
//! /api/{entity}...`, which `metap` core removed entirely on 2026-09-21
//! (`../../metap-docs/docs/roadmap/90-remove-rest-entity-crud.md`) — meaning `control-plane` had
//! been calling a REST surface that no longer exists on any current `zones-service`/
//! `alerting-service` build since that date, silently broken end to end (never caught, since
//! nothing had exercised the live control-plane -> edge-plane pipeline since REST entity CRUD was
//! removed). Rewritten onto `POST /graphql`, the same migration `platform-ui`/`metap-demo-jira/web`
//! already went through (`97-platform-fields-typed-objects.md`'s sibling phases 93/94) — field
//! names/selection sets below follow `metap-graphql/src/naming.rs`'s camelCase convention
//! (`waf.zones` -> `wafZones`/`wafZonesList`), hand-mirrored the same way `platform-ui`'s
//! `graphqlNaming.ts` does, since there is no shared source of truth for this mapping across the
//! Rust/TS boundary (or, here, Rust/Rust across two separately-deployed binaries).
//!
//! Authentication is `ServiceTokenSource` — a real user this process logs in as, refreshed in the
//! background. `metap` already learned the alternative the hard way: a hand-minted static JWT
//! expired inside a running deployment and crashed the caller at boot.
//!
//! **`zones_url`/`alerting_url` must point at the GraphQL gateway (`../data-plane/graphql-gateway`,
//! port 4000 in dev), not at `zones-service`/`alerting-service`'s own ports** — found live wiring
//! the first real e2e proof of this pipeline. Neither service mounts `metap-graphql-http::router()`
//! itself (each only exposes gRPC, `GRPC_ENABLED`, for the gateway to aggregate — see each
//! service's own `main.rs`); `POST /graphql` against a bare service port 404s. `login_url` (auth
//! only) is unaffected — it still points at `zones-service`'s own `/auth/login`, since the gateway
//! has no login endpoint of its own and only decode-verifies the bearer token this process already
//! minted.

use anyhow::Context;
use metap::runtime::service_token::ServiceTokenSource;
use serde_json::{json, Value};

/// A record as reshaped from the GraphQL response — the fixed envelope's `id`/`status` plus
/// whichever data fields this file actually reads (`compile.rs`), same shape the old REST
/// `Envelope<Record>` used to hand back.
#[derive(Debug, Clone)]
pub struct Record {
    pub id: String,
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

/// Builds a `Record` from one GraphQL result object: `id`/`status` pulled off the envelope,
/// everything in `fields` copied into `data` under its own name (already flat — none of the
/// fields this file selects are `Reference`-typed, so no nested-object unwrapping is needed).
fn record_from_json(mut raw: Value, fields: &[&str]) -> Record {
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let status = raw
        .get("status")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut data = serde_json::Map::new();
    if let Some(object) = raw.as_object_mut() {
        for field in fields {
            if let Some(value) = object.remove(*field) {
                data.insert((*field).to_string(), value);
            }
        }
    }
    Record { id, status, data }
}

const ZONE_FIELDS: &[&str] = &[
    "hostname",
    "originAddress",
    "protectionMode",
    "configVersion",
];
const DDOS_FIELDS: &[&str] = &[
    "enabled",
    "sensitivity",
    "action",
    "requestRateThreshold",
    "burstWindow",
    "pathPrefix",
    "httpMethod",
    "priority",
];
const RULE_FIELDS: &[&str] = &[
    "enabled",
    "ruleType",
    "matchCondition",
    "rateLimitThreshold",
    "rateLimitWindow",
    "name",
    "action",
    "priority",
];
const IP_ACCESS_LIST_FIELDS: &[&str] = &["enabled", "type", "value"];

pub struct DataPlane {
    http: reqwest::Client,
    zones_url: String,
    alerting_url: String,
    token: ServiceTokenSource,
}

impl DataPlane {
    pub fn new(
        http: reqwest::Client,
        zones_url: String,
        alerting_url: String,
        token: ServiceTokenSource,
    ) -> Self {
        Self {
            http,
            zones_url,
            alerting_url,
            token,
        }
    }

    /// POSTs one GraphQL query/mutation, returns the parsed `data` object on success.
    /// `Ok(None)` means the request succeeded but every error present was a `404`-shaped
    /// not-found (the only error shape a caller here treats as a normal "gone" answer, matching
    /// REST's old `404 -> Ok(None)` behavior) — any other error bails.
    async fn graphql(
        &self,
        base: &str,
        query: &str,
        variables: Value,
    ) -> anyhow::Result<Option<Value>> {
        let url = format!("{base}/graphql");
        let response = self
            .http
            .post(&url)
            .bearer_auth(self.token.current())
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("POST {url} returned {status}");
        }
        let body: Value = response
            .json()
            .await
            .with_context(|| format!("parsing {url}"))?;

        if let Some(errors) = body.get("errors").and_then(Value::as_array) {
            if !errors.is_empty() {
                let all_not_found = errors
                    .iter()
                    .all(|e| e.pointer("/extensions/status").and_then(Value::as_i64) == Some(404));
                if all_not_found {
                    return Ok(None);
                }
                anyhow::bail!("POST {url} returned GraphQL errors: {errors:?}");
            }
        }
        Ok(body.get("data").cloned())
    }

    async fn list(
        &self,
        base: &str,
        list_field: &str,
        fields: &[&str],
        filter: Value,
        limit: i64,
    ) -> anyhow::Result<Vec<Record>> {
        let selection = fields.join(" ");
        let query = format!(
            "query($filter: Json, $limit: Int) {{ result: {list_field}(filter: $filter, limit: $limit) {{ records {{ id status {selection} }} }} }}"
        );
        let data = self
            .graphql(base, &query, json!({ "filter": filter, "limit": limit }))
            .await?
            .unwrap_or(Value::Null);
        let records = data
            .pointer("/result/records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(records
            .into_iter()
            .map(|r| record_from_json(r, fields))
            .collect())
    }

    async fn get(
        &self,
        base: &str,
        get_field: &str,
        fields: &[&str],
        id: &str,
    ) -> anyhow::Result<Option<Record>> {
        let selection = fields.join(" ");
        let query = format!(
            "query($id: ID!) {{ result: {get_field}(id: $id) {{ id status {selection} }} }}"
        );
        let Some(data) = self.graphql(base, &query, json!({ "id": id })).await? else {
            // A deleted zone is the normal way this happens — the delete event and this fetch
            // race, and "gone" is a valid answer the caller turns into an unpublish.
            return Ok(None);
        };
        match data.get("result") {
            None | Some(Value::Null) => Ok(None),
            Some(record) => Ok(Some(record_from_json(record.clone(), fields))),
        }
    }

    pub async fn zone(&self, zone_id: &str) -> anyhow::Result<Option<Record>> {
        self.get(&self.zones_url, "wafZones", ZONE_FIELDS, zone_id)
            .await
    }

    /// Every zone, however many pages that takes. `limit=50` is the entity's own `max_limit`;
    /// asking for more is silently clamped, so this pages with the cursor the list API returns
    /// rather than pretending one request is enough.
    ///
    /// **Known limitation carried over unchanged from REST**: only the first page is fetched
    /// (same as before this file's GraphQL rewrite) — a tenant with more than 50 zones would need
    /// this to actually follow `nextCursor`, which neither the old REST client nor this one does.
    pub async fn all_zones(&self) -> anyhow::Result<Vec<Record>> {
        self.list(&self.zones_url, "wafZonesList", ZONE_FIELDS, json!({}), 50)
            .await
    }

    /// Every DDoS policy on this zone — a zone can carry more than one, scoped by path/method
    /// (Increment 4), so this is a list rather than the single `Option<Record>` it used to be.
    pub async fn ddos_policies_for(&self, zone_id: &str) -> anyhow::Result<Vec<Record>> {
        self.list(
            &self.zones_url,
            "wafDdosPoliciesList",
            DDOS_FIELDS,
            json!({ "zoneId": zone_id }),
            50,
        )
        .await
    }

    pub async fn rules_for(&self, zone_id: &str) -> anyhow::Result<Vec<Record>> {
        self.list(
            &self.zones_url,
            "wafFirewallRulesList",
            RULE_FIELDS,
            json!({ "zoneId": zone_id }),
            50,
        )
        .await
    }

    /// Tenant-wide ("global") `FirewallRule`s — `zoneId` unset applies to every zone. `zoneId: ""`
    /// (empty value) is `metap-query`'s own "field is unset" convention
    /// (`crates/metap-query/src/query_planner.rs`: empty filter value compiles to `IS NULL`, not
    /// an equality match against the literal empty string), the same convention already used
    /// throughout this platform for an optional `Reference` field with no value assigned — and
    /// still honoured identically over GraphQL's `filter: Json` argument as it was over REST's
    /// `?zoneId=` query param, since both go through the same `QueryPlanner`.
    pub async fn tenant_wide_rules_for(&self) -> anyhow::Result<Vec<Record>> {
        self.list(
            &self.zones_url,
            "wafFirewallRulesList",
            RULE_FIELDS,
            json!({ "zoneId": "" }),
            50,
        )
        .await
    }

    pub async fn ip_access_lists_for(&self, zone_id: &str) -> anyhow::Result<Vec<Record>> {
        self.list(
            &self.zones_url,
            "wafIpAccessListsList",
            IP_ACCESS_LIST_FIELDS,
            json!({ "zoneId": zone_id }),
            100,
        )
        .await
    }

    /// Tenant-wide ("global") `IpAccessList` entries — same `zoneId: ""` (empty) convention as
    /// `tenant_wide_rules_for` above.
    pub async fn tenant_wide_ip_access_lists_for(&self) -> anyhow::Result<Vec<Record>> {
        self.list(
            &self.zones_url,
            "wafIpAccessListsList",
            IP_ACCESS_LIST_FIELDS,
            json!({ "zoneId": "" }),
            100,
        )
        .await
    }

    /// Writes one `SecurityEvent` into `alerting-service` — the up-direction, going through the
    /// same generic GraphQL mutation the portal uses, so validation/permission/outbox all still
    /// apply. This is the reason telemetry routes through this worker at all rather than the edge
    /// writing directly (see `ingest.rs`).
    pub async fn create_security_event(&self, payload: &Value) -> anyhow::Result<()> {
        let query =
            "mutation($data: Json!) { result: createWafSecurityEvents(data: $data) { id } }";
        self.graphql(&self.alerting_url, query, json!({ "data": payload }))
            .await?;
        Ok(())
    }
}
