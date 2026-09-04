//! Per-client request budgets, in memory.
//!
//! **Fixed windows, not a sliding log.** A sliding window keeps a timestamp per request, which at
//! edge volumes is unbounded memory driven by attacker traffic — the wrong failure mode for the
//! component whose job is surviving a flood. A fixed window is one counter per key, resets on a
//! boundary, and its known weakness (up to 2× the budget across a boundary) is acceptable for
//! mitigation thresholds that are approximate to begin with.
//!
//! **Per-node, not global.** Each edge node counts what it sees. With N nodes behind round-robin
//! DNS the effective global budget is roughly N× the configured one. Making it exact needs a
//! shared counter on the hot path (a Redis round trip per request), which costs more than the
//! precision is worth here. Documented rather than pretended otherwise — an operator sizing a
//! threshold needs to know this.
//!
//! Sharded by key hash so concurrent requests for different clients don't queue behind one mutex.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const SHARDS: usize = 32;
/// How much idle time before a counter is dropped during pruning. Longer than any realistic
/// window so an active client's counter is never pruned mid-window.
const IDLE_EVICTION: Duration = Duration::from_secs(300);

struct Counter {
    window_start: Instant,
    hits: u32,
    last_seen: Instant,
}

pub struct RateLimiter {
    shards: Vec<Mutex<HashMap<String, Counter>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            shards: (0..SHARDS).map(|_| Mutex::new(HashMap::new())).collect(),
        }
    }

    fn shard_for(&self, key: &str) -> &Mutex<HashMap<String, Counter>> {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        &self.shards[(hasher.finish() as usize) % SHARDS]
    }

    /// Records one hit against `key` and reports whether the client is now **over** `threshold`
    /// within `window`.
    ///
    /// Counting happens before the verdict, so the request that crosses the line is itself
    /// treated as over budget — the alternative (allow the crossing request, block the next)
    /// makes a threshold of 1 meaningless.
    pub fn check(&self, key: &str, threshold: u32, window: Duration) -> bool {
        let now = Instant::now();
        let mut shard = match self.shard_for(key).lock() {
            Ok(shard) => shard,
            // A panic while holding this lock would otherwise take rate limiting out for the
            // whole shard permanently. Failing open here is the deliberate choice: dropping
            // legitimate traffic because of an internal bug is worse than briefly not
            // rate-limiting.
            Err(poisoned) => poisoned.into_inner(),
        };

        let counter = shard.entry(key.to_string()).or_insert(Counter {
            window_start: now,
            hits: 0,
            last_seen: now,
        });
        if now.duration_since(counter.window_start) >= window {
            counter.window_start = now;
            counter.hits = 0;
        }
        counter.hits += 1;
        counter.last_seen = now;
        counter.hits > threshold
    }

    /// Drops counters nothing has touched recently. Called on a timer rather than during `check`
    /// so the hot path never pays for cleanup, and so a flood from many distinct IPs cannot make
    /// every request walk a growing map.
    pub fn prune(&self) {
        let now = Instant::now();
        for shard in &self.shards {
            let mut shard = match shard.lock() {
                Ok(shard) => shard,
                Err(poisoned) => poisoned.into_inner(),
            };
            shard.retain(|_, counter| now.duration_since(counter.last_seen) < IDLE_EVICTION);
        }
    }

    pub fn tracked_keys(&self) -> usize {
        self.shards
            .iter()
            .map(|shard| shard.lock().map(|s| s.len()).unwrap_or(0))
            .sum()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Rate-limit key for a firewall rule: one budget per (zone, rule, client).
pub fn rule_key(zone_id: &str, rule_id: &str, client_ip: &str) -> String {
    format!("r:{zone_id}:{rule_id}:{client_ip}")
}

/// Rate-limit key for the zone's DDoS policy: one budget per (zone, client), independent of any
/// rule.
pub fn ddos_key(zone_id: &str, client_ip: &str) -> String {
    format!("d:{zone_id}:{client_ip}")
}
