# ADR-0005: Edge chạy trên PoP riêng và hạ tầng có sẵn

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Xây mạng PoP toàn cầu tốn vốn lớn; chỉ dùng hạ tầng thuê thì phụ thuộc đối tác và khó tối ưu chi
phí ở khu vực trọng điểm.

## Quyết định

Hỗ trợ cả hai. Edge là **một artifact** chạy được trên bare metal, VM, container. Fleet registry
quản lý node theo nơi chạy (PoP riêng, đối tác, cloud, hạ tầng khách). Định tuyến và L3/L4 dùng năng
lực của nơi đặt node khi có (anycast, scrubbing của đối tác).

## Hệ quả

- Mở được thị trường mới bằng hạ tầng thuê, đầu tư PoP riêng ở nơi có lưu lượng lớn.
- Không giả định anycast riêng; phải hỗ trợ định tuyến DNS theo địa lý.
- Cùng artifact là tiền đề cho hybrid và on-prem ([ADR-0006](0006-saas-first-on-prem-capable.md)).
- Node trên hạ tầng đối tác là môi trường kém tin cậy hơn: danh tính node, mTLS, key cert chỉ trong
  RAM là bắt buộc ([08 §8.2](../08-cross-cutting/00-index.md#82-bảo-mật-của-chính-nền-tảng)).

## Phương án đã cân nhắc

- **Chỉ PoP riêng**: chi phí và thời gian triển khai lớn.
- **Chỉ hạ tầng đối tác**: phụ thuộc, khó khác biệt về hiệu năng và giá.
