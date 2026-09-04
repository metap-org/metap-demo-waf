//! **The cross-plane wire contract.** Everything `edge-plane` is allowed to know about a zone's
//! configuration is in this file — nothing else crosses the boundary.
//!
//! Two properties this shape is designed for, both from `data-plane/docs/04-architecture-boundary.md`:
//!
//! 1. **`edge-plane` never learns `data-plane`'s schema.** It does not see `metap`'s `records`
//!    table, its JSONB field bag, entity names, or workflow state names. It sees a flat,
//!    already-decided rule list. That is the whole point of compiling here rather than letting the
//!    edge read config itself.
//! 2. **Evaluation at the edge is a lookup, not a decision.** Anything that can be resolved once,
//!    centrally — priority ordering, disabled rules, a paused zone, monitor-mode downgrading — is
//!    resolved here so the hot path does the minimum work per request.
//!
//! `schema_version` is checked by the edge on load. A rule-set written by a newer control-plane
//! than the edge understands is ignored (the edge keeps serving its last good snapshot) rather
//! than half-parsed — a config-distribution bug must not become an outage.
//!
//! **The match grammar here is the *compiled* one, deliberately separate from the authoring
//! grammar.** Whether `FirewallRule.matchCondition` in the portal reuses `metap-permission`'s
//! `PolicyCondition` or gets its own syntax is still an open question in
//! `data-plane/docs/02-domain-model.md`. That question does not have to block the edge: whatever
//! the portal ends up authoring, `compile.rs` translates it into this fixed, small,
//! request-oriented form. Answering the authoring question later changes `compile.rs` only.

use serde::{Deserialize, Serialize};

/// Bumped whenever this file's shape changes incompatibly. The edge refuses anything higher than
/// the version it was built against.
pub const RULESET_SCHEMA_VERSION: u32 = 1;

/// Redis key holding one zone's compiled rule-set, keyed by hostname (what the edge has in hand
/// from the `Host` header — no lookup table needed on the hot path).
pub fn zone_key(hostname: &str) -> String {
    format!("waf:zone:{hostname}")
}

/// Redis key holding just this zone's `config_version`, so the edge's refresh ticker can ask
/// "did anything change" for a fraction of the bytes of the full rule-set. `Zone.configVersion`
/// is the agreed staleness signal (`04-architecture-boundary.md`) — not a timestamp.
pub fn zone_version_key(hostname: &str) -> String {
    format!("waf:zone-version:{hostname}")
}

/// Redis SET of every hostname currently served. The edge uses it to notice zones that have
/// appeared or disappeared; without it a deleted zone would keep being served from the edge's
/// in-memory snapshot forever.
pub const ZONE_INDEX_KEY: &str = "waf:zones";

/// Bumped on every completed full resync. An edge that has been disconnected long enough to
/// distrust its incremental state can compare this and reload everything.
pub const EPOCH_KEY: &str = "waf:ruleset-epoch";

/// What the edge does with a request. Ordered by severity — `Block` wins if two rules somehow
/// both match, though `first match wins` means that shouldn't arise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Log,
    Challenge,
    Block,
}

/// Request attributes a compiled predicate may test. A closed enum, not a string: the edge must
/// never receive a field name it has to interpret at runtime, and an unknown field arriving from
/// a newer control-plane is a rule the edge cannot honour — better to fail parsing the whole
/// zone (and keep the last good snapshot) than to silently skip a security rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    /// Path only, no query string.
    UriPath,
    UriQuery,
    Method,
    /// Value of a named header — the name lives in `Predicate::param`.
    Header,
    /// Client IP as text, for exact/`in` comparisons. CIDR membership uses `SourceIpCidr`.
    SourceIp,
    /// `value` is a CIDR string (`10.0.0.0/8`); matches if the client IP is inside it.
    SourceIpCidr,
    /// ISO-3166 alpha-2, resolved by the edge from its own geo source.
    Country,
    UserAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    Eq,
    NotEq,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    /// `values` non-empty; true when the field equals any of them.
    In,
    NotIn,
    /// Case-insensitive substring — the common case for user-agent and path matching, spelled
    /// explicitly so the edge never has to guess about casing.
    ContainsCi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Predicate {
    pub field: Field,
    pub op: Op,
    /// Single-value comparisons. `None` with `In`/`NotIn`, which read `values` instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
    /// Header name for `Field::Header`; unused otherwise. Lower-cased by `compile.rs` so the edge
    /// can compare against hyper's already-lowercased header names without allocating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

/// Boolean tree over predicates. Kept shallow in practice (the portal authors one predicate per
/// rule today) but expressed as a tree so a richer authoring grammar doesn't need a new schema
/// version to compile into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchExpr {
    /// Matches every request — how an IP-list or rate-limit-only rule with no condition compiles.
    Always,
    Predicate(Predicate),
    All(Vec<MatchExpr>),
    Any(Vec<MatchExpr>),
    Not(Box<MatchExpr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    pub threshold: u32,
    pub window_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledRule {
    /// The `FirewallRule` record id — travels back up in telemetry as `triggeredById` so the
    /// portal can show which rule fired without the edge knowing anything else about it.
    pub id: String,
    pub name: String,
    /// Informational at the edge (`waf`/`rateLimit`/`ipFirewall`/`geoFirewall`) — behaviour is
    /// fully described by `match_expr` + `rate_limit` + `action`. Carried so telemetry and logs
    /// can say what kind of rule fired.
    pub rule_type: String,
    pub action: Action,
    pub priority: i64,
    pub match_expr: MatchExpr,
    /// Present only for rate-limit rules: the rule matches when `match_expr` holds **and** the
    /// client is over this budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledDdos {
    pub sensitivity: String,
    pub action: Action,
    pub request_rate_threshold: u32,
    pub burst_window_seconds: u32,
}

/// One zone, fully resolved. Disabled rules and disabled policies are dropped during compilation
/// rather than carried with a flag — the edge should never hold a rule it must remember not to
/// apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledZone {
    pub schema_version: u32,
    pub zone_id: String,
    pub tenant_id: String,
    pub hostname: String,
    pub origin_address: String,
    /// `active` or `paused`. A `pending`/`suspended` zone is never published at all — see
    /// `compile::publishable`.
    pub status: String,
    /// `enforce` or `monitor`. In monitor mode the edge still evaluates and still reports, but
    /// downgrades every action to `Log` — done at the edge rather than by rewriting actions here
    /// so telemetry can report what *would* have happened.
    pub protection_mode: String,
    pub config_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ddos: Option<CompiledDdos>,
    /// Already sorted by `priority` ascending — first match wins, and the edge must not have to
    /// sort on the hot path.
    pub rules: Vec<CompiledRule>,
    pub compiled_at: String,
}
