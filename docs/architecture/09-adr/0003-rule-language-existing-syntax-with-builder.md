# ADR-0003: Ngôn ngữ rule dùng cú pháp có sẵn, builder và text qua một AST

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Khách enterprise vừa cần UI cho người không viết code, vừa cần text để review, diff và đưa vào
IaC. Tự phát minh ngôn ngữ mới bắt khách học thêm và bắt đội tự viết tài liệu, công cụ.

## Quyết định

- Dùng một **cú pháp biểu thức đã có trên thị trường** cho custom rule (họ cú pháp kiểu
  wirefilter: `http.request.uri.path contains "/admin" and ip.src in {...}`). Chọn cụ thể biến thể
  nào là việc tiếp theo, ghi ở [11](../11-risks/00-index.md).
- Builder UI và text là hai cách hiển thị **một AST**; chuyển qua lại không mất thông tin.
- Text là nguồn sự thật được lưu, diff, review, dùng trong IaC.
- Field và toán tử là tập đóng, có kiểu; validator làm việc trên AST, từ chối field không biết.

## Hệ quả

- Khách và đối tác tích hợp đã quen cú pháp; tài liệu ít hơn.
- Builder phải biểu diễn được mọi AST hợp lệ; biểu thức quá phức tạp cho builder hiển thị ở chế
  độ chỉ text nhưng vẫn hợp lệ.
- Phần mở rộng ngoài cú pháp gốc (field riêng của sản phẩm) phải được ghi rõ.

## Phương án đã cân nhắc

- **DSL riêng**: linh hoạt nhưng tốn công tài liệu, công cụ, đào tạo khách.
- **Cho khách viết SecLang**: mạnh nhưng khó, dễ viết rule chậm hoặc nguy hiểm; giữ SecLang cho
  managed ruleset ([ADR-0004](0004-managed-ruleset-crs-on-seclang.md)).
- **Chỉ có builder, lưu JSON**: khó review và IaC.
