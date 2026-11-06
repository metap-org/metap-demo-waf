# ADR-0014: CEL là ngôn ngữ custom rule

- Trạng thái: Chấp nhận (chốt biến thể cú pháp của [ADR-0003](0003-rule-language-existing-syntax-with-builder.md))
- Ngày: 2026-11-06

## Bối cảnh

ADR-0003 chọn cú pháp có sẵn, builder ↔ text qua AST. Hai ứng viên: họ cú pháp wirefilter (quen
với người dùng WAF nhờ Cloudflare) và CEL (Common Expression Language).

## Quyết định

Dùng **CEL** với thư viện `cel-go`, môi trường khai báo tập biến đóng (`http.request.*`, `ip.*`,
`geo.*`, `bot.*`) và hàm riêng (`ip.inCidr`, `ip.inList`, `geo.country`, `matchesRegex` có giới hạn).

```text
http.request.uri.path.startsWith("/admin") && !ip.inList(ip.src, "office") && geo.country(ip.src) != "VN"
```

## Lý do

- Có kiểm tra kiểu và **ước lượng chi phí** sẵn trong `cel-go`: validator chặn rule quá đắt theo
  entitlements ngay ở Control.
- Không Turing-complete, luôn dừng; được dùng ở Kubernetes, Envoy, Google Cloud nên có tài liệu và
  công cụ.
- Checked AST tuần tự hoá được dạng protobuf: Control kiểm tra một lần, edge nạp AST đã kiểm.
- Không phải tự viết và duy trì parser.

## Hệ quả

- Builder sinh text CEL; text CEL quá phức tạp hiển thị ở chế độ chỉ text.
- Khách quen cú pháp Cloudflare cần bảng đối chiếu; có thể thêm trình chuyển đổi một chiều từ
  wirefilter sang CEL cho migration.

## Phương án đã cân nhắc

- **Wirefilter**: quen thuộc hơn với khách WAF, nhưng không có bản Go chính thức; phải tự viết
  parser, kiểm tra kiểu, ước lượng chi phí.
