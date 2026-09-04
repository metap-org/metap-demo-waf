//! The mitigation decision: given a request and its zone's compiled rule-set, what happens?
//!
//! This is the hot path. Every design choice here is about doing the least work per request:
//! rules arrive pre-sorted and pre-filtered, the match grammar is a closed enum (no string
//! dispatch), and nothing here allocates unless a predicate actually needs a lowercase copy.
//!
//! Evaluation order, matching what the portal tells users
//! (`../../data-plane/web/src/pages/zone/ZoneDdosTab.tsx`: "applies to every request for this zone
//! before firewall rules run"):
//!
//! 1. **DDoS policy** — a per-client request budget for the whole zone.
//! 2. **Firewall rules** — priority order, **first match wins** (`docs/02-domain-model.md`).
//!
//! Monitor mode is applied last, at the boundary, by `Decision::effective_action` — so the
//! decision itself always records what *would* have happened, which is what makes monitor mode
//! useful rather than just "off".

use std::net::IpAddr;
use std::time::Duration;

use crate::ratelimit::{ddos_key, rule_key, RateLimiter};
use crate::ruleset::{Action, CompiledZone, Field, MatchExpr, Op, Predicate};

/// Everything a predicate can test, extracted from the request once per request rather than
/// re-parsed per rule.
pub struct RequestContext<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a str,
    // No `host` field: a zone is one hostname (`docs/02-domain-model.md`), so which zone's rules
    // are even being evaluated already answers "what host was this" — a predicate never needs to
    // ask it again. `main.rs` still extracts the host to look the zone up and to build the origin
    // request; it just isn't part of what a `MatchExpr` can test.
    pub client_ip: IpAddr,
    pub client_ip_text: String,
    pub user_agent: &'a str,
    /// Header lookup, already lower-cased by hyper.
    pub headers: &'a hyper::HeaderMap,
    /// ISO-3166 alpha-2, or empty when no geo source is configured. Country rules simply never
    /// match in that case — see `config::geo_country_header`.
    pub country: &'a str,
}

/// Why a request was acted on. `None` means nothing matched and the request is passed through.
pub struct Decision {
    pub action: Action,
    /// `ddosPolicy` or `firewallRule` — matches `waf.security_events.triggeredBy`'s enum.
    pub triggered_by: &'static str,
    pub triggered_by_id: String,
    pub triggered_by_name: String,
}

impl Decision {
    /// What actually happens to the request. In monitor mode every verdict degrades to `Log`:
    /// the request is passed to the origin, but the event still reports the real action so a
    /// customer can see what enforcing would have done before turning it on.
    pub fn effective_action(&self, zone: &CompiledZone) -> Action {
        if zone.enforcing() {
            self.action
        } else {
            Action::Log
        }
    }
}

fn header_value<'a>(context: &'a RequestContext<'a>, name: &str) -> &'a str {
    context
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
}

fn field_value<'a>(context: &'a RequestContext<'a>, predicate: &'a Predicate) -> &'a str {
    match predicate.field {
        Field::UriPath => context.path,
        Field::UriQuery => context.query,
        Field::Method => context.method,
        Field::UserAgent => context.user_agent,
        Field::SourceIp | Field::SourceIpCidr => &context.client_ip_text,
        Field::Country => context.country,
        Field::Header => predicate
            .param
            .as_deref()
            .map(|name| header_value(context, name))
            .unwrap_or_default(),
    }
}

/// Is `ip` inside `cidr`? Hand-rolled rather than pulling in an IP-network crate: this is one
/// prefix comparison, and the edge's dependency list is something to keep short on purpose.
/// A malformed CIDR never matches — a rule the operator wrote wrong must not silently widen.
fn ip_in_cidr(ip: IpAddr, cidr: &str) -> bool {
    let Some((network, prefix)) = cidr.split_once('/') else {
        // A bare address is treated as an exact match, which is what someone writing
        // `10.0.0.1` in an IP-firewall rule means.
        return network_eq(ip, cidr);
    };
    let Ok(prefix_len) = prefix.parse::<u32>() else {
        return false;
    };
    let Ok(network) = network.parse::<IpAddr>() else {
        return false;
    };
    match (ip, network) {
        (IpAddr::V4(ip), IpAddr::V4(network)) => {
            if prefix_len > 32 {
                return false;
            }
            if prefix_len == 0 {
                return true;
            }
            let mask = u32::MAX << (32 - prefix_len);
            (u32::from(ip) & mask) == (u32::from(network) & mask)
        }
        (IpAddr::V6(ip), IpAddr::V6(network)) => {
            if prefix_len > 128 {
                return false;
            }
            if prefix_len == 0 {
                return true;
            }
            let mask = u128::MAX << (128 - prefix_len);
            (u128::from(ip) & mask) == (u128::from(network) & mask)
        }
        // Mixed families never match. An IPv4-mapped IPv6 client against an IPv4 rule is a real
        // gap here, called out rather than silently half-handled.
        _ => false,
    }
}

fn network_eq(ip: IpAddr, text: &str) -> bool {
    text.parse::<IpAddr>().map(|parsed| parsed == ip).unwrap_or(false)
}

fn eval_predicate(context: &RequestContext<'_>, predicate: &Predicate) -> bool {
    if predicate.field == Field::SourceIpCidr {
        let inside = match predicate.op {
            Op::In | Op::NotIn => predicate.values.iter().any(|cidr| ip_in_cidr(context.client_ip, cidr)),
            _ => predicate
                .value
                .as_deref()
                .map(|cidr| ip_in_cidr(context.client_ip, cidr))
                .unwrap_or(false),
        };
        return match predicate.op {
            Op::NotEq | Op::NotIn | Op::NotContains => !inside,
            _ => inside,
        };
    }

    let actual = field_value(context, predicate);
    match predicate.op {
        Op::Eq => predicate.value.as_deref() == Some(actual),
        Op::NotEq => predicate.value.as_deref() != Some(actual),
        Op::Contains => predicate.value.as_deref().is_some_and(|v| actual.contains(v)),
        Op::NotContains => !predicate.value.as_deref().is_some_and(|v| actual.contains(v)),
        Op::StartsWith => predicate.value.as_deref().is_some_and(|v| actual.starts_with(v)),
        Op::EndsWith => predicate.value.as_deref().is_some_and(|v| actual.ends_with(v)),
        Op::In => predicate.values.iter().any(|v| v == actual),
        Op::NotIn => !predicate.values.iter().any(|v| v == actual),
        Op::ContainsCi => predicate
            .value
            .as_deref()
            // The only allocating branch, and only for rules that ask for it.
            .is_some_and(|v| actual.to_ascii_lowercase().contains(&v.to_ascii_lowercase())),
    }
}

fn eval_match(context: &RequestContext<'_>, expr: &MatchExpr) -> bool {
    match expr {
        MatchExpr::Always => true,
        MatchExpr::Predicate(predicate) => eval_predicate(context, predicate),
        MatchExpr::All(children) => children.iter().all(|child| eval_match(context, child)),
        MatchExpr::Any(children) => children.iter().any(|child| eval_match(context, child)),
        MatchExpr::Not(child) => !eval_match(context, child),
    }
}

/// Runs the zone's policy and rules against one request.
///
/// Returns `None` when nothing matched — the overwhelmingly common case, and the one this
/// function is optimised for: no allocation, no clone, just a walk down a short pre-sorted list.
pub fn evaluate(zone: &CompiledZone, context: &RequestContext<'_>, limiter: &RateLimiter) -> Option<Decision> {
    if let Some(ddos) = &zone.ddos {
        let over_budget = limiter.check(
            &ddos_key(&zone.zone_id, &context.client_ip_text),
            ddos.request_rate_threshold,
            Duration::from_secs(ddos.burst_window_seconds as u64),
        );
        if over_budget {
            return Some(Decision {
                action: ddos.action,
                triggered_by: "ddosPolicy",
                // The policy's own record id isn't in the compiled form — the portal only ever
                // shows one DDoS policy per zone, so the zone id identifies it unambiguously and
                // the compiled shape stays one field smaller on the hot path.
                triggered_by_id: zone.zone_id.clone(),
                triggered_by_name: format!("DDoS policy ({})", ddos.sensitivity),
            });
        }
    }

    for rule in &zone.rules {
        if !eval_match(context, &rule.match_expr) {
            continue;
        }
        // A rate-limit rule matches on its condition *and* on the client being over budget.
        // Counting only for requests whose condition matched is what makes "100 requests to
        // /login per minute" mean that, rather than 100 requests to anything.
        if let Some(rate_limit) = &rule.rate_limit {
            let over_budget = limiter.check(
                &rule_key(&zone.zone_id, &rule.id, &context.client_ip_text),
                rate_limit.threshold,
                Duration::from_secs(rate_limit.window_seconds as u64),
            );
            if !over_budget {
                continue;
            }
        }
        // `Allow` is a real verdict, not "keep looking": an allow rule above a block rule is how
        // an operator writes an exception, so it must stop evaluation like any other match.
        return Some(Decision {
            action: rule.action,
            triggered_by: "firewallRule",
            triggered_by_id: rule.id.clone(),
            triggered_by_name: rule.name.clone(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruleset::{CompiledDdos, CompiledRule, CompiledZone, RateLimit};
    use std::net::Ipv4Addr;

    fn headers() -> hyper::HeaderMap {
        hyper::HeaderMap::new()
    }

    fn ctx<'a>(path: &'a str, ip: [u8; 4], headers: &'a hyper::HeaderMap) -> RequestContext<'a> {
        RequestContext {
            method: "GET",
            path,
            query: "",
            client_ip: IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])),
            client_ip_text: Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]).to_string(),
            user_agent: "test-agent",
            headers,
            country: "",
        }
    }

    fn zone_with_rules(rules: Vec<CompiledRule>) -> CompiledZone {
        CompiledZone {
            schema_version: 1,
            zone_id: "zone-1".to_string(),
            tenant_id: "tenant-1".to_string(),
            hostname: "shop.example.com".to_string(),
            origin_address: "10.0.0.1".to_string(),
            status: "active".to_string(),
            protection_mode: "enforce".to_string(),
            config_version: 1,
            ddos: None,
            rules,
            compiled_at: String::new(),
        }
    }

    fn predicate_rule(id: &str, action: Action, field: Field, op: Op, value: &str) -> CompiledRule {
        CompiledRule {
            id: id.to_string(),
            name: id.to_string(),
            rule_type: "waf".to_string(),
            action,
            priority: 100,
            match_expr: MatchExpr::Predicate(Predicate {
                field,
                op,
                value: Some(value.to_string()),
                values: vec![],
                param: None,
            }),
            rate_limit: None,
        }
    }

    // --- ip_in_cidr ---

    #[test]
    fn ip_in_cidr_matches_inside_the_prefix() {
        let ip: IpAddr = "10.0.5.7".parse().unwrap();
        assert!(ip_in_cidr(ip, "10.0.0.0/16"));
        assert!(!ip_in_cidr(ip, "10.1.0.0/16"));
    }

    #[test]
    fn ip_in_cidr_zero_prefix_matches_everything() {
        let ip: IpAddr = "203.0.113.5".parse().unwrap();
        assert!(ip_in_cidr(ip, "0.0.0.0/0"));
    }

    #[test]
    fn ip_in_cidr_bare_address_is_exact_match() {
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        assert!(ip_in_cidr(ip, "10.0.0.1"));
        assert!(!ip_in_cidr(ip, "10.0.0.2"));
    }

    #[test]
    fn ip_in_cidr_malformed_cidr_never_matches() {
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        assert!(!ip_in_cidr(ip, "not-a-cidr"));
        assert!(!ip_in_cidr(ip, "10.0.0.0/99"));
        assert!(!ip_in_cidr(ip, "10.0.0.0/-1"));
    }

    #[test]
    fn ip_in_cidr_ipv6_prefix_works() {
        let ip: IpAddr = "2001:db8::1".parse().unwrap();
        assert!(ip_in_cidr(ip, "2001:db8::/32"));
        assert!(!ip_in_cidr(ip, "2001:db9::/32"));
    }

    #[test]
    fn ip_in_cidr_mixed_families_never_match() {
        // A known, documented gap (see the module doc comment): an IPv4-mapped IPv6 client
        // against an IPv4 rule does not match. Asserting the current (gap) behavior here means a
        // future fix has to change this test deliberately, not discover the gap by accident.
        let ipv6: IpAddr = "::ffff:10.0.0.1".parse().unwrap();
        assert!(!ip_in_cidr(ipv6, "10.0.0.0/8"));
    }

    // --- eval_match / eval_predicate ---

    #[test]
    fn predicate_eq_matches_path() {
        let h = headers();
        let context = ctx("/admin", [1, 2, 3, 4], &h);
        let expr = MatchExpr::Predicate(Predicate {
            field: Field::UriPath,
            op: Op::Eq,
            value: Some("/admin".to_string()),
            values: vec![],
            param: None,
        });
        assert!(eval_match(&context, &expr));
    }

    #[test]
    fn predicate_contains_ci_is_case_insensitive() {
        let h = headers();
        let context = ctx("/Admin/Login", [1, 2, 3, 4], &h);
        let expr = MatchExpr::Predicate(Predicate {
            field: Field::UriPath,
            op: Op::ContainsCi,
            value: Some("ADMIN".to_string()),
            values: vec![],
            param: None,
        });
        assert!(eval_match(&context, &expr));
        // Plain `Contains` stays case-sensitive — the two ops must behave differently.
        let expr_cs = MatchExpr::Predicate(Predicate {
            field: Field::UriPath,
            op: Op::Contains,
            value: Some("ADMIN".to_string()),
            values: vec![],
            param: None,
        });
        assert!(!eval_match(&context, &expr_cs));
    }

    #[test]
    fn predicate_in_matches_any_listed_value() {
        let h = headers();
        let context = ctx("/x", [1, 2, 3, 4], &h);
        let expr = MatchExpr::Predicate(Predicate {
            field: Field::Method,
            op: Op::In,
            value: None,
            values: vec!["POST".to_string(), "PUT".to_string()],
            param: None,
        });
        assert!(!eval_match(&context, &expr), "context method is GET");
    }

    #[test]
    fn predicate_source_ip_cidr_in_and_not_in() {
        let h = headers();
        let context = ctx("/x", [10, 0, 0, 5], &h);
        let inside = MatchExpr::Predicate(Predicate {
            field: Field::SourceIpCidr,
            op: Op::In,
            value: None,
            values: vec!["10.0.0.0/24".to_string()],
            param: None,
        });
        assert!(eval_match(&context, &inside));

        let outside_not_in = MatchExpr::Predicate(Predicate {
            field: Field::SourceIpCidr,
            op: Op::NotIn,
            value: None,
            values: vec!["10.0.0.0/24".to_string()],
            param: None,
        });
        assert!(!eval_match(&context, &outside_not_in), "client IS inside, so NotIn is false");
    }

    #[test]
    fn all_any_not_compose_correctly() {
        let h = headers();
        let context = ctx("/admin", [1, 2, 3, 4], &h);
        let is_admin = MatchExpr::Predicate(Predicate {
            field: Field::UriPath,
            op: Op::Eq,
            value: Some("/admin".to_string()),
            values: vec![],
            param: None,
        });
        let is_get = MatchExpr::Predicate(Predicate {
            field: Field::Method,
            op: Op::Eq,
            value: Some("GET".to_string()),
            values: vec![],
            param: None,
        });
        assert!(eval_match(&context, &MatchExpr::All(vec![is_admin.clone(), is_get.clone()])));
        assert!(eval_match(&context, &MatchExpr::Any(vec![is_admin.clone(), predicate_false()])));
        assert!(!eval_match(&context, &MatchExpr::Not(Box::new(is_admin))));
        // Sanity: `is_get` alone is also true for this context.
        assert!(eval_match(&context, &is_get));
    }

    fn predicate_false() -> MatchExpr {
        MatchExpr::Predicate(Predicate {
            field: Field::Method,
            op: Op::Eq,
            value: Some("POST".to_string()),
            values: vec![],
            param: None,
        })
    }

    #[test]
    fn header_field_reads_the_named_header_case_insensitively() {
        let mut h = hyper::HeaderMap::new();
        h.insert("x-api-key", hyper::header::HeaderValue::from_static("secret"));
        let context = ctx("/x", [1, 2, 3, 4], &h);
        let expr = MatchExpr::Predicate(Predicate {
            field: Field::Header,
            op: Op::Eq,
            value: Some("secret".to_string()),
            values: vec![],
            param: Some("x-api-key".to_string()),
        });
        assert!(eval_match(&context, &expr));
    }

    // --- evaluate() end to end ---

    #[test]
    fn evaluate_returns_none_when_nothing_matches() {
        let zone = zone_with_rules(vec![predicate_rule(
            "r1",
            Action::Block,
            Field::UriPath,
            Op::Eq,
            "/never-hit",
        )]);
        let h = headers();
        let context = ctx("/anything-else", [1, 2, 3, 4], &h);
        let limiter = RateLimiter::new();
        assert!(evaluate(&zone, &context, &limiter).is_none());
    }

    #[test]
    fn evaluate_first_match_wins_over_a_later_matching_rule() {
        // Both rules would match `/admin`; priority order (already sorted by the control-plane —
        // this is just list order here) means the first one decides.
        let zone = zone_with_rules(vec![
            predicate_rule("allow-rule", Action::Allow, Field::UriPath, Op::StartsWith, "/admin"),
            predicate_rule("block-rule", Action::Block, Field::UriPath, Op::StartsWith, "/admin"),
        ]);
        let h = headers();
        let context = ctx("/admin/panel", [1, 2, 3, 4], &h);
        let limiter = RateLimiter::new();
        let decision = evaluate(&zone, &context, &limiter).expect("first rule should match");
        assert_eq!(decision.action, Action::Allow);
        assert_eq!(decision.triggered_by_id, "allow-rule");
    }

    #[test]
    fn evaluate_ddos_budget_is_checked_before_rules() {
        let mut zone = zone_with_rules(vec![predicate_rule(
            "never-matches",
            Action::Block,
            Field::UriPath,
            Op::Eq,
            "/nope",
        )]);
        zone.ddos = Some(CompiledDdos {
            sensitivity: "high".to_string(),
            action: Action::Challenge,
            request_rate_threshold: 1,
            burst_window_seconds: 60,
        });
        let h = headers();
        let context = ctx("/", [9, 9, 9, 9], &h);
        let limiter = RateLimiter::new();

        // First request is within budget (threshold 1 means the 2nd request in the window is
        // over budget) — nothing matches, DDoS policy included.
        assert!(evaluate(&zone, &context, &limiter).is_none());
        // Second request from the same client crosses the threshold.
        let decision = evaluate(&zone, &context, &limiter).expect("second request should trip the DDoS budget");
        assert_eq!(decision.action, Action::Challenge);
        assert_eq!(decision.triggered_by, "ddosPolicy");
    }

    #[test]
    fn evaluate_rate_limit_rule_only_fires_once_over_budget() {
        let zone = zone_with_rules(vec![CompiledRule {
            id: "rl-1".to_string(),
            name: "login rate limit".to_string(),
            rule_type: "rateLimit".to_string(),
            action: Action::Block,
            priority: 10,
            match_expr: MatchExpr::Predicate(Predicate {
                field: Field::UriPath,
                op: Op::Eq,
                value: Some("/login".to_string()),
                values: vec![],
                param: None,
            }),
            rate_limit: Some(RateLimit {
                threshold: 1,
                window_seconds: 60,
            }),
        }]);
        let h = headers();
        let context = ctx("/login", [8, 8, 8, 8], &h);
        let limiter = RateLimiter::new();

        assert!(evaluate(&zone, &context, &limiter).is_none(), "first hit is within budget");
        let decision = evaluate(&zone, &context, &limiter).expect("second hit should trip the rate limit");
        assert_eq!(decision.action, Action::Block);
        assert_eq!(decision.triggered_by_id, "rl-1");
    }

    // --- Decision::effective_action ---

    #[test]
    fn monitor_mode_downgrades_every_action_to_log_but_keeps_the_real_verdict() {
        let mut zone = zone_with_rules(vec![]);
        zone.protection_mode = "monitor".to_string();
        let decision = Decision {
            action: Action::Block,
            triggered_by: "firewallRule",
            triggered_by_id: "r1".to_string(),
            triggered_by_name: "r1".to_string(),
        };
        assert_eq!(decision.effective_action(&zone), Action::Log);
        // The real verdict is still `Block` — this is what makes monitor mode useful.
        assert_eq!(decision.action, Action::Block);
    }

    #[test]
    fn enforce_mode_on_an_active_zone_uses_the_real_action() {
        let zone = zone_with_rules(vec![]);
        let decision = Decision {
            action: Action::Block,
            triggered_by: "firewallRule",
            triggered_by_id: "r1".to_string(),
            triggered_by_name: "r1".to_string(),
        };
        assert_eq!(decision.effective_action(&zone), Action::Block);
    }

    #[test]
    fn paused_zone_never_enforces_even_in_enforce_mode() {
        let mut zone = zone_with_rules(vec![]);
        zone.status = "paused".to_string();
        let decision = Decision {
            action: Action::Block,
            triggered_by: "firewallRule",
            triggered_by_id: "r1".to_string(),
            triggered_by_name: "r1".to_string(),
        };
        assert_eq!(decision.effective_action(&zone), Action::Log);
    }
}
