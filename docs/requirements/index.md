# Requirements WAAP

Tài liệu này nói **hệ thống phải làm gì**: chức năng, quy tắc nghiệp vụ, dữ liệu, tiêu chí chấp nhận, và
phân tích từng nghiệp vụ theo cách khách thật làm. Phần **làm bằng gì** (plane, thành phần, công nghệ, ADR) nằm
ở [kiến trúc](../architecture/index.md).

Trạng thái: **bản nháp, đang phân tích** (2026-11-06). Chưa mục nào được chốt.

## Vì sao tách khỏi kiến trúc

- Requirements do chủ sản phẩm quyết, kiến trúc do kỹ thuật quyết; trộn hai thứ trong một bộ tài liệu làm
  khó thấy cái gì là yêu cầu, cái gì là lựa chọn thiết kế.
- Requirements thay đổi theo phân tích nghiệp vụ, thường nhanh hơn kiến trúc.
- Kiến trúc trỏ sang requirements để giải thích lý do, nhưng không chép lại nội dung.

## Các mục

1. [Bản đồ phân rã chức năng](01-functional-decomposition.md): chức năng theo nghiệp vụ (F1–F12) và theo plane,
   liên kết giữa các plane (LK).
2. [Requirements](02-requirements.md): yêu cầu chức năng có tiêu chí chấp nhận, quy tắc nghiệp vụ, yêu cầu
   dữ liệu, phi chức năng, giao tiếp ngoài, câu hỏi mở.
3. [Phân tích nghiệp vụ](03-business-analysis/00-index.md): từng nghiệp vụ theo vòng đời của khách, bắt đầu từ
   [onboarding](03-business-analysis/01-onboarding.md).

Quy ước đánh số giống kiến trúc: số đầu tên file là số mục, `00-index.md` là trang của thư mục. Mã (F, FR, BR,
DT, NFR, IR, OQ, LK…) giải thích ở [glossary](../architecture/12-glossary/00-index.md#1213-quy-ước-mã-trong-tài-liệu),
dùng chung cho hai bộ tài liệu.

## Quan hệ với kiến trúc

| Requirements | Kiến trúc dùng ở |
|---|---|
| Tổng quan nghiệp vụ, mục tiêu chất lượng | [1. Introduction and Goals](../architecture/01-introduction/00-index.md) (giữ ở kiến trúc vì arc42 đặt ở đó) |
| Kịch bản chất lượng đo được (QS) | [10. Quality Requirements](../architecture/10-quality/00-index.md) |
| Liên kết giữa các plane (LK) | [5.5 Hợp đồng giữa các plane](../architecture/05-building-blocks/05-contracts.md) |
| Yêu cầu dữ liệu (DT) | [5.6](../architecture/05-building-blocks/06-data-model.md), [5.8.14](../architecture/05-building-blocks/08-components/14-management-database.md) |
