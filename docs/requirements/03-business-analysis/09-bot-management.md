# 3.9 Bot management và challenge

> **Trạng thái: đang mở, chưa có quyết định nào (2026-11-07).** Theo chủ dự án, bot và challenge là một vùng
> nghiệp vụ lớn, cần phân tích riêng; API security tách sang [3.12](12-api-security.md). Trang này chỉ liệt kê
> kịch bản và câu hỏi. Thiết kế kỹ thuật ở kiến trúc 8.16.

## 1. Phạm vi

Phân biệt người thật với máy trên traffic của khách, cho máy tốt đi qua, chặn hoặc làm chậm máy xấu, và hỏi
lại (challenge) khi không chắc mà không làm hỏng trải nghiệm của người thật. Liên quan: F3.6, F3.8.1, F5.1.5 ở
[§1](../01-functional-decomposition.md).

Khác với WAF: WAF nhìn **nội dung** request (payload có độc không), bot management nhìn **ai gửi**. Một
request hoàn toàn sạch vẫn có thể là bot (đăng nhập bằng mật khẩu bị lộ, mua hết vé, lấy giá).

## 2. Kịch bản theo từng trục

### A. Khách muốn chống cái gì

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-A1 | Lấy dữ liệu (scraping): giá, nội dung, danh bạ | Phân biệt với đối tác lấy dữ liệu hợp lệ |
| KB9-A2 | Thử mật khẩu bị lộ (credential stuffing), dò mật khẩu | Lượng thấp, phân tán, request sạch |
| KB9-A3 | Tạo tài khoản ảo, spam form, spam bình luận | |
| KB9-A4 | Gom hàng, săn vé, flash sale (scalping) | Cần chặn trong vài giây đầu mở bán |
| KB9-A5 | Gửi OTP SMS hàng loạt (SMS pumping) làm khách mất tiền SMS | Phổ biến ở thị trường trong nước |
| KB9-A6 | Gian lận quảng cáo, click ảo, đánh giá ảo | |
| KB9-A7 | Bot gây quá tải (DDoS L7 bằng trình duyệt thật) | Giao với 3.3 |
| KB9-A8 | Giữ chỗ không mua (đặt phòng, đặt bàn, giỏ hàng) | |
| KB9-A9 | Dò lỗ hổng tự động (scanner) | Giao với WAF |

### B. Loại bot

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-B1 | Bot tốt đã xác minh: Google, Bing, Cốc Cốc, Apple | Danh sách bot trong nước có đủ không, ai duy trì |
| KB9-B2 | Bot xem trước link: Zalo, Facebook, Messenger, Slack, Telegram | Chặn thì link chia sẻ mất ảnh và tiêu đề |
| KB9-B3 | Công cụ giám sát uptime, SEO, đo hiệu năng | Khách dùng chính các công cụ này |
| KB9-B4 | Crawler AI (huấn luyện, trả lời câu hỏi) | Vùng xám; khách tự chọn |
| KB9-B5 | Đối tác lấy dữ liệu theo hợp đồng | Allowlist theo gì: IP, header ký, API key |
| KB9-B6 | Bot xấu đơn giản (script, thư viện HTTP) | Dễ nhận ra |
| KB9-B7 | Bot xấu tinh vi: trình duyệt headless, proxy dân cư, giải captcha thuê | Chạy đua với kẻ tấn công |
| KB9-B8 | Tool tự động hoá hợp lệ của chính khách (test, kiểm thử tải) | Khách tự chặn mình |

### C. Tín hiệu nhận diện

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-C1 | IP và ASN (trung tâm dữ liệu, proxy, VPN, Tor) | Proxy dân cư làm tín hiệu IP vô dụng; người dùng thật cũng dùng VPN |
| KB9-C2 | Dấu vân tay TLS, HTTP/2 | Thư viện mới, trình duyệt mới làm dấu vân tay đổi |
| KB9-C3 | Challenge JavaScript, kiểm môi trường trình duyệt | Không chạy được với client không phải trình duyệt |
| KB9-C4 | Hành vi: tốc độ, thứ tự trang, chuột, bàn phím | Thu thập hành vi là dữ liệu cá nhân |
| KB9-C5 | Tín hiệu theo tài khoản (một tài khoản đăng nhập từ nhiều nơi) | Edge không biết tài khoản trừ khi khách cho biết |
| KB9-C6 | Tín hiệu chéo tenant (IP này đang tấn công khách khác) | Dùng dữ liệu tenant này cho tenant khác (giống OQ-51) |

### D. Hành động

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-D1 | Cho qua, ghi log | |
| KB9-D2 | Challenge ẩn (JavaScript, proof of work) | Tốn pin, chậm trên máy yếu |
| KB9-D3 | Challenge tương tác (captcha) | Mất đơn, trợ năng |
| KB9-D4 | Chặn | |
| KB9-D5 | Giới hạn tốc độ theo phiên, làm chậm (tarpit) | |
| KB9-D6 | Trả dữ liệu giả cho bot lấy dữ liệu | Rủi ro pháp lý, khách có muốn không |
| KB9-D7 | Hành động theo path: chặt ở đăng nhập, thanh toán, OTP; lỏng ở trang tin | Cấu hình theo nhóm path |

### E. Trải nghiệm challenge

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-E1 | Request AJAX, fetch trong ứng dụng một trang (SPA) bị challenge | Trang challenge HTML trả về cho một request JSON làm ứng dụng hỏng |
| KB9-E2 | Trình duyệt trong app (Zalo, Facebook, TikTok) | Hay chặn cookie, JavaScript khác thường |
| KB9-E3 | Người dùng tắt cookie, trình duyệt riêng tư, chặn script | Challenge lặp vô hạn |
| KB9-E4 | Mạng di động chậm, máy cấu hình thấp | Challenge mất nhiều giây |
| KB9-E5 | Người khuyết tật, trình đọc màn hình | Captcha hình ảnh không dùng được |
| KB9-E6 | Ngôn ngữ, thương hiệu trên trang challenge | Theo gói |
| KB9-E7 | Trang đã được cache, trang nằm trong iframe | Challenge không hiện được |

### F. Clearance (giấy thông hành sau khi qua challenge)

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-F1 | Thời hạn clearance | Ngắn thì người dùng bị hỏi lại nhiều; dài thì bot dùng lại |
| KB9-F2 | Gắn clearance với IP hoặc dấu vân tay | Người dùng di động đổi IP liên tục |
| KB9-F3 | Dùng chung clearance giữa các subdomain của zone | |
| KB9-F4 | Clearance bị đánh cắp và bán lại (dịch vụ giải challenge) | |

### G. Client không phải trình duyệt

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-G1 | Ứng dụng mobile của khách | Cần SDK hoặc chứng thực thiết bị (attestation) |
| KB9-G2 | Smart TV, thiết bị IoT, máy POS | Không chạy được challenge |
| KB9-G3 | Gọi máy với máy (backend của đối tác, webhook) | Allowlist theo client đã xác định (BR-25) |

### H. Đo lường và tinh chỉnh

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-H1 | Khách muốn biết bao nhiêu % traffic là bot, loại nào | Dashboard bot |
| KB9-H2 | Tỉ lệ challenge, tỉ lệ vượt challenge, người thật bị hỏi lại | Mục tiêu QS-21 |
| KB9-H3 | Bật log-only trước khi chặn | Như WAF |
| KB9-H4 | Khách gắn nhãn "đây là người / đây là bot" | Học từ nhãn |

### I. Pháp lý, quyền riêng tư, thương mại

| Mã | Kịch bản | Concern |
|---|---|---|
| KB9-I1 | Dấu vân tay, dữ liệu hành vi là dữ liệu cá nhân | Nghị định 13/2023; thông báo cho người dùng cuối |
| KB9-I2 | Captcha của bên thứ ba gửi dữ liệu ra nước ngoài | |
| KB9-I3 | Request bị challenge có tính vào usage không | BR-7 nói traffic bị chặn hoặc challenge không tính; request vượt challenge thì tính |
| KB9-I4 | Mức bot theo gói (8.18): Free chặn bot rõ ràng, Pro theo lớp, Business có điểm, Enterprise có mô hình | Khách Pro gặp bot tinh vi |

## 3. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-81 | Captcha tự làm, bên thứ ba, hay không có captcha (chỉ challenge ẩn)? | KB9-D3, KB9-E5, KB9-I2 | |
| OQ-82 | Crawler AI: mặc định cho, chặn, hay bắt khách chọn khi bật? | KB9-B4 | |
| OQ-83 | Nhận diện app mobile thật (SDK, attestation) có trong lộ trình không? | KB9-G1 | |
| OQ-85 | Bot trong nước (Cốc Cốc, bot xem trước link của Zalo) có nằm trong danh sách bot đã xác minh không, ai duy trì? | KB9-B1, KB9-B2 | |
| OQ-100 | Bài toán nào ưu tiên ở MVP: scraping, credential stuffing, flash sale, SMS pumping, spam form? | KB9-A1…KB9-A9 | SMS pumping và credential stuffing hay gặp nhất ở khách trong nước |
| OQ-101 | Request AJAX/JSON bị challenge: trả mã lỗi để ứng dụng tự xử lý, hay cấm challenge trên path API? | KB9-E1 | |
| OQ-102 | Có làm trả dữ liệu giả, làm chậm (tarpit) cho bot không? | KB9-D5, KB9-D6 | |
| OQ-103 | Clearance: thời hạn, gắn với IP hay không, dùng chung giữa subdomain? | KB9-F1…KB9-F4 | |
| OQ-104 | Có thu thập tín hiệu hành vi (chuột, bàn phím) không? Thông báo cho người dùng cuối thế nào? | KB9-C4, KB9-I1 | |
| OQ-105 | Tín hiệu bot chéo tenant có dùng không? | KB9-C6 | Gộp với OQ-51 |
| OQ-106 | Khách gói thấp bị bot tinh vi: có bán lẻ tính năng bot không, hay phải nâng gói? | KB9-I4 | Gắn với KB5-B4 |

## 4. Phụ thuộc

FR-3-08, FR-5-03, BR-7, BR-25 ở [§2](../02-requirements.md); kiến trúc 8.16, ADR-0038; QS-20, QS-21.
