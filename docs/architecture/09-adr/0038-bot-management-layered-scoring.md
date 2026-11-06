# ADR-0038: Bot management chấm điểm nhiều lớp ở tầng L7

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- Bot management chạy toàn bộ ở **tầng L7**, trước rate limit và custom rule. Tầng WAF không
  tham gia ([ADR-0034](0034-pre-filter-and-rule-split-across-tiers.md)).
- Tín hiệu theo lớp, rẻ trước đắt sau ([8.16 §2](../08-cross-cutting/16-bot-management.md#2-các-lớp-tín-hiệu)):
  - bot đã xác minh;
  - danh sách của khách;
  - fingerprint tĩnh (JA4, HTTP/2, header) và độ nhất quán với User-Agent;
  - reputation mạng;
  - hành vi ngắn hạn;
  - kết quả JS challenge;
  - mô hình.
- Kết quả là `bot.score` (1–99), `bot.class`, `bot.category`, `bot.signals`; tất cả dùng được trong
  policy và custom rule.
- Giai đoạn Mở rộng dùng **điểm có trọng số công bố**. Mô hình học máy (cây quyết định, suy luận µs ở
  edge) chỉ thay thế khi đo được tốt hơn trên tập đánh giá, và vẫn phải trả `signals` để giải thích.
- Danh sách bot đã xác minh do NCC duy trì (dải IP công bố + kiểm rDNS), phát qua bundle chung. Bot tự
  xưng là bot tốt mà không khớp thì bị coi là giả mạo.
- **Challenge do edge tự phát và kiểm**, không phụ thuộc CAPTCHA bên thứ ba (không gửi dữ liệu người dùng
  ra ngoài, chạy được on-prem). CAPTCHA bên thứ ba là tuỳ chọn.
- Dữ liệu bot (reputation, mô hình) đi cùng quy trình phát hành như ruleset: đánh giá, log-only song
  song, rồi mới chuyển mặc định.
- Đoạn JS thu tín hiệu ngầm mặc định tắt.

## Hệ quả

- L7 cần thêm bộ tính fingerprint TLS/HTTP/2 ở tầng nhận kết nối. Đây là một tiêu chí khi chọn nền
  L7 ([ADR-0031](0031-edge-l7-and-waf-engine-options.md)): nền nào cho truy cập ClientHello và frame
  HTTP/2 thô.
- Telemetry thêm `bot.*`, JA4, fingerprint HTTP/2 (đã băm).
- Gộp reputation chéo tenant phải ẩn danh, chỉ thống kê theo fingerprint, IP, ASN.

## Phương án đã cân nhắc

- **Mua dịch vụ bot detection bên thứ ba qua API:** nhanh nhưng thêm một lời gọi mạng trên đường request
  (vi phạm [ADR-0001](0001-four-planes.md)) và gửi dữ liệu người dùng ra ngoài.
- **Chỉ dùng rate limit và reputation IP:** không bắt được bot dùng proxy dân cư và trình duyệt headless.
