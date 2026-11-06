# 3. Phân tích nghiệp vụ

> **Trạng thái: đang làm (2026-11-06).** Mỗi nghiệp vụ phân tích xong sẽ cập nhật lại
> [bản đồ phân rã chức năng](../01-functional-decomposition.md), [requirements](../02-requirements.md) và
> [hợp đồng giữa các plane](../../architecture/05-building-blocks/05-contracts.md). Những chỗ thay đổi được liệt kê ở cuối
> từng trang.

Bản đồ phân rã nói hệ thống **có** những chức năng gì. Mục này đi qua từng nghiệp vụ theo trình tự khách
thật làm. Với mỗi nghiệp vụ, nó trả lời các câu hỏi sau:

- Bước nào phải làm trước bước nào, và vì sao.
- Khách phải chuẩn bị gì trước khi bắt đầu.
- Điều kiện nào được kiểm ở mỗi cổng.
- Bước nào hay hỏng, và hỏng thì xử lý thế nào.
- Mỗi bước sinh ra dữ liệu gì.

Thứ tự phân tích đi theo vòng đời của khách: onboarding trước, vì đó là lần đầu sản phẩm chạm vào traffic
thật và cũng là lúc dễ gây sự cố nhất.

| Mục | Nghiệp vụ | Trạng thái |
|---|---|---|
| [requirements §3.1](01-onboarding.md) | Onboarding một website: từ lúc thêm domain tới khi traffic được bảo vệ ở chế độ chặn | Đang mở: đã liệt kê kịch bản và câu hỏi, chủ dự án đang phân tích; luồng chưa viết |
| 3.2 | Viết, duyệt, phát hành một thay đổi policy | Chưa làm |
| 3.3 | Phản ứng khi bị tấn công | Chưa làm |
| 3.4 | Xử lý chặn nhầm | Chưa làm |
| 3.5 | Mua, đổi gói, vượt hạn mức, thanh toán | Chưa làm |
| 3.6 | Quản lý thành viên, SSO, API key | Chưa làm |
| 3.7 | Rời dịch vụ: gỡ zone, xoá org, xoá dữ liệu | Chưa làm |

Mỗi trang dùng cùng một khung:

1. Mục tiêu và định nghĩa "xong".
2. Tác nhân và biến thể theo phân khúc.
3. Khách cần chuẩn bị gì.
4. Các bước, thứ tự và cổng.
5. Chi tiết từng bước: đầu vào, xử lý, kết quả, lỗi thường gặp, plane, chức năng, dữ liệu.
6. Trường hợp đặc biệt.
7. Mục tiêu thời gian.
8. Thay đổi kéo theo ở §1, §2 và ở architecture (5.5, các thành phần).
9. Câu hỏi mở.
