# 2. Architecture Constraints

## 2.1 Kỹ thuật

| Ràng buộc | Lý do / nguồn |
|---|---|
| Edge viết bằng Rust; Control, Analytics, Management viết bằng Go | [ADR-0030](../09-adr/0030-language-per-workload.md) |
| Management plane xây trên `metap` (Go) | Tận dụng entity CRUD, workflow, RBAC/ABAC, audit, outbox có sẵn ([ADR-0009](../09-adr/0009-metap-for-management-plane-only.md)) |
| Edge, Control engine, Analytics **không** phụ thuộc `metap` | Yêu cầu độ trễ, khối lượng và chu kỳ deploy khác hẳn entity CRUD |
| Managed ruleset là OWASP CRS chạy trên engine tương thích SecLang (MVP: libmodsecurity v3) | [ADR-0004](../09-adr/0004-managed-ruleset-crs-on-seclang.md), [ADR-0031](../09-adr/0031-edge-in-rust-on-pingora.md) |
| Ngôn ngữ rule cho khách dùng cú pháp biểu thức đã có trên thị trường, không tự phát minh | [ADR-0003](../09-adr/0003-rule-language-existing-syntax-with-builder.md) |
| Không plane nào phụ thuộc dịch vụ managed chỉ có trên một cloud | Giữ được on-prem và hybrid ([ADR-0006](../09-adr/0006-saas-first-on-prem-capable.md)) |
| Edge chạy được trên bare metal, VM và container với cùng một bản build | Chạy được trên PoP riêng lẫn hạ tầng đối tác ([ADR-0005](../09-adr/0005-edge-on-own-and-existing-infra.md)) |
| Giao tiếp giữa các plane qua hợp đồng có version, không gọi trực tiếp vào database của plane khác | Mỗi plane deploy độc lập |

## 2.2 Tổ chức

| Ràng buộc | Hệ quả |
|---|---|
| Ra thị trường bằng SaaS trước | Thứ tự xây theo [ADR-0006](../09-adr/0006-saas-first-on-prem-capable.md); on-prem không được làm chậm MVP nhưng không được bị chặn đường |
| Đội nhỏ | Ưu tiên mua/dùng mã nguồn mở cho phần không khác biệt (CRS, engine SecLang, L3/L4 qua hạ tầng); tự xây phần tạo khác biệt (vòng đời policy, xử lý chặn nhầm, analytics theo rule) |
| Có đội threat research của NCC | System ruleset riêng là tài sản sản phẩm, phát hành theo quy trình như phần mềm |

## 2.3 Pháp lý và tuân thủ

| Ràng buộc | Hệ quả |
|---|---|
| Access log chứa IP, header, có thể chứa dữ liệu cá nhân (Nghị định 13/2023/NĐ-CP, GDPR với khách EU) | Che/loại trường nhạy cảm tại edge trước khi gửi đi; retention có giới hạn theo gói; xoá được theo tenant |
| Khách cần data residency | Analytics đặt theo region dữ liệu; log không rời region của tenant |
| Khách PCI DSS / SOC 2 | Audit log bất biến; phân tách nhiệm vụ (4-eyes) khi đổi policy; báo cáo xuất được |
| Private key chứng chỉ của khách | Lưu và phân phối như bí mật cấp cao nhất; edge chỉ giữ trong RAM |

## 2.4 Convention

- Tài liệu theo arc42, quyết định ghi thành ADR trong [09](../09-adr/00-index.md).
- Tài liệu viết tiếng Việt, thuật ngữ kỹ thuật giữ tiếng Anh (cùng chính sách của org).
- Diagram bằng Mermaid trong Markdown.
