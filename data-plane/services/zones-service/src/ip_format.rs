//! Format validation for `waf.ip_access_lists.value`, used by
//! `routes::ip_access_list_value_guard` at save time.
//!
//! Mirrors the parsing `edge-plane/waf-edge/src/evaluate.rs::ip_in_cidr` does at match time — a
//! bare IP, or `network/prefix` with a prefix valid for that address family — but only checks the
//! value parses at all, not whether the address is the correctly-masked base of its own prefix.
//! `10.0.0.5/24` is accepted: the edge masks both sides when matching a request's source IP
//! against this value, so an unaligned network address is not an authoring mistake worth
//! rejecting, just a slightly imprecise one.

use std::net::IpAddr;

pub fn is_valid_ip_or_cidr(value: &str) -> bool {
    let Some((network, prefix)) = value.split_once('/') else {
        return value.parse::<IpAddr>().is_ok();
    };
    let Ok(network) = network.parse::<IpAddr>() else {
        return false;
    };
    let Ok(prefix_len) = prefix.parse::<u32>() else {
        return false;
    };
    match network {
        IpAddr::V4(_) => prefix_len <= 32,
        IpAddr::V6(_) => prefix_len <= 128,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_ipv4_or_ipv6_address_is_valid() {
        assert!(is_valid_ip_or_cidr("10.0.0.5"));
        assert!(is_valid_ip_or_cidr("2001:db8::1"));
    }

    #[test]
    fn a_cidr_within_range_is_valid_even_when_unaligned() {
        assert!(is_valid_ip_or_cidr("10.0.0.0/16"));
        assert!(is_valid_ip_or_cidr("10.0.0.5/24"));
        assert!(is_valid_ip_or_cidr("2001:db8::/32"));
    }

    #[test]
    fn a_prefix_out_of_range_for_the_family_is_invalid() {
        assert!(!is_valid_ip_or_cidr("10.0.0.0/33"));
        assert!(!is_valid_ip_or_cidr("2001:db8::/129"));
    }

    #[test]
    fn garbage_is_invalid() {
        assert!(!is_valid_ip_or_cidr(""));
        assert!(!is_valid_ip_or_cidr("not-an-ip"));
        assert!(!is_valid_ip_or_cidr("10.0.0.0/not-a-number"));
        assert!(!is_valid_ip_or_cidr("10.0.0.0/-1"));
    }
}
