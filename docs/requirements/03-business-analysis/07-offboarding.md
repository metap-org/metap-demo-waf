# 3.7 Rời dịch vụ: gỡ zone, xoá org, xoá dữ liệu

> **Trạng thái: đang mở, chưa có quyết định nào (2026-11-06).** Trang này chỉ liệt kê kịch bản và câu hỏi.

## 1. Phạm vi

Khách gỡ một zone, huỷ gói, xoá org, hoặc NCC chấm dứt dịch vụ. Gồm việc trả traffic về origin, xuất và xoá
dữ liệu. Liên quan: F2.4.3, F1.1.4, F9.3 ở [§1](../01-functional-decomposition.md).

## 2. Kịch bản theo từng trục

### A. Gỡ một zone

| Mã | Kịch bản | Concern |
|---|---|---|
| KB7-A1 | Khách gỡ zone sau khi đã trỏ DNS về origin | Bình thường |
| KB7-A2 | Khách gỡ zone khi DNS vẫn trỏ về edge | Edge trả gì: lỗi, trang thông báo, hay vẫn chuyển tiếp một thời gian |
| KB7-A3 | Origin đã khoá chỉ nhận edge | Trỏ DNS về origin xong thì origin từ chối người dùng; cần nhắc mở lại |
| KB7-A4 | Origin không có cert vì trước đây edge lo TLS | HTTPS lỗi sau khi rời |
| KB7-A5 | Gỡ nhầm zone | Khôi phục trong bao lâu |

### B. Huỷ gói, xoá org

| Mã | Kịch bản | Concern |
|---|---|---|
| KB7-B1 | Huỷ gói, giữ org và cấu hình | Zone về gói nào (Free, tạm dừng) |
| KB7-B2 | Xoá org | Thời hạn huỷ yêu cầu xoá; ai được yêu cầu (chỉ owner) |
| KB7-B3 | Xoá account trong org | Zone của account đi đâu |
| KB7-B4 | Org còn nợ | Có cho xoá không |

### C. Dữ liệu

| Mã | Kịch bản | Concern |
|---|---|---|
| KB7-C1 | Xuất trước khi xoá: cấu hình (dạng API hoặc Terraform), log, audit, hoá đơn | Định dạng, kích thước |
| KB7-C2 | Audit không được xoá nhưng có dữ liệu cá nhân | Ẩn danh (OQ-6) |
| KB7-C3 | Hoá đơn, chứng từ phải giữ theo luật kế toán | Giữ dù org đã xoá |
| KB7-C4 | Bản sao lưu vẫn chứa dữ liệu đã xoá | Xoá khỏi backup hay để hết hạn backup |
| KB7-C5 | Tenant Enterprise có database riêng | Huỷ database và khoá mã hoá |
| KB7-C6 | Log đã gửi sang SIEM của khách | Ngoài trách nhiệm của NCC |

### D. NCC chấm dứt

| Mã | Kịch bản | Concern |
|---|---|---|
| KB7-D1 | Vi phạm điều khoản, gian lận thanh toán | Khoá có báo trước không |
| KB7-D2 | Website lừa đảo, phishing, nội dung vi phạm pháp luật đứng sau edge | Xem 3.10; khoá ngay |
| KB7-D3 | Yêu cầu của cơ quan nhà nước | Quy trình, căn cứ pháp lý |
| KB7-D4 | NCC ngừng một gói hoặc một tính năng | Báo trước bao lâu |

### E. Domain

| Mã | Kịch bản | Concern |
|---|---|---|
| KB7-E1 | Domain hết hạn đăng ký khi zone còn hoạt động | Zone tiếp tục, hay tạm dừng |
| KB7-E2 | Domain đổi chủ (bán, chuyển nhượng) | Chủ mới không được thừa hưởng cấu hình và log của chủ cũ |

## 3. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-69 | Zone đã gỡ mà DNS vẫn trỏ về edge: edge trả gì, trong bao lâu? | KB7-A2 | |
| OQ-70 | Thời hạn giữ dữ liệu sau khi xoá org, thời hạn khôi phục zone gỡ nhầm | KB7-A5, KB7-B2 | Gắn với OQ-6 |
| OQ-71 | Xoá khỏi bản sao lưu: xoá chủ động hay chờ backup hết hạn? | KB7-C4 | |
| OQ-72 | Xuất cấu hình dạng nào khi rời (JSON của API, Terraform)? | KB7-C1 | |
| OQ-73 | Quy trình khoá theo yêu cầu cơ quan nhà nước và khi phát hiện vi phạm | KB7-D2, KB7-D3 | Cần tư vấn pháp lý |
| OQ-74 | Domain hết hạn hoặc đổi chủ: phát hiện thế nào, làm gì? | KB7-E1, KB7-E2 | |

## 4. Phụ thuộc

FR-1-03, FR-2-13, FR-9-03…FR-9-05, BR-12, BR-16, DT-7, OQ-6 ở [§2](../02-requirements.md); `DataErasureRequested`
ở kiến trúc 5.5.
