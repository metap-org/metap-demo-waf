//! Turning a change into a published rule-set — the shared middle both entry points use.
//!
//! Two paths reach here and they must not drift, which is why they share one function:
//! - **`subscribe.rs`** (incremental): an outbox event says something about zone X changed.
//! - **`resync.rs`** (periodic): sweep every zone, whatever has or hasn't been heard about.
//!
//! The incremental path is an *optimisation*. Convergence is guaranteed by the sweep, because an
//! event can always be missed: a message dead-letters, a replica dies mid-handle, someone edits
//! the database directly. Anything that only worked because an event arrived would be a latent
//! outage.

use crate::compile::{compile_zone, publishable};
use crate::dataplane::DataPlane;
use crate::distribute::Distributor;

/// Recompiles and republishes one zone by id, or unpublishes it if it has gone away or is no
/// longer in a servable state. Idempotent — running it twice against unchanged input writes the
/// same bytes, which is what makes the periodic sweep safe to run every minute.
///
/// `known_hostname` is the hostname this zone was last published under, when the caller knows it.
/// It matters for the delete case: once the record is gone there is nothing left to read a
/// hostname from, so without it the stale rule-set would sit in Redis — still protecting (or
/// still exposing) that hostname — until the next full resync noticed.
pub async fn sync_zone(
    data_plane: &DataPlane,
    distributor: &Distributor,
    zone_id: &str,
    tenant_id: &str,
    known_hostname: Option<&str>,
) -> anyhow::Result<()> {
    let Some(zone) = data_plane.zone(zone_id).await? else {
        if let Some(hostname) = known_hostname {
            distributor.unpublish(hostname).await?;
        } else {
            tracing::debug!(zone_id, "zone deleted and no known hostname — leaving it to the next resync");
        }
        return Ok(());
    };

    let hostname = zone.str("hostname").map(str::to_string);
    if !publishable(&zone) {
        if let Some(hostname) = hostname.as_deref().or(known_hostname) {
            distributor.unpublish(hostname).await?;
        }
        return Ok(());
    }

    let ddos = data_plane.ddos_policy_for(zone_id).await?;
    let rules = data_plane.rules_for(zone_id).await?;
    let Some(compiled) = compile_zone(&zone, tenant_id, ddos.as_ref(), &rules) else {
        tracing::warn!(zone_id, "zone could not be compiled (missing hostname?), skipping");
        return Ok(());
    };

    // A hostname rename leaves the rule-set under the old key, which would keep the old hostname
    // protected by a config nobody can see in the portal any more.
    if let Some(previous) = known_hostname {
        if previous != compiled.hostname {
            distributor.unpublish(previous).await?;
        }
    }

    distributor.publish(&compiled).await
}

/// Resolves which zone a change event is about. A `waf.zones` event names the zone directly; a
/// policy/rule event names its own record, whose `zoneId` is the zone to recompile.
pub fn zone_id_from_event(entity: &str, payload: &serde_json::Value) -> Option<String> {
    let data = payload.get("data");
    match entity {
        "waf.zones" => payload.get("recordId").and_then(|v| v.as_str()).map(str::to_string),
        "waf.ddos_policies" | "waf.firewall_rules" => data
            .and_then(|d| d.get("zoneId"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        _ => None,
    }
}

/// The hostname carried in a `waf.zones` event's payload, when there is one. A delete event has
/// no `data` at all (`emit_deleted` only carries `tenantId`/`recordId`), which is exactly why
/// `sync_zone` also accepts the hostname from the caller's own index.
pub fn hostname_from_event(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("data")
        .and_then(|d| d.get("hostname"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}
