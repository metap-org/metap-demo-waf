# ADR-0031: Lựa chọn nền L7 edge và engine WAF

- Trạng thái: **Đề xuất, chờ chọn** sau spike (§6). Thay thế quyết định cũ ở [ADR-0013](0013-edge-gateway-custom-go-with-coraza.md), giờ là phương án D bên dưới.
- Ngày: 2026-11-06

## 1. Bối cảnh

Edge phải chạy pipeline 12 bước ([5.3](../05-building-blocks/03-edge-plane.md)) với độ trễ thêm
p99 ≤ 2 ms (QS-1), cô lập tài nguyên theo tenant, fail mode theo zone, nạp snapshot nguyên khối, và
chạy OWASP CRS ([ADR-0004](0004-managed-ruleset-crs-on-seclang.md)). Hai lựa chọn gắn với nhau:

- **Nền L7:** thứ nhận kết nối, TLS, HTTP và proxy tới origin; quyết định luôn ngôn ngữ của edge.
- **Engine WAF:** thứ chạy SecLang/CRS; không phải engine nào cũng ghép được với nền nào.

## 2. Tiêu chí

| # | Tiêu chí | Vì sao quan trọng | Trọng số |
|---|---|---|---|
| T1 | Độ trễ p99 và CPU mỗi request khi chạy CRS | QS-1, chi phí hạ tầng | Cao |
| T2 | Độ trung thành với CRS (tỉ lệ pass `go-ftw`) | Managed ruleset là lõi sản phẩm | Cao |
| T3 | An toàn bộ nhớ trên đường traffic | Parse dữ liệu không tin cậy từ internet | Cao |
| T4 | Mức kiểm soát pipeline (thứ tự phase, ngân sách thời gian, governor theo tenant, fail mode, swap snapshot) | Các yêu cầu sản phẩm ở 5.3, 8.5, QS-8 | Cao |
| T5 | Độ chín, cộng đồng, lịch sử vận hành ở quy mô lớn | Rủi ro vận hành | Trung bình |
| T6 | Kỹ năng đội | Org hiện chủ yếu viết Go; vừa bỏ Rust | Trung bình |
| T7 | Dùng lại code cho replay và `edge-verify` | [ADR-0026](0026-replay-on-request-samples-with-shared-pipeline.md) | Trung bình |
| T8 | HTTP/3 | Kỳ vọng của khách, không bắt buộc ở MVP | Thấp |
| T9 | License phát hành on-prem | [ADR-0024](0024-licensing-policy-for-shipped-components.md) | Điều kiện loại |

## 3. Phương án nền L7

| | A. Rust + Pingora | B. Envoy (C++) | C. NGINX / OpenResty (C + LuaJIT) | D. Go tự xây | E. HAProxy (C) |
|---|---|---|---|---|---|
| Cách gắn pipeline | Code Rust trong hook của Pingora | Filter: wasm, Lua, `ext_proc` (gọi service ngoài), hoặc filter C++ tự viết | Lua trong các phase của OpenResty; module C | Code Go trực tiếp | SPOE: gửi request sang agent ngoài process |
| T1 độ trễ | Tốt, không GC | Tốt ở proxy; wasm/`ext_proc` thêm chi phí mỗi request | Rất tốt ở proxy; Lua nhanh nhưng phần nặng là WAF | Có GC, rủi ro p99 khi CRS cấp phát nhiều | Rất tốt ở proxy; SPOE thêm một chặng IPC |
| T3 an toàn bộ nhớ | Cao (Rust) | Proxy C++ đã kiểm chứng, code mình viết trong wasm thì an toàn | Lõi C đã kiểm chứng; module C mình viết thì không | Cao (Go) | Lõi C đã kiểm chứng; agent viết bằng gì tuỳ |
| T4 kiểm soát pipeline | Cao | Trung bình: theo mô hình filter chain, governor theo tenant khó | Trung bình: theo các phase của NGINX | Cao | Thấp: logic chính nằm ngoài process |
| T5 độ chín | Pingora dùng ở quy mô Cloudflare, cộng đồng nhỏ hơn | Rất chín, hệ sinh thái lớn | Rất chín, phổ biến nhất | Thư viện chuẩn chín, nhưng proxy là tự viết | Rất chín |
| T6 kỹ năng | Cần người Rust | Cần C++ hoặc wasm | Cần Lua/C | Có sẵn | Cấu hình HAProxy + Go cho agent |
| T7 dùng lại cho replay | Có (crate) | Chỉ khi logic nằm trong wasm/`ext_proc` | Khó (Lua gắn với NGINX) | Có (package) | Có, nếu agent là service riêng |
| T8 HTTP/3 | Chưa xác nhận, có thể dùng `quinn` | Có | Có (NGINX mới) | Có (`quic-go`) | Có |

## 4. Phương án engine WAF

| | 1. libmodsecurity v3 | 2. Coraza | 3. SecLang thuần Rust | 4. Tự viết engine |
|---|---|---|---|---|
| Ngôn ngữ | C++ (OWASP duy trì; bản mới nhất 3.0.17, 9/2026) | Go (OWASP) | Rust (`zentinel-modsec`, `sentinel-modsec`, bản 0.x) | Theo nền |
| T2 CRS | Engine tham chiếu của CRS | Tương thích CRS đầy đủ, được CRS test | Tự công bố tương thích; chưa kiểm chứng độc lập | Phải tự đạt |
| T3 an toàn | Thấp: có lịch sử CVE lỗi bộ nhớ | Cao | Cao | Tuỳ |
| T5 độ chín | Cao (dùng rộng với NGINX) | Trung bình–cao | Thấp | Không có |
| Ghép được với | A (FFI), C (connector chính thức), E (agent) | D (nhúng), B (proxy-wasm), E (`coraza-spoa`), A (chỉ qua binding kéo runtime Go, không nên) | A | Mọi nền |

## 5. Tổ hợp khả thi

| Tổ hợp | Điểm mạnh | Điểm yếu | Nhận xét |
|---|---|---|---|
| **A1** Pingora + libmodsecurity | Kiểm soát pipeline cao, không GC, CRS tham chiếu | Code C++ trong process; cần người Rust | Ứng viên cho hiệu năng |
| **A3** Pingora + SecLang Rust | An toàn bộ nhớ toàn phần | Engine còn non | Hướng dài hạn của A1, không dùng cho MVP |
| **B2** Envoy + Coraza proxy-wasm | Proxy rất chín, HTTP/3 sẵn | Chi phí wasm mỗi request; governor theo tenant khó | Ứng viên cho độ chín |
| **C1** OpenResty + ModSecurity connector | Tổ hợp phổ biến nhất, nhiều người vận hành | Pipeline theo phase NGINX; module C; khó dùng lại cho replay | Mốc so sánh hiệu năng |
| **D2** Go tự xây + Coraza nhúng | Đội làm được ngay; một ngôn ngữ với Control; replay dùng chung | GC và rủi ro p99 | Ứng viên cho tốc độ ra MVP |
| **E2** HAProxy + `coraza-spoa` | Proxy rất nhanh và chín | WAF ngoài process, thêm một chặng; pipeline phân tán hai nơi | Ít hợp với yêu cầu pipeline |

## 6. Đề xuất: spike so sánh trước khi chọn

**Shortlist:** **A1**, **D2**, **B2**. **C1** chạy làm mốc so sánh, vì đó là cách làm phổ biến nhất.

Mỗi tổ hợp dựng một prototype tối thiểu, cùng điều kiện:

- Phase cần có: access list, rate limit, CRS, 10 custom rule, quyết định, proxy.
- Swap snapshot nóng.
- Governor theo tenant ở mức đơn giản.

| Đo | Cách đo | Ngưỡng để đạt |
|---|---|---|
| Độ trễ thêm | Corpus traffic tốt + tấn công, CRS PL1 và PL2, 1 node, tải 50% và 90% công suất | p99 ≤ 2 ms ở PL1, tải 50% |
| CPU mỗi 1.000 request | Cùng tải | Dùng để ước chi phí hạ tầng, không có ngưỡng cứng |
| CRS | `go-ftw` với bộ test chính thức của CRS v4 | ≥ 99% pass ở PL1–PL2 |
| Cô lập tenant | Một tenant gửi body lớn + rule nặng | p99 tenant khác tăng ≤ 10% (QS-8) |
| Swap snapshot | Đổi config dưới tải | Không lỗi, không tăng p99 quá 10% |
| Công sức | Ước người-tuần để có đủ 12 phase + governor + fail mode | Dùng để so sánh |
| Fuzz | 24 giờ fuzz lớp nhận request | Không crash |

Thời hạn đề xuất: 3 tuần, 2 người. Kết quả ghi thành bản sửa của ADR này, trạng thái chuyển sang
"Chấp nhận" với tổ hợp được chọn.

**Gợi ý nếu không làm spike:** chọn **D2** nếu ưu tiên ra MVP nhanh với đội hiện tại, chấp nhận rủi ro
p99 và giữ trait `WafEngine` để chuyển sang A1 sau. Chọn **A1** nếu QS-1 là cam kết bán hàng ngay từ đầu.

## 7. Phần không phụ thuộc lựa chọn

Những thứ sau đúng với mọi tổ hợp và đã được thiết kế ở [5.8.1](../05-building-blocks/08-components/01-edge-gateway.md)
và [5.8.2](../05-building-blocks/08-components/02-edge-agent-and-state.md):

- Thứ tự 12 phase, ngân sách thời gian, fail mode, governor theo tenant.
- Snapshot bất biến có chữ ký, swap nguyên khối ([ADR-0015](0015-snapshot-format-and-distribution.md)).
- Engine WAF đặt sau một interface để thay được.
- Telemetry qua Vector, request ID, trang chặn.

## 8. Ảnh hưởng tới ADR khác

| ADR | Phụ thuộc |
|---|---|
| [0030](0030-language-per-workload.md) | Ngôn ngữ edge theo tổ hợp được chọn: A → Rust, B → C++/wasm, C → Lua/C, D → Go |
| [0032](0032-cel-checked-at-control-ir-at-edge.md) | IR cần khi edge không phải Go. Nếu chọn D, edge có thể dùng thẳng `cel-go`; IR vẫn dùng được và giữ một nguồn ngữ nghĩa |
| [0026](0026-replay-on-request-samples-with-shared-pipeline.md) | Replay viết bằng ngôn ngữ của edge để dùng chung code pipeline |
