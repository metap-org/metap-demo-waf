# 5.5 Hợp đồng giữa các plane

Mỗi hợp đồng có schema version. Bên nhận phải chấp nhận version hiện tại và version liền trước
(N-1), để mỗi plane deploy độc lập. Bên nhận gặp version mới hơn mức nó hiểu thì từ chối cả bản
và giữ trạng thái cũ, không đọc dở.

| Hợp đồng | Từ → Tới | Nội dung | Giao nhận |
|---|---|---|---|
| **PolicyVersionPublished** | Management → Control | zone ID, PolicyVersion ID và cha, AST của rule, cấu hình ruleset, exception, cài đặt zone, người duyệt, kênh (thường / khẩn) | Outbox → at-least-once; Control bỏ trùng theo version ID |
| **ZoneChanged** | Management → Control | hostname, origin, trạng thái, fail mode, yêu cầu cert | Như trên |
| **EntitlementsChanged** | Management → Control, Analytics | Giới hạn theo gói: tỉ lệ lấy mẫu log, retention tối đa, khoảng truy vấn, số rule, đích export, tính năng bật | Như trên; có hiệu lực không cần deploy |
| **Snapshot** | Control → Edge | Bundle theo zone + bundle chung; schema version; chữ ký; version của từng phần | Pull theo version, kiểm chữ ký trước khi nạp |
| **SnapshotAnnouncement** | Control → Edge | "zone X có version N" hoặc "bundle chung có version M"; cờ khẩn | Push nhẹ, mất cũng không sao vì agent tự kiểm định kỳ |
| **NodeStatus** | Edge → Control | Node ID, phần mềm, version đang chạy theo zone, lỗi nạp, chỉ số sức khoẻ | Heartbeat định kỳ + sự kiện khi đổi version |
| **TelemetryRecord** | Edge → Analytics | Loại (security event / access log), request ID, thời điểm, zone, tenant, policy version, bước pipeline, rule ID, hành động, trường request đã che | At-least-once; Analytics bỏ trùng theo request ID + loại |
| **Query** | Management → Analytics | Truy vấn có cấu trúc (không phải ngôn ngữ truy vấn tự do của store), danh tính người gọi, phạm vi zone | Đồng bộ, có giới hạn chi phí |
| **AlertRaised / CaseProposed** | Analytics → Management | Điều kiện khớp, mẫu event, đề xuất mitigation | At-least-once, bỏ trùng theo khoá cảnh báo |
| **RolloutSignal** | Analytics → Control | Chỉ số theo zone/policy version trong cửa sổ canary | Định kỳ trong lúc rollout |
| **UsageReport** | Analytics → Management | Số đo sử dụng theo tenant và kỳ | Định kỳ, có thể chạy lại |

## Snapshot gồm gì

```text
bundle chung (version M)
  ├── ruleset releases: CRS vX, system ruleset vY
  ├── cài đặt toàn cục, danh sách chặn khẩn của NCC
  └── khoá HMAC cấp và kiểm clearance token (hiện tại + trước đó)
bundle zone (version N, tham chiếu bundle chung ≥ M)
  ├── hostname, origin, TLS (tham chiếu cert đã mã hoá), fail mode
  ├── custom rule đã biên dịch theo priority
  ├── cấu hình managed ruleset: paranoia, ngưỡng anomaly, rule tắt, exclusion
  ├── access list, rate limit, DDoS L7, Under Attack mode
  └── entitlements áp ở edge: tỉ lệ lấy mẫu, giới hạn tài nguyên
```

Disabled rule bị loại khi biên dịch chứ không mang theo cờ: edge không bao giờ giữ một rule mà nó
phải nhớ là không áp.
