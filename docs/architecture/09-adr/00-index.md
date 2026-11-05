# 9. Architecture Decisions

Mỗi quyết định một file, định dạng: trạng thái, ngày, bối cảnh, quyết định, hệ quả, phương án đã
cân nhắc. Quyết định bị thay thế giữ lại với trạng thái "Bị thay thế bởi ADR-xxxx".

| ADR | Quyết định | Trạng thái |
|---|---|---|
| [0001](0001-four-planes.md) | Bốn plane, edge độc lập runtime | Chấp nhận |
| [0002](0002-central-compile-signed-snapshots.md) | Compile tập trung, snapshot bất biến có chữ ký, rollout theo đợt | Chấp nhận |
| [0003](0003-rule-language-existing-syntax-with-builder.md) | Ngôn ngữ rule dùng cú pháp có sẵn, builder ↔ text qua AST | Chấp nhận |
| [0004](0004-managed-ruleset-crs-on-seclang.md) | Managed ruleset: OWASP CRS trên Coraza/ModSecurity + system ruleset riêng | Chấp nhận |
| [0005](0005-edge-on-own-and-existing-infra.md) | Edge chạy trên PoP riêng và hạ tầng có sẵn | Chấp nhận |
| [0006](0006-saas-first-on-prem-capable.md) | SaaS trước, giữ đường hybrid và on-prem | Chấp nhận |
| [0007](0007-log-by-subscription-entitlements.md) | Log và giới hạn theo subscription qua entitlements | Chấp nhận |
| [0008](0008-under-attack-mode.md) | Under Attack mode theo zone | Chấp nhận |
| [0009](0009-metap-for-management-plane-only.md) | `metap` chỉ cho Management plane | Chấp nhận |
| [0010](0010-analytics-one-write-path-many-stores.md) | Analytics: một luồng ghi, nhiều store | Chấp nhận |
| [0011](0011-approximate-fleet-rate-limiting.md) | Rate limit toàn fleet là xấp xỉ | Chấp nhận |

Quyết định đang chờ: xem [11. Risks and Technical Debt](../11-risks/00-index.md#113-quyết-định-còn-mở).
