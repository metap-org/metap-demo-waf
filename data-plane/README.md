# data-plane

Portal nghiệp vụ cho sản phẩm **WAAP** (Web Application & API Protection, kiểu Cloudflare) —
nơi khách hàng cấu hình chính sách bảo vệ (DDoS L7, WAF, vulnerability scan), xem attack
analytics, quản lý incident. Đây là **data-plane** của `metap-demo-waf` — nơi giữ dữ liệu/config
nguồn (source of truth), xây nhanh trên nền `metap` (metadata-driven CRUD/workflow/permission).

Không thuộc phạm vi thư mục này: **`../control-plane`** (lấy config từ đây, tính toán, đẩy
xuống edge) và **`../edge-plane`** (thực thi mitigation thật tại biên) — xem
`docs/04-architecture-boundary.md` cho luồng dữ liệu đầy đủ giữa 3 phần.

## Cấu trúc: 3 service, không còn 1 binary (2026-09-01)

`data-plane/` là **Cargo workspace**, không phải 1 package — tách theo pillar, mỗi pillar 1
binary/deploy cycle riêng, cùng trỏ **1 database chung theo tenant** (`Router::pool_for`, không
tách theo data layer — xem lý do kỹ thuật ở doc-comment mỗi service's `main.rs`):

| Service | Sở hữu | Port (HTTP / gRPC opt-in) |
|---|---|---|
| [`services/zones-service`](services/zones-service/README.md) | `waf.zones`, `waf.ddos_policies`, `waf.firewall_rules` | 3000 / 3001 |
| [`services/scanning-service`](services/scanning-service/README.md) | `waf.scan_jobs`, `waf.scan_findings` | 3010 / 3011 |
| [`services/alerting-service`](services/alerting-service/README.md) | `waf.security_events`, `waf.incidents`, `waf.alert_policies`, `waf.alert_notifications` | 3020 / 3021 |

`zoneId` ở `scanning-service`/`alerting-service` là **`String` thuần, không phải `Reference`**
(cùng workaround `waf.security_events.triggeredById` đã dùng cho case polymorphic) — đăng ký
`zone_entity()` ở 2 service đó chỉ để pass `validate_references()` sẽ đồng thời lộ CRUD
`waf.zones` qua route generic `/api/:entity*` của chính chúng, phá nguyên tắc "1 service sở hữu
Zone". Chi tiết đầy đủ ở từng service's `main.rs` doc comment.

**[GraphQL gateway](graphql-gateway/README.md)** (cho FE gộp query cross-service) — đã dựng, chạy
thật, tái dùng nguyên trạng `metap/crates/graphql-gateway` (không code mới, chỉ cấu hình ở
`graphql-gateway/.env`) — 1 instance riêng cho WAF (không gộp chung upstream với
`metap-demo-crm`/`metap-demo-jira`), 3 `UPSTREAM_<N>_*` trỏ vào 3 service trên, port 4000. **Dùng
được cho cả mutation** (2026-09-02, đã verify sống: mutation qua `/graphql` phản ánh đúng
`updated_by` của user thật gọi, và bị 403 đúng khi user không có quyền) — token của caller được
forward nguyên vẹn xuống upstream khi hợp lệ (`RequestContext::forwarded_bearer_token`), enforce
đúng permission/audit trail của người gọi thật, không phải danh nghĩa service account; điều này
chạy được vì gateway verify qua cùng 1 trust root với 3 service (2026-09-04: chuyển từ share 1
file RSA private key sang `metap-jwks` — Ed25519, `zones-service` publish `/.well-known/
jwks.json`, mọi nơi (kể cả `zones-service` tự nó) verify qua `JWKS_URL`; gateway giờ chỉ cần
`JWKS_URL`, không cần giữ bản PEM riêng nữa. Cả 3 service vẫn cùng giữ 1 private key để mint —
chưa chuyển sang mô hình 1-issuer-duy-nhất, đó là bước tiếp theo nếu cần giảm blast radius thật
sự; xem `../CLAUDE.md`'s mục JWKS/rotation cho chi tiết. RSA vẫn còn làm fallback nếu
`JWKS_PRIVATE_KEY_PATH`/`JWKS_KID_PATH` chưa được set). Khi không có token forward (vd
lúc gateway tự fetch schema lúc boot), gateway tự login bằng service-account riêng
(`UPSTREAM_<N>_SERVICE_EMAIL`/`SERVICE_PASSWORD`, xem `graphql-gateway/README.md`'s Auth section) —
không còn JWT tĩnh mint tay phải refresh thủ công nữa.

**Ngoài phạm vi 3 service này** (Customer Portal backend) — Admin Portal backend
(`Plan`/`Subscription`/billing, `docs/12-billing-plans.md`, chưa xây) là 1 trục tách khác hẳn
(platform-level, xuyên tenant, không phải per-tenant DB), đi theo pattern
`metap-control`/`metap-lowcode`'s platform-level service khi build, không phải pillar service
nào ở trên — xem `docs/09-access-control.md`'s mô hình 2-portal.

**`web/`'s dev server route đúng cả 3 service** (`vite.config.ts`, verify qua `curl` thật) —
middleware tự forward theo tiền tố tên entity (không dùng `server.proxy`'s `router` option của
Vite — tìm ra Vite 8 không tôn trọng option đó, xem `docs/61-*` cho chi tiết), cộng 1 middleware
gộp `GET /metadata/entities` từ cả 3 service (không có entity trong path — không thuộc về đúng
1 service nào). Production thật (không phải Vite dev server) sẽ cần Traefik hoặc reverse-proxy
tương đương làm việc này, chưa dựng.

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

Backend (Rust, scaffold từ `metap`'s `templates/metap-app`) đã chạy được — **cả 9 entity của
pillar 1-4 đã đăng ký, build sạch, test qua API thật** (2026-08-30):
- `waf.zones` — workflow `pending→active→paused→suspended`, guard `activate` chặn zone chưa đủ
  cả `hasConfig` **và** `verificationStatus = verified`.
- `waf.ddos_policies` — `zoneId` unique (1 policy/zone, enforce thật).
- `waf.firewall_rules` — `matchCondition` lưu JSON tự do.
- `waf.scan_jobs` — workflow lặp `idle↔queued↔running↔completed/failed` (test cả vòng lặp lại).
- `waf.scan_findings` — workflow remediation `open→confirmed→fixed`/`falsePositive`/`accepted`.
- `waf.security_events` — log, không workflow, `triggeredByName` denormalize.
- `waf.incidents` — workflow `open→acknowledged→mitigating→resolved` (test hết chuỗi).
- `waf.alert_policies` / `waf.alert_notifications` — CRUD.

**Tách thành 3 service** (2026-09-01, xem bảng ở trên) — `cargo build/test --workspace` sạch cho
cả 3, mỗi service tự verify đúng entity mình sở hữu (test `owns_exactly_its_own_N_entities`).
**Verify sống qua HTTP thật sau khi tách** (cùng ngày, cả 3 service chạy thật + tenant/token
thật): tạo `ScanJob` ở `scanning-service` với `zoneId` (String) trỏ 1 Zone có sẵn ở
`zones-service` — thành công, không lỗi ràng buộc; `GET`/`POST /api/waf.zones` ở
`scanning-service` **và** `alerting-service` đều `404 entity_not_found` — xác nhận route CRUD
`waf.zones` không tồn tại ở 2 service không sở hữu nó; `zones-service`'s `relatedDisplay` cho
`DdosPolicy.zoneId` (Reference nội bộ) vẫn tự resolve đúng, không bị ảnh hưởng.

**Frontend** (`web/`, 2026-08-30): dựng từ `@metap/platform-ui` (repo `../../platform-ui`, kế
thừa `packages/platform-react` cũ của `metap`) + `@metap/ui` (repo `../../design-system`) —
generic list/form/detail/workflow UI tự sinh từ metadata, y hệt cách `metap`'s `apps/crm-fe`
dùng. Đã test thật qua browser (Playwright) **trước khi tách**: login (local JWT), nav 9 entity,
list Zone, tạo/xem/sửa record, workflow diagram + nút transition tự sinh. **Routing qua 3
service đã verify sống (2026-09-01) bằng `curl` qua dev server thật** — xem ghi chú "Cấu trúc" ở
trên; chưa re-test qua browser thật sau khi tách (chỉ verify tầng HTTP, chưa click qua UI).

**Zone overview** (`src/demo/ZoneOverviewPage.tsx`, `/zones/:zoneId/overview`, link từ trang chủ
"/") — màn đầu tiên gọi [GraphQL gateway](graphql-gateway/README.md) thật từ FE thay vì REST, gộp
DDoS policy + firewall rules + scan gần nhất + incident gần nhất của 1 zone trong 1 query. Dùng
`useGraphQLQuery` mới thêm vào `@metap/platform-ui` (cùng vai trò `useApiQuery` bên REST). Verify
sống bằng đúng token đăng nhập thật qua `curl` tới `/graphql` của dev server — phát hiện + sửa 1
bug thật lúc verify (gateway ban đầu dùng keypair riêng, không decode được token đăng nhập thật —
xem `graphql-gateway/README.md`'s mục Auth).

Còn thiếu — toàn bộ phần **Custom** (không phải CRUD generic) đã liệt kê ở
`docs/13-screen-api-map.md`: portal IA riêng theo `docs/07-portal-features.md` (hiện chỉ là danh
sách entity phẳng + Zone overview, chưa có tab Zone-centric đầy đủ/whitelist-blacklist view/
dashboard), DNS/verify domain thật, dashboard aggregate, job correlation
`SecurityEvent→Incident`, job gửi alert thật, `metap-cron` wiring cho `ScanJob.schedule`, access
control theo (user, domain) (`docs/09-access-control.md`), permission policy cho role khác
admin, `Plan`/`Subscription`
(`docs/12-billing-plans.md`, chưa build) — cùng Traefik/reverse-proxy thật cho production
(`web/`'s dev-server routing chỉ dùng được lúc `pnpm dev`), Admin Portal backend riêng.

## Development

```bash
# 1 lần cho cả 3 service — Postgres/RabbitMQ dùng chung với ../metap (docker compose up -d
# postgres rabbitmq ở đó nếu chưa chạy). Key JWT dùng chung, đặt ở data-plane/keys/.
cargo run --manifest-path ../../metap/crates/dev-tools/Cargo.toml -- gen-keys keys           # RSA fallback
cargo run --manifest-path ../../metap/crates/dev-tools/Cargo.toml -- gen-jwks-key keys       # EdDSA trust root (mặc định dùng cái này, xem README's auth section)
cargo run --manifest-path ../../metap/crates/dev-tools/Cargo.toml -- provision-tenant <tenantId> schema <email> <password>
cargo run --manifest-path ../../metap/crates/dev-tools/Cargo.toml -- mint-token <tenantId> <userId>

# Mỗi service: copy .env.example riêng rồi chạy (từ data-plane/, workspace root)
cp services/zones-service/.env.example services/zones-service/.env
cp services/scanning-service/.env.example services/scanning-service/.env
cp services/alerting-service/.env.example services/alerting-service/.env

cargo run -p zones-service      # http://localhost:3000
cargo run -p scanning-service   # http://localhost:3010
cargo run -p alerting-service   # http://localhost:3020
```

### Dev nhanh — tự build lại khi sửa code (`cargo-watch`)

Tương tự `../../metap-demo-crm/README.md` — mỗi service chạy `cargo watch` riêng (workspace này
có 3 package), theo dõi thêm `../../metap/crates` vì đó là path dependency:

```bash
cargo install cargo-watch   # 1 lần, nếu chưa có
cargo watch --watch services/zones-service/src --watch ../../metap/crates -x 'run -p zones-service'
cargo watch --watch services/scanning-service/src --watch ../../metap/crates -x 'run -p scanning-service'
cargo watch --watch services/alerting-service/src --watch ../../metap/crates -x 'run -p alerting-service'
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

## Docker (2026-09-01)

`docker-compose.yml` (thư mục này) orchestrate 5 container: `zones-service`, `scanning-service`,
`alerting-service`, `graphql-gateway`, `web` — build từ 5 Dockerfile (3 service's
`services/*/Dockerfile`, `../../metap/crates/graphql-gateway/Dockerfile`, `web/Dockerfile`).
**Không tự chạy Postgres/RabbitMQ riêng** — 3 service backend trỏ vào cùng instance dev dùng
chung với `../../metap` (`docker compose up -d postgres rabbitmq` từ đó, cổng host 5433/5672)
qua `host.docker.internal` (đúng pattern `../../metap/docker-compose.yml`'s `prometheus`/`k6` đã
dùng để với tới `crm-server` chạy trên host — xem comment đầu file `docker-compose.yml` này).

**Mọi build đều dùng `context: ../../` (root `metap-org`), không phải `.`/`data-plane/`** — kể cả
3 service backend, vì `metap = { path = "../../../../metap/crates/metap" }` trong mỗi
`services/*/Cargo.toml` trỏ ra ngoài `data-plane/`, cần context rộng hơn mới thấy được (xem
comment đầu mỗi `services/*/Dockerfile` cho chi tiết cách COPY giữ nguyên cấu trúc lồng nhau giữa
`metap/` và `metap-demo-waf/data-plane/` để path dependency đó resolve đúng trong image — **vẫn
giữ path dependency, không đổi sang `git`**, để build local vẫn lấy code chưa commit ở
`../../metap` thay vì 1 ref đã push). `graphql-gateway` build trong workspace `../../metap` của
chính nó, không cross-repo. `../../.dockerignore` (root `metap-org`, lần đầu tiên có file này)
giữ context rộng đó không kéo theo `target/`/`node_modules/`/`.git` của các repo khác (tổng
`target/` 4 repo Rust ~49GB lúc viết file này).

**`web`** — build production (nginx + static assets), khác hẳn `pnpm dev` ở mục Frontend phía
trên (`pnpm dev` vẫn chạy trên host, không đổi gì). Cùng lý do cần context root: `web/
package.json`'s `link:../../../platform-ui`/`link:../../../design-system` cần thấy 2 sibling repo
đó. `web/nginx.conf` viết lại đúng logic route-theo-entity của `vite.config.ts`'s middleware
dev-only — có 1 gap chưa giải được, ghi rõ trong comment đầu `nginx.conf`: `/metadata/entities`
(không kèm tên entity) chỉ proxy về `zones-service`, không gộp được cả 3 service như middleware
dev (nginx thuần không tự gộp JSON từ 3 upstream).

```bash
cp .env.example .env
# tạo 1 service-account user thật (xem .env.example's comment cho lệnh create-user/seed-admin đầy
# đủ), điền email/password vào WAF_SERVICE_EMAIL/WAF_SERVICE_PASSWORD
docker compose up -d --build
```

Lần build đầu sẽ lâu (`zones-service`/`scanning-service`/`alerting-service` mỗi cái tự build lại
toàn bộ `metap/crates` do context/COPY layer đổi cùng đợt sửa context — không cache được từ lần
build cũ khi context còn hẹp) — lần build sau nhanh hơn nhờ Docker cache mỗi `metap/`/
`metap-demo-waf` COPY layer riêng.

Mỗi service's `Cargo.toml`'s `metap` dependency là **path dependency** vào
`../../../../metap/crates/metap` (local dev, dùng code hiện có trên máy thay vì phụ thuộc branch
đã push) — đổi sang `git` dependency khi cần build ở CI/deploy, xem comment trong từng
`Cargo.toml`.
