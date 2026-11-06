# ADR-0012: Go là ngôn ngữ chính cho mọi plane

- Trạng thái: Bị thay thế bởi [ADR-0030](0030-language-per-workload.md)
- Ngày: 2026-11-06

## Bối cảnh

Org đã chuyển toàn bộ backend (`metap`, `metap-lowcode`, demo WAF) sang Go. Đội nhỏ, không đủ
người duy trì nhiều ngôn ngữ. Edge cần hiệu năng cao; replay cần chạy đúng logic của edge.

## Quyết định

Go cho edge gateway, Control, các service Analytics và Management. Ngoại lệ chỉ là thành phần mua
hoặc mã nguồn mở chạy như process riêng (Vector, Kafka, ClickHouse, PostgreSQL, OpenBao, Valkey).

## Hệ quả

- Edge và replay dùng chung package `pipeline`: mô phỏng chạy đúng code đang chặn traffic.
- Coraza (Go) nhúng thẳng vào gateway, không cần FFI ([ADR-0013](0013-edge-gateway-custom-go-with-coraza.md)).
- GC của Go là rủi ro cho độ trễ p99 của edge: phải benchmark, giới hạn cấp phát trên đường
  request, tinh chỉnh `GOGC`/`GOMEMLIMIT` ([11 R1](../11-risks/00-index.md#111-rủi-ro)).

## Phương án đã cân nhắc

- **Rust cho edge** (Pingora): độ trễ đuôi tốt hơn, nhưng org vừa bỏ Rust; replay phải viết lại
  logic hoặc gọi qua FFI.
- **NGINX/OpenResty + ModSecurity**: trưởng thành, nhưng pipeline tuỳ biến viết bằng Lua/C, khó
  dùng chung với replay và khó kiểm thử.
