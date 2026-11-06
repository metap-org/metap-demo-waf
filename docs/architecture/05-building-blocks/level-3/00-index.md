# Level 3: Detail design theo container

C4 Component cho từng container ở level 2. Mỗi file có component diagram, bảng trách nhiệm và
thiết kế, hợp đồng hoặc schema chính, hành vi khi lỗi. Công nghệ theo
[07-technology-map.md](../07-technology-map.md).

| File | Container | Plane |
|---|---|---|
| [10-edge-gateway.md](10-edge-gateway.md) | Edge gateway, package `pipeline` | Edge |
| [11-edge-agent-and-state.md](11-edge-agent-and-state.md) | Policy agent, state sync, Vector | Edge |
| [20-config-and-compiler.md](20-config-and-compiler.md) | config-service, compiler, định dạng snapshot | Control |
| [21-rollout-distribution-fleet.md](21-rollout-distribution-fleet.md) | rollout-orchestrator, distribution, regional relay, fleet-registry | Control |
| [22-cert-ruleset-release.md](22-cert-ruleset-release.md) | cert-service, pipeline phát hành ruleset | Control |
| [30-ingestion-and-storage.md](30-ingestion-and-storage.md) | Ingest gateway, Kafka, enricher, ClickHouse, archiver | Analytics |
| [31-query-detection-export-replay.md](31-query-detection-export-replay.md) | query-api, detection, notifier, exporter, replay | Analytics |
| [40-management.md](40-management.md) | waap-management (metap-go), portal | Management |

## Danh sách deployable

| Deployable | Ngôn ngữ / sản phẩm | Chạy ở |
|---|---|---|
| `edge` (gateway + agent + state sync) | Go | Edge node |
| `vector` | Vector | Edge node |
| `ingest-gateway` | Vector (aggregator) | Mỗi Analytics region |
| `regional-relay` | Go | Mỗi region, tuỳ chọn ở PoP lớn |
| `config-service`, `compiler`, `rollout-orchestrator`, `distribution`, `fleet-registry`, `cert-service` | Go | Core region (Kubernetes) |
| `enricher`, `archiver`, `query-api`, `detection`, `notifier`, `exporter`, `replay` | Go | Mỗi Analytics region (Kubernetes) |
| `waap-management` | Go (`metap-go`) | Core region (Kubernetes) |
| `portal` | React (static) | CDN / core region |
| PostgreSQL, RabbitMQ, OpenBao, Valkey, Kafka, ClickHouse, object storage | Mã nguồn mở | Theo [07](../../07-deployment/00-index.md) |
