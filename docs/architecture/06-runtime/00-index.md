# 6. Runtime View

Các kịch bản chính, mỗi kịch bản là một luồng xuyên plane.

| # | Kịch bản |
|---|---|
| 6.1 | [Một request qua edge](#61-một-request-qua-edge) |
| 6.2 | [Thay đổi policy theo đường thường](#62-thay-đổi-policy-theo-đường-thường) |
| 6.3 | [Bật Under Attack mode, rule khẩn](#63-bật-under-attack-mode-rule-khẩn) |
| 6.4 | [Onboarding một zone](#64-onboarding-một-zone) |
| 6.5 | [Xử lý một request bị chặn nhầm](#65-xử-lý-một-request-bị-chặn-nhầm) |
| 6.6 | [Cảnh báo và case](#66-cảnh-báo-và-case) |
| 6.7 | [Phát hành CRS hoặc system ruleset mới](#67-phát-hành-crs-hoặc-system-ruleset-mới) |
| 6.8 | [Edge node khởi động, mất kết nối, phục hồi](#68-edge-node-khởi-động-mất-kết-nối-phục-hồi) |
| 6.9 | [Đổi gói subscription](#69-đổi-gói-subscription) |
| 6.10 | [Luồng Analytics](10-analytics-flows.md): đường ghi, ngân sách độ trễ, dashboard, điều tra, cảnh báo, export, replay, gate, metering, retention, lỗi theo chặng |

## 6.1 Một request qua edge

```mermaid
sequenceDiagram
    participant U as End-user
    participant D as DNS điều hướng
    participant P as L7
    participant W as WAF inspection
    participant C as Cache
    participant O as Origin
    participant L as Vector
    U->>D: shop.example.com (CNAME tới hostname edge)
    D-->>U: IP của PoP gần, đang khoẻ
    U->>P: HTTPS request
    P->>P: TLS, giới hạn giao thức, access list, UAM, rate limit, custom rule phần pre
    alt bị chặn hoặc challenge ở pre-filter
        P-->>U: 403 hoặc challenge (có request ID)
        P->>L: security event (bước, rule ID, policy version)
    else cần kiểm tra
        P->>W: Inspect(header, body tới giới hạn)
        W-->>P: Verdict (rule ID, anomaly score)
        alt verdict block / challenge
            P-->>U: 403 hoặc challenge
            P->>L: security event
        else cho qua
            P->>C: tra cache
            alt hit
                C-->>U: response từ cache
            else miss
                P->>O: request đã lọc
                O-->>P: response
                P->>C: lưu nếu cache được
                P-->>U: response
            end
        end
    end
    P->>L: access log (cache_status, inspected, theo tỉ lệ lấy mẫu)
```

Không bước nào gọi mạng ra ngoài PoP, ngoài kết nối tới origin. Request bị chặn ở pre-filter không
tốn CPU của tầng WAF.

## 6.2 Thay đổi policy theo đường thường

```mermaid
sequenceDiagram
    actor A as Quản trị bảo mật
    actor B as Người duyệt
    participant M as Management
    participant An as Analytics
    participant C as Control
    participant E as Edge
    A->>M: soạn rule (builder hoặc text)
    M->>M: parse, validate trên AST
    M->>An: replay bản nháp trên 7 ngày
    An-->>M: thêm 1.240 request bị chặn, 3 nghi chặn nhầm
    A->>M: gửi duyệt kèm kết quả mô phỏng
    B->>M: duyệt
    M->>C: PolicyVersionPublished v42 (qua outbox)
    C->>C: biên dịch, kiểm tra, ký
    C->>E: canary: một nhóm node (hoặc log-only)
    E-->>C: ack v42
    E->>An: event gắn v42
    An-->>C: tỉ lệ chặn, FP trong ngưỡng
    C->>E: region rồi toàn fleet
    C-->>M: v42 trên 100% node
```

Gate hỏng ở bất kỳ đợt nào: orchestrator dừng, đưa các node đã nhận về version trước, báo trạng
thái về Management.

## 6.3 Bật Under Attack mode, rule khẩn

```mermaid
sequenceDiagram
    actor S as SOC / NCC
    participant M as Management
    participant C as Control
    participant E as Edge (mọi node)
    S->>M: bật Under Attack mode cho zone (hạn tự tắt 2 giờ)
    M->>M: ghi audit, không qua duyệt
    M->>C: PolicyVersionPublished (kênh khẩn)
    C->>C: biên dịch, kiểm tra, ký
    C->>E: fast lane: báo + snapshot ngay, không chia đợt
    E-->>C: ack (mục tiêu ≤ 5 s toàn fleet)
    Note over E: request chưa có clearance token nhận challenge
    C->>C: hết hạn: tự phát version tắt UAM
```

Rule khẩn của NCC (zero-day) đi cùng đường, bắt đầu từ console NCC thay vì Management của khách,
và nằm trong bundle chung.

## 6.4 Onboarding một zone

> **Chờ onboarding.** Sequence này theo bản nháp chưa duyệt ở
> [requirements §3.1](../../requirements/03-business-analysis/01-onboarding.md), chỉ cho một kịch bản (DNS đầy
> đủ, origin có IP public, cổng chuẩn). Onboarding đang được phân tích lại theo nhiều kịch bản.

Đường chính của bản nháp: xác minh và cert xong
**trước** khi khách đổi DNS, bật chặn chỉ sau giai đoạn log-only có số liệu.

```mermaid
sequenceDiagram
    actor K as Khách
    participant M as Management
    participant C as Control
    participant E as Edge
    participant D as DNS / CA
    participant A as Analytics
    K->>M: thêm domain (OB2)
    M->>C: ZoneChanged (chờ xác minh)
    C->>D: tra DNS hiện tại, CAA
    C-->>M: ZoneCheckResult (dns_discovery, caa)
    K->>D: tạo CNAME _acme-challenge (OB3)
    C->>D: kiểm từ nhiều resolver
    C-->>M: ZoneCheckResult (ownership)
    C->>D: ACME DNS-01
    D-->>C: cert
    C-->>M: CertStatusChanged (issued)
    C->>E: OriginProbe ở 2 PoP (OB4)
    E-->>C: kết quả
    C-->>M: ZoneCheckResult (origin)
    K->>M: chọn preset (OB6), kích hoạt (OG1)
    M->>C: PolicyVersionPublished, ZoneChanged (active)
    C->>E: Snapshot
    K->>E: kiểm thử qua curl --resolve (OB7)
    K->>D: đổi CNAME về edge (OB8)
    C-->>M: ZoneCheckResult (dns_routing)
    E->>A: TelemetryRecord (log-only)
    M->>A: Query: traffic đầu tiên, báo cáo log-only (OB9)
    K->>M: change request bật chặn (OB10)
    M->>C: PolicyVersionPublished
```

## 6.5 Xử lý một request bị chặn nhầm

```mermaid
sequenceDiagram
    actor U as End-user
    actor S as SOC
    participant M as Management
    participant An as Analytics
    U->>S: báo bị chặn, gửi request ID trên trang chặn
    S->>M: tra request ID
    M->>An: Query (request ID)
    An-->>M: bước 12, CRS 942100, khớp tham số "q"
    S->>M: đánh dấu chặn nhầm
    M->>An: ghi nhãn FP (nuôi tỉ lệ FP theo rule)
    S->>M: tạo exception: bỏ 942100 cho path /search, tham số q
    M->>An: replay exception 7 ngày
    An-->>M: mở thêm 310 request, 0 đáng ngờ
    Note over M: vào vòng đời policy như 6.2
```

## 6.6 Cảnh báo và case

```mermaid
sequenceDiagram
    participant E as Edge
    participant An as Analytics
    participant M as Management
    actor S as SOC
    E->>An: event chặn tăng vọt từ một ASN
    An->>An: alert policy khớp (ngưỡng hoặc baseline)
    An->>M: AlertRaised + CaseProposed (mẫu event, đề xuất chặn ASN 2 giờ)
    An->>S: thông báo qua kênh đã cấu hình
    S->>M: mở case, chấp nhận đề xuất
    Note over M: rule tạm có hạn, đi đường thường hoặc khẩn tuỳ mức độ
    S->>M: đóng case kèm tóm tắt
```

## 6.7 Phát hành CRS hoặc system ruleset mới

1. Ruleset release kéo CRS mới (hoặc nhận bản system ruleset từ threat research).
2. Chạy regression: traffic mẫu tốt (không được chặn) và mẫu tấn công (phải chặn), đo độ trễ.
3. Gắn version, đưa vào config store.
4. Rollout trên toàn fleet ở chế độ **log-only song song**: bản cũ vẫn quyết định, bản mới chỉ
   ghi event, để so sánh tỉ lệ chặn trên traffic thật.
5. Zone chọn "tự nhận bản mới" được chuyển sau khi gate qua; zone cố định version thấy thông báo
   và danh sách rule thay đổi.

## 6.8 Edge node khởi động, mất kết nối, phục hồi

- **Khởi động:** node xác thực với Control bằng danh tính đã cấp khi enrollment, kéo bundle chung
  và mọi bundle zone được gán, kiểm chữ ký, nạp; chỉ nhận traffic sau khi nạp xong.
- **Mất Control:** chạy tiếp bằng snapshot hiện có vô thời hạn; health check của PoP vẫn xanh.
- **Mất Analytics:** log vào buffer đĩa; đầy thì bỏ access log trước, giữ security event.
- **Phục hồi:** agent so version với Control, kéo bản đầy đủ nếu lệch; log shipper xả buffer theo
  thứ tự thời gian.

## 6.9 Đổi gói subscription

1. Khách đổi gói trong Management; thanh toán xác nhận.
2. Management phát `EntitlementsChanged`.
3. Control đưa giới hạn áp ở edge (tỉ lệ lấy mẫu, số rule, giới hạn tài nguyên) vào bundle zone
   kế tiếp; Analytics áp retention và giới hạn truy vấn mới.
4. Hạ gói không xoá ngay dữ liệu cũ hơn retention mới: dữ liệu hết hạn theo lịch, khách được báo trước.
