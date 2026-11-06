# ADR-0032: CEL được kiểm tra ở Control, edge thực thi IR đã biên dịch

- Trạng thái: **Đề xuất**, theo kết quả [ADR-0031](0031-edge-l7-and-waf-engine-options.md): custom rule phần pre chạy ở edge proxy ([ADR-0034](0034-pre-filter-and-rule-split-across-tiers.md)) nên cần IR nếu proxy không viết bằng Go; phần inspect chạy ở tầng WAF (mặc định Go) có thể dùng thẳng `cel-go`. Sửa đổi [ADR-0014](0014-cel-as-rule-language.md)
- Ngày: 2026-11-06

## Bối cảnh

ADR-0014 chọn CEL vì `cel-go` có kiểm tra kiểu và ước lượng chi phí. Edge có thể không viết bằng Go
([ADR-0031](0031-edge-l7-and-waf-engine-options.md)); thư viện CEL cho Rust, C++, Lua kém trưởng thành hơn `cel-go`,
và để hai implementation CEL tự diễn giải cùng một text thì rủi ro lệch ngữ nghĩa.

## Quyết định

- Khách vẫn viết CEL (builder ↔ text không đổi).
- **Control** (Go, `cel-go`) parse, check kiểu, ước lượng chi phí, rồi **hạ checked AST xuống IR** của
  riêng mình: cây biểu thức có kiểu gồm hằng, tham chiếu field, phép so sánh và logic, `in`, các hàm
  trong danh sách cho phép (chuỗi, IP/CIDR, geo, ASN, regex). IR nằm trong bundle zone dạng protobuf.
- **Edge** chỉ thực thi IR, không parse CEL. Regex dùng engine ngữ nghĩa kiểu RE2 (thời gian tuyến
  tính) ở mọi ngôn ngữ, cùng họ với RE2 mà `cel-go` dùng.
- **Bộ test đối chiếu chung:** tập vector `(biểu thức, input, kết quả mong đợi)` chạy trên cả `cel-go`
  lẫn evaluator của edge trong CI; lệch là lỗi chặn merge ở cả hai repo.

## Hệ quả

- Một nguồn ngữ nghĩa (Control); edge đơn giản và nhanh hơn vì không có giai đoạn parse.
- Thêm một hàm vào ngôn ngữ rule phải làm ở cả hai phía và thêm vector test.
