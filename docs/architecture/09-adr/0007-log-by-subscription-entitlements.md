# ADR-0007: Log và giới hạn theo subscription, thực thi bằng entitlements

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Access log là khoản chi phí hạ tầng lớn nhất. Mức giữ log (lấy mẫu, retention, khoảng tra cứu) là
quyết định kinh doanh, phải đổi được theo gói mà không deploy.

## Quyết định

- Gói cước định nghĩa **entitlements**: tỉ lệ lấy mẫu access log, retention tối đa theo loại log,
  khoảng truy vấn và chi phí truy vấn, số đích export, số rule, tính năng bật.
- Management phát `EntitlementsChanged`; Control đưa phần áp ở edge vào snapshot; Analytics áp
  retention và giới hạn truy vấn.
- Security event (request bị chặn hoặc khớp rule) **không bao giờ bị lấy mẫu**; chỉ access log của
  request bình thường được lấy mẫu.

## Hệ quả

- Đổi gói có hiệu lực trong vài phút, không cần deploy.
- Dashboard phải biết tỉ lệ lấy mẫu để ngoại suy đúng tổng traffic.
- Hạ gói: dữ liệu cũ hết hạn theo lịch, khách được báo trước, không xoá đột ngột.

## Phương án đã cân nhắc

- **Retention cố định cho mọi khách**: không thu tiền được theo giá trị, chi phí không kiểm soát.
- **If-else theo tên gói trong code**: mỗi gói mới là một lần deploy.
