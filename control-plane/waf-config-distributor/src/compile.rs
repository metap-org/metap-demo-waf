//! `data-plane` records → `ruleset::CompiledZone`. The "compile" half of this worker's name.
//!
//! Everything decided here is something the edge then never has to decide per request:
//! - a zone that must not be served at all is filtered out (`publishable`);
//! - disabled rules and disabled DDoS policies are dropped, not carried with a flag;
//! - rules are sorted by priority once, so "first match wins" is a plain iteration at the edge;
//! - the portal's authoring shape for `matchCondition` is translated into the fixed compiled
//!   grammar (`ruleset::MatchExpr`).
//!
//! That last one is where the still-open authoring-grammar question
//! (`data-plane/docs/02-domain-model.md`) is absorbed. `parse_match` accepts what the portal
//! writes today and is the single function to change when that question is answered — the edge
//! contract does not move.

use serde_json::Value;

use crate::dataplane::Record;
use crate::ruleset::{
    Action, CompiledDdos, CompiledRule, CompiledZone, Field, MatchExpr, Op, Predicate, RateLimit,
    RULESET_SCHEMA_VERSION,
};

/// Should this zone be served at the edge at all?
///
/// Only `active` and `paused` are published. `pending` has never been verified, and `suspended`
/// is terminal — publishing either would put a hostname into the edge's routing table that the
/// product says is not protected. `paused` *is* published because pausing means "stop enforcing",
/// not "stop serving": the edge still needs the origin address to pass traffic through, and
/// `protection_mode` handling is what makes it inert.
pub fn publishable(zone: &Record) -> bool {
    matches!(
        zone.str("status").or(zone.status.as_deref()),
        Some("active") | Some("paused")
    )
}

fn action_from(raw: Option<&str>) -> Action {
    match raw {
        Some("block") => Action::Block,
        Some("challenge") => Action::Challenge,
        Some("allow") => Action::Allow,
        // `log` and anything unrecognised both land here. Defaulting an unknown action to `Log`
        // rather than `Block` is deliberate: a control-plane that doesn't understand a newer
        // action value must not start blocking traffic on a guess.
        _ => Action::Log,
    }
}

fn field_from(raw: &str) -> Option<Field> {
    match raw {
        "uri.path" | "uriPath" => Some(Field::UriPath),
        "uri.query" | "uriQuery" => Some(Field::UriQuery),
        "method" | "http.method" => Some(Field::Method),
        "header" | "http.header" => Some(Field::Header),
        "ip" | "source.ip" | "sourceIp" => Some(Field::SourceIp),
        "ip.cidr" | "sourceIpCidr" => Some(Field::SourceIpCidr),
        "country" | "geo.country" => Some(Field::Country),
        "userAgent" | "http.userAgent" => Some(Field::UserAgent),
        _ => None,
    }
}

fn op_from(raw: &str) -> Option<Op> {
    match raw {
        "eq" | "equals" => Some(Op::Eq),
        "ne" | "neq" | "notEquals" => Some(Op::NotEq),
        "contains" => Some(Op::Contains),
        "notContains" => Some(Op::NotContains),
        "containsCi" | "icontains" => Some(Op::ContainsCi),
        "startsWith" => Some(Op::StartsWith),
        "endsWith" => Some(Op::EndsWith),
        "in" => Some(Op::In),
        "notIn" => Some(Op::NotIn),
        _ => None,
    }
}

/// Translates one authored `matchCondition` into the compiled grammar.
///
/// Returns `None` for anything it cannot represent — and the caller **drops that rule** rather
/// than publishing it with a weakened condition. A firewall rule that matches more than its
/// author wrote is worse than a firewall rule that is missing: the missing one shows up as
/// "my rule isn't working", the broadened one silently blocks real traffic.
pub fn parse_match(raw: Option<&Value>) -> Option<MatchExpr> {
    let Some(raw) = raw else {
        // No condition at all is meaningful for IP-list and rate-limit rules: the rule applies to
        // every request and the rate-limit budget (or the value list) is the real condition.
        return Some(MatchExpr::Always);
    };
    match raw {
        Value::Null => Some(MatchExpr::Always),
        Value::Object(object) => {
            if let Some(Value::Array(items)) = object.get("all") {
                let parsed: Option<Vec<_>> = items.iter().map(|item| parse_match(Some(item))).collect();
                return Some(MatchExpr::All(parsed?));
            }
            if let Some(Value::Array(items)) = object.get("any") {
                let parsed: Option<Vec<_>> = items.iter().map(|item| parse_match(Some(item))).collect();
                return Some(MatchExpr::Any(parsed?));
            }
            if let Some(inner) = object.get("not") {
                return Some(MatchExpr::Not(Box::new(parse_match(Some(inner))?)));
            }

            let field = field_from(object.get("field")?.as_str()?)?;
            let op = op_from(object.get("op")?.as_str()?)?;
            let values: Vec<String> = match object.get("values") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect(),
                _ => Vec::new(),
            };
            let value = object.get("value").and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                // A number or boolean in a request-attribute comparison is still text at the
                // edge (a header value, a path segment), so normalise here rather than making the
                // edge handle three JSON types.
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            });
            if matches!(op, Op::In | Op::NotIn) {
                if values.is_empty() {
                    return None;
                }
            } else if value.is_none() {
                return None;
            }
            let param = object
                .get("param")
                .or_else(|| object.get("header"))
                .and_then(Value::as_str)
                // Lower-cased here so the edge can compare against hyper's already-lowercased
                // header names without allocating per request.
                .map(|s| s.to_ascii_lowercase());
            if matches!(field, Field::Header) && param.is_none() {
                return None;
            }
            Some(MatchExpr::Predicate(Predicate {
                field,
                op,
                value,
                values,
                param,
            }))
        }
        _ => None,
    }
}

fn compile_rule(rule: &Record) -> Option<CompiledRule> {
    if rule.bool("enabled") == Some(false) {
        return None;
    }
    let rule_type = rule.str("ruleType").unwrap_or("waf").to_string();
    let match_expr = parse_match(rule.data.get("matchCondition"))?;
    let rate_limit = if rule_type == "rateLimit" {
        Some(RateLimit {
            threshold: rule.i64("rateLimitThreshold").unwrap_or(100).max(1) as u32,
            window_seconds: rule.i64("rateLimitWindow").unwrap_or(60).max(1) as u32,
        })
    } else {
        None
    };
    Some(CompiledRule {
        id: rule.id.clone(),
        name: rule.str("name").unwrap_or("unnamed").to_string(),
        rule_type,
        action: action_from(rule.str("action")),
        priority: rule.i64("priority").unwrap_or(1000),
        match_expr,
        rate_limit,
    })
}

fn compile_ddos(policy: &Record) -> Option<CompiledDdos> {
    if policy.bool("enabled") == Some(false) {
        return None;
    }
    Some(CompiledDdos {
        sensitivity: policy.str("sensitivity").unwrap_or("medium").to_string(),
        action: action_from(policy.str("action")),
        request_rate_threshold: policy.i64("requestRateThreshold").unwrap_or(500).max(1) as u32,
        burst_window_seconds: policy.i64("burstWindow").unwrap_or(60).max(1) as u32,
    })
}

pub fn compile_zone(
    zone: &Record,
    tenant_id: &str,
    ddos: Option<&Record>,
    rules: &[Record],
) -> Option<CompiledZone> {
    let hostname = zone.str("hostname")?.to_string();
    let mut compiled: Vec<CompiledRule> = rules.iter().filter_map(compile_rule).collect();
    let dropped = rules.len() - compiled.len();
    if dropped > 0 {
        // Loud on purpose: a rule silently disappearing between the portal and the edge is
        // exactly the failure mode nobody notices until an attack gets through.
        tracing::warn!(
            hostname,
            dropped,
            "some firewall rules were not compiled (disabled, or an unrepresentable match condition)"
        );
    }
    compiled.sort_by_key(|rule| rule.priority);

    Some(CompiledZone {
        schema_version: RULESET_SCHEMA_VERSION,
        zone_id: zone.id.clone(),
        tenant_id: tenant_id.to_string(),
        hostname,
        origin_address: zone.str("originAddress").unwrap_or_default().to_string(),
        status: zone
            .str("status")
            .or(zone.status.as_deref())
            .unwrap_or("pending")
            .to_string(),
        protection_mode: zone.str("protectionMode").unwrap_or("monitor").to_string(),
        config_version: zone.i64("configVersion").unwrap_or(0),
        ddos: ddos.and_then(compile_ddos),
        rules: compiled,
        compiled_at: chrono::Utc::now().to_rfc3339(),
    })
}
