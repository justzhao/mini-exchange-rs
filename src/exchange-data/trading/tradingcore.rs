use crate::exchange_data::books::localbook::LocalBook;
use crate::exchange_data::engine::ExecutionEngine;
use crate::exchange_data::risk::riskmanger::RiskManager;
use crate::exchange_data::books::localbook::{
    BookSnapshot, RebuildBookResult,
};
use crate::exchange_data::features::orderbookfeature::OrderBookFeature;
use crate::exchange_data::signals::orderbooksignal::{
    OrderBookSignal, SignalSide,
};
use crate::exchange_data::signals::strategy::Strategy;
use crate::exchange_data::engine::Position;

#[derive(Debug, PartialEq, Eq)]
pub enum SnapshotResult {
    Ignored,
    NoSignal,
    SignalPassed,
    RiskRejected,
    SignalPending,
    LiquidityRejected,
    
}

#[derive(Debug, Clone, Copy)]
struct PendingSignal {
    signal: OrderBookSignal,
    due_at_ms: u64,
}

pub struct TradingCore<E: ExecutionEngine, S: Strategy> {
    book: LocalBook,
    engine: E,
    strategy: S,
    risk: RiskManager,
    latency_ms: u64,
    pending_signal: Option<PendingSignal>,  
}

impl<E: ExecutionEngine,S: Strategy> TradingCore<E,S> {

    fn required_quantity_raw(&self, side: SignalSide) -> i128 {
        let order_qty = i128::from(self.engine.order_qty().raw());
        let position_qty =
            i128::from(self.engine.position_qty().raw());
    
        match (self.engine.position(), side) {
            (_, SignalSide::Holder)
            | (Position::Long, SignalSide::Buy)
            | (Position::Short, SignalSide::Sell) => 0,
    
            (Position::Flat, _) => order_qty,
    
            // 反向操作：平掉旧仓，再开新仓。
            (Position::Long, SignalSide::Sell)
            | (Position::Short, SignalSide::Buy) => {
                position_qty + order_qty
            }
        }
    }

    pub fn new(
        coin: impl Into<String>,
        engine: E,
        risk: RiskManager,
        strategy:S
    ) -> Self {
        Self {
            book: LocalBook::new(coin),
            engine,
            strategy,
            risk,
            latency_ms: 0,
            pending_signal: None,
        }
    }
    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub fn on_snapshot_at(
        &mut self,
        snapshot: &impl BookSnapshot,
        now_ms: u64,
    ) -> Result<(SnapshotResult, Option<OrderBookSignal>), String> {
        match self.book.rebuild_book(snapshot) {
            RebuildBookResult::Updated => {}
    
            RebuildBookResult::InvalidCoin
            | RebuildBookResult::StaleTime => {
                return Ok((SnapshotResult::Ignored,None));
            }
    
            RebuildBookResult::InvalidSnapshot => {
                return Err(format!(
                    "invalid snapshot: coin={}, time={}",
                    snapshot.coin(),
                    snapshot.time(),
                ));
            }
        }
    
        let Some(feature) = OrderBookFeature::new(&self.book) else {
            return Ok((SnapshotResult::NoSignal,None));
        };
    

        let current_signal = self.strategy.on_feature(&feature);

        let (Some((bid, bid_level)), Some((ask, ask_level))) =
                 (self.book.best_bid(), self.book.best_ask())
        else {
            return Ok((SnapshotResult::NoSignal, Some(current_signal)));
        };
       
        let bid_quantity = i128::from(bid_level.sz.raw());
        let ask_quantity = i128::from(ask_level.sz.raw());

        // 优先处理之前已经提交的信号。
        let (signal, from_pending) = match self.pending_signal {
            Some(pending) => {
                if now_ms < pending.due_at_ms {
                    return Ok((
                        SnapshotResult::SignalPending,
                        Some(current_signal),
                    ));
                }

                self.pending_signal = None;
                (pending.signal, true)
            }

            None => (current_signal, false),
        };

            let px = match signal.side {
                SignalSide::Buy => ask,
                SignalSide::Sell => bid,
                SignalSide::Holder => {
                    return Ok((SnapshotResult::NoSignal, Some(signal)));
                }
            };

            // 新提交时检查一次；等待到期后，按最新价格和仓位再检查。
            if let Err(err) = self.risk.check(
                signal.side,
                self.engine.order_qty(),
                self.engine.position(),
                self.engine.position_qty(),
                px,
            ) {
                eprintln!("[risk] rejected: {err:?}");
                return Ok((SnapshotResult::RiskRejected, Some(signal)));
            }

            // 新信号且配置了延迟：先保存，不立即执行。
            if !from_pending && self.latency_ms > 0 {
                let due_at_ms = now_ms
                    .checked_add(self.latency_ms)
                    .ok_or_else(|| "信号到期时间溢出".to_string())?;

                self.pending_signal = Some(PendingSignal {
                    signal,
                    due_at_ms,
                });

                return Ok((SnapshotResult::SignalPending, Some(signal)));
            }

            let required = self.required_quantity_raw(signal.side);

            let available = match signal.side {
                SignalSide::Buy => ask_quantity,
                SignalSide::Sell => bid_quantity,
                SignalSide::Holder => 0,
            };

            if required > available {
                eprintln!(
                    "[execution] liquidity rejected: required_raw={}, available_raw={}",
                    required,
                    available,
                );
            
                return Ok((
                    SnapshotResult::LiquidityRejected,
                    Some(signal),
                ));
            }

            


            // 零延迟信号，或者已经到期的等待信号。
            self.engine.on_signal(
                signal.side,
                bid,
                ask,
                now_ms,
            );

            Ok((SnapshotResult::SignalPassed, Some(signal)))
    }

    pub fn on_snapshot(
        &mut self,
        snapshot: &impl BookSnapshot,
    ) -> Result<(SnapshotResult, Option<OrderBookSignal>), String> {
        self.on_snapshot_at(snapshot, snapshot.time())
    }

    pub fn engine(&self) -> &E {
        &self.engine
    }
    
    pub fn book(&self) -> &LocalBook {
        &self.book
    }
    pub fn has_pending_signal(&self) -> bool {
        self.pending_signal.is_some()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange_data::engine::paperengine::PaperEngine;
    use crate::exchange_data::engine::Position;
    use crate::exchange_data::hyperliquid::L2Book;
    use crate::exchange_data::risk::riskmanger::RiskConfig;
    use crate::exchange_data::signals::strategy::ImbalanceStrategy;
    use crate::exchange_data::types::{
        money::Money,
        quantity::Quantity,
    };

    fn snapshot(time: u64, bid: &str, ask: &str) -> L2Book {
        serde_json::from_value(serde_json::json!({
            "coin": "ETH",
            "time": time,
            "levels": [
                [{"px": bid, "sz": "10", "n": 1}],
                [{"px": ask, "sz": "1", "n": 1}]
            ]
        }))
        .expect("测试盘口应合法")
    }

    #[test]
    fn delayed_buy_uses_arrival_quote() {
        let engine = PaperEngine::new(
            Quantity::from_raw(10_000),
            5,
        )
        .expect("引擎配置应合法");

        let risk = RiskManager::new(RiskConfig {
            max_order_size: Quantity::from_raw(1_000_000),
            max_position: Quantity::from_raw(1_000_000),
            max_notional: Money::from_raw(100_000_000_000),
        });

        let mut core = TradingCore::new(
            "ETH",
            engine,
            risk,
            ImbalanceStrategy::new(0.3),
        )
        .with_latency(100);

        // 1000ms：出现 Buy，但尚未执行。
        let first = snapshot(1000, "2999.00", "3000.00");
        let (result, _) = core.on_snapshot_at(&first, 1000).unwrap();

        assert_eq!(result, SnapshotResult::SignalPending);
        assert!(core.engine().orders().is_empty());
        assert_eq!(core.engine().position(), Position::Flat);

        // 1050ms：还没等够 100ms。
        let second = snapshot(1050, "3000.00", "3001.00");
        let (result, _) = core.on_snapshot_at(&second, 1050).unwrap();

        assert_eq!(result, SnapshotResult::SignalPending);
        assert!(core.engine().fills().is_empty());

        // 1120ms：第一条到期后的有效盘口，按新 ask 成交。
        let third = snapshot(1120, "3001.00", "3002.00");
        let (result, _) = core.on_snapshot_at(&third, 1120).unwrap();

        assert_eq!(result, SnapshotResult::SignalPassed);
        assert_eq!(core.engine().position(), Position::Long);
        assert_eq!(core.engine().orders().len(), 1);
        assert_eq!(core.engine().fills().len(), 1);

        let fill = &core.engine().fills()[0];
        assert_eq!(fill.price.ticks(), 300_200);
        assert_eq!(fill.time, 1120);
    }


    #[test]
    fn insufficient_ask_rejects_buy() {
        let mut core = test_core();

        let mut snap = snapshot(1000, "2999.00", "3000.00");
        snap.levels[1][0].sz = "0.005".into();

        let (result, _) = core.on_snapshot_at(&snap, 1000).unwrap();

        assert_eq!(result, SnapshotResult::LiquidityRejected);
        assert!(core.engine().orders().is_empty());
        assert!(core.engine().fills().is_empty());
        assert_eq!(core.engine().position(), Position::Flat);
        assert_eq!(core.engine().realized_pnl().raw(), 0);
    }


    #[test]
fn insufficient_bid_rejects_entire_reversal() {
    let mut core = test_core();

    // 先正常开多。
    let first = snapshot(1000, "2999.00", "3000.00");
    core.on_snapshot_at(&first, 1000).unwrap();

    assert_eq!(core.engine().position(), Position::Long);

    let pnl_before = core.engine().realized_pnl();
    let entry_before = core.engine().entry_price();

    // 买量远小于卖量，产生 Sell 信号。
    let mut second = snapshot(2000, "3010.00", "3011.00");
    second.levels[0][0].sz = "0.015".into();
    second.levels[1][0].sz = "1".into();

    let (result, _) = core.on_snapshot_at(&second, 2000).unwrap();

    assert_eq!(result, SnapshotResult::LiquidityRejected);

    // 整次反向被拒绝，不能先平掉原来的多仓。
    assert_eq!(core.engine().orders().len(), 1);
    assert_eq!(core.engine().fills().len(), 1);
    assert_eq!(core.engine().position(), Position::Long);
    assert_eq!(core.engine().position_qty().raw(), 10_000);
    assert_eq!(core.engine().entry_price(), entry_before);
    assert_eq!(core.engine().realized_pnl(), pnl_before);
}


    fn test_core() -> TradingCore<PaperEngine, ImbalanceStrategy> {
        let engine = PaperEngine::new(
            Quantity::from_raw(10_000), // 0.01 ETH
            5,
        )
        .unwrap();
    
        let risk = RiskManager::new(RiskConfig {
            max_order_size: Quantity::from_raw(1_000_000),
            max_position: Quantity::from_raw(1_000_000),
            max_notional: Money::from_raw(100_000_000_000),
        });
    
        TradingCore::new(
            "ETH",
            engine,
            risk,
            ImbalanceStrategy::new(0.3),
        )
    }


    #[test]
fn pending_signal_waits_for_newer_snapshot() {
    let mut core = test_core().with_latency(100);

    let first = snapshot(1000, "2999.00", "3000.00");
    core.on_snapshot_at(&first, 1000).unwrap();

    assert!(core.has_pending_signal());
    assert!(core.engine().fills().is_empty());

    // 本地时间已经到期，但交易所时间相同：忽略此快照。
    let same_time = snapshot(1000, "3000.00", "3001.00");
    let (result, _) =
        core.on_snapshot_at(&same_time, 1120).unwrap();

    assert_eq!(result, SnapshotResult::Ignored);
    assert!(core.has_pending_signal());
    assert!(core.engine().fills().is_empty());

    // 更新的快照到达，使用这条盘口执行。
    let newer = snapshot(1001, "3001.00", "3002.00");
    let (result, _) =
        core.on_snapshot_at(&newer, 1121).unwrap();

    assert_eq!(result, SnapshotResult::SignalPassed);
    assert!(!core.has_pending_signal());
    assert_eq!(core.engine().fills().len(), 1);
    assert_eq!(core.engine().fills()[0].price, "3002".parse().unwrap());
    assert_eq!(core.engine().fills()[0].time, 1121);
}
}