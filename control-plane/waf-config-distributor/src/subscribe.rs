//! The incremental path: RabbitMQ outbox events → recompile the affected zone.
//!
//! Subscribes with `metap_infra::run_resilient_consumer` rather than a hand-rolled loop — that
//! wrapper already handles reconnect-with-backoff, and `metap` learned the cost of the naive
//! version (every consumer bailing on disconnect and waiting for a process manager to restart it)
//! well enough to centralise it.
//!
//! Routing key `waf.*.record.*` catches create/update/delete on every WAF entity; the handler
//! filters to the three that actually shape edge behaviour. Subscribing narrowly per entity would
//! mean three queues and three consumers for no gain.

use std::sync::Arc;

use metap::infra::{run_resilient_consumer, ConsumedEvent, EventBus, RabbitEventBus};

use crate::dataplane::DataPlane;
use crate::distribute::Distributor;
use crate::sync::{hostname_from_event, sync_zone, zone_id_from_event};

/// Entities whose changes can alter what the edge should do. `waf.scan_*`, `waf.incidents`,
/// `waf.alert_*` all flow through the same exchange and are deliberately ignored here — none of
/// them affects request handling.
const WATCHED: [&str; 3] = ["waf.zones", "waf.ddos_policies", "waf.firewall_rules"];

pub async fn run(
    amqp_url: String,
    queue: String,
    data_plane: Arc<DataPlane>,
    distributor: Arc<Distributor>,
    shutdown: impl std::future::Future<Output = ()>,
) -> anyhow::Result<()> {
    run_resilient_consumer(
        &queue,
        "waf.*.record.*",
        None,
        || {
            let amqp_url = amqp_url.clone();
            async move { RabbitEventBus::connect(&amqp_url).await }
        },
        |event: ConsumedEvent| {
            let data_plane = data_plane.clone();
            let distributor = distributor.clone();
            async move {
                handle(&data_plane, &distributor, event).await;
            }
        },
        shutdown,
    )
    .await
}

async fn handle(data_plane: &DataPlane, distributor: &Distributor, event: ConsumedEvent) {
    // `<entity>.record.<action>` — the entity name is everything before `.record.`.
    let Some(entity) = event.routing_key.split(".record.").next() else {
        event.ack().await.ok();
        return;
    };
    if !WATCHED.contains(&entity) {
        event.ack().await.ok();
        return;
    }

    let tenant_id = event
        .payload
        .get("tenantId")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let Some(zone_id) = zone_id_from_event(entity, &event.payload) else {
        // A rule/policy event with no `zoneId` (a delete, which carries no `data`) can't be
        // resolved to a zone from the event alone. Ack it and let the periodic resync pick the
        // change up — better than requeueing forever on a message that will never carry more.
        tracing::debug!(
            routing_key = event.routing_key,
            "event carries no resolvable zone, leaving it to the next resync"
        );
        event.ack().await.ok();
        return;
    };

    let known_hostname = hostname_from_event(&event.payload);
    match sync_zone(
        data_plane,
        distributor,
        &zone_id,
        &tenant_id,
        known_hostname.as_deref(),
    )
    .await
    {
        Ok(()) => {
            event.ack().await.ok();
        }
        Err(err) => {
            // Requeue: the usual cause is `data-plane` or Redis being briefly unreachable, which
            // clears on its own. The periodic resync is the backstop if it doesn't, so this never
            // needs to be the only thing that can recover.
            tracing::warn!(error = %err, zone_id, "failed to sync zone, requeueing");
            event.nack(true).await.ok();
        }
    }
}
