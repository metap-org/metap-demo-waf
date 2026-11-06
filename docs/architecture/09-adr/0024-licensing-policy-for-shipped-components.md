# ADR-0024: Chính sách license cho thành phần phát hành

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

Thành phần phát hành tới khách (on-prem, hybrid, edge trên hạ tầng đối tác) chỉ dùng license
permissive (Apache-2.0, MIT, BSD, ISC, PostgreSQL) hoặc weak copyleft cấp file (MPL-2.0).
**Không** dùng BSL, SSPL, Elastic License, AGPL trong bản phát hành.

| Bị loại | Thay bằng |
|---|---|
| Redis (RSAL/SSPL/AGPL) | Valkey |
| Vault (BSL) | OpenBao |
| Redpanda (BSL) | Apache Kafka |
| Elasticsearch (SSPL/ELv2/AGPL) | ClickHouse (hoặc OpenSearch nếu cần full-text) |
| MinIO (AGPL, bản OSS đã lưu trữ) | SeaweedFS |
| Grafana (AGPL) trong bản on-prem | Dashboard trong portal; Grafana chỉ dùng nội bộ NCC |

## Hệ quả

- Mỗi dependency mới phải qua kiểm tra license trong CI (ví dụ `go-licenses` cho Go module).
- Dữ liệu (geo/ASN, threat feed) xét riêng theo hợp đồng ([ADR-0025](0025-geo-asn-data-commercial-license.md)).
