# mini-exchange-rs

用 Rust 写的迷你行情订阅系统。当前通过 WebSocket 订阅 [Hyperliquid](https://hyperliquid.xyz) 的成交数据，解码后广播给多个 Handler 处理。

## 功能

- 订阅 Hyperliquid 指定币种的 `trades`
- 自动重连（指数退避）
- 应用层 ping / pong 保活
- 事件通过 `broadcast` 分发给多个 Handler

## 环境

- Rust 1.85+（edition 2024）
- 网络可访问 `wss://api.hyperliquid.xyz/ws`

## 运行

```bash
cargo run
```

默认订阅 BTC trades，并用 `PrintHandler` 打印解码后的事件。

## 使用方式

```rust
Stream::new(Hyperliquid::mainnet(), vec![PrintHandler])
    .subscribe_trades("BTC")
    .run()
    .await;
```

- 可链式调用多次 `subscribe_trades` 订阅多个币种
- `vec![...]` 里可放多个同类型 Handler

## 目录结构

```
src/
├── main.rs
└── exchange-data/
    ├── mod.rs
    ├── stream.rs       # Stream / Exchange / Handler 抽象
    └── hyperliquid.rs  # Hyperliquid WebSocket 实现
```

## 架构简述

```
Hyperliquid WS
    → decode → broadcast::Sender<Event>
                    ├─ Handler task 1
                    ├─ Handler task 2
                    └─ ...
```

- `Exchange`：交易所差异（地址、编码、解码、连接与订阅）
- `Stream`：组装订阅列表与 Handler，负责事件分发
- `Handler`：消费事件（打印、入库、算信号等）

## 后续可做

- 接入更多交易所
- 更丰富的 Handler（落库、信号）
- 统一日志（tracing）
- 配置化订阅币种与 Handler
