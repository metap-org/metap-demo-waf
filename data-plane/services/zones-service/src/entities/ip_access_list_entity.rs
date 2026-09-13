//! `waf.ip_access_lists` — whitelist/blacklist by IP or CIDR.
//!
//! **Deliberately a separate entity, reversing a documented decision.**
//! `docs/06-onboarding-rules-lists.md` §5 originally concluded whitelist/blacklist should reuse
//! `FirewallRule.ruleType ∈ {ipFirewall, geoFirewall}` rather than get its own entity ("Không cần
//! entity `IpAccessList` riêng"). The project owner chose to reverse that for a clearer, purpose-
//! built API/UI (no `matchCondition` JSON to author or explain) rather than the generic rule
//! engine's shared shape. Geo (country-code) access rules stay on `FirewallRule` — this entity is
//! IP/CIDR only.
//!
//! `control-plane/waf-config-distributor/src/compile.rs::compile_ip_access_list` still compiles a
//! row down to the same `CompiledRule`/`Predicate` shape `FirewallRule` already produces (no new
//! wire type), and `edge-plane`'s evaluate order was changed alongside this (see
//! `edge-plane/waf-edge/src/evaluate.rs`'s module doc comment) so that a match here — allow or
//! block — is checked before `DdosPolicy`, matching real Cloudflare-style IP Access Rules
//! behaviour (§5a: "whitelist chỉ có ý nghĩa thật nếu nó bypass được cả DdosPolicy").

use metap::prelude::{
    submit_entity, EntityAuditConfig, EntityDefinition, EntityField, EntityListView, FieldKind,
};

fn field(name: &str, label: &str, kind: FieldKind, required: bool, indexed: bool) -> EntityField {
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
        sortable: None,
        storage: None,
        min: None,
        max: None,
        min_length: None,
        max_length: None,
        computed: None,
    }
}

pub fn ip_access_list_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.ip_access_lists".to_string(),
        label: "IP Access List".to_string(),
        table_name: metap_reconciler::qualified_table_name_in("waf.ip_access_lists", "waf"),
        fields: vec![
            EntityField {
                name: "zoneId".to_string(),
                label: "Zone".to_string(),
                kind: FieldKind::Reference,
                // Nullable for the same tenant-wide ("global") reason as `FirewallRule.zoneId`
                // (see `firewall_rule_entity.rs`'s own doc comment) — `null` applies to every
                // zone in the tenant.
                required: Some(false),
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
            EntityField {
                enum_values: Some(vec!["whitelist".to_string(), "blacklist".to_string()]),
                ..field("type", "Type", FieldKind::Enum, true, true)
            },
            // One IP or CIDR per row, validated at save time by
            // `routes::ip_access_list_value_guard` — see that function's doc comment. Bulk-add in
            // the portal creates N rows client-side rather than this field taking a list.
            field("value", "IP / CIDR", FieldKind::String, true, false),
            field("enabled", "Enabled", FieldKind::Boolean, false, true),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec![
                "zoneId".to_string(),
                "type".to_string(),
                "value".to_string(),
                "enabled".to_string(),
            ],
            filters: vec![
                "zoneId".to_string(),
                "type".to_string(),
                "enabled".to_string(),
            ],
            required_fields: vec![],
            default_sort: None,
            max_limit: 100,
        }],
        workflow: None,
        unique_constraints: vec![],
        audit: Some(EntityAuditConfig { enabled: true }),
    }
}

submit_entity!(ip_access_list_entity);
