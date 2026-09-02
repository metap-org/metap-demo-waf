//! `waf.scan_findings` — 1 vulnerability found by a `ScanJob` run. See
//! `docs/02-domain-model.md`'s `ScanFinding` section and
//! `docs/08-module-detail-specs.md`'s decision #3: dedupe across runs of the same `ScanJob` uses
//! `(scanJobId, category, endpoint)` as the identity key (there's no separate `ScanRun` entity —
//! only `lastSeenAt` distinguishes "still present" from "new") — that dedupe logic lives wherever
//! the scan engine writes results back (out of portal/data-plane scope, `docs/13-screen-api-map.md`),
//! not here; this entity just needs `firstSeenAt`/`lastSeenAt` to support it.

use metap::prelude::{
    submit_entity, EntityDefinition, EntityField, EntityListView, EntityWorkflow, FieldKind,
    WorkflowTransition,
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
        computed: None,
    }
}

fn enum_field(
    name: &str,
    label: &str,
    values: &[&str],
    required: bool,
    indexed: bool,
) -> EntityField {
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

pub fn scan_finding_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.scan_findings".to_string(),
        label: "Scan Finding".to_string(),
        table_name: "records".to_string(),
        fields: vec![
            EntityField {
                name: "scanJobId".to_string(),
                label: "Scan Job".to_string(),
                kind: FieldKind::Reference,
                required: Some(true),
                indexed: Some(true),
                unique: None,
                enum_values: None,
                ref_entity: Some("waf.scan_jobs".to_string()),
                ref_display_field: Some("scanType".to_string()),
                searchable: None,
                search_mode: None,
                sortable: None,
                storage: None,
                min: None,
                max: None,
                min_length: None,
                max_length: None,
                computed: None,
            },
            enum_field(
                "severity",
                "Severity",
                &["info", "low", "medium", "high", "critical"],
                true,
                true,
            ),
            field("category", "Category", FieldKind::String, true, true, true),
            field(
                "endpoint",
                "Endpoint",
                FieldKind::String,
                true,
                false,
                false,
            ),
            field(
                "description",
                "Description",
                FieldKind::String,
                false,
                false,
                false,
            ),
            enum_field(
                "remediationStatus",
                "Remediation Status",
                &["open", "confirmed", "falsePositive", "fixed", "accepted"],
                false,
                true,
            ),
            field(
                "firstSeenAt",
                "First Seen At",
                FieldKind::Datetime,
                false,
                false,
                true,
            ),
            field(
                "lastSeenAt",
                "Last Seen At",
                FieldKind::Datetime,
                false,
                false,
                true,
            ),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "scanJobId".to_string(),
                "severity".to_string(),
                "category".to_string(),
                "endpoint".to_string(),
                "remediationStatus".to_string(),
                "lastSeenAt".to_string(),
            ],
            filters: vec![
                "scanJobId".to_string(),
                "severity".to_string(),
                "remediationStatus".to_string(),
            ],
            default_sort: Some("-lastSeenAt".to_string()),
            max_limit: 50,
        }],
        workflow: Some(EntityWorkflow {
            state_field: "remediationStatus".to_string(),
            initial_state: "open".to_string(),
            terminal_states: vec![
                "fixed".to_string(),
                "falsePositive".to_string(),
                "accepted".to_string(),
            ],
            transitions: vec![
                transition("confirm", "open", "confirmed", "Confirm"),
                transition("markFixed", "confirmed", "fixed", "Mark Fixed"),
                transition(
                    "markFalsePositive",
                    "open",
                    "falsePositive",
                    "Mark False Positive",
                ),
                transition("accept", "open", "accepted", "Accept Risk"),
            ],
        }),
    }
}

submit_entity!(scan_finding_entity);
