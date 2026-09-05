# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

`metap-demo-waf` is a demo **WAAP** (Web Application & API Protection, Cloudflare-style) product.
**All three planes have code now** (2026-09-04) — `data-plane/` (3 pillar services + GraphQL
gateway config + Customer Portal frontend), `control-plane/` (`waf-config-distributor`), and
`edge-plane/` (`waf-edge`, the mitigation engine). See the plane table below. **All 3 Rust
workspaces build/clippy/test clean and `data-plane/web` passes `tsc`/`oxlint`/`prettier`/`vite
build`, and `data-plane`'s own 3 e2e tests now pass against a real Postgres too** (verified
2026-09-04, same day, two separate passes — see Working conventions for exactly what each covered
and what still hasn't run: `control-plane`/`edge-plane` still have no live-infra e2e proof, and
nothing has yet shown a portal rule change actually reaching the edge). The product/architecture
spec lives in `data-plane/docs/` and remains the source of truth
for anything not yet built. Read
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
| `data-plane/` | Business portal (source of truth): Zone, DDoS policy, firewall rule, vulnerability scan, incident, alert — built on `metap`, full CRUD/workflow/permission UI | 3 services (`zones`/`scanning`/`alerting`) + `waf-graphql-gateway` (own binary, `data-plane/graphql-gateway/`, not just config — 7 custom mutations, wraps `metap`'s generic gateway library via `build_with_extensions`) + Customer Portal frontend (10-module IA, zone-centric). Custom non-CRUD endpoints live in each service's `src/routes.rs`. **Both compose files must build/run `waf-graphql-gateway`, not `metap/crates/metap-graphql-gateway`'s generic binary** — found live, 2026-09-04: they'd been pointed at the generic one, silently dropping every custom field (`docs/roadmap/76-waf-portal-live-bugfixes.md`) |
| `control-plane/` | Headless worker: pulls config changes from `data-plane` (RabbitMQ outbox subscribe), compiles them into an edge-ready rule-set, writes to Redis/DragonflyDB. No UI, not CRUD | `waf-config-distributor` — 3 jobs in one process: subscribe (fast path), periodic full resync (**the convergence guarantee**), telemetry ingest. The Redis contract is `waf-config-distributor/src/ruleset.rs` |
| `edge-plane/` | High-performance, low-latency mitigation engine: evaluates rules against real traffic, blocks/challenges/logs. Deliberately **not** built on `metap`/metadata-driven approach | `waf-edge` — hyper 1.x, **zero `metap` dependency anywhere in the tree**. `ArcSwap` rule snapshot (a request never touches Redis), DDoS budget → priority-ordered rules → block/challenge/log → proxy to origin |

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

Telemetry direction (`SecurityEvent`, edge → up) **was decided in Phase 72 (2026-09-04): option 2**
— the edge batches to `control-plane`, which writes into `data-plane` through the ordinary CRUD
route. That is the option `04-architecture-boundary.md` already leaned toward, and it keeps the
"edge never talks to `data-plane` directly" rule intact. It was decided in-session rather than by
the project owner, so it is flagged in `../metap-docs/docs/roadmap/72-control-edge-planes.md` and
in that PR, and is cheap to reverse (the edge knows exactly one ingest URL).

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

- Three independent Cargo workspaces, one per plane — never one workspace spanning them, since a
  shared workspace would mean a portal change rebuilds and redeploys the edge. `cargo
  build/test/clippy --workspace` from `data-plane/`, `control-plane/`, or `edge-plane/`
  separately; `pnpm dev`/`tsc -b`/`oxlint` from `data-plane/web`.
- `edge-plane/` must never gain a `metap` dependency. If something there needs a `metap`
  primitive, that is a signal the work belongs in `control-plane` instead.
- **Phases 70/71 (2026-09-03) and 72 (2026-09-04) landed unverified on purpose, then got a
  dedicated build/test/verify pass the same day (2026-09-04).** `cargo build`/`clippy --all-targets
  -- -D warnings`/`test --workspace` are clean across all 3 Rust workspaces (`data-plane`,
  `control-plane`, `edge-plane`); `data-plane/web` passes `tsc -b`/`oxlint`/`prettier --check`/a
  real `vite build`. That pass found and fixed real bugs — 48 TypeScript errors (every `toast()`
  call site used a shape the design system doesn't have), a clippy error, 6 dead-code warnings —
  and added 80 unit tests for the previously-untested pure logic (`aggregate` SQL planning,
  `compile.rs`'s zone/rule compilation, `evaluate.rs`'s mitigation decision, the rate limiter, the
  clearance cookie).
- **A second pass the same day (2026-09-04) ran the 3 `data-plane` services' own e2e tests against
  a real Postgres for the first time.** Docker Hub image pulls are blocked in this environment by
  an org network policy (403 at the proxy's CONNECT layer, confirmed via `/__agentproxy/status` —
  not retried/bypassed, per that proxy's own instructions), so a native (apt-installed, non-Docker)
  Postgres/RabbitMQ stood in for `docker compose up -d postgres rabbitmq`, matched to that file's
  own `metap`/`metap` credential convention. `zones-service`/`scanning-service`/`alerting-service`'s
  `http_server.rs` `#[ignore]` e2e test (`cargo test -p <service> --test http_server -- --ignored`)
  found and fixed a real bug in all 3: the test minted a JWT and called `POST /api/test.tasks`
  without ever seeding a `user_roles` row for that user, so `PermissionService::check_action`'s
  deny-by-default entity-level check (no matching policy → forbidden) correctly rejected it —
  `201` expected, `403` actual. `metap-http`'s own canonical `http_server.rs` (the template this
  was copied from) seeds `INSERT INTO user_roles (..., 'admin')` before minting its token; this
  repo's copy had dropped that line. Fixed identically in all 3 (seed + matching teardown); all 3
  now pass against live Postgres. `metap` core's own `cargo test --workspace -- --ignored` (run
  the same session, same native Postgres/RabbitMQ) is green across dozens of test files — strong
  evidence the platform primitives Phase 70-72 build on are sound beyond unit-test level. **Still
  not covered**: `control-plane`/`edge-plane` have no live-infra e2e tests of their own yet (their
  test suites are pure-logic unit tests, already green), and there is still no end-to-end proof
  that a rule change on the portal reaches the edge and actually blocks a request — see "Còn lại"
  in `../metap-docs/docs/roadmap/72-control-edge-planes.md` for exactly what that leaves open.
  Full detail on all 3 passes (what shipped unverified, what the first verify pass then found, and
  what this live-Postgres pass found) is in the "Xác minh" / "Đã verify" sections of
  `../metap-docs/docs/roadmap/70-aggregate-api.md`, `71-waf-admin-portal.md`, and
  `72-control-edge-planes.md`.
- Docs are the spec. If an implementation choice isn't settled in `data-plane/docs/`, don't invent
  one silently — it's likely one of the explicitly-flagged open questions above.
- **Auth moved from a shared static RSA keypair to `metap-jwks` (Ed25519, 2026-09-04).** The 3
  `data-plane` services + `graphql-gateway` used to share 1 `dev-jwt-private.pem` file (copied
  into every container) for both mint and verify. They now default to a `metap-jwks` trust root
  instead — `zones-service` publishes `/.well-known/jwks.json`, every service (including
  `zones-service` itself) verifies via `JWKS_URL`, and `graphql-gateway` verifies the same way
  (no private key file needed there at all now). This was additive/opt-in work in `metap` core
  (`metap-http::AppState.token_verifier`/`token_signer`, `metap-jwks::{TokenVerifier,
  TokenSigner}`, `dev-tools gen-jwks-key`) — `metap-demo-crm`/`metap-demo-jira`/`metap-lowcode`
  are unaffected, verified by building them unchanged. **Scope deliberately stopped short of a
  single-issuer topology**: all 3 services still hold the same private key locally (mint
  themselves), matching the old RSA topology's blast radius — only rotation got safer (`JwksKeyStore`'s
  3-step add/promote/retire), not the "1 process holds the only private key" property JWKS
  otherwise enables. See `data-plane/README.md`'s GraphQL gateway section for the auth-flow
  detail and the 3 services' own `.env.example` for the exact env vars
  (`JWKS_PRIVATE_KEY_PATH`/`JWKS_KID_PATH`/`JWKS_URL`). RSA (`AUTH_JWT_PUBLIC_KEY_PATH`/
  `AUTH_JWT_PRIVATE_KEY_PATH`, `dev-tools gen-keys`) still works as an explicit fallback if the
  JWKS env vars are unset.
