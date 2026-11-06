# ADR-0044: Realtime gateway cho portal và runtime của notifier

- Trạng thái: Chấp nhận (2026-11-06): **R3**, Elixir + Phoenix cho realtime gateway và notifier
- Ngày: 2026-11-06
- Liên quan: [ADR-0030](0030-language-per-workload.md), [5.8.13](../05-building-blocks/08-components/13-realtime-gateway.md), [5.8.11](../05-building-blocks/08-components/11-notification-fanout.md)

## Bối cảnh

[ADR-0030](0030-language-per-workload.md) không chọn Elixir cho notifier và ghi điều kiện xem lại: (a)
cần đẩy event realtime tới hàng chục nghìn phiên portal cùng lúc. Ở quy mô thiết kế 1 triệu zone, điều
kiện này xảy ra: 10.000–30.000 phiên lúc bình thường, 50.000–100.000 phiên trong đợt tấn công lớn, kèm
hàng chục nghìn kết nối mới trong vài phút ([5.8.13](../05-building-blocks/08-components/13-realtime-gateway.md)).

Yêu cầu:

- Kết nối dài (WebSocket) với số lượng lớn, mỗi phiên có hàng đợi và giới hạn riêng.
- Gom số liệu theo zone **chỉ cho zone đang có người xem**, dùng chung cho mọi người xem cùng zone.
- Fan-out trong cụm, tự phục hồi khi một phần lỗi, nối lại có số thứ tự.
- License đóng gói được cho on-prem ([ADR-0024](0024-licensing-policy-for-shipped-components.md)).

Notifier ([5.8.11](../05-building-blocks/08-components/11-notification-fanout.md)) có đặc tính gần
giống: fan-out nhiều kênh, retry, cô lập lỗi theo kênh, theo khách.

## Phương án

| | R1. Go tự xây | R2. Centrifugo (Go, server realtime dựng sẵn) | R3. Elixir + Phoenix Channels | R4. Dịch vụ managed (Pusher, Ably) |
|---|---|---|---|---|
| Kết nối dài số lượng lớn | Làm được; tự viết hàng đợi theo phiên, nối lại, fan-out cụm | Có sẵn, đã chạy ở quy mô triệu kết nối | Có sẵn (process mỗi phiên, Phoenix PubSub, Presence) | Có sẵn |
| Gom số liệu theo zone đang xem | Tự viết (goroutine + map + khoá) | Phải đặt ở dịch vụ khác rồi publish vào Centrifugo | Tự nhiên: một process mỗi zone, supervisor quản lý vòng đời | Phải đặt ở dịch vụ khác |
| Tự phục hồi | Tự viết | Có cho phần kết nối | OTP supervisor cho cả kết nối lẫn aggregator | Có |
| Dùng lại cho notifier | Notifier Go hiện tại | Không | Có: process mỗi kênh, retry, Oban (job queue trên PostgreSQL, giữ nguyên bảng `deliveries`) | Không |
| Ngôn ngữ, vận hành | Không thêm runtime | Không thêm runtime; thêm một sản phẩm cần vận hành | **Thêm runtime thứ ba** (BEAM): tuyển người, CI, quan sát | Không vận hành |
| License, on-prem | Tốt | Apache-2.0 (bản PRO là thương mại, không dùng) | Apache-2.0 / MIT (Elixir, Phoenix, Broadway, Oban bản mã nguồn mở) | **Loại**: không on-prem, dữ liệu ra ngoài region |

## Quyết định

Chọn **R3** cho cả realtime gateway và notifier:

- Hai thành phần có cùng đặc tính (nhiều thực thể sống lâu, fan-out, retry, cô lập lỗi), đúng thế mạnh
  của BEAM; gom vào một runtime giúp chi phí thêm runtime được chia cho hai việc.
- Phần khó nhất của realtime gateway là gom số liệu theo zone đang xem và nối lại không mất dữ liệu;
  với R3 đó là vài GenServer có supervisor, với R1/R2 là code đồng bộ tự viết.
- Ranh giới ngôn ngữ trùng ranh giới thành phần: gateway và notifier chỉ đọc Kafka, RabbitMQ,
  PostgreSQL (`deliveries`) qua hợp đồng có sẵn; Go vẫn là ngôn ngữ của enricher, Query API, detection.

**R2** là phương án dự phòng nếu không muốn thêm runtime: Centrifugo giữ kết nối, một dịch vụ Go nhỏ
làm zone aggregator rồi publish vào Centrifugo; notifier giữ Go.

Realtime gateway xây ở giai đoạn Mở rộng; MVP dùng Query API với refresh vài giây cho số ít phiên đầu
tiên. Notifier viết bằng Elixir ngay từ MVP để không phải viết hai lần.

## Hệ quả

- Thêm Elixir vào danh sách ngôn ngữ của [ADR-0030](0030-language-per-workload.md): Go cho Control,
  Analytics (phần xử lý dữ liệu), Management; edge theo ADR-0031; Elixir cho realtime gateway và notifier.
- Cần ít nhất hai người đọc được Elixir; CI, image, quan sát (metrics Prometheus, trace OpenTelemetry)
  cho runtime BEAM.
- Notifier chuyển sang Elixir không đổi hợp đồng: vẫn bảng `deliveries`, vẫn adapter theo kênh, vẫn
  các sự kiện từ detection.
- Enricher thêm live tap (Go) để đầu vào của gateway tỉ lệ với số zone đang xem.
