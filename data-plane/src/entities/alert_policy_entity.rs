//! `waf.alert_policies` — Tenant-scoped (not per-Zone: 1 policy watches N zones, each zone
//! counted separately — `docs/08-module-detail-specs.md` module 8 copy note: "N event trong M
//! phút trên CÙNG 1 zone", not summed across zones). See `docs/02-domain-model.md`.

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

pub fn alert_policy_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.alert_policies".to_string(),
        label: "Alert Policy".to_string(),
        table_name: "records".to_string(),
        fields: vec![
            field("name", "Name", FieldKind::String, true, false, true),
            field("thresholdCount", "Threshold Count", FieldKind::Number, true, false, false),
            field("windowMinutes", "Window (minutes)", FieldKind::Number, true, false, false),
            field("channels", "Channels", FieldKind::Json, true, false, false),
            field("enabled", "Enabled", FieldKind::Boolean, false, true, false),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "name".to_string(),
                "thresholdCount".to_string(),
                "windowMinutes".to_string(),
                "enabled".to_string(),
            ],
            filters: vec!["enabled".to_string()],
            default_sort: Some("-createdAt".to_string()),
            max_limit: 50,
        }],
        workflow: None,
    }
}

submit_entity!(alert_policy_entity);
