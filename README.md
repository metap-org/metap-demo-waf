# metap-demo-waf

Sản phẩm demo **WAAP** (Web Application & API Protection, kiểu Cloudflare) — build portal nhanh
trên nền `metap` (`../metap`), phần thực thi bảo vệ thật tách riêng. Repo chia 3 thư mục theo
đúng 3 vai trò:

| Thư mục | Vai trò | Trạng thái |
|---|---|---|
| [`data-plane/`](data-plane/) | Nơi giữ dữ liệu/config nguồn — portal nghiệp vụ (Zone, DDoS policy, firewall rule, vulnerability scan, incident, alert), build trên `metap` | Backend: cả 9 entity pillar 1-4 chạy được, test qua API thật. Frontend (`web/`, `@metap/platform-ui`): generic list/form/detail/workflow UI chạy được, test qua browser thật — còn thiếu portal IA riêng theo `docs/07` + logic custom (DNS verify, dashboard, correlation, alert gửi thật) |
| `control-plane/` | Lấy config từ `data-plane`, tính toán (compile) thành rule-set, đẩy (distribute) xuống `edge-plane` — worker nền, không UI | Chưa bắt đầu |
| `edge-plane/` | Thực thi mitigation thật tại biên (nhận traffic, evaluate rule, chặn/challenge) — hệ thống riêng, hiệu năng cao | Chưa bắt đầu |

Xem `data-plane/docs/04-architecture-boundary.md` cho luồng dữ liệu đầy đủ giữa 3 plane (config
đi xuống, telemetry đi lên) và lý do tách theo hướng này.

Bắt đầu đọc từ `data-plane/docs/01-product-vision.md`.
