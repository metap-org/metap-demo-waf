//! `waf.ddos_policies` — DDoS L7 policy for a `Zone`. `zoneId` is `unique` because the business
//! rule is "0..1 policy in effect per zone at a time" (`docs/02-domain-model.md`); metap has no
//! separate 1:1-relationship concept, so a unique constraint on the FK field is how that's
//! expressed.

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

fn enum_field(name: &str, label: &str, values: &[&str], required: bool) -> EntityField {
    EntityField {
        enum_values: Some(values.iter().map(|v| v.to_string()).collect()),
        ..field(name, label, FieldKind::Enum, required, false, false)
    }
}

pub fn ddos_policy_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.ddos_policies".to_string(),
        label: "DDoS Policy".to_string(),
        table_name: "records".to_string(),
        fields: vec![
            EntityField {
                name: "zoneId".to_string(),
                label: "Zone".to_string(),
                kind: FieldKind::Reference,
                required: Some(true),
                indexed: Some(true),
                unique: Some(true),
                enum_values: None,
                ref_entity: Some("waf.zones".to_string()),
                ref_display_field: Some("hostname".to_string()),
                searchable: None,
                search_mode: None,
                sortable: None,
                storage: None,
                min: None,
                max: None,
                min_length: None,
                max_length: None,
            },
            enum_field("sensitivity", "Sensitivity", &["low", "medium", "high", "aggressive"], true),
            field(
                "requestRateThreshold",
                "Request Rate Threshold",
                FieldKind::Number,
                true,
                false,
                true,
            ),
            field("burstWindow", "Burst Window (s)", FieldKind::Number, true, false, false),
            enum_field("action", "Action", &["log", "challenge", "block"], true),
            field("enabled", "Enabled", FieldKind::Boolean, false, true, false),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "zoneId".to_string(),
                "sensitivity".to_string(),
                "action".to_string(),
                "enabled".to_string(),
            ],
            filters: vec!["zoneId".to_string(), "enabled".to_string()],
            default_sort: Some("-createdAt".to_string()),
            max_limit: 50,
        }],
        workflow: None,
    }
}

submit_entity!(ddos_policy_entity);
