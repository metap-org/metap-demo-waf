# alerting-service

**Dùng để làm gì**: gộp trụ cột "Analytics + Alerting + Incident management" — thứ 4 trong 4 trụ
cột sản phẩm (`../../docs/01-product-vision.md`), phần chứng minh 3 trụ cột kia (DDoS/WAF/Scan)
thực sự hoạt động: log traffic bị chặn/log/challenge (`SecurityEvent`), gộp thành sự cố cần SOC
xử lý (`Incident`), và cấu hình cảnh báo tự động (`AlertPolicy`/`AlertNotification`).

## Entity sở hữu

| Entity | Dùng để làm gì |
|---|---|
| `waf.security_events` | Log 1 request bị `DdosPolicy`/`FirewallRule` match — ghi từ `edge-plane` gửi lên, khối lượng lớn nhất hệ thống, không có workflow (append-only) |
| `waf.incidents` | Gộp nhiều `SecurityEvent` liên quan (cùng zone/nguồn/khung giờ) thành 1 case SOC cần xử lý — workflow `open→acknowledged→mitigating→resolved` |
| `waf.alert_policies` | Cấu hình "khi nào báo động" — ngưỡng số event trong khung thời gian, kênh gửi (email/webhook). Theo tenant, không theo zone — 1 policy theo dõi nhiều zone, mỗi zone tính riêng |
| `waf.alert_notifications` | Log audit mỗi lần thực sự gửi cảnh báo — tách khỏi `AlertPolicy` vì là lịch sử phát sinh, không phải cấu hình |

## Vì sao 4 entity này chung 1 service

Logic gộp `SecurityEvent` → `Incident` (correlation) chạy qua trigger `OnRecordEvent` trên
`waf.security_events.created`, gọi thẳng vào `CrudService` **in-process** để tạo/update
`Incident` — giữ 2 entity này chung 1 service tránh biến correlation logic (chạy khá thường
xuyên, khối lượng cao) thành 1 cross-service call. `AlertPolicy`/`AlertNotification` đi kèm vì
cùng vòng đời "phản ứng lại SecurityEvent/Incident".

## Quan hệ với `zones-service`

`SecurityEvent.zoneId`/`Incident.zoneId` là **`String` thuần, không phải `Reference`** — cùng lý
do `scanning-service`'s `ScanJob.zoneId` (xem `src/entities/security_event_entity.rs`'s doc
comment): `waf.zones` thuộc `zones-service`, đăng ký nó ở đây để pass validate sẽ lộ CRUD
`waf.zones` qua route generic của chính service này.

## Chạy

```bash
cp .env.example .env   # chỉnh DATABASE_URL/keys nếu cần
cargo run -p alerting-service   # từ data-plane/ (workspace root)
```

Mặc định `PORT=3020`, gRPC opt-in ở `GRPC_PORT=3021` (đặt `GRPC_ENABLED=true` để bật — dùng cho
`graphql-gateway` gộp cross-service, xem root README).
