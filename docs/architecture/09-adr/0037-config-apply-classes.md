# ADR-0037: Phân lớp cấu hình theo cách áp dụng trên edge

- Trạng thái: Chấp nhận
- Ngày: 2026-11-06

## Bối cảnh

"Snapshot bất biến, swap nguyên khối" ([ADR-0015](0015-snapshot-format-and-distribution.md)) đúng cho
cấu hình của zone. Nhưng không phải mọi cấu hình đều có cùng chi phí khi áp dụng:

- Build lại ruleset CRS hay nạp MMDB mất vài giây và cần RAM gấp đôi.
- Đổi cổng nghe hay số worker không thể swap trong process đang chạy.

Nếu không phân lớp, một release ruleset có thể làm cả PoP thiếu CPU hoặc OOM cùng lúc, còn fast lane có
thể vô tình kéo theo một lần build nặng.

## Quyết định

- Bốn lớp áp dụng ([8.15 §2](../08-cross-cutting/15-config-propagation-and-apply.md#2-bốn-lớp-áp-dụng)):
  - **A** đổi nóng;
  - **B** build nền rồi đổi nóng;
  - **C** reload process (hot restart, giữ socket);
  - **D** nâng phần mềm.
- Mọi cấu hình của zone hoặc tenant phải là lớp A. Lớp B chỉ cho dữ liệu lớn dùng chung do NCC phát hành
  (ruleset release, MMDB, reputation list lớn). Lớp C/D chỉ cho **node profile** và binary, không do khách
  kích hoạt.
- Fast lane chỉ chở thay đổi lớp A. Rule khẩn nằm trong một lớp rule khẩn nhỏ riêng, chạy trước CRS, để
  không phải build lại CRS.
- Trên node:
  - build ở thread nền có giới hạn CPU, kiểm RAM trước khi build lớp B (thiếu thì NACK);
  - so le build lớp B tối đa 25% node WAF mỗi PoP;
  - giữ sẵn 3 manifest gần nhất để rollback.
- Hai tầng swap độc lập; Inspect request mang version của proxy, WAF giữ version hiện tại và version
  trước; một node chỉ tính là "đã nhận" khi cả hai tầng ACK.
- Node profile là bundle riêng có chữ ký, rollout như phần mềm.

## Hệ quả

- Compiler phải biết mỗi thay đổi thuộc lớp nào và từ chối đưa thay đổi lớp B vào fast lane.
- Node cần dư RAM cho lớp B. Kích thước node WAF phải tính theo RAM của hai instance ruleset cùng lúc.
- Rate limit counter giữ theo hash định nghĩa rule: đổi ngưỡng hoặc khoá của một rule thì counter của rule
  đó bắt đầu lại.
