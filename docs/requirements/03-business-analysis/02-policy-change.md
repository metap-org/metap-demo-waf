# 3.2 Viết, duyệt, phát hành một thay đổi policy

> **Trạng thái: đang mở, đã quyết 1/8 câu (2026-11-06).** Trang này liệt kê kịch bản và câu hỏi; quyết định của
> chủ dự án ghi ở [§3](#3-đã-quyết). Cột "ghi chú ban đầu" là ý kiến lúc ghi, không phải quyết định.

## 1. Phạm vi

Bắt đầu khi ai đó muốn đổi cách edge xử lý traffic của một hoặc nhiều zone. Kết thúc khi thay đổi đã chạy
trên edge và được theo dõi, hoặc đã bị huỷ hay rollback. Không gồm: kênh khẩn khi đang bị tấn công (3.3),
exception sinh từ chặn nhầm (3.4), cập nhật ruleset do NCC phát hành (chỉ ghi phần ảnh hưởng tới khách, ở
trục I).

Liên quan: F5 ở [§1](../01-functional-decomposition.md#f5-vòng-đời-policy), FR-5-xx ở
[§2](../02-requirements.md#f5-vòng-đời-policy).

## 2. Kịch bản theo từng trục

### A. Ai thay đổi, qua kênh nào

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-A1 | Quản trị bảo mật sửa trên portal | Kênh chính |
| KB2-A2 | DevOps sửa qua public API hoặc Terraform | Cùng quyền, cùng kiểm tra như UI; drift giữa Terraform và UI |
| KB2-A3 | Policy đồng bộ từ Git (GitOps) | Git là nguồn sự thật thì sửa trên UI bị khoá hay bị ghi đè ở lần đồng bộ sau |
| KB2-A4 | Support của NCC sửa thay khách | Cần quyền khách cấp, audit hiện cho khách, có cần khách duyệt không |
| KB2-A5 | Developer chỉ được sửa trong phạm vi được giao | Phạm vi là zone, path hay loại rule |
| KB2-A6 | Org chỉ có một người | 4-eyes không áp được |

### B. Loại thay đổi

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-B1 | Custom rule, rate limit, access list | Ảnh hưởng trực tiếp traffic |
| KB2-B2 | Cấu hình managed ruleset: paranoia, ngưỡng anomaly, tắt rule | Một con số đổi có thể đổi kết quả hàng nghìn rule |
| KB2-B3 | Exception | Mở rộng quá tay là lỗ hổng; hẹp quá thì không hết chặn nhầm |
| KB2-B4 | Cài đặt zone (TLS, fail mode, origin, cache rule) | Có đi qua cùng change request với rule không; đổi origin sai làm sập website |
| KB2-B5 | Cấu hình bot, API schema | Ảnh hưởng client không chạy được JavaScript (app mobile, đối tác) |
| KB2-B6 | Chỉ đổi tên, mô tả, thứ tự hiển thị | Có cần duyệt không |
| KB2-B7 | Chuyển một rule từ log-only sang chặn | Là thay đổi rủi ro nhất trong thực tế |

### C. Phạm vi

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-C1 | Một zone | Mặc định |
| KB2-C2 | Nhóm zone theo tag | Một change request cho nhiều zone; một zone lỗi biên dịch thì cả nhóm dừng hay phát phần còn lại |
| KB2-C3 | Template ở cấp org, kế thừa xuống zone | Sửa template ảnh hưởng hàng nghìn zone; zone có quyền ghi đè không |
| KB2-C4 | Thay đổi hàng loạt 100.000 zone (Terraform, đổi mặc định) | QS-27; duyệt một lần hay từng zone |

### D. Kiểm tra trước khi phát hành

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-D1 | Kiểm cú pháp và biên dịch thử | Lỗi phải chỉ đúng vị trí trong rule |
| KB2-D2 | Mô phỏng trên traffic lịch sử | Chỉ gói có mẫu request; zone ít traffic không đủ mẫu; mẫu chứa dữ liệu cá nhân |
| KB2-D3 | Thay đổi vượt giới hạn gói (số rule, độ phức tạp) | Báo khi soạn hay khi phát hành |
| KB2-D4 | Rule đúng cú pháp nhưng chặn tất cả (ví dụ điều kiện luôn đúng) | Có cảnh báo "rule này khớp X% traffic" trước khi phát hành không |

### E. Duyệt

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-E1 | 4-eyes bật, người duyệt khác người tạo | Người duyệt vắng mặt, thay đổi kẹt |
| KB2-E2 | Duyệt theo mức rủi ro | Đổi mô tả tự duyệt; chuyển log-only sang chặn cần duyệt |
| KB2-E3 | Khung giờ cho phép thay đổi | Thay đổi khẩn ngoài giờ |
| KB2-E4 | Người duyệt chỉ có quyền trên một phần zone trong change request | Duyệt một phần hay từ chối cả request |
| KB2-E5 | Change request để lâu, bản nháp đã đổi tiếp | Duyệt bản nào; hết hạn duyệt |

### F. Phát hành và theo dõi

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-F1 | Phát hành đi qua canary theo cell | Khách thấy gì trong lúc canary; mất bao lâu |
| KB2-F2 | Gate canary của NCC tự rollback thay đổi của khách | Khách được báo thế nào, lý do có hiểu được không |
| KB2-F3 | Một phần node áp lỗi | Zone chạy hai version cùng lúc |
| KB2-F4 | Sau khi phát hành, tỉ lệ chặn tăng vọt | Có tự cảnh báo khách trong 15 phút đầu không |
| KB2-F5 | Khách muốn phát hành theo tỉ lệ traffic (1% → 10% → 100%) | Có làm không, ở gói nào |

### G. Xung đột

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-G1 | Hai người sửa cùng bản nháp | Khoá, gộp, hay báo xung đột version |
| KB2-G2 | Terraform apply trong lúc có change request đang chờ duyệt trên UI | Cái nào thắng |
| KB2-G3 | Rule tạm của kênh khẩn đang chạy khi có thay đổi thường | Phát hành thường có xoá rule tạm không |

### H. Rollback

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-H1 | Rollback về version trước | Có cần duyệt không |
| KB2-H2 | Rollback về version dùng tính năng đã hết sau khi hạ gói | Từ chối, hay rollback một phần |
| KB2-H3 | Rollback về version tham chiếu ruleset release đã ngừng | Dùng release nào |
| KB2-H4 | Rollback chỉ một rule, không rollback cả version | Có cần không |

### I. Thay đổi do NCC phát hành

| Mã | Kịch bản | Concern |
|---|---|---|
| KB2-I1 | Bản CRS hoặc system ruleset mới đổi hành vi zone dù khách không đổi gì | Tự áp, hay khách chọn khi nào nhận; có giai đoạn log-only cho rule mới không |
| KB2-I2 | Khách Enterprise muốn ghim version ruleset | Ghim được bao lâu; bản vá bảo mật có bắt buộc không |
| KB2-I3 | Rule khẩn của NCC cho CVE mới | Có hiện trong lịch sử thay đổi của zone không |

## 3. Đã quyết

| Ngày | Câu hỏi | Quyết định | Hệ quả cần phân tích tiếp |
|---|---|---|---|
| 2026-11-06 | OQ-35 | **UI là nguồn sự thật.** Public API, Terraform, GitOps vẫn có; khi xung đột, bản trên UI thắng. Terraform và GitOps chỉ báo drift, không ghi đè thay đổi đã làm trên UI | Drift được phát hiện lúc nào (lúc `plan`, lúc đồng bộ định kỳ) và báo cho ai; GitOps thành công cụ đề xuất thay đổi (tạo change request) thay vì áp thẳng; Terraform `apply` gặp bản UI mới hơn thì lỗi hay bỏ qua (KB2-A2, KB2-A3, KB2-G2) |

## 4. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-33 | Mọi thay đổi đều đi qua change request, hay chỉ thay đổi ảnh hưởng traffic? Cài đặt zone (origin, TLS) có đi cùng đường với rule không? | KB2-B4, KB2-B6 | |
| OQ-34 | Org một người: 4-eyes tắt hẳn, hay thay bằng bước tự xác nhận và thời gian chờ? | KB2-A6 | |
| ~~OQ-35~~ | Khi dùng GitOps hoặc Terraform: UI bị khoá chỉ đọc, hay cho sửa và báo drift? | KB2-A2, KB2-A3, KB2-G2 | **Đã quyết**, xem §3 |
| OQ-36 | Cập nhật ruleset của NCC: tự áp cho mọi zone, hay khách chọn thời điểm; rule mới có chạy log-only trước không? | KB2-I1, KB2-I2 | |
| OQ-37 | Có duyệt theo mức rủi ro (thay đổi nhỏ tự duyệt) không? | KB2-E2 | |
| OQ-38 | Thay đổi template ảnh hưởng nhiều zone: duyệt một lần, phát hành theo đợt? | KB2-C2, KB2-C3 | |
| OQ-39 | Khách có thấy và điều khiển được tốc độ phát hành (canary, phần trăm traffic) không? | KB2-F1, KB2-F5 | |
| OQ-40 | Support của NCC có được sửa policy thay khách không, có cần khách duyệt không? | KB2-A4 | |

## 5. Phụ thuộc

FR-5-01…FR-5-19, BR-4, BR-5, BR-6, BR-18, BR-21 ở [§2](../02-requirements.md) viết trước phân tích này;
sẽ đối chiếu lại khi các câu hỏi trên có trả lời.
