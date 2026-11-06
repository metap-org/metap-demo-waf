# ADR-0019: Vector làm log shipper trên node

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- Gateway ghi bản ghi telemetry (protobuf) vào ring buffer trong RAM, gửi không chặn qua Unix
  socket tới **Vector** chạy cạnh gateway. Ring buffer đầy thì bỏ và tăng bộ đếm, không làm chậm request.
- Vector lo: buffer đĩa, lấy mẫu access log theo entitlements, che thêm trường (VRL), nén, gửi
  Kafka với retry.
- Che trường nhạy cảm cơ bản (`Authorization`, cookie, mật khẩu) làm ngay trong gateway; Vector chỉ
  che thêm theo cấu hình khách.

## Lý do

- Buffer đĩa, retry, backpressure, sink Kafka đều có sẵn và đã được kiểm chứng.
- Tách I/O mạng khỏi process gateway: Vector chậm hay khởi động lại không ảnh hưởng request.

## Hệ quả

- Cấu hình Vector (tỉ lệ lấy mẫu, luật che) sinh từ snapshot; policy agent ghi lại file cấu hình
  và gọi reload của Vector.
