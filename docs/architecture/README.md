# Kiến trúc WAAP mục tiêu

Tài liệu kiến trúc cho sản phẩm WAAP enterprise, dựng từ yêu cầu sản phẩm chứ **không** dựa trên
code hiện có trong repo này. Công nghệ cụ thể (store, stream, ngôn ngữ edge) chưa chốt; mọi khối là
vai trò, không phải sản phẩm.

Mở trực tiếp file HTML bằng trình duyệt, không cần build.

| File | Nội dung |
|---|---|
| [`waap-enterprise-blueprint.html`](waap-enterprise-blueprint.html) | Bản đồ nghiệp vụ (MVP / Mở rộng / Nâng cao), C1/C2, các luồng chính, NFR, phạm vi của `metap`, đối chiếu với demo, lộ trình 3 giai đoạn |
| [`waap-architecture-draft.html`](waap-architecture-draft.html) | Bản vẽ nháp: tổng thể 4 plane, bên trong Control / Edge / Analytics / Management, triển khai vật lý, sequence một thay đổi rule, câu hỏi mở |

Tên gọi: phần `data-plane/` của repo này là **Management plane** theo nghĩa chuẩn ngành; traffic
thật đi qua **Edge plane**; log và phân tích nằm ở **Analytics plane** (chưa có trong code).
`data-plane/docs/` vẫn là spec của phần portal hiện tại; thư mục này là kiến trúc đích.
