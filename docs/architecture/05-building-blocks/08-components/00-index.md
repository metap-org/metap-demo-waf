# 5.8 Level 3: detail design theo container (C4 Component)

C4 Component cho từng container ở level 2. Mỗi file có component diagram, bảng trách nhiệm và
thiết kế, hợp đồng hoặc schema chính, hành vi khi lỗi. Công nghệ theo
[07-technology-map.md](../07-technology-map.md).

| Mục | File | Container | Plane |
|---|---|---|---|
| 5.8.1 | [01-edge-L7.md](01-l7.md) | L7: pre-filter, challenge, cache, gọi WAF | Edge |
| 5.8.2 | [02-waf-inspection.md](02-waf-inspection.md) | WAF inspection: CRS, custom rule phần inspect | Edge |
| 5.8.3 | [03-dns-and-traffic-steering.md](03-dns-and-traffic-steering.md) | DNS điều hướng, DNS authoritative (Mở rộng) | Edge |
| 5.8.4 | [04-edge-agent-and-state.md](04-edge-agent-and-state.md) | Policy agent, state sync, Vector | Edge |
| 5.8.5 | [05-config-and-compiler.md](05-config-and-compiler.md) | config-service, compiler, định dạng snapshot | Control |
| 5.8.6 | [06-rollout-distribution-fleet.md](06-rollout-distribution-fleet.md) | rollout-orchestrator, distribution, regional relay, fleet-registry | Control |
| 5.8.7 | [07-cert-ruleset-release.md](07-cert-ruleset-release.md) | cert-service, pipeline phát hành ruleset | Control |
| 5.8.8 | [08-ingestion-and-storage.md](08-ingestion-and-storage.md) | Ingest gateway, Kafka, enricher, ClickHouse, archiver | Analytics |
| 5.8.9 | [09-query-detection-export-replay.md](09-query-detection-export-replay.md) | query-api, detection, notifier, exporter, replay | Analytics |
| 5.8.10 | [10-alerting-rules-and-algorithms.md](10-alerting-rules-and-algorithms.md) | Cảnh báo WAF và DDoS: danh mục, cấu hình ngưỡng, thuật toán | Analytics |
| 5.8.11 | [11-notification-fanout.md](11-notification-fanout.md) | Notifier: fan-out nhiều kênh, khách tự tích hợp, kênh theo gói | Analytics |
| 5.8.12 | [12-management.md](12-management.md) | waap-management (metap-go), portal | Management |

## Danh sách deployable

| Deployable | Ngôn ngữ / sản phẩm | Chạy ở |
|---|---|---|
| `l7-proxy` (L7 + agent + state sync) | Chờ chọn ([ADR-0031](../../09-adr/0031-edge-l7-and-waf-engine-options.md)) | Edge node (pool RAM) |
| `waf-inspection` (engine + agent) | Go mặc định (Coraza) | Edge node (pool CPU, hoặc chung host với cgroup riêng) |
| `edge-dns` | CoreDNS + plugin | Mỗi PoP |
| `edge-verify` (nạp thử snapshot, lint ruleset) | Cùng ngôn ngữ và code với edge | Cạnh compiler và pipeline phát hành ruleset |
| `vector` | Vector | Edge node |
| `ingest-gateway` | Vector (aggregator) | Mỗi Analytics region |
| `regional-relay` | Go | Mỗi region, tuỳ chọn ở PoP lớn |
| `config-service`, `compiler`, `rollout-orchestrator`, `distribution`, `fleet-registry`, `cert-service` | Go | Core region (Kubernetes) |
| `enricher`, `archiver`, `query-api`, `detection`, `notifier`, `exporter` | Go | Mỗi Analytics region (Kubernetes) |
| `replay` | Cùng ngôn ngữ và code `pipeline` với edge | Mỗi Analytics region (Kubernetes) |
| `waap-management` | Go (`metap-go`) | Core region (Kubernetes) |
| `portal` | React (static) | CDN / core region |
| PostgreSQL, RabbitMQ, OpenBao, Valkey, Kafka, ClickHouse, object storage | Mã nguồn mở | Theo [07](../../07-deployment/00-index.md) |
