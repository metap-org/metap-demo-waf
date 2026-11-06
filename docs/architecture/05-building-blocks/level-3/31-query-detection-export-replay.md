# C3 · Query API, detection & alerting, export, replay (Analytics plane)

## Component diagram

```mermaid
flowchart LR
    mgmt["Management"]
    rmq[("RabbitMQ")]
    tenr[("Kafka · enriched")]
    ch[("ClickHouse")]
    s3[("Parquet · samples")]
    cmp["Control · compiler"]

    subgraph QA["query-api (Go)"]
        authz["AuthZ<br/>tenant · zone · vai trò"]
        dsl["Query DSL → SQL"]
        cost["Cost guard"]
    end
    subgraph DET["detection (Go)"]
        sd["Streaming detector<br/>cửa sổ trượt"]
        pd["Scheduled detector<br/>truy vấn MV mỗi phút"]
        ad["Anomaly detector<br/>baseline (Nâng cao)"]
        ae["Alert engine<br/>trạng thái · gộp · im lặng"]
        nt["Notifier"]
    end
    subgraph EXP["exporter (Go)"]
        rt["Router<br/>tenant → đích"]
        dq["Hàng đợi theo đích"]
        fmt["Formatter<br/>JSON · CEF · LEEF"]
        snk["Sink<br/>HTTP · syslog TLS · S3"]
    end
    subgraph RP["replay (Go workers)"]
        job["Job runner"]
        pipe["package pipeline"]
        cmpr["So sánh quyết định"]
    end
    pgA[("PostgreSQL · analytics")]
    dests["SIEM của khách"]
    chan["Email · chat · on-call · webhook"]

    mgmt --> authz --> dsl --> cost --> ch
    tenr --> sd --> ae
    ch --> pd --> ae
    ch --> ad --> ae
    ae --> pgA
    ae --> nt --> chan
    ae -->|AlertRaised · CaseProposed| rmq --> mgmt
    tenr --> rt --> dq --> fmt --> snk --> dests
    rt --> pgA
    mgmt -->|yêu cầu replay| job
    cmp -->|snapshot nháp| job
    s3 --> job --> pipe --> cmpr --> pgA
    cmpr --> mgmt
```

## Query API

| Component | Thiết kế |
|---|---|
| **AuthZ** | Nhận danh tính đã xác thực từ Management (token dịch vụ + claim người dùng); xác định tập zone được đọc theo RBAC/ABAC |
| **Query DSL → SQL** | Truy vấn có cấu trúc (bộ lọc theo trường cho phép, khoảng thời gian, group by, top-N), không nhận SQL tự do; builder luôn chèn `tenant_id = ? AND zone_id IN (?)` và tham số hoá mọi giá trị |
| **Cost guard** | Giới hạn khoảng thời gian theo entitlements; setting ClickHouse mỗi truy vấn: `max_execution_time`, `max_rows_to_read`, `max_memory_usage`; quota theo tenant |
| **Lớp phòng thủ thứ hai** | User ClickHouse chỉ đọc cho query-api, row policy theo tenant set qua setting phiên |

Loại truy vấn: tra request ID, danh sách event có lọc, chuỗi thời gian (từ MV), top-N (IP, path,
rule, ASN, quốc gia), hiệu quả rule theo policy version, số liệu sử dụng.

## Detection & alerting

| Component | Thiết kế |
|---|---|
| **Streaming detector** | Consumer `telemetry.enriched.v1`; với alert policy loại "nhanh" (ví dụ > N block trong 1 phút từ một ASN), giữ counter cửa sổ trượt theo khoá trong RAM, phân mảnh theo partition |
| **Scheduled detector** | Mỗi phút chạy truy vấn của alert policy trên MV (tỉ lệ 5xx, tỉ lệ chặn, traffic tăng vọt) |
| **Anomaly detector** | Nâng cao: baseline theo zone, giờ trong tuần (EWMA + mùa vụ), lưu trong ClickHouse; vượt dải → tín hiệu cho alert engine |
| **Alert engine** | Trạng thái alert (`firing`, `resolved`) trong PostgreSQL; gộp theo khoá; giờ im lặng; đề xuất mitigation (chặn ASN/IP có hạn, bật Under Attack mode) gửi Management dạng `CaseProposed` |
| **Notifier** | Kênh: email (SMTP relay), chat (Slack, Teams, Telegram), on-call (PagerDuty, Opsgenie), webhook ký HMAC; retry có backoff, bỏ trùng theo khoá alert |

## Exporter

| Component | Thiết kế |
|---|---|
| **Router** | Cấu hình đích lấy từ Management (qua sự kiện); mỗi bản ghi enriched được định tuyến tới các đích của tenant đó |
| **Hàng đợi theo đích** | Mỗi đích có hàng đợi và offset riêng (lưu PostgreSQL), nên đích chậm không làm chậm đích khác; đích tụt hậu quá retention Kafka → đánh dấu mất dữ liệu, báo khách |
| **Formatter** | JSON (mặc định), CEF, LEEF; chọn trường xuất |
| **Sink** | HTTP (Splunk HEC, Elastic/OpenSearch bulk, Datadog, endpoint tuỳ ý), syslog qua TLS, ghi object storage của khách |
| **Bảo mật** | Đích kiểm qua SSRF guard (cùng chính sách với `metap-cron` webhook); credential đích lưu qua `SecretStore` |

## Replay

| Component | Thiết kế |
|---|---|
| **Job runner** | Nhận yêu cầu (zone, snapshot nháp, khoảng thời gian, cỡ mẫu); lấy snapshot nháp do compiler sinh (chưa ký, không rollout) |
| **package pipeline** | Cùng code với edge ([ADR-0026](../../09-adr/0026-replay-on-request-samples-with-shared-pipeline.md)); không có I/O mạng; rate limit được mô phỏng theo thời gian trong mẫu |
| **So sánh** | Với mỗi mẫu: quyết định gốc vs quyết định mới; kết quả: số request thêm bị chặn, bớt bị chặn, theo rule, kèm ví dụ; lưu PostgreSQL, trả Management |
| **Quy mô** | Mặc định 7 ngày, tối đa N mẫu theo gói; worker scale ngang, chia theo file Parquet |
