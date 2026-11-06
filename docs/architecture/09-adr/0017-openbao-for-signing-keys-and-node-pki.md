# ADR-0017: OpenBao cho khoá ký, mã hoá key cert và CA của node

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- **OpenBao transit engine:** ký manifest snapshot (Ed25519, khoá không bao giờ rời OpenBao); mã
  hoá phong bì private key cert của khách.
- **OpenBao PKI engine:** CA nội bộ cấp chứng chỉ X.509 ngắn hạn cho edge node (mTLS tới Control
  và relay).
- **ACME:** `lego` trong Cert manager; HTTP-01 challenge được phát xuống edge qua fast lane, DNS-01
  qua provider của khách hoặc DNS của NCC.

## Lý do

- OpenBao là bản fork mã nguồn mở (MPL-2.0) của Vault, chạy được on-prem. Vault đã chuyển sang BSL.
- `metap-control::SecretStore` đã có backend Vault, nên dùng lại được với OpenBao.

## Hệ quả

- OpenBao trở thành thành phần trọng yếu của Control: chạy HA, sao lưu, quy trình unseal.
