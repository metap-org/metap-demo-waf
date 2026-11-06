# ADR-0034: Pre-filter và challenge ở tầng L7; custom rule chia theo tầng

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- **Ở tầng L7, trước WAF:** access list (allow, deny, reputation), Under Attack mode và mọi
  challenge, rate limit, DDoS L7, bot tín hiệu rẻ. WAF có thể trả verdict `challenge`, nhưng việc
  phát challenge, kiểm kết quả và cấp clearance token luôn ở L7.
- **Allow có phạm vi:** một mục allow chọn bỏ qua những gì (rate limit, challenge, WAF, hoặc tất cả).
  Allow bỏ qua WAF thì L7 không gọi WAF.
- **Chia custom rule theo trường được dùng**, giữ nguyên ngữ nghĩa first-match theo priority:
  - Compiler xếp rule theo priority, tìm rule **đầu tiên** cần trường đắt (body, tham số đã parse,
    JSON path, kết quả của CRS).
  - Mọi rule **đứng trước** nó chạy ở L7 (phần pre); rule đó và mọi rule **sau** nó chạy ở WAF
    (phần inspect, WAF cũng có đủ trường rẻ).
  - Nhờ vậy không có trường hợp rule priority thấp ở L7 chặn trước một rule priority cao ở WAF.
- Portal hiển thị rule nào chạy ở tầng nào và cảnh báo khi một rule cần body đứng quá cao làm giảm
  phần lọc được ở L7.

## Hệ quả

- Khách không phải biết có hai tầng; ngữ nghĩa giống như chạy một danh sách duy nhất.
- Compiler cần biết với mỗi field là rẻ hay đắt (danh sách cố định trong môi trường CEL).
