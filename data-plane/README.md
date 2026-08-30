# data-plane

Portal nghiệp vụ cho sản phẩm **WAAP** (Web Application & API Protection, kiểu Cloudflare) —
nơi khách hàng cấu hình chính sách bảo vệ (DDoS L7, WAF, vulnerability scan), xem attack
analytics, quản lý incident. Đây là **data-plane** của `metap-demo-waf` — nơi giữ dữ liệu/config
nguồn (source of truth), xây nhanh trên nền `metap` (metadata-driven CRUD/workflow/permission).

Không thuộc phạm vi thư mục này: **`../control-plane`** (lấy config từ đây, tính toán, đẩy
xuống edge) và **`../edge-plane`** (thực thi mitigation thật tại biên) — xem
`docs/04-architecture-boundary.md` cho luồng dữ liệu đầy đủ giữa 3 phần.

Bắt đầu đọc nghiệp vụ từ `docs/01-product-vision.md`; `docs/05-metap-technical-mapping.md` là
bản ánh xạ kỹ thuật (entity/workflow/permission cụ thể trên `metap`) mà code dưới đây bám theo.

### Mục lục `docs/`

| Doc | Nội dung |
|---|---|
| `01-product-vision.md` | Scope v1 (4 trụ cột) + 3 mảng hạ tầng phát sinh (onboarding/access-control/billing) |
| `02-domain-model.md` | Domain model đầy đủ (business-level) — mọi entity, kể cả field/entity phát sinh từ `06-12` |
| `03-personas-workflows.md` | Persona, RBAC gợi ý, core workflow |
| `04-architecture-boundary.md` | Ranh giới 3 plane, luồng config/telemetry, SLA đồng bộ edge (10-30s) |
| `05-metap-technical-mapping.md` | Ánh xạ `EntityDefinition`/workflow/permission cụ thể — code bám theo đây |
| `06-onboarding-rules-lists.md` | Domain verification, rule config, whitelist/blacklist, domain/subdomain |
| `07-portal-features.md` | Sitemap + feature list toàn portal, ưu tiên P0/P1/P2 |
| `08-module-detail-specs.md` | Từng module xuống mức màn hình/field — 6 quyết định nghiệp vụ đã chốt |
| `09-access-control.md` | Mô hình 2-portal (Admin/Customer), gán quyền theo (user, domain) |
| `10-attack-visibility.md` | Dashboard WAF/DDoS, lịch sử/chi tiết tấn công, access log (quyết định scope) |
| `11-onboarding-dns-resolution.md` | Tra DNS/IP lúc onboard, phân biệt ownership verification vs DNS routing status |
| `12-billing-plans.md` | `Plan`/`Subscription`, quota theo gói, câu hỏi mở về thanh toán |
| `13-screen-api-map.md` | Từng màn hình ↔ API cụ thể — cái nào generic (metap tự sinh) vs cần code thêm |
| `14-cloudflare-gap-analysis.md` | So sánh feature với Cloudflare thật — cái nào cố tình loại, cái nào là gap thật |

## Trạng thái

Backend skeleton (Rust, scaffold từ `metap`'s `templates/metap-app`) đã chạy được — **cả 9 entity
của pillar 1-4 đã đăng ký, build sạch, test qua API thật** (2026-08-30):
- `waf.zones` — workflow `pending→active→paused→suspended`, guard `activate` chặn zone chưa đủ
  cả `hasConfig` **và** `verificationStatus = verified`.
- `waf.ddos_policies` — `zoneId` unique (1 policy/zone, enforce thật).
- `waf.firewall_rules` — `matchCondition` lưu JSON tự do.
- `waf.scan_jobs` — workflow lặp `idle↔queued↔running↔completed/failed` (test cả vòng lặp lại).
- `waf.scan_findings` — workflow remediation `open→confirmed→fixed`/`falsePositive`/`accepted`.
- `waf.security_events` — log, không workflow, `triggeredByName` denormalize.
- `waf.incidents` — workflow `open→acknowledged→mitigating→resolved` (test hết chuỗi).
- `waf.alert_policies` / `waf.alert_notifications` — CRUD.

**Frontend** (`web/`, 2026-08-30): dựng từ `@metap/platform-ui` (repo `../../platform-ui`, kế
thừa `packages/platform-react` cũ của `metap`) + `@metap/ui` (repo `../../design-system`) — generic
list/form/detail/workflow UI tự sinh từ metadata, y hệt cách `metap`'s `apps/crm-fe` dùng. Đã test
thật qua browser (Playwright): login (local JWT), nav 9 entity, list Zone (đúng cột đã khai ở
`list_views`), tạo/xem/sửa record, **workflow diagram + nút transition (Pause/Suspend...) tự sinh
từ `EntityWorkflow`** — không viết tay 1 dòng UI nào cho nghiệp vụ. 0 console error.

Còn thiếu — toàn bộ phần **Custom** (không phải CRUD generic) đã liệt kê ở
`docs/13-screen-api-map.md`: portal IA riêng theo `docs/07-portal-features.md` (hiện chỉ là danh
sách entity phẳng, chưa có tab Zone-centric/whitelist-blacklist view/dashboard), DNS/verify domain
thật, dashboard aggregate, job correlation `SecurityEvent→Incident`, job gửi alert thật,
`metap-cron` wiring cho `ScanJob.schedule`, access control theo (user, domain)
(`docs/09-access-control.md`), permission policy cho role khác admin, `Plan`/`Subscription`
(`docs/12-billing-plans.md`, chưa build).

## Development

```bash
cp .env.example .env   # đã trỏ sẵn về Postgres/RabbitMQ dev của ../metap (localhost:5433/5672)

# Postgres/RabbitMQ dùng chung với ../metap (docker compose up -d postgres rabbitmq ở đó nếu
# chưa chạy) — DB đã có sẵn schema `control`/`entities` từ `metap`'s db-migrate.

cargo run --manifest-path ../../metap/crates/dev-tools/Cargo.toml -- gen-keys keys
cargo run --manifest-path ../../metap/crates/dev-tools/Cargo.toml -- provision-tenant <tenantId> schema <email> <password>
cargo run --manifest-path ../../metap/crates/dev-tools/Cargo.toml -- mint-token <tenantId> <userId>

cargo run   # http://localhost:3000, GET /health không cần token
```

### Frontend (`web/`)

```bash
cd web
pnpm install   # link:../../../platform-ui + link:../../../design-system — 2 repo sibling này
               # phải tồn tại ở đúng vị trí, xem package.json
pnpm dev       # http://localhost:5173, proxy /api /metadata /auth /admin /health sang :3000
```

Login bằng user local đã tạo lúc `provision-tenant` ở trên (email/password đã đặt lúc đó) — không
cần bước riêng.

`Cargo.toml`'s `metap` dependency là **path dependency** vào `../../metap/crates/metap`
(local dev, dùng code hiện có trên máy thay vì phụ thuộc branch đã push) — đổi sang `git`
dependency khi cần build ở CI/deploy, xem comment trong `Cargo.toml`.
