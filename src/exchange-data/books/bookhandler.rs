use crate::exchange_data::types::price::Price;

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

const RECORD_QUEUE_SIZE: usize = 4096;

pub struct BookHandler<E: ExecutionEngine> {
    book: LocalBook,
    eval: SignalEvaluator,
    record_tx: mpsc::Sender<MarketRecord>,
    exec: E,
    risk: RiskManager,
}

impl<E: ExecutionEngine> BookHandler<E> {
    pub fn new(coin: impl Into<String>, exec: E, risk: RiskManager) -> Self {
        let (record_tx, record_rx) = mpsc::channel(RECORD_QUEUE_SIZE);
        recorder::spawn(record_rx);
        Self {
            book: LocalBook::new(coin),
            eval: SignalEvaluator::new(),
            record_tx,
            exec,
            risk,
        }
    }
}

pub trait BookEvent {
    type Snap: BookSnapshot + std::fmt::Debug;
    fn as_book(&self) -> Option<&Self::Snap>;
}

//impl<E: BookEvent> Handler<E> for BookHandler {
impl<Ev: BookEvent, Eng: ExecutionEngine> Handler<Ev> for BookHandler<Eng> {
    fn on_event(&mut self, event: Ev) {
        if let Some(snap) = event.as_book() {
            match self.book.rebuild_book(snap) {
                RebuildBookResult::Updated => {}
                RebuildBookResult::InvalidCoin 
                | RebuildBookResult::StaleTime 
                | RebuildBookResult::InvalidSnapshot=> {
                    return;
                }
            }

            let Some(feat) = OrderBookFeature::new(&self.book) else {
                return;
            };
            let sig = OrderBookSignal::from_feature(&feat, 0.3);

            //  self.eval.push(self.book.time(), feat.mid, sig.side);
            // self.eval.on_mid(self.book.time(), feat.mid);

            if let Some(rec) = MarketRecord::from_book_feat_sig(&self.book, &feat, &sig) {
                if let Err(err) = self.record_tx.try_send(rec) {
                    eprintln!("[警告] record queue full or closed: {err}");
                }
            }

            if let (Some((bid, _)), Some((ask, _))) = (self.book.best_bid(), self.book.best_ask()) {

              let px = match sig.side {
                    SignalSide::Buy => ask,
                    SignalSide::Sell => bid,
                    SignalSide::Holder => return,
                 };    
            
                match self.risk.check(
                    sig.side,
                    self.exec.order_qty(),
                    self.exec.position(),  
                    self.exec.position_qty(),
                    px,
                ) {
                    Ok(()) => {
                        // Legacy book uses f64. Reject unsupported precision rather than round prices.
                        // Exact raw-feed parsing belongs to the later order-book migration.
                        match (
                            bid.to_string().parse::<Price>(),
                            ask.to_string().parse::<Price>(),
                        ) {
                            (Ok(bid), Ok(ask)) => {
                                self.exec.on_signal(sig.side, bid, ask, self.book.time())
                            }
                            _ => eprintln!(
                                "[paper] quote unsupported by two-decimal Price: {bid}/{ask}"
                            ),
                        }
                    }
                    Err(e) => eprintln!("[risk] reject {:?}", e),
                }
            }

            // if let (Some((bid, _)), Some((ask, _))) = (self.book.best_bid(), self.book.best_ask()) {
            //     self.exec.on_signal(sig.side, bid, ask, self.book.time());
            // }
        }
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
        );

        // 1) 合法更新：要有 bid/ask，否则走不到 on_signal
        handler.on_event(FakeBookEvent(FakeSnapshot {
            coin: "ETH",
            time: 100,
            bids: vec![(price("100.0"), qty(10_000_000), 1)], // 买盘更厚，产生 Buy 信号
            asks: vec![(price("101.0"), qty(1_000_000), 1)],
        }));
        assert_eq!(handler.exec.calls, 1);

        // 2) 错币种 → 不应再调用
        handler.on_event(FakeBookEvent(FakeSnapshot {
            coin: "BTC",
            time: 200,
            bids: vec![(price("100.0"), qty(10_000_000), 1)],
            asks: vec![(price("101.0"), qty(1_000_000), 1)],
        }));
        assert_eq!(handler.exec.calls, 1);

        // 3) 旧时间 → 仍不应调用
        handler.on_event(FakeBookEvent(FakeSnapshot {
            coin: "ETH",
            time: 50,
            bids: vec![(price("90.0"), qty(10_000_000), 1)],
            asks: vec![(price("91.0"), qty(1_000_000), 1)],
        }));
        assert_eq!(handler.exec.calls, 1);
    }
}
