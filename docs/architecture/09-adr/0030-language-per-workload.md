# ADR-0030: Chọn ngôn ngữ theo đặc tính tải của từng plane

- Trạng thái: Chấp nhận cho Control, Analytics, Management; **phần edge chờ chọn** theo [ADR-0031](0031-edge-l7-and-waf-engine-options.md). Thay thế [ADR-0012](0012-go-as-primary-language.md)
- Ngày: 2026-11-06

## Bối cảnh

ADR-0012 chọn Go cho mọi plane để đội chỉ cần một ngôn ngữ. Nhưng các plane có đặc tính tải khác hẳn
nhau. Edge xử lý từng request từ internet với ngân sách p99 ≤ 2 ms; GC của Go là rủi ro lớn nhất cho
mục tiêu đó ([11 R1](../11-risks/00-index.md#111-rủi-ro)). Các plane còn lại chủ yếu là I/O, logic
nghiệp vụ, khối lượng vừa phải; ở đó tốc độ phát triển và việc dùng chung `metap-go` quan trọng hơn.

## Quyết định

| Plane / thành phần | Ngôn ngữ | Lý do |
|---|---|---|
| **Edge proxy** | **Chờ chọn** ([ADR-0031](0031-edge-l7-and-waf-engine-options.md)); shortlist Rust (Pingora), Go, Envoy (C++ + wasm) | Quyết định bằng spike |
| **WAF inspection** | Go mặc định (Coraza), so sánh libmodsecurity (C++) | Process riêng nên GC và rủi ro bộ nhớ của engine không lan sang proxy ([ADR-0033](0033-two-tier-edge-proxy-and-waf.md)) |
| **Replay worker** | Cùng ngôn ngữ với edge | Dùng chung code pipeline với edge để mô phỏng chạy đúng code đang chặn traffic ([ADR-0026](0026-replay-on-request-samples-with-shared-pipeline.md)) |
| **Công cụ `edge-verify`** | Cùng ngôn ngữ với edge | Compiler và pipeline phát hành ruleset nạp thử snapshot bằng đúng loader của edge |
| Log shipper, ingest gateway | Vector (Rust, có sẵn) | [ADR-0019](0019-vector-as-node-log-shipper.md) |
| **Control** | Go | Logic điều phối, khối lượng nhỏ; `cel-go` cho kiểm tra rule ([ADR-0032](0032-cel-checked-at-control-ir-at-edge.md)) |
| **Analytics services** (enricher, query, detection, notifier, exporter) | Go | I/O-bound, scale ngang; enricher chuyển sang Rust chỉ khi profiling cho thấy chi phí CPU chiếm phần lớn |
| **Management** | Go (`metap-go`) | [ADR-0009](0009-metap-for-management-plane-only.md) |

Nếu edge không viết bằng Go, ranh giới ngôn ngữ trùng ranh giới plane, giao tiếp qua protobuf/gRPC đã có
([05-contracts](../05-building-blocks/05-contracts.md)); không có FFI giữa Go và Rust.

## Hệ quả

- Không còn ràng buộc "một ngôn ngữ cho mọi plane": edge được chọn theo hiệu năng, phần còn lại giữ Go.
  Edge vốn đã là codebase và chu kỳ deploy riêng, không phụ thuộc `metap`, nên ranh giới rõ.
- Nếu chọn Rust cho edge: org vừa bỏ Rust khỏi các repo khác (2026-11-01), cần người viết Rust thành
  thạo ([11 R12](../11-risks/00-index.md#111-rủi-ro)).
- Nếu edge proxy không phải Go: kiểm tra rule bằng `cel-go` nhưng phần pre thực thi bằng ngôn ngữ khác, cần bộ test
  đối chiếu ngữ nghĩa ([ADR-0032](0032-cel-checked-at-control-ir-at-edge.md)).

## Phương án đã cân nhắc

- **Tự viết edge bằng C hoặc C++:** hiệu năng tương đương Rust nhưng code mới viết bằng C/C++ xử lý dữ
  liệu không tin cậy từ internet là nguồn lỗ hổng bộ nhớ. Khác với việc dùng lõi C/C++ đã được kiểm
  chứng nhiều năm (NGINX, Envoy, HAProxy), vẫn nằm trong các phương án của ADR-0031.
- **Elixir cho notifier:** BEAM rất hợp cho fan-out (mỗi kênh một process, supervisor tự khởi động
  lại, retry). Không chọn vì khối lượng thông báo nhỏ (hàng nghìn tin, không phải hàng triệu), Go xử
  lý thừa sức với thiết kế ở [5.8.11](../05-building-blocks/08-components/11-notification-fanout.md),
  còn thêm runtime thứ ba thì tốn vận hành và tuyển người. Xem lại khi: (a) cần đẩy event realtime tới
  hàng chục nghìn phiên portal cùng lúc (Phoenix Channels là lựa chọn mạnh), hoặc (b) số tin thông báo
  vượt mức một cụm Go nhỏ xử lý được.
