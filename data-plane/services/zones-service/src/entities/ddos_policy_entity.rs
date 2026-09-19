//! `waf.ddos_policies` — DDoS L7 policy for a `Zone`.
//!
//! **`zoneId` uniqueness removed 2026-09-13** (Increment 4, anti-DDoS L7 expansion): a zone can now
//! have multiple policies at once, scoped by `pathPrefix`/`httpMethod` so different endpoints can
//! carry different thresholds (e.g. a tighter budget on `/login` than the rest of the site).
//! `priority` breaks ties the same way `FirewallRule.priority` already does — first match (by
//! scope, in priority order) wins, checked in `edge-plane/waf-edge/src/evaluate.rs`'s DDoS
//! fallback step. An absent `pathPrefix`/`httpMethod` (or `httpMethod: "any"`) matches everything,
//! preserving today's "one policy protects the whole zone" behaviour as the default shape.

use metap::prelude::{
    submit_entity, EntityAuditConfig, EntityDefinition, EntityField, EntityListView, FieldKind,
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
        table_name: metap_reconciler::qualified_table_name_in("waf.ddos_policies", "waf"),
        fields: vec![
            EntityField {
                name: "zoneId".to_string(),
                label: "Zone".to_string(),
                kind: FieldKind::Reference,
                required: Some(true),
                indexed: Some(true),
                unique: None,
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
                computed: None,
            },
            enum_field(
                "sensitivity",
                "Sensitivity",
                &["low", "medium", "high", "aggressive"],
                true,
            ),
            field(
                "requestRateThreshold",
                "Request Rate Threshold",
                FieldKind::Number,
                true,
                false,
                true,
            ),
            field(
                "burstWindow",
                "Burst Window (s)",
                FieldKind::Number,
                true,
                false,
                false,
            ),
            enum_field("action", "Action", &["log", "challenge", "block"], true),
            field("enabled", "Enabled", FieldKind::Boolean, false, true, false),
            field("priority", "Priority", FieldKind::Number, true, false, true),
            // Absent/empty = applies to every path — same "empty means unscoped" convention as
            // `FirewallRule.matchCondition`'s `None` case.
            field(
                "pathPrefix",
                "Path Prefix",
                FieldKind::String,
                false,
                false,
                false,
            ),
            enum_field(
                "httpMethod",
                "HTTP Method",
                &[
                    "any", "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS",
                ],
                false,
            ),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "zoneId".to_string(),
                "sensitivity".to_string(),
                "action".to_string(),
                "pathPrefix".to_string(),
                "httpMethod".to_string(),
                "priority".to_string(),
                "enabled".to_string(),
            ],
            filters: vec!["zoneId".to_string(), "enabled".to_string()],
            required_fields: vec![],
            default_sort: Some("priority".to_string()),
            max_limit: 50,
        }],
        workflow: None,
        unique_constraints: vec![],
        audit: Some(EntityAuditConfig {
            enabled: true,
            redacted_fields: vec![],
        }),
    }
}

submit_entity!(ddos_policy_entity);
