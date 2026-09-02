# zones-service

**Dùng để làm gì**: quản lý danh sách website/domain khách hàng đăng ký bảo vệ (`Zone`) cùng 2
loại chính sách gắn trực tiếp với 1 zone — chính sách chống DDoS L7 (`DdosPolicy`) và rule WAF/
rate-limit/IP-geo firewall (`FirewallRule`). Đây là service **anchor** của toàn bộ Customer
Portal backend — mọi service khác (`scanning-service`, `alerting-service`) đều nói tới 1 `Zone`
qua `zoneId`, nên `zones-service` luôn phải được provision/reconcile trước tiên khi triển khai
lần đầu (xem "Thứ tự triển khai" bên dưới).

## Entity sở hữu

| Entity | Dùng để làm gì |
|---|---|
| `waf.zones` | 1 website/domain khách đăng ký bảo vệ — hostname, origin server thật đứng sau, trạng thái bật/tắt bảo vệ (`pending→active→paused→suspended`), chế độ `monitor` (chỉ log) hay `enforce` (chặn thật) |
| `waf.ddos_policies` | Ngưỡng phát hiện + hành động chống DDoS L7 cho 1 zone (tối đa 1 policy hiệu lực/zone) |
| `waf.firewall_rules` | Rule engine dùng chung cho WAF custom rule / rate-limit / IP-geo firewall — nhiều rule/zone, có thứ tự ưu tiên |

## Vì sao 3 entity này chung 1 service

`Zone`'s workflow guard `activate` phụ thuộc field kỹ thuật `hasConfig`, được app logic tự cập
nhật mỗi khi 1 `DdosPolicy`/`FirewallRule` tạo/xoá cho zone đó (xem `zone_entity.rs`'s doc
comment). Giữ hook này in-process (không phải gọi cross-service) ở đúng chỗ nhạy cảm nhất
(workflow guard) — tách nó ra sẽ biến 1 write đơn giản thành 1 network call có thể fail, làm
guard kém tin cậy hơn hẳn.

## Không sở hữu gì

Không đăng ký entity của `scanning-service` (`waf.scan_jobs`/`waf.scan_findings`) hay
`alerting-service` (`waf.security_events`/`waf.incidents`/`waf.alert_policies`/
`waf.alert_notifications`) — cố tình, để `/api/:entity*` của service này không lộ route CRUD
cho entity không thuộc mình (xem `main.rs`'s doc comment giải thích đầy đủ lý do kỹ thuật).

## Thứ tự triển khai lần đầu

`scanning-service`/`alerting-service`'s entity có field `zoneId` (String, không FK — xem
`scanning-service/src/entities/scan_job_entity.rs`) — không có ràng buộc DB bắt `zones-service`
chạy trước, nhưng **về nghiệp vụ** một `Zone` phải tồn tại trước khi tạo `ScanJob`/
`SecurityEvent`/`Incident` trỏ `zoneId` tới nó mới có ý nghĩa. Khuyến nghị: chạy migrate/reconcile
`zones-service` trước tiên trên 1 tenant mới, dù không phải yêu cầu kỹ thuật cứng.

## Chạy

```bash
cp .env.example .env   # chỉnh DATABASE_URL/keys nếu cần
cargo run -p zones-service   # từ data-plane/ (workspace root)
```

Mặc định `PORT=3000`, gRPC opt-in ở `GRPC_PORT=3001` (đặt `GRPC_ENABLED=true` để bật — dùng cho
`graphql-gateway` gộp cross-service, xem root README).
