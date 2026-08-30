# 04 — Architecture Boundary: 3 Plane của `metap-demo-waf`

Ghi lại đúng quyết định đã thống nhất, để không lẫn lộn phạm vi khi vào chi tiết kỹ thuật. Repo
`metap-demo-waf` chia 3 thư mục top-level theo đúng 3 vai trò dưới đây — mỗi thư mục 1 codebase/
deploy cycle riêng.

## Ranh giới

- **`data-plane/` (thư mục này) = nơi giữ dữ liệu/config nguồn (source of truth).** Portal cấu
  hình nghiệp vụ (Zone, DdosPolicy, FirewallRule, ScanJob/Finding, Incident, Alert) — build trên
  `metap`, hưởng sẵn CRUD/permission/workflow generic, RBAC matrix, low-code entity nếu cần mở
  rộng nhanh. Đây là nơi duy nhất khách hàng/SOC tương tác trực tiếp qua UI.
- **`control-plane/` = lấy config từ `data-plane`, tính toán, đẩy xuống `edge-plane`.** Không có
  UI, không phải CRUD — là 1 (hoặc vài) worker/service chạy nền, subscribe thay đổi từ
  `data-plane`, biên dịch (compile) thành rule-set mà `edge-plane` hiểu được, rồi phân phối
  (distribute) xuống. Vai trò tương đương "Quicksilver" của Cloudflare hay control-plane API kiểu
  xDS (Envoy) — chỉ khác là pull-from-cache thay vì push-stream (xem chi tiết bên dưới).
- **`edge-plane/` = hệ thống riêng, tự code, thực thi mitigation thật.** Nhận traffic, evaluate
  rule, chặn/challenge request — bài toán hiệu năng cao, latency thấp, không phải CRUD, không hợp
  với metadata-driven approach của `metap`. Chỉ đọc config đã được `control-plane` tính toán sẵn,
  không bao giờ tự đọc thẳng `data-plane`.

## Luồng dữ liệu

**Config đi xuống (`data-plane` → `control-plane` → `edge-plane`):**

```
Zone/DdosPolicy/FirewallRule đổi (qua portal, data-plane)
  → metap outbox (transaction cùng lúc ghi DB — không mất event kể cả RabbitMQ down)
  → outbox-publisher → RabbitMQ
  → control-plane: 1 worker (kiểu notification-worker của metap, tên gợi ý
    "waf-config-distributor") subscribe event, build rule-set đã tính toán sẵn cho từng Zone,
    ghi vào Redis/DragonflyDB (metap-cache đã có RedisCache, không cần code cache mới)
  → edge-plane đọc thẳng Redis (latency thấp, nhiều edge node đọc cùng 1 key không tải lên
    data-plane)
```

`Zone.configVersion` (định nghĩa ở `data-plane`) tăng mỗi lần có thay đổi — `control-plane` và
`edge-plane` đều so `configVersion` để biết cache có đang stale không, không cần đoán qua
timestamp.

**SLA đồng bộ config xuống edge: 10-30 giây** — từ lúc khách bấm bật/tắt 1 rule/policy (hoặc đổi
`protectionMode`) ở `data-plane` tới lúc `edge-plane` thực sự áp dụng thay đổi đó. Vì có độ trễ
này (kiến trúc pull-based/cache, không phải push tức thời), các guard/validation ở `data-plane`
(vd `Zone.activate`) **không nên** coi việc bật/tắt 1 rule là 1 thao tác "đồng bộ, có kết quả tức
thời" — đó là quyết định đã áp dụng khi thiết kế guard `hasConfig` (chỉ check *có tồn tại*
policy/rule, không check *đang bật*, xem `08-module-detail-specs.md` quyết định #2).

**Telemetry đi lên (`edge-plane` → `control-plane`/`data-plane`):** `SecurityEvent` (số lượng
lớn, near real-time) — cách ghi vào `data-plane` chưa chốt kỹ thuật, 2 hướng khả dĩ để bàn sau:
1. `edge-plane` gọi thẳng `metap-grpc`'s generic `RecordService.Create` trên `data-plane` (đã có
   sẵn, generic mọi entity) — đơn giản, nhưng N edge node × traffic lớn → nhiều lời gọi gRPC nhỏ
   lẻ tới `data-plane`.
2. `edge-plane` tự batch/buffer, gửi qua `control-plane` trước (aggregate/dedupe ở đó), rồi
   `control-plane` mới ghi xuống `data-plane` theo batch lớn hơn — đúng vai trò trung gian
   `control-plane` đã có sẵn cho chiều xuống, tận dụng luôn cho chiều lên thay vì thêm 1 đường
   ghi trực tiếp `edge-plane` → `data-plane` khác.

**Chưa chốt hướng nào** — cần bàn kỹ khi vào phase kỹ thuật, vì đây là điểm duy nhất
`SecurityEvent` (khối lượng lớn nhất hệ thống) có khả năng phá nguyên tắc kiến trúc "mọi write đi
qua `CrudService`" mà `metap` đang giữ nghiêm ngặt ở mọi entity khác. Hướng 2 (qua `control-plane`)
có vẻ hợp lý hơn vì giữ đúng nguyên tắc "`edge-plane` không bao giờ nói chuyện trực tiếp với
`data-plane`" đã chốt ở trên.

## Vì sao tách 3 plane

- `data-plane` cần đổi nhanh theo nghiệp vụ (thêm field, đổi workflow, thêm loại rule) — đúng thế
  mạnh `metap` (metadata-driven, low-code).
- `edge-plane` cần ổn định, hiệu năng cao, không đổi theo từng lần khách chỉnh 1 field UI — không
  nên chung 1 codebase/deploy cycle với `data-plane`.
- `control-plane` đứng giữa để 2 phía không cần biết nội bộ của nhau: `edge-plane` không cần biết
  schema Postgres/JSONB của `metap`, `data-plane` không cần biết edge-plane có bao nhiêu node/đặt
  ở đâu. Redis/DragonflyDB (`control-plane` ghi, `edge-plane` đọc) tận dụng cái đã có sẵn trong
  `metap` (`metap-cache`) thay vì xây thêm hạ tầng cache mới.
