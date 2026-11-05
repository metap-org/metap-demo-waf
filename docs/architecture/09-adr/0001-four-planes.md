# ADR-0001: Bốn plane, edge độc lập runtime

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Sản phẩm có bốn loại công việc với yêu cầu khác hẳn nhau: nghiệp vụ quản trị (đúng, đủ quyền,
audit), biến ý định thành config (an toàn, nhanh), xử lý traffic (độ trễ thấp, 99.99%), và log
(khối lượng lớn, giữ lâu, truy vấn nhiều kiểu). Gộp chúng làm một hệ thống buộc phần khắt khe nhất
(edge) đi theo chu kỳ deploy và điểm lỗi của phần dễ thay đổi nhất (portal).

## Quyết định

Tách thành **Management, Control, Edge, Analytics**. Mỗi plane deploy độc lập, giao tiếp qua hợp
đồng có version. Edge không phụ thuộc runtime vào plane nào: không gọi mạng ra ngoài node trên
đường đi của request, mất mọi kết nối vẫn phục vụ bằng snapshot hiện có. Không có luồng trực tiếp
từ Edge tới Management.

## Hệ quả

- Portal sập hoặc đang deploy không ảnh hưởng traffic khách.
- Phải định nghĩa và giữ tương thích hợp đồng giữa các plane ([05-contracts](../05-building-blocks/05-contracts.md)).
- Dữ liệu người dùng thấy về traffic luôn trễ hơn traffic thật (qua Analytics), chấp nhận với mục
  tiêu ≤ 60 s.

## Phương án đã cân nhắc

- **Ba plane, log nằm trong Management** (như demo hiện tại): khối lượng log làm nghẽn database
  nghiệp vụ, không có chỗ cho retention theo gói hay residency.
- **Edge đọc trực tiếp config từ Management**: edge phải hiểu schema nghiệp vụ, mọi thay đổi
  schema portal thành rủi ro outage.
