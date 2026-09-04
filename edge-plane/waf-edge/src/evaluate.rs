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
    pub host: &'a str,
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
