# ADR-0013: Edge gateway tự xây bằng Go, nhúng Coraza

- Trạng thái: Mở lại thành phương án **D2** trong [ADR-0031](0031-edge-l7-and-waf-engine-options.md) (chờ chọn)
- Ngày: 2026-11-06

## Bối cảnh

Edge cần pipeline 12 bước theo thứ tự sản phẩm, ngân sách thời gian từng bước, fail mode theo zone,
cô lập tài nguyên theo tenant, và nạp snapshot nguyên khối. Proxy đa dụng không có sẵn những thứ
này; mở rộng chúng thì bị giới hạn bởi mô hình plugin.

## Quyết định

- Gateway tự xây trên thư viện chuẩn Go (`net/http`, `crypto/tls`, `httputil.ReverseProxy`) và
  `quic-go` cho HTTP/3.
- **Coraza v3** nhúng trong process làm engine SecLang, chạy **OWASP CRS v4**. Một instance WAF
  được dựng sẵn cho mỗi tổ hợp (ruleset release, cấu hình CRS của zone) lúc nạp snapshot; mỗi request
  là một transaction.
- Regression cho ruleset dùng `go-ftw` (bộ test chính thức của CRS) cộng corpus riêng.

## Hệ quả

- Toàn quyền với thứ tự pipeline, ngân sách thời gian, cô lập tenant.
- Phải tự lo những thứ proxy có sẵn: connection pool tới origin, retry, health check, giới hạn
  header/body, HTTP/2 tuning. Phạm vi này nhỏ và đã có thư viện chuẩn.
- Coraza là phần đắt nhất trong ngân sách độ trễ; benchmark là việc đầu tiên của giai đoạn MVP.
- Lối thoát nếu benchmark không đạt: đặt Envoy phía trước làm TLS/HTTP, gateway Go chạy sau qua
  `ext_proc`; pipeline giữ nguyên.

## Phương án đã cân nhắc

- **Envoy + Coraza proxy-wasm**: chi phí wasm cho mỗi request, khó cô lập tài nguyên theo tenant.
- **Caddy + module Coraza**: dựng nhanh, nhưng pipeline bị gò theo mô hình module của Caddy.
- **libmodsecurity**: C++, cần cgo; dự án đã chuyển giao sang OWASP sau khi Trustwave ngừng hỗ trợ.
