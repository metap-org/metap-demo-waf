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

## 6.1 Một request qua edge

```mermaid
sequenceDiagram
    participant U as End-user
    participant G as Edge gateway
    participant S as Snapshot (RAM)
    participant L as Log shipper
    participant O as Origin
    U->>G: HTTPS request (SNI = shop.example.com)
    G->>S: tra zone theo hostname
    G->>G: TLS, chuẩn hoá, access list, UAM, rate limit
    G->>G: managed ruleset (anomaly score), custom rule
    alt bị chặn
        G-->>U: 403, trang chặn có request ID
        G->>L: security event (rule ID, policy version)
    else cho qua
        G->>O: request đã lọc
        O-->>G: response
        G-->>U: response
    end
    G->>L: access log (theo tỉ lệ lấy mẫu của gói)
```

Không bước nào gọi mạng ra ngoài node, ngoài kết nối tới origin.

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

```mermaid
sequenceDiagram
    actor A as Khách
    participant M as Management
    participant C as Control
    participant D as DNS / CA
    A->>M: thêm domain, origin
    M->>C: ZoneChanged (chờ xác minh)
    C->>D: kiểm bản ghi xác minh
    D-->>C: hợp lệ
    C->>D: ACME: xin cert
    D-->>C: cert
    C->>C: kiểm tra origin từ edge
    C-->>M: zone sẵn sàng
    Note over M: managed ruleset bật log-only 7 ngày
    A->>M: trỏ CNAME về edge
    M-->>A: sau 7 ngày: báo cáo rule nào sẽ chặn, đề xuất bật enforce
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
    An-->>M: bước 8, CRS 942100, khớp tham số "q"
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
