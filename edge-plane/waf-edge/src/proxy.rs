//! Passing an allowed request through to the customer's origin.
//!
//! Scope, stated plainly: this is a **reverse proxy adequate for a mitigation demo**, not a
//! production CDN edge. It does not do connection pooling per origin beyond what
//! `hyper_util`'s client gives it, TLS to the origin is plain `http` unless the origin address
//! says otherwise, and it buffers request bodies rather than streaming them. Those are real
//! limitations, listed here rather than discovered later.
//!
//! What it does get right, because getting them wrong would be a security bug rather than a
//! performance one:
//! - hop-by-hop headers are stripped (RFC 9110 §7.6.1), so `Connection`/`Upgrade` from a client
//!   cannot leak into the origin request;
//! - `X-Forwarded-For` is *set*, not appended to whatever the client sent — a client-supplied
//!   `X-Forwarded-For` reaching the origin would let anyone forge their own source address for
//!   every downstream system that trusts that header;
//! - the origin's own hop-by-hop headers are stripped on the way back.

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::header::{HeaderName, HeaderValue, HOST};
use hyper::{Request, Response, Uri};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use crate::body::BoxBody;

/// Headers that describe one hop of a connection and must never be forwarded across it.
const HOP_BY_HOP: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

pub type ProxyClient = Client<HttpConnector, http_body_util::Full<Bytes>>;

pub fn build_client() -> ProxyClient {
    let mut connector = HttpConnector::new();
    connector.set_nodelay(true);
    // Bounded so a slow origin ties up a connection attempt for a known amount of time rather
    // than indefinitely; the caller turns a failure into a 502 page.
    connector.set_connect_timeout(Some(std::time::Duration::from_secs(5)));
    Client::builder(TokioExecutor::new()).build(connector)
}

/// Builds the origin URI from the zone's configured origin address plus the incoming path.
///
/// The address as authored in the portal may be bare (`203.0.113.10`, `origin.example.com`) or a
/// full URL. A bare address is treated as `http` — the origin hop is typically inside the
/// customer's own network, and guessing `https` for an origin that only speaks `http` fails in a
/// way that looks like the WAF broke the site.
fn origin_uri(origin_address: &str, path_and_query: &str) -> Option<Uri> {
    let base = if origin_address.starts_with("http://") || origin_address.starts_with("https://") {
        origin_address.trim_end_matches('/').to_string()
    } else {
        format!("http://{}", origin_address.trim_end_matches('/'))
    };
    format!("{base}{path_and_query}").parse::<Uri>().ok()
}

pub async fn forward(
    client: &ProxyClient,
    request: Request<hyper::body::Incoming>,
    origin_address: &str,
    client_ip: &str,
    host: &str,
) -> Result<Response<BoxBody>, ProxyError> {
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    let Some(uri) = origin_uri(origin_address, &path_and_query) else {
        return Err(ProxyError::BadOrigin);
    };

    let (parts, body) = request.into_parts();
    // Buffered, not streamed: streaming a request body through hyper 1.x means threading the
    // incoming body type through every handler, and this proxy's job is evaluating rules rather
    // than moving large uploads. A real edge would stream; this one is honest about not doing so.
    let collected = body.collect().await.map_err(|_| ProxyError::BadRequestBody)?.to_bytes();

    let mut upstream = Request::builder().method(parts.method.clone()).uri(uri);
    {
        let headers = upstream.headers_mut().expect("builder has headers");
        for (name, value) in parts.headers.iter() {
            if HOP_BY_HOP.contains(&name.as_str()) {
                continue;
            }
            headers.insert(name.clone(), value.clone());
        }
        // The origin should see the hostname the visitor asked for, not the origin's own address.
        if let Ok(value) = HeaderValue::from_str(host) {
            headers.insert(HOST, value);
        }
        // Overwritten, never appended — see the module doc.
        if let Ok(value) = HeaderValue::from_str(client_ip) {
            headers.insert(HeaderName::from_static("x-forwarded-for"), value.clone());
            headers.insert(HeaderName::from_static("x-real-ip"), value);
        }
        headers.insert(HeaderName::from_static("x-forwarded-proto"), HeaderValue::from_static("http"));
        // Lets an origin tell WAF-proxied traffic from direct traffic — the basis of any
        // "only accept traffic from the edge" origin lock-down.
        headers.insert(HeaderName::from_static("x-waf-edge"), HeaderValue::from_static("1"));
    }

    let upstream = upstream
        .body(http_body_util::Full::new(collected))
        .map_err(|_| ProxyError::BadOrigin)?;

    let response = client.request(upstream).await.map_err(|_| ProxyError::Unreachable)?;
    let (parts, body) = response.into_parts();
    let mut out = Response::builder().status(parts.status);
    {
        let headers = out.headers_mut().expect("builder has headers");
        for (name, value) in parts.headers.iter() {
            if HOP_BY_HOP.contains(&name.as_str()) {
                continue;
            }
            headers.insert(name.clone(), value.clone());
        }
    }
    // The origin's body streams back untouched; only the headers were filtered. `Incoming`'s
    // error type is already `hyper::Error`, which is what `BoxBody` carries, so this is a plain
    // box with no error mapping.
    out.body(body.boxed()).map_err(|_| ProxyError::Unreachable)
}

#[derive(Debug)]
pub enum ProxyError {
    /// The zone's configured origin address does not form a usable URL.
    BadOrigin,
    BadRequestBody,
    Unreachable,
}
