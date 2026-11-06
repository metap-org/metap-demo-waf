# ADR-0004: Managed ruleset là OWASP CRS trên engine SecLang, cộng system ruleset riêng

- Trạng thái: Chấp nhận; engine cụ thể **chờ chọn** ở [ADR-0031](0031-edge-l7-and-waf-engine-options.md)
- Ngày: 2026-11-05

## Bối cảnh

Managed ruleset là yêu cầu MVP để bán. Tự viết bộ rule phủ OWASP Top 10 ngang CRS tốn nhiều năm.
NCC vẫn cần tài sản riêng (rule cho lỗ hổng mới, rule theo kinh nghiệm vận hành) để khác biệt.

## Quyết định

- Bước managed ruleset ở edge là **engine tương thích SecLang** (Coraza hoặc ModSecurity) chạy
  **OWASP CRS**, dùng anomaly scoring và paranoia level của CRS.
- NCC duy trì **system ruleset riêng** viết bằng SecLang, phát hành có version như phần mềm.
- Khách cấu hình chứ không viết SecLang: paranoia level, ngưỡng anomaly, tắt rule ID, exclusion có
  phạm vi.
- CRS upstream và system ruleset đều đi qua Ruleset release: regression test, log-only song song,
  rồi mới thành mặc định ([06 §6.7](../06-runtime/00-index.md#67-phát-hành-crs-hoặc-system-ruleset-mới)).

## Hệ quả

- Có bộ rule đã được kiểm chứng rộng rãi ngay từ MVP.
- Engine SecLang là phần nặng nhất trong ngân sách độ trễ; phải benchmark sớm với CRS ở các
  paranoia level và giới hạn kiểm tra body theo entitlements.
- Chọn Coraza hay ModSecurity phụ thuộc ngôn ngữ của edge gateway; quyết định đi cùng lựa chọn
  công nghệ edge.

## Phương án đã cân nhắc

- **Tự viết toàn bộ ruleset**: kiểm soát hoàn toàn, không khả thi về thời gian.
- **Mua ruleset thương mại**: phụ thuộc vendor, chi phí theo lưu lượng.
