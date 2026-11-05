# ADR-0011: Rate limit toàn fleet là xấp xỉ

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Khách đặt ngưỡng "100 request/phút mỗi IP" và kỳ vọng nó đúng trên toàn fleet, không phải trên
từng node. Đếm chính xác toàn cục cần một lời gọi mạng mỗi request, phá vỡ [ADR-0001](0001-four-planes.md).

## Quyết định

Mỗi node đếm cục bộ và **gộp định kỳ** với node khác (state sync); quyết định dựa trên số cục bộ
cộng số đã gộp gần nhất. Sai số tối đa bằng lưu lượng của một chu kỳ gộp.

## Hệ quả

- Không có lời gọi mạng trên đường request.
- Ngưỡng nhỏ (vài request/giây) kém chính xác hơn ngưỡng lớn; ghi rõ trong tài liệu sản phẩm.
- Mất kết nối giữa node: rơi về đếm cục bộ (lỏng hơn, không chặn nhầm thêm).

## Phương án đã cân nhắc

- **Kho counter tập trung đồng bộ**: chính xác nhưng thêm độ trễ và điểm lỗi.
- **Chỉ đếm cục bộ**: kẻ tấn công rải request qua nhiều node vượt ngưỡng dễ dàng.
