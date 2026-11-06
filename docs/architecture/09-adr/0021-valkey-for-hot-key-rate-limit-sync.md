# ADR-0021: Valkey theo region để đồng bộ counter của hot key

- Trạng thái: Chấp nhận (cụ thể hoá [ADR-0011](0011-approximate-fleet-rate-limiting.md))
- Ngày: 2026-11-06

## Quyết định

- Mỗi node đếm cục bộ mọi khoá. Chỉ khoá vượt một tỉ lệ ngưỡng (ví dụ 10%) mới được coi là **hot**.
- Mỗi chu kỳ (mặc định 1 s), state sync gửi phần tăng của hot key lên cụm **Valkey của region**
  bằng pipeline `INCRBY` + `EXPIRE` theo cửa sổ, nhận lại tổng toàn region.
- Quyết định chặn dùng `max(cục bộ, tổng region gần nhất + cục bộ chưa gửi)`.
- Đồng bộ giữa các region không làm ở MVP; một khoá tấn công rải nhiều region được đếm theo từng region.

## Lý do

- Số khoá cần đồng bộ nhỏ (chỉ hot key), nên tải lên Valkey nhỏ và cố định theo chu kỳ.
- Valkey là bản fork BSD-3 của Redis; Redis đã đổi license.

## Hệ quả

- Mất Valkey: rơi về đếm cục bộ, không chặn nhầm thêm.
- Hybrid: khách cần chạy Valkey nếu muốn rate limit toàn cụm node của họ.
