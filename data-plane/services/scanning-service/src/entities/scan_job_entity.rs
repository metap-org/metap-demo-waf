//! `waf.scan_jobs` — a recurring scan config (not a single run), see `docs/02-domain-model.md`'s
//! `ScanJob` section. `status` tracks the *latest run*, not the job's own lifecycle — it loops
//! (`idle/completed/failed → queued → running → completed/failed`), it doesn't terminate.
//!
//! `terminal_states` is left empty on purpose: confirmed (`docs/08-module-detail-specs.md`
//! decision #1's research) that `metap-workflow` doesn't actually enforce `terminal_states` at
//! runtime — it's descriptive metadata only — but a recurring job genuinely has no terminal
//! state, so leaving it empty is the accurate description, not a workaround.
//!
//! Actually *running* a scan (the DAST engine itself) is out of this entity's/portal's scope —
//! see `docs/13-screen-api-map.md`'s module 5 note: this is a `waf.scan_jobs`/`waf.scan_findings`
//! storage/config concern only, same boundary as `edge-plane` executing WAF separately from
//! `data-plane` holding its config. Something external reads `status: queued` records and drives
//! them through `start`/`complete`/`fail` via the generic transition API.
//!
//! **`zoneId` is `String`, not `Reference`, since the pillar split (2026-09-01).** `waf.zones`
//! is owned by `zones-service`, a separate binary from this one — `Reference`'s
//! `validate_references()` requires the target `EntityDefinition` registered in the SAME
//! `MetadataRegistry` (confirmed against `metap-metadata`'s own source), and registering
//! `zone_entity()` here just to satisfy that would *also* expose full CRUD for `waf.zones` on
//! this service's generic `/api/:entity*` route — the same shape of problem
//! `incident_entity.rs`'s `assignedTo` doc comment already documents for a platform-level
//! entity. Same workaround: plain `String`, no FK, no auto `relatedDisplay` — the FE resolves a
//! zone's hostname by calling `zones-service` directly (or via the GraphQL gateway once wired).

use metap::prelude::{
    submit_entity, EntityDefinition, EntityField, EntityListView, EntityWorkflow, FieldKind, WorkflowTransition,
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

pub fn scan_job_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.scan_jobs".to_string(),
        label: "Scan Job".to_string(),
        table_name: "records".to_string(),
        fields: vec![
            field("zoneId", "Zone", FieldKind::String, true, true, false),
            enum_field("scanType", "Scan Type", &["quickScan", "fullScan", "apiScan"], true, true),
            field("schedule", "Schedule (cron)", FieldKind::String, false, false, false),
            enum_field(
                "status",
                "Status",
                &["idle", "queued", "running", "completed", "failed"],
                false,
                true,
            ),
            field("lastRunAt", "Last Run At", FieldKind::Datetime, false, false, true),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "zoneId".to_string(),
                "scanType".to_string(),
                "schedule".to_string(),
                "status".to_string(),
                "lastRunAt".to_string(),
            ],
            filters: vec!["zoneId".to_string(), "status".to_string()],
            default_sort: Some("-createdAt".to_string()),
            max_limit: 50,
        }],
        workflow: Some(EntityWorkflow {
            state_field: "status".to_string(),
            initial_state: "idle".to_string(),
            terminal_states: vec![],
            transitions: vec![
                transition("run", "idle", "queued", "Run"),
                transition("run", "completed", "queued", "Run again"),
                transition("run", "failed", "queued", "Retry"),
                transition("start", "queued", "running", "Start"),
                transition("complete", "running", "completed", "Complete"),
                transition("fail", "running", "failed", "Fail"),
            ],
        }),
    }
}

submit_entity!(scan_job_entity);
