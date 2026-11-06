# ADR-0018: Apache Kafka làm ingestion bus

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

Apache Kafka chế độ KRaft. Topic theo loại bản ghi (`telemetry.security.v1`,
`telemetry.access.v1`, `telemetry.sample.v1`, `telemetry.enriched.v1`, `node.status.v1`),
partition theo khoá `tenant_id`, giữ 3 ngày để consumer chậm và chạy lại đuổi kịp.

## Lý do

- Băng thông và độ bền đã được kiểm chứng ở quy mô log HTTP; consumer group độc lập cho từng loại
  đọc (enricher, archiver, exporter, detector).
- Apache-2.0, chạy được on-prem; operator Strimzi trên Kubernetes.

## Phương án đã cân nhắc

- **Redpanda**: vận hành đơn giản hơn nhưng license BSL, không phát hành on-prem được.
- **NATS JetStream**: nhẹ, nhưng hệ sinh thái cho throughput log lớn và công cụ kết nối kém hơn.
- **Pulsar**: mạnh nhưng nhiều thành phần hơn đáng kể.
