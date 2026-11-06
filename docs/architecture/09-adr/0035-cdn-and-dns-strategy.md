# ADR-0035: Chiến lược CDN và DNS

- Trạng thái: **Đề xuất, chờ chọn**
- Ngày: 2026-11-06

## 1. Bối cảnh

WAF đứng một mình là một chặng thêm vào đường đi của traffic: khách trả tiền cho bảo mật nhưng phải
chịu thêm độ trễ và chi phí băng thông. Khi edge đồng thời là **CDN**, traffic đã đi qua edge vì lý do
hiệu năng (cache gần người dùng, giảm băng thông origin), và WAF trở thành phần cộng thêm gần như miễn
phí về độ trễ. Đó là lý do các nhà cung cấp WAAP lớn đều bán kèm CDN:

- Băng thông tiết kiệm được ở origin bù chi phí dịch vụ.
- Mạng đủ rộng để làm CDN cũng là mạng đủ rộng để hấp thụ DDoS.
- Khách đã đưa toàn bộ traffic qua thì khó chuyển đi, nên bán thêm bảo mật dễ hơn.

DNS có hai vai trò khác nhau ([5.8.3](../05-building-blocks/08-components/03-dns-and-traffic-steering.md)):
DNS điều hướng nội bộ (cần từ MVP để đưa người dùng tới PoP khoẻ, gần nhất) và DNS authoritative cho
zone của khách (sản phẩm bán thêm).

## 2. Tiêu chí

| Tiêu chí | Ý nghĩa |
|---|---|
| Giá trị bán | Có bán được như gói trọn (bảo mật + hiệu năng) không |
| Chi phí | Băng thông, lưu trữ cache, vận hành |
| Thời gian ra thị trường | Cái gì phải có cho MVP |
| Độ trễ | Số chặng thêm vào đường đi |
| Kiểm soát | Tự quyết cache, purge, routing, khả năng chống DDoS |
| Hybrid / on-prem | Có chạy được khi edge đặt trong hạ tầng khách không |

## 3. Phương án

| | A. Tự xây CDN theo giai đoạn | B. Đứng sau CDN của khách | C. Bán lại CDN đối tác, WAAP đứng sau | D. Chỉ WAAP, không CDN |
|---|---|---|---|---|
| Đường đi | User → edge (cache + WAF) → origin | User → CDN khách → edge WAAP → origin | User → CDN đối tác → edge WAAP → origin | User → edge WAAP → origin |
| Giá trị bán | Cao nhất: gói trọn | Trung bình: bán được cho khách đã có hợp đồng CDN | Trung bình: gói trọn nhưng biên lợi nhuận mỏng | Thấp: chỉ bảo mật |
| Chi phí | Đầu tư cache, băng thông lớn | Thấp | Trả theo băng thông cho đối tác | Thấp |
| Ra thị trường | Cache cơ bản nhanh (nằm sẵn trong proxy); CDN đầy đủ mất nhiều giai đoạn | Nhanh | Trung bình (tích hợp, hợp đồng) | Nhanh nhất |
| Độ trễ | Không thêm chặng | Thêm một chặng; cache nằm trước WAF nên request cache hit không qua WAF | Thêm một chặng | Thêm một chặng so với đi thẳng origin |
| Kiểm soát | Toàn quyền | Mất IP thật nếu không cấu hình header tin cậy; DDoS do CDN khách gánh | Phụ thuộc đối tác | Toàn quyền |
| Hybrid / on-prem | Cache chạy được trong hạ tầng khách | Phù hợp | Không | Phù hợp |

## 4. Đề xuất: A theo giai đoạn, kèm B như chế độ tương thích

| Giai đoạn | CDN | DNS |
|---|---|---|
| **MVP** | Cache cơ bản trong tầng proxy: tôn trọng `Cache-Control`, cache rule theo path/đuôi file, purge theo URL và prefix, `stale-if-error`, cache status trong log | DNS điều hướng nội bộ (GeoDNS + health); khách onboard bằng CNAME |
| **MVP** | Chế độ **"đứng sau CDN của khách"** (phương án B): danh sách IP nguồn tin cậy của CDN khách, lấy IP client từ header khách chỉ định, từ chối traffic không đến từ CDN đó | |
| **Mở rộng** | Tiered cache / origin shield, purge theo tag, cache key tuỳ biến, `stale-while-revalidate`, cache theo thiết bị | DNS authoritative cho zone của khách: bản ghi proxied, DNSSEC, load balancing và origin failover |
| **Nâng cao** | Tối ưu ảnh, edge compute, video | DNS firewall, phân tích truy vấn |

Lý do không chọn C làm đường chính: biên lợi nhuận phụ thuộc giá băng thông của đối tác và không kiểm
soát được lớp cache, nhưng vẫn có thể dùng như kênh mở rộng vùng phủ ([ADR-0005](0005-edge-on-own-and-existing-infra.md)).

## 5. Hệ quả nếu chọn đề xuất

- Khả năng cache trở thành một tiêu chí khi chọn nền proxy ([ADR-0031](0031-edge-l7-and-waf-engine-options.md)).
- PoP cần NVMe cho cache và băng thông lớn hơn; metering tính theo băng thông phục vụ (đã có trong
  `usage_1h`).
- Management thêm cache rule, purge, và sau này bản ghi DNS ([5.8.12](../05-building-blocks/08-components/12-management.md)).
- Purge là một loại thay đổi đi fast lane, có SLO riêng (QS-15).
- DNS server dùng CoreDNS (Apache-2.0) với plugin riêng; PowerDNS và Knot (GPL) không dùng trong bản
  phát hành theo [ADR-0024](0024-licensing-policy-for-shipped-components.md).
