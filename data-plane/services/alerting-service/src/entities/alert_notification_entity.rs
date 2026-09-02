//! `waf.alert_notifications` — audit log of 1 actual alert send, separate from `AlertPolicy`
//! (config) since it's history, not configuration. See `docs/02-domain-model.md`. No workflow —
//! append-only, written once when a send is attempted.

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

pub fn alert_notification_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.alert_notifications".to_string(),
        label: "Alert Notification".to_string(),
        table_name: "records".to_string(),
        fields: vec![
            EntityField {
                name: "alertPolicyId".to_string(),
                label: "Alert Policy".to_string(),
                kind: FieldKind::Reference,
                required: Some(true),
                indexed: Some(true),
                unique: None,
                enum_values: None,
                ref_entity: Some("waf.alert_policies".to_string()),
                ref_display_field: Some("name".to_string()),
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
            field(
                "triggeredAt",
                "Triggered At",
                FieldKind::Datetime,
                true,
                true,
                true,
            ),
            enum_field("channel", "Channel", &["email", "webhook"], true, false),
            enum_field(
                "deliveryStatus",
                "Delivery Status",
                &["sent", "failed"],
                true,
                true,
            ),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "alertPolicyId".to_string(),
                "triggeredAt".to_string(),
                "channel".to_string(),
                "deliveryStatus".to_string(),
            ],
            filters: vec!["alertPolicyId".to_string(), "deliveryStatus".to_string()],
            default_sort: Some("-triggeredAt".to_string()),
            max_limit: 50,
        }],
        workflow: None,
    }
}

submit_entity!(alert_notification_entity);
