# 3.3 Phản ứng khi bị tấn công

> **Trạng thái: đang mở, chưa có quyết định nào (2026-11-06).** Trang này chỉ liệt kê kịch bản và câu hỏi.

## 1. Phạm vi

Từ lúc một zone bắt đầu bị tấn công cho tới khi tấn công dừng, mitigation tạm được gỡ và khách có báo cáo.
Gồm cả tấn công làm ảnh hưởng khách khác hoặc chính nền tảng. Liên quan: F3.3, F5.5, F8 ở
[§1](../01-functional-decomposition.md).

## 2. Kịch bản theo từng trục

### A. Loại tấn công

| Mã | Kịch bản | Concern |
|---|---|---|
| KB3-A1 | Flood HTTP từ nhiều nguồn (botnet) | DDoS L7 tự động; ngưỡng theo baseline của zone |
| KB3-A2 | Flood từ ít nguồn | Pre-filter, rate limit theo IP đủ |
| KB3-A3 | Low-and-slow (Slowloris, giữ kết nối) | Giới hạn kết nối ở L7 |
| KB3-A4 | Tấn công vào endpoint đắt (tìm kiếm, đăng nhập, xuất báo cáo) với lượng nhỏ | Không vượt ngưỡng DDoS nhưng làm sập origin |
| KB3-A5 | Credential stuffing, dò mật khẩu | Bot, ATO (Nâng cao) |
| KB3-A6 | Khai thác CVE mới công bố | Rule khẩn của NCC, virtual patch |
| KB3-A7 | Volumetric L3/L4 | Ngoài edge HTTP; phụ thuộc hạ tầng hoặc đối tác |
| KB3-A8 | Đánh thẳng vào IP origin, bỏ qua edge | WAF không thấy gì; origin chưa khoá (3.1) |
| KB3-A9 | Tấn công vào DNS điều hướng, portal, API của NCC | Ảnh hưởng mọi khách |
| KB3-A10 | Tống tiền: đe doạ DDoS kèm đòi tiền | Khách cần hỗ trợ ngoài kỹ thuật |

### B. Ai phát hiện

| Mã | Kịch bản | Concern |
|---|---|---|
| KB3-B1 | Edge tự phát hiện và giảm thiểu | Khách biết qua cảnh báo; có thể không cần làm gì |
| KB3-B2 | Detection của Analytics cảnh báo | Trễ bao lâu |
| KB3-B3 | Khách thấy origin chậm hoặc chết trước khi hệ thống phát hiện | Khách cần nút "đang bị tấn công" |
| KB3-B4 | NOC thấy PoP hoặc cell quá tải | Tấn công nhắm một zone kéo theo zone khác |

### C. Ai phản ứng

| Mã | Kịch bản | Concern |
|---|---|---|
| KB3-C1 | Tự động hoàn toàn | Rủi ro chặn nhầm khi bật mạnh tay |
| KB3-C2 | Khách bật Under Attack mode, thêm rule tạm | Khách không rành thì không biết làm gì |
| KB3-C3 | NCC (NOC, SOC của NCC) bật thay khách | Có quyền không, theo gói nào, khách có đồng ý trước không |
| KB3-C4 | Tự bật Under Attack khi phát hiện (Enterprise) | Người dùng thật bị challenge |
| KB3-C5 | Tấn công ngoài giờ, không ai trực | Gói thấp không có người trực |

### D. Ảnh hưởng tới khách khác và nền tảng

| Mã | Kịch bản | Concern |
|---|---|---|
| KB3-D1 | Zone bị tấn công lớn được chuyển sang cell cách ly | Zone đó có chậm hơn không |
| KB3-D2 | Zone Free bị tấn công vượt mọi ngưỡng | Chặn toàn bộ traffic zone, rút khỏi edge, hay vẫn giữ |
| KB3-D3 | Nhiều zone cùng bị tấn công (chiến dịch) | Thứ tự hy sinh theo gói (8.18) |
| KB3-D4 | Tấn công làm tăng chi phí băng thông của NCC | Ai chịu |

### E. Chi phí cho khách

| Mã | Kịch bản | Concern |
|---|---|---|
| KB3-E1 | Traffic bị chặn không tính tiền | Đã có (BR-7) |
| KB3-E2 | Tấn công lọt qua (trông như traffic sạch) và bị tính phí vượt | Tranh chấp hoá đơn; có hoàn không |
| KB3-E3 | Gói có trần bị chạm trần vì tấn công | Website bị chuyển tiếp tối giản đúng lúc bị tấn công |

### F. Giao tiếp và kết thúc

| Mã | Kịch bản | Concern |
|---|---|---|
| KB3-F1 | Khách cần biết gì trong lúc tấn công | Lượng traffic, đã chặn bao nhiêu, origin còn sống không |
| KB3-F2 | Tắt Under Attack, gỡ rule tạm khi nào | Tự hết hạn hay khách tắt; tấn công quay lại |
| KB3-F3 | Báo cáo sau sự cố | Theo gói nào, ai viết |
| KB3-F4 | SLA bị vi phạm trong lúc tấn công | Credit |
| KB3-F5 | Cơ quan chức năng yêu cầu log để điều tra | Quy trình pháp lý, dữ liệu nào được giao |

## 3. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-41 | NCC có được bật mitigation thay khách không, ở gói nào, có cần khách đồng ý trước không? | KB3-C3, KB3-C5 | |
| OQ-42 | Zone gói thấp bị tấn công vượt khả năng: chặn toàn bộ, rút khỏi edge (trả DNS về origin), hay giữ và chịu? | KB3-D2 | |
| OQ-43 | Tấn công lọt và bị tính phí vượt: chính sách hoàn tiền thế nào? Trần cứng có tạm nới khi đang bị tấn công không? | KB3-E2, KB3-E3 | |
| OQ-44 | Báo cáo sau sự cố và SLA credit theo gói nào? | KB3-F3, KB3-F4 | |
| OQ-45 | Quy trình khi cơ quan chức năng yêu cầu log | KB3-F5 | Cần tư vấn pháp lý |
| OQ-46 | Có hỗ trợ khách bị tống tiền DDoS không (tư vấn, liên hệ cơ quan chức năng)? | KB3-A10 | |
| OQ-47 | Tấn công vào endpoint đắt mà không vượt ngưỡng DDoS: có sản phẩm cho việc này không (rate limit theo endpoint mặc định, bảo vệ origin theo độ trễ)? | KB3-A4 | |

## 4. Phụ thuộc

FR-3-04, FR-3-05, FR-5-15, FR-8-05…FR-8-07, BR-6, BR-7, BR-10 ở [§2](../02-requirements.md); thứ tự hy sinh và
cell cách ly ở kiến trúc 8.18, ADR-0041.
