# ADR-0028: Notification theo adapter, khách tự tích hợp qua webhook chuẩn, kênh mở theo gói

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Bối cảnh

Khách enterprise dùng nhiều hệ thống nhận cảnh báo khác nhau (email, chat, on-call, ITSM, SOAR tự xây).
NCC không thể viết tích hợp riêng cho từng hệ thống. Kênh cũng là thứ để phân tầng giá.

## Quyết định

- Notifier fan-out mỗi cảnh báo thành một job cho mỗi kênh, lưu trong bảng `deliveries` (PostgreSQL,
  `SKIP LOCKED`); mỗi loại kênh một pool worker; idempotency theo `(alert_id, event, channel)`; retry
  lũy thừa có jitter; lỗi vĩnh viễn báo về chủ kênh.
- Mỗi kênh là một **adapter** sau một interface chung (`ValidateConfig`, `Send`, `Test`); thêm kênh là
  thêm adapter phía NCC.
- Khách tự tích hợp bằng **webhook chung** theo CloudEvents 1.0, schema có version, chữ ký HMAC-SHA256
  kèm timestamp; tuỳ chọn template body; API kéo và ack. Khách không chạy code trong hệ thống.
- **Email luôn có** và là kênh dự phòng cho cảnh báo critical. Kênh khác, số routing rule, escalation,
  hạn mức SMS là **entitlements** theo gói, kiểm ở Management lúc cấu hình và ở notifier lúc gửi.
- Hạ gói không xoá kênh, chỉ tạm dừng và rơi về email.

## Hệ quả

- Không cần message broker riêng cho thông báo: khối lượng nhỏ, bảng PostgreSQL cho log gửi và retry
  dễ kiểm tra.
- Webhook ra ngoài phải qua SSRF guard; bí mật kênh lưu qua `SecretStore`.
- SMS và gọi thoại tính vào metering.

## Phương án đã cân nhắc

- **Dịch vụ thông báo bên thứ ba** (SaaS gửi đa kênh): nhanh nhưng không chạy được on-prem, thêm nơi
  giữ dữ liệu khách.
- **Cho khách viết hàm biến đổi (script)**: linh hoạt nhưng là bề mặt tấn công và khó hỗ trợ; template
  có danh sách biến cho phép đủ cho hầu hết trường hợp.
