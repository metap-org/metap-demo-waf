//! `waf.firewall_rules` — shared rule engine backing WAF custom rules, rate limiting, and IP/geo
//! firewall in one entity (`ruleType` just groups them for UI; `matchCondition` + `action` is
//! the same evaluate loop for all of them). See `docs/02-domain-model.md`'s `FirewallRule`
//! section.
//!
//! `matchCondition` is deliberately its own JSON grammar, not `metap-permission`'s
//! `PolicyCondition` — see `docs/05-metap-technical-mapping.md`'s "`matchCondition` của
//! `FirewallRule` — quyết định" section for why reusing that type would be wrong (no
//! `uri.*`/`header.*`/`body.*` namespace, missing `Contains`/`Regex`/`CidrMatch` operators).
//! Stored as opaque `Json` here; the grammar itself and its validation belong to `edge-plane`/
//! `control-plane`, not this entity definition.

use metap::prelude::{EntityDefinition, EntityField, EntityListView, FieldKind};

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

pub fn firewall_rule_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.firewall_rules".to_string(),
        label: "Firewall Rule".to_string(),
        table_name: "records".to_string(),
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
            },
            field("name", "Name", FieldKind::String, true, false, true),
            field("priority", "Priority", FieldKind::Number, true, false, true),
            field("matchCondition", "Match Condition", FieldKind::Json, true, false, false),
            enum_field(
                "ruleType",
                "Rule Type",
                &["waf", "rateLimit", "ipFirewall", "geoFirewall"],
                true,
                true,
            ),
            field(
                "rateLimitThreshold",
                "Rate Limit Threshold",
                FieldKind::Number,
                false,
                false,
                false,
            ),
            field("rateLimitWindow", "Rate Limit Window (s)", FieldKind::Number, false, false, false),
            enum_field("action", "Action", &["allow", "block", "challenge", "log"], true, false),
            field("enabled", "Enabled", FieldKind::Boolean, false, true, false),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "zoneId".to_string(),
                "name".to_string(),
                "priority".to_string(),
                "ruleType".to_string(),
                "action".to_string(),
                "enabled".to_string(),
            ],
            filters: vec!["zoneId".to_string(), "ruleType".to_string(), "enabled".to_string()],
            default_sort: Some("priority".to_string()),
            max_limit: 100,
        }],
        workflow: None,
    }
}
