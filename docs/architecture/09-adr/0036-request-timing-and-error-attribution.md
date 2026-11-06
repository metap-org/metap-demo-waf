# ADR-0036: Đo thời gian theo chặng và quy mỗi response về một nguồn

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Bối cảnh

Khi khách báo "site chậm" hay "bị lỗi 502", cả khách lẫn support phải trả lời được ngay: chậm ở chặng
nào (client, WAAP, mạng tới origin, website), và lỗi do website hay do WAAP. Nếu không phân biệt được,
mọi sự cố của website sẽ bị đổ cho WAAP. Ngược lại, lỗi của WAAP cũng có thể bị che đi trong số liệu
lỗi chung.

## Quyết định

- L7 ghi **mốc thời gian theo chặng** cho mọi request ([8.14 §2](../08-cross-cutting/14-latency-and-error-attribution.md#2-điểm-đo-và-chỉ-số)).
  Các chặng gồm:
  - TLS và RTT phía client;
  - từng phase pre-filter;
  - WAF (tách hàng đợi, engine, mạng);
  - cache;
  - kết nối, TLS và TTFB phía origin;
  - truyền body về client.
- Chỉ số **`edge_us`** là thời gian do WAAP gây ra, không gồm thời gian chờ origin và chờ client. SLI độ
  trễ của NCC dùng chỉ số này.
- Mọi response mang **`status_source`**, một trong các giá trị: `origin`, `cache`, `edge_policy`,
  `edge_request`, `client_abort`, `origin_unreachable`, `edge_platform`. Response do WAAP tự sinh thay
  origin mang thêm **`edge_error`** theo bảng mã cố định.
- Client nhận **status chuẩn** (502/503/504), kèm header `X-WAAP-Error` và mã trên trang lỗi. Không dùng
  mã ngoài chuẩn kiểu 52x.
- SLI lỗi của NCC chỉ đếm `edge_platform`.
- `Server-Timing` là tuỳ chọn theo zone, mặc định tắt. RUM và probe cho zone của khách để ở giai đoạn Mở rộng.

## Hệ quả

- Bản ghi telemetry dài thêm khoảng 20 trường số; MV thêm quantile state theo chặng.
- Trace một request trong portal hiển thị được waterfall thật, không phải ước lượng.
- Bảng mã `edge_error` là hợp đồng với khách (công cụ giám sát của họ đọc header). Chỉ được thêm mã,
  không đổi nghĩa mã cũ.

## Phương án đã cân nhắc

- **Mã status riêng 520–530 như một số nhà cung cấp CDN:** khách nhìn status là biết ngay, nhưng đây là
  mã ngoài chuẩn HTTP, một số thư viện và công cụ giám sát xử lý sai. Chọn status chuẩn kèm header.
- **Chỉ đo tổng thời gian:** không trả lời được "chậm ở đâu", và lỗi của website sẽ bị tính vào WAAP.
