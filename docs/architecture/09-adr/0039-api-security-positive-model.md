# ADR-0039: API security theo mô hình cho phép, dựa trên schema và danh tính consumer

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- **Discovery** chạy ở Analytics: gom path thành template, thống kê theo endpoint, so với kho API để tìm
  endpoint mới, shadow, zombie, không dùng; sinh schema nháp cho khách duyệt.
- **Kho API** ở Management là nguồn sự thật; khách tải OpenAPI 3.x hoặc duyệt schema nháp.
- **Kiểm schema chia hai tầng:**
  - route table và các kiểm không cần body (endpoint có khai báo, tham số path/query/header,
    content-type, kích thước) ở **proxy**;
  - kiểm body JSON và GraphQL ở **WAF**.
  - Compiler sinh cả hai từ một schema, cùng manifest.
- Chế độ log-only mặc định khi mới tải schema; khách chuyển enforce theo endpoint.
- **Xác thực ở edge:** JWT (chữ ký theo JWKS, `exp`, `iss`, `aud`), mTLS client cert, có mặt API key.
  **JWKS do Control kéo** từ URL của khách và phát qua fast lane; edge không gọi ra ngoài trên đường request.
- **Danh tính consumer** (từ JWT `sub`, API key, cert subject hoặc cookie, khách chọn) được băm thành
  `api.consumer`. Đây là khoá cho rate limit, quota và analytics của API thay cho IP.
- BOLA và dữ liệu nhạy cảm trong response ở giai đoạn Nâng cao. Kiểm response chỉ bật theo endpoint,
  có lấy mẫu.

## Hệ quả

- Kết quả discovery phụ thuộc tỉ lệ lấy mẫu log của gói: endpoint ít traffic xuất hiện chậm hơn.
- Schema sai sẽ chặn nhầm traffic hợp lệ, nên bắt buộc log-only trước và hiển thị chênh lệch giữa
  discovery và schema.
- WAAP phát hiện được bất thường kiểu BOLA nhưng không sửa được lỗi phân quyền của origin; tài liệu sản
  phẩm phải nói rõ ranh giới này.

## Phương án đã cân nhắc

- **Chỉ dùng CRS cho API:** bắt được payload độc, không bắt được endpoint lạ, lạm dụng logic hay lạm dụng
  theo consumer.
- **Kiểm toàn bộ schema ở WAF:** đơn giản hơn, nhưng request tới endpoint không khai báo vẫn tốn CPU của WAF.
