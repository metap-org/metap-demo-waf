//! The apex-domain grouping algorithm `docs/06-onboarding-rules-lists.md` §1 already accepted for
//! v1: the last 2 labels of a hostname, split on `.` — no Public Suffix List, so a compound TLD
//! like `.co.uk` groups on `co.uk` rather than the real apex. Accepted trade-off at this scale, not
//! a bug to fix later without a real PSL dependency.
//!
//! Used both by `routes::zone_domain_guard` (auto-attach a new `Zone` to its `Domain` at create
//! time) and by the one-off backfill migration that grouped pre-existing zones the same way.

/// A hostname with fewer than 2 labels (a bare TLD, or already just 2 labels) returns itself
/// unchanged — there's nothing shorter to group on.
pub fn apex_domain(hostname: &str) -> String {
    let labels: Vec<&str> = hostname.trim_end_matches('.').split('.').collect();
    if labels.len() <= 2 {
        return hostname.to_string();
    }
    labels[labels.len() - 2..].join(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_apex_hostname_is_unchanged() {
        assert_eq!(apex_domain("example.com"), "example.com");
    }

    #[test]
    fn a_subdomain_reduces_to_its_apex() {
        assert_eq!(apex_domain("shop.example.com"), "example.com");
        assert_eq!(apex_domain("api.shop.example.com"), "example.com");
    }

    #[test]
    fn a_single_label_hostname_is_unchanged() {
        assert_eq!(apex_domain("localhost"), "localhost");
    }

    #[test]
    fn a_compound_tld_is_not_specially_handled_accepted_v1_limitation() {
        // Groups on "co.uk", not the real apex "example.co.uk" — documented limitation, not a bug.
        assert_eq!(apex_domain("shop.example.co.uk"), "co.uk");
    }
}
