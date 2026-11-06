# ADR-0045: Cert node ngắn hạn tự gia hạn, re-enroll credential, thu hồi không dùng CRL

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06
- Liên quan: [ADR-0017](0017-openbao-for-signing-keys-and-node-pki.md), [8.19](../08-cross-cutting/19-node-identity-and-mtls.md)

## Bối cảnh

Hàng trăm tới hàng nghìn edge node chạy trên PoP riêng, hạ tầng đối tác và hạ tầng của khách; mọi kênh
điều khiển và log dùng mTLS. Cần:

- không có bước gia hạn bằng tay nào ở quy mô này;
- thu hồi nhanh một node bị chiếm;
- node mất mạng lâu vẫn tự quay lại được;
- cert hết hạn không bao giờ làm mất traffic của khách.

## Quyết định

1. **Cert node 24 h**, gia hạn ở 2/3 thời hạn có jitter, mỗi lần một cặp khoá mới, private key không
   rời node (TPM nếu có).
2. **Node identity service** ở Control là bên duy nhất node gọi để xin cert; dịch vụ này kiểm fleet
   registry rồi mới nhờ OpenBao ký. OpenBao không lộ ra mạng edge.
3. **Re-enroll credential** riêng, chỉ để xin cấp lại cert khi cert đã hết hạn, dùng được khi node
   offline ≤ 30 ngày và chưa bị thu hồi; quá điều kiện thì cần người duyệt.
4. **Thu hồi** bằng thời hạn ngắn cộng danh sách chặn phát qua fast lane; kết nối dài có tuổi tối đa 1 h.
   Không CRL, không OCSP.
5. **Dịch vụ trung tâm** dùng cert-manager với issuer OpenBao, cert 7 ngày.
6. **Edge → origin** dùng cert client theo zone do Origin CA của NCC cấp, 90 ngày, tự gia hạn; khách
   tin CA chứ không ghim cert lá.

## Hệ quả

- Không có cert node nào sống quá 24 h; lộ khoá của một node chỉ có giá trị tới lần hết hạn kế tiếp.
- Node identity service và OpenBao nằm trên đường gia hạn của cả fleet: chạy HA; nếu cả hai sập lâu hơn
  ≈ 8 h (phần còn lại của thời hạn sau khi bắt đầu gia hạn), node mất kênh điều khiển và log, nhưng traffic
  vẫn được phục vụ bằng snapshot đang có.
- Tải ký: 1.000 node gia hạn mỗi 16 h là chưa tới 0,02 lần ký/giây, không đáng kể; jitter tránh dồn.
- Cần TPM hoặc file mã hoá trên node cho re-enroll credential; node không có TPM có rủi ro lộ credential
  cao hơn, bù bằng điều kiện dải mạng và giám sát tần suất re-enroll.

## Phương án đã cân nhắc

- **Cert dài hạn (1 năm) + CRL/OCSP:** ít lần ký, nhưng thu hồi phụ thuộc phân phối CRL tới mọi bên
  nhận, và một khoá bị lộ có giá trị cả năm.
- **SPIFFE/SPIRE:** đúng mô hình danh tính workload ngắn hạn, nhưng thêm một hệ thống nữa trên mọi node
  của đối tác và khách; giữ làm hướng tương thích (định dạng SAN theo SPIFFE ID) để có thể chuyển sau.
- **Bắt enroll lại bằng tay khi cert hết hạn:** an toàn nhất nhưng không chịu nổi khi một sự cố mạng
  làm hàng trăm node offline quá 24 h cùng lúc.
