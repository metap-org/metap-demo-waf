# 5.6 Mô hình dữ liệu

## Management (logic)

Thiết kế vật lý (bảng, index, tenant, partition): [5.8.14](08-components/14-management-database.md).

```mermaid
erDiagram
    ORG ||--o{ ACCOUNT : has
    ACCOUNT ||--o{ ZONE : owns
    ACCOUNT ||--o{ USER_ROLE : grants
    ACCOUNT ||--|| SUBSCRIPTION : has
    SUBSCRIPTION }o--|| PLAN : "is on"
    PLAN ||--|| ENTITLEMENTS : defines
    ZONE ||--o{ HOSTNAME : serves
    ZONE ||--|| ORIGIN : "forwards to"
    ZONE ||--o{ CERTIFICATE : uses
    ZONE ||--o{ POLICY_VERSION : "versioned as"
    POLICY_VERSION ||--o{ CUSTOM_RULE : contains
    POLICY_VERSION ||--o{ ACCESS_LIST : contains
    POLICY_VERSION ||--o{ RATE_LIMIT_RULE : contains
    POLICY_VERSION ||--|| MANAGED_RULESET_CONFIG : contains
    POLICY_VERSION ||--o{ EXCEPTION : contains
    POLICY_VERSION ||--|| ZONE_SETTINGS : contains
    POLICY_VERSION ||--o{ CACHE_RULE : contains
    ZONE ||--o{ PURGE_REQUEST : requests
    ZONE ||--o{ DNS_RECORD : "has (Mở rộng)"
    POLICY_VERSION ||--|| BOT_POLICY : contains
    POLICY_VERSION ||--o{ API_SCHEMA : contains
    ZONE ||--o{ API_ENDPOINT : inventories
    ZONE ||--o| API_AUTH_CONFIG : configures
    CHANGE_REQUEST }o--|| POLICY_VERSION : proposes
    CHANGE_REQUEST ||--o{ APPROVAL : needs
    ACCOUNT ||--o{ ALERT_POLICY : configures
    ACCOUNT ||--o{ CASE : tracks
    ACCOUNT ||--o{ AUDIT_ENTRY : records
```

| Entity | Ghi chú |
|---|---|
| **PolicyVersion** | Bất biến sau khi duyệt. Một zone có đúng một version đang enforce và có thể có một version đang canary. Rollback là tạo PolicyVersion mới trỏ về nội dung của bản cũ, không sửa lịch sử. |
| **CustomRule** | Lưu **text** làm nguồn sự thật, kèm AST đã parse; có priority, hành động, chế độ log-only, ngày hết hạn tuỳ chọn (cho rule tạm) |
| **ManagedRulesetConfig** | Ruleset release đang theo (cố định hay tự nhận bản mới), paranoia level, ngưỡng anomaly inbound/outbound, danh sách rule ID tắt |
| **Exception** | Phạm vi bắt buộc: rule ID (hoặc nhóm) + điều kiện (path, method, tham số, header); không có "tắt toàn bộ" ẩn |
| **ZoneSettings** | Fail mode, Under Attack mode (bật/tắt, hạn tự tắt), TLS, giới hạn body |
| **Entitlements** | Thuộc gói, không thuộc zone; Control và Analytics thực thi ([ADR-0007](../09-adr/0007-log-by-subscription-entitlements.md)) |
| **CacheRule** | Điều kiện (path, đuôi file, header), TTL, cache key tuỳ biến, bỏ qua cache, miễn WAF cho tài nguyên tĩnh ([ADR-0035](../09-adr/0035-cdn-and-dns-strategy.md)) |
| **PurgeRequest** | Theo URL, prefix, tag (Mở rộng); đi fast lane; trạng thái hoàn tất theo % node |
| **DnsRecord** | Mở rộng, khi khách chuyển DNS authoritative sang NCC: loại, tên, giá trị, TTL, proxied (trả IP edge thay vì IP origin) |
| **BotPolicy** | Hành động theo `bot.class`, `bot.category` và nhóm path; bot đã xác minh bỏ qua những gì; danh sách bot của khách; bật/tắt JS thu tín hiệu ([8.16](../08-cross-cutting/16-bot-management.md)) |
| **ApiEndpoint** | Kho API: `(host, method, path template)`, nguồn (schema / discovery), trạng thái (đã xác nhận, shadow, zombie, không dùng), nhãn nhạy cảm, chế độ kiểm (tắt / log-only / enforce) |
| **ApiSchema** | OpenAPI 3.x khách tải hoặc sinh từ discovery, có version |
| **ApiAuthConfig** | JWT (issuer, audience, URL JWKS), CA client cho mTLS, header API key, nguồn danh tính consumer ([8.17](../08-cross-cutting/17-api-security.md)) |
| **AuditEntry** | Chỉ ghi thêm; gồm diff, người làm, kênh (UI/API/IaC), IP, change request liên quan |

## Mô hình rule

```mermaid
flowchart LR
    builder["Builder UI"] -->|sinh| ast["AST"]
    text["Text biểu thức"] -->|parse| ast
    ast -->|in ra| text
    ast -->|hiển thị| builder
    ast --> val["Validator"] --> comp["Compiler (Control)"] --> edge["Dạng thực thi ở edge"]
```

- Builder và text là hai cách hiển thị một AST; sửa ở đâu cũng được, chuyển qua lại không mất
  thông tin ([ADR-0003](../09-adr/0003-rule-language-existing-syntax-with-builder.md)).
- Text là thứ được lưu, diff, review và dùng trong IaC.
- Field và toán tử là tập đóng, có kiểu (IP, chuỗi, số, danh sách); validator từ chối field không
  biết thay vì bỏ qua.
- Managed ruleset dùng SecLang riêng; khách không viết SecLang, chỉ cấu hình (paranoia, ngưỡng,
  tắt rule, exclusion). System ruleset của NCC viết bằng SecLang.
