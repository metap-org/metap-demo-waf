# `waf-graphql-gateway`

**Dùng để làm gì**: 1 endpoint GraphQL duy nhất cho toàn bộ Customer Portal — gộp CRUD/list của cả
9 entity từ 3 service backend (`zones-service`/`scanning-service`/`alerting-service`) **cộng thêm
8 field custom** (verify DNS, test origin, sync config state, run scan job, test alert policy,
correlate incidents, evaluate alerts, aggregate) cho những action không phải CRUD mà
`metap-graphql` không tự sinh được từ metadata. Đây là giao thức chuẩn FE→BE của portal
(2026-09-04, `../metap-docs/docs/frontend-checklist.md` — chuyển hẳn từ REST sang GraphQL, trừ
`/auth/*`/`/preferences/*` — xem `src/main.rs`'s doc comment cho lý do các field custom phải nằm
ở binary này chứ không phải ở `metap`).

**Có code riêng ở đây** (khác bản cũ chỉ có `.env` + chạy thẳng binary generic của `metap`) —
`src/main.rs` là 1 binary mỏng dựng trên thư viện `metap-graphql-gateway`
(`../../../metap/crates/graphql-gateway`), gọi `schema_builder::build_with_extensions` để merge
8 field custom vào schema generic trước khi finish. Chạy:

```bash
cp .env.example .env   # điền UPSTREAM_<N>_SERVICE_EMAIL/SERVICE_PASSWORD — 1 user thật (tạo qua
                        # dev-tools create-user + seed-admin), gateway tự login qua /auth/login
cargo run -p waf-graphql-gateway
```

Mặc định `PORT=4000`. `GET /graphql/playground` (GraphiQL, non-prod) để tự gõ query thử.

## 8 field custom

Mỗi field là 1 proxy mỏng — parse arg, forward bearer token của caller, gọi thẳng REST endpoint
đã có sẵn (đã test), trả JSON verbatim làm scalar `Json`. Không có business logic nào viết lại ở
GraphQL layer.

| Field | Kiểu | REST endpoint gốc |
|---|---|---|
| `verifyZoneDns(zoneId: ID!)` | Mutation | `POST /api/waf.zones/{id}/verify-dns` (zones) |
| `testZoneOrigin(zoneId: ID!)` | Mutation | `POST /api/waf.zones/{id}/test-origin` (zones) |
| `syncZoneConfigState(zoneId: ID!)` | Mutation | `POST /api/waf.zones/{id}/sync-config-state` (zones) |
| `runScanJob(jobId: ID!)` | Mutation | `POST /api/waf.scan_jobs/{id}/run` (scanning) |
| `testAlertPolicy(policyId: ID!)` | Mutation | `POST /api/waf.alert_policies/{id}/test` (alerting) |
| `correlateIncidents(zoneId: String)` | Mutation | `POST /internal/incidents/correlate` (alerting) |
| `evaluateAlerts` | Mutation | `POST /internal/alerts/evaluate` (alerting) |
| `aggregate(entity: String!, spec: Json!)` | Query | `POST /api/{entity}/aggregate` — routed tới đúng service theo tên entity |

`aggregate` là Query dù REST là `POST` — về ngữ nghĩa vẫn là read (đúng cách `metap-http` gác nó
bằng `AuthContext`, cùng cổng với `list`), và nó không đi qua `RecordBackend`/`CompositeBackend`
như CRUD/list generic — Phase 70 (`../../../metap-docs/docs/roadmap/70-aggregate-api.md`) chưa
từng thêm `aggregate` vào trait `RecordBackend`/gRPC proto, nên phải proxy REST trực tiếp như 7
field kia, không đi qua đường gRPC generic.

REST base URL của mỗi upstream được suy ra từ `UPSTREAM_<N>_METADATA_URL` (bỏ hậu tố
`/metadata/entities`) — không cần thêm biến env riêng, vì thông tin đã có sẵn trong config generic.

## Auth — dùng chung key với 3 service, không phải key riêng

`AUTH_JWT_PUBLIC_KEY_PATH` trỏ vào **`../keys/dev-jwt-public.pem`** — đúng key 3 service WAF
(`zones-service`/`scanning-service`/`alerting-service`) đang share, **không phải** 1 keypair
riêng cho gateway. **Bug thật tìm được lúc build T6** (`data-plane/web/src/demo/
ZoneOverviewPage.tsx`, consumer đầu tiên gọi gateway từ browser): bản đầu tự sinh 1 keypair riêng
cho gateway — token đăng nhập thật (`/auth/login`, ký bởi key chung của 3 service) không decode
được ở gateway (`401 invalid or expired token`), vì gateway kiểm theo key khác. Sửa bằng trỏ về
đúng key chung — giờ 1 token đăng nhập bình thường dùng được cho cả REST lẫn `/graphql`, không
cần "token gateway" riêng.

Gateway decode-only ở bước NÀY (xác nhận người gọi có token hợp lệ để vào `/graphql` được). Từ
đó xuống upstream (2026-09-02, xem `metap/crates/graphql-gateway/README.md`'s Auth section cho
chi tiết đầy đủ, và 8 field custom ở trên forward cùng token đó thẳng tới REST endpoint): vì
gateway + cả 3 service WAF share đúng 1 keypair (`../keys/dev-jwt-*.pem`), token của caller được
**forward nguyên vẹn** xuống upstream — permission check ở upstream chạy theo đúng identity/role
người gọi thật, không phải 1 service account chung. Khi không có token để forward (lúc gateway tự
fetch schema lúc boot), gateway tự login bằng service-account riêng
(`UPSTREAM_<N>_SERVICE_EMAIL`/`SERVICE_PASSWORD`), tự refresh trước khi hết hạn — không còn JWT
tĩnh mint tay phải dán vào `.env` và refresh thủ công mỗi giờ nữa (bug thật gặp phải: token cũ hết
hạn → gateway 401 lúc boot → cả `web` domino theo). **Dùng được cho cả mutation qua `/graphql`**,
không còn giới hạn "chỉ query, mutation phải đi REST" của bản trước.

## Đã verify sống (2026-09-02, trước khi có 8 field custom)

Cả 3 upstream chạy thật (`GRPC_ENABLED=true`), gateway boot xác nhận `schema built, entities=9`.
1 query GraphQL gộp `wafZones` + `wafScanJobsList` + `wafIncidentsList` trong 1 request → 1
response chứa đủ dữ liệu thật từ cả 3 service — bằng chứng BFF thật, không phải suy luận.

**T6 (2026-09-01)**: `ZoneOverviewPage` (`../web/src/demo/ZoneOverviewPage.tsx`, đã xoá — thay
bằng `RelatedRecordsPanel` chạy trên metadata `relatedViews`) — màn FE đầu tiên gọi gateway thật,
qua `@metap/platform-ui`'s `useGraphQLQuery`. Verify bằng đúng token đăng nhập thật (không phải
token test riêng) qua `curl` tới `web/`'s dev server `/graphql` — xác nhận cả bug keypair ở trên
lẫn toàn bộ query hoạt động đúng.

**Identity propagation + self-refreshing login (2026-09-02)**: mutation `updateWafIncidents` qua
`/graphql` bằng token của 1 user thật (không phải service account) → `records.updated_by` phản
ánh đúng user_id thật đó, không phải service account. Lặp lại bằng 1 user không có quyền → `403
forbidden`, đúng permission check của upstream đánh giá theo role người gọi thật. Gateway sau đó
được rebuild sang service-auth login-based, verify boot log có bước login trước "fetching schema",
cùng bộ curl positive/negative trên vẫn pass — không regression.

## Đã verify sống (8 field custom, 2026-09-04)

Build/clippy/fmt sạch (`cargo build/clippy -D warnings/fmt --check -p waf-graphql-gateway`). Chạy
thật cả 3 upstream + gateway trên Postgres/RabbitMQ sống (`GRPC_ENABLED=true`), boot log xác nhận
`schema built, entities=9` — cả 8 field custom gọi qua `/graphql` bằng token đăng nhập thật, đều
trả đúng kết quả REST endpoint gốc trả (dữ liệu thật đọc/ghi qua Postgres):

- `verifyZoneDns`/`testZoneOrigin`/`syncZoneConfigState` (zones) — DoH lookup thật, origin probe
  thật (tới `1.2.3.4` fail đúng như kỳ vọng, không phải origin thật), config-state sync thật.
- `runScanJob` (scanning) — queue thật, đúng thông báo "chưa có scanner backend" khi `SCANNER_URL`
  không set.
- `testAlertPolicy`/`correlateIncidents`/`evaluateAlerts` (alerting) — webhook test thật (403 vì
  URL giả), correlate/evaluate chạy đúng logic thật trên `records` rỗng.
- `aggregate(entity, spec)` — tạo 1 zone thật qua GraphQL (`createWafZones`), `aggregate(entity:
  "waf.zones", spec: {metrics: ["count"], groupBy: "status"})` trả đúng `[{count: 1, group:
  "pending"}]`.

Không phát hiện bug nào ở resolver — 1 lỗi gặp lúc test hoá ra là do gọi sai shape `metrics`
(`{fn: "count"}` thay vì `"count"`, đúng dạng wire `AggregateMetric::parse` cần) ở phía test, xác
nhận lại bằng cách gọi thẳng REST endpoint gốc với cùng shape sai → cùng lỗi 422 y hệt.

## Phase 4 — FE (`data-plane/web/src/api/waf.ts`) chuyển hẳn sang gọi qua đây (2026-09-04)

`api/waf.ts` (đọc doc comment đầu file đó cho chi tiết đầy đủ) không còn gọi REST nữa — mọi hàm
export giữ nguyên tên/chữ ký, chỉ đổi phần thân sang gọi `/graphql`:

- `useRecords`/`useRecord` (list/get chung) — dựng selection set từ field list thật của entity
  (`useEntity`/`fetchEntityFields`, gọi `GET /metadata/entities/{entity}` — cố tình để lại REST vì
  đây là schema reflection, không phải business data, cùng nhóm với `/auth/*`/`/preferences/*`),
  rồi reshape response phẳng của GraphQL về lại đúng shape `WafRecord<T> = {id, ..., data: {...}}`
  cũ — không màn hình nào trong `pages/*.tsx` phải đổi gì.
- `createRecord`/`updateRecord`/`deleteRecord`/`transitionRecord` — gọi thẳng
  `create{Type}`/`update{Type}`/`delete{Type}`/`transition{Type}` sinh tự động từ metadata (không
  phải field custom ở file này), cùng cách resolve field list ở trên.
- `useAggregate` + 7 action custom (`verifyDns` → `verifyZoneDns`, ...) — gọi thẳng field custom
  tương ứng ở trên; vì mỗi field custom trả nguyên JSON response gốc của REST endpoint làm giá trị
  scalar `Json`, type `{data: ...}` cũ trong `waf.ts` không đổi 1 byte nào.

Verify: `tsc -b`/`oxlint`/`prettier --check`/`vite build` sạch trên `data-plane/web` (backend
`waf-graphql-gateway` đã verify sống ở trên rồi, không đổi lại lần này). Theo đúng "Frontend
verification policy" của `../../CLAUDE.md` (repo `metap`) — không tự browser-test, viết code +
typecheck/lint xong là báo cáo, để user tự kiểm trên trình duyệt.
