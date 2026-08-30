# control-plane

Chưa bắt đầu code — placeholder. Vai trò: lấy config từ `../data-plane` (subscribe outbox event
qua RabbitMQ), biên dịch (compile) thành rule-set mà `../edge-plane` hiểu được, phân phối
(distribute) xuống qua Redis/DragonflyDB. Không có UI, không phải CRUD.

Tên gợi ý cho worker chính: `waf-config-distributor` — xem đầy đủ luồng dữ liệu, lý do tách plane,
và các điểm còn chưa chốt (đặc biệt: `SecurityEvent` telemetry đi lên nên qua đây hay đi thẳng)
ở `../data-plane/docs/04-architecture-boundary.md`.
