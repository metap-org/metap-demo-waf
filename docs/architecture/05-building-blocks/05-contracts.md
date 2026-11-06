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
| **Inspection API** | Edge proxy → WAF inspection (trong PoP) | Header, body theo chunk, ngữ cảnh zone/tenant/request ID; trả verdict | gRPC stream đồng bộ, có deadline; xem mục bên dưới |
| **TelemetryRecord** | Edge → Analytics | Loại (security event / access log / mẫu request), request ID, thời điểm, zone, tenant, policy version, bước pipeline, rule ID, hành động, `status_source`, `edge_error`, thời gian theo chặng ([8.14](../08-cross-cutting/14-latency-and-error-attribution.md)), trường request đã che | At-least-once; Analytics bỏ trùng theo request ID + loại |
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
  ├── phần proxy
  │     ├── hostname, origin, TLS (tham chiếu cert đã mã hoá), fail mode
  │     ├── access list, rate limit, DDoS L7, Under Attack mode
  │     ├── custom rule phần pre (IR), inspect gate (miễn WAF cho tài nguyên tĩnh)
  │     ├── cache rule, cấu hình purge
  │     └── entitlements áp ở proxy: tỉ lệ lấy mẫu, giới hạn body buffer, kết nối
  └── phần WAF
        ├── cấu hình managed ruleset: paranoia, ngưỡng anomaly, rule tắt, exclusion
        ├── custom rule phần inspect
        └── entitlements áp ở WAF: phần CPU, kích thước body kiểm tra
```

Disabled rule bị loại khi biên dịch chứ không mang theo cờ: edge không bao giờ giữ một rule mà nó
phải nhớ là không áp.

Policy agent ở tầng proxy chỉ nạp phần proxy (và bundle chung), ở tầng WAF chỉ nạp phần WAF; cùng một
manifest nên hai tầng luôn chạy cùng version policy.

## Inspection API (edge proxy → WAF)

Hợp đồng chuẩn giữa hai tầng ([ADR-0031](../09-adr/0031-edge-l7-and-waf-engine-options.md) §5), độc lập
với nền proxy được chọn.

```protobuf
service Inspection {
  rpc Inspect(stream InspectRequest) returns (stream InspectResponse);
}
message InspectRequest {
  oneof m {
    RequestHead head  = 1;  // zone_id, tenant_id, request_id, policy_version_id, method, uri,
                            // header, client_ip, geo, asn, bot_score, deadline_us
    bytes       chunk = 2;  // body, theo thứ tự, tổng tối đa theo entitlements
    End         end   = 3;  // body_truncated
  }
}
message InspectResponse {
  oneof m {
    Verdict    verdict    = 1;  // có thể gửi trước khi nhận hết body
    Overloaded overloaded = 2;  // admission control từ chối, proxy áp fail mode
    Error      error      = 3;
  }
}
message Verdict {
  Action action = 1;            // ALLOW, LOG, CHALLENGE, BLOCK
  repeated string rule_ids = 2;
  uint32 anomaly_score = 3;
  repeated MatchedField matched = 4;  // tên trường + đoạn giá trị đã che
  uint32 engine_time_us = 5;
}
```

| Quy tắc | Giá trị |
|---|---|
| Deadline | Proxy đặt theo ngân sách của zone (mặc định 1,5 ms); quá hạn → fail mode |
| Verdict sớm | WAF trả ngay khi điểm anomaly vượt ngưỡng; proxy ngừng gửi body |
| Tương thích | N/N-1 như mọi hợp đồng; WAF nhận `ext_proc`/SPOE qua adapter nếu proxy là Envoy/HAProxy |
| Bảo mật | Unix socket khi chung host, mTLS khi khác node |
