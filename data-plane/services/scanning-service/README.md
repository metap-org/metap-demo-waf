# scanning-service

**Dùng để làm gì**: quản lý cấu hình quét lỗ hổng (vulnerability scanning) cho từng zone — lịch
quét/kiểu quét (`ScanJob`) và kết quả từng lần quét (`ScanFinding`). Đây là trụ cột "Vulnerability
Scanning" trong 4 trụ cột sản phẩm (`../../docs/01-product-vision.md`).

**Không** tự chạy engine quét thật (DAST) — service này chỉ lưu **cấu hình** quét
(`scanType`/`schedule`) và **kết quả** (`ScanFinding`), giống hệt cách `edge-plane` thực thi
mitigation tách khỏi `data-plane` giữ config. Cái gì đó bên ngoài đọc `ScanJob.status: queued` rồi
tự chạy quét, đẩy kết quả vào qua route generic `POST /api/waf.scan_findings`, rồi chuyển trạng
thái `ScanJob` qua `start`/`complete`/`fail` (route generic `POST /api/waf.scan_jobs/:id/transition`).

## Entity sở hữu

| Entity | Dùng để làm gì |
|---|---|
| `waf.scan_jobs` | 1 cấu hình quét lặp lại cho 1 zone — kiểu quét (`quickScan`/`fullScan`/`apiScan`), lịch cron (tuỳ chọn), trạng thái lần chạy gần nhất (`idle↔queued↔running↔completed/failed`, lặp vô hạn, không có trạng thái "xong hẳn") |
| `waf.scan_findings` | 1 lỗ hổng cụ thể tìm được trong 1 lần chạy `ScanJob` — mức độ nghiêm trọng, endpoint, mô tả, workflow xử lý riêng (`open→confirmed→fixed`/`falsePositive`/`accepted`) |

## Quan hệ với `zones-service`

`ScanJob.zoneId` là **`String` thuần, không phải `Reference`** — `waf.zones` thuộc
`zones-service`, 1 binary khác; đăng ký `zone_entity()` ở đây chỉ để pass validate sẽ đồng thời
lộ CRUD `waf.zones` qua route generic của chính service này (xem
`src/entities/scan_job_entity.rs`'s doc comment). FE tự gọi `zones-service` để lấy `hostname`
hiển thị cho 1 `zoneId`, hoặc dùng GraphQL gateway (khi có) để gộp trong 1 query.

## Chạy

```bash
cp .env.example .env   # chỉnh DATABASE_URL/keys nếu cần
cargo run -p scanning-service   # từ data-plane/ (workspace root)
```

Mặc định `PORT=3010`, gRPC opt-in ở `GRPC_PORT=3011` (đặt `GRPC_ENABLED=true` để bật — dùng cho
`graphql-gateway` gộp cross-service, xem root README).
