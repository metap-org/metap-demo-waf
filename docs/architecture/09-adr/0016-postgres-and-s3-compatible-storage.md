# ADR-0016: PostgreSQL cho trạng thái điều khiển, object storage chuẩn S3 cho artifact

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- **PostgreSQL** cho Management (qua `metap`), config store, fleet registry, trạng thái rollout,
  trạng thái exporter và detection. Mỗi plane một database riêng, không dùng chung bảng.
- **Object storage chuẩn S3** cho artifact snapshot, archive Parquet, mẫu request. SaaS dùng dịch
  vụ S3-compatible bất kỳ; tự host và on-prem dùng **SeaweedFS**.

## Lý do

- Org đã vận hành PostgreSQL; khối lượng trạng thái điều khiển nhỏ, cần transaction.
- Chỉ dùng API S3 chung, không tính năng riêng của một cloud.
- MinIO bản OSS đã bị lưu trữ (2026-04) và dùng AGPL, nên không chọn.

## Hệ quả

- Rollout orchestrator dùng hàng đợi `FOR UPDATE SKIP LOCKED` trên PostgreSQL, cùng pattern
  `metap-cron` đã có.
