# ADR-0033: Edge tách hai tầng: L7 và WAF inspection

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Bối cảnh

Thiết kế trước đặt mọi thứ trong một binary edge: TLS, kết nối, pre-filter, CRS, L7. Hai loại tải
trong đó khác hẳn nhau:

- Giữ kết nối, TLS, buffer, cache, L7: tốn **RAM, số kết nối, băng thông**.
- Kiểm tra nội dung bằng CRS: tốn **CPU**, đột biến khi bị tấn công tầng ứng dụng.

Gộp chung thì đỉnh CPU của WAF làm chậm việc giữ kết nối của mọi tenant, và mọi request (kể cả request
lẽ ra bị chặn từ cửa bằng whitelist/blacklist hay rate limit) đều tốn chi phí kiểm tra.

## Quyết định

- **Tầng 1, L7** ([5.8.1](../05-building-blocks/08-components/01-l7.md)): kết nối, TLS,
  giới hạn giao thức, pre-filter (access list, reputation, rate limit, DDoS L7, challenge, Under Attack
  mode, bot tín hiệu rẻ, custom rule phần pre), cache, proxy tới origin, telemetry.
- **Tầng 2, WAF inspection** ([5.8.2](../05-building-blocks/08-components/02-waf-inspection.md)): managed
  ruleset, custom rule phần inspect, API schema. Không giữ kết nối client, không gọi origin; nhận request
  qua **Inspection API** (gRPC stream) và trả verdict.
- L7 chỉ gọi WAF cho request đã qua pre-filter và không được miễn.
- Hai tầng scale độc lập; PoP nhỏ đặt chung host (process riêng, cgroup CPU riêng, Unix socket), PoP lớn
  tách pool node.
- Tầng WAF có admission control theo tenant; quá tải thì trả `Overloaded` ngay, L7 áp fail mode của zone.
- Ngân sách p99: L7 ≤ 0,5 ms, gọi WAF ≤ 1,5 ms, tổng ≤ 2 ms (QS-1 giữ nguyên).

## Hệ quả

- Khi bị tấn công, phần lớn lưu lượng bị loại ở L7 với chi phí rất thấp; CPU của WAF dành cho phần
  còn lại.
- Engine WAF chạy process riêng: crash hay lỗi bộ nhớ của engine không kéo theo kết nối client; GC (nếu
  engine là Go) không làm dừng L7. Điều này làm thay đổi đánh giá các phương án ở [ADR-0031](0031-edge-l7-and-waf-engine-options.md).
- Thêm một chặng mạng trong PoP (thường 0,1–0,3 ms) và một hợp đồng phải giữ tương thích
  ([5.5](../05-building-blocks/05-contracts.md#inspection-api-l7--waf)).
- L7 phải buffer body tới giới hạn để gửi WAF, nên RAM của L7 tính theo cả kết nối lẫn body.
- Snapshot chia phần cho từng tầng; policy agent chạy ở cả hai tầng.

## Phương án đã cân nhắc

- **Một binary làm hết** (thiết kế trước): ít chặng hơn nhưng tải CPU và RAM giành nhau, không scale riêng.
- **WAF làm reverse proxy nối tiếp** (L7 → WAF proxy → origin): WAF phải giữ kết nối và stream
  response, mất lợi ích tách tải; hai lần proxy.
