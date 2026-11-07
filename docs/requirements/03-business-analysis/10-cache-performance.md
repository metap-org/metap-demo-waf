# 3.10 Cache, hiệu năng, nội dung

> **Trạng thái: đang mở, chưa có quyết định nào (2026-11-07).** Theo chủ dự án, cache là một vùng nghiệp vụ lớn,
> cần phân tích riêng. Trang này chỉ liệt kê kịch bản và câu hỏi. Chiến lược CDN ở kiến trúc ADR-0035.

## 1. Phạm vi

Khách dùng edge để phục vụ nội dung nhanh hơn và giảm tải origin: quyết định cái gì được lưu, lưu bao lâu, xoá
khi nào; tránh để cache làm lộ dữ liệu hoặc bị đầu độc; và chịu trách nhiệm về nội dung đi qua edge. Liên quan:
F4 ở [§1](../01-functional-decomposition.md#f4-hiệu-năng-và-phân-phối).

Cache là lý do nhiều khách dùng dịch vụ (ADR-0035), nhưng cũng là chỗ dễ gây sự cố nhất sau chặn nhầm: một
quyết định cache sai có thể đưa trang của người này cho người khác.

## 2. Kịch bản theo từng trục

### A. Loại nội dung

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-A1 | Tài nguyên tĩnh (ảnh, CSS, JS, font) | An toàn, lợi ích lớn |
| KB10-A2 | HTML công khai (trang tin, trang sản phẩm) | Cache được nếu không phụ thuộc người dùng |
| KB10-A3 | HTML theo người dùng (giỏ hàng, trang cá nhân, đã đăng nhập) | Không được cache, hoặc chỉ cache phần chung |
| KB10-A4 | API GET công khai (danh sách sản phẩm, tỉ giá) | TTL ngắn |
| KB10-A5 | File lớn tải xuống, tải theo đoạn (range) | Dung lượng cache theo gói, PoP |
| KB10-A6 | Video theo đoạn (HLS, DASH) | Lưu lượng lớn; đang ở mức Nâng cao |
| KB10-A7 | Trang lỗi (404, 500) | Cache trang lỗi làm lỗi kéo dài |

### B. Quyết định cache

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-B1 | Origin gửi `Cache-Control` đúng | Tôn trọng |
| KB10-B2 | Origin không gửi gì, hoặc gửi sai (WordPress mặc định, framework) | Mặc định cache gì |
| KB10-B3 | Response có `Set-Cookie`, request có `Authorization` hoặc cookie phiên | Không cache, kể cả khi khách cấu hình nhầm |
| KB10-B4 | Cache key: query string (bỏ `utm_*`, `fbclid`), cookie, ngôn ngữ, thiết bị, quốc gia | Mỗi biến thể nhân số bản lưu |
| KB10-B5 | `Vary` từ origin | `Vary: *`, `Vary: User-Agent` làm cache vô dụng |
| KB10-B6 | Khách ghi đè TTL bằng cache rule | Rule rộng quá cache cả trang riêng tư |

### C. An toàn của cache

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-C1 | Cache deception: `/account/profile.php/x.css` được cache như file tĩnh | Lộ trang riêng tư của người dùng |
| KB10-C2 | Cache poisoning: header không nằm trong cache key (`X-Forwarded-Host`) làm origin trả nội dung độc, bị lưu cho mọi người | |
| KB10-C3 | Một người dùng đã đăng nhập làm trang công khai bị lưu kèm dữ liệu của họ | Lỗi của origin, nhưng hậu quả do cache |
| KB10-C4 | Cache phần đã bị WAF chặn hoặc đang bị challenge | Không được lưu |

### D. Thời gian sống và làm mới

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-D1 | `stale-while-revalidate`, `stale-if-error` | Đã có `stale-if-error` ở MVP |
| KB10-D2 | Kiểm lại với origin (`ETag`, `304`) | Origin không hỗ trợ |
| KB10-D3 | Nội dung cần cập nhật ngay (giá, tồn kho) | TTL ngắn, hoặc xoá theo sự kiện |

### E. Xoá cache (purge)

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-E1 | Xoá theo URL, prefix, tag, toàn zone | Hạn mức theo gói (8.18) |
| KB10-E2 | Xoá sau mỗi lần deploy, gọi từ CI | Qua API |
| KB10-E3 | CMS tự xoá khi sửa bài (plugin WordPress) | Có làm plugin không |
| KB10-E4 | Xoá toàn zone lúc cao điểm | Mọi request dồn về origin cùng lúc |
| KB10-E5 | Nội dung bị gỡ ở origin vì pháp lý nhưng vẫn nằm trong cache | Phải xoá chắc chắn, có xác nhận |
| KB10-E6 | Purge có cần change request không | Purge là lệnh, không phải cấu hình (đề xuất ở OQ-33) |

### F. Bảo vệ origin

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-F1 | Nhiều request cùng lúc cho một object chưa có trong cache | Gộp thành một request về origin |
| KB10-F2 | Nhiều PoP cùng hỏi origin | Tầng cache trung gian (origin shield), Mở rộng |
| KB10-F3 | Cache lạnh sau khi mở PoP mới, sau khi xoá toàn zone | Origin chịu tải đột biến |

### G. Tối ưu khác

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-G1 | Nén gzip, brotli | Có làm ở edge không khi origin đã nén |
| KB10-G2 | HTTP/2, HTTP/3 tới người dùng | |
| KB10-G3 | Tối ưu ảnh, đổi định dạng (WebP, AVIF) | Mức Nâng cao |
| KB10-G4 | Người dùng ở nước ngoài truy cập website trong nước | PoP ở đâu |

### H. Thương mại và đo lường

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-H1 | Dung lượng cache theo gói, theo PoP | Gói thấp bị đẩy ra trước |
| KB10-H2 | Băng thông phục vụ từ cache có tính tiền không | Theo 8.18 thì tính là băng thông sạch |
| KB10-H3 | Khách muốn thấy tỉ lệ cache hit, băng thông origin tiết kiệm được | Chứng minh giá trị |
| KB10-H4 | Khách gỡ lỗi: vì sao trang này không được cache | Header trạng thái cache, công cụ xem cache key |

### I. Nội dung đi qua edge

| Mã | Kịch bản | Concern |
|---|---|---|
| KB10-I1 | Website lừa đảo, phishing, phát tán mã độc dùng edge để giấu origin | Nhận báo cáo abuse, xác minh, khoá; NCC có trách nhiệm gì |
| KB10-I2 | Yêu cầu gỡ nội dung vi phạm bản quyền | Quy trình |
| KB10-I3 | Nội dung bị cấm theo pháp luật Việt Nam, yêu cầu của cơ quan quản lý | Chặn theo quốc gia, khoá zone |
| KB10-I4 | Yêu cầu tiết lộ origin thật của một website (nạn nhân, cơ quan chức năng) | Quy trình, căn cứ |

## 3. Câu hỏi cần chủ dự án quyết

| ID | Câu hỏi | Kịch bản | Ghi chú ban đầu |
|---|---|---|---|
| OQ-86 | Mặc định cache gì khi origin không nói gì? | KB10-A1…KB10-A3, KB10-B2 | Chỉ tài nguyên tĩnh theo đuôi file là an toàn |
| OQ-87 | Có làm plugin cho CMS phổ biến (WordPress) không? | KB10-E3 | |
| OQ-88 | Chính sách abuse: ai nhận báo cáo, thời gian xử lý, khi nào khoá zone, có tiết lộ origin không? | KB10-I1…KB10-I4 | Cần tư vấn pháp lý; dịch vụ che origin luôn bị dùng cho mục đích xấu |
| OQ-112 | Có luôn chặn cache response có `Set-Cookie` hoặc request có `Authorization`, kể cả khi khách cấu hình cache rule? | KB10-B3, KB10-B6 | Đây là chỗ chống rò rỉ |
| OQ-113 | Có chống cache deception (kiểm `Content-Type` khớp đuôi file) mặc định không? | KB10-C1 | |
| OQ-114 | Cache key mặc định: bỏ tham số marketing (`utm_*`, `fbclid`) không? | KB10-B4 | |
| OQ-115 | Xoá toàn zone: có giới hạn hoặc xác nhận thêm không, để tránh dội origin? | KB10-E4, KB10-F3 | |
| OQ-116 | Băng thông phục vụ từ cache có tính phí như băng thông sạch không? | KB10-H2 | Ảnh hưởng 8.18 |
| OQ-117 | Nén và HTTP/3 có ở MVP không? | KB10-G1, KB10-G2 | |

## 4. Phụ thuộc

FR-4-01…FR-4-04 ở [§2](../02-requirements.md); kiến trúc ADR-0035, 8.18. Chính sách abuse và an toàn cache
chưa có requirement nào.
