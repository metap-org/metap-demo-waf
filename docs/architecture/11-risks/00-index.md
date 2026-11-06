# 11. Risks and Technical Debt

## 11.1 Rủi ro

| # | Rủi ro | Khả năng | Tác động | Giảm thiểu | Dấu hiệu cần hành động |
|---|---|---|---|---|---|
| R1 | Coraza + CRS và GC của Go vượt ngân sách độ trễ QS-1 | Trung bình | Cao | Benchmark ngay đầu giai đoạn MVP; giới hạn kiểm tra body theo entitlements; paranoia mặc định 1; tối thiểu cấp phát trên đường request; lối thoát Envoy + `ext_proc` ([ADR-0013](../09-adr/0013-edge-gateway-custom-go-with-coraza.md)) | p99 > 2 ms trên traffic mẫu |
| R2 | Chặn nhầm làm mất niềm tin khách | Cao | Cao | Log-only 7 ngày khi onboarding; replay trước khi enforce; xử lý chặn nhầm tự phục vụ; tỉ lệ FP theo rule | Tỉ lệ FP của một rule > ngưỡng; ticket support về chặn nhầm tăng |
| R3 | Chi phí log vượt doanh thu | Trung bình | Cao | Entitlements, lấy mẫu access log, tách store theo chi phí | Chi phí log / doanh thu theo tenant > ngưỡng |
| R4 | Config hỏng ra toàn fleet | Thấp | Rất cao | Compiler kiểm tra, chữ ký, canary có gate, rollback tức thì | Lỗi nạp snapshot ở canary |
| R5 | Node trên hạ tầng đối tác bị chiếm | Thấp | Cao | Danh tính node, mTLS, thu hồi node, key cert chỉ trong RAM, keyless sau này | Hành vi bất thường từ một node |
| R6 | Phụ thuộc đối tác cho L3/L4 | Trung bình | Trung bình | Nhiều đối tác; hợp đồng có SLA | Đối tác không đáp ứng tấn công lớn |
| R7 | On-prem làm phức tạp mọi quyết định công nghệ | Trung bình | Trung bình | Ràng buộc "không dịch vụ độc quyền một cloud" từ đầu; chưa xây on-prem tới giai đoạn 3 | Khách lớn đòi on-prem sớm |
| R8 | Rate limit xấp xỉ không đạt kỳ vọng khách | Trung bình | Thấp | Ghi rõ trong tài liệu; chu kỳ gộp ngắn ở PoP lớn | Khiếu nại ngưỡng nhỏ không chính xác |
| R10 | Hai broker (RabbitMQ, Kafka) tăng vận hành | Trung bình | Thấp | Chỉ dùng RabbitMQ cho sự kiện điều khiển khối lượng nhỏ; gộp khi `metap` có adapter Kafka ([ADR-0022](../09-adr/0022-rabbitmq-for-control-events-kafka-for-telemetry.md)) | Sự cố vận hành broker lặp lại |
| R11 | Mẫu request đầy đủ lộ dữ liệu nhạy cảm | Thấp | Cao | Che trường bắt buộc tại edge, khách tắt được, retention ngắn, mã hoá at-rest ([ADR-0026](../09-adr/0026-replay-on-request-samples-with-shared-pipeline.md)) | Khách hoặc kiểm toán phản đối |
| R9 | Đội nhỏ dàn trải 4 plane | Cao | Cao | Mua/dùng mã nguồn mở cho phần không khác biệt; làm đúng thứ tự giai đoạn | Trễ MVP |

## 11.2 Chênh lệch với code demo hiện tại

Code trong repo là demo, không phải điểm xuất phát bắt buộc. Những chỗ khác kiến trúc đích:

| Chủ đề | Demo | Kiến trúc đích |
|---|---|---|
| Plane | 3 plane; log nằm trong database của portal | 4 plane, Analytics riêng |
| Snapshot | Mỗi zone một key trong kho key-value, không chữ ký, không ack | Bundle bất biến có chữ ký, ack theo version, rollout theo đợt |
| Managed ruleset | Không có | CRS + system ruleset trên engine SecLang |
| Ngôn ngữ rule | JSON grammar riêng | Cú pháp có sẵn, AST, builder ↔ text |
| Rate limit | Đếm cục bộ từng node | Gộp định kỳ toàn fleet |
| Log | Chỉ security event, buffer trong RAM, ghi vào Postgres của portal | Access log + event, buffer đĩa, bus bền, store riêng |
| Vòng đời policy | Monitor mode, `configVersion` chỉ là số đếm | PolicyVersion bất biến, mô phỏng, duyệt, canary, rollback |
| Fleet | Một process, không danh tính node | Registry, mTLS, health, độ phủ version |
| Cert, Under Attack, SSO, entitlements, SIEM export | Không có | MVP |

Khuyến nghị: giữ Management (portal trên `metap`) làm nền và tiến hoá dần; viết lại Control và
Edge theo hợp đồng ở [05-contracts](../05-building-blocks/05-contracts.md); xây mới Analytics.

## 11.3 Quyết định còn mở

Đã chốt ngày 2026-11-06: biến thể cú pháp rule ([ADR-0014](../09-adr/0014-cel-as-rule-language.md),
CEL), Coraza hay ModSecurity ([ADR-0013](../09-adr/0013-edge-gateway-custom-go-with-coraza.md),
Coraza), công nghệ từng plane ([05 · bản đồ công nghệ](../05-building-blocks/07-technology-map.md)).

| Chủ đề | Cần quyết | Ảnh hưởng |
|---|---|---|
| Tự bật Under Attack mode theo detection | Có cho phép, mặc định tắt hay bật | [ADR-0008](../09-adr/0008-under-attack-mode.md) |
| Bảng entitlements cụ thể | Giá trị từng gói (lấy mẫu, retention, tỉ lệ mẫu request, số rule) | [ADR-0007](../09-adr/0007-log-by-subscription-entitlements.md), mô hình giá |
| Cấp phép và cập nhật on-prem | Cách phát hành, cách cập nhật ruleset cho khách offline | [ADR-0006](../09-adr/0006-saas-first-on-prem-capable.md) |
| Nhà cung cấp geo/ASN | MaxMind hay IPinfo, theo giá và điều khoản phân phối lại | [ADR-0025](../09-adr/0025-geo-asn-data-commercial-license.md) |
| Đối tác L3/L4 và hạ tầng PoP đầu tiên | Chọn đối tác, region đầu tiên | [ADR-0005](../09-adr/0005-edge-on-own-and-existing-infra.md) |
| Phân công zone cho node | MVP mọi node phục vụ mọi zone; khi nào chia shard | [level-3/21](../05-building-blocks/level-3/21-rollout-distribution-fleet.md) |
