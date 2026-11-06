# 5.3 Edge plane

Nhiều PoP, nhiều region, chạy trên PoP riêng, hạ tầng đối tác hoặc trong hạ tầng của khách
([ADR-0005](../09-adr/0005-edge-on-own-and-existing-infra.md)). Mỗi PoP có **ba lớp** với tài nguyên
khác nhau ([ADR-0033](../09-adr/0033-two-tier-edge-proxy-and-waf.md)), cộng DNS điều hướng:

| Lớp | Làm gì | Tài nguyên chính | Chi tiết |
|---|---|---|---|
| L3/L4 | Hấp thụ flood mạng, anycast | Băng thông | Hạ tầng / đối tác; PoP riêng: XDP (Mở rộng) |
| **Edge proxy** | TLS, kết nối, pre-filter, challenge, rate limit, cache, proxy tới origin | **RAM, kết nối, băng thông** | [5.8.1](08-components/01-edge-proxy.md) |
| **WAF inspection** | CRS, custom rule cần body, API schema; trả verdict | **CPU** | [5.8.2](08-components/02-waf-inspection.md) |
| DNS điều hướng | Đưa người dùng tới PoP gần, khoẻ | Nhẹ | [5.8.3](08-components/03-dns-and-traffic-steering.md) |

## Bên trong một PoP

```mermaid
flowchart LR
    ctrl["Control · Distribution"]
    peers["PoP khác · Valkey region"]
    ana["Analytics · Ingest gateway"]
    dns["DNS điều hướng"]
    req["Request"]
    origin["Origin / origin shield"]

    subgraph POP["PoP"]
        subgraph PX["Pool edge proxy (RAM)"]
            agp["Policy agent"]
            px["Edge proxy<br/>pre-filter · challenge · cache"]
            sync["State sync"]
        end
        subgraph WF["Pool WAF inspection (CPU)"]
            agw["Policy agent"]
            waf["WAF inspection<br/>CRS · rule inspect"]
        end
        ship["Vector<br/>buffer đĩa"]
    end

    dns -.->|IP của PoP| req
    req --> px
    px <-->|Inspection API| waf
    px --> origin
    ctrl --> agp
    ctrl --> agw
    agp -.->|ack, health| ctrl
    agw -.->|ack, health| ctrl
    px <--> sync <--> peers
    px --> ship --> ana
    waf --> ship
```

| Thành phần | Trách nhiệm | Khi mất kết nối |
|---|---|---|
| **Policy agent** (ở cả hai pool) | Kéo snapshot, kiểm chữ ký, nạp phần của tầng mình, swap nguyên khối, giữ last-good, báo ack | Chạy tiếp bản đang có |
| **Edge proxy** | Pipeline bước 2–10, 12–14 ([5.8.1](08-components/01-edge-proxy.md#thứ-tự-xử-lý)) | Không phụ thuộc mạng ngoài PoP |
| **WAF inspection** | Bước 11: managed ruleset, custom rule phần inspect, schema body API, GraphQL | Không phụ thuộc mạng ngoài PoP |
| **State sync** | Counter rate limit và DDoS L7 ([ADR-0011](../09-adr/0011-approximate-fleet-rate-limiting.md)) | Rơi về đếm cục bộ |
| **Vector** | Gom event và access log của cả hai tầng, gửi ingest gateway | Buffer đĩa |

## Pipeline request

| # | Bước | Tầng | Ghi chú |
|---|---|---|---|
| 1 | L3/L4 mitigation | Hạ tầng | Volumetric |
| 2 | TLS | Proxy | Cert theo SNI; ghi fingerprint JA4, HTTP/2 |
| 3 | Giới hạn giao thức, chuẩn hoá | Proxy | Chặn request sai chuẩn, smuggling |
| 4 | Access list, reputation | Proxy | `allow` có phạm vi, có thể bỏ qua cả WAF |
| 5 | Bot scoring | Proxy | Chỉ tính điểm và lớp ([8.16](../08-cross-cutting/16-bot-management.md)) |
| 6 | Under Attack mode, challenge | Proxy | Mọi challenge đều ở proxy ([ADR-0034](../09-adr/0034-pre-filter-and-rule-split-across-tiers.md)); bot đã xác minh bỏ qua theo cấu hình |
| 7 | API: endpoint, xác thực, consumer, schema rẻ | Proxy | [8.17](../08-cross-cutting/17-api-security.md) |
| 8 | Rate limit, DDoS L7 | Proxy | Khoá theo IP, phiên hoặc consumer |
| 9 | Policy bot, custom rule phần pre | Proxy | Rule trước rule đầu tiên cần body |
| 10 | Inspect gate | Proxy | Gọi hay không gọi WAF |
| 11 | Managed ruleset, custom rule phần inspect, schema body, GraphQL | **WAF** | Chỉ request còn lại sau bước 4–10 |
| 12 | Quyết định | Proxy | allow / log / challenge / block |
| 13 | Cache | Proxy | Mặc định sau WAF; tài nguyên tĩnh có thể miễn WAF |
| 14 | Proxy tới origin | Proxy | Qua origin shield ở Mở rộng |

Mỗi bước có ngân sách thời gian và hành vi khi lỗi theo zone (fail-open hoặc fail-closed); tầng WAF
không trả lời kịp cũng áp fail mode này.

## Cô lập tenant

- Ở proxy: giới hạn kết nối, RAM cho body buffer, băng thông log theo tenant.
- Ở WAF: phần CPU theo gói (fair share), kích thước body được kiểm tra, độ phức tạp regex (chặn ở compiler).
- Tenant vượt giới hạn bị hạ cấp theo fail mode của chính nó, không làm chậm tenant khác.
