//! Entity definitions for `data-plane` — see `../../docs/02-domain-model.md` (business-level
//! spec) and `../../docs/05-metap-technical-mapping.md` (the concrete mapping these files
//! implement) for why each entity/field/workflow is shaped this way.

pub mod alert_notification_entity;
pub mod alert_policy_entity;
pub mod ddos_policy_entity;
pub mod firewall_rule_entity;
pub mod incident_entity;
pub mod scan_finding_entity;
pub mod scan_job_entity;
pub mod security_event_entity;
pub mod zone_entity;
