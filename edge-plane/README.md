# edge-plane

Chưa bắt đầu code — placeholder. Vai trò: thực thi mitigation thật tại biên (nhận traffic,
evaluate rule DDoS L7/WAF/firewall, chặn/challenge/log request) — hệ thống hiệu năng cao,
latency thấp, cố tình không dùng `metap`/metadata-driven approach.

Chỉ đọc config đã được `../control-plane` tính toán sẵn (qua Redis/DragonflyDB) — không bao giờ
tự đọc thẳng `../data-plane` (Postgres). Xem `../data-plane/docs/04-architecture-boundary.md`
cho luồng dữ liệu đầy đủ.
