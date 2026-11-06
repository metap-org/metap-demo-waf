# 6.10 Luồng Analytics

Toàn bộ đường đi của dữ liệu traffic: từ lúc gateway tạo bản ghi tới từng kiểu đọc. Thành phần chi
tiết ở [level-3/30](../05-building-blocks/level-3/30-ingestion-and-storage.md) và
[level-3/31](../05-building-blocks/level-3/31-query-detection-export-replay.md).

| # | Luồng |
|---|---|
| A | [Toàn cảnh](#a-toàn-cảnh) |
| B | [Đường ghi và ngân sách độ trễ](#b-đường-ghi-và-ngân-sách-độ-trễ) |
| C | [Dashboard](#c-dashboard) |
| D | [Điều tra và nhãn chặn nhầm](#d-điều-tra-và-nhãn-chặn-nhầm) |
| E | [Phát hiện, cảnh báo, case](#e-phát-hiện-cảnh-báo-case) |
| F | [Xuất sang SIEM](#f-xuất-sang-siem) |
| G | [Replay](#g-replay) |
| H | [Gate cho rollout](#h-gate-cho-rollout) |
| I | [Metering và billing](#i-metering-và-billing) |
| J | [Retention và xoá dữ liệu](#j-retention-và-xoá-dữ-liệu) |
| K | [Backpressure và lỗi theo từng chặng](#k-backpressure-và-lỗi-theo-từng-chặng) |

## A. Toàn cảnh

```mermaid
flowchart LR
    subgraph EDGE["Edge node"]
        gw["Gateway<br/>tạo bản ghi"]
        vec["Vector<br/>lấy mẫu · che · buffer đĩa"]
    end
    subgraph AN["Analytics region"]
        ig["Ingest gateway"]
        subgraph K["Kafka"]
            raw[("security · access · sample")]
            enr[("enriched")]
        end
        en["Enricher"]
        ch[("ClickHouse<br/>raw + MV")]
        ar["Archiver"]
        s3[("Parquet")]
        qa["Query API"]
        sd["Streaming detector"]
        pd["Scheduled detector"]
        ex["Exporter"]
        rp["Replay"]
        me["Metering"]
    end
    subgraph MG["Management"]
        ui["Portal · API"]
        case["Case"]
        bill["Billing"]
    end
    ctl["Control · rollout gate"]
    siem["SIEM khách"]
    ntf["Kênh thông báo"]

    gw --> vec --> ig --> raw --> en
    en --> ch
    en --> enr
    raw -->|sample| ar
    enr --> ar --> s3
    enr --> sd
    enr --> ex --> siem
    ch --> pd
    ch --> qa --> ui
    ch --> me --> bill
    ch -.-> ctl
    s3 --> rp --> ui
    sd --> case
    pd --> case
    sd --> ntf
    pd --> ntf
```

Một luồng ghi (gateway → Kafka → enricher), sau đó rẽ ra theo kiểu đọc. Mọi nhánh đọc đều từ Kafka
hoặc ClickHouse hoặc Parquet, không nhánh nào đọc database của Management hay Control.

## B. Đường ghi và ngân sách độ trễ

```mermaid
sequenceDiagram
    participant G as Gateway
    participant V as Vector (edge)
    participant I as Ingest gateway
    participant K as Kafka
    participant E as Enricher
    participant C as ClickHouse
    G->>V: bản ghi protobuf (Unix socket, không chặn)
    V->>V: lấy mẫu access log, che trường, ghi buffer đĩa
    V->>I: lô nén zstd (gRPC, mTLS)
    I->>I: kiểm schema, quota tenant/node
    I->>K: produce acks=all
    K-->>I: ack
    I-->>V: ack đầu cuối → xoá khỏi buffer đĩa
    E->>K: poll
    E->>E: bỏ trùng request ID, geo/ASN, tên rule, expire_at
    E->>C: insert theo lô
    E->>K: produce telemetry.enriched.v1
    E->>K: commit offset (sau khi cả hai thành công)
```

| Chặng | Ngân sách p95 | Ghi chú |
|---|---|---|
| Gateway → Vector | < 100 ms | Ring buffer trong RAM, ghi không chặn |
| Vector gom lô và gửi | ≤ 2 s | Lô theo kích thước hoặc thời gian |
| Ingest gateway → Kafka ack | ≤ 1 s | `acks=all` |
| Kafka → enricher | ≤ 1 s | Consumer lag là chỉ số chính |
| Enricher gom lô → ClickHouse | ≤ 2 s | |
| ClickHouse hiển thị dữ liệu | ≤ 5 s | Async insert, merge chạy nền |
| **Tổng tới tra cứu được** | **≤ 15 s** | Còn dư so với QS-11 (≤ 60 s) để chịu đỉnh tải |

## C. Dashboard

```mermaid
sequenceDiagram
    actor U as SOC
    participant M as Management
    participant Q as Query API
    participant C as ClickHouse
    U->>M: mở dashboard zone, 24 giờ
    M->>Q: TimeSeries(zone, 24h, group by action) + danh tính người gọi
    Q->>Q: kiểm quyền zone, kiểm khoảng thời gian theo gói
    Q->>C: SELECT ... FROM traffic_1m WHERE tenant_id = ? AND zone_id IN (?) ...
    C-->>Q: chuỗi theo phút
    Q->>Q: nhân ngược tỉ lệ lấy mẫu cho access log
    Q-->>M: dữ liệu + tỉ lệ lấy mẫu đã dùng
    M-->>U: biểu đồ, có tổng traffic làm mẫu số
```

- Dashboard luôn đọc từ materialized view (`traffic_1m`, `traffic_1h`, `rule_efficacy_1h`), không quét bảng raw.
- Khoảng ≤ 6 giờ dùng `traffic_1m`, dài hơn dùng `traffic_1h`; Query API tự chọn.
- Security event không bị lấy mẫu nên số chặn là số thật; chỉ tổng traffic là ước lượng khi gói có lấy mẫu.

## D. Điều tra và nhãn chặn nhầm

```mermaid
sequenceDiagram
    actor S as SOC
    participant M as Management
    participant Q as Query API
    participant C as ClickHouse
    participant R as RabbitMQ
    S->>M: tra request ID
    M->>Q: LookupRequest(id)
    Q->>C: security_events / access_logs WHERE tenant_id = ? AND request_id = ?
    C-->>Q: bản ghi (bloom filter trên request_id)
    Q-->>M: bước pipeline, rule, policy version, trường khớp
    S->>M: đánh dấu chặn nhầm
    M->>R: FpLabelled(tenant, request_id, rule_id, policy_version)
    R->>C: ghi fp_labels (consumer trong Analytics)
    Note over C: rule_efficacy_1h join fp_labels → tỉ lệ FP theo rule và version
```

Nhãn chặn nhầm là dữ liệu của người dùng nên được tạo ở Management (có audit), rồi chép sang
Analytics để tính toán; Analytics không phải nơi gốc của nhãn.

## E. Phát hiện, cảnh báo, case

```mermaid
sequenceDiagram
    participant K as Kafka enriched
    participant SD as Streaming detector
    participant C as ClickHouse
    participant PD as Scheduled detector
    participant AE as Alert engine
    participant N as Notifier
    participant M as Management
    K->>SD: event liên tục
    SD->>SD: cửa sổ trượt theo khoá (ASN, IP, zone)
    SD->>AE: vượt ngưỡng nhanh
    loop mỗi phút
        PD->>C: truy vấn alert policy trên MV
        C-->>PD: tỉ lệ 5xx, tỉ lệ chặn, traffic tăng vọt
        PD->>AE: điều kiện khớp
    end
    AE->>AE: gộp theo khoá, giờ im lặng, trạng thái firing
    AE->>N: gửi kênh đã cấu hình
    AE->>M: AlertRaised + CaseProposed (qua RabbitMQ)
    AE->>M: AlertResolved khi hết điều kiện
```

| Loại phát hiện | Nguồn | Độ trễ | Ví dụ |
|---|---|---|---|
| Nhanh | Streaming trên Kafka | Giây | Hơn N request bị chặn trong 1 phút từ một ASN |
| Theo lịch | Scheduled trên MV | ≤ 2 phút | Tỉ lệ 5xx của origin > 5% trong 5 phút |
| Bất thường | Baseline trong ClickHouse (Nâng cao) | Phút | Traffic lệch khỏi dải bình thường theo giờ trong tuần |

## F. Xuất sang SIEM

```mermaid
sequenceDiagram
    participant K as Kafka enriched
    participant R as Router
    participant Q as Hàng đợi đích X
    participant F as Formatter
    participant D as SIEM của khách
    participant P as PostgreSQL analytics
    K->>R: bản ghi tenant T
    R->>Q: đích của T (theo cấu hình từ Management)
    Q->>F: lô
    F->>D: gửi (HTTP / syslog TLS / S3)
    alt thành công
        D-->>F: 2xx
        F->>P: lưu offset của đích X
    else lỗi
        F->>F: retry có backoff, đích khác không bị chặn
        Note over Q: tụt hậu quá retention Kafka → đánh dấu mất dữ liệu, báo khách
    end
```

Export đọc từ Kafka chứ không từ ClickHouse, nên log tới SIEM của khách cùng nhịp với dashboard và
không cạnh tranh tài nguyên truy vấn.

## G. Replay

```mermaid
sequenceDiagram
    actor A as Quản trị bảo mật
    participant M as Management
    participant C as Control compiler
    participant RP as Replay worker
    participant S3 as Parquet (mẫu request)
    A->>M: mô phỏng bản nháp trên 7 ngày
    M->>C: biên dịch nháp (không ký, không rollout)
    C-->>RP: snapshot nháp
    M->>RP: job(zone, 7 ngày, cỡ mẫu)
    RP->>S3: đọc mẫu request của zone
    RP->>RP: chạy package pipeline với snapshot nháp
    RP->>RP: so với quyết định gốc trong mẫu
    RP-->>M: thêm chặn / bớt chặn theo rule, ví dụ, khoảng tin cậy
```

## H. Gate cho rollout

Trong lúc một đợt rollout đang quan sát, orchestrator hỏi Analytics chỉ số của các zone liên quan,
tách theo `policy_version_id`:

| Chỉ số | So sánh |
|---|---|
| Tỉ lệ chặn | Version mới vs version cũ, cùng khoảng thời gian, trên các node đã nhận vs chưa nhận |
| Tỉ lệ 5xx tới origin | Không tăng quá ngưỡng |
| Độ trễ thêm tại edge (từ metrics nền tảng) | p99 không tăng quá ngưỡng |

Vì mọi bản ghi mang policy version, so sánh này không cần biết node nào ở đợt nào.

## I. Metering và billing

```mermaid
sequenceDiagram
    participant C as ClickHouse usage_1h
    participant ME as Metering job
    participant M as Management billing
    loop mỗi giờ
        ME->>C: request, băng thông, dung lượng log theo tenant
        ME->>M: UsageReport(tenant, kỳ, số đo) qua RabbitMQ
    end
    M->>M: cộng dồn theo kỳ thanh toán, so với quota
    M->>M: cuối kỳ: hoá đơn
```

`UsageReport` idempotent theo `(tenant, giờ)`: chạy lại một giờ ghi đè chứ không cộng thêm.

## J. Retention và xoá dữ liệu

| Dữ liệu | Nơi | Cơ chế hết hạn |
|---|---|---|
| Kafka topic | Kafka | 3 ngày, cố định |
| Raw (`security_events`, `access_logs`) | ClickHouse | 3 ngày trên SSD rồi sang volume S3; xoá theo `expire_at` của từng dòng (theo gói lúc ghi) |
| Số liệu gộp | ClickHouse | Dài hơn raw (ví dụ 13 tháng), vì nhỏ và cần cho báo cáo năm |
| Archive, mẫu request | Parquet | Lifecycle theo prefix tenant; legal hold bằng tag |
| Nhãn FP, trạng thái alert | ClickHouse / PostgreSQL | Theo vòng đời rule, alert |

Chấm dứt hợp đồng: xoá theo `tenant_id` ở ClickHouse (partition theo ngày, lọc theo tenant), xoá
prefix trong object storage, xoá đích export; ghi biên bản xoá vào audit.

## K. Backpressure và lỗi theo từng chặng

| Chặng hỏng | Dữ liệu nằm ở đâu | Tác động người dùng | Phục hồi |
|---|---|---|---|
| Vector trên node chậm | Ring buffer gateway, rồi bỏ access log | Thiếu access log của node đó | Tự hết khi Vector bắt kịp |
| Ingest gateway / mạng tới region | Buffer đĩa Vector | Log trễ | Xả buffer theo thứ tự thời gian |
| Quota tenant bị vượt | Bị bỏ tại gateway (access log trước) | Thiếu access log của tenant đó, có số đếm | Tăng gói hoặc tự hết khi traffic giảm |
| Kafka | Buffer đĩa Vector (qua ack đầu cuối) | Log trễ | Như trên |
| Enricher chậm | Kafka (3 ngày) | Dashboard, tra cứu, export, streaming detector đều trễ vì cùng phụ thuộc dữ liệu đã làm giàu | Scale enricher, consumer lag là cảnh báo chính |
| ClickHouse | Kafka | Dashboard, tra cứu, scheduled detector ngừng; streaming detector và export vẫn chạy | Enricher retry, không commit offset cho tới khi ghi được |
| Đích SIEM của khách | Hàng đợi đích + Kafka | Chỉ đích đó trễ | Retry; quá 3 ngày thì mất phần quá hạn, báo khách |
