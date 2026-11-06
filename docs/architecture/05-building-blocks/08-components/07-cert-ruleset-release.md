# 5.8.7 Cert & key manager, ruleset release (Control plane)

## Cert & key manager

```mermaid
flowchart LR
    ev["ZoneChanged<br/>yêu cầu cert"]
    subgraph CK["cert-service (Go)"]
        dv["Domain verifier<br/>DNS TXT · CNAME"]
        acme["ACME client · lego"]
        up["Upload handler<br/>cert khách tự mang"]
        rn["Renewal scheduler"]
        ks["Key server<br/>mTLS cho node"]
    end
    bao["OpenBao transit<br/>mã hoá phong bì"]
    pg[("PostgreSQL · control")]
    ca["CA (ACME)"]
    dns["DNS"]
    fast["Fast lane<br/>HTTP-01 token"]
    cfg["Config repository<br/>tham chiếu cert"]
    nodes["Edge agents"]

    ev --> dv --> dns
    dv --> acme <--> ca
    acme --> fast
    acme --> bao
    up --> bao
    bao --> pg
    acme --> cfg
    up --> cfg
    rn --> acme
    nodes --> ks
    ks --> pg
    ks --> bao
```

| Component | Thiết kế |
|---|---|
| **Domain verifier** | Kiểm xác minh sở hữu (mặc định CNAME uỷ quyền `_acme-challenge`, hoặc TXT), trạng thái trỏ DNS, CAA, tra DNS khi thêm domain; điều phối kiểm origin từ edge (`OriginProbe`); kết quả là điều kiện để xin cert và gửi về Management qua `ZoneCheckResult` ([1.7.1](../../01-introduction/07-business-analysis/01-onboarding.md)) |
| **ACME client** | `lego`; DNS-01 qua uỷ quyền `_acme-challenge` là mặc định, để cert đầu có trước khi khách đổi DNS; HTTP-01 (token phát xuống edge qua fast lane, edge trả lời `/.well-known/acme-challenge/`) khi khách chỉ xác minh bằng TXT; nhiều CA để dự phòng |
| **Upload handler** | Cert khách tự mang: kiểm chuỗi, khớp hostname, hạn |
| **Lưu trữ** | Private key mã hoá phong bì bằng OpenBao transit, lưu ciphertext trong PostgreSQL; cert công khai và digest đưa vào bundle zone |
| **Renewal scheduler** | Gia hạn khi còn 30 ngày; thất bại liên tiếp → cảnh báo khách và NOC |
| **Key server** | Node gọi qua mTLS, chỉ nhận key của zone được gán; giải mã bằng OpenBao, trả qua kênh mTLS; ghi audit mỗi lần cấp |

## Ruleset release

Cùng pipeline này phát hành **dữ liệu bot** ([8.16 §6](../../08-cross-cutting/16-bot-management.md#6-vòng-học-ở-analytics)): danh sách bot đã xác minh (dải IP công bố của các bot lớn, kiểm rDNS định kỳ), reputation fingerprint/ASN/IP gộp từ Analytics, và mô hình bot. Mô hình và reputation qua đánh giá trên tập giữ lại và log-only song song như ruleset.

Pipeline phát hành CRS upstream, system ruleset của NCC, và dữ liệu geo/ASN, chạy như CI.

```mermaid
flowchart LR
    crs["CRS upstream<br/>tag mới"]
    sys["Repo system ruleset<br/>SecLang"]
    geo["Dữ liệu geo/ASN<br/>hàng tuần"]
    subgraph RR["ruleset-release pipeline"]
        lint["Lint SecLang<br/>nạp thử bằng edge-verify"]
        ftw["go-ftw<br/>test CRS + test riêng"]
        corp["Corpus traffic<br/>tốt / tấn công"]
        perf["Benchmark<br/>độ trễ theo paranoia"]
        pub["Publish release"]
    end
    repo["Config repository<br/>global_versions"]
    orch["Orchestrator<br/>log-only song song"]

    crs --> lint
    sys --> lint
    geo --> pub
    lint --> ftw --> corp --> perf --> pub --> repo --> orch
```

| Bước | Điều kiện qua |
|---|---|
| Lint | Mọi rule nạp được bằng engine WAF đang dùng ở edge (qua `edge-verify`); không trùng rule ID giữa CRS và system ruleset (system ruleset dùng dải ID riêng) |
| go-ftw | 100% test CRS chính thức pass ở các paranoia level hỗ trợ; test riêng của system ruleset pass |
| Corpus | Traffic tốt: tỉ lệ chặn không tăng quá ngưỡng so với release trước; traffic tấn công: tỉ lệ phát hiện không giảm |
| Benchmark | p99 theo paranoia không tăng quá ngưỡng |
| Rollout | Log-only song song trên toàn fleet (bản cũ quyết định, bản mới chỉ ghi), so sánh trên traffic thật ([06 §6.7](../../06-runtime/00-index.md#67-phát-hành-crs-hoặc-system-ruleset-mới)) |

Rule khẩn (zero-day) đi nhánh rút gọn: lint + test riêng cho rule đó + corpus traffic tốt, rồi fast lane.
