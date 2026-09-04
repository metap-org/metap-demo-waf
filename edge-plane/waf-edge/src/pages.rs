//! What a blocked or challenged visitor actually sees.
//!
//! `../../data-plane/docs/14-cloudflare-gap-analysis.md` calls this out as a real gap worth
//! closing in v1: `FirewallRule.action = block` said "chặn" but nothing defined the response, and
//! a block that returns a bare connection reset is indistinguishable from the site being down.
//! That doc's recommendation was a single default page for v1 with per-zone customisation later —
//! which is exactly what this is.
//!
//! Deliberately plain: no external assets (a blocked visitor cannot be assumed to reach a CDN),
//! no branding (per-zone branding is the v2 feature), and a ray id so a customer reporting "I got
//! blocked" gives their operator something to search for.

use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, RETRY_AFTER, SET_COOKIE};
use hyper::{Response, StatusCode};

use crate::body::full;
use crate::body::BoxBody;

/// Short random token shown to the visitor and attached to the security event, so a support
/// conversation can connect the two. Not a security control — it identifies a request, it does
/// not authorise anything.
pub fn ray_id() -> String {
    // Time-based rather than a UUID dependency: uniqueness only has to hold well enough to find
    // one request in a log, and the edge's dependency list is kept deliberately short.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

fn page(title: &str, headline: &str, detail: &str, ray: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<style>
  body {{ font: 15px/1.5 system-ui, sans-serif; background: #0b0d10; color: #e6e8eb;
         display: flex; min-height: 100vh; margin: 0; align-items: center; justify-content: center; }}
  main {{ max-width: 34rem; padding: 2rem; }}
  h1 {{ font-size: 1.35rem; margin: 0 0 .5rem; }}
  p {{ color: #9aa3ad; margin: 0 0 1rem; }}
  code {{ font-size: .8rem; color: #6f7883; }}
</style></head>
<body><main>
  <h1>{headline}</h1>
  <p>{detail}</p>
  <code>Ray ID: {ray}</code>
</main></body></html>"#
    )
}

pub fn blocked(ray: &str) -> Response<BoxBody> {
    let html = page(
        "Request blocked",
        "This request was blocked",
        "The security rules for this site do not allow this request. If you believe this is a \
         mistake, contact the site owner and quote the ray ID below.",
        ray,
    );
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        // A blocked response must never be cached by anything in between — the rule that produced
        // it can be changed at any moment, and a cached 403 would outlive the rule.
        .header(CACHE_CONTROL, "no-store")
        .body(full(html))
        .expect("static response builds")
}

/// The challenge interstitial.
///
/// **This is a demo-grade challenge, and it is important not to overstate it.** It proves the
/// client runs JavaScript and honours cookies — enough to turn away trivial scripted floods,
/// which is what the v1 `challenge` action is for. It is not a CAPTCHA and not a proof-of-work,
/// so it does not stop a determined attacker who reads this page once. Making it stronger is a
/// real piece of work (`docs/01-product-vision.md` puts bot management in v2), not a tweak here.
pub fn challenge(clearance_cookie: &str, ray: &str) -> Response<BoxBody> {
    let html = format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Checking your browser</title>
<style>
  body {{ font: 15px/1.5 system-ui, sans-serif; background: #0b0d10; color: #e6e8eb;
         display: flex; min-height: 100vh; margin: 0; align-items: center; justify-content: center; }}
  main {{ max-width: 34rem; padding: 2rem; text-align: center; }}
  h1 {{ font-size: 1.35rem; margin: 0 0 .5rem; }}
  p {{ color: #9aa3ad; }}
  code {{ font-size: .8rem; color: #6f7883; }}
</style></head>
<body><main>
  <h1>Checking your browser…</h1>
  <p>This takes a moment. You will be redirected automatically.</p>
  <code>Ray ID: {ray}</code>
  <script>setTimeout(function () {{ location.reload(); }}, 1200);</script>
</main></body></html>"#
    );
    Response::builder()
        .status(StatusCode::SERVICE_UNAVAILABLE)
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .header(CACHE_CONTROL, "no-store")
        .header(RETRY_AFTER, "2")
        .header(SET_COOKIE, clearance_cookie)
        .body(full(html))
        .expect("static response builds")
}

/// A `Host` this node has no rule-set for. Not a 404: the request never reached an origin, and
/// saying "not found" would imply it did. 421 is the accurate answer — this server is not
/// configured to serve that authority.
pub fn unknown_host() -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::MISDIRECTED_REQUEST)
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .header(CACHE_CONTROL, "no-store")
        .body(full(page(
            "Not configured",
            "This hostname is not configured",
            "No protected zone matches the host in this request.",
            "-",
        )))
        .expect("static response builds")
}

/// The origin could not be reached. Distinct from `blocked` on purpose: a customer debugging
/// their site needs to know whether the WAF stopped a request or their own server did not answer.
pub fn origin_unreachable(ray: &str) -> Response<BoxBody> {
    Response::builder()
        .status(StatusCode::BAD_GATEWAY)
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .header(CACHE_CONTROL, "no-store")
        .body(full(page(
            "Origin unreachable",
            "The origin server did not respond",
            "The request passed this site's security rules, but the origin server could not be \
             reached.",
            ray,
        )))
        .expect("static response builds")
}
