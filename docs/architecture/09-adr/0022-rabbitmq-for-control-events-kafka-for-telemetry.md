# ADR-0022: RabbitMQ cho sự kiện điều khiển, Kafka cho telemetry

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- Sự kiện từ Management sang Control (`PolicyVersionPublished`, `ZoneChanged`,
  `EntitlementsChanged`) và từ Analytics về Management (`AlertRaised`, `CaseProposed`,
  `UsageReport`) đi qua **RabbitMQ**, dùng outbox và `EventBus` có sẵn của `metap`.
- Telemetry khối lượng lớn đi qua **Kafka** ([ADR-0018](0018-kafka-as-telemetry-bus.md)).

## Lý do

- Sự kiện điều khiển ít (vài nghìn mỗi ngày), cần routing và xác nhận từng sự kiện; `metap` đã có
  sẵn outbox ghi cùng transaction với RabbitMQ.
- Không phải viết adapter Kafka cho `EventBus` của `metap` ngay bây giờ.

## Hệ quả

- Vận hành hai broker. Nếu sau này `metap` có adapter Kafka cho `EventBus`, có thể gộp về một; quyết
  định đó không ảnh hưởng hợp đồng sự kiện.
