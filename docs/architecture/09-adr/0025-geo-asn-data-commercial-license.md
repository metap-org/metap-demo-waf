# ADR-0025: Dữ liệu geo/ASN dùng giấy phép thương mại

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

Dùng dữ liệu geo/ASN định dạng MMDB từ nhà cung cấp có giấy phép **thương mại cho phép phân phối
lại** tới edge node và bản on-prem (MaxMind GeoIP2 hoặc IPinfo; chọn khi đàm phán giá). Dữ liệu
được phát xuống edge trong bundle chung, cập nhật hàng tuần qua vòng đời phát hành như ruleset.

## Lý do

Bản miễn phí (GeoLite2) có điều khoản EULA không phù hợp cho sản phẩm thương mại phân phối lại.
Geo/ASN là đầu vào của access list, custom rule và dashboard, nên sai lệch hoặc vi phạm license đều
ảnh hưởng trực tiếp tới khách.
