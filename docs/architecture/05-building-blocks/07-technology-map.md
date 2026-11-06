# 5.7 Bản đồ công nghệ

Lựa chọn công nghệ cho từng thành phần ở level 2. Mỗi dòng có ADR giải thích lý do và phương án đã
cân nhắc. Detail design mức C3 ở [08-components/](08-components/00-index.md).

Hai tiêu chí chạy xuyên suốt:

1. **Ngôn ngữ theo đặc tính tải** ([ADR-0030](../09-adr/0030-language-per-workload.md)): Go cho Control,
   Analytics, Management; edge (cùng replay và `edge-verify`) **chờ chọn** theo [ADR-0031](../09-adr/0031-edge-l7-and-waf-engine-options.md).
2. **Mọi thứ phải đóng gói được cho on-prem mà không vướng license**
   ([ADR-0024](../09-adr/0024-licensing-policy-for-shipped-components.md)). Không BSL, SSPL, AGPL
   trong phần phát hành cho khách; không dịch vụ managed độc quyền một cloud.

## Edge plane

| Thành phần | Công nghệ | License | ADR |
|---|---|---|---|
| Edge proxy (TLS, kết nối, pre-filter, challenge, cache, proxy) | **Chờ chọn.** Shortlist: Rust + Pingora, Go tự xây, Envoy; mốc NGINX (cache) và HAProxy (gọi WAF) | Apache-2.0 / BSD | [0031](../09-adr/0031-edge-l7-and-waf-engine-options.md), [0033](../09-adr/0033-two-tier-edge-proxy-and-waf.md) |
| HTTP/3 | Theo nền L7 được chọn (Pingora chưa xác nhận; Envoy, NGINX, `quic-go` có) | — | [0031](../09-adr/0031-edge-l7-and-waf-engine-options.md) |
| WAF inspection (tầng 2) | Dịch vụ riêng; engine mặc định Coraza (Go), so sánh libmodsecurity v3 trong spike; OWASP CRS v4 | Apache-2.0 | [0031](../09-adr/0031-edge-l7-and-waf-engine-options.md) |
| Proxy ↔ WAF | Inspection API riêng (gRPC stream); adapter `ext_proc`/SPOE nếu proxy là Envoy/HAProxy | Apache-2.0 | [0031](../09-adr/0031-edge-l7-and-waf-engine-options.md) |
| Cache (CDN) | Trong tầng proxy: RAM + NVMe, purge qua fast lane | — | [0035](../09-adr/0035-cdn-and-dns-strategy.md) |
| DNS điều hướng, DNS authoritative (Mở rộng) | CoreDNS + plugin riêng | Apache-2.0 | [0035](../09-adr/0035-cdn-and-dns-strategy.md) |
| Custom rule engine | CEL kiểm tra ở Control (`cel-go`); phần pre chạy IR ở proxy, phần inspect chạy ở WAF | Apache-2.0 | [0032](../09-adr/0032-cel-checked-at-control-ir-at-edge.md), [0034](../09-adr/0034-pre-filter-and-rule-split-across-tiers.md) |
| IP / CIDR matching | Trie IPv4/IPv6 theo ngôn ngữ edge | MIT / Apache-2.0 | — |
| Geo / ASN | Định dạng MMDB, dữ liệu thương mại có quyền phân phối lại | Theo hợp đồng | [0025](../09-adr/0025-geo-asn-data-commercial-license.md) |
| Policy agent, state sync | Policy agent trong binary của cả hai tầng; state sync trong edge proxy; con trỏ nguyên tử tới runtime bất biến | — | [0015](../09-adr/0015-snapshot-format-and-distribution.md) |
| Rate limit đồng bộ | Valkey theo region, chỉ cho hot key | BSD-3 | [0021](../09-adr/0021-valkey-for-hot-key-rate-limit-sync.md) |
| Log shipper | Vector (process riêng trên node) | MPL-2.0 | [0019](../09-adr/0019-vector-as-node-log-shipper.md) |
| L3/L4 | Năng lực hạ tầng / đối tác; PoP riêng: XDP (Mở rộng) | — | [0005](../09-adr/0005-edge-on-own-and-existing-infra.md) |

## Control plane

| Thành phần | Công nghệ | License | ADR |
|---|---|---|---|
| Config API, compiler, orchestrator, distribution, fleet | Go services | — | [0030](../09-adr/0030-language-per-workload.md) |
| Config store, fleet registry, trạng thái rollout | PostgreSQL | PostgreSQL | [0016](../09-adr/0016-postgres-and-s3-compatible-storage.md) |
| Artifact store | Object storage chuẩn S3; tự host: SeaweedFS | Apache-2.0 | [0016](../09-adr/0016-postgres-and-s3-compatible-storage.md) |
| Snapshot | Protobuf + zstd, content-addressed (SHA-256), ký Ed25519 | — | [0015](../09-adr/0015-snapshot-format-and-distribution.md) |
| Kênh Control ↔ node | gRPC stream hai chiều kiểu xDS (desired state, ACK/NACK) + HTTPS pull qua regional relay | Apache-2.0 | [0015](../09-adr/0015-snapshot-format-and-distribution.md) |
| Khoá ký, mã hoá key cert, CA cho node | OpenBao (transit, PKI) | MPL-2.0 | [0017](../09-adr/0017-openbao-for-signing-keys-and-node-pki.md) |
| ACME | `lego` | MIT | [0017](../09-adr/0017-openbao-for-signing-keys-and-node-pki.md) |
| Regression ruleset, self-test snapshot | `go-ftw` + corpus traffic tốt/xấu; `edge-verify` (cùng code với edge) | Apache-2.0 | [0031](../09-adr/0031-edge-l7-and-waf-engine-options.md) |
| Nhận sự kiện từ Management | RabbitMQ (outbox của `metap`) | MPL-2.0 | [0022](../09-adr/0022-rabbitmq-for-control-events-kafka-for-telemetry.md) |

## Analytics plane

| Thành phần | Công nghệ | License | ADR |
|---|---|---|---|
| Ingest gateway (điểm vào log từ edge) | Vector chế độ aggregator, mTLS theo cert node | MPL-2.0 | [0019](../09-adr/0019-vector-as-node-log-shipper.md) |
| Ingestion bus | Apache Kafka (KRaft) | Apache-2.0 | [0018](../09-adr/0018-kafka-as-telemetry-bus.md) |
| Stream processor, exporter, detection, notifier, query API | Go services (`franz-go`, `clickhouse-go`) | BSD-3 / Apache-2.0 | [0030](../09-adr/0030-language-per-workload.md) |
| Hàng đợi gửi cảnh báo | Bảng `deliveries` trên PostgreSQL (SKIP LOCKED), adapter theo kênh | — | [0028](../09-adr/0028-notification-adapters-and-plan-gated-channels.md) |
| Hot search + số liệu gộp | ClickHouse (bảng raw + materialized view) | Apache-2.0 | [0020](../09-adr/0020-clickhouse-and-parquet-for-analytics.md) |
| Archive, mẫu request cho replay | Parquet trên object storage | Apache-2.0 | [0020](../09-adr/0020-clickhouse-and-parquet-for-analytics.md), [0026](../09-adr/0026-replay-on-request-samples-with-shared-pipeline.md) |
| Replay | Cùng ngôn ngữ với edge, dùng chung code pipeline | — | [0026](../09-adr/0026-replay-on-request-samples-with-shared-pipeline.md) |

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
