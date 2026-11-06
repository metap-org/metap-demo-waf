# ADR-0008: Under Attack mode theo zone

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Khi bị tấn công L7 lớn, khách cần một hành động duy nhất, có hiệu lực trong vài giây, không cần
viết rule. Đối thủ (Cloudflare) đã định hình kỳ vọng này.

## Quyết định

- Mỗi zone có công tắc **Under Attack mode**, bật/tắt qua portal và API, có hạn tự tắt tuỳ chọn.
- Bật/tắt đi **fast lane**, không qua duyệt, có audit; mục tiêu ≤ 5 s toàn fleet.
- Khi bật, request không có clearance token hợp lệ nhận challenge; token có MAC (HMAC), kiểm cục bộ
  ở mọi node ([08 §8.8](../08-cross-cutting/00-index.md#88-challenge-và-clearance)).
- Đứng ở bước 5 của pipeline: sau access list (allowlist vẫn qua), trước rate limit.
- Detection **chỉ cảnh báo và gợi ý bật** ở MVP; tự bật có hẹn giờ tắt là tuỳ chọn chưa chốt.

## Hệ quả

- Cần cơ chế challenge và clearance từ MVP, dùng lại cho bot management sau này.
- API client và bot tốt bị ảnh hưởng khi bật; cần exception theo path/API key hoặc allowlist.

## Phương án đã cân nhắc

- **Chỉ dùng rate limit chặt hơn**: không phân biệt được trình duyệt thật với botnet phân tán.
