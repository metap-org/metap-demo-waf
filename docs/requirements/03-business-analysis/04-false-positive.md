# 3.4 Xử lý chặn nhầm

> **Trạng thái: đang mở, chưa có quyết định nào (2026-11-06).** Trang này chỉ liệt kê kịch bản và câu hỏi.

## 1. Phạm vi

Từ lúc một người dùng thật hoặc client hợp lệ bị chặn hoặc bị challenge, cho tới khi nguyên nhân được sửa
và có bằng chứng không còn chặn nhầm. Đây là nguyên nhân chính làm khách mất niềm tin (R2). Liên quan: F7.5,
F5.1.4 ở [§1](../01-functional-decomposition.md).

## 2. Kịch bản theo từng trục

### A. Ai phát hiện

| Mã | Kịch bản | Concern |
|---|---|---|
| KB4-A1 | End-user bị chặn, gọi hotline hoặc nhắn cho chủ website | End-user không biết request ID là gì |
| KB4-A2 | Chủ website thấy doanh thu hoặc đơn hàng giảm | Không biết là do chặn |
| KB4-A3 | SOC thấy rule khớp bất thường | Tỉ lệ chặn nhầm theo rule |
| KB4-A4 | Hệ thống tự nghi (rule khớp nhiều trên người dùng đã đăng nhập, trên cùng path) | Mức tin cậy của tín hiệu |
| KB4-A5 | Đối tác (cổng thanh toán, webhook) báo bị từ chối | Client máy không đọc được trang chặn |

### B. End-user thấy gì

| Mã | Kịch bản | Concern |
|---|---|---|
| KB4-B1 | Trang chặn có request ID và hướng dẫn liên hệ | Không lộ rule nào chặn (kẻ tấn công cũng đọc được) |
| KB4-B2 | Trang chặn tuỳ biến theo thương hiệu, tiếng Việt và tiếng Anh | Theo gói |
| KB4-B3 | Nút "báo chặn nhầm" trên trang chặn, gửi thẳng cho chủ website | Bị lạm dụng để spam; dữ liệu end-user gửi lên |
| KB4-B4 | API client nhận lỗi dạng JSON thay vì trang HTML | Định dạng lỗi theo `Accept` |
| KB4-B5 | Challenge lặp vô hạn với trình duyệt chặn cookie, mạng chậm | Người dùng kẹt |

### C. Điều tra

| Mã | Kịch bản | Concern |
|---|---|---|
| KB4-C1 | Tra request ID: rule, phần request khớp, version | Phần request khớp có thể chứa mật khẩu, dữ liệu cá nhân |
| KB4-C2 | Không có request ID (end-user chỉ nhớ giờ và trang) | Tìm theo IP, giờ, path |
| KB4-C3 | Chặn nhầm ngoài retention của gói | Không còn bằng chứng |
| KB4-C4 | Chặn ở bước pre-filter hoặc DDoS, không phải rule WAF | Khách không có rule nào để sửa |

### D. Cách xử lý

| Mã | Kịch bản | Concern |
|---|---|---|
| KB4-D1 | Exception hẹp nhất (rule × path × tham số) | Khách thường chọn cách rộng nhất cho nhanh |
| KB4-D2 | Tắt cả rule, chuyển rule sang log-only | Mở lỗ cho cả zone |
| KB4-D3 | Allowlist IP | IP động, IP NAT của nhà mạng dùng chung cho hàng nghìn người |
| KB4-D4 | Exception có hạn, tự hết hạn | Hết hạn thì chặn nhầm quay lại |
| KB4-D5 | Đang mất tiền (thanh toán hỏng): cần sửa trong vài phút | Có đi kênh khẩn không duyệt không |
| KB4-D6 | Developer tạo exception | Có cần duyệt không |

### E. Chặn nhầm do NCC

| Mã | Kịch bản | Concern |
|---|---|---|
| KB4-E1 | Bản ruleset mới gây chặn nhầm ở nhiều khách cùng lúc | NCC rollback; khách có được báo không |
| KB4-E2 | Dữ liệu bot hoặc reputation sai (IP nhà mạng bị đánh dấu xấu) | Ảnh hưởng một vùng địa lý |
| KB4-E3 | Lỗi engine | Khách không tự sửa được |

### F. Học từ chặn nhầm

| Mã | Kịch bản | Concern |
|---|---|---|
| KB4-F1 | Nhãn chặn nhầm của khách dùng để cải thiện ruleset chung | Dữ liệu một tenant dùng cho tenant khác: quyền riêng tư, hợp đồng |
| KB4-F2 | Gợi ý exception từ nhãn của khách khác cùng ứng dụng (cùng CMS) | Như trên |

## 3. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-48 | End-user bị chặn liên hệ ai? Có nút "báo chặn nhầm" gửi chủ website không? | KB4-A1, KB4-B3 | |
| OQ-49 | Exception khẩn khi đang mất tiền: có đi kênh khẩn không cần duyệt không, giới hạn thế nào? | KB4-D5 | |
| OQ-50 | Ai được xem phần request đã khớp (có thể chứa dữ liệu cá nhân)? Che những gì mặc định? | KB4-C1 | Có thể cần quyền riêng |
| OQ-51 | Có dùng nhãn chặn nhầm chéo tenant không, với điều kiện gì? | KB4-F1, KB4-F2 | |
| OQ-52 | NCC cam kết gì khi chặn nhầm do ruleset của NCC (thời gian rollback, thông báo, credit)? | KB4-E1, KB4-E2 | |
| OQ-53 | Allowlist IP có cảnh báo khi IP thuộc dải NAT dùng chung của nhà mạng không? | KB4-D3 | |

## 4. Phụ thuộc

FR-7-03, FR-7-07, FR-5-04, BR-21 ở [§2](../02-requirements.md).
