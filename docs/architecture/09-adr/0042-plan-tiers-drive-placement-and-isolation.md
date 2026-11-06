# ADR-0042: Gói quyết định chỗ đặt zone, tài nguyên và thứ tự hy sinh

- Trạng thái: Chấp nhận (cơ chế); giá và giá trị từng entitlement chờ business chốt
- Ngày: 2026-11-06
- Liên quan: [ADR-0007](0007-log-by-subscription-entitlements.md), [ADR-0041](0041-cells-and-zone-placement.md), [8.18](../08-cross-cutting/18-segments-and-plans.md)

## Bối cảnh

Ở quy mô 1 triệu zone, 82% zone là Free và Pro nhưng chỉ tạo khoảng 16% traffic; 2% zone Enterprise tạo
khoảng 47% traffic và 60% tải WAF ([8.18 §1](../08-cross-cutting/18-segments-and-plans.md#1-phân-khúc)).
Hai nhóm có đặc tính tải ngược nhau: rất nhiều zone nguội và ít zone rất nóng. Chi phí của nhóm đầu là
chi phí theo zone, của nhóm sau là chi phí theo request.

Nếu mọi zone được phục vụ như nhau:

- RAM và cache của node bị chia cho hàng trăm nghìn zone Free nguội, zone trả tiền chịu tỉ lệ trúng
  thấp hơn;
- một đợt flood vào nhiều zone Free có thể đẩy p99 của khách Business, Enterprise trên cùng node;
- không giữ được chi phí Free gần bằng 0, nên không bán được gói Free.

## Quyết định

Gói (qua entitlements) là đầu vào của bốn quyết định kiến trúc, không chỉ của danh sách tính năng:

1. **Pool cell theo gói:** Free → pool mật độ cao; Pro, Business → pool chuẩn; Enterprise → pool
   enterprise hoặc cell riêng; mọi gói → cell cách ly khi bị tấn công. Trong pool vẫn theo
   [ADR-0041](0041-cells-and-zone-placement.md).
2. **Tài nguyên theo gói:** profile WAF (lite / PL1 / PL1–4 / tuỳ chỉnh), trọng số fair share CPU WAF
   (1 / 2 / 4 / 8), quota cache, kích thước body kiểm tra, lấy mẫu và retention log, ngân sách RAM
   runtime (pool Enterprise giữ mọi zone luôn nóng).
3. **Thứ tự hy sinh khi quá tải** cố định theo gói, từ Free lên; DDoS L3/L4 và pre-filter không bao
   giờ bị hạ theo gói.
4. **Thứ tự thay đổi:** canary chạy trên pool Free trước, Enterprise cuối; hàng đợi compiler đường
   thường ưu tiên gói cao, thay đổi hàng loạt của Free chạy nền; fast lane như nhau cho mọi gói.

Đổi gói chỉ là đổi entitlements và `cell_id` của zone; không deploy, không đổi phần mềm.

## Hệ quả

- Chi phí phục vụ zone Free giữ ở khoảng 0,18 USD/tháng; zone p99 của mỗi gói trả phí vẫn có lãi ở giá
  tham chiếu (QS-33).
- Khách trả tiền không có láng giềng là 650.000 site miễn phí (QS-32).
- Phải vận hành ba pool cell và công cụ chuyển zone giữa pool khi đổi gói (QS-34, [8.13 A16](../08-cross-cutting/13-operational-behaviors.md#a16-cell-quá-tải-hoặc-zone-cần-cách-ly)).
- Pool Free có thể thiếu năng lực trước các pool khác; đó là chủ đích, có event và báo cáo riêng.
- Free cần chống lạm dụng (giới hạn zone mỗi account, xác minh domain, Trust and Safety).

## Phương án đã cân nhắc

- **Mọi zone chung pool, chỉ khác tính năng:** đơn giản hơn nhưng không cô lập được, và chi phí theo
  zone nguội rơi vào RAM của node phục vụ khách lớn.
- **Free chỉ có L7, không WAF:** rẻ hơn nữa nhưng Free không có giá trị bảo mật để thu hút và thiếu
  dữ liệu tấn công cho cả fleet; profile lite chỉ tốn khoảng 1% CPU WAF.
- **Tính tiền theo request cho mọi gói:** công bằng về chi phí nhưng khách nhỏ không dự đoán được hoá
  đơn, và hoá đơn tăng khi bị tấn công; chọn giá cố định theo zone + fair use, Enterprise theo cam kết.
