# ADR-0031: Edge viết bằng Rust trên Pingora, engine WAF sau một trait

- Trạng thái: Chấp nhận (thay thế [ADR-0013](0013-edge-gateway-custom-go-with-coraza.md); chốt phần còn mở của [ADR-0004](0004-managed-ruleset-crs-on-seclang.md))
- Ngày: 2026-11-06

## Quyết định

- **Khung proxy:** Pingora (Apache-2.0) cho listener, TLS, HTTP/1.1, HTTP/2, pool kết nối tới origin,
  khởi động lại không rớt kết nối. Pipeline 12 bước là code của mình chạy trong các hook của Pingora.
- **HTTP/3:** chưa xác nhận Pingora hỗ trợ; nếu chưa thì dùng `quinn` + `h3` cho listener HTTP/3 riêng,
  đổ vào cùng pipeline, hoặc để HTTP/3 sau MVP. Cần kiểm tra trong spike đầu tiên.
- **TLS:** rustls hoặc BoringSSL qua Pingora; chọn theo kết quả benchmark và yêu cầu FIPS của khách.
- **Engine WAF đặt sau trait `WafEngine`**, không gắn chặt vào một thư viện:
  - **MVP: libmodsecurity v3** (OWASP duy trì, engine tham chiếu của CRS) qua FFI, chạy OWASP CRS v4.
  - **Đánh giá song song** engine SecLang thuần Rust (ví dụ `zentinel-modsec`, `sentinel-modsec`,
    hiện ở bản 0.x). Một engine chỉ được thay vào khi qua 100% bộ test CRS chính thức (`go-ftw`) ở các
    paranoia level hỗ trợ và benchmark không kém hơn.
- **Custom rule:** Rust thực thi IR do Control biên dịch từ CEL ([ADR-0032](0032-cel-checked-at-control-ir-at-edge.md)).
- **Snapshot nóng:** `arc-swap` cho runtime bất biến; request đang chạy giữ bản cũ.

## Hệ quả

- Không còn rủi ro GC; cấp phát trên đường request kiểm soát được (arena, buffer tái sử dụng).
- libmodsecurity là code C++ trong process, có lịch sử CVE lỗi bộ nhớ. Giảm thiểu: pin version và quy
  trình vá nhanh, fuzz qua FFI, giới hạn kích thước và thời gian kiểm tra body; trait `WafEngine` để
  thay bằng engine thuần Rust khi đủ chín.
- Phải tự viết hook cho những thứ Pingora không có sẵn (pipeline, governor theo tenant, challenge).

## Phương án đã cân nhắc

- **Go + Coraza nhúng** (ADR-0013): một ngôn ngữ với Control, nhưng GC; bị thay bởi [ADR-0030](0030-language-per-workload.md).
- **Binding Rust của Coraza:** kéo runtime Go vào process Rust, mất lợi ích chính của việc chuyển.
- **Envoy hoặc NGINX + ModSecurity:** mô hình plugin gò bó pipeline, khó cô lập tài nguyên theo tenant.
- **Tự viết engine SecLang ngay từ đầu:** khối lượng lớn; để mở qua trait thay vì làm trước.
