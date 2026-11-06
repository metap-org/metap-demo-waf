# ADR-0020: ClickHouse cho hot search và số liệu gộp, Parquet trên object storage cho archive

- Trạng thái: Chấp nhận (cụ thể hoá [ADR-0010](0010-analytics-one-write-path-many-stores.md))
- Ngày: 2026-11-06

## Quyết định

- **ClickHouse** giữ hai vai trò của ADR-0010 bằng hai nhóm bảng:
  - *raw* (`security_events`, `access_logs`): từng request, `ORDER BY (tenant_id, zone_id, ts)`,
    skip index bloom filter trên `request_id`; retention theo từng dòng qua cột `expire_at` tính từ
    entitlements lúc ghi.
  - *gộp*: materialized view `AggregatingMergeTree` theo phút/giờ.
  - Tiered storage: SSD vài ngày đầu, sau đó chuyển sang volume S3.
- **Archive** dài hạn: Parquet theo tenant/giờ/region trên object storage, do archiver đọc từ
  Kafka ghi ra. Dùng cho yêu cầu pháp lý và replay.

## Lý do

- Một engine phục vụ cả tra cứu theo request ID lẫn số liệu gộp ở quy mô log HTTP; đây là cách
  nhiều nhà cung cấp CDN/WAF đang dùng.
- Retention theo tenant làm được bằng TTL trên cột, không cần bảng riêng cho từng gói.
- Apache-2.0, operator Kubernetes có sẵn, chạy được on-prem.

## Phương án đã cân nhắc

- **OpenSearch cho hot search**: tìm kiếm văn bản tốt hơn nhưng tốn tài nguyên ở quy mô access log;
  sản phẩm không cần full-text trên log. Elasticsearch bị loại vì license.
- **Hai engine riêng cho search và gộp**: thêm vận hành không cần thiết ở giai đoạn đầu.
