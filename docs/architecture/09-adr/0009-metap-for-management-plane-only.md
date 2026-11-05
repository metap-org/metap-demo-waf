# ADR-0009: `metap` chỉ dùng cho Management plane

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

`metap` cung cấp entity CRUD sinh từ metadata, workflow, RBAC/ABAC, audit trail, outbox. Đó chính
là thứ Management cần. Ba plane còn lại có yêu cầu khác hẳn.

## Quyết định

| Plane | Dùng `metap` |
|---|---|
| Management | Có: entity, workflow duyệt và case, quyền, audit, outbox sang Control |
| Control | Một phần: có thể đặt config store/API trên `metap`; compiler, rollout, distribution, fleet tự xây |
| Edge | Không |
| Analytics | Không; `metap` chỉ giữ thứ phái sinh do người dùng tạo (saved search, alert policy, định nghĩa báo cáo, case) |

## Hệ quả

- Management ra nhanh nhờ hạ tầng có sẵn.
- Edge và Analytics chọn công nghệ độc lập theo yêu cầu độ trễ và khối lượng.
- Không được để logic edge hay truy vấn log "tiện tay" chui vào `metap`.
