# 5.8.1 Edge gateway

Một binary Rust chạy trên mỗi edge node, chứa gateway, policy agent và state sync
([ADR-0030](../../09-adr/0030-language-per-workload.md), [ADR-0031](../../09-adr/0031-edge-in-rust-on-pingora.md),
[ADR-0015](../../09-adr/0015-snapshot-format-and-distribution.md)). File này mô tả phần gateway; agent
và state sync ở [02-edge-agent-and-state.md](02-edge-agent-and-state.md).

## Component diagram

```mermaid
flowchart TB
    lb["L4 / anycast<br/>PROXY protocol"]
    subgraph GW["edge binary (Rust) · gateway trên Pingora"]
        lst["Listener<br/>TCP · QUIC"]
        tls["TLS terminator<br/>chọn cert theo SNI"]
        http["HTTP server<br/>h1 · h2 · h3"]
        rid["Request ID"]
        zr["Zone router<br/>host → ZoneRuntime"]
        gov["Resource governor<br/>quota theo tenant"]
        subgraph PIPE["crate pipeline (dùng chung với replay)"]
            norm["Normalizer"]
            acl["Access list"]
            uam["Challenge & clearance"]
            rl["Rate limiter"]
            waf["WafEngine<br/>MVP: libmodsecurity v3"]
            cel["Custom rule<br/>evaluator IR"]
            dec["Decision"]
        end
        resp["Response writer<br/>trang chặn · challenge"]
        up["Upstream proxy<br/>pool theo origin"]
        tel["Telemetry emitter<br/>ring buffer"]
        met["Metrics · health"]
    end
    snap[("ZoneRuntime / GlobalRuntime<br/>arc-swap")]
    sync["State sync"]
    vec["Vector · Unix socket"]
    origin["Origin"]

    lb --> lst --> tls --> http --> rid --> zr --> gov --> norm --> acl --> uam --> rl --> waf --> cel --> dec
    dec -->|block / challenge| resp
    dec -->|allow / log| up --> origin
    tls -.-> snap
    zr -.-> snap
    rl <--> sync
    dec --> tel
    up --> tel
    tel --> vec
```

## Thành phần

| Component | Trách nhiệm | Thiết kế |
|---|---|---|
| **Listener** | Nhận kết nối TCP (443, 80) và QUIC (443/udp) | Pingora listener; PROXY protocol v2 khi đứng sau L4 load balancer của đối tác; giới hạn kết nối theo IP nguồn. QUIC/HTTP/3: dùng hỗ trợ của Pingora nếu có, nếu chưa thì listener riêng bằng `quinn` + `h3` đổ vào cùng pipeline (cần kiểm trong spike) |
| **TLS terminator** | Chọn cert theo SNI, thương lượng TLS | Callback chọn cert tra bảng `hostname → cert` trong GlobalRuntime; rustls hoặc BoringSSL; session ticket key xoay qua bundle chung; ALPN `h2`, `http/1.1` |
| **HTTP server** | h1/h2 (Pingora), h3 | Timeout header/idle/body theo cấu hình zone; giới hạn kích thước header |
| **Request ID** | Định danh duy nhất cho mỗi request | UUIDv7 (có thứ tự thời gian); ghi vào header gửi origin (`X-Request-ID`), trang chặn và mọi bản ghi telemetry |
| **Zone router** | Tìm cấu hình zone theo `Host` | Map bất biến trong GlobalRuntime; host lạ trả 421 / trang mặc định, ghi event |
| **Resource governor** | Cô lập tenant | Semaphore số request đồng thời và ngân sách CPU kiểm tra body theo tenant; vượt quota thì hạ cấp theo fail mode của zone |
| **Normalizer** | Chuẩn hoá trước khi kiểm tra | Decode percent/unicode, gộp `//`, `/./`, `/../`; giới hạn số tham số, độ sâu JSON; giữ cả bản gốc cho engine WAF (CRS tự chuẩn hoá bằng transform riêng) |
| **Access list** | IP/CIDR/geo/ASN | Trie IPv4/IPv6 dựng sẵn khi nạp snapshot; geo/ASN tra MMDB trong RAM; kết quả `allow` gắn cờ bỏ qua các phase đã cấu hình |
| **Challenge & clearance** | Under Attack mode, challenge của rule | Token = HMAC-SHA256(zone, hạn, IP prefix, UA hash) với khoá trong bundle chung (khoá hiện tại + khoá trước); challenge JS proof-of-work, mức khó theo cấu hình |
| **Rate limiter** | Counter theo khoá rule | Sliding window hai bucket, map phân mảnh theo hash để giảm tranh chấp; hot key gửi state sync ([ADR-0021](../../09-adr/0021-valkey-for-hot-key-rate-limit-sync.md)) |
| **WafEngine** | Chạy CRS + system ruleset | Trait, MVP hiện thực bằng libmodsecurity v3 qua FFI: một instance rules dựng sẵn theo `(ruleset release, cấu hình CRS của zone)`, dùng chung giữa các zone cùng cấu hình; mỗi request một transaction; đọc body tối đa theo entitlements, phần còn lại stream thẳng. Engine thuần Rust thay vào khi qua bộ test CRS ([ADR-0031](../../09-adr/0031-edge-in-rust-on-pingora.md)) |
| **Custom rule** | Biểu thức của khách theo priority | Thực thi **IR** do Control biên dịch từ CEL, không parse CEL ở edge ([ADR-0032](../../09-adr/0032-cel-checked-at-control-ir-at-edge.md)); IR dựng thành closure lúc nạp snapshot; first match wins; giới hạn số bước đánh giá |
| **Decision** | Kết hợp kết quả các phase | allow / log / challenge / block; log-only hạ mọi hành động; ghi `Verdict` (phase, rule ID, hành động, điểm anomaly) |
| **Response writer** | Trang chặn, challenge | Template theo zone (logo, màu), luôn in request ID và thời điểm |
| **Upstream proxy** | Chuyển tới origin | Pool kết nối của Pingora theo origin; mTLS client cert; timeout, retry cho request idempotent, health check thụ động |
| **Telemetry emitter** | Phát bản ghi | Ring buffer không khoá, task riêng gửi Unix socket tới Vector; đầy thì bỏ access log trước, tăng bộ đếm `telemetry_dropped_total` |
| **Metrics · health** | Quan sát nền tảng | Prometheus `/metrics` trên cổng nội bộ: độ trễ theo phase, verdict, lỗi upstream, mức đầy ring buffer, version snapshot |

## Hợp đồng của crate `pipeline`

Crate dùng chung với replay worker và công cụ `edge-verify`
([ADR-0026](../../09-adr/0026-replay-on-request-samples-with-shared-pipeline.md)); không phụ thuộc mạng
hay Pingora, để replay dựng request từ mẫu Parquet.

```rust
pub trait Phase: Send + Sync {
    fn name(&self) -> &'static str;
    /// Không được chặn I/O; `budget` là ngân sách thời gian của phase.
    fn eval(&self, req: &Request, state: &mut State, budget: Duration) -> Outcome;
}

pub enum Outcome {
    Continue,
    Allow { rule_id: RuleId },
    Log { rule_id: RuleId },
    Challenge { rule_id: RuleId },
    Block { rule_id: RuleId, score: u16 },
    SkipPhases(PhaseSet),
    Error(PhaseError),          // lỗi hoặc quá hạn → áp fail mode của zone
}

pub trait WafEngine: Send + Sync {
    fn new_transaction(&self, req: &Request) -> Box<dyn WafTransaction + '_>;
}

pub struct Pipeline {
    phases: Vec<Arc<dyn Phase>>, // thứ tự lấy từ ZoneRuntime
    fail_mode: FailMode,         // Open | Closed
}

impl Pipeline {
    pub fn run(&self, req: &Request) -> Verdict { /* ... */ }
}
```

- `Request` là bản đã chuẩn hoá, sở hữu dữ liệu của nó, không giữ kiểu của Pingora.
- Mỗi phase có ngân sách thời gian riêng (cấu hình trong snapshot); quá hạn trả `Error`, pipeline áp
  fail mode và phát event `phase_timeout`.

## Hiệu năng

- Không có GC; cấp phát trên đường request giữ ở mức tối thiểu bằng buffer tái sử dụng theo worker và
  arena cho dữ liệu của một request.
- Ranh giới FFI với libmodsecurity là chỗ tốn kém nhất: truyền header và body theo lô, không gọi FFI
  cho từng trường.
- Benchmark chuẩn (CI): traffic mẫu với CRS paranoia 1 và 2, 50 custom rule; báo p50/p99 theo phase.
  Ngưỡng QS-1 là điều kiện merge cho thay đổi trong crate `pipeline`.

## An toàn

- Toàn bộ code của mình là Rust an toàn; `unsafe` chỉ ở lớp binding libmodsecurity, được fuzz riêng.
- libmodsecurity được pin version; quy trình vá CVE trong 72 giờ qua rollout phần mềm edge thường
  (hoặc fast lane cho cấu hình giảm thiểu, ví dụ tắt transform bị lỗi).

## Lỗi và hành vi

| Tình huống | Hành vi |
|---|---|
| Phase lỗi hoặc quá hạn | Fail mode của zone; event `phase_error` |
| Engine WAF panic hoặc lỗi FFI | Bắt tại ranh giới, coi là lỗi phase, áp fail mode; worker bị lỗi lặp lại thì được khởi động lại |
| Origin không trả lời | 502/504 với trang lỗi có request ID; event upstream |
| Snapshot chưa nạp lúc khởi động | Không mở listener cho tới khi nạp xong (health check đỏ) |
| Tenant vượt quota tài nguyên | Hạ cấp theo fail mode của tenant đó; tenant khác không bị ảnh hưởng |
| Ring buffer telemetry đầy | Bỏ access log, giữ security event tới khi đầy hẳn; bộ đếm lộ ra metrics |
