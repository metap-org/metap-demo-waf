# 4. Solution Strategy

Các quyết định nền tảng, mỗi quyết định trả lời cho một mục tiêu chất lượng ở
[1.3](../01-introduction/00-index.md#13-mục-tiêu-chất-lượng). Chi tiết và phương án đã cân nhắc nằm
trong ADR.

| Mục tiêu | Cách tiếp cận | ADR |
|---|---|---|
| Q1, Q5 | **Bốn plane với chu kỳ deploy và yêu cầu riêng.** Management (nghiệp vụ), Control (biến ý định thành config chạy được), Edge (traffic thật), Analytics (log và phân tích). Edge không phụ thuộc runtime vào plane nào. | [0001](../09-adr/0001-four-planes.md) |
| Q1, Q2 | **Compile tập trung, edge chỉ nhận snapshot bất biến có chữ ký.** Edge không hiểu khái niệm nghiệp vụ, không đọc database, không quyết định gì ngoài so khớp request với config đã biên dịch. | [0002](../09-adr/0002-central-compile-signed-snapshots.md) |
| Q2 | **Policy là artifact có version, đi qua vòng đời** nháp → kiểm tra → mô phỏng → duyệt → version → canary → enforce → theo dõi → rollback. Đường khẩn bỏ qua đợt nhưng vẫn có version. | [0002](../09-adr/0002-central-compile-signed-snapshots.md) |
| Q2 | **Rule cho khách dùng cú pháp có sẵn, builder và text là hai cách hiển thị một AST.** | [0003](../09-adr/0003-rule-language-existing-syntax-with-builder.md) |
| Q2, thời gian ra thị trường | **Managed ruleset dựa trên CRS + engine SecLang, cộng system ruleset riêng.** Không tự viết lại bộ rule nền. | [0004](../09-adr/0004-managed-ruleset-crs-on-seclang.md) |
| Q4 | **Một luồng ghi, nhiều store đọc.** Log vào một bus bền; hot search, columnar, archive, detection, export mỗi thứ đọc riêng. Mọi bản ghi mang request ID, rule ID và policy version. | [0010](../09-adr/0010-analytics-one-write-path-many-stores.md) |
| Q3 | **Cô lập theo tầng:** quota tài nguyên theo tenant ở edge, phân vùng theo tenant ở bus và store, Query API là cửa duy nhất đọc dữ liệu traffic. | [08](../08-cross-cutting/00-index.md#81-multi-tenancy-và-cô-lập) |
| Thương mại | **Gói cước là cấu hình (entitlements), không phải code.** Management phát entitlements; Control và Analytics thực thi. | [0007](../09-adr/0007-log-by-subscription-entitlements.md) |
| Q5 | **Một edge artifact chạy mọi nơi; ba mô hình triển khai** SaaS, hybrid, on-prem dùng cùng các plane. | [0005](../09-adr/0005-edge-on-own-and-existing-infra.md), [0006](../09-adr/0006-saas-first-on-prem-capable.md) |
| Thời gian ra thị trường | **`metap` cho Management plane**, tự xây phần còn lại. | [0009](../09-adr/0009-metap-for-management-plane-only.md) |
| Ứng phó tấn công | **Under Attack mode** là một công tắc theo zone, đi đường khẩn. | [0008](../09-adr/0008-under-attack-mode.md) |
| Q1, Q3 | **Edge hai tầng:** L7 (RAM, kết nối) lọc rẻ, challenge, cache; WAF inspection (CPU) chỉ nhận phần còn lại. Scale độc lập, crash của engine không kéo theo kết nối. | [0033](../09-adr/0033-two-tier-edge-proxy-and-waf.md), [0034](../09-adr/0034-pre-filter-and-rule-split-across-tiers.md) |
| Thương mại, Q1 | **CDN là một phần của sản phẩm** (đề xuất, chờ chọn): cache trong tầng L7 từ MVP, chế độ đứng sau CDN của khách, DNS authoritative ở giai đoạn Mở rộng. | [0035](../09-adr/0035-cdn-and-dns-strategy.md) |
| Q1 | **Rate limit toàn fleet là xấp xỉ**: đếm cục bộ, gộp định kỳ. | [0011](../09-adr/0011-approximate-fleet-rate-limiting.md) |

## Thứ tự xây dựng

Ba giai đoạn theo mức nghiệp vụ ở [1.2](../01-introduction/00-index.md#12-tổng-quan-yêu-cầu):

1. **MVP bán được:** edge hai tầng với đủ pipeline trừ bot/API schema; cache cơ bản, DNS điều hướng,
   chế độ đứng sau CDN của khách; CRS + system ruleset; Under Attack
   mode; cert tự động; snapshot có version, ack, rollback; access log, tra cứu, dashboard, SIEM
   export; cảnh báo; xử lý chặn nhầm; SSO, RBAC, audit, entitlements, metering. Chạy SaaS.
2. **Cạnh tranh:** bot, API security, mô phỏng lịch sử, canary có gate tự động, duyệt 4-eyes,
   Terraform/GitOps, case management, báo cáo tuân thủ, reputation, virtual patching, L3/L4 qua
   đối tác, hybrid deployment, tiered cache / origin shield, DNS authoritative cho khách.
3. **Khác biệt:** phát hiện bất thường, chống chiếm tài khoản, client-side protection, tín hiệu
   chéo tenant, data residency mở rộng, on-prem đầy đủ.
