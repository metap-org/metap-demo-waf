# 3. Phân tích nghiệp vụ

> **Trạng thái: bước 1 (concern) cho 11 nghiệp vụ, chờ chủ dự án trả lời (2026-11-06).** Chưa nghiệp vụ nào
> có luồng được duyệt.

Bản đồ phân rã nói hệ thống **có** những chức năng gì. Mục này đi qua từng nghiệp vụ theo trình tự khách
thật làm. Khi xong, mỗi nghiệp vụ trả lời được các câu hỏi sau:

- Bước nào phải làm trước bước nào, và vì sao.
- Khách phải chuẩn bị gì trước khi bắt đầu.
- Điều kiện nào được kiểm ở mỗi cổng.
- Bước nào hay hỏng, và hỏng thì xử lý thế nào.
- Mỗi bước sinh ra dữ liệu gì.

Thứ tự phân tích đi theo vòng đời của khách: onboarding trước, vì đó là lần đầu sản phẩm chạm vào traffic
thật và cũng là lúc dễ gây sự cố nhất.

Mỗi nghiệp vụ đi qua hai bước, và **không viết bước 2 khi chủ dự án chưa trả lời bước 1**:

1. **Concern**: kịch bản theo từng trục (mã `KB<số trang>-<trục><số>`) và câu hỏi cần chủ dự án quyết
   (`OQ-…`). Ý kiến lúc ghi để ở cột "ghi chú ban đầu", không phải quyết định.
2. **Luồng**: sau khi có trả lời; khi đó cập nhật [§1](../01-functional-decomposition.md),
   [§2](../02-requirements.md) và kiến trúc.

| Mục | Nghiệp vụ | Câu hỏi | Trạng thái |
|---|---|---|---|
| [3.1](01-onboarding.md) | Onboarding một website | OQ-21…OQ-32 | Bước 1, chờ chủ dự án (có bản nháp luồng cũ chưa duyệt ở phụ lục) |
| [3.2](02-policy-change.md) | Viết, duyệt, phát hành một thay đổi policy | OQ-33…OQ-40 | Bước 1, chờ chủ dự án |
| [3.3](03-attack-response.md) | Phản ứng khi bị tấn công | OQ-41…OQ-47 | Bước 1, chờ chủ dự án |
| [3.4](04-false-positive.md) | Xử lý chặn nhầm | OQ-48…OQ-53 | Bước 1, chờ chủ dự án |
| [3.5](05-plans-billing.md) | Mua, đổi gói, vượt hạn mức, thanh toán | OQ-54…OQ-61 | Bước 1, chờ chủ dự án |
| [3.6](06-members-access.md) | Quản lý thành viên, SSO, API key | OQ-62…OQ-68 | Bước 1, chờ chủ dự án |
| [3.7](07-offboarding.md) | Rời dịch vụ: gỡ zone, xoá org, xoá dữ liệu | OQ-69…OQ-74 | Bước 1, chờ chủ dự án |
| [3.8](08-monitoring-investigation.md) | Giám sát, điều tra, cảnh báo, báo cáo | OQ-75…OQ-80 | Bước 1, chờ chủ dự án |
| [3.9](09-bot-api-security.md) | Bật bot management và API security | OQ-81…OQ-85 | Bước 1, chờ chủ dự án |
| [3.10](10-cache-performance.md) | Cache, hiệu năng, nội dung (gồm chính sách abuse) | OQ-86…OQ-88 | Bước 1, chờ chủ dự án |
| [3.11](11-provider-operations.md) | Vận hành của NCC: threat research, support, sự cố nền tảng, bán hàng | OQ-89…OQ-93 | Bước 1, chờ chủ dự án |

## Câu hỏi nối giữa các nghiệp vụ

Một số câu hỏi xuất hiện ở nhiều trang; trả lời một lần cho tất cả:

| Chủ đề | Câu hỏi |
|---|---|
| Một khách nhỏ nhiều domain, agency, reseller, MSSP | OQ-27, OQ-54, OQ-59, OQ-62, OQ-63, OQ-75 |
| Ai được xem dữ liệu request (dữ liệu cá nhân) | OQ-50, OQ-68, OQ-84 |
| NCC làm thay khách (bật mitigation, sửa policy, khôi phục quyền) | OQ-40, OQ-41, OQ-64 |
| Pháp lý: cơ quan chức năng, lưu log, abuse | OQ-45, OQ-73, OQ-79, OQ-88 |
| Dữ liệu cá nhân trong audit và khi xoá | OQ-6, OQ-70, OQ-71 |
