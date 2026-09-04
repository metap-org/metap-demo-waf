//! The rule-set contract, as the edge sees it.
//!
//! **This is a deliberate duplicate of
//! `../../control-plane/waf-config-distributor/src/ruleset.rs`, not an oversight.** Sharing it as
//! a crate would make `edge-plane` and `control-plane` one compilation unit and therefore one
//! deploy cycle, which is the exact coupling the three-plane split exists to prevent
//! (`../../data-plane/docs/04-architecture-boundary.md`). The contract is JSON on a Redis key —
//! a wire format, like an HTTP API — and both sides owning their own parser for it is the normal
//! shape for that, not duplication of logic.
//!
//! `SCHEMA_VERSION` is what keeps the two copies honest: a rule-set written by a newer
//! control-plane than this binary understands is rejected wholesale and the previous snapshot is
//! kept, so a contract change can never be half-applied at the edge.
//!
//! Note what is *absent* here compared to the portal's model: no entity names, no workflow state
//! machine, no tenant routing, no `metap` types at all. The edge knows hostnames, rules and
//! actions. That is the whole vocabulary.

use serde::Deserialize;

/// Highest rule-set schema this binary can read.
pub const SCHEMA_VERSION: u32 = 1;

pub const ZONE_INDEX_KEY: &str = "waf:zones";
pub const EPOCH_KEY: &str = "waf:ruleset-epoch";

pub fn zone_key(hostname: &str) -> String {
    format!("waf:zone:{hostname}")
}

pub fn zone_version_key(hostname: &str) -> String {
    format!("waf:zone-version:{hostname}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Allow,
    Log,
    Challenge,
    Block,
}

impl Action {
    /// What the portal calls this in `waf.security_events.action`. `Allow` never produces an
    /// event, so it has no name here beyond the fallback.
    pub fn event_name(self) -> &'static str {
        match self {
            Action::Block => "blocked",
            Action::Challenge => "challenged",
            Action::Allow | Action::Log => "logged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    UriPath,
    UriQuery,
    Method,
    Header,
    SourceIp,
    SourceIpCidr,
    Country,
    UserAgent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    Eq,
    NotEq,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    In,
    NotIn,
    ContainsCi,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Predicate {
    pub field: Field,
    pub op: Op,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub values: Vec<String>,
    /// Header name for `Field::Header`, already lower-cased by the control-plane.
    #[serde(default)]
    pub param: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchExpr {
    Always,
    Predicate(Predicate),
    All(Vec<MatchExpr>),
    Any(Vec<MatchExpr>),
    Not(Box<MatchExpr>),
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    pub threshold: u32,
    pub window_seconds: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledRule {
    pub id: String,
    pub name: String,
    // `rule_type`/`priority` are deserialized but never read by `evaluate.rs`: priority ordering
    // already happened at the control-plane (`rules` arrives pre-sorted — see `CompiledZone`'s
    // own doc comment), and `rule_type` is informational, not behavioral (`match_expr`/
    // `rate_limit`/`action` fully describe what a rule does). Kept on the struct rather than
    // dropped from the wire contract so `{:?}` in a log line still shows them for debugging, and
    // so parsing a real control-plane payload doesn't silently discard fields a future change
    // here might want to read.
    #[allow(dead_code)]
    pub rule_type: String,
    pub action: Action,
    #[allow(dead_code)]
    pub priority: i64,
    pub match_expr: MatchExpr,
    #[serde(default)]
    pub rate_limit: Option<RateLimit>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledDdos {
    pub sensitivity: String,
    pub action: Action,
    pub request_rate_threshold: u32,
    pub burst_window_seconds: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledZone {
    pub schema_version: u32,
    pub zone_id: String,
    // Not read by any request-path logic (the edge never scopes anything by tenant — that's a
    // `data-plane` concept it deliberately doesn't know) or by cache/refresh logic. Kept for the
    // same "wire contract fidelity + shows up in `{:?}` logging" reasoning as `CompiledRule`'s
    // `rule_type`/`priority` above.
    #[allow(dead_code)]
    pub tenant_id: String,
    pub hostname: String,
    pub origin_address: String,
    pub status: String,
    pub protection_mode: String,
    pub config_version: i64,
    #[serde(default)]
    pub ddos: Option<CompiledDdos>,
    /// Already priority-sorted by the control-plane — the edge iterates, it never sorts.
    pub rules: Vec<CompiledRule>,
    /// When the control-plane compiled this zone. Not read today; kept for the same reason as
    /// `tenant_id` above, and as the obvious field a future staleness check would key off.
    #[serde(default)]
    #[allow(dead_code)]
    pub compiled_at: String,
}

impl CompiledZone {
    /// Monitor mode means "evaluate and report, change nothing". Handled here rather than by the
    /// control-plane rewriting every action to `Log`, so telemetry can still report what *would*
    /// have happened — which is the entire value of monitor mode to a customer deciding whether
    /// to enforce.
    pub fn enforcing(&self) -> bool {
        self.protection_mode == "enforce" && self.status == "active"
    }
}
