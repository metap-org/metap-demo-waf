# 3.11 Vận hành của NCC: threat research, support, sự cố nền tảng, bán hàng

> **Trạng thái: đang mở, chưa có quyết định nào (2026-11-06).** Trang này chỉ liệt kê kịch bản và câu hỏi. Vận
> hành kỹ thuật (runbook, giám sát) ở kiến trúc 8.12, 8.13.

## 1. Phạm vi

Các nghiệp vụ phía NCC mà khách chạm tới: phát hành bảo vệ mới, hỗ trợ khách, xử lý sự cố của chính nền
tảng, bán và triển khai cho Enterprise, làm việc với đối tác hạ tầng. Liên quan: F11, F12 ở
[§1](../01-functional-decomposition.md).

## 2. Kịch bản theo từng trục

### A. Threat research

| Mã | Kịch bản | Concern |
|---|---|---|
| KB11-A1 | CVE nghiêm trọng mới công bố, đang bị khai thác | Từ lúc biết tới lúc có rule khẩn: bao lâu, ai duyệt |
| KB11-A2 | Rule khẩn chặn nhầm | Rollback, thông báo (3.4) |
| KB11-A3 | Khách hỏi "tôi có được bảo vệ khỏi CVE X không" | Trang công bố theo CVE |
| KB11-A4 | Nguồn threat intel (feed thương mại, CERT trong nước) | Chi phí, điều khoản |

### B. Support

| Mã | Kịch bản | Concern |
|---|---|---|
| KB11-B1 | Kênh: email, chat, điện thoại, Zalo | Theo gói |
| KB11-B2 | Giờ hỗ trợ: giờ hành chính hay 24/7 | Theo gói |
| KB11-B3 | Thời gian phản hồi theo mức nghiêm trọng | SLA support |
| KB11-B4 | Support cần xem dữ liệu của khách | BR-20 |
| KB11-B5 | Tài liệu tự phục vụ, tiếng Việt | |

### C. Sự cố của nền tảng

| Mã | Kịch bản | Concern |
|---|---|---|
| KB11-C1 | Một PoP hoặc region lỗi | Khách có được báo không |
| KB11-C2 | Lỗi do NCC làm sập traffic của khách | Thông báo, báo cáo sự cố, credit |
| KB11-C3 | Trang trạng thái công khai | Có không, mức chi tiết |
| KB11-C4 | Bảo trì có kế hoạch | Báo trước bao lâu |

### D. Bán hàng và Enterprise

| Mã | Kịch bản | Concern |
|---|---|---|
| KB11-D1 | Dùng thử có hỗ trợ (POC) cho Enterprise | Bao lâu, miễn phí không, tiêu chí thành công |
| KB11-D2 | Hợp đồng có điều khoản riêng (SLA cao hơn, phạt) | Ảnh hưởng thiết kế (cell riêng, database riêng) |
| KB11-D3 | Hybrid, on-prem | Ai vận hành, ai cập nhật |

### E. Đối tác hạ tầng

| Mã | Kịch bản | Concern |
|---|---|---|
| KB11-E1 | PoP chạy trên hạ tầng của đối tác | SLA giữa hai bên, ai xử lý sự cố |
| KB11-E2 | Đối tác chống DDoS L3/L4 | Phạm vi, chi phí |
| KB11-E3 | Đối tác bán lại dịch vụ | Xem 3.5 |

## 3. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-89 | Kênh, giờ và thời gian phản hồi support theo gói | KB11-B1…KB11-B3 | |
| OQ-90 | Có trang trạng thái công khai không? Báo cáo sự cố nền tảng gửi cho ai? | KB11-C1…KB11-C3 | |
| OQ-91 | Mục tiêu thời gian từ CVE tới rule khẩn; có trang công bố theo CVE không? | KB11-A1, KB11-A3 | |
| OQ-92 | POC cho Enterprise: thời gian, chi phí, ai làm | KB11-D1 | |
| OQ-93 | On-prem: NCC vận hành hay khách tự vận hành? | KB11-D3 | |

## 4. Phụ thuộc

FR-11-01…FR-11-03, FR-12-01…FR-12-05 ở [§2](../02-requirements.md); runbook ở kiến trúc 8.13.
