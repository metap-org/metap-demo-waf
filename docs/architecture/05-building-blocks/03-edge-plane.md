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
| **Edge proxy** | Pipeline bước 2–9, 11–13 ([5.8.1](08-components/01-edge-proxy.md#thứ-tự-xử-lý)) | Không phụ thuộc mạng ngoài PoP |
| **WAF inspection** | Bước 10: managed ruleset, custom rule phần inspect, API schema | Không phụ thuộc mạng ngoài PoP |
| **State sync** | Counter rate limit và DDoS L7 ([ADR-0011](../09-adr/0011-approximate-fleet-rate-limiting.md)) | Rơi về đếm cục bộ |
| **Vector** | Gom event và access log của cả hai tầng, gửi ingest gateway | Buffer đĩa |

## Pipeline request

| # | Bước | Tầng | Ghi chú |
|---|---|---|---|
| 1 | L3/L4 mitigation | Hạ tầng | Volumetric |
| 2 | TLS | Proxy | Cert theo SNI |
| 3 | Giới hạn giao thức, chuẩn hoá | Proxy | Chặn request sai chuẩn, smuggling |
| 4 | Access list, reputation | Proxy | `allow` có phạm vi, có thể bỏ qua cả WAF |
| 5 | Under Attack mode, challenge | Proxy | Mọi challenge đều ở proxy ([ADR-0034](../09-adr/0034-pre-filter-and-rule-split-across-tiers.md)) |
| 6 | Rate limit, DDoS L7 | Proxy | |
| 7 | Bot (tín hiệu rẻ) | Proxy | Mở rộng |
| 8 | Custom rule phần pre | Proxy | Rule trước rule đầu tiên cần body |
| 9 | Inspect gate | Proxy | Gọi hay không gọi WAF |
| 10 | Managed ruleset, custom rule phần inspect, API schema | **WAF** | Chỉ request còn lại sau bước 4–9 |
| 11 | Quyết định | Proxy | allow / log / challenge / block |
| 12 | Cache | Proxy | Mặc định sau WAF; tài nguyên tĩnh có thể miễn WAF |
| 13 | Proxy tới origin | Proxy | Qua origin shield ở Mở rộng |

Mỗi bước có ngân sách thời gian và hành vi khi lỗi theo zone (fail-open hoặc fail-closed); tầng WAF
không trả lời kịp cũng áp fail mode này.

## Cô lập tenant

- Ở proxy: giới hạn kết nối, RAM cho body buffer, băng thông log theo tenant.
- Ở WAF: phần CPU theo gói (fair share), kích thước body được kiểm tra, độ phức tạp regex (chặn ở compiler).
- Tenant vượt giới hạn bị hạ cấp theo fail mode của chính nó, không làm chậm tenant khác.
