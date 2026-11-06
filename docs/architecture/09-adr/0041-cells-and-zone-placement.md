# ADR-0041: Cell và cách đặt zone

- Trạng thái: **Đề xuất, chờ chọn** (phụ thuộc mô hình mạng của [ADR-0005](0005-edge-on-own-and-existing-infra.md) và [ADR-0035](0035-cdn-and-dns-strategy.md))
- Ngày: 2026-11-06

## Bối cảnh

Với 1 triệu zone và traffic đuôi dài, có ba lý do để không cho mọi node phục vụ mọi zone:

- RAM và tỉ lệ trúng cache của node: node chỉ cần runtime và nội dung cache của ít zone hơn.
- Blast radius: một cấu hình lỗi hay một đợt tấn công không nên ảnh hưởng tới mọi khách.
- Cô lập: zone lớn hoặc đang bị tấn công cần được tách ra.

## Phương án

| | C1. Không cell | C2. Cell theo VIP | C3. Hash theo SNI trong PoP | C4. Cell theo PoP |
|---|---|---|---|---|
| Cách làm | Mọi node phục vụ mọi zone, nạp khi cần | Mỗi cell có VIP riêng ở mọi PoP; DNS trả VIP của cell chứa zone; L4 load balancer chuyển VIP tới nhóm node | L4 load balancer đọc SNI trong ClientHello, hash zone vào một nhóm node | Mỗi zone chỉ được phục vụ ở một tập PoP |
| RAM và cache trên node | Kém nhất (mọi zone có thể tới mọi node) | Tốt | Tốt nhất (chọn đúng node theo zone) | Tốt |
| Blast radius | Toàn fleet | Một cell | Một nhóm node | Một tập PoP |
| Di chuyển zone | Không cần | Đổi DNS/VIP (TTL 60 s) | Đổi bảng hash | Đổi DNS; người dùng có thể đi xa hơn |
| Cần gì | — | Nhiều VIP; kiểm soát L4 load balancer | L4 load balancer hiểu TLS/QUIC; khó với HTTP/3 | Kiểm soát định tuyến theo PoP |
| Hợp với hạ tầng đối tác | Tốt | Trung bình (cần cấp nhiều IP) | Thấp | Trung bình |

## Đề xuất

**C2 (cell theo VIP)**, kết hợp nạp khi cần của [ADR-0040](0040-config-distribution-at-scale.md):

- Bắt đầu với 32–64 cell chuẩn, cell riêng cho khách enterprise và zone lớn, một cell cách ly cho zone
  đang bị tấn công.
- Zone được gán theo hash `zone_id`; top zone theo traffic được cân bằng theo tải.
- Rollout canary theo cell.
- PoP nhỏ cho nhiều cell dùng chung node nhưng vẫn giữ VIP riêng.

C1 dùng được cho giai đoạn đầu (ít zone): khi đó cell chỉ là khái niệm logic, mọi VIP cùng trỏ vào một
nhóm node; chuyển sang C2 không cần đổi node hay giao thức.

## Hệ quả nếu chọn đề xuất

- Cần nhiều dải IP (anycast hoặc theo PoP) để mỗi cell có VIP riêng ở mọi PoP.
- DNS điều hướng trả theo cell ([5.8.3](../05-building-blocks/08-components/03-dns-and-traffic-steering.md)).
- Cần công cụ cân bằng cell (theo traffic, RAM, CPU WAF) và quy trình chuyển zone sang cell cách ly ([8.13](../08-cross-cutting/13-operational-behaviors.md)).
