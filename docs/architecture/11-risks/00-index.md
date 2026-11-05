# 11. Risks and Technical Debt

## 11.1 Rủi ro

| # | Rủi ro | Khả năng | Tác động | Giảm thiểu | Dấu hiệu cần hành động |
|---|---|---|---|---|---|
| R1 | Engine SecLang + CRS vượt ngân sách độ trễ QS-1 | Trung bình | Cao | Benchmark ngay đầu giai đoạn MVP; giới hạn kiểm tra body theo entitlements; paranoia mặc định 1 | p99 > 2 ms trên traffic mẫu |
| R2 | Chặn nhầm làm mất niềm tin khách | Cao | Cao | Log-only 7 ngày khi onboarding; replay trước khi enforce; xử lý chặn nhầm tự phục vụ; tỉ lệ FP theo rule | Tỉ lệ FP của một rule > ngưỡng; ticket support về chặn nhầm tăng |
| R3 | Chi phí log vượt doanh thu | Trung bình | Cao | Entitlements, lấy mẫu access log, tách store theo chi phí | Chi phí log / doanh thu theo tenant > ngưỡng |
| R4 | Config hỏng ra toàn fleet | Thấp | Rất cao | Compiler kiểm tra, chữ ký, canary có gate, rollback tức thì | Lỗi nạp snapshot ở canary |
| R5 | Node trên hạ tầng đối tác bị chiếm | Thấp | Cao | Danh tính node, mTLS, thu hồi node, key cert chỉ trong RAM, keyless sau này | Hành vi bất thường từ một node |
| R6 | Phụ thuộc đối tác cho L3/L4 | Trung bình | Trung bình | Nhiều đối tác; hợp đồng có SLA | Đối tác không đáp ứng tấn công lớn |
| R7 | On-prem làm phức tạp mọi quyết định công nghệ | Trung bình | Trung bình | Ràng buộc "không dịch vụ độc quyền một cloud" từ đầu; chưa xây on-prem tới giai đoạn 3 | Khách lớn đòi on-prem sớm |
| R8 | Rate limit xấp xỉ không đạt kỳ vọng khách | Trung bình | Thấp | Ghi rõ trong tài liệu; chu kỳ gộp ngắn ở PoP lớn | Khiếu nại ngưỡng nhỏ không chính xác |
| R9 | Đội nhỏ dàn trải 4 plane | Cao | Cao | Mua/dùng mã nguồn mở cho phần không khác biệt; làm đúng thứ tự giai đoạn | Trễ MVP |

## 11.2 Chênh lệch với code demo hiện tại

Code trong repo là demo, không phải điểm xuất phát bắt buộc. Những chỗ khác kiến trúc đích:

| Chủ đề | Demo | Kiến trúc đích |
|---|---|---|
| Plane | 3 plane; log nằm trong database của portal | 4 plane, Analytics riêng |
| Snapshot | Mỗi zone một key trong kho key-value, không chữ ký, không ack | Bundle bất biến có chữ ký, ack theo version, rollout theo đợt |
| Managed ruleset | Không có | CRS + system ruleset trên engine SecLang |
| Ngôn ngữ rule | JSON grammar riêng | Cú pháp có sẵn, AST, builder ↔ text |
| Rate limit | Đếm cục bộ từng node | Gộp định kỳ toàn fleet |
| Log | Chỉ security event, buffer trong RAM, ghi vào Postgres của portal | Access log + event, buffer đĩa, bus bền, store riêng |
| Vòng đời policy | Monitor mode, `configVersion` chỉ là số đếm | PolicyVersion bất biến, mô phỏng, duyệt, canary, rollback |
| Fleet | Một process, không danh tính node | Registry, mTLS, health, độ phủ version |
| Cert, Under Attack, SSO, entitlements, SIEM export | Không có | MVP |

Khuyến nghị: giữ Management (portal trên `metap`) làm nền và tiến hoá dần; viết lại Control và
Edge theo hợp đồng ở [05-contracts](../05-building-blocks/05-contracts.md); xây mới Analytics.

## 11.3 Quyết định còn mở

| Chủ đề | Cần quyết | Ảnh hưởng |
|---|---|---|
| Biến thể cú pháp rule | Chọn cụ thể biến thể và tập field ([ADR-0003](../09-adr/0003-rule-language-existing-syntax-with-builder.md)) | Parser, builder, tài liệu |
| Coraza hay ModSecurity | Theo công nghệ của edge gateway ([ADR-0004](../09-adr/0004-managed-ruleset-crs-on-seclang.md)) | Hiệu năng, cách nhúng |
| Công nghệ từng plane | Edge gateway, bus, các store, config store | Toàn bộ [10.3](../10-quality/00-index.md#103-quy-mô-thiết-kế) |
| Tự bật Under Attack mode theo detection | Có cho phép, mặc định tắt hay bật | [ADR-0008](../09-adr/0008-under-attack-mode.md) |
| Bảng entitlements cụ thể | Giá trị từng gói | [ADR-0007](../09-adr/0007-log-by-subscription-entitlements.md), mô hình giá |
| Cấp phép và cập nhật on-prem | Cách phát hành, cách cập nhật ruleset cho khách offline | [ADR-0006](../09-adr/0006-saas-first-on-prem-capable.md) |
