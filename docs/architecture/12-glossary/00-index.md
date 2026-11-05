# 12. Glossary

| Thuật ngữ | Nghĩa |
|---|---|
| **WAAP** | Web Application & API Protection: WAF, DDoS L7, bot, API security gộp thành một sản phẩm |
| **Management plane** | Plane nghiệp vụ: portal, API, tenant, policy, case, billing. Thư mục `data-plane/` của repo demo |
| **Control plane** | Plane biến ý định thành config edge chạy được và phân phối nó |
| **Edge plane** | Đường đi của traffic thật; theo nghĩa chuẩn ngành là *data plane* |
| **Analytics plane** | Plane nhận, lưu, truy vấn, phân tích log |
| **PoP** | Point of Presence: một điểm đặt edge node |
| **Zone** | Một site/domain khách bảo vệ, đơn vị cấu hình chính |
| **PolicyVersion** | Bản policy bất biến của một zone, sinh ra khi được duyệt |
| **Snapshot / bundle** | Bản config đã biên dịch, có chữ ký, edge nạp nguyên khối |
| **Fast lane** | Đường phân phối khẩn bỏ qua các đợt rollout |
| **Canary** | Đợt rollout đầu tiên trên một nhóm node nhỏ hoặc ở chế độ log-only |
| **Gate** | Điều kiện tự động để một đợt rollout đi tiếp |
| **Last-good** | Snapshot gần nhất đã nạp thành công, giữ để quay về |
| **Log-only / monitor** | Chế độ đánh giá và ghi event nhưng không thực thi hành động |
| **Managed ruleset** | Bộ rule do NCC duy trì: OWASP CRS + system ruleset |
| **System ruleset** | Bộ rule SecLang riêng của NCC |
| **CRS** | OWASP Core Rule Set |
| **SecLang** | Ngôn ngữ rule của ModSecurity, Coraza cũng hỗ trợ |
| **Paranoia level** | Mức độ chặt của CRS (1–4); càng cao càng bắt nhiều, càng dễ chặn nhầm |
| **Anomaly scoring** | CRS cộng điểm từ nhiều rule rồi so với ngưỡng thay vì chặn ngay ở rule đầu tiên |
| **Exception / exclusion** | Bỏ qua một rule cho một phạm vi cụ thể (path, tham số, header) |
| **Custom rule** | Rule khách viết bằng biểu thức, qua builder hoặc text |
| **AST** | Cây cú pháp của biểu thức rule; dạng chung của builder và text |
| **Under Attack mode (UAM)** | Chế độ challenge mọi request chưa có clearance token |
| **Clearance token** | Token có chữ ký cấp sau khi qua challenge, kiểm được ở mọi node |
| **Fail-open / fail-closed** | Cho qua hoặc chặn khi một bước kiểm tra lỗi hoặc quá hạn |
| **False positive (FP)** | Request hợp lệ bị chặn nhầm |
| **Entitlements** | Giới hạn và tính năng theo gói cước, dạng cấu hình |
| **Request ID** | Định danh duy nhất của request, in trên trang chặn, khoá tra cứu và bỏ trùng |
| **Replay** | Chạy policy nháp trên traffic lịch sử để mô phỏng tác động |
| **SIEM export** | Đẩy log liên tục sang hệ thống SIEM của khách |
| **Outbox** | Ghi sự kiện cùng transaction với dữ liệu nghiệp vụ, phát sau; không mất sự kiện |
| **Hybrid** | Management/Control của NCC, edge (và có thể Analytics) trong hạ tầng khách |
