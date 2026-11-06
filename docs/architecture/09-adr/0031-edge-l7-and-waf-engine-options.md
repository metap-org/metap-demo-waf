# ADR-0031: Lựa chọn nền cho tầng edge proxy và engine cho tầng WAF

- Trạng thái: **Đề xuất, chờ chọn** sau spike (§7). Phương án cũ ở [ADR-0013](0013-edge-gateway-custom-go-with-coraza.md) nằm trong các lựa chọn dưới đây.
- Ngày: 2026-11-06

## 1. Bối cảnh

Edge có hai tầng ([ADR-0033](0033-two-tier-edge-proxy-and-waf.md)), nên có **ba lựa chọn**:

1. **Nền cho tầng proxy:** kết nối, TLS, pre-filter, challenge, cache, proxy tới origin. Tốn RAM, kết
   nối, băng thông.
2. **Engine cho tầng WAF:** chạy SecLang/CRS và custom rule phần inspect. Tốn CPU.
3. **Cách proxy gọi WAF:** giao thức kiểm tra giữa hai tầng.

Hai tầng là hai process riêng, nên hai lựa chọn đầu gần như độc lập: proxy viết bằng một ngôn ngữ, WAF
có thể viết bằng ngôn ngữ khác.

## 2. Tiêu chí

| # | Tiêu chí | Áp cho | Trọng số |
|---|---|---|---|
| T1 | Độ trễ p99, bộ nhớ mỗi kết nối, CPU mỗi request | Cả hai | Cao |
| T2 | Độ trung thành với CRS (`go-ftw`) | WAF | Cao |
| T3 | An toàn bộ nhớ trên đường traffic | Proxy (nhận traffic internet) cao hơn WAF (đã cô lập process) | Cao |
| T4 | Mức kiểm soát pre-filter, challenge, governor theo tenant, fail mode, swap snapshot | Proxy | Cao |
| T5 | **Khả năng cache** (RAM + đĩa, purge, cache key) | Proxy | Cao ([ADR-0035](0035-cdn-and-dns-strategy.md)) |
| T6 | **Gọi dịch vụ kiểm tra ngoài kèm body** (stream, timeout, fail mode) | Proxy | Cao |
| T7 | Độ chín, vận hành ở quy mô lớn | Cả hai | Trung bình |
| T8 | Kỹ năng đội (chủ yếu Go) | Cả hai | Trung bình |
| T9 | Dùng lại code cho replay và `edge-verify` | Cả hai | Trung bình |
| T10 | HTTP/3 | Proxy | Thấp |
| T11 | License phát hành on-prem | Cả hai | Điều kiện loại |

## 3. Nền cho tầng proxy

| | P1. Rust + Pingora | P2. Envoy | P3. NGINX / OpenResty | P4. HAProxy | P5. Go tự xây |
|---|---|---|---|---|---|
| T1 | Tốt, không GC | Tốt | Rất tốt | Rất tốt | Tốt; GC ít ảnh hưởng hơn trước vì CRS đã ra khỏi process |
| T3 | Cao | Lõi C++ đã kiểm chứng; logic của mình trong wasm/Lua | Lõi C đã kiểm chứng; logic Lua | Lõi C đã kiểm chứng | Cao |
| T4 | Cao (code của mình) | Trung bình (filter chain) | Trung bình (phase của NGINX, Lua) | Thấp–trung bình (Lua, cấu hình) | Cao |
| T5 cache | Có (`pingora-cache`), cần tự hoàn thiện purge, tầng đĩa | Yếu (cache filter cơ bản) | **Rất mạnh** (`proxy_cache`, đĩa, purge qua module) | Yếu (cache nhỏ trong RAM) | Phải tự viết |
| T6 gọi WAF | Tự viết client gRPC | **Có sẵn** (`ext_proc`) | Yếu (không có cơ chế gửi body sang dịch vụ ngoài sẵn) | **Có sẵn** (SPOE) | Tự viết client gRPC |
| T7 | Dùng ở quy mô Cloudflare; cộng đồng nhỏ | Rất chín | Rất chín, phổ biến nhất | Rất chín | Proxy tự viết |
| T8 | Cần người Rust | C++ / wasm | Lua | Cấu hình + Lua | Có sẵn |
| T10 | Chưa xác nhận | Có | Có | Có | Có (`quic-go`) |

Nhận xét: không nền nào mạnh cả về cache lẫn gọi WAF sẵn. P3 mạnh nhất về cache nhưng yếu về gọi dịch
vụ kiểm tra; P2/P4 ngược lại. P1 và P5 phải tự viết cả hai nhưng kiểm soát hoàn toàn.

## 4. Engine cho tầng WAF

| | W1. Coraza (Go) | W2. libmodsecurity v3 (C++) | W3. SecLang thuần Rust |
|---|---|---|---|
| T2 | Tương thích CRS đầy đủ, được CRS test | Engine tham chiếu của CRS | Tự công bố tương thích (`zentinel-modsec`, `sentinel-modsec`, 0.x), chưa kiểm chứng độc lập |
| T1 | Có GC, nhưng giờ chỉ ảnh hưởng tầng WAF | Nhanh; cần wrapper (Rust/Go/C++) làm dịch vụ gRPC | Chưa có số liệu độc lập |
| T3 | Cao | Thấp, nhưng đã cô lập trong process riêng nên crash không kéo theo proxy | Cao |
| T7 | Trung bình–cao; có sẵn `coraza-spoa` cho HAProxy, proxy-wasm cho Envoy | Cao (dùng rộng với NGINX) | Thấp |
| T8 | Có sẵn (Go) | Cần C++ cho wrapper hoặc FFI | Cần Rust |
| T9 | Replay chạy cùng engine dễ | Cần wrapper chung | Như W1 nếu replay viết Rust |

Vì tầng WAF đã tách process, nhược điểm lớn nhất của W1 (GC) và W2 (an toàn bộ nhớ) đều giảm nhiều.
**W1 là mặc định hợp lý**; W2 đưa vào spike để so CPU mỗi request, vì CPU tầng WAF là chi phí hạ tầng lớn.

## 5. Cách proxy gọi WAF

| | G1. Inspection API riêng (gRPC stream) | G2. Envoy `ext_proc` | G3. HAProxy SPOE |
|---|---|---|---|
| Mô tả | Proto của mình ([5.5](../05-building-blocks/05-contracts.md#inspection-api-edge-proxy--waf)) | Giao thức có sẵn của Envoy | Giao thức có sẵn của HAProxy |
| Ưu | Độc lập nền proxy; đúng thứ mình cần (verdict sớm, ngân sách thời gian) | Không phải viết client | Không phải viết client; có `coraza-spoa` |
| Nhược | Phải viết client ở proxy | Gắn với Envoy | Gắn với HAProxy; giới hạn kích thước body |

Đề xuất: **G1 là hợp đồng chuẩn**. Nếu chọn P2 hoặc P4, viết adapter nhỏ ở phía WAF nhận `ext_proc`
hoặc SPOE rồi chuyển sang G1, để tầng WAF chỉ có một giao diện.

## 6. Tổ hợp và shortlist

| Tổ hợp | Proxy | WAF | Gọi | Mạnh | Yếu |
|---|---|---|---|---|---|
| **S1** | P2 Envoy | W1 Coraza | G2 → G1 | Proxy rất chín, tích hợp kiểm tra sẵn, HTTP/3 | Cache yếu: cần thêm tầng cache (ví dụ Varnish) hoặc chờ CDN tự xây |
| **S2** | P1 Pingora | W1 Coraza (hoặc W2) | G1 | Kiểm soát cao, không GC ở proxy, có nền cache | Cần người Rust; tự viết client, purge, cache đĩa |
| **S3** | P5 Go | W1 Coraza | G1 | Đội làm được ngay; replay dùng chung code cả hai tầng | Tự viết cache và proxy |
| **S4** | P4 HAProxy | W1 Coraza | G3 (`coraza-spoa`) | Proxy rất nhanh và chín, tích hợp WAF có sẵn | Cache yếu; pre-filter phải viết bằng Lua/cấu hình |
| **S5** | P3 NGINX | W1 Coraza | Module/njs tự viết → G1 | Cache mạnh nhất, phổ biến nhất | Gọi WAF kèm body không có sẵn; pre-filter bằng Lua |

**Shortlist cho spike:** S2, S3, S1. S5 chạy làm mốc cho phần cache, S4 làm mốc cho phần gọi WAF.
Tầng WAF dùng W1 trong mọi tổ hợp; spike đo thêm W2 trên cùng tầng WAF để so CPU.

## 7. Spike

Thời hạn 3 tuần, 2 người. Mỗi tổ hợp dựng tối thiểu:

- Ở proxy: access list, rate limit, challenge, custom rule phần pre, cache cơ bản.
- Gọi WAF với CRS PL1/PL2 và custom rule phần inspect.
- Swap snapshot nóng, governor theo tenant đơn giản.

| Đo | Ngưỡng để đạt |
|---|---|
| Độ trễ thêm, request đi qua WAF, CRS PL1, tải 50% | p99 ≤ 2 ms |
| Độ trễ thêm, request bị chặn ở pre-filter hoặc cache hit được miễn WAF | p99 ≤ 0,5 ms |
| Tỉ lệ request bị loại ở proxy trong kịch bản flood từ ít nguồn | ≥ 95%, CPU tầng WAF không tăng quá 20% |
| `go-ftw`, CRS v4 | ≥ 99% pass ở PL1–PL2 |
| Cô lập tenant (QS-8) | p99 tenant khác tăng ≤ 10% |
| Cache: hit ratio trên corpus static, purge theo URL tới mọi node | Đo và so sánh; purge ≤ 5 s trong PoP |
| Bộ nhớ mỗi 10.000 kết nối giữ mở | Đo và so sánh |
| CPU tầng WAF mỗi 1.000 request (W1 vs W2) | Đo và so sánh |
| Fuzz proxy 24 giờ | Không crash |
| Công sức tới đủ tính năng MVP | Ước người-tuần |

Kết quả ghi thành bản sửa của ADR này, trạng thái chuyển sang "Chấp nhận".

**Gợi ý nếu không làm spike:** **S3** (Go cả hai tầng) để ra MVP nhanh với đội hiện tại, giữ G1 làm
hợp đồng để sau thay proxy bằng P1 mà không đụng tầng WAF; **S2** nếu độ trễ đuôi của proxy và hiệu quả
RAM là cam kết bán hàng ngay từ đầu.

## 8. Ảnh hưởng tới ADR khác

| ADR | Phụ thuộc |
|---|---|
| [0030](0030-language-per-workload.md) | Ngôn ngữ proxy theo P đã chọn; tầng WAF mặc định Go (W1) |
| [0032](0032-cel-checked-at-control-ir-at-edge.md) | Custom rule phần pre chạy ở proxy: cần IR nếu proxy không phải Go; phần inspect chạy ở WAF (Go) có thể dùng `cel-go` |
| [0026](0026-replay-on-request-samples-with-shared-pipeline.md) | Replay chạy code của cả hai tầng (thư viện, hoặc binary ở chế độ offline nếu khác ngôn ngữ) |
| [0035](0035-cdn-and-dns-strategy.md) | T5 (cache) nặng hay nhẹ tuỳ chiến lược CDN được chọn |
