# 12. Glossary

| Thuật ngữ | Nghĩa |
|---|---|
| **WAAP** | Web Application & API Protection: WAF, DDoS L7, bot, API security gộp thành một sản phẩm |
| **Management plane** | Plane nghiệp vụ: portal, API, tenant, policy, case, billing. Thư mục `data-plane/` của repo demo |
| **Control plane** | Plane biến ý định thành config edge chạy được và phân phối nó |
| **Edge plane** | Đường đi của traffic thật; theo nghĩa chuẩn ngành là *data plane* |
| **Analytics plane** | Plane nhận, lưu, truy vấn, phân tích log |
| **PoP** | Point of Presence: một điểm đặt edge node |
| **Edge proxy** | Tầng 1 của edge: kết nối, TLS, pre-filter, challenge, cache, proxy tới origin |
| **WAF inspection** | Tầng 2 của edge: dịch vụ kiểm tra nội dung (CRS, rule cần body), trả verdict |
| **Pre-filter** | Các bước lọc rẻ ở proxy trước khi gọi WAF: access list, challenge, rate limit, custom rule phần pre |
| **Inspection API** | Hợp đồng gRPC giữa edge proxy và WAF inspection |
| **Origin shield / tiered cache** | Một lớp cache trung gian giữa PoP và origin, giảm số request về origin |
| **Purge** | Xoá nội dung khỏi cache theo URL, prefix hoặc tag |
| **DNS điều hướng** | DNS của NCC trả IP của PoP gần và khoẻ cho hostname edge |
| **DNS authoritative** | Khi khách chuyển quản lý DNS của domain sang NCC |
| **Chế độ đứng sau CDN** | WAAP nhận traffic đã qua CDN của khách, lấy IP client từ header tin cậy |
| **Zone** | Một site/domain khách bảo vệ, đơn vị cấu hình chính |
| **PolicyVersion** | Bản policy bất biến của một zone, sinh ra khi được duyệt |
| **Snapshot / bundle** | Bản config đã biên dịch, có chữ ký, edge nạp nguyên khối |
| **Lớp áp dụng A/B/C/D** | Đổi nóng / build nền rồi đổi nóng / reload process / nâng phần mềm ([8.15](../08-cross-cutting/15-config-propagation-and-apply.md)) |
| **Node profile** | Cấu hình của node hoặc PoP (cổng, worker, RAM, cache đĩa), rollout như phần mềm |
| **Lớp rule khẩn** | Bộ rule nhỏ của NCC chạy trước CRS, đi fast lane không cần build lại CRS |
| **Fast lane** | Đường phân phối khẩn bỏ qua các đợt rollout |
| **Canary** | Đợt rollout đầu tiên trên một nhóm node nhỏ hoặc ở chế độ log-only |
| **Gate** | Điều kiện tự động để một đợt rollout đi tiếp |
| **Last-good** | Snapshot gần nhất đã nạp thành công, giữ để quay về |
| **Log-only / monitor** | Chế độ đánh giá và ghi event nhưng không thực thi hành động |
| **Managed ruleset** | Bộ rule do NCC duy trì: OWASP CRS + system ruleset |
| **System ruleset** | Bộ rule SecLang riêng của NCC |
| **CRS** | OWASP Core Rule Set |
| **SecLang** | Ngôn ngữ rule của ModSecurity; Coraza và một số engine thuần Rust cũng hỗ trợ |
| **IR (custom rule)** | Dạng trung gian Control biên dịch từ CEL, edge thực thi |
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
| **status_source** | Ai sinh ra status code của một response: origin, cache, edge_policy, edge_request, client_abort, origin_unreachable, edge_platform |
| **edge_error** | Mã chi tiết khi WAAP tự sinh response thay cho origin (ví dụ `origin_connect_timeout`) |
| **edge_us** | Thời gian do WAAP gây ra cho một request, không gồm chờ origin và client |
| **RUM** | Real user monitoring: đo trải nghiệm thật trên trình duyệt người dùng |
| **Request ID** | Định danh duy nhất của request, in trên trang chặn, khoá tra cứu và bỏ trùng |
| **Replay** | Chạy policy nháp trên traffic lịch sử để mô phỏng tác động |
| **SIEM export** | Đẩy log liên tục sang hệ thống SIEM của khách |
| **Outbox** | Ghi sự kiện cùng transaction với dữ liệu nghiệp vụ, phát sau; không mất sự kiện |
| **Hybrid** | Management/Control của NCC, edge (và có thể Analytics) trong hạ tầng khách |
