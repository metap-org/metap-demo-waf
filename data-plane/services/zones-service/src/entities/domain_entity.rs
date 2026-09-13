//! `waf.domains` — the apex domain a `Zone` (subdomain) hangs off.
//!
//! **Deliberately reverses `docs/06-onboarding-rules-lists.md` §1's original decision** ("Không
//! tạo entity `Domain` cha chứa nhiều `Zone` con" — keep `Zone.hostname` flat, group by apex only
//! at render time, add wildcard hostnames instead). The project owner chose a real parent entity
//! instead, for domain-ownership verification to happen once per apex domain rather than once per
//! subdomain. Wildcard hostname support is explicitly out of scope for this change — it stays the
//! doc's other, un-taken proposal.
//!
//! Ownership verification lives here (was on `Zone` before this change): `verificationToken`/
//! `verificationMethod`/`verificationStatus` prove control over the apex domain once, and every
//! `Zone` under it inherits that. `Zone.verificationStatus` still exists as a technical mirror
//! field, kept in sync by `routes::verify_domain_dns`'s cascade — see `zone_entity.rs`'s own doc
//! comment for why that mirror avoids needing a workflow guard to read a related entity's field
//! (`PolicyCondition` has no cross-entity attribute lookup, the same reason `Zone.hasConfig`
//! exists as its own technical field instead of a live count).
//!
//! DNS *routing* (is the hostname's DNS actually pointed at the edge yet) is a separate concern
//! that stays per-`Zone` — see `routes::verify_dns`'s own doc comment — since different subdomains
//! under the same apex can have different CNAME targets.

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

fn enum_field(name: &str, label: &str, values: &[&str], required: bool) -> EntityField {
    EntityField {
        enum_values: Some(values.iter().map(|v| v.to_string()).collect()),
        ..field(name, label, FieldKind::Enum, required, true)
    }
}

pub fn domain_entity() -> EntityDefinition {
    EntityDefinition {
        name: "waf.domains".to_string(),
        label: "Domain".to_string(),
        table_name: metap_reconciler::qualified_table_name_in("waf.domains", "waf"),
        fields: vec![
            EntityField {
                unique: Some(true),
                searchable: Some(true),
                ..field("apexDomain", "Apex Domain", FieldKind::String, true, true)
            },
            // Auto-generated server-side (`routes::zone_domain_guard`) when a zone's hostname
            // resolves to a new apex domain — never entered by hand.
            field(
                "verificationToken",
                "Verification Token",
                FieldKind::String,
                true,
                false,
            ),
            enum_field(
                "verificationMethod",
                "Verification Method",
                &["dnsTxt", "httpFile"],
                true,
            ),
            enum_field(
                "verificationStatus",
                "Verification Status",
                &["unverified", "verified"],
                true,
            ),
        ],
        list_views: vec![EntityListView {
            name: "default".to_string(),
            label: "Default".to_string(),
            fields: vec!["apexDomain".to_string(), "verificationStatus".to_string()],
            filters: vec!["apexDomain".to_string(), "verificationStatus".to_string()],
            required_fields: vec![],
            default_sort: None,
            max_limit: 100,
        }],
        workflow: None,
        unique_constraints: vec![],
        audit: Some(EntityAuditConfig { enabled: true }),
    }
}

submit_entity!(domain_entity);
