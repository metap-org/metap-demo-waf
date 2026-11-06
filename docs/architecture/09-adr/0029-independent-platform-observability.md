# ADR-0029: Giám sát nền tảng độc lập, theo SLO và theo hành vi

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Bối cảnh

Giám sát kiểu "process còn sống, CPU bao nhiêu" không trả lời được câu hỏi khách quan tâm: WAF có đang
chặn thật không, config có tới edge không, log có mất không. Nếu giám sát đi qua chính pipeline
Analytics thì Analytics sập cũng làm NCC mù.

## Quyết định

- Stack riêng: OpenTelemetry Collector, VictoriaMetrics, VictoriaLogs, Jaeger, Alertmanager; Grafana
  chỉ nội bộ. Metrics edge đẩy thẳng tới VictoriaMetrics của region (remote_write, mTLS), không qua Kafka.
- **10 SLI/SLO** theo hành vi ([01-platform-monitoring](../08-cross-cutting/12-platform-monitoring.md)),
  cảnh báo người trực theo **burn rate hai cửa sổ** (14,4× trong 1 h và 5 m; 6× trong 6 h và 30 m;
  1× trong 3 ngày → ticket).
- **Probe tổng hợp** chạy đúng hành vi từ đầu tới cuối trên zone canary: request hợp lệ, request tấn
  công phải bị chặn, đổi config phải tới mọi node, log phải tra cứu được, cảnh báo phải tới kênh.
- **Đối soát** số security event phát ra ở edge với số nhận vào ClickHouse theo giờ.
- Watchdog bên ngoài (dead man's switch) để biết hệ thống cảnh báo còn sống.
- Mỗi cảnh báo P1/P2 phải có runbook ([02-operational-behaviors](../08-cross-cutting/13-operational-behaviors.md)).

## Hệ quả

- Cần zone canary thật trên mọi PoP và đích export/webhook test của NCC.
- Thêm một cụm observability phải vận hành; quy mô nhỏ hơn nhiều so với Analytics của khách.
