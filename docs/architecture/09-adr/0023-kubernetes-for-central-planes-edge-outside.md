# ADR-0023: Kubernetes cho các plane trung tâm, edge chạy ngoài Kubernetes

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Quyết định

- Management, Control, Analytics chạy trên **Kubernetes**, đóng gói bằng Helm. Thành phần có trạng
  thái dùng operator: CloudNativePG (PostgreSQL), Strimzi (Kafka), Altinity Operator (ClickHouse),
  OpenBao Helm chart, Valkey.
- Edge là **một binary + Vector**, chạy bằng systemd trên bare metal/VM hoặc container; Kubernetes
  là tuỳ chọn, không bắt buộc.

## Lý do

- Cùng một bộ Helm chạy được trên cloud bất kỳ và on-prem của khách ([ADR-0006](0006-saas-first-on-prem-capable.md)).
- Edge cần kiểm soát mạng và CPU trực tiếp (anycast, XDP, pin CPU); hạ tầng đối tác không phải nơi
  nào cũng có Kubernetes.
