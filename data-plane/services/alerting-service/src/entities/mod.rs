//! Entity definitions owned by `alerting-service` — see `../../../../docs/02-domain-model.md`
//! (business-level spec) and `../../../../docs/05-metap-technical-mapping.md` (the concrete
//! mapping these files implement) for why each entity/field/workflow is shaped this way.

pub mod alert_notification_entity;
pub mod alert_policy_entity;
pub mod incident_entity;
pub mod security_event_entity;
