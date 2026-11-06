# 9. Architecture Decisions

Mỗi quyết định một file, định dạng: trạng thái, ngày, bối cảnh, quyết định, hệ quả, phương án đã
cân nhắc. Quyết định bị thay thế giữ lại với trạng thái "Bị thay thế bởi ADR-xxxx".

| ADR | Quyết định | Trạng thái |
|---|---|---|
| [0001](0001-four-planes.md) | Bốn plane, edge độc lập runtime | Chấp nhận |
| [0002](0002-central-compile-signed-snapshots.md) | Compile tập trung, snapshot bất biến có chữ ký, rollout theo đợt | Chấp nhận |
| [0003](0003-rule-language-existing-syntax-with-builder.md) | Ngôn ngữ rule dùng cú pháp có sẵn, builder ↔ text qua AST | Chấp nhận |
| [0004](0004-managed-ruleset-crs-on-seclang.md) | Managed ruleset: OWASP CRS trên engine SecLang + system ruleset riêng | Chấp nhận |
| [0005](0005-edge-on-own-and-existing-infra.md) | Edge chạy trên PoP riêng và hạ tầng có sẵn | Chấp nhận |
| [0006](0006-saas-first-on-prem-capable.md) | SaaS trước, giữ đường hybrid và on-prem | Chấp nhận |
| [0007](0007-log-by-subscription-entitlements.md) | Log và giới hạn theo subscription qua entitlements | Chấp nhận |
| [0008](0008-under-attack-mode.md) | Under Attack mode theo zone | Chấp nhận |
| [0009](0009-metap-for-management-plane-only.md) | `metap` chỉ cho Management plane | Chấp nhận |
| [0010](0010-analytics-one-write-path-many-stores.md) | Analytics: một luồng ghi, nhiều store | Chấp nhận |
| [0011](0011-approximate-fleet-rate-limiting.md) | Rate limit toàn fleet là xấp xỉ | Chấp nhận |
| [0012](0012-go-as-primary-language.md) | Go là ngôn ngữ chính cho mọi plane | Bị thay thế bởi 0030 |
| [0013](0013-edge-gateway-custom-go-with-coraza.md) | Edge gateway tự xây bằng Go, nhúng Coraza + CRS v4 | Mở lại thành phương án D2 của 0031 |
| [0014](0014-cel-as-rule-language.md) | CEL là ngôn ngữ custom rule | Chấp nhận, sửa bởi 0032 |
| [0015](0015-snapshot-format-and-distribution.md) | Snapshot protobuf + zstd ký Ed25519; gRPC desired state + HTTPS pull qua relay | Chấp nhận |
| [0016](0016-postgres-and-s3-compatible-storage.md) | PostgreSQL cho trạng thái điều khiển, object storage chuẩn S3 (SeaweedFS khi tự host) | Chấp nhận |
| [0017](0017-openbao-for-signing-keys-and-node-pki.md) | OpenBao cho khoá ký, mã hoá key cert, CA của node | Chấp nhận |
| [0018](0018-kafka-as-telemetry-bus.md) | Apache Kafka làm ingestion bus | Chấp nhận |
| [0019](0019-vector-as-node-log-shipper.md) | Vector làm log shipper trên node | Chấp nhận |
| [0020](0020-clickhouse-and-parquet-for-analytics.md) | ClickHouse cho hot search và số liệu gộp, Parquet cho archive | Chấp nhận |
| [0021](0021-valkey-for-hot-key-rate-limit-sync.md) | Valkey theo region cho counter hot key | Chấp nhận |
| [0022](0022-rabbitmq-for-control-events-kafka-for-telemetry.md) | RabbitMQ cho sự kiện điều khiển, Kafka cho telemetry | Chấp nhận |
| [0023](0023-kubernetes-for-central-planes-edge-outside.md) | Kubernetes cho plane trung tâm, edge ngoài Kubernetes | Chấp nhận |
| [0024](0024-licensing-policy-for-shipped-components.md) | Không BSL/SSPL/AGPL trong thành phần phát hành | Chấp nhận |
| [0025](0025-geo-asn-data-commercial-license.md) | Dữ liệu geo/ASN dùng giấy phép thương mại | Chấp nhận |
| [0026](0026-replay-on-request-samples-with-shared-pipeline.md) | Replay trên mẫu request đầy đủ, dùng chung pipeline | Chấp nhận |
| [0027](0027-explicit-alert-formulas.md) | Cảnh báo theo template có công thức tường minh, baseline median/MAD theo phút trong tuần | Chấp nhận |
| [0028](0028-notification-adapters-and-plan-gated-channels.md) | Notification: adapter theo kênh, webhook CloudEvents ký HMAC, email luôn có, kênh khác theo gói | Chấp nhận |
| [0029](0029-independent-platform-observability.md) | Giám sát nền tảng độc lập với Analytics, SLO + burn rate, probe theo hành vi | Chấp nhận |
| [0030](0030-language-per-workload.md) | Ngôn ngữ theo đặc tính tải: Go cho Control/Analytics/Management, edge theo 0031; Elixir đã cân nhắc | Chấp nhận (phần edge chờ 0031) |
| [0031](0031-edge-l7-and-waf-engine-options.md) | Nền L7 edge và engine WAF: so sánh 6 tổ hợp, shortlist A1 / D2 / B2, spike để chọn | **Đề xuất, chờ chọn** |
| [0032](0032-cel-checked-at-control-ir-at-edge.md) | CEL kiểm tra ở Control, edge thực thi IR | Đề xuất, theo 0031 |

Quyết định đang chờ: xem [11. Risks and Technical Debt](../11-risks/00-index.md#113-quyết-định-còn-mở).
