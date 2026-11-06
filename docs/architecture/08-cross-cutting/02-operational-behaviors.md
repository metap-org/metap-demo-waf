# 8.13 Hành vi vận hành

Mỗi hành vi được phân tích theo cùng khung: **mục tiêu** (SLO hoặc điều kiện thành công), **tín hiệu**
(đo bằng gì), **cảnh báo** (điều kiện cụ thể, mức), **tự phản ứng** (hệ thống tự làm gì), **runbook**
(người trực làm gì). SLO, probe và thuật toán burn rate ở [01-platform-monitoring.md](01-platform-monitoring.md).

Mức cảnh báo: **P1** gọi on-call ngay; **P2** gọi on-call trong giờ, ngoài giờ trong 30 phút; **P3**
ticket trong giờ làm việc.

## A. Hành vi của hệ thống khi đang chạy

### A1. Một PoP xuống cấp hoặc sập

| | |
|---|---|
| Mục tiêu | Traffic của PoP chuyển sang PoP khác trong ≤ 60 s; S1 không vỡ |
| Tín hiệu | Probe P1 theo PoP từ ≥ 3 điểm đo; tỉ lệ lỗi `error_source=platform` theo PoP; số node sống của PoP |
| Cảnh báo | P1: P1 lỗi từ ≥ 2/3 điểm đo trong 2 chu kỳ liên tiếp. P2: một PoP mất > 30% node |
| Tự phản ứng | Health checker rút PoP khỏi anycast (rút route) hoặc khỏi GeoDNS; trả lại khi P1 xanh 5 phút liên tục |
| Runbook | Xác nhận đã rút; kiểm hạ tầng/đối tác của PoP; kiểm dung lượng PoP lân cận đủ gánh; không trả PoP thủ công khi P1 chưa xanh |

### A2. Node nạp snapshot lỗi (NACK)

| | |
|---|---|
| Mục tiêu | Không node nào chạy snapshot hỏng; node lỗi vẫn phục vụ bằng bản cũ |
| Tín hiệu | `waap_agent_nack_total{stage}`; tỉ lệ NACK theo manifest trong fleet registry |
| Cảnh báo | P2: một manifest bị NACK ở > 1% node đích. P3: một node NACK lặp lại > 3 manifest liên tiếp |
| Tự phản ứng | Orchestrator dừng đợt khi NACK vượt ngưỡng gate, rollback các node đã nhận |
| Runbook | Đọc `stage` + lỗi trong NACK (chữ ký, schema, dựng CEL/Coraza, self-test); nếu lỗi theo nhóm node → kiểm phiên bản phần mềm edge của nhóm đó; nếu lỗi theo manifest → chặn manifest, báo đội compiler |

### A3. Node mất kết nối Control lâu

| | |
|---|---|
| Mục tiêu | Node vẫn phục vụ; độ lệch version được biết và giới hạn |
| Tín hiệu | `waap_agent_control_connected`; độ lệch giữa manifest node đang chạy và manifest đích; tuổi của snapshot (`last_good_age`) |
| Cảnh báo | P3: node lệch version > 10 phút. P2: > 5% node một region lệch > 10 phút (thường là sự cố mạng tới Control/relay) |
| Tự phản ứng | Agent thử lại với backoff, chuyển relay dự phòng; node lệch quá 24 h tự rút khỏi health check của PoP (để không phục vụ bằng config quá cũ) |
| Runbook | Kiểm relay của region, kết nối tới distribution, cert node còn hạn; nếu cert node hết hạn do mất kết nối kéo dài → enroll lại |

### A4. Độ trễ thêm tại edge tăng

| | |
|---|---|
| Mục tiêu | S2 |
| Tín hiệu | `waap_edge_phase_duration_seconds{phase}` theo node, zone; GC pause; CPU; tỉ lệ request kiểm tra body |
| Cảnh báo | Burn rate S2 (page/ticket theo §5 của 01-platform-monitoring) |
| Tự phản ứng | Resource governor hạ cấp tenant vượt quota; phase quá ngân sách áp fail mode của zone |
| Runbook | Tìm phase tăng (thường WAF); khoanh theo zone/tenant (rule mới? paranoia tăng? body lớn?); theo node (GC, CPU, hàng xóm ồn); nếu do release phần mềm hoặc ruleset mới → rollback release đó |

### A5. Fail mode bị kích hoạt hàng loạt

| | |
|---|---|
| Mục tiêu | Fail-open không kéo dài âm thầm (WAF không chặn mà không ai biết) |
| Tín hiệu | `waap_edge_failmode_triggered_total{zone,phase,mode}`; probe P2 |
| Cảnh báo | P1: probe P2 thất bại ở bất kỳ PoP nào (WAF thực tế không chặn). P2: tỉ lệ request bị fail-open của một phase > 0,1% trong 10 phút toàn fleet |
| Tự phản ứng | Báo khách bị ảnh hưởng qua cảnh báo hệ thống trong portal |
| Runbook | Như A4 để tìm nguyên nhân; nếu fail-open do snapshot thiếu phase → rollback manifest; ghi sự cố, thông báo khách nếu kéo dài > 15 phút |

### A6. Tenant gây ồn (noisy neighbor)

| | |
|---|---|
| Mục tiêu | Tenant khác trên cùng node không bị ảnh hưởng (QS-8) |
| Tín hiệu | `waap_edge_governor_throttled_total{tenant}`; CPU kiểm tra body theo tenant; p99 của các tenant khác trên node đó |
| Cảnh báo | P3: một tenant bị throttle liên tục > 1 giờ. P2: p99 của tenant khác tăng > 10% cùng lúc với throttle |
| Tự phản ứng | Governor hạ cấp tenant ồn theo fail mode của chính tenant đó |
| Runbook | Liên hệ khách (rule đắt? bị tấn công body lớn?); đề xuất gói cao hơn hoặc chỉnh rule; nếu cần, chuyển zone đó sang nhóm node riêng (khi đã có phân công zone theo node) |

### A7. Pipeline log trễ

| | |
|---|---|
| Mục tiêu | S6 (tra cứu được trong ≤ 60 s) |
| Tín hiệu | Probe P5; consumer lag của enricher (theo giây, không theo số bản ghi); lỗi insert ClickHouse; buffer đĩa Vector trên edge |
| Cảnh báo | Burn rate S6. P2: lag enricher > 120 s trong 5 phút. P3: buffer đĩa edge > 50% ở > 5% node |
| Tự phản ứng | HPA scale enricher theo lag; ingest gateway ưu tiên security event khi bị dồn |
| Runbook | Lag tăng đều mọi partition → thiếu năng lực enricher hoặc ClickHouse chậm (kiểm số part, merge); lag ở một số partition → tenant đột biến (khoá partition nóng); ClickHouse lỗi → enricher tự giữ offset, ưu tiên phục hồi ClickHouse |

### A8. Mất security event

| | |
|---|---|
| Mục tiêu | S7 |
| Tín hiệu | Job đối soát theo giờ (§7 của 01-platform-monitoring); `telemetry_dropped_total` trên edge; bản ghi bị ingest gateway từ chối |
| Cảnh báo | P3: lệch > 0,01% sau 2 giờ. P2: lệch tập trung ở một node hoặc một region, hoặc kéo dài 3 giờ |
| Tự phản ứng | Không (mất dữ liệu không tự phục hồi được); đánh dấu khoảng thời gian bị thiếu cho tenant liên quan |
| Runbook | Khoanh chặng mất: edge (ring buffer, buffer đĩa đầy) → ingest gateway (từ chối, throttle) → enricher (dedupe sai) → ClickHouse; báo khách nếu ảnh hưởng tới dữ liệu tuân thủ |

### A9. Đĩa sắp đầy (Kafka, ClickHouse, buffer edge)

| | |
|---|---|
| Mục tiêu | Không bao giờ đầy đĩa |
| Tín hiệu | Dung lượng còn lại và **tốc độ tăng** (dự báo tuyến tính 6 giờ) |
| Cảnh báo | P2: dự báo đầy trong < 24 h. P1: dự báo đầy trong < 4 h hoặc còn < 10% |
| Tự phản ứng | ClickHouse chuyển part sang volume S3 sớm hơn; Kafka không tự xoá thêm (retention cố định) |
| Runbook | Thêm dung lượng hoặc broker/shard; tìm tenant tăng đột biến; với Kafka, retention theo thời gian nên đĩa tăng là do lưu lượng tăng, không phải consumer chậm; với buffer edge, kiểm kết nối tới ingest gateway |

### A10. Cert của khách sắp hết hạn hoặc gia hạn lỗi

| | |
|---|---|
| Mục tiêu | Không zone nào phục vụ cert hết hạn |
| Tín hiệu | Số cert hết hạn < 14 ngày, < 3 ngày; lỗi ACME theo nguyên nhân (DNS sai, CA lỗi, rate limit của CA) |
| Cảnh báo | Khách: cảnh báo trong portal và email từ 14 ngày. NOC P3: > 1% cert lỗi gia hạn cùng nguyên nhân. P1: bất kỳ cert nào còn < 24 h |
| Tự phản ứng | Thử lại có backoff; chuyển CA dự phòng khi lỗi do CA |
| Runbook | Lỗi DNS → khách sửa bản ghi; lỗi CA → kiểm trạng thái CA, rate limit; cert < 24 h → liên hệ khách trực tiếp |

### A11. Khách bị tấn công lớn

| | |
|---|---|
| Mục tiêu | Khách tự xử lý được bằng công cụ có sẵn; nền tảng không bị kéo sập; khách khác không bị ảnh hưởng |
| Tín hiệu | Cảnh báo D1–D5 của khách ([32](../05-building-blocks/level-3/32-alerting-rules-and-algorithms.md)); rps theo PoP so với năng lực; tỉ lệ request bị DDoS L7 giảm thiểu |
| Cảnh báo | NOC P2: một zone chiếm > 20% rps của một PoP. P1: PoP vượt 80% năng lực |
| Tự phản ứng | DDoS L7 và rate limit của zone; Under Attack mode nếu khách đã bật tự động (chưa chốt); L3/L4 của hạ tầng/đối tác |
| Runbook | Liên hệ khách (gói Business+), đề xuất Under Attack mode, rule chặn theo top nguồn; nếu PoP quá tải → bật scrubbing của đối tác cho dải IP bị tấn công, phân tán qua nhiều PoP; ghi sự cố, báo cáo sau tấn công cho khách |

### A12. Provider kênh cảnh báo lỗi

| | |
|---|---|
| Mục tiêu | S8 |
| Tín hiệu | Tỉ lệ lỗi theo provider (SMTP relay, Slack, SMS); job tồn |
| Cảnh báo | Burn rate S8. P2: lỗi một provider > 10% trong 10 phút |
| Tự phản ứng | Chuyển provider dự phòng (SMTP, SMS); cảnh báo critical rơi về email |
| Runbook | Kiểm trạng thái provider, quota, khoá API; thông báo khách nếu một loại kênh ngừng > 30 phút |

## B. Thao tác của người vận hành

### B1. Rollout phần mềm edge

| | |
|---|---|
| Mục tiêu | Không vỡ S1, S2, S3 trong và sau rollout |
| Cách làm | Canary PoP → 10% node mỗi region → 50% → 100%; mỗi đợt quan sát ≥ 30 phút; node rút traffic (drain) trước khi khởi động lại, chờ kết nối cũ kết thúc |
| Gate | Error rate, p99 theo phase, NACK, probe P1/P2 trên node mới so với node cũ cùng PoP |
| Tự phản ứng | Gate hỏng → dừng, đưa node đã nâng về bản trước |
| Runbook | Không rollout ngoài giờ có người trực; không rollout đồng thời với release ruleset; theo dõi dashboard so sánh "bản mới vs bản cũ" |

### B2. Phát hành CRS hoặc system ruleset

| | |
|---|---|
| Mục tiêu | Không tăng chặn nhầm; không giảm khả năng phát hiện; không tăng độ trễ quá ngưỡng |
| Cách làm | Pipeline release ([22](../05-building-blocks/level-3/22-cert-ruleset-release.md)) → log-only song song toàn fleet ≥ 24 h → chuyển mặc định cho zone "tự nhận bản mới" |
| Gate | Chênh lệch tỉ lệ chặn bản mới vs bản cũ trên traffic thật theo rule; W5 (nghi chặn nhầm) trên dữ liệu log-only |
| Tự phản ứng | Dừng chuyển mặc định nếu gate hỏng |
| Runbook | Rule mới gây chênh lệch lớn → tắt rule đó trong release, phát hành lại; thông báo thay đổi cho khách cố định version |

### B3. Rule khẩn (zero-day)

| | |
|---|---|
| Mục tiêu | Tới 95% node trong ≤ 5 s (S5), không gây chặn nhầm diện rộng |
| Cách làm | Threat research viết rule → lint + test riêng + corpus traffic tốt → duyệt bởi người thứ hai của NCC → fast lane, ban đầu ở chế độ log-only 10 phút nếu tình huống cho phép |
| Gate | Số request khớp rule trong log-only; W5 cho rule đó |
| Tự phản ứng | Không (quyết định của người) |
| Runbook | Chuyển enforce khi log-only sạch; theo dõi 1 giờ; ghi vào changelog ruleset; chuyển rule vào release thường sau khi có bản CRS chính thức |

### B4. Thêm PoP, enroll node, ngừng node

| | |
|---|---|
| Mục tiêu | Node mới chỉ nhận traffic khi đã nạp snapshot và qua probe; node ngừng không làm rơi request |
| Cách làm | Thêm: bootstrap token một lần → enroll → nạp snapshot → probe P1/P2 trực tiếp IP node → vào health check. Ngừng: rút khỏi health check → drain → thu hồi cert node |
| Tín hiệu | Trạng thái enroll, `waap_agent_control_connected`, kết quả probe theo node |
| Runbook | Không thêm node vào anycast/DNS trước khi probe xanh; thu hồi cert ngay khi ngừng hoặc nghi bị chiếm |

### B5. Xoay khoá

| Khoá | Chu kỳ | Cách làm |
|---|---|---|
| Ký manifest (Ed25519, OpenBao) | 12 tháng hoặc khi nghi lộ | Phát khoá công khai mới trong bundle chung (node chấp nhận cả cũ và mới) → chờ 100% node ack → ký bằng khoá mới → bỏ khoá cũ khỏi danh sách sau 7 ngày |
| HMAC clearance token | 30 ngày | Thêm khoá mới làm khoá cấp, giữ khoá cũ để kiểm trong thời hạn token dài nhất, rồi bỏ |
| CA cho node | 24 tháng | CA trung gian mới, node gia hạn dần theo vòng đời cert 24 h; bỏ CA cũ khi không còn cert nào |
| Mã hoá key cert khách (transit) | 12 tháng | Xoay version khoá trong OpenBao, rewrap dần ciphertext |

Tín hiệu: tỉ lệ node đã có khoá mới, lỗi kiểm chữ ký/token sau xoay. Cảnh báo P1 nếu lỗi kiểm chữ ký
tăng sau khi xoay (dừng và quay lại khoá cũ).

### B6. Scale Analytics

| | |
|---|---|
| Khi nào | Dự báo dung lượng (A9) hoặc lag (A7) chạm ngưỡng; tăng tenant lớn |
| Kafka | Thêm broker, tăng partition (chỉ tăng, khoá `tenant_id` đổi partition chỉ ảnh hưởng thứ tự giữa bản ghi cũ và mới của tenant, chấp nhận được vì enricher bỏ trùng theo request ID) |
| ClickHouse | Thêm shard; dữ liệu mới phân theo hash `tenant_id`, dữ liệu cũ không di chuyển, hết hạn tự nhiên |
| Runbook | Thực hiện ngoài giờ cao điểm; theo dõi lag, lỗi insert trong 1 giờ sau thay đổi |

### B7. Sao lưu và DR core region

| Thành phần | Sao lưu | RPO / RTO mục tiêu |
|---|---|---|
| PostgreSQL (Management, Control) | WAL liên tục + snapshot hằng ngày sang region khác (CloudNativePG) | RPO ≤ 5 phút, RTO ≤ 1 giờ |
| OpenBao | Snapshot Raft hằng giờ, mã hoá, sang region khác; unseal key chia cho nhiều người | RPO ≤ 1 giờ, RTO ≤ 1 giờ |
| Artifact store | Replicate sang region khác | RPO ≤ 15 phút |
| ClickHouse | Không sao lưu raw (tái tạo được từ archive Parquet trong phạm vi retention); sao lưu bảng gộp và nhãn FP hằng ngày | RPO ≤ 24 giờ cho số liệu gộp |

Diễn tập DR mỗi quý: dựng core ở region dự phòng từ sao lưu, edge nối vào Control mới, đo RTO thật.
Trong lúc core sập, edge vẫn phục vụ (QS-2), nên RTO của core ảnh hưởng tới khả năng đổi config, không
ảnh hưởng traffic.

### B8. Nâng schema telemetry

| | |
|---|---|
| Cách làm | Thêm field mới vào protobuf (chỉ thêm, không đổi nghĩa field cũ) → nâng consumer (enricher, exporter, detector) đọc được N+1 → nâng ClickHouse schema (thêm cột) → rollout edge phát N+1 |
| Tín hiệu | Bản ghi bị ingest gateway đưa vào `telemetry.rejected.v1` |
| Cảnh báo | P2: rejected > 0 trong 5 phút sau rollout edge |
| Runbook | Rejected tăng → dừng rollout edge, kiểm consumer nào chưa nâng |

### B9. Xoá dữ liệu tenant

| | |
|---|---|
| Cách làm | Yêu cầu từ Management (chấm dứt hợp đồng hoặc yêu cầu pháp lý) → job xoá: ClickHouse theo `tenant_id`, prefix object storage, đích export, mẫu request → biên bản xoá vào audit |
| Tín hiệu | Trạng thái job, số dòng/object còn lại sau xoá |
| Cảnh báo | P3: job chưa xong sau 72 giờ |
| Runbook | Xác minh bằng truy vấn đếm sau khi xoá; gửi biên bản cho khách |

### B10. NCC truy cập dữ liệu khách để hỗ trợ

| | |
|---|---|
| Cách làm | Support yêu cầu quyền theo ticket → khách hoặc người duyệt NCC chấp nhận → quyền đọc có thời hạn (mặc định 4 giờ), chỉ trong phạm vi zone của ticket |
| Tín hiệu | Mọi truy vấn của support đi qua Query API với danh tính support, ghi audit, khách thấy được |
| Cảnh báo | P2: truy cập không có ticket hợp lệ (không được xảy ra; nếu có là lỗi phân quyền) |

## C. Trực vận hành

| Mục | Quy ước |
|---|---|
| Ca trực | Edge/Control và Analytics/Management là hai vòng trực riêng khi đội đủ người; giai đoạn đầu một vòng chung |
| Mức sự cố | SEV1: S1 hoặc S3 vỡ trên nhiều PoP, hoặc mất dữ liệu diện rộng. SEV2: một PoP hoặc một plane trung tâm xuống cấp. SEV3: một thành phần, chưa ảnh hưởng khách |
| Trang trạng thái | Cập nhật cho SEV1/SEV2 trong ≤ 15 phút |
| Sau sự cố | Postmortem không đổ lỗi cho SEV1/SEV2 trong 5 ngày làm việc; mỗi hành động khắc phục có người chịu trách nhiệm và hạn |
| Runbook | Mỗi cảnh báo P1/P2 có link tới runbook tương ứng trong tài liệu này; cảnh báo không có runbook không được lên production |
