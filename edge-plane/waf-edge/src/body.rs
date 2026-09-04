//! One body type for everything this proxy returns.
//!
//! hyper 1.x makes the body type part of the response type, and this server produces two kinds:
//! bytes it generated itself (a block page) and bytes streaming back from an origin. `BoxBody`
//! erases that difference so every handler shares one signature.

use bytes::Bytes;
use http_body_util::combinators::BoxBody as HttpBoxBody;
use http_body_util::{BodyExt, Full};

pub type BoxBody = HttpBoxBody<Bytes, hyper::Error>;

/// A complete in-memory body — block pages, challenge pages, health responses.
pub fn full(body: impl Into<Bytes>) -> BoxBody {
    Full::new(body.into())
        // `Full` is infallible; the boxed type carries `hyper::Error` so it can also hold a
        // streaming origin response, so the error type is widened rather than the two kept apart.
        .map_err(|never| match never {})
        .boxed()
}
