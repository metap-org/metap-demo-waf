# Overview HTML (v1)

Bản HTML để đọc và trình bày kiến trúc WAAP. Nguồn chính vẫn là Markdown ở
[`../architecture/`](../architecture/index.md); các file ở đây là bản xem, không sửa tay.

Mở trực tiếp bằng trình duyệt (tải file về, hoặc qua GitHub Pages / trình xem HTML). GitHub không render
HTML trong giao diện repo. Cần mạng để tải font và Mermaid từ CDN; thiếu mạng thì sơ đồ hiện dạng text.

| File | Nội dung | Trạng thái |
|---|---|---|
| [`v1-waap-architecture.html`](v1-waap-architecture.html) | Toàn bộ tài liệu arc42 (mục 1–12, ADR 0001–0042) trong một trang, có mục lục | **Hiện hành**, sinh từ `../architecture/` |
| [`v1-waap-enterprise-blueprint.html`](v1-waap-enterprise-blueprint.html) | Blueprint nghiệp vụ WAAP enterprise: nhóm năng lực, 4 plane, lộ trình | Ảnh chụp 2026-11-05; phần kiến trúc đã được thay bằng bản arc42 |
| [`v1-waap-architecture-draft.html`](v1-waap-architecture-draft.html) | Kiến trúc nháp level 1–2, chưa gắn công nghệ | Ảnh chụp 2026-11-05; chưa có edge hai tầng, CDN, sharding |
| [`v1-waap-architecture-c1-c3.html`](v1-waap-architecture-c1-c3.html) | Bản C1–C3 đầu tiên | Ảnh chụp 2026-11-05; lựa chọn công nghệ ở đây (Go cho mọi plane, edge một tầng) đã bị thay bởi ADR-0030, ADR-0031, ADR-0033 |

Khi bản hiện hành và ảnh chụp mâu thuẫn, bản hiện hành (và Markdown) đúng.

## Sinh lại bản hiện hành

Sau khi sửa Markdown trong `../architecture/`:

```bash
pip install -r docs/overview/tools/requirements.txt
python3 docs/overview/tools/render_arch.py
```

Không tham số: đọc `docs/architecture/`, ghi đè `docs/overview/v1-waap-architecture.html`, chạy được từ
thư mục nào cũng được. Muốn ghi ra file khác (ví dụ `v2-…`):
`python3 docs/overview/tools/render_arch.py docs/architecture docs/overview/v2-waap-architecture.html`.

`tools/arch_template.html` là khung trang (mục lục, giao diện sáng/tối, nạp Mermaid). Thay đổi lớn về cấu
trúc tài liệu nên tăng prefix (`v2-…`) thay vì ghi đè, để giữ được bản đã gửi cho người khác.
