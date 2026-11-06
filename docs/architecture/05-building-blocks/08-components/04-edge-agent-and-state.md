# 5.8.4 Policy agent, state sync, log shipping trên node

Ba thành phần chạy cạnh hai tầng edge ([ADR-0033](../../09-adr/0033-two-tier-edge-proxy-and-waf.md)):

- **Policy agent** là module trong binary của **cả hai tầng** (edge proxy và WAF inspection); mỗi tầng
  chỉ nạp phần bundle của mình từ cùng một manifest, nên hai tầng luôn chạy cùng version policy.
- **State sync** là module trong binary edge proxy (rate limit nằm ở proxy).
- **Vector** là process riêng trên mỗi node, nhận bản ghi từ cả hai tầng.

Diagram dưới vẽ cho tầng proxy; tầng WAF giống phần policy agent, không có state sync.

## Component diagram

```mermaid
flowchart LR
    dist["Control · Distribution<br/>gRPC stream"]
    relay["Regional relay<br/>HTTPS theo digest"]
    keys["Control · Cert & key<br/>mTLS"]
    vk[("Valkey region")]
    ig["Analytics · Ingest gateway"]

    subgraph BIN["edge binary"]
        subgraph AG["Policy agent"]
            enr["Enrollment<br/>CSR · gia hạn cert node"]
            cc["Control client<br/>desired state · ACK/NACK"]
            fet["Fetcher<br/>digest · chữ ký"]
            ld["Loader<br/>decode · dựng runtime"]
            sw["Swapper<br/>con trỏ nguyên tử"]
            lg[("Last-good<br/>đĩa, không bí mật")]
            sec["Secret fetcher<br/>key cert trong RAM"]
            vcfg["Vector config writer"]
        end
        subgraph ST["State sync"]
            hk["Hot key detector"]
            sc["Sync client"]
        end
        gw["Edge proxy"]
    end
    vec["Vector<br/>buffer đĩa"]

    enr --> cc
    dist <--> cc
    cc --> fet
    relay --> fet
    fet --> ld --> sw --> gw
    ld --> lg
    lg -.->|khởi động khi mất Control| ld
    keys --> sec --> ld
    ld --> vcfg --> vec
    gw <--> hk --> sc <--> vk
    gw -->|Unix socket| vec -->|vector protocol, mTLS| ig
```

## Policy agent

| Component | Trách nhiệm | Thiết kế |
|---|---|---|
| **Enrollment** | Danh tính node | Lần đầu: bootstrap token một lần dùng + CSR → OpenBao PKI cấp cert ngắn hạn (24 h); tự gia hạn ở 2/3 thời hạn; cert chứa node ID, nơi chạy, region |
| **Control client** | Kênh điều khiển | gRPC stream hai chiều có mTLS; nhận `DesiredState{manifest_digest, nonce, urgent}`; trả `Ack{nonce}` hoặc `Nack{nonce, error}`; heartbeat 10 s kèm `NodeStatus` |
| **Fetcher** | Lấy bundle | Kéo manifest + bundle còn thiếu theo digest từ relay; kiểm SHA-256 từng bundle, kiểm chữ ký Ed25519 của manifest bằng khoá công khai nhúng sẵn (kèm danh sách khoá xoay vòng) |
| **Loader** | Biến bundle thành runtime | Giải nén zstd, decode protobuf, kiểm schema version; dựng trie IP, evaluator IR của custom rule, instance engine WAF, template; chạy **self-test** (vài request mẫu đi qua pipeline) trước khi chấp nhận |
| **Swapper** | Áp nguyên khối | Con trỏ nguyên tử tới `GlobalRuntime` và map zone bất biến (`arc-swap` ở Rust, `atomic.Pointer` ở Go); request đang chạy giữ runtime cũ tới khi xong |
| **Lớp áp dụng** | Đổi nóng hay build nền | Theo [8.15](../../08-cross-cutting/15-config-propagation-and-apply.md): phần lớp B (ruleset release, MMDB) build ở thread nền giới hạn CPU, kiểm RAM trước (thiếu thì NACK `insufficient_memory`), so le trong PoP theo lệnh của distribution |
| **Last-good** | Khởi động độc lập | Giữ 3 manifest gần nhất đã nạp thành công trên đĩa (rollback chỉ cần swap); khởi động khi không gọi được Control thì nạp từ đây |
| **Secret fetcher** | Private key cert | Gọi Cert & key service qua mTLS lấy key của zone được gán, chỉ giữ trong RAM; mất kết nối khi khởi động thì zone đó không phục vụ TLS cho tới khi lấy được |
| **Vector config writer** | Đồng bộ cấu hình log | Sinh file cấu hình Vector (tỉ lệ lấy mẫu theo zone, luật che) từ snapshot, gọi reload |

### Trạng thái nạp

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Fetching: DesiredState mới
    Fetching --> Verifying: đủ bundle
    Fetching --> Failed: lỗi tải
    Verifying --> Loading: digest + chữ ký đúng
    Verifying --> Failed: sai chữ ký
    Loading --> SelfTest
    SelfTest --> Active: pass → swap, ACK
    SelfTest --> Failed: fail
    Loading --> Failed: schema lạ, lỗi dựng
    Failed --> Idle: NACK, giữ runtime cũ
    Active --> Idle
```

## State sync

| Component | Thiết kế |
|---|---|
| **Hot key detector** | Mỗi counter cục bộ có cờ hot khi vượt 10% ngưỡng của rule trong cửa sổ hiện tại; chỉ hot key được đồng bộ |
| **Sync client** | Mỗi 1 s gửi pipeline `INCRBY key Δ` + `EXPIRE` theo cửa sổ tới Valkey của region, nhận tổng; khoá Valkey: `rl:{tenant}:{rule}:{hash(key)}:{window}`; giới hạn số khoá mỗi lần gửi theo tenant |
| **Hợp nhất** | `count = max(local, regional_last + local_unsynced)` |
| **Lỗi** | Valkey không trả lời → dùng `local`, metrics `ratelimit_sync_degraded` |

## Log shipping (Vector)

| Giai đoạn | Thiết kế |
|---|---|
| Nguồn | Unix socket, bản ghi protobuf có độ dài tiền tố |
| Biến đổi | Lấy mẫu access log theo zone (security event không lấy mẫu); che trường theo cấu hình khách bằng VRL; gắn `node_id`, `region` |
| Buffer | Disk buffer có giới hạn dung lượng; khi đầy, nguồn access log bị chặn trước, security event dùng buffer riêng |
| Sink | Ingest gateway của Analytics region được gán, qua giao thức `vector` (gRPC) có mTLS bằng cert node, nén zstd; chỉ xoá khỏi buffer đĩa khi gateway xác nhận đã ghi Kafka (end-to-end ack). Edge không biết Kafka tồn tại |
| Quan sát | Metrics của Vector (độ trễ, mức buffer, lỗi gửi) được gom vào `/metrics` của node |
