# ADR-0010: Analytics: một luồng ghi, nhiều store theo kiểu đọc

- Trạng thái: Chấp nhận
- Ngày: 2026-11-05

## Bối cảnh

Log được đọc theo bốn cách khác nhau: tra từng request (điều tra, chặn nhầm), số liệu gộp
(dashboard, báo cáo), lưu dài hạn rẻ (tuân thủ, replay), và luồng liên tục (SIEM export, phát
hiện). Không store nào tốt cho cả bốn.

## Quyết định

- Edge ghi vào một **ingestion bus bền**, phân vùng theo tenant, ack sau khi ghi bền.
- Stream processor làm giàu và ghi ra **hot search**, **columnar store**, **archive**; nuôi
  **detection** và **metering**.
- **Log export đọc thẳng từ bus**, không từ store.
- **Query API** là cửa duy nhất để Management đọc dữ liệu traffic.
- Mọi bản ghi mang request ID, rule ID, policy version.

## Hệ quả

- Mỗi store tối ưu được chi phí và độ trễ cho kiểu đọc của nó.
- Có nhiều bản sao dữ liệu; retention phải áp nhất quán ở mọi store.
- Chọn công nghệ cho bus và từng store là quyết định riêng, chưa chốt.

## Phương án đã cân nhắc

- **Một store tìm kiếm cho mọi thứ**: đắt ở quy mô access log đầy đủ, yếu ở số liệu gộp dài hạn.
- **Ghi log vào database của Management**: như demo hiện tại; không scale, không residency.
