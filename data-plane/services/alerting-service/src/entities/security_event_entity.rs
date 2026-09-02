//! `waf.security_events` — log of 1 request matched by a `DdosPolicy`/`FirewallRule`, written by
//! `edge-plane` (near real-time, high volume — see `docs/02-domain-model.md`'s `SecurityEvent`
//! section). No workflow — pure append-only log, never transitions state.
//!
//! `triggeredById` is `String`, not `Reference`: a `Reference` field's `ref_entity` is one fixed
//! target, but this needs to point at either `waf.ddos_policies` or `waf.firewall_rules`
//! depending on `triggeredBy` — no polymorphic reference support in `metap` (confirmed research,
//! `docs/05-metap-technical-mapping.md`). `triggeredByName` denormalizes the rule/policy's name
//! at write time (`docs/08-module-detail-specs.md` decision #4) — this entity is the highest
//! volume in the whole system, an N+1 lookup per list render doesn't scale.
//!
//! **`zoneId` is also `String`, not `Reference`, since the pillar split (2026-09-01)** — same
//! reason as `triggeredById` above in spirit, different root cause: `waf.zones` is owned by
//! `zones-service`, a separate binary from this one, and registering `zone_entity()` here just
//! to satisfy `validate_references()` would leak full CRUD for `waf.zones` onto this service's
//! `/api/:entity*` route (see `scan_job_entity.rs`'s `zoneId` doc comment in `scanning-service`
//! for the full explanation — same fix applied here).
//!
//! `table_name: "records"` for now (shared table) — table-per-entity is available
//! (`metap-reconciler` + `reconciler-orchestrator`, `docs/05-metap-technical-mapping.md`) but not
//! worth flipping on until real volume shows up; demo-scale traffic doesn't need it yet.

use metap::prelude::{submit_entity, EntityDefinition, EntityField, EntityListView, FieldKind};

fn field(
    name: &str,
    label: &str,
    kind: FieldKind,
    required: bool,
    indexed: bool,
    sortable: bool,
) -> EntityField {
    EntityField {
        name: name.to_string(),
        label: label.to_string(),
        kind,
        required: required.then_some(true),
        indexed: indexed.then_some(true),
        unique: None,
        enum_values: None,
        ref_entity: None,
        ref_display_field: None,
        searchable: None,
        search_mode: None,
        sortable: sortable.then_some(true),
        storage: None,
        min: None,
        max: None,
        min_length: None,
        max_length: None,
    }
}

fn enum_field(name: &str, label: &str, values: &[&str], required: bool, indexed: bool) -> EntityField {
    EntityField {
        enum_values: Some(values.iter().map(|v| v.to_string()).collect()),
        ..field(name, label, FieldKind::Enum, required, indexed, false)
    }
}

pub fn security_event_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.security_events".to_string(),
        label: "Security Event".to_string(),
        table_name: "records".to_string(),
        fields: vec![
            field("zoneId", "Zone", FieldKind::String, true, true, false),
            enum_field("triggeredBy", "Triggered By", &["ddosPolicy", "firewallRule"], true, true),
            field("triggeredById", "Triggered By Id", FieldKind::String, true, false, false),
            field("triggeredByName", "Triggered By Name", FieldKind::String, false, false, false),
            enum_field("action", "Action", &["logged", "challenged", "blocked"], true, true),
            field("sourceIp", "Source IP", FieldKind::String, true, true, false),
            field("requestPath", "Request Path", FieldKind::String, true, false, false),
            field("occurredAt", "Occurred At", FieldKind::Datetime, true, true, true),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "zoneId".to_string(),
                "triggeredByName".to_string(),
                "action".to_string(),
                "sourceIp".to_string(),
                "requestPath".to_string(),
                "occurredAt".to_string(),
            ],
            filters: vec![
                "zoneId".to_string(),
                "triggeredBy".to_string(),
                "action".to_string(),
                "sourceIp".to_string(),
            ],
            default_sort: Some("-occurredAt".to_string()),
            max_limit: 50,
        }],
        workflow: None,
    }
}

submit_entity!(security_event_entity);
