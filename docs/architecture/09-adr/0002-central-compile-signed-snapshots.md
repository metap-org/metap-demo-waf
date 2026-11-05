# ADR-0002: Compile tập trung, edge nhận snapshot bất biến có chữ ký

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Nguyên nhân outage phổ biến nhất của WAF/CDN là đẩy config hỏng ra toàn fleet cùng lúc. Edge cũng
cần quyết định trong vài micro giây, nên không thể diễn giải ý định nghiệp vụ trên đường request.

## Quyết định

- Control biên dịch PolicyVersion đã duyệt thành **snapshot**: bản đầy đủ, bất biến, có version,
  có chữ ký, chia thành bundle theo zone và bundle chung.
- Mọi thứ giải quyết được một lần ở trung tâm (thứ tự priority, rule bị tắt, log-only, giới hạn
  theo gói) được giải quyết ở compiler; edge chỉ so khớp.
- Đường thường đi theo đợt (canary → region → toàn fleet) với gate tự động; đường khẩn bỏ qua đợt
  nhưng vẫn qua compiler và vẫn có version.
- Node kiểm chữ ký và schema trước khi nạp, swap nguyên khối, giữ bản last-good.

## Hệ quả

- Rollback là chuyển về version trước đã có sẵn, không cần biên dịch lại.
- Biết chính xác node nào chạy version nào (ack theo version).
- Compiler trở thành điểm kiểm soát an toàn quan trọng; cần bộ test riêng.
- Kích thước snapshot tăng theo số rule; giới hạn bằng entitlements và delta khi truyền.

## Phương án đã cân nhắc

- **Edge kéo từng rule riêng lẻ từ một kho key-value**: không có tính nguyên khối, node có thể
  chạy nửa cũ nửa mới; không có chữ ký.
- **Đẩy đồng loạt không chia đợt**: đơn giản hơn nhưng một lỗi lọt compiler thành outage toàn fleet.
