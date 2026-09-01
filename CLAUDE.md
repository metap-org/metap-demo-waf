# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

`metap-demo-waf` is a demo **WAAP** (Web Application & API Protection, Cloudflare-style) product.
**As of now the repo contains no code — only architecture/product docs.** Every directory has a
placeholder `README.md`; the real content lives in `data-plane/docs/`. Read
`data-plane/docs/01-product-vision.md` through `04-architecture-boundary.md` in order before
proposing or writing any code — they contain the settled decisions (scope, domain model, personas,
plane boundaries) that later work must stay consistent with. Docs are written in Vietnamese.

This repo is built on top of a sibling repo, `../metap` (a metadata-driven platform core: Rust/axum/
sqlx/PostgreSQL/RabbitMQ, outbox pattern, generic CRUD/workflow/permission engine). `metap-demo-waf`
is one product built on that platform, not a fork of it — `metap`'s own *code/build* conventions
(its Cargo workspace commands, crate layering, `CLAUDE.md`) live in that separate sibling repo and
don't apply here directly. That's different from the *operational* conventions in
`../CLAUDE.md` (root of `metap-org`) — respond in Vietnamese, never commit without being asked,
check `target/` size before a build session — which do apply here, same as every repo in this
directory, since this file doesn't override any of them. `metap`'s primitives
(`metap-metadata`, `metap-workflow`, `metap-cron`, `metap-permission`, `metap-cache`, `metap-grpc`,
`metap-reconciler`, `metap-storage`) are the ones `data-plane/` is meant to reuse rather than
reinvent.

## Repo structure: 3 planes, 3 deploy cycles

The repo is deliberately split into three top-level directories, each a separate codebase/deploy
cycle with a hard boundary — do not blur these when implementing:

| Directory | Role | Status |
|---|---|---|
| `data-plane/` | Business portal (source of truth): Zone, DDoS policy, firewall rule, vulnerability scan, incident, alert — built on `metap`, full CRUD/workflow/permission UI | Docs done, no code yet |
| `control-plane/` | Headless worker(s): pulls config changes from `data-plane` (RabbitMQ outbox subscribe), compiles them into an edge-ready rule-set, writes to Redis/DragonflyDB. No UI, not CRUD. Suggested worker name: `waf-config-distributor` | Not started |
| `edge-plane/` | High-performance, low-latency mitigation engine: evaluates rules against real traffic, blocks/challenges/logs. Deliberately **not** built on `metap`/metadata-driven approach | Not started |

Key rule, stated repeatedly in the docs: **`edge-plane` never talks to `data-plane` directly.** It
only reads config that `control-plane` has already computed into Redis/DragonflyDB.

Data flow:
```
data-plane (Zone/DdosPolicy/FirewallRule change via portal)
  → metap outbox (same transaction as the DB write)
  → outbox-publisher → RabbitMQ
  → control-plane worker subscribes, compiles a per-Zone rule-set, writes to Redis/DragonflyDB
  → edge-plane reads Redis directly (low latency, many edge nodes share one key)
```
`Zone.configVersion` increments on every change to a zone's policies/rules; `control-plane` and
`edge-plane` both compare it to detect stale cache instead of guessing from timestamps.

Telemetry direction (`SecurityEvent`, edge → up) is **explicitly unresolved** — see the "Chưa chốt"
section of `04-architecture-boundary.md` for the two candidate approaches (edge calls
`metap-grpc`'s generic `RecordService.Create` directly, vs. edge batches through `control-plane`
first). Don't silently pick one when implementing telemetry ingestion; flag it.

## Domain model (business-level, not yet `EntityDefinition` code)

`data-plane/docs/02-domain-model.md` has full field-level detail. Summary of the entity graph:

```
Tenant (reused from metap control.tenants)
  └─ Zone (protected site/domain; status: pending→active→paused→(active); terminal: suspended)
       ├─ DdosPolicy (0..1 active at a time)
       ├─ FirewallRule (0..N — shared rule engine for WAF custom rules/rate-limit/IP-geo firewall;
       │    ordered by priority, first match wins)
       ├─ ScanJob (0..N, schedule via metap-cron cron expression)
       │    └─ ScanFinding (0..N; remediationStatus workflow: open→confirmed→fixed / falsePositive / accepted)
       ├─ SecurityEvent (0..N — high volume, written by edge-plane, candidate for
       │    metap-reconciler table-per-entity instead of the generic `records` table)
       └─ Incident (0..N — correlates SecurityEvents; status: open→acknowledged→mitigating→resolved,
            a metap-workflow EntityWorkflow)
AlertPolicy (Tenant-scoped, watches N zones)
  └─ AlertNotification (delivery log, sent/failed)
```

Notable open questions flagged in the docs (don't resolve unilaterally — surface them):
- Whether `FirewallRule.matchCondition` reuses `metap-permission`'s `PolicyCondition` grammar or
  needs its own (request fields like `uri.path`/`header.x`/`body.y` vs. entity fields).
- Whether `Incident` correlation is a static rule or per-tenant configurable threshold.
- `SecurityEvent` retention/archival policy (cold storage via `metap-storage`?).

## v1 scope

Four pillars only — treat anything else as out of scope unless the user says otherwise:
1. **DDoS L7 Protection** (policy per Zone: threshold/sensitivity/action)
2. **WAF / Firewall Rules** (one shared match-condition→action engine for WAF custom rules, rate
   limiting, and IP/geo firewall — deliberately not three separate features)
3. **Vulnerability Scanning** (scheduled/manual scan jobs → findings → remediation tracking)
4. **Analytics + Alerting + Incident management** (required to make the other three pillars provable)

Explicitly out of v1 (documented so it isn't forgotten, not "never"): Bot management, API schema
validation/discovery, managed WAF ruleset (OWASP CRS-style), TLS/certificate management, Page
Shield, Attack Surface Management, L3/L4 DDoS.

## Personas / RBAC

Reuses `metap`'s existing RBAC matrix + ABAC condition builder — no new permission engine, just new
action sets for these entities. Roles: Platform Admin (all tenants), Tenant Admin (full control
within tenant), Security Analyst/SOC (handles Incidents/SecurityEvents, edits rules, can't delete
Zone), Developer (owns ScanFinding remediation only, no DdosPolicy/FirewallRule access), Viewer
(read-only Analytics/Incident).

## Working conventions

- There is no build/lint/test tooling yet in this repo — nothing to run. When code is added to a
  plane, check that plane's own README/CLAUDE.md (once it exists) rather than assuming `metap`'s
  commands apply.
- Docs are the spec. If an implementation choice isn't settled in `data-plane/docs/`, don't invent
  one silently — it's likely one of the explicitly-flagged open questions above.
