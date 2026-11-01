use crate::exchange_data::signals::strategy::Strategy;
use crate::exchange_data::types::price::Price;
use tokio::task::JoinHandle;
use tokio::sync::mpsc;

use crate::exchange_data::books::localbook::BookSnapshot;
use crate::exchange_data::books::localbook::LocalBook;
use crate::exchange_data::books::localbook::RebuildBookResult;
use crate::exchange_data::engine::ExecutionEngine;
use crate::exchange_data::evals::pendingeval::SignalEvaluator;
use crate::exchange_data::features::orderbookfeature::OrderBookFeature;
use crate::exchange_data::records::marketrecord::MarketRecord;
use crate::exchange_data::records::recorder;
use crate::exchange_data::risk::riskmanger::RiskManager;
use crate::exchange_data::signals::orderbooksignal::OrderBookSignal;
use crate::exchange_data::stream::Handler;
use crate::SignalSide;
use crate::exchange_data::trading::tradingcore::{
    TradingCore, SnapshotResult,
};
use crate::exchange_data::signals::strategy::ImbalanceStrategy;
const RECORD_QUEUE_SIZE: usize = 4096;

pub struct BookHandler<E: ExecutionEngine,S:Strategy> {
    core: TradingCore<E,S>,
    eval: SignalEvaluator,
    record_tx: mpsc::Sender<MarketRecord>,
    record_task: JoinHandle<()>,
    record_dropped: u64,
}

impl<E: ExecutionEngine,S:Strategy> BookHandler<E,S> {
    pub fn new(coin: impl Into<String>, exec: E, risk: RiskManager,strategy: S) -> Self {
        let (record_tx, record_rx) = mpsc::channel(RECORD_QUEUE_SIZE);
        let record_task = recorder::spawn(record_rx);
        Self {
            core: TradingCore::new(coin, exec, risk, strategy),
            eval: SignalEvaluator::new(),
            record_tx,
            record_task,
            record_dropped: 0,
        }
    }

    pub async fn shutdown(self) -> Result<(), tokio::task::JoinError> {
        eprintln!(
            "[recorder] dropped feature records: {}",
            self.record_dropped,
        );
        drop(self.record_tx);
        self.record_task.await
    }
}

pub trait BookEvent {
    type Snap: BookSnapshot + std::fmt::Debug;
    fn as_book(&self) -> Option<&Self::Snap>;
}

//impl<E: BookEvent> Handler<E> for BookHandler {
impl<Ev: BookEvent+Send, Eng: ExecutionEngine,S:Strategy> Handler<Ev> for BookHandler<Eng,S> {
    async fn on_event(&mut self, event: Ev)  {
        let Some(snap) = event.as_book() else {
            return;
        };
    
        let (result, signal) = match self.core.on_snapshot(snap) {
            Ok(outcome) => outcome,
            Err(err) => {
                eprintln!("[book] snapshot failed: {err}");
                return;
            }
        };
    
        if result == SnapshotResult::Ignored {
            return;
        }
    
        // 没有生成信号时，不生成这条特征记录。
        // Holder 也是信号，会包含在 Some 中。
        let Some(signal) = signal else {
            return;
        };
    
        let book = self.core.book();
    
        let Some(feature) = OrderBookFeature::new(book) else {
            return;
        };
    
        let Some(record) =
            MarketRecord::from_book_feat_sig(book, &feature, &signal)
        else {
            return;
        };
    
        match self.record_tx.try_send(record) {
            Ok(()) => {}
    
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.record_dropped += 1;
            }
    
            Err(mpsc::error::TrySendError::Closed(_)) => {
                eprintln!("[recorder] recording task has stopped");
            }
        }
    }

    fn shutdown(
        self,
    ) -> impl Future<Output = Result<(), tokio::task::JoinError>> + Send
    where
        Self: Sized,
    {
        BookHandler::shutdown(self)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange_data::engine::Position;
    use crate::exchange_data::signals::orderbooksignal::SignalSide;
    use crate::exchange_data::risk::riskmanger::RiskConfig;
    use crate::exchange_data::types::{money::Money, price::Price, quantity::Quantity};

    fn price(value: &str) -> Price {
        value.parse().expect("测试价格应合法")
    }

    fn qty(raw: i64) -> Quantity {
        Quantity::from_raw(raw)
    }

    #[derive(Debug)]
    struct FakeSnapshot {
        coin: &'static str,
        time: u64,
        bids: Vec<(Price, Quantity, u32)>,
        asks: Vec<(Price, Quantity, u32)>,
    }
    impl BookSnapshot for FakeSnapshot {
        fn coin(&self) -> &str {
            self.coin
        }
        fn time(&self) -> u64 {
            self.time
        }
        fn bids(&self) -> Result<Vec<(Price, Quantity, u32)>, String> {

            Ok(self.bids.clone())
        }
    
        fn asks(&self) -> Result<Vec<(Price, Quantity, u32)>, String> {
            Ok(self.asks.clone())
        }
    }

    #[test]
    fn rebuild_empty_book() {
        let mut book = LocalBook::new("ETH");
        let snap = FakeSnapshot {
            coin: "ETH",
            time: 100,
            bids: vec![],
            asks: vec![],
        };
        assert_eq!(book.rebuild_book(&snap), RebuildBookResult::Updated);
        assert_eq!(book.time(), 100);
        assert!(book.best_bid().is_none());
        assert!(book.best_ask().is_none());
    }

    #[test]
    fn rebuild_updates_on_newer_snapshot() {
        let mut book = LocalBook::new("ETH");
        let snap = FakeSnapshot {
            coin: "ETH",
            time: 100,
            bids: vec![(price("100.0"), qty(1_000_000), 1)],
            asks: vec![(price("101.0"), qty(2_000_000), 1)],
        };
        assert_eq!(book.rebuild_book(&snap), RebuildBookResult::Updated);
        assert_eq!(book.time(), 100);
        let (bid_px, bid_level) = book.best_bid().unwrap();
        let (ask_px, ask_level) = book.best_ask().unwrap();

        assert_eq!(bid_px, price("100.0"));
        assert_eq!(ask_px, price("101.0"));
        assert_eq!(bid_level.sz, qty(1_000_000));
        assert_eq!(ask_level.sz, qty(2_000_000));
    }
    #[test]
    fn rebuild_rejects_wrong_coin() {
        let mut book = LocalBook::new("ETH");

        let snap1 = FakeSnapshot {
            coin: "ETH",
            time: 100,
            bids: vec![(price("102.0"), qty(2_000_000), 1)],
            asks: vec![(price("103.0"), qty(2_000_000), 1)],
        };
        assert_eq!(book.rebuild_book(&snap1), RebuildBookResult::Updated);
        assert_eq!(book.time(), 100); // 未改

        let snap = FakeSnapshot {
            coin: "BTC",
            time: 104,
            bids: vec![(price("100.0"), qty(1_000_000), 1)],
            asks: vec![(price("101.0"), qty(1_000_000), 1)],
        };
        assert_eq!(book.rebuild_book(&snap), RebuildBookResult::InvalidCoin);
        assert_eq!(book.time(), 100); // 未改
        let (bid_px, bid_level) = book.best_bid().unwrap();
        let (ask_px, ask_level) = book.best_ask().unwrap();

        assert_eq!(bid_px, price("102.0")); // 未改
        assert_eq!(ask_px, price("103.0")); // 未改
        assert_eq!(bid_level.sz, qty(2_000_000));
        assert_eq!(ask_level.sz, qty(2_000_000));
    }
    #[test]
    fn rebuild_rejects_stale_time() {
        let mut book = LocalBook::new("ETH");
        let newer = FakeSnapshot {
            coin: "ETH",
            time: 100,
            bids: vec![(price("100.0"), qty(1_000_000), 1)],
            asks: vec![(price("101.0"), qty(1_000_000), 1)],
        };
        assert_eq!(book.rebuild_book(&newer), RebuildBookResult::Updated);
        let older = FakeSnapshot {
            coin: "ETH",
            time: 50,
            bids: vec![(price("90.0"), qty(1_000_000), 1)],
            asks: vec![(price("91.0"), qty(1_000_000), 1)],
        };
        assert_eq!(book.rebuild_book(&older), RebuildBookResult::StaleTime);
        assert_eq!(book.time(), 100);
        assert_eq!(book.best_bid().unwrap().0, price("100.0")); // 没被旧快照覆盖
    }

    #[test]
    fn rebuild_rejects_same_timestamp() {
        let mut book = LocalBook::new("ETH");
        let first = FakeSnapshot {
            coin: "ETH",
            time: 100,
            bids: vec![(price("100.0"), qty(1_000_000), 1)],
            asks: vec![(price("101.0"), qty(1_000_000), 1)],
        };
        assert_eq!(book.rebuild_book(&first), RebuildBookResult::Updated);

        let same_time = FakeSnapshot {
            coin: "ETH",
            time: 100,                  // 相同
            bids: vec![(price("90.0"), qty(1_000_000), 1)], // 故意不同，证明没被覆盖
            asks: vec![(price("91.0"), qty(1_000_000), 1)],
        };
        assert_eq!(book.rebuild_book(&same_time), RebuildBookResult::StaleTime);
        assert_eq!(book.best_bid().unwrap().0, price("100.0"));
    }

    struct CountingEngine {
        calls: u32,
        qty: Quantity,
    }

    impl ExecutionEngine for CountingEngine {
        fn on_signal(&mut self, _side: SignalSide, _bid: Price, _ask: Price, _time: u64) {
            self.calls += 1;
        }
        fn order_qty(&self) -> Quantity {
            self.qty
        }
        fn position(&self) -> Position {
            Position::Flat
        }
        fn position_qty(&self) -> Quantity {
            Quantity::from_raw(0)
        }
    }

    #[derive(Debug)]
    struct FakeBookEvent(FakeSnapshot);

    impl BookEvent for FakeBookEvent {
        type Snap = FakeSnapshot;
        fn as_book(&self) -> Option<&FakeSnapshot> {
            Some(&self.0)
        }
    }

    #[tokio::test]
    async fn ignored_snapshots_do_not_call_exec() {
        let engine = CountingEngine {
            calls: 0,
            qty: qty(10_000), // 0.01
        };
        let mut handler = BookHandler::new(
            "ETH",
            engine,
            RiskManager::new(RiskConfig {
                max_order_size: qty(1_000_000), // 1
                max_position: qty(1_000_000),   // 1
                max_notional: Money::from_raw(100_000_000_000_000), // 1,000,000
            }),
            ImbalanceStrategy::new(0.3),
        );

        // 1) 合法更新：要有 bid/ask，否则走不到 on_signal
        handler.on_event(FakeBookEvent(FakeSnapshot {
            coin: "ETH",
            time: 100,
            bids: vec![(price("100.0"), qty(10_000_000), 1)], // 买盘更厚，产生 Buy 信号
            asks: vec![(price("101.0"), qty(1_000_000), 1)],
        })).await;
        assert_eq!(handler.core.engine().calls, 1);

        // 2) 错币种 → 不应再调用
        handler.on_event(FakeBookEvent(FakeSnapshot {
            coin: "BTC",
            time: 200,
            bids: vec![(price("100.0"), qty(10_000_000), 1)],
            asks: vec![(price("101.0"), qty(1_000_000), 1)],
        })).await;
        assert_eq!(handler.core.engine().calls, 1);

        // 3) 旧时间 → 仍不应调用
        handler.on_event(FakeBookEvent(FakeSnapshot {
            coin: "ETH",
            time: 50,
            bids: vec![(price("90.0"), qty(10_000_000), 1)],
            asks: vec![(price("91.0"), qty(1_000_000), 1)],
        })).await;
        assert_eq!(handler.core.engine().calls, 1);
    }
}
