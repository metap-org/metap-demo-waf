# ADR-0030: Chọn ngôn ngữ theo đặc tính tải của từng plane

- Trạng thái: Chấp nhận (thay thế [ADR-0012](0012-go-as-primary-language.md))
- Ngày: 2026-11-06

## Bối cảnh

ADR-0012 chọn Go cho mọi plane để đội chỉ cần một ngôn ngữ. Nhưng các plane có đặc tính tải khác hẳn
nhau. Edge xử lý từng request từ internet với ngân sách p99 ≤ 2 ms; GC của Go là rủi ro lớn nhất cho
mục tiêu đó ([11 R1](../11-risks/00-index.md#111-rủi-ro)). Các plane còn lại chủ yếu là I/O, logic
nghiệp vụ, khối lượng vừa phải; ở đó tốc độ phát triển và việc dùng chung `metap-go` quan trọng hơn.

## Quyết định

| Plane / thành phần | Ngôn ngữ | Lý do |
|---|---|---|
| **Edge** (gateway, policy agent, state sync) | **Rust** | Không GC nên độ trễ đuôi ổn định; an toàn bộ nhớ khi parse dữ liệu không tin cậy; hệ sinh thái proxy đã được kiểm chứng ở quy mô CDN ([ADR-0031](0031-edge-in-rust-on-pingora.md)) |
| **Replay worker** | Rust | Dùng chung crate `pipeline` với edge để mô phỏng chạy đúng code đang chặn traffic ([ADR-0026](0026-replay-on-request-samples-with-shared-pipeline.md)) |
| **Công cụ `edge-verify`** | Rust | Compiler và pipeline phát hành ruleset nạp thử snapshot bằng đúng loader của edge |
| Log shipper, ingest gateway | Vector (Rust, có sẵn) | [ADR-0019](0019-vector-as-node-log-shipper.md) |
| **Control** | Go | Logic điều phối, khối lượng nhỏ; `cel-go` cho kiểm tra rule ([ADR-0032](0032-cel-checked-at-control-ir-at-edge.md)) |
| **Analytics services** (enricher, query, detection, notifier, exporter) | Go | I/O-bound, scale ngang; enricher chuyển sang Rust chỉ khi profiling cho thấy chi phí CPU chiếm phần lớn |
| **Management** | Go (`metap-go`) | [ADR-0009](0009-metap-for-management-plane-only.md) |

Ranh giới giữa Rust và Go là ranh giới plane, giao tiếp qua protobuf/gRPC đã có
([05-contracts](../05-building-blocks/05-contracts.md)); không có FFI giữa Go và Rust.

## Hệ quả

- Hai ngôn ngữ chính. Edge vốn đã là codebase và chu kỳ deploy riêng, không phụ thuộc `metap`, nên
  ranh giới rõ.
- Org vừa bỏ Rust khỏi các repo khác (2026-11-01); đội edge cần người viết Rust thành thạo. Đây là
  rủi ro tuyển dụng và đào tạo ([11 R12](../11-risks/00-index.md#111-rủi-ro)).
- Kiểm tra rule viết bằng Go (`cel-go`) nhưng thực thi bằng Rust, nên phải có bộ test đối chiếu ngữ
  nghĩa chung ([ADR-0032](0032-cel-checked-at-control-ir-at-edge.md)).

## Phương án đã cân nhắc

- **C hoặc C++ cho edge:** hiệu năng tương đương Rust nhưng lỗi bộ nhớ trên dữ liệu không tin cậy từ
  internet thành lỗ hổng ngay trên đường traffic; chi phí kiểm định cao hơn nhiều so với lợi ích.
- **Go cho edge** (ADR-0012): một ngôn ngữ, nhưng GC làm khó giữ p99 ≤ 2 ms khi CRS cấp phát nhiều.
- **Elixir cho notifier:** BEAM rất hợp cho fan-out (mỗi kênh một process, supervisor tự khởi động
  lại, retry). Không chọn vì khối lượng thông báo nhỏ (hàng nghìn tin, không phải hàng triệu), Go xử
  lý thừa sức với thiết kế ở [5.8.9](../05-building-blocks/08-components/09-notification-fanout.md),
  còn thêm runtime thứ ba thì tốn vận hành và tuyển người. Xem lại khi: (a) cần đẩy event realtime tới
  hàng chục nghìn phiên portal cùng lúc (Phoenix Channels là lựa chọn mạnh), hoặc (b) số tin thông báo
  vượt mức một cụm Go nhỏ xử lý được.
