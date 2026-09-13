//! Compiles `Op::Regex` patterns once, not per request.
//!
//! `Predicate::value` carries the raw pattern text over the wire (a `Regex` object can't be
//! serialized), so there's no single point where a `CompiledZone` load could pre-compile every
//! pattern once and hand back a ready-to-use object without restructuring the wire types. Instead,
//! this caches by pattern text: the first request that hits a given pattern pays the compile cost,
//! every later request (same pattern, any zone) hits the cache — matching the same "hot path does
//! the minimum work" principle `evaluate.rs`'s own doc comment states, just applied lazily instead
//! of at load time.
//!
//! The control-plane already validates a pattern compiles before publishing it
//! (`compile.rs`'s `parse_match`) and drops the rule otherwise, so a pattern arriving here should
//! always be valid — but a defensively-wrong or hand-edited Redis value must still never panic the
//! edge, so a bad pattern just never matches (same "malformed input never widens what matches"
//! rule `evaluate.rs`'s `ip_in_cidr` already follows).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use regex::Regex;

fn cache() -> &'static Mutex<HashMap<String, Option<Arc<Regex>>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<Regex>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// `None` for a pattern that fails to compile — cached too, so a bad pattern doesn't re-attempt
/// compilation on every request that hits it.
pub fn compiled(pattern: &str) -> Option<Arc<Regex>> {
    let mut cache = cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(entry) = cache.get(pattern) {
        return entry.clone();
    }
    let compiled = Regex::new(pattern).ok().map(Arc::new);
    cache.insert(pattern.to_string(), compiled.clone());
    compiled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_and_caches_a_valid_pattern() {
        let first = compiled(r"^/admin/\d+$").expect("valid pattern compiles");
        assert!(first.is_match("/admin/42"));
        assert!(!first.is_match("/admin/abc"));
        // Second call must hit the cache (same Arc, not a fresh compile) — Arc::ptr_eq proves it.
        let second = compiled(r"^/admin/\d+$").expect("cached pattern still returns Some");
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn an_invalid_pattern_returns_none_and_stays_none() {
        assert!(compiled("[invalid(regex").is_none());
        // Cached as a miss too — calling again must not panic or somehow succeed.
        assert!(compiled("[invalid(regex").is_none());
    }
}
