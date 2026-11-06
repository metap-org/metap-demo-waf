# ADR-0027: Cảnh báo theo template có công thức tường minh

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Bối cảnh

Khách cần cảnh báo cho cả WAF lẫn DDoS, chỉnh được ngưỡng, và hiểu vì sao một cảnh báo bật. Cảnh báo
khó giải thích (mô hình học máy hộp đen, truy vấn tự viết) dẫn tới hai hậu quả: khách tắt cảnh báo
vì nhiễu, hoặc không tin cảnh báo khi có sự cố thật.

## Quyết định

- Cảnh báo là **template trong danh mục cố định** (WAF: W1–W6, DDoS: D1–D6), mỗi template có công
  thức tín hiệu ghi rõ trong tài liệu ([5.8.10](../05-building-blocks/08-components/10-alerting-rules-and-algorithms.md)).
- Khách chỉnh tham số: cửa sổ, mode ngưỡng (`static`, `relative`, `statistical`), ngưỡng warning và
  critical, `floor`, `min_volume`, `for`, điều kiện tự tắt, `group_by`, kênh.
- Baseline là **median và MAD** của cùng phút trong tuần ở 4 tuần trước, loại các phút đã có
  cảnh báo critical; không dùng trung bình/độ lệch chuẩn vì bị đợt tấn công cũ kéo lệch.
- Mỗi cảnh báo gửi kèm giá trị, ngưỡng đã tính, baseline, số lần vượt liên tiếp, sai số do lấy mẫu,
  top nguồn đóng góp.
- Có chức năng xem thử policy trên 7 ngày dữ liệu, dùng chung code với detector.

## Hệ quả

- Phát hiện bất thường bằng mô hình phức tạp hơn (giai đoạn Nâng cao) phải ra được cùng dạng bằng
  chứng (giá trị, kỳ vọng, khoảng) để vào cùng máy trạng thái và cùng giao diện.
- Template mới phải thêm vào danh mục kèm công thức, không có cảnh báo "tuỳ ý".

## Phương án đã cân nhắc

- **Cho khách viết truy vấn cảnh báo tự do**: linh hoạt nhưng khó hỗ trợ, khó kiểm soát chi phí truy
  vấn, khó giải thích.
- **Chỉ ngưỡng tĩnh**: đơn giản nhưng nhiễu với traffic có mùa vụ, bỏ sót với zone lưu lượng thấp.
