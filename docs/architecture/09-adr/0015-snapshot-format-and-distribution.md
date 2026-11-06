# ADR-0015: Định dạng snapshot và kênh phân phối

- Trạng thái: Chấp nhận; cấu trúc manifest và cách node nhận bundle sửa bởi [ADR-0040](0040-config-distribution-at-scale.md) (manifest hai tầng theo bucket, routing index, PoP config store, nạp khi cần)
- Ngày: 2026-11-06

## Quyết định

- **Định dạng:** protobuf, nén zstd, định danh bằng SHA-256 của nội dung (content-addressed). Một
  **manifest** nhỏ liệt kê digest của bundle chung và các bundle zone, ký Ed25519 bằng khoá ở OpenBao.
- **Kênh điều khiển:** mỗi node giữ một gRPC stream hai chiều (mTLS) tới Distribution, theo mô
  hình **desired state** của xDS: Control gửi "node này phải chạy manifest X", node trả ACK hoặc
  NACK kèm lỗi, có nonce để khớp.
- **Kênh dữ liệu:** node kéo bundle theo digest qua HTTPS từ **regional relay** (cache bất biến,
  vì nội dung theo digest không bao giờ đổi); relay kéo từ artifact store.
- **Policy agent nằm trong binary của từng tầng** (proxy và WAF, [ADR-0033](0033-two-tier-edge-proxy-and-waf.md)), không phải process riêng: swap con trỏ nguyên khối, không cần IPC. Mỗi tầng chỉ nạp phần bundle của mình từ cùng một manifest.
- **Bí mật không nằm trong snapshot:** private key cert được agent lấy riêng qua mTLS, chỉ giữ trong RAM.

## Hệ quả

- Bundle giống nhau cho mọi node nên cache được ở relay; một zone đổi chỉ tải lại bundle zone đó.
- Mất kết nối chỉ cần so manifest khi nối lại.
- Phải tự viết server desired-state; không dùng nguyên xDS của Envoy vì tài nguyên khác hẳn.

## Phương án đã cân nhắc

- **Kho key-value chung mà edge tự đọc** (như demo): không nguyên khối, không chữ ký, không ack.
- **Pub/sub broker xuống từng node** (NATS, Kafka): thêm một hệ thống phải chạy ở mọi PoP và
  hybrid; kênh gRPC + HTTPS đủ cho số node dự kiến.
