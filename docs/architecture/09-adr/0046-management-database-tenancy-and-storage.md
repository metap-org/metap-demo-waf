# ADR-0046: Database của Management: tenant là org, bảng chung, PolicyVersion là tài liệu bất biến

- Trạng thái: Chấp nhận (2026-11-06)
- Ngày: 2026-11-06
- Liên quan: [ADR-0009](0009-metap-for-management-plane-only.md), [5.8.14](../05-building-blocks/08-components/14-management-database.md)

## Bối cảnh

Management chạy trên `metap-go`: entity khai báo bằng metadata, mỗi entity một bảng, tenant qua
`control.tenants` với hai chiến lược (schema riêng hoặc database riêng). Ở quy mô thiết kế có khoảng
400.000 org, 1 triệu zone, 50 triệu PolicyVersion mỗi năm. Khi rà `metap-go` thấy bảng entity chưa có
index theo `tenant_id`, `unique` đang có hiệu lực toàn bảng, và reconciler chưa hỗ trợ partition.

## Quyết định

1. **Tenant của metap là org.** Account, zone và mọi thứ khác là entity trong tenant.
2. **Bảng chung trong schema `waap`, phân biệt bằng `tenant_id`** cho mọi org; **database riêng** cho
   Enterprise cần cô lập vật lý hoặc residency. Không dùng schema riêng cho mỗi tenant ở SaaS.
3. **Đối tượng policy nháp là entity từng dòng; PolicyVersion là một tài liệu jsonb bất biến** kèm
   `content_hash` trùng digest bundle ở Control.
4. **Registry toàn cục** (`waap_global`) cho giá trị phải duy nhất trên toàn nền tảng (hostname, tiền tố
   API key, domain SSO, nơi đặt tenant), ghi theo thứ tự claim trước, entity sau.
5. **Bảng lớn chỉ ghi thêm** của WAAP (usage theo ngày, purge) quản lý bằng migration SQL, partition
   theo thời gian; audit dùng bảng partition theo tháng của `metap-go`, archive Parquet sau 13 tháng,
   không xoá.
6. **Không lưu bí mật trong database**: private key cert, credential, secret SSO nằm ở OpenBao/`SecretStore`.
7. **Một primary + replica đọc, PgBouncer chế độ transaction**; không shard.
8. Sửa thiếu sót ở `metap-go` thay vì vá trong sản phẩm: index theo tenant, `uniqueScope`, field `system`,
   partition audit đã làm (Phase 103 của `metap-docs`).
9. Residency của cấu hình: database riêng đặt trong region của org.

## Hệ quả

- Catalog PostgreSQL không phình theo số org; thêm org không cần DDL.
- Truy vấn theo tenant dùng index `(tenant_id, …)` do reconciler của `metap-go` tự tạo.
- Tenant ở database riêng vẫn phải ghi `waap_global` trong database dùng chung: chỉ các registry nhỏ đó,
  không phải dữ liệu nghiệp vụ.
- Lịch sử policy rẻ để lưu và so sánh; truy vấn "rule X có trong những version nào" phải đọc tài liệu.

## Phương án đã cân nhắc

- **Tenant = account:** cô lập theo đơn vị gói, nhưng user và SSO thuộc org sẽ phải lặp ở nhiều tenant.
- **Schema riêng cho mỗi tenant:** cô lập tốt hơn, nhưng hàng trăm nghìn schema × vài chục bảng làm
  catalog, autovacuum và migration không chịu nổi.
- **PolicyVersion sao chép từng dòng sang bảng version:** truy vấn lịch sử bằng SQL dễ hơn, nhưng gấp
  hàng chục lần số dòng và compiler phải ghép lại thành tài liệu.
- **Shard PostgreSQL (Citus) theo tenant:** chưa cần ở 200–400 GB dữ liệu nóng và vài trăm ghi mỗi giây;
  giữ làm hướng khi số liệu thật vượt.
