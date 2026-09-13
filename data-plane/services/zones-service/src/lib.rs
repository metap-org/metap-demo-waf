//! Library target, alongside `main.rs`'s binary target — added so integration tests
//! (`tests/firewall_rule_match_condition_guard_postgres.rs`) can exercise this crate's own
//! middleware/route logic directly, the same "binary + lib" shape `metap`'s own ops binaries
//! already use (`outbox-publisher`, `notification-worker`, `cron-scheduler`). `main.rs` pulls its
//! modules from here rather than declaring them itself, so there is exactly one definition of
//! each, never two copies to drift.

pub mod apex_domain;
pub mod entities;
pub mod ip_format;
pub mod match_condition;
pub mod routes;
