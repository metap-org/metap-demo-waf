# WAF `graphql-gateway` instance

**Dùng để làm gì**: 1 endpoint GraphQL duy nhất gộp cả 9 entity từ 3 service Customer Portal
backend (`zones-service`/`scanning-service`/`alerting-service`) — cho FE (hoặc bất kỳ client
nào) query xuyên nhiều service trong 1 request, thay vì tự gọi 3 REST endpoint riêng rồi tự join.
Ví dụ: 1 trang "Zone overview" cần hostname (`zones-service`) + scan gần nhất
(`scanning-service`) + incident gần nhất (`alerting-service`) — 1 GraphQL query là đủ.

**Không có code riêng ở đây** — chỉ cấu hình (`.env`) + keypair. Binary thật là
`metap/crates/graphql-gateway` (đã có sẵn, dùng chung cho cả `crm-server`/`jira-server`'s
gateway — instance này KHÔNG gộp chung upstream với instance đó, WAF là sản phẩm khác):

```bash
cp .env.example .env   # điền UPSTREAM_<N>_SERVICE_EMAIL/SERVICE_PASSWORD — 1 user thật (tạo qua
                        # dev-tools create-user + seed-admin), gateway tự login qua /auth/login
cargo run --manifest-path ../../../metap/crates/graphql-gateway/Cargo.toml
```

Mặc định `PORT=4000`. `GET /graphql/playground` (GraphiQL, non-prod) để tự gõ query thử.

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
chi tiết đầy đủ): vì gateway + cả 3 service WAF share đúng 1 keypair (`../keys/dev-jwt-*.pem`),
token của caller được **forward nguyên vẹn** xuống upstream — permission check ở upstream chạy
theo đúng identity/role người gọi thật, không phải 1 service account chung. Khi không có token để
forward (lúc gateway tự fetch schema lúc boot), gateway tự login bằng service-account riêng
(`UPSTREAM_<N>_SERVICE_EMAIL`/`SERVICE_PASSWORD`), tự refresh trước khi hết hạn — không còn JWT
tĩnh mint tay phải dán vào `.env` và refresh thủ công mỗi giờ nữa (bug thật gặp phải: token cũ hết
hạn → gateway 401 lúc boot → cả `web` domino theo). **Dùng được cho cả mutation qua `/graphql`**,
không còn giới hạn "chỉ query, mutation phải đi REST" của bản trước.

## Đã verify sống (2026-09-02)

Cả 3 upstream chạy thật (`GRPC_ENABLED=true`), gateway boot xác nhận `schema built, entities=9`.
1 query GraphQL gộp `wafZones` + `wafScanJobsList` + `wafIncidentsList` trong 1 request → 1
response chứa đủ dữ liệu thật từ cả 3 service — bằng chứng BFF thật, không phải suy luận.

**T6 (2026-09-01)**: `ZoneOverviewPage` (`../web/src/demo/ZoneOverviewPage.tsx`) — màn FE đầu
tiên gọi gateway thật, qua `@metap/platform-ui`'s `useGraphQLQuery` (lớp fetch GraphQL dùng
chung mới, cùng vai trò `apiFetch`/`useApiQuery` bên REST). Verify bằng đúng token đăng nhập
thật (không phải token test riêng) qua `curl` tới `web/`'s dev server `/graphql` — xác nhận cả
bug keypair ở trên lẫn toàn bộ query hoạt động đúng.

**Identity propagation + self-refreshing login (2026-09-02)**: mutation `updateWafIncidents` qua
`/graphql` bằng token của 1 user thật (không phải service account) → `records.updated_by` phản
ánh đúng user_id thật đó, không phải service account. Lặp lại bằng 1 user không có quyền → `403
forbidden`, đúng permission check của upstream đánh giá theo role người gọi thật. Gateway sau đó
được rebuild sang service-auth login-based, verify boot log có bước login trước "fetching schema",
cùng bộ curl positive/negative trên vẫn pass — không regression.
