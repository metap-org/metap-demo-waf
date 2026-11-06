# 3.9 Bật bot management và API security

> **Trạng thái: đang mở, chưa có quyết định nào (2026-11-06).** Trang này chỉ liệt kê kịch bản và câu hỏi. Thiết
> kế kỹ thuật ở kiến trúc 8.16, 8.17.

## 1. Phạm vi

Khách bật và tinh chỉnh bảo vệ bot và bảo vệ API trên một zone đang chạy. Hai tính năng này dễ làm hỏng
client hợp lệ hơn WAF thông thường, nên được phân tích riêng. Liên quan: F3.6, F3.7, F5.1.5, F5.1.8 ở
[§1](../01-functional-decomposition.md).

## 2. Kịch bản theo từng trục

### A. Client hợp lệ dễ bị ảnh hưởng

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-A1 | Ứng dụng mobile gọi API, không chạy được JavaScript challenge | Cách nhận diện app thật |
| KB9-A2 | Bot tìm kiếm (Google, Bing, Cốc Cốc) | Danh sách bot đã xác minh có đủ bot trong nước không |
| KB9-A3 | Crawler AI | Chặn hay cho mặc định; khách chọn |
| KB9-A4 | Đối tác lấy dữ liệu hợp lệ (so sánh giá, tổng hợp tin) | Allowlist theo gì |
| KB9-A5 | Công cụ giám sát uptime, webhook | |
| KB9-A6 | Người dùng sau NAT nhà mạng, trình duyệt cũ, mạng chậm | Bị chấm điểm bot sai |

### B. Challenge

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-B1 | Challenge bằng JavaScript ẩn | |
| KB9-B2 | Challenge tương tác (captcha) | Tự làm hay dùng bên thứ ba; trợ năng cho người khuyết tật; dữ liệu đi ra nước ngoài |
| KB9-B3 | Challenge trên trang thanh toán | Mất đơn |

### C. API security

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-C1 | Tự phát hiện endpoint từ traffic | Endpoint lạ có thể là của kẻ tấn công dò; ai xác nhận |
| KB9-C2 | Khách tải OpenAPI lên nhưng tài liệu cũ, không khớp thực tế | Chạy log-only bao lâu trước khi chặn theo schema |
| KB9-C3 | Xác thực JWT với nhiều issuer, khoá xoay | |
| KB9-C4 | Consumer là gì: API key trong header, `sub` của JWT, IP | Rate limit theo consumer |
| KB9-C5 | GraphQL (một endpoint, nhiều thao tác), gRPC | |
| KB9-C6 | API trả dữ liệu nhạy cảm trong response | Edge có đọc response không |

## 3. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-81 | Captcha tự làm, bên thứ ba, hay không có captcha (chỉ challenge ẩn)? | KB9-B2 | |
| OQ-82 | Crawler AI: mặc định cho, chặn, hay bắt khách chọn khi bật? | KB9-A3 | |
| OQ-83 | Nhận diện app mobile thật (attestation, SDK) có trong lộ trình không? | KB9-A1 | |
| OQ-84 | API discovery có lưu mẫu request, response không (dữ liệu cá nhân)? | KB9-C1, KB9-C6 | |
| OQ-85 | Bot tìm kiếm trong nước có nằm trong danh sách bot đã xác minh không, ai duy trì? | KB9-A2 | |

## 4. Phụ thuộc

FR-3-08, FR-3-09, FR-5-03, FR-5-19 ở [§2](../02-requirements.md).
