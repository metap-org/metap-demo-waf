# ADR-0019: Vector làm log shipper trên node và ingest gateway

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- Gateway ghi bản ghi telemetry (protobuf) vào ring buffer trong RAM, gửi không chặn qua Unix
  socket tới **Vector** chạy cạnh gateway. Ring buffer đầy thì bỏ và tăng bộ đếm, không làm chậm request.
- Vector lo: buffer đĩa, lấy mẫu access log theo entitlements, che thêm trường (VRL), nén, gửi
  tới **ingest gateway** của Analytics region với retry.
- Ingest gateway cũng là Vector (chế độ aggregator): mTLS theo cert node, kiểm schema, quota theo
  tenant/node, ghi Kafka. Edge không ghi thẳng Kafka và không giữ credential Kafka.
- Che trường nhạy cảm cơ bản (`Authorization`, cookie, mật khẩu) làm ngay trong gateway; Vector chỉ
  che thêm theo cấu hình khách.

## Lý do

- Buffer đĩa, retry, backpressure, sink Kafka đều có sẵn và đã được kiểm chứng.
- Tách I/O mạng khỏi process gateway: Vector chậm hay khởi động lại không ảnh hưởng request.

## Hệ quả

- Cấu hình Vector (tỉ lệ lấy mẫu, luật che) sinh từ snapshot; policy agent ghi lại file cấu hình
  và gọi reload của Vector.
