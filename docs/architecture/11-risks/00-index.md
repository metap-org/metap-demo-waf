# 11. Risks and Technical Debt

## 11.1 Rủi ro

| # | Rủi ro | Khả năng | Tác động | Giảm thiểu | Dấu hiệu cần hành động |
|---|---|---|---|---|---|
| R1 | Engine WAF + CRS vượt ngân sách độ trễ QS-1 | Trung bình | Cao | Spike so sánh trước khi chọn ([ADR-0031](../09-adr/0031-edge-l7-and-waf-engine-options.md)); giới hạn kiểm tra body theo entitlements; paranoia mặc định 1; engine sau interface `WafEngine` để thay | p99 > 2 ms trên traffic mẫu |
| R2 | Chặn nhầm làm mất niềm tin khách | Cao | Cao | Log-only 7 ngày khi onboarding; replay trước khi enforce; xử lý chặn nhầm tự phục vụ; tỉ lệ FP theo rule | Tỉ lệ FP của một rule > ngưỡng; ticket support về chặn nhầm tăng |
| R3 | Chi phí log vượt doanh thu | Trung bình | Cao | Entitlements, lấy mẫu access log, tách store theo chi phí | Chi phí log / doanh thu theo tenant > ngưỡng |
| R4 | Config hỏng ra toàn fleet | Thấp | Rất cao | Compiler kiểm tra, chữ ký, canary có gate, rollback tức thì | Lỗi nạp snapshot ở canary |
| R5 | Node trên hạ tầng đối tác bị chiếm | Thấp | Cao | Danh tính node, mTLS, thu hồi node, key cert chỉ trong RAM, keyless sau này | Hành vi bất thường từ một node |
| R6 | Phụ thuộc đối tác cho L3/L4 | Trung bình | Trung bình | Nhiều đối tác; hợp đồng có SLA | Đối tác không đáp ứng tấn công lớn |
| R7 | On-prem làm phức tạp mọi quyết định công nghệ | Trung bình | Trung bình | Ràng buộc "không dịch vụ độc quyền một cloud" từ đầu; chưa xây on-prem tới giai đoạn 3 | Khách lớn đòi on-prem sớm |
| R8 | Rate limit xấp xỉ không đạt kỳ vọng khách | Trung bình | Thấp | Ghi rõ trong tài liệu; chu kỳ gộp ngắn ở PoP lớn | Khiếu nại ngưỡng nhỏ không chính xác |
| R10 | Hai broker (RabbitMQ, Kafka) tăng vận hành | Trung bình | Thấp | Chỉ dùng RabbitMQ cho sự kiện điều khiển khối lượng nhỏ; gộp khi `metap` có adapter Kafka ([ADR-0022](../09-adr/0022-rabbitmq-for-control-events-kafka-for-telemetry.md)) | Sự cố vận hành broker lặp lại |
| R11 | Mẫu request đầy đủ lộ dữ liệu nhạy cảm | Thấp | Cao | Che trường bắt buộc tại edge, khách tắt được, retention ngắn, mã hoá at-rest ([ADR-0026](../09-adr/0026-replay-on-request-samples-with-shared-pipeline.md)) | Khách hoặc kiểm toán phản đối |
| R12 | Nếu chọn Rust cho edge: thiếu người viết Rust (org vừa bỏ Rust ở repo khác) | Trung bình | Cao | Tuyển hoặc đào tạo sớm; edge là codebase tách biệt ([ADR-0030](../09-adr/0030-language-per-workload.md)) | Tiến độ edge chậm hơn các plane khác |
| R13 | Nếu chọn libmodsecurity: lỗi bộ nhớ (C++) trên đường traffic | Trung bình | Cao | Pin version, vá trong 72 giờ, fuzz lớp tích hợp, bắt lỗi tại ranh giới; thay engine khi có lựa chọn an toàn hơn qua bộ test CRS | CVE mới của libmodsecurity |
| R14 | Nếu edge proxy không phải Go: ngữ nghĩa custom rule lệch giữa `cel-go` và evaluator IR | Thấp | Cao | Bộ vector test đối chiếu chạy ở cả hai phía trong CI ([ADR-0032](../09-adr/0032-cel-checked-at-control-ir-at-edge.md)) | Vector test lệch |
| R15 | Chặng proxy → WAF làm vượt ngân sách độ trễ | Thấp | Trung bình | PoP nhỏ đặt chung host qua Unix socket; verdict sớm; đo trong spike ([ADR-0033](../09-adr/0033-two-tier-edge-proxy-and-waf.md)) | p99 gọi WAF > 1,5 ms |
| R16 | Tự xây CDN tốn đầu tư băng thông và lưu trữ trước khi có doanh thu | Trung bình | Cao | Làm theo giai đoạn: cache cơ bản trong proxy ở MVP, tiered cache sau; chế độ đứng sau CDN của khách để bán sớm ([ADR-0035](../09-adr/0035-cdn-and-dns-strategy.md)) | Chi phí băng thông / doanh thu theo PoP |
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
CEL), công nghệ của Control, Analytics, Management ([05 · bản đồ công nghệ](../05-building-blocks/07-technology-map.md)).

| Chủ đề | Cần quyết | Ảnh hưởng |
|---|---|---|
| Tự bật Under Attack mode theo detection | Có cho phép, mặc định tắt hay bật | [ADR-0008](../09-adr/0008-under-attack-mode.md) |
| Bảng entitlements cụ thể | Giá trị từng gói (lấy mẫu, retention, tỉ lệ mẫu request, số rule) | [ADR-0007](../09-adr/0007-log-by-subscription-entitlements.md), mô hình giá |
| Cấp phép và cập nhật on-prem | Cách phát hành, cách cập nhật ruleset cho khách offline | [ADR-0006](../09-adr/0006-saas-first-on-prem-capable.md) |
| Nhà cung cấp geo/ASN | MaxMind hay IPinfo, theo giá và điều khoản phân phối lại | [ADR-0025](../09-adr/0025-geo-asn-data-commercial-license.md) |
| Đối tác L3/L4 và hạ tầng PoP đầu tiên | Chọn đối tác, region đầu tiên | [ADR-0005](../09-adr/0005-edge-on-own-and-existing-infra.md) |
| **Chiến lược CDN và DNS** | Tự xây theo giai đoạn + chế độ đứng sau CDN khách (đề xuất), hay đứng sau CDN, bán lại CDN đối tác, hoặc không CDN | [ADR-0035](../09-adr/0035-cdn-and-dns-strategy.md); kéo theo tiêu chí cache khi chọn nền proxy |
| **Nền tầng proxy và engine tầng WAF** | Chọn sau spike 3 tuần: shortlist S2 (Pingora + Coraza), S3 (Go + Coraza), S1 (Envoy + Coraza); mốc S5 (NGINX, cho cache) và S4 (HAProxy + SPOE); đo thêm libmodsecurity ở tầng WAF; kèm HTTP/3 | [ADR-0031](../09-adr/0031-edge-l7-and-waf-engine-options.md); kéo theo ngôn ngữ proxy, replay, `edge-verify`, [ADR-0032](../09-adr/0032-cel-checked-at-control-ir-at-edge.md) |
| Phân công zone cho node | MVP mọi node phục vụ mọi zone; khi nào chia shard | [5.8.6](../05-building-blocks/08-components/06-rollout-distribution-fleet.md) |
