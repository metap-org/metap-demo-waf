# 3.10 Cache, hiệu năng, nội dung

> **Trạng thái: đang mở, chưa có quyết định nào (2026-11-06).** Trang này chỉ liệt kê kịch bản và câu hỏi.

## 1. Phạm vi

Khách dùng cache để giảm tải origin, xoá cache khi đổi nội dung, và chịu trách nhiệm về nội dung đi qua
edge. Liên quan: F4 ở [§1](../01-functional-decomposition.md#f4-hiệu-năng-và-phân-phối).

## 2. Kịch bản theo từng trục

### A. Cái gì được cache

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-A1 | Site động (giỏ hàng, đăng nhập, trang cá nhân) | Cache nhầm trang của người này cho người khác là rò rỉ dữ liệu, rủi ro lớn nhất |
| KB10-A2 | Origin không gửi `Cache-Control` đúng | Mặc định cache gì |
| KB10-A3 | Cache key theo query string, cookie, ngôn ngữ, thiết bị | |
| KB10-A4 | File lớn, video, tải theo đoạn (range) | Giới hạn kích thước theo gói |

### B. Xoá cache

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-B1 | Xoá sau mỗi lần deploy | Qua API trong CI |
| KB10-B2 | CMS tự xoá khi sửa bài (plugin WordPress) | Có làm plugin không |
| KB10-B3 | Xoá theo tag | Mở rộng |

### C. Origin lỗi

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-C1 | Origin chết, phục vụ bản cũ | Bao lâu, có báo người dùng không |
| KB10-C2 | Origin chậm | Edge có timeout ngắn hơn trình duyệt |

### D. Nội dung đi qua edge

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-D1 | Website lừa đảo, phishing, phát tán mã độc dùng edge để giấu origin | Nhận báo cáo abuse, xác minh, khoá; NCC có trách nhiệm gì |
| KB10-D2 | Yêu cầu gỡ nội dung vi phạm bản quyền | Quy trình |
| KB10-D3 | Yêu cầu tiết lộ origin thật của một website (nạn nhân, cơ quan chức năng) | Quy trình, căn cứ |

## 3. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-86 | Mặc định cache gì khi origin không nói gì? | KB10-A1, KB10-A2 | Chỉ tài nguyên tĩnh là an toàn |
| OQ-87 | Có làm plugin cho CMS phổ biến (WordPress) không? | KB10-B2 | |
| OQ-88 | Chính sách abuse: ai nhận báo cáo, thời gian xử lý, khi nào khoá zone, có tiết lộ origin không? | KB10-D1…KB10-D3 | Cần tư vấn pháp lý; dịch vụ che origin luôn bị dùng cho mục đích xấu |

## 4. Phụ thuộc

FR-4-01…FR-4-04 ở [§2](../02-requirements.md). Chính sách abuse chưa có requirement nào.
