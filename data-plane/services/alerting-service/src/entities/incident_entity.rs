//! `waf.incidents` — groups related `SecurityEvent`s into 1 SOC-manageable case. See
//! `docs/02-domain-model.md`'s `Incident` section. `assignedTo` is `String` (a user id/email),
//! not `Reference` — same reason as `jira-server`'s `watcher_entity.rs`'s `userEmail`:
//! `metap`'s users live in the platform-level `control` schema, not as a registered
//! `EntityDefinition` in this app's own `MetadataRegistry`, so `validate_references()` would
//! reject a `Reference` pointing at it (confirmed against `metap-metadata`'s own test suite).
//!
//! **`zoneId` is `String` for the exact same class of reason, since the pillar split
//! (2026-09-01)**: `waf.zones` now lives in a separate binary (`zones-service`), and registering
//! `zone_entity()` here just to pass `validate_references()` would leak full CRUD for
//! `waf.zones` onto this service's `/api/:entity*` route — see
//! `scanning-service/src/entities/scan_job_entity.rs`'s `zoneId` doc comment for the full
//! explanation.
//!
//! The correlation logic that actually *creates* an Incident from N `SecurityEvent`s is out of
//! this entity's/portal's scope — real business logic, not CRUD (`docs/13-screen-api-map.md`
//! module 7). This entity is just where the result lands.

use metap::prelude::{
    submit_entity, submit_field_display_hints, EntityDefinition, EntityField, EntityListView, EntityWorkflow,
    FieldDisplayHint, FieldKind, WorkflowTransition,
};

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

fn transition(action: &str, from: &str, to: &str, label: &str) -> WorkflowTransition {
    WorkflowTransition {
        action: action.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        label: label.to_string(),
        guard: None,
        validator: None,
        set_fields: None,
    }
}

pub fn incident_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.incidents".to_string(),
        label: "Incident".to_string(),
        table_name: "records".to_string(),
        fields: vec![
            field("zoneId", "Zone", FieldKind::String, true, true, false),
            field("title", "Title", FieldKind::String, true, false, true),
            enum_field("severity", "Severity", &["low", "medium", "high", "critical"], true, true),
            enum_field(
                "status",
                "Status",
                &["open", "acknowledged", "mitigating", "resolved"],
                false,
                true,
            ),
            field("eventCount", "Event Count", FieldKind::Number, false, false, true),
            field("assignedTo", "Assigned To", FieldKind::String, false, true, false),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "title".to_string(),
                "zoneId".to_string(),
                "severity".to_string(),
                "status".to_string(),
                "assignedTo".to_string(),
            ],
            filters: vec![
                "zoneId".to_string(),
                "severity".to_string(),
                "status".to_string(),
                "assignedTo".to_string(),
            ],
            default_sort: Some("-createdAt".to_string()),
            max_limit: 50,
        }],
        workflow: Some(EntityWorkflow {
            state_field: "status".to_string(),
            initial_state: "open".to_string(),
            terminal_states: vec!["resolved".to_string()],
            transitions: vec![
                transition("acknowledge", "open", "acknowledged", "Acknowledge"),
                transition("startMitigating", "acknowledged", "mitigating", "Start Mitigating"),
                transition("resolve", "mitigating", "resolved", "Resolve"),
            ],
        }),
    }
}

submit_entity!(incident_entity);

/// `assignedTo` is a `metap` user id (see the entity's own doc comment for why it can't be a
/// `Reference`) — this tells `@metap/platform-ui`'s generic list/detail views to resolve it to
/// an email via `GET /users` instead of showing the raw id. See `FieldDisplayHint`'s doc comment
/// (`metap-metadata`) for why this is a separate registration, not a field on `EntityField`.
fn incident_field_display_hints() -> Vec<FieldDisplayHint> {
    vec![FieldDisplayHint {
        field: "assignedTo".to_string(),
        resolve_via: "users".to_string(),
    }]
}

submit_field_display_hints!("waf.incidents", incident_field_display_hints);
