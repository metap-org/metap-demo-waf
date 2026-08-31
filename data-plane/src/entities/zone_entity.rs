//! `waf.zones` — the anchor entity every other config entity (`DdosPolicy`, `FirewallRule`)
//! hangs off via a `zoneId` reference. See `docs/02-domain-model.md`'s `Zone` section for the
//! business spec and `docs/05-metap-technical-mapping.md`'s Workflow section for why
//! `hasConfig` exists, `docs/06-onboarding-rules-lists.md` for domain ownership verification,
//! and `docs/11-onboarding-dns-resolution.md` for `dnsRoutingStatus`.
//!
//! `hasConfig` is a technical field, not a business one: `PolicyCondition` (the grammar
//! `WorkflowTransition::guard` is written in) can only compare one attribute against a
//! literal — it has no count/aggregate operator over a zone's related `DdosPolicy`/
//! `FirewallRule` records. So the "can't activate an empty zone" guard can't read those
//! related records directly; instead the app layer flips `hasConfig` to `true` whenever a
//! `DdosPolicy` or `FirewallRule` is created for this zone (and back to `false` if the last one
//! is deleted), and the `activate` guard below just checks that flag. Doesn't matter whether
//! that policy/rule is itself `enabled` (`08-module-detail-specs.md` decision #2) — toggling a
//! rule on/off is a runtime concern with its own edge-sync SLA (10-30s,
//! `04-architecture-boundary.md`), separate from "has this zone been configured at all".
//!
//! `verificationStatus`/`verificationToken`/`verificationMethod` gate `activate` alongside
//! `hasConfig` — proves the customer controls the hostname (ACME-style DNS-TXT/HTTP-file
//! challenge) before edge-plane ever routes traffic for it. `dnsRoutingStatus` is a separate,
//! non-gating field — whether the hostname's DNS has actually been pointed at edge-plane yet is
//! informational only, checked independently of ownership.

use metap::permission::{ConditionOp, PolicyValue};
use metap::prelude::{
    submit_entity, EntityDefinition, EntityField, EntityListView, EntityWorkflow, FieldKind, PolicyCondition,
    WorkflowTransition,
};
use serde_json::json;

fn field(
    name: &str,
    label: &str,
    kind: FieldKind,
    required: bool,
    indexed: bool,
    searchable: bool,
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
        searchable: searchable.then_some(true),
        search_mode: None,
        sortable: sortable.then_some(true),
        storage: None,
        min: None,
        max: None,
        min_length: None,
        max_length: None,
    }
}

fn enum_field(name: &str, label: &str, values: &[&str], required: bool, indexed: bool, sortable: bool) -> EntityField {
    EntityField {
        enum_values: Some(values.iter().map(|v| v.to_string()).collect()),
        ..field(name, label, FieldKind::Enum, required, indexed, false, sortable)
    }
}

pub fn zone_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.zones".to_string(),
        label: "Zone".to_string(),
        table_name: "records".to_string(),
        fields: vec![
            EntityField {
                unique: Some(true),
                ..field("hostname", "Hostname", FieldKind::String, true, true, true, true)
            },
            field(
                "originAddress",
                "Origin Address",
                FieldKind::String,
                true,
                false,
                false,
                false,
            ),
            enum_field(
                "status",
                "Status",
                &["pending", "active", "paused", "suspended"],
                false,
                true,
                true,
            ),
            enum_field("protectionMode", "Protection Mode", &["monitor", "enforce"], true, true, false),
            field("configVersion", "Config Version", FieldKind::Number, false, false, false, true),
            field("hasConfig", "Has Config", FieldKind::Boolean, false, false, false, false),
            field(
                "verificationToken",
                "Verification Token",
                FieldKind::String,
                false,
                false,
                false,
                false,
            ),
            enum_field(
                "verificationMethod",
                "Verification Method",
                &["dnsTxt", "httpFile"],
                false,
                false,
                false,
            ),
            enum_field(
                "verificationStatus",
                "Verification Status",
                &["unverified", "verified"],
                false,
                true,
                false,
            ),
            enum_field(
                "dnsRoutingStatus",
                "DNS Routing Status",
                &["notRouted", "routed", "unknown"],
                false,
                false,
                false,
            ),
            field("lastDnsCheckAt", "Last DNS Check At", FieldKind::Datetime, false, false, false, true),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "hostname".to_string(),
                "status".to_string(),
                "protectionMode".to_string(),
                "configVersion".to_string(),
            ],
            filters: vec!["hostname".to_string(), "status".to_string(), "protectionMode".to_string()],
            default_sort: Some("-createdAt".to_string()),
            max_limit: 50,
        }],
        workflow: Some(EntityWorkflow {
            state_field: "status".to_string(),
            initial_state: "pending".to_string(),
            terminal_states: vec!["suspended".to_string()],
            transitions: vec![
                WorkflowTransition {
                    action: "activate".to_string(),
                    from: "pending".to_string(),
                    to: "active".to_string(),
                    label: "Activate".to_string(),
                    guard: Some(PolicyCondition::All {
                        all: vec![
                            PolicyCondition::Attribute {
                                attribute: "hasConfig".to_string(),
                                op: ConditionOp::Eq,
                                value: PolicyValue::Literal { literal: json!(true) },
                            },
                            PolicyCondition::Attribute {
                                attribute: "verificationStatus".to_string(),
                                op: ConditionOp::Eq,
                                value: PolicyValue::Literal { literal: json!("verified") },
                            },
                        ],
                    }),
                    validator: None,
                    set_fields: None,
                },
                WorkflowTransition {
                    action: "pause".to_string(),
                    from: "active".to_string(),
                    to: "paused".to_string(),
                    label: "Pause".to_string(),
                    guard: None,
                    validator: None,
                    set_fields: None,
                },
                WorkflowTransition {
                    action: "resume".to_string(),
                    from: "paused".to_string(),
                    to: "active".to_string(),
                    label: "Resume".to_string(),
                    guard: None,
                    validator: None,
                    set_fields: None,
                },
                WorkflowTransition {
                    action: "suspend".to_string(),
                    from: "active".to_string(),
                    to: "suspended".to_string(),
                    label: "Suspend".to_string(),
                    guard: None,
                    validator: None,
                    set_fields: None,
                },
                WorkflowTransition {
                    action: "suspend".to_string(),
                    from: "paused".to_string(),
                    to: "suspended".to_string(),
                    label: "Suspend".to_string(),
                    guard: None,
                    validator: None,
                    set_fields: None,
                },
            ],
        }),
    }
}

submit_entity!(zone_entity);
