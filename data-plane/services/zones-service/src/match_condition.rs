//! Validates a `FirewallRule.matchCondition` JSON blob against the same field/operator vocabulary
//! `control-plane/waf-config-distributor/src/compile.rs`'s `parse_match` accepts.
//!
//! Deliberately a **second, independent copy** of that vocabulary rather than a shared crate —
//! same reasoning `edge-plane`/`control-plane` already give for duplicating `ruleset.rs` byte for
//! byte (`data-plane/docs/04-architecture-boundary.md`): this plane and `control-plane` are
//! separate deploy cycles, and the wire shape is a JSON contract like an HTTP API, not shared
//! Rust code. Unlike `parse_match`, this never builds a `MatchExpr` — `zones-service` has no
//! reason to depend on `control-plane`'s types, it only needs a yes/no answer at save time.
//!
//! Kept in sync with `compile.rs` by hand. If the two ever drift, the effect is narrow and
//! self-correcting: a condition this validator wrongly accepts still gets caught (and silently
//! dropped, per `parse_match`'s own doc comment) when `waf-config-distributor` actually compiles
//! it — this is a best-effort, immediate-feedback nicety at save time, not the source of truth for
//! what the edge can represent.

use serde_json::Value;

fn field_is_known(raw: &str) -> bool {
    matches!(
        raw,
        "uri.path"
            | "uriPath"
            | "uri.query"
            | "uriQuery"
            | "method"
            | "http.method"
            | "header"
            | "http.header"
            | "ip"
            | "source.ip"
            | "sourceIp"
            | "ip.cidr"
            | "sourceIpCidr"
            | "country"
            | "geo.country"
            | "userAgent"
            | "http.userAgent"
    )
}

fn op_is_known(raw: &str) -> bool {
    matches!(
        raw,
        "eq" | "equals"
            | "ne"
            | "neq"
            | "notEquals"
            | "contains"
            | "notContains"
            | "containsCi"
            | "icontains"
            | "startsWith"
            | "endsWith"
            | "in"
            | "notIn"
            | "regex"
    )
}

/// Mirrors `parse_match`'s acceptance rules exactly (including its permissive `None`/`null` ->
/// "always matches" case) so a rule this function accepts is, as far as this vocabulary can tell,
/// one `waf-config-distributor` will actually compile rather than silently drop.
pub fn is_valid_match_condition(raw: Option<&Value>) -> bool {
    let Some(raw) = raw else {
        return true;
    };
    match raw {
        Value::Null => true,
        Value::Object(object) => {
            if let Some(Value::Array(items)) = object.get("all") {
                return items
                    .iter()
                    .all(|item| is_valid_match_condition(Some(item)));
            }
            if let Some(Value::Array(items)) = object.get("any") {
                return items
                    .iter()
                    .all(|item| is_valid_match_condition(Some(item)));
            }
            if let Some(inner) = object.get("not") {
                return is_valid_match_condition(Some(inner));
            }

            let Some(field) = object.get("field").and_then(Value::as_str) else {
                return false;
            };
            if !field_is_known(field) {
                return false;
            }
            let Some(op) = object.get("op").and_then(Value::as_str) else {
                return false;
            };
            if !op_is_known(op) {
                return false;
            }
            let has_values =
                matches!(object.get("values"), Some(Value::Array(items)) if !items.is_empty());
            let has_value = object.get("value").is_some_and(|v| !v.is_null());
            if matches!(op, "in" | "notIn") {
                if !has_values {
                    return false;
                }
            } else if !has_value {
                return false;
            }
            if field == "header" || field == "http.header" {
                let has_param = object
                    .get("param")
                    .or_else(|| object.get("header"))
                    .and_then(Value::as_str)
                    .is_some();
                if !has_param {
                    return false;
                }
            }
            if op == "regex" {
                let pattern_compiles = object
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|pattern| regex::Regex::new(pattern).is_ok());
                if !pattern_compiles {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn none_and_null_are_always_valid() {
        assert!(is_valid_match_condition(None));
        assert!(is_valid_match_condition(Some(&Value::Null)));
    }

    #[test]
    fn a_simple_predicate_is_valid() {
        let raw = json!({ "field": "uri.path", "op": "contains", "value": "/admin" });
        assert!(is_valid_match_condition(Some(&raw)));
    }

    #[test]
    fn an_unknown_field_or_op_is_invalid() {
        assert!(!is_valid_match_condition(Some(
            &json!({ "field": "bogus", "op": "eq", "value": "x" })
        )));
        assert!(!is_valid_match_condition(Some(
            &json!({ "field": "method", "op": "bogus", "value": "x" })
        )));
    }

    #[test]
    fn header_field_requires_a_param_or_header_name() {
        assert!(!is_valid_match_condition(Some(
            &json!({ "field": "header", "op": "eq", "value": "1" })
        )));
        assert!(is_valid_match_condition(Some(
            &json!({ "field": "header", "op": "eq", "value": "1", "param": "x-api-key" })
        )));
    }

    #[test]
    fn in_and_not_in_require_non_empty_values() {
        assert!(!is_valid_match_condition(Some(
            &json!({ "field": "sourceIp", "op": "in", "values": [] })
        )));
        assert!(is_valid_match_condition(Some(
            &json!({ "field": "sourceIp", "op": "in", "values": ["1.2.3.4"] })
        )));
    }

    #[test]
    fn regex_op_requires_a_pattern_that_actually_compiles() {
        assert!(is_valid_match_condition(Some(
            &json!({ "field": "uri.path", "op": "regex", "value": r"^/admin/\d+$" })
        )));
        assert!(!is_valid_match_condition(Some(
            &json!({ "field": "uri.path", "op": "regex", "value": "[invalid(regex" })
        )));
    }

    #[test]
    fn all_any_not_recurse_and_require_every_child_valid() {
        let valid = json!({
            "all": [
                { "field": "method", "op": "eq", "value": "POST" },
                { "not": { "field": "sourceIp", "op": "eq", "value": "10.0.0.1" } },
            ]
        });
        assert!(is_valid_match_condition(Some(&valid)));

        let invalid = json!({
            "any": [
                { "field": "method", "op": "eq", "value": "POST" },
                { "field": "bogus", "op": "eq", "value": "x" },
            ]
        });
        assert!(!is_valid_match_condition(Some(&invalid)));
    }
}
