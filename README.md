# mini-exchange-rs

用 Rust 写的迷你行情订阅系统。通过 WebSocket 订阅 [Hyperliquid](https://hyperliquid.xyz)，解码后经 `broadcast` 分发给多个 Handler；可维护本地 L2 订单簿。

## 功能

- 订阅 Hyperliquid：`trades` / `l2Book`（按 `(coin, kind)` 配置）
- WebSocket 自动重连（指数退避）
- 应用层 ping / pong 保活
- 事件广播给多个 Handler（慢消费者会 `Lagged` 丢事件）
- 本地订单簿：用 L2 快照整本重建（`LocalBook`）
- 混合 Handler：`AppHandler` 枚举（`Print` / `Book`），避免 `Box<dyn>`

## 环境

- Rust 1.85+（edition 2024）
- 网络可访问 `wss://api.hyperliquid.xyz/ws`

## 运行

```bash
cargo run
```

默认订阅 `ETH` 的 `l2Book`，用 `BookHandler` 重建本地簿并打印快照。

## 使用方式

```rust
use exchange_data::apphandler::AppHandler;
use exchange_data::books::bookhandler::BookHandler;
use exchange_data::hyperliquid::Hyperliquid;
use exchange_data::stream::{PrintHandler, Stream};

Stream::new(
    Hyperliquid::mainnet(),
    vec![
        AppHandler::Print(PrintHandler),
        AppHandler::Book(BookHandler::new("ETH")),
    ],
)
.subscribe([("ETH", "l2Book")])
// .subscribe([("BTC", "trades")])
.run()
.await;
```

- `subscribe` 接受任意 `IntoIterator`，元素可 `Into` 成交易所的 `Sub`（HL 为 `(coin, kind)` → `HlSub`）
- 可链式多次 `subscribe`
- 混合 Handler 用 `AppHandler`；同类型则直接 `Vec<PrintHandler>` 等即可

## 目录结构

```
src/
├── main.rs
└── exchange-data/
    ├── mod.rs
    ├── stream.rs          # Stream / Exchange / Handler / PrintHandler
    ├── hyperliquid.rs     # Hyperliquid WS、Sub/Event、decode
    ├── apphandler.rs      # AppHandler 枚举（混合 Handler）
    └── books/
        ├── mod.rs
        ├── localbook.rs   # LocalBook + BookSnapshot
        └── bookhandler.rs # BookHandler + BookEvent
```

## 架构

```
Hyperliquid WS
    → decode → Event (Trade | L2Book)
    → broadcast::Sender
         ├─ AppHandler::Print  → 打印
         └─ AppHandler::Book   → LocalBook::rebuild_book
```

| 模块 | 职责 |
|------|------|
| `Exchange` | 交易所差异：URL、编码、ping、decode、连接与订阅 |
| `Stream` | 组装订阅与 Handler，broadcast 分发 |
| `Handler` | 消费事件 |
| `BookSnapshot` / `BookEvent` | 把订单簿与具体交易所类型解耦 |
| `LocalBook` | 用 L2 快照清空并重建 bids/asks（`BTreeMap`） |

### 本地簿说明

- HL `l2Book` 是约每区块的快照，不是增量 tick；本地实现为 **整本替换**
- 价格目前用 `OrderedFloat<f64>` 做 map key（便于学习）；生产环境更常见 **整数 tick / 定点**，避免浮点精度问题
- `best_bid` = bids 最高价（`next_back`），`best_ask` = asks 最低价（`next`）

## 依赖

- `tokio` / `tokio-tungstenite`：异步与 WebSocket
- `serde` / `serde_json`：JSON 编解码
- `futures-util`：流拆分
- `ordered-float`：订单簿价格排序
- `tracing` / `tracing-subscriber`：日志（可接）

## 后续可做

- 更多交易所实现同一 `Exchange` trait
- 价格/数量改为 tick 整数或 `Decimal`
- Handler：落库、信号、BBO
- 用 tracing 统一日志
- 配置化订阅与 Handler
