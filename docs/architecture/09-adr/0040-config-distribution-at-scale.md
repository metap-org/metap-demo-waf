# ADR-0040: Phân phối cấu hình ở quy mô 1 triệu zone

- Trạng thái: Chấp nhận (sửa đổi [ADR-0015](0015-snapshot-format-and-distribution.md))
- Ngày: 2026-11-06

## Bối cảnh

Ước lượng ở [7.5](../07-deployment/05-capacity-and-sharding.md) cho thấy ba điểm:

- Runtime của mọi zone cộng lại khoảng 80 GB, cert khoảng 8 GB, trong khi mỗi PoP chỉ có 10–30% zone
  thật sự có traffic.
- Một manifest liệt kê mọi zone dài 40 MB mỗi lần ký, với khoảng 2 thay đổi/s.
- ACK theo từng zone trên từng node là hàng trăm triệu trạng thái.

Thiết kế "mọi node nhận mọi zone qua một manifest" của ADR-0015 không đứng được ở quy mô này.

## Quyết định

- **Manifest hai tầng:**
  - 4.096 bucket theo hash `zone_id`;
  - bucket manifest liệt kê digest bundle của các zone trong bucket;
  - top manifest liệt kê digest của mọi bucket (≈ 128 KB), ký Ed25519;
  - thay đổi được gom theo cửa sổ 1 s (fast lane có cửa sổ riêng).
- **Routing index** (hostname, cổng → zone, cell, cert, digest), khoảng 200 MB, nằm trong RAM của mọi
  node; cập nhật bằng delta theo bucket.
- **PoP config store:** mỗi PoP giữ một bản sao đủ trên NVMe của bundle và cert đã mã hoá (khoảng 16 GB)
  cho các cell PoP phục vụ, đồng bộ theo bucket manifest qua regional relay.
- **Node nạp khi cần:**
  - runtime của zone được dựng khi có request đầu tiên, lấy dữ liệu từ PoP store (khoảng 1 ms);
  - giữ trong LRU theo ngân sách RAM;
  - làm nóng trước theo danh sách zone nóng mà Analytics tính mỗi giờ;
  - zone nóng được dựng lại ngay khi đổi version, zone nguội dựng lại ở request kế tiếp.
- **ACK theo version của top manifest**, tăng đơn điệu; độ phủ của một zone suy ra từ version tổng.
- **Instance CRS dùng chung theo profile**; rule tắt áp bằng lọc kết quả, chỉ exclusion theo trường mới
  cần instance riêng. Khi instance riêng chưa sẵn, kiểm bằng profile chuẩn kèm cờ `waf_profile_fallback`.

## Hệ quả

- RAM của node phụ thuộc vào tập zone đang hoạt động, không phụ thuộc tổng số zone (QS-24).
- Request đầu tiên của một zone nguội trên một node chậm thêm vài ms (QS-25); làm nóng trước giảm việc
  này cho zone có traffic.
- PoP store trở thành thành phần trọng yếu của PoP: chạy dự phòng; node vẫn lấy được trực tiếp từ
  regional relay khi PoP store lỗi.
- Self-test ở compiler kiểm theo bundle zone như trước; thêm kiểm tính nhất quán giữa routing index và
  bucket manifest.

## Phương án đã cân nhắc

- **Đẩy đủ vào RAM mọi node:** đơn giản, nhưng tốn khoảng 90 GB RAM mỗi node cho dữ liệu phần lớn không dùng.
- **Đẩy đủ xuống đĩa mọi node, đọc khi cần** (mô hình KV nhân bản toàn phần): không cần PoP store, nhưng
  nhân 15–20 lần dung lượng đĩa và băng thông đồng bộ trong mỗi PoP. Một PoP store mỗi PoP đủ dùng, vì
  lấy dữ liệu trong PoP chỉ tốn khoảng 1 ms.
- **Chỉ đẩy zone tới PoP đang có traffic của zone đó:** tiết kiệm hơn nữa, nhưng traffic thay đổi theo
  giờ và khi chuyển PoP (sự cố, tấn công) thì PoP mới phải kéo dữ liệu từ xa vào đúng lúc tải cao.
