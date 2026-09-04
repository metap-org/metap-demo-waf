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

#[cfg(test)]
use serde_json::json;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, status: Option<&str>, data: Value) -> Record {
        Record {
            id: id.to_string(),
            status: status.map(str::to_string),
            data: data.as_object().cloned().unwrap_or_default(),
        }
    }

    fn zone(status: &str, extra: Value) -> Record {
        let mut data = json!({
            "hostname": "shop.example.com",
            "originAddress": "10.0.0.1",
            "status": status,
            "protectionMode": "enforce",
            "configVersion": 3,
        });
        if let (Value::Object(base), Value::Object(more)) = (&mut data, extra) {
            base.extend(more);
        }
        record("zone-1", Some(status), data)
    }

    #[test]
    fn publishable_accepts_active_and_paused_only() {
        assert!(publishable(&zone("active", json!({}))));
        assert!(publishable(&zone("paused", json!({}))));
        assert!(!publishable(&zone("pending", json!({}))));
        assert!(!publishable(&zone("suspended", json!({}))));
    }

    #[test]
    fn action_from_unknown_value_defaults_to_log_not_block() {
        // A control-plane that doesn't recognise a newer action value must never start blocking
        // traffic on a guess — see the doc comment on `action_from`.
        assert_eq!(action_from(Some("something-new")), Action::Log);
        assert_eq!(action_from(None), Action::Log);
        assert_eq!(action_from(Some("block")), Action::Block);
        assert_eq!(action_from(Some("challenge")), Action::Challenge);
        assert_eq!(action_from(Some("allow")), Action::Allow);
    }

    #[test]
    fn parse_match_none_and_null_both_mean_always() {
        assert!(matches!(parse_match(None), Some(MatchExpr::Always)));
        assert!(matches!(parse_match(Some(&Value::Null)), Some(MatchExpr::Always)));
    }

    #[test]
    fn parse_match_simple_predicate() {
        let raw = json!({ "field": "uri.path", "op": "contains", "value": "/admin" });
        let expr = parse_match(Some(&raw)).unwrap();
        match expr {
            MatchExpr::Predicate(p) => {
                assert_eq!(p.field, Field::UriPath);
                assert_eq!(p.op, Op::Contains);
                assert_eq!(p.value.as_deref(), Some("/admin"));
            }
            other => panic!("expected Predicate, got {other:?}"),
        }
    }

    #[test]
    fn parse_match_header_field_lowercases_the_header_name() {
        let raw = json!({ "field": "header", "op": "eq", "value": "1", "param": "X-Custom-Header" });
        let expr = parse_match(Some(&raw)).unwrap();
        match expr {
            MatchExpr::Predicate(p) => assert_eq!(p.param.as_deref(), Some("x-custom-header")),
            other => panic!("expected Predicate, got {other:?}"),
        }
    }

    #[test]
    fn parse_match_header_field_without_param_is_unrepresentable() {
        let raw = json!({ "field": "header", "op": "eq", "value": "1" });
        assert!(parse_match(Some(&raw)).is_none());
    }

    #[test]
    fn parse_match_all_any_not_compose() {
        let raw = json!({
            "all": [
                { "field": "method", "op": "eq", "value": "POST" },
                { "not": { "field": "sourceIp", "op": "eq", "value": "10.0.0.1" } },
            ]
        });
        let expr = parse_match(Some(&raw)).unwrap();
        match expr {
            MatchExpr::All(children) => {
                assert_eq!(children.len(), 2);
                assert!(matches!(children[1], MatchExpr::Not(_)));
            }
            other => panic!("expected All, got {other:?}"),
        }
    }

    #[test]
    fn parse_match_in_requires_non_empty_values() {
        let empty = json!({ "field": "sourceIp", "op": "in", "values": [] });
        assert!(parse_match(Some(&empty)).is_none());

        let filled = json!({ "field": "sourceIp", "op": "in", "values": ["1.2.3.4", "5.6.7.8"] });
        let expr = parse_match(Some(&filled)).unwrap();
        match expr {
            MatchExpr::Predicate(p) => assert_eq!(p.values, vec!["1.2.3.4", "5.6.7.8"]),
            other => panic!("expected Predicate, got {other:?}"),
        }
    }

    #[test]
    fn parse_match_unknown_field_or_op_is_unrepresentable() {
        assert!(parse_match(Some(&json!({ "field": "bogus", "op": "eq", "value": "x" }))).is_none());
        assert!(parse_match(Some(&json!({ "field": "method", "op": "bogus", "value": "x" }))).is_none());
    }

    #[test]
    fn parse_match_missing_value_for_a_non_in_op_is_unrepresentable() {
        assert!(parse_match(Some(&json!({ "field": "method", "op": "eq" }))).is_none());
    }

    #[test]
    fn compile_rule_drops_disabled_rules() {
        let rule = record(
            "rule-1",
            None,
            json!({ "name": "r", "ruleType": "waf", "action": "block", "priority": 10, "enabled": false }),
        );
        assert!(compile_rule(&rule).is_none());
    }

    #[test]
    fn compile_rule_with_unrepresentable_condition_is_dropped() {
        let rule = record(
            "rule-1",
            None,
            json!({
                "name": "r", "ruleType": "waf", "action": "block", "priority": 10, "enabled": true,
                "matchCondition": { "field": "bogus", "op": "eq", "value": "x" },
            }),
        );
        assert!(compile_rule(&rule).is_none());
    }

    #[test]
    fn compile_rule_rate_limit_type_carries_threshold_and_window() {
        let rule = record(
            "rule-1",
            None,
            json!({
                "name": "login-rl", "ruleType": "rateLimit", "action": "challenge", "priority": 5,
                "enabled": true, "rateLimitThreshold": 20, "rateLimitWindow": 30,
            }),
        );
        let compiled = compile_rule(&rule).unwrap();
        assert_eq!(compiled.id, "rule-1");
        assert_eq!(compiled.action, Action::Challenge);
        let rl = compiled.rate_limit.expect("rate limit expected for rateLimit rule type");
        assert_eq!(rl.threshold, 20);
        assert_eq!(rl.window_seconds, 30);
        // No condition given -> Always, meaning the budget applies to every request the rule
        // type is scoped to.
        assert!(matches!(compiled.match_expr, MatchExpr::Always));
    }

    #[test]
    fn compile_rule_non_rate_limit_type_has_no_rate_limit() {
        let rule = record(
            "rule-1",
            None,
            json!({ "name": "r", "ruleType": "waf", "action": "block", "priority": 10, "enabled": true }),
        );
        let compiled = compile_rule(&rule).unwrap();
        assert!(compiled.rate_limit.is_none());
    }

    #[test]
    fn compile_ddos_drops_when_disabled() {
        let policy = record(
            "policy-1",
            None,
            json!({ "sensitivity": "high", "action": "block", "requestRateThreshold": 500, "burstWindow": 60, "enabled": false }),
        );
        assert!(compile_ddos(&policy).is_none());
    }

    #[test]
    fn compile_ddos_enabled_carries_fields_through() {
        let policy = record(
            "policy-1",
            None,
            json!({ "sensitivity": "high", "action": "block", "requestRateThreshold": 500, "burstWindow": 60, "enabled": true }),
        );
        let compiled = compile_ddos(&policy).unwrap();
        assert_eq!(compiled.sensitivity, "high");
        assert_eq!(compiled.action, Action::Block);
        assert_eq!(compiled.request_rate_threshold, 500);
        assert_eq!(compiled.burst_window_seconds, 60);
    }

    #[test]
    fn compile_zone_sorts_rules_by_priority_and_drops_unrepresentable_ones() {
        let z = zone("active", json!({}));
        let rules = vec![
            record(
                "r-high-priority-num",
                None,
                json!({ "name": "second", "ruleType": "waf", "action": "log", "priority": 200, "enabled": true }),
            ),
            record(
                "r-low-priority-num",
                None,
                json!({ "name": "first", "ruleType": "waf", "action": "block", "priority": 10, "enabled": true }),
            ),
            // Dropped: disabled.
            record(
                "r-disabled",
                None,
                json!({ "name": "disabled", "ruleType": "waf", "action": "block", "priority": 1, "enabled": false }),
            ),
        ];
        let compiled = compile_zone(&z, "tenant-1", None, &rules).unwrap();
        assert_eq!(compiled.rules.len(), 2, "the disabled rule must be dropped");
        assert_eq!(compiled.rules[0].name, "first", "lower priority number sorts first");
        assert_eq!(compiled.rules[1].name, "second");
        assert_eq!(compiled.schema_version, RULESET_SCHEMA_VERSION);
        assert_eq!(compiled.tenant_id, "tenant-1");
        assert_eq!(compiled.zone_id, "zone-1");
    }

    #[test]
    fn compile_zone_without_hostname_fails() {
        let z = record("zone-1", Some("active"), json!({ "status": "active" }));
        assert!(compile_zone(&z, "tenant-1", None, &[]).is_none());
    }

    #[test]
    fn compile_zone_carries_ddos_only_when_present_and_enabled() {
        let z = zone("active", json!({}));
        let ddos = record(
            "ddos-1",
            None,
            json!({ "sensitivity": "medium", "action": "challenge", "requestRateThreshold": 500, "burstWindow": 60, "enabled": true }),
        );
        let compiled = compile_zone(&z, "tenant-1", Some(&ddos), &[]).unwrap();
        assert!(compiled.ddos.is_some());

        let compiled_no_ddos = compile_zone(&z, "tenant-1", None, &[]).unwrap();
        assert!(compiled_no_ddos.ddos.is_none());
    }
}
