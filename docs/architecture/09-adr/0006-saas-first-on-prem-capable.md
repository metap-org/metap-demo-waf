# ADR-0006: SaaS trước, kiến trúc giữ đường cho hybrid và on-prem

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Khách tài chính, chính phủ thường yêu cầu on-prem hoặc dữ liệu không rời hạ tầng của họ. Làm
on-prem ngay từ đầu làm chậm MVP.

## Quyết định

- Ra thị trường bằng **SaaS**.
- Kiến trúc không được chặn **hybrid** (Management/Control của NCC, edge và có thể Analytics tại
  khách) và **on-prem** (cả bốn plane tại khách).
- Ràng buộc kèm theo: không plane nào dùng dịch vụ chỉ có trên một cloud; mỗi plane đóng gói được
  độc lập; edge gọi lên Control (từ trong ra), không cần mở cổng vào hạ tầng khách.

## Hệ quả

- Một số lựa chọn công nghệ bị loại dù tiện cho SaaS (dịch vụ managed độc quyền của một cloud).
- On-prem cần bản Analytics gọn hơn cho quy mô một khách; xác định khi tới giai đoạn Nâng cao.
- Cấp phép và cập nhật on-prem là việc riêng, chưa thiết kế.

## Phương án đã cân nhắc

- **Chỉ SaaS**: mất phân khúc yêu cầu on-prem.
- **On-prem trước**: chậm ra thị trường, khó vận hành tập trung.
