# 1. Introduction and Goals

## 1.1 Sản phẩm làm gì

Đứng trước website và API của khách hàng doanh nghiệp, lọc traffic độc hại trước khi tới origin,
và cho đội bảo mật của khách công cụ để viết rule, điều tra, xử lý chặn nhầm, chứng minh hiệu quả.
Bán dạng SaaS; kiến trúc phải cho phép cài on-prem và chạy lai (xem
[ADR-0006](../09-adr/0006-saas-first-on-prem-capable.md)).

## 1.2 Tổng quan yêu cầu

Sáu nhóm nghiệp vụ. Mức **MVP** là mức tối thiểu để ký hợp đồng doanh nghiệp đầu tiên, không phải
mức demo. **Mở rộng** để ngang đối thủ trong vòng đánh giá, **Nâng cao** để khác biệt.

| Nhóm | MVP | Mở rộng | Nâng cao |
|---|---|---|---|
| Bảo vệ runtime | Managed ruleset (CRS + system ruleset), custom rule, rate limit toàn fleet, access list IP/geo/ASN, DDoS L7, Under Attack mode, TLS + cert tự động, bảo vệ origin | DDoS L3/L4 (qua hạ tầng/đối tác), bot management (điểm bot, bot đã xác minh, crawler AI, challenge; [8.16](../08-cross-cutting/16-bot-management.md)), API security (discovery, schema, JWT/mTLS, rate limit theo consumer, GraphQL; [8.17](../08-cross-cutting/17-api-security.md)) | Chống chiếm tài khoản, mô hình bot học máy, phát hiện BOLA, dữ liệu nhạy cảm trong response, client-side protection |
| Vòng đời policy | Builder + text, kiểm tra trước khi lưu, log-only theo rule/zone, exception có phạm vi, version, diff, rollback một bấm | Mô phỏng trên traffic lịch sử, canary, duyệt 4-eyes, policy-as-code (API/Terraform/GitOps), kế thừa org → nhóm zone → zone | |
| Quan sát & vận hành bảo mật | Security event realtime, access log theo gói, tra cứu theo request ID, dashboard (traffic, bảo mật, hiệu quả rule), đánh dấu chặn nhầm + tỉ lệ FP theo rule, cảnh báo đa kênh, SIEM export | Case/sự cố, báo cáo định kỳ và tuân thủ | Phát hiện bất thường theo baseline |
| Threat intelligence | Cập nhật ruleset có version, rule khẩn toàn fleet | IP reputation, virtual patching theo CVE | Tín hiệu chéo tenant (ẩn danh) |
| Tenant & thương mại | Onboarding (DNS, xác minh domain, cert), org/account/zone, RBAC + SSO, audit log, gói/quota/metering/billing, public API | SCIM, báo cáo SLA | Data residency cho log |
| Hiệu năng & phân phối (CDN, DNS) | Cache cơ bản (Cache-Control, cache rule, purge theo URL/prefix, stale-if-error), DNS điều hướng, chế độ đứng sau CDN của khách | Tiered cache / origin shield, purge theo tag, DNS authoritative cho khách (proxied record, DNSSEC), load balancing và origin failover | Tối ưu ảnh, edge compute, video |
| Vận hành nhà cung cấp | Quản lý fleet, giám sát lan truyền config, failover | Chống noisy neighbor | |

CDN không phải phần phụ: khi traffic đã đi qua edge vì hiệu năng, WAF trở thành phần cộng thêm gần như
không tốn độ trễ, và băng thông tiết kiệm ở origin bù chi phí dịch vụ ([ADR-0035](../09-adr/0035-cdn-and-dns-strategy.md)).

Vulnerability scanning (DAST) không phải lõi WAAP; nếu làm, là sản phẩm riêng dùng chung tenant.

Bảng trên phân rã chi tiết thành chức năng lá ở [1.5 Bản đồ phân rã chức năng](05-functional-decomposition.md),
và thành requirement có tiêu chí chấp nhận ở [1.6 Requirements](06-requirements.md) (cả hai đang chờ review).
Từng nghiệp vụ được phân tích theo trình tự khách làm ở [1.7 Phân tích nghiệp vụ](07-business-analysis/00-index.md),
bắt đầu từ [onboarding một website](07-business-analysis/01-onboarding.md).

## 1.3 Mục tiêu chất lượng

Sáu mục tiêu xếp theo thứ tự ưu tiên. Khi hai mục xung đột, mục đứng trên thắng. Kịch bản đo được
ở [10. Quality Requirements](../10-quality/00-index.md).

| # | Mục tiêu | Ý nghĩa |
|---|---|---|
| Q1 | **Edge nhanh và luôn sống** | Thêm ít độ trễ cho mỗi request; tiếp tục phục vụ khi mọi plane khác sập |
| Q2 | **Thay đổi an toàn** | Không thay đổi config nào, của khách hay của nhà cung cấp, gây outage toàn fleet |
| Q3 | **Cô lập tenant** | Một tenant không đọc được dữ liệu, không làm chậm, không làm sập tenant khác |
| Q4 | **Bằng chứng đầy đủ** | Log và event không mất; mọi quyết định chặn truy ngược được tới rule và version |
| Q5 | **Triển khai được ở nhiều nơi** | Cùng một kiến trúc chạy trên PoP riêng, hạ tầng có sẵn, và on-prem |
| Q6 | **Mở rộng tới 1 triệu zone với chi phí hợp lý** | Tài nguyên mỗi node phụ thuộc vào traffic đang hoạt động, không phụ thuộc tổng số zone; chi phí trên request giảm khi quy mô tăng |

## 1.4 Các bên liên quan

| Vai trò | Phía | Kỳ vọng |
|---|---|---|
| Quản trị bảo mật | Khách | Viết và duyệt policy, kiểm soát thay đổi, báo cáo cho lãnh đạo |
| SOC analyst | Khách | Thấy tấn công gần realtime, điều tra nhanh, xử lý chặn nhầm không cần gọi support |
| DevOps / platform | Khách | Quản lý bằng API và IaC, không phải bấm UI; tích hợp CI/CD |
| Chủ tài khoản / tài chính | Khách | Gói rõ ràng, hoá đơn khớp với dùng thật |
| Kiểm toán / tuân thủ | Khách | Audit log, retention đúng cam kết, báo cáo PCI DSS / SOC 2 |
| Product / sales | NCC | Lộ trình tính năng theo mức MVP → Nâng cao, demo được cho khách |
| NOC / SRE | NCC | Biết node nào chạy version nào, rollback được trong vài phút |
| Threat research | NCC | Phát hành system ruleset và rule khẩn an toàn |
| Support | NCC | Tra request ID của khách, không cần quyền đọc toàn bộ dữ liệu tenant |
| Đối tác hạ tầng | Ngoài | Chạy edge node trên hạ tầng của họ theo hợp đồng ([ADR-0005](../09-adr/0005-edge-on-own-and-existing-infra.md)) |
