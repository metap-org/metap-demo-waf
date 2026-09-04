//! The challenge clearance cookie: proof that a client already passed a challenge recently.
//!
//! A keyed hash of `(zone, client ip, expiry)`, so a cookie cannot be moved to another IP or
//! another zone and cannot be extended by editing the expiry — the expiry is inside the hashed
//! input. No server-side state, which is what lets any node in a fleet honour a clearance issued
//! by any other node (given a shared `CLEARANCE_SECRET`).
//!
//! **Strength, stated honestly:** SHA-256 over `secret || fields` is not HMAC, and this whole
//! mechanism is only as strong as "an attacker has not read this file and does not have the
//! secret". It raises the cost of a trivial scripted flood, which is what the v1 `challenge`
//! action is for. Bot management proper is v2 (`docs/01-product-vision.md`); if this ever needs
//! to resist a motivated attacker, it needs a real HMAC and a real challenge, not a tweak here.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn sign(secret: &str, zone_id: &str, client_ip: &str, expires_at: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b"|");
    hasher.update(zone_id.as_bytes());
    hasher.update(b"|");
    hasher.update(client_ip.as_bytes());
    hasher.update(b"|");
    hasher.update(expires_at.to_string().as_bytes());
    // Hex-encoded by hand: `hex` would be a dependency for six lines.
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The `Set-Cookie` value handed out with a challenge page.
///
/// `HttpOnly` so page scripts can't read it, `SameSite=Lax` so it survives a normal navigation
/// back to the site, and `Path=/` because a challenge clears the visitor for the whole zone.
pub fn issue(cookie_name: &str, secret: &str, zone_id: &str, client_ip: &str, ttl: Duration) -> String {
    let expires_at = now_secs() + ttl.as_secs();
    let signature = sign(secret, zone_id, client_ip, expires_at);
    format!(
        "{cookie_name}={expires_at}.{signature}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax",
        ttl.as_secs()
    )
}

/// Is the cookie this client presented a valid, unexpired clearance for this zone and IP?
pub fn verify(cookie_header: &str, cookie_name: &str, secret: &str, zone_id: &str, client_ip: &str) -> bool {
    let Some(value) = cookie_value(cookie_header, cookie_name) else {
        return false;
    };
    let Some((expires_raw, signature)) = value.split_once('.') else {
        return false;
    };
    let Ok(expires_at) = expires_raw.parse::<u64>() else {
        return false;
    };
    if expires_at <= now_secs() {
        return false;
    }
    // Comparing the full hex string; a timing side channel on a challenge clearance is not a
    // meaningful threat next to the fact that this is not an HMAC to begin with (module doc).
    sign(secret, zone_id, client_ip, expires_at) == signature
}

/// Pulls one cookie out of a `Cookie:` header without a cookie-parsing dependency.
fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|pair| {
        let pair = pair.trim();
        let (key, value) = pair.split_once('=')?;
        (key == name).then_some(value)
    })
}
