# 8. Cross-cutting Concepts

| File | Nội dung |
|---|---|
| [00-index.md](00-index.md) | 8.1–8.11: multi-tenancy, bảo mật, quyền, version, fail mode, quyền riêng tư, entitlements, challenge, giao nhận, thời gian |
| [12-platform-monitoring.md](12-platform-monitoring.md) | 8.12 Giám sát nền tảng: SLI/SLO, probe tổng hợp theo hành vi, cảnh báo theo burn rate, đối soát mất dữ liệu |
| [13-operational-behaviors.md](13-operational-behaviors.md) | 8.13 Hành vi vận hành: từng tình huống và thao tác với tín hiệu, cảnh báo, tự phản ứng, runbook |
| [14-latency-and-error-attribution.md](14-latency-and-error-attribution.md) | 8.14 Đo độ trễ theo tầng, đo chặng tới client, quy mỗi status code về đúng nguồn |
| [15-config-propagation-and-apply.md](15-config-propagation-and-apply.md) | 8.15 Luồng cấu hình xuống edge; cấu hình nào đổi nóng, build nền, reload, nâng phần mềm |
| [16-bot-management.md](16-bot-management.md) | 8.16 Bot management: tín hiệu nhiều lớp, điểm và lớp, policy, challenge, vòng học, quyền riêng tư, chống chiếm tài khoản |
| [17-api-security.md](17-api-security.md) | 8.17 API security: discovery, kho API, kiểm schema, JWT/mTLS, danh tính consumer, GraphQL, BOLA, dữ liệu nhạy cảm |
| [18-segments-and-plans.md](18-segments-and-plans.md) | 8.18 Phân khúc khách, tải theo phân khúc, chi phí phục vụ, gói giá và entitlements, kiến trúc theo gói, thứ tự hy sinh khi quá tải |

## 8.1 Multi-tenancy và cô lập

| Nơi | Cơ chế |
|---|---|
| Management | Mọi entity mang tenant; quyền kiểm tra ở service, không ở UI; phân cấp org → account → zone |
| Control | Snapshot theo zone; node chỉ nhận bundle của zone được gán; key cert mã hoá theo node được phép |
| Edge | Giới hạn tài nguyên theo tenant (CPU kiểm tra body, kích thước body, băng thông log); tenant vượt giới hạn bị hạ cấp theo fail mode của chính nó |
| Analytics | Phân vùng theo tenant ở bus và store; Query API là cửa duy nhất, luôn thêm điều kiện tenant/zone theo danh tính người gọi |
| Support NCC | Truy cập dữ liệu tenant theo yêu cầu, có thời hạn, ghi audit, khách thấy được |

## 8.2 Bảo mật của chính nền tảng

- **Danh tính node:** node enrollment một lần, nhận danh tính riêng; mọi kết nối node ↔ Control
  dùng mTLS; thu hồi được từng node.
- **Snapshot có chữ ký:** khoá ký nằm ở Control, node chỉ có khoá công khai; node bị chiếm không
  tạo được snapshot cho node khác.
- **Private key cert của khách:** lưu mã hoá, giải mã ở node chỉ trong RAM; hướng mở rộng là
  keyless (khoá ở hạ tầng khách) cho khách yêu cầu.
- **Phân tách nhiệm vụ:** duyệt 4-eyes cho thay đổi policy (bật theo account); console NCC tách
  khỏi portal khách, có duyệt riêng cho rule khẩn.
- **Audit:** mọi thay đổi ở Management và mọi hành động của NCC trên fleet ghi audit chỉ-ghi-thêm.

## 8.3 Danh tính và quyền

- SSO (SAML, OIDC), SCIM để đồng bộ user và nhóm; MFA bắt buộc với tài khoản không qua SSO.
- Vai trò mặc định: Account owner, Security admin, SOC analyst, Developer (chỉ đọc + exception
  trong phạm vi được giao), Viewer, Billing. ABAC giới hạn theo zone hoặc nhóm zone.
- API key có phạm vi (zone, hành động) và hạn; UI, API và IaC dùng cùng mô hình quyền.

## 8.4 Version và tương thích

- Mọi hợp đồng giữa plane có schema version; bên nhận chấp nhận N và N-1
  ([05-contracts.md](../05-building-blocks/05-contracts.md)).
- Edge gặp snapshot có schema mới hơn mức hiểu: từ chối, giữ bản cũ, báo lỗi nạp lên fleet registry.
- Phần mềm edge nâng cấp trước, sau đó Control mới bắt đầu phát schema mới.

## 8.5 Hành vi khi lỗi

- Fail-open hoặc fail-closed chọn theo zone, áp cho từng bước pipeline khi lỗi hoặc quá ngân sách
  thời gian; mỗi lần kích hoạt phát event riêng để khách thấy.
- Mặc định: fail-open (thương mại điện tử thường chọn), khách tài chính thường đổi sang fail-closed.
- Tầng WAF không trả lời kịp, quá tải (`Overloaded`) hoặc mất cả pool trong PoP cũng áp đúng fail mode
  này ở L7; pre-filter, challenge và rate limit vẫn chạy vì nằm ở L7 ([ADR-0033](../09-adr/0033-two-tier-edge-proxy-and-waf.md)).
- Origin lỗi: phục vụ bản cũ trong cache nếu có (`stale-if-error`).

## 8.6 Quyền riêng tư

- Che tại edge, trước khi log rời node: `Authorization`, cookie, trường form chứa mật khẩu, và
  danh sách trường do khách cấu hình thêm.
- IP có thể lưu dạng cắt bớt hoặc băm theo cấu hình tenant.
- Xoá theo tenant khi chấm dứt hợp đồng; retention không vượt entitlements.

## 8.7 Entitlements

Giá trị từng gói, chi phí phục vụ và cách gói quyết định chỗ đặt zone: [8.18](18-segments-and-plans.md).

Gói cước là cấu hình, không phải nhánh code ([ADR-0007](../09-adr/0007-log-by-subscription-entitlements.md)).

| Entitlement | Thực thi ở |
|---|---|
| Tỉ lệ lấy mẫu access log | Edge (log shipper), qua snapshot |
| Hạn mức băng thông, request; trần cứng (dùng thử, Free có trần) | Analytics (đếm), Management (quyết định), Edge (áp khi chạm trần, lớp A) ([ADR-0043](../09-adr/0043-usage-based-pricing-and-metering.md)) |
| Retention tối đa theo loại log | Analytics (stream processor, store) |
| Khoảng thời gian truy vấn tối đa, chi phí truy vấn | Analytics (Query API) |
| Số đích SIEM export | Management + Analytics |
| Kênh cảnh báo, số routing rule, escalation, hạn mức SMS ([33](../05-building-blocks/08-components/11-notification-fanout.md#kênh-theo-gói)) | Management (validate) + notifier (lúc gửi) |
| Số rule, số zone, tính năng (bot, API security, replay, canary) | Management (validate) + Control (compiler) |
| Giới hạn tài nguyên kiểm tra tại edge | Edge, qua snapshot |

## 8.8 Challenge và clearance

- Dùng chung cho Under Attack mode và bot management.
- Edge phát challenge, tự kiểm kết quả, cấp **clearance token có MAC (HMAC-SHA256)** gắn zone, thời hạn và
  đặc điểm client; node nào trong fleet cũng kiểm được token mà không gọi mạng.
- Khoá HMAC xoay vòng qua bundle chung; node chấp nhận khoá hiện tại và khoá liền trước. Dùng khoá đối xứng vì chính edge cấp token, nên mọi node đằng nào cũng phải giữ khoá cấp.

## 8.9 Giao nhận và trùng lặp

- Management → Control: outbox cùng transaction với lần ghi, at-least-once; Control bỏ trùng theo
  version ID.
- Edge → Analytics: at-least-once; bỏ trùng theo request ID + loại bản ghi.
- Không có chỗ nào yêu cầu exactly-once; mọi consumer idempotent.

## 8.10 Thời gian

- Mọi thời điểm lưu UTC; giao diện hiển thị theo múi giờ người dùng.
- Phân biệt thời điểm xảy ra (edge) và thời điểm nhận (Analytics); dashboard và retention dùng
  thời điểm xảy ra; độ lệch giữa hai thời điểm là chỉ số vận hành.

## 8.11 Quan sát chính nền tảng

Thiết kế đầy đủ ở [12-platform-monitoring.md](12-platform-monitoring.md) và [13-operational-behaviors.md](13-operational-behaviors.md).

Tách khỏi telemetry của khách. Chỉ số NCC theo dõi: độ trễ thêm tại edge theo bước pipeline, độ
phủ version (bao nhiêu % node đã ack version mới nhất), thời gian lan truyền, lỗi nạp snapshot, độ
trễ từ edge tới tra cứu được, mức đầy buffer đĩa, độ trễ theo đích export.
