# ADR-0026: Replay chạy trên mẫu request đầy đủ, dùng chung pipeline với edge

- Trạng thái: Chấp nhận; theo [ADR-0030](0030-language-per-workload.md), replay worker viết bằng ngôn ngữ của edge (chờ chọn ở [ADR-0031](0031-edge-l7-and-waf-engine-options.md)) để dùng chung code pipeline
- Ngày: 2026-11-06

## Bối cảnh

Access log chỉ giữ một số trường (path, status, IP, rule khớp), không đủ để đánh giá lại CRS hay
một custom rule đọc header/body. Replay chỉ dựa trên access log sẽ cho kết quả sai.

## Quyết định

- Edge gửi **mẫu request đầy đủ** (header, query, body cắt ngắn, đã che trường nhạy cảm) với tỉ lệ
  nhỏ theo zone, luôn kèm toàn bộ request bị chặn hoặc khớp rule (trong giới hạn kích thước). Tỉ lệ
  mẫu là entitlement.
- Mẫu đi qua topic `telemetry.sample.v1`, lưu Parquet trên object storage, retention theo gói.
- Replay worker dựng pipeline bằng **cùng thư viện `pipeline`** với edge, nạp snapshot nháp do compiler sinh
  ra, chạy trên mẫu, so với quyết định gốc.

## Hệ quả

- Kết quả replay là ước lượng theo mẫu, hiển thị kèm khoảng tin cậy và số mẫu.
- Mẫu request là dữ liệu nhạy cảm nhất trong hệ thống: che trường bắt buộc, khách tắt được hoàn toàn,
  retention ngắn mặc định.
