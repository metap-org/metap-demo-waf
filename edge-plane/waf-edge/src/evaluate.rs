//! The mitigation decision: given a request and its zone's compiled rule-set, what happens?
//!
//! This is the hot path. Every design choice here is about doing the least work per request:
//! rules arrive pre-sorted and pre-filtered, the match grammar is a closed enum (no string
//! dispatch), and nothing here allocates unless a predicate actually needs a lowercase copy.
//!
//! Evaluation order (`data-plane/docs/06-onboarding-rules-lists.md` §5a — whitelist/blacklist
//! only means something if it bypasses everything else, `DdosPolicy` included, the way real
//! Cloudflare-style IP Access Rules work):
//!
//! 1. **`IpAccessList` rules** — allow (whitelist) first, then block (blacklist), regardless of
//!    any rule's own `priority`. `priority` only tie-breaks within the same action group; a match
//!    here stops evaluation immediately, same as any other rule match below.
//! 2. **Regular firewall rules** — priority order, **first match wins** (`docs/02-domain-model.md`).
//! 3. **DDoS policy** — a per-client request budget for the whole zone, checked **only when
//!    nothing above matched**. A request already decided by an `IpAccessList` entry or a
//!    `FirewallRule` is not also counted against the DDoS budget — avoids double-guarding a
//!    request a rule has already ruled on, and is what actually makes an allow rule "bypass DDoS"
//!    true rather than aspirational.
//!
//! `compile_zone` (control-plane) is what puts `zone.rules` in this order (`IpAccessList`-derived
//! entries first, allow before block, then regular rules by priority) — this function's own loop
//! is a plain, unconditional "first match wins" walk that does not need to know a rule's origin
//! to respect the contract above.
//!
//! Monitor mode is applied last, at the boundary, by `Decision::effective_action` — so the
//! decision itself always records what *would* have happened, which is what makes monitor mode
//! useful rather than just "off".

use std::net::IpAddr;
use std::time::Duration;

use crate::ratelimit::{ddos_key, rule_key, RateLimiter};
use crate::ruleset::{Action, CompiledDdos, CompiledZone, Field, MatchExpr, Op, Predicate};

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
    text.parse::<IpAddr>()
        .map(|parsed| parsed == ip)
        .unwrap_or(false)
}

fn eval_predicate(context: &RequestContext<'_>, predicate: &Predicate) -> bool {
    if predicate.field == Field::SourceIpCidr {
        let inside = match predicate.op {
            Op::In | Op::NotIn => predicate
                .values
                .iter()
                .any(|cidr| ip_in_cidr(context.client_ip, cidr)),
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
        Op::Contains => predicate
            .value
            .as_deref()
            .is_some_and(|v| actual.contains(v)),
        Op::NotContains => !predicate
            .value
            .as_deref()
            .is_some_and(|v| actual.contains(v)),
        Op::StartsWith => predicate
            .value
            .as_deref()
            .is_some_and(|v| actual.starts_with(v)),
        Op::EndsWith => predicate
            .value
            .as_deref()
            .is_some_and(|v| actual.ends_with(v)),
        Op::In => predicate.values.iter().any(|v| v == actual),
        Op::NotIn => !predicate.values.iter().any(|v| v == actual),
        Op::ContainsCi => predicate
            .value
            .as_deref()
            // The only allocating branch, and only for rules that ask for it.
            .is_some_and(|v| {
                actual
                    .to_ascii_lowercase()
                    .contains(&v.to_ascii_lowercase())
            }),
        Op::Regex => predicate
            .value
            .as_deref()
            .and_then(crate::regex_cache::compiled)
            .is_some_and(|re| re.is_match(actual)),
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
pub fn evaluate(
    zone: &CompiledZone,
    context: &RequestContext<'_>,
    limiter: &RateLimiter,
) -> Option<Decision> {
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

    // Only reached when nothing above matched — see this module's doc comment for why DDoS is
    // the last resort rather than the first check. `zone.ddos` is already priority-sorted by the
    // control-plane (Increment 4: a zone can carry more than one scoped policy) — first policy
    // whose scope matches this request *and* whose own budget is exceeded wins, exactly the same
    // "first match wins" contract `zone.rules` above already has.
    for ddos in &zone.ddos {
        if !ddos_scope_matches(ddos, context) {
            continue;
        }
        let over_budget = limiter.check(
            &ddos_key(&zone.zone_id, &ddos.id, &context.client_ip_text),
            ddos.request_rate_threshold,
            Duration::from_secs(ddos.burst_window_seconds as u64),
        );
        if over_budget {
            return Some(Decision {
                action: ddos.action,
                triggered_by: "ddosPolicy",
                triggered_by_id: ddos.id.clone(),
                triggered_by_name: format!("DDoS policy ({})", ddos.sensitivity),
            });
        }
    }

    None
}

/// Does this policy's scope apply to the current request? Both `path_prefix`/`http_method` absent
/// (or `http_method: "any"`) means "every request" — the default shape a single, unscoped policy
/// already had before Increment 4.
fn ddos_scope_matches(ddos: &CompiledDdos, context: &RequestContext<'_>) -> bool {
    if let Some(prefix) = &ddos.path_prefix {
        if !context.path.starts_with(prefix.as_str()) {
            return false;
        }
    }
    if let Some(method) = &ddos.http_method {
        if method != "any" && method != context.method {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ruleset::{CompiledRule, RateLimit};
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
            ddos: Vec::new(),
            rules,
            compiled_at: String::new(),
        }
    }

    fn ddos_policy(
        id: &str,
        action: Action,
        threshold: u32,
        path_prefix: Option<&str>,
        http_method: Option<&str>,
        priority: i64,
    ) -> CompiledDdos {
        CompiledDdos {
            id: id.to_string(),
            sensitivity: "high".to_string(),
            action,
            request_rate_threshold: threshold,
            burst_window_seconds: 60,
            path_prefix: path_prefix.map(str::to_string),
            http_method: http_method.map(str::to_string),
            priority,
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
        assert!(
            !eval_match(&context, &outside_not_in),
            "client IS inside, so NotIn is false"
        );
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
        assert!(eval_match(
            &context,
            &MatchExpr::All(vec![is_admin.clone(), is_get.clone()])
        ));
        assert!(eval_match(
            &context,
            &MatchExpr::Any(vec![is_admin.clone(), predicate_false()])
        ));
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
    fn predicate_regex_matches_the_pattern() {
        let h = headers();
        let context = ctx("/admin/42", [1, 2, 3, 4], &h);
        let expr = MatchExpr::Predicate(Predicate {
            field: Field::UriPath,
            op: Op::Regex,
            value: Some(r"^/admin/\d+$".to_string()),
            values: vec![],
            param: None,
        });
        assert!(eval_match(&context, &expr));

        let non_matching = ctx("/admin/abc", [1, 2, 3, 4], &h);
        assert!(!eval_match(&non_matching, &expr));
    }

    #[test]
    fn predicate_regex_with_an_invalid_pattern_never_matches() {
        let h = headers();
        let context = ctx("/anything", [1, 2, 3, 4], &h);
        let expr = MatchExpr::Predicate(Predicate {
            field: Field::UriPath,
            op: Op::Regex,
            value: Some("[invalid(regex".to_string()),
            values: vec![],
            param: None,
        });
        assert!(!eval_match(&context, &expr));
    }

    #[test]
    fn header_field_reads_the_named_header_case_insensitively() {
        let mut h = hyper::HeaderMap::new();
        h.insert(
            "x-api-key",
            hyper::header::HeaderValue::from_static("secret"),
        );
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
            predicate_rule(
                "allow-rule",
                Action::Allow,
                Field::UriPath,
                Op::StartsWith,
                "/admin",
            ),
            predicate_rule(
                "block-rule",
                Action::Block,
                Field::UriPath,
                Op::StartsWith,
                "/admin",
            ),
        ]);
        let h = headers();
        let context = ctx("/admin/panel", [1, 2, 3, 4], &h);
        let limiter = RateLimiter::new();
        let decision = evaluate(&zone, &context, &limiter).expect("first rule should match");
        assert_eq!(decision.action, Action::Allow);
        assert_eq!(decision.triggered_by_id, "allow-rule");
    }

    #[test]
    fn evaluate_ddos_budget_is_checked_only_when_no_rule_matched() {
        let mut zone = zone_with_rules(vec![predicate_rule(
            "never-matches",
            Action::Block,
            Field::UriPath,
            Op::Eq,
            "/nope",
        )]);
        zone.ddos = vec![ddos_policy("ddos-1", Action::Challenge, 1, None, None, 100)];
        let h = headers();
        let context = ctx("/", [9, 9, 9, 9], &h);
        let limiter = RateLimiter::new();

        // First request is within budget (threshold 1 means the 2nd request in the window is
        // over budget) — nothing matches, DDoS policy included.
        assert!(evaluate(&zone, &context, &limiter).is_none());
        // Second request from the same client crosses the threshold. The zone's only rule never
        // matches this path, so the DDoS check is still reached and fires.
        let decision = evaluate(&zone, &context, &limiter)
            .expect("second request should trip the DDoS budget");
        assert_eq!(decision.action, Action::Challenge);
        assert_eq!(decision.triggered_by, "ddosPolicy");
        assert_eq!(decision.triggered_by_id, "ddos-1");
    }

    #[test]
    fn evaluate_scoped_ddos_policies_only_apply_to_their_own_path() {
        // Two policies on the same zone, scoped to different paths — a request to /login must
        // only ever be checked against the /login-scoped policy's own budget, never the
        // catch-all one, and vice versa.
        let mut zone = zone_with_rules(vec![]);
        zone.ddos = vec![
            ddos_policy("login-policy", Action::Block, 1, Some("/login"), None, 10),
            ddos_policy("catch-all", Action::Challenge, 1, None, None, 100),
        ];
        let h = headers();
        let limiter = RateLimiter::new();

        let login_ctx = ctx("/login", [1, 1, 1, 1], &h);
        assert!(
            evaluate(&zone, &login_ctx, &limiter).is_none(),
            "1st /login request is in budget"
        );
        let decision = evaluate(&zone, &login_ctx, &limiter)
            .expect("2nd /login request trips the /login-scoped policy");
        assert_eq!(decision.triggered_by_id, "login-policy");
        assert_eq!(decision.action, Action::Block);

        // A different client hitting a different path never touches the /login policy's budget —
        // it falls through to the catch-all policy instead, with its own independent counter.
        let other_ctx = ctx("/checkout", [2, 2, 2, 2], &h);
        assert!(
            evaluate(&zone, &other_ctx, &limiter).is_none(),
            "1st /checkout request is in budget under the catch-all policy"
        );
        let decision = evaluate(&zone, &other_ctx, &limiter)
            .expect("2nd /checkout request trips the catch-all policy");
        assert_eq!(decision.triggered_by_id, "catch-all");
        assert_eq!(decision.action, Action::Challenge);
    }

    #[test]
    fn evaluate_scoped_ddos_policy_respects_http_method() {
        let mut zone = zone_with_rules(vec![]);
        zone.ddos = vec![ddos_policy(
            "post-only",
            Action::Block,
            1,
            None,
            Some("POST"),
            10,
        )];
        let limiter = RateLimiter::new();
        let h = headers();

        let mut get_ctx = ctx("/", [3, 3, 3, 3], &h);
        get_ctx.method = "GET";
        // A GET request never matches a POST-scoped policy, however many times it repeats.
        assert!(evaluate(&zone, &get_ctx, &limiter).is_none());
        assert!(evaluate(&zone, &get_ctx, &limiter).is_none());

        let mut post_ctx = ctx("/", [3, 3, 3, 3], &h);
        post_ctx.method = "POST";
        assert!(
            evaluate(&zone, &post_ctx, &limiter).is_none(),
            "1st POST is in budget"
        );
        let decision =
            evaluate(&zone, &post_ctx, &limiter).expect("2nd POST trips the POST-scoped policy");
        assert_eq!(decision.triggered_by_id, "post-only");
    }

    #[test]
    fn evaluate_a_matching_rule_suppresses_the_ddos_check() {
        // Same threshold-1 setup as above, but this time the zone's rule matches every request
        // (`Op::StartsWith` against an empty prefix) — the DDoS budget must never be consulted,
        // even on the 2nd request that would otherwise trip it.
        let mut zone = zone_with_rules(vec![predicate_rule(
            "always-matches",
            Action::Log,
            Field::UriPath,
            Op::StartsWith,
            "",
        )]);
        zone.ddos = vec![ddos_policy("ddos-1", Action::Challenge, 1, None, None, 100)];
        let h = headers();
        let context = ctx("/", [7, 7, 7, 7], &h);
        let limiter = RateLimiter::new();

        for _ in 0..2 {
            let decision = evaluate(&zone, &context, &limiter).expect("the rule always matches");
            assert_eq!(decision.triggered_by, "firewallRule");
            assert_eq!(decision.triggered_by_id, "always-matches");
        }
    }

    #[test]
    fn evaluate_an_ip_access_list_allow_match_bypasses_ddos_and_later_rules() {
        // `rule_type: "ipAccessList"` is set here purely for realism (telemetry) — `evaluate()`
        // itself never reads it; what actually gives this rule priority is its position at the
        // front of `zone.rules`, which is `compile_zone`'s job in the real pipeline.
        let mut access_rule = predicate_rule(
            "wl-1",
            Action::Allow,
            Field::SourceIpCidr,
            Op::Eq,
            "5.5.5.5",
        );
        access_rule.rule_type = "ipAccessList".to_string();
        let block_everything = predicate_rule(
            "block-all",
            Action::Block,
            Field::UriPath,
            Op::StartsWith,
            "",
        );
        let mut zone = zone_with_rules(vec![access_rule, block_everything]);
        zone.ddos = vec![ddos_policy("ddos-1", Action::Challenge, 1, None, None, 100)];
        let h = headers();
        let context = ctx("/", [5, 5, 5, 5], &h);
        let limiter = RateLimiter::new();

        // Even on the 2nd request (which would trip the DDoS budget) and despite a
        // block-everything rule right after it, the whitelist match wins.
        for _ in 0..2 {
            let decision = evaluate(&zone, &context, &limiter).expect("the allow rule matches");
            assert_eq!(decision.action, Action::Allow);
            assert_eq!(decision.triggered_by_id, "wl-1");
        }
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

        assert!(
            evaluate(&zone, &context, &limiter).is_none(),
            "first hit is within budget"
        );
        let decision =
            evaluate(&zone, &context, &limiter).expect("second hit should trip the rate limit");
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
