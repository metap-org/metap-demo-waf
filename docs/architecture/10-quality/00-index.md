# 10. Quality Requirements

Các con số là **mục tiêu đề xuất**, cần chốt cùng product trước khi thiết kế chi tiết. Mỗi kịch
bản phải đo được bằng chỉ số nền tảng ở [08 §8.11](../08-cross-cutting/00-index.md#811-quan-sát-chính-nền-tảng).

## 10.1 Cây chất lượng

```mermaid
flowchart LR
    q["Chất lượng"]
    q --> q1["Q1 Edge nhanh, luôn sống"]
    q --> q2["Q2 Thay đổi an toàn"]
    q --> q3["Q3 Cô lập tenant"]
    q --> q4["Q4 Bằng chứng đầy đủ"]
    q --> q5["Q5 Triển khai nhiều nơi"]
    q1 --> l1["Độ trễ thêm"]
    q1 --> l2["Khả dụng edge"]
    q2 --> s1["Lan truyền"]
    q2 --> s2["Giới hạn tác động config lỗi"]
    q2 --> s3["Rollback"]
    q3 --> t1["Noisy neighbor"]
    q3 --> t2["Truy cập dữ liệu chéo"]
    q4 --> e1["Không mất log"]
    q4 --> e2["Độ trễ tới tra cứu"]
    q4 --> e3["Truy vết quyết định"]
    q5 --> p1["Cùng artifact"]
```

## 10.2 Kịch bản

| ID | Thuộc tính | Kịch bản | Mục tiêu |
|---|---|---|---|
| QS-1 | Độ trễ | Request bình thường qua zone có CRS paranoia 1 và 50 custom rule | Độ trễ thêm p99 ≤ 2 ms, p50 ≤ 0,5 ms (không tính kết nối tới origin) |
| QS-2 | Khả dụng | Core region sập 4 giờ | Traffic khách không bị ảnh hưởng; edge ≥ 99,99%/tháng |
| QS-3 | Khả dụng | Management bảo trì | Portal ≥ 99,9%/tháng; traffic không bị ảnh hưởng |
| QS-4 | Lan truyền | Bản policy được duyệt, đường thường không có canary hold | p95 ≤ 30 s tới 100% node đang sống |
| QS-5 | Lan truyền | Bật Under Attack mode hoặc rule khẩn | ≤ 5 s tới 95% node, ≤ 15 s tới 100% |
| QS-6 | An toàn thay đổi | Snapshot làm tăng lỗi hoặc độ trễ lọt qua compiler | Chỉ nhóm canary bị ảnh hưởng; tự rollback trong ≤ 2 phút |
| QS-7 | Rollback | Người dùng bấm rollback | Version trước trên 100% node trong ≤ 60 s |
| QS-8 | Cô lập | Một tenant gửi body lớn và regex nặng gấp 10 lần bình thường | p99 của tenant khác trên cùng node tăng ≤ 10% |
| QS-9 | Cô lập | User tenant A gọi Query API với zone của tenant B | Bị từ chối; có audit |
| QS-10 | Không mất log | Analytics region không nhận log trong 6 giờ | Không mất security event; access log mất chỉ khi buffer đĩa đầy, có chỉ số báo |
| QS-11 | Độ trễ log | Request bị chặn | Tra cứu được theo request ID trong ≤ 60 s |
| QS-12 | Truy vết | Bất kỳ request bị chặn nào | Xác định được rule ID, bước pipeline, policy version, người duyệt version đó |
| QS-13 | Triển khai | Cùng bản build edge | Chạy trên PoP riêng, VM đối tác, container trong hạ tầng khách mà không đổi code |
| QS-14 | Thương mại | Khách đổi gói | Entitlements mới có hiệu lực ≤ 5 phút, không deploy |

## 10.3 Quy mô thiết kế

Cần product xác nhận; dùng để chọn công nghệ.

| Chỉ số | Giả định ban đầu |
|---|---|
| Request / giây toàn fleet (đỉnh) | 1 triệu |
| Tenant / zone | 1.000 / 20.000 |
| Rule / zone (tối đa theo gói cao nhất) | 1.000 custom rule |
| Access log / ngày (trước lấy mẫu) | ~ 50 tỉ bản ghi |
| Retention tối đa | 365 ngày (archive), 30 ngày (hot) |
