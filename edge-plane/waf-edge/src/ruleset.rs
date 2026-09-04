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
    pub rule_type: String,
    pub action: Action,
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
    #[serde(default)]
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
