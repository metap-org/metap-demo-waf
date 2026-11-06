# ADR-0043: Giá theo phí nền + hạn mức + phí vượt, đo usage không lấy mẫu, trần cứng ở edge

- Trạng thái: Chấp nhận (cơ chế); con số giá và hạn mức chờ business chốt
- Ngày: 2026-11-06
- Liên quan: [ADR-0007](0007-log-by-subscription-entitlements.md), [ADR-0042](0042-plan-tiers-drive-placement-and-isolation.md), [8.18](../08-cross-cutting/18-segments-and-plans.md)

## Bối cảnh

Bản giá đầu tiên ở 8.18 dùng giá phẳng theo zone, băng thông không giới hạn và có gói Free từ đầu. Khi
tính lại theo hồ sơ **công ty phần mềm thuê hạ tầng**:

- Băng thông là khoản lớn nhất (≈ 520 đ/GB khi thuê, gấp khoảng 10 lần phần xử lý L7 + WAF cho cùng một
  request), và không có biên để cạnh tranh với nhà mạng.
- Giá phẳng làm zone dùng nhiều nhất trong mỗi gói lỗ nặng (một zone Business 10.000 rps tốn hàng nghìn
  USD mỗi tháng nhưng trả 236 USD).
- Gói Free ở quy mô lớn chỉ khả thi với đơn giá băng thông của nhà mạng hoặc peering.
- Access log bị lấy mẫu theo gói nên không dùng được để tính tiền.

## Quyết định

1. **Giá = phí nền + hạn mức + phí vượt.** Phí nền là phần **bảo mật** (theo zone, theo tính năng, biên
   cao); hạn mức và phí vượt là phần **phân phối** (băng thông sạch, request, gần giá vốn). Không gói nào
   có băng thông không giới hạn.
2. **Traffic bị edge chặn hoặc challenge không tính phí.** Chỉ traffic sạch tới client được tính.
3. **Đo usage bằng bộ đếm riêng, không lấy mẫu:** policy agent đếm theo zone mỗi phút và gửi
   `UsageCounter` có số thứ tự; Analytics cộng chính xác, bỏ trùng theo `(node, zone, phút)`, đối soát
   trước khi xuất hoá đơn.
4. **Trần cứng thực thi ở edge** cho Dùng thử, Free có trần và chế độ chuyển tiếp tối giản: vượt trần
   thì Management phát entitlements mới, Control đẩy qua fast lane (lớp A); gói trả phí không bị cắt
   traffic, chỉ tính phí vượt và cảnh báo ở 80%, 100%.
5. **Không mở Free khi ra mắt** với hồ sơ công ty phần mềm; dùng thử 14 ngày thay thế. Free có trần chỉ
   mở khi có hồ sơ chi phí cho phép (nhà mạng, peering, đối tác CDN) hoặc với số lượng giới hạn.

## Hệ quả

- Zone dùng nhiều trả nhiều; biên của từng gói không còn phụ thuộc vào việc zone nặng hiếm.
- Hoá đơn của khách thay đổi theo traffic: cần dashboard usage theo zone, cảnh báo ngưỡng, ước tính hoá
  đơn trong kỳ.
- Thêm loại bản ghi `UsageCounter` trong telemetry, buffer riêng không bị bỏ khi đầy như access log
  ([5.5](../05-building-blocks/05-contracts.md)).
- Độ trễ áp trần cứng tới ≈ 10 phút; phần vượt trong khoảng đó chấp nhận bỏ qua.
- Hồ sơ nhà mạng chỉ đổi đơn giá đầu vào; cơ chế giữ nguyên.

## Phương án đã cân nhắc

- **Giá phẳng, fair use mềm** (bản trước): đơn giản cho khách, nhưng lỗ ở zone nặng và không kiểm soát
  được chi phí băng thông.
- **Tính hoàn toàn theo usage, không phí nền:** công bằng về chi phí, nhưng không thu được giá trị bảo
  mật và hoá đơn của khách nhỏ khó đoán.
- **Tính usage từ access log lấy mẫu rồi ngoại suy:** rẻ, nhưng sai số không chấp nhận được khi đưa vào
  hoá đơn.
