# 5.7 Bản đồ công nghệ

Lựa chọn công nghệ cho từng thành phần ở level 2. Mỗi dòng có ADR giải thích lý do và phương án đã
cân nhắc. Detail design mức C3 ở [08-components/](08-components/00-index.md).

Hai tiêu chí chạy xuyên suốt:

1. **Một ngôn ngữ chính: Go** ([ADR-0012](../09-adr/0012-go-as-primary-language.md)). Org đã chuyển
   toàn bộ backend sang Go; edge và replay dùng chung thư viện pipeline.
2. **Mọi thứ phải đóng gói được cho on-prem mà không vướng license**
   ([ADR-0024](../09-adr/0024-licensing-policy-for-shipped-components.md)). Không BSL, SSPL, AGPL
   trong phần phát hành cho khách; không dịch vụ managed độc quyền một cloud.

## Edge plane

| Thành phần | Công nghệ | License | ADR |
|---|---|---|---|
| Edge gateway (TLS, HTTP/1.1, HTTP/2, pipeline, proxy) | Go tự xây trên `net/http`, `crypto/tls`, `httputil.ReverseProxy` | BSD-3 | [0013](../09-adr/0013-edge-gateway-custom-go-with-coraza.md) |
| HTTP/3 | `quic-go` | MIT | [0013](../09-adr/0013-edge-gateway-custom-go-with-coraza.md) |
| Managed ruleset engine | Coraza v3 (nhúng) + OWASP CRS v4 | Apache-2.0 | [0013](../09-adr/0013-edge-gateway-custom-go-with-coraza.md) |
| Custom rule engine | CEL (`cel-go`) + thư viện hàm riêng (IP/CIDR, geo, ASN) | Apache-2.0 | [0014](../09-adr/0014-cel-as-rule-language.md) |
| IP / CIDR matching | Bảng định tuyến dạng trie (`bart` hoặc tương đương) | MIT | — |
| Geo / ASN | Định dạng MMDB, dữ liệu thương mại có quyền phân phối lại | Theo hợp đồng | [0025](../09-adr/0025-geo-asn-data-commercial-license.md) |
| Policy agent, state sync | Module trong cùng binary gateway | — | [0015](../09-adr/0015-snapshot-format-and-distribution.md) |
| Rate limit đồng bộ | Valkey theo region, chỉ cho hot key | BSD-3 | [0021](../09-adr/0021-valkey-for-hot-key-rate-limit-sync.md) |
| Log shipper | Vector (process riêng trên node) | MPL-2.0 | [0019](../09-adr/0019-vector-as-node-log-shipper.md) |
| L3/L4 | Năng lực hạ tầng / đối tác; PoP riêng: XDP (Mở rộng) | — | [0005](../09-adr/0005-edge-on-own-and-existing-infra.md) |

## Control plane

| Thành phần | Công nghệ | License | ADR |
|---|---|---|---|
| Config API, compiler, orchestrator, distribution, fleet | Go services | — | [0012](../09-adr/0012-go-as-primary-language.md) |
| Config store, fleet registry, trạng thái rollout | PostgreSQL | PostgreSQL | [0016](../09-adr/0016-postgres-and-s3-compatible-storage.md) |
| Artifact store | Object storage chuẩn S3; tự host: SeaweedFS | Apache-2.0 | [0016](../09-adr/0016-postgres-and-s3-compatible-storage.md) |
| Snapshot | Protobuf + zstd, content-addressed (SHA-256), ký Ed25519 | — | [0015](../09-adr/0015-snapshot-format-and-distribution.md) |
| Kênh Control ↔ node | gRPC stream hai chiều kiểu xDS (desired state, ACK/NACK) + HTTPS pull qua regional relay | Apache-2.0 | [0015](../09-adr/0015-snapshot-format-and-distribution.md) |
| Khoá ký, mã hoá key cert, CA cho node | OpenBao (transit, PKI) | MPL-2.0 | [0017](../09-adr/0017-openbao-for-signing-keys-and-node-pki.md) |
| ACME | `lego` | MIT | [0017](../09-adr/0017-openbao-for-signing-keys-and-node-pki.md) |
| Regression ruleset | `go-ftw` + corpus traffic tốt/xấu | Apache-2.0 | [0013](../09-adr/0013-edge-gateway-custom-go-with-coraza.md) |
| Nhận sự kiện từ Management | RabbitMQ (outbox của `metap`) | MPL-2.0 | [0022](../09-adr/0022-rabbitmq-for-control-events-kafka-for-telemetry.md) |

## Analytics plane

| Thành phần | Công nghệ | License | ADR |
|---|---|---|---|
| Ingest gateway (điểm vào log từ edge) | Vector chế độ aggregator, mTLS theo cert node | MPL-2.0 | [0019](../09-adr/0019-vector-as-node-log-shipper.md) |
| Ingestion bus | Apache Kafka (KRaft) | Apache-2.0 | [0018](../09-adr/0018-kafka-as-telemetry-bus.md) |
| Stream processor, exporter, detection, notifier, query API | Go services (`franz-go`, `clickhouse-go`) | BSD-3 / Apache-2.0 | [0012](../09-adr/0012-go-as-primary-language.md) |
| Hàng đợi gửi cảnh báo | Bảng `deliveries` trên PostgreSQL (SKIP LOCKED), adapter theo kênh | — | [0028](../09-adr/0028-notification-adapters-and-plan-gated-channels.md) |
| Hot search + số liệu gộp | ClickHouse (bảng raw + materialized view) | Apache-2.0 | [0020](../09-adr/0020-clickhouse-and-parquet-for-analytics.md) |
| Archive, mẫu request cho replay | Parquet trên object storage | Apache-2.0 | [0020](../09-adr/0020-clickhouse-and-parquet-for-analytics.md), [0026](../09-adr/0026-replay-on-request-samples-with-shared-pipeline.md) |
| Replay | Go, dùng chung thư viện pipeline với edge | — | [0026](../09-adr/0026-replay-on-request-samples-with-shared-pipeline.md) |

## Management plane

| Thành phần | Công nghệ | License | ADR |
|---|---|---|---|
| Backend nghiệp vụ | `metap-go` (modular monolith), PostgreSQL | — | [0009](../09-adr/0009-metap-for-management-plane-only.md) |
| Frontend | React + `@metap/platform-ui` + `@metap/ui` | — | — |
| SSO | OIDC (`metap-auth`) + SAML (`crewjam/saml`) | BSD-2 | — |
| IaC | Terraform provider (`terraform-plugin-framework`) | MPL-2.0 | — |
| Thanh toán | Adapter cổng thanh toán (trong nước và quốc tế) | — | — |

## Nền tảng vận hành

| Thành phần | Công nghệ | License | ADR |
|---|---|---|---|
| Runtime cho Management, Control, Analytics | Kubernetes + Helm; operator: CloudNativePG, Strimzi, Altinity ClickHouse Operator | Apache-2.0 | [0023](../09-adr/0023-kubernetes-for-central-planes-edge-outside.md) |
| Edge | Binary + systemd hoặc container; không bắt buộc Kubernetes | — | [0023](../09-adr/0023-kubernetes-for-central-planes-edge-outside.md) |
| Quan sát nền tảng | OpenTelemetry Collector, VictoriaMetrics, VictoriaLogs, Jaeger, Alertmanager, prober tự viết; Grafana chỉ nội bộ NCC | Apache-2.0 / AGPL nội bộ | [0029](../09-adr/0029-independent-platform-observability.md) |
