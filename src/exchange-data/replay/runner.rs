use crate::exchange_data::books::localbook::{
    LocalBook, RebuildBookResult,
};
use crate::exchange_data::engine::ExecutionEngine;
use crate::exchange_data::engine::paperengine::PaperEngine;
use crate::exchange_data::features::orderbookfeature::OrderBookFeature;
use crate::exchange_data::hyperliquid::L2Book;
use crate::exchange_data::trading::tradingcore::SnapshotResult;
use crate::exchange_data::types::price::Price;
use crate::exchange_data::signals::orderbooksignal::{
    OrderBookSignal, SignalSide,
};
use crate::RiskManager;
use crate::exchange_data::engine::Position;
use crate::exchange_data::types::money::Money;
use crate::exchange_data::trading::tradingcore::TradingCore;
use crate::exchange_data::signals::strategy::ImbalanceStrategy;
use crate::exchange_data::signals::strategy::Strategy;
use super::clock::ReplayClock;
use crate::exchange_data::records::bookrecord::BookRecord;

#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub coin: String,
    pub imbalance_threshold: f64,
    pub initial_capital: Money,
}
#[derive(Debug, PartialEq, Eq)]
pub struct EquityPoint {
    pub time: u64,
    pub equity: Money,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReplayReport {
    pub updated_snapshots: usize,
    pub rejected_signals: usize,
    pub new_orders: usize,
    pub new_fills: usize,
    pub final_position: Position,
    pub final_realized_pnl: Money,
    pub final_total_fees: Money,
    pub final_mark_price: Option<Price>,
    pub final_unrealized_pnl: Option<Money>,   
    pub equity_curve: Vec<EquityPoint>,  
    pub max_drawdown: Money,   
    pub pending_signal_at_end: bool,
    pub liquidity_rejections: usize,
}


fn calculate_equity<S: Strategy>(
    core: &TradingCore<PaperEngine, S>,
    initial_capital: Money,
) -> Result<Money, String> {
    let engine = core.engine();
    let book = core.book();

    let mark_price = match engine.position() {
        Position::Flat => None,
    
        Position::Long => Some(
            book.best_bid()
                .map(|(price, _)| price)
                .ok_or_else(|| "多头持仓缺少买价".to_string())?
        ),
    
        Position::Short => Some(
            book.best_ask()
                .map(|(price, _)| price)
                .ok_or_else(|| "空头持仓缺少卖价".to_string())?
        ),
    };

    let unrealized = if let Some(price) = mark_price {
        engine.unrealized_pnl(price)
            .map_err(|err| format!("浮动盈亏计算失败: {err:?}"))?
    } else {
        Money::from_raw(0)
    };
    

    let raw = initial_capital.raw()
        .checked_add(engine.realized_pnl().raw())
        .and_then(|value| value.checked_add(unrealized.raw()))
        .ok_or_else(|| "权益计算溢出".to_string())?;

    Ok(Money::from_raw(raw))
}

fn calculate_max_drawdown(
    initial_capital: Money,
    curve: &[EquityPoint],
) -> Result<Money, String> {
    let mut peak = initial_capital.raw();
    let mut max_drawdown = 0;

    for point in curve {
        let current = point.equity.raw();

        peak = peak.max(current);

        let drawdown = peak.checked_sub(current)
            .ok_or_else(|| "回撤计算溢出".to_string())?;

        max_drawdown = max_drawdown.max(drawdown);
    }

    Ok(Money::from_raw(max_drawdown))
}

pub fn run_replay<S: Strategy>(
    snapshots: &[L2Book],
    core: &mut TradingCore<PaperEngine, S>,
    initial_capital: Money,
) -> Result<ReplayReport, String> {
    // 人工样本没有接收时间，暂用交易所时间。
    let events = snapshots.iter()
        .map(|snapshot| (snapshot.time, snapshot));

    run_replay_events(events, core, initial_capital)
}

pub fn run_recorded_replay<S: Strategy>(
    records: &[BookRecord],
    core: &mut TradingCore<PaperEngine, S>,
    initial_capital: Money,
) -> Result<ReplayReport, String> {
    let events = records.iter()
        .map(|record| (record.received_at_ms, &record.snapshot));

    run_replay_events(events, core, initial_capital)
}

fn run_replay_events<'a, S: Strategy>(
    events: impl IntoIterator<Item = (u64, &'a L2Book)>,
    core: &mut TradingCore<PaperEngine, S>,
    initial_capital: Money,
) -> Result<ReplayReport, String> {

    let mut equity_curve = Vec::new();

    let mut updated_snapshots = 0;
    let mut rejected_signals = 0;
    let mut liquidity_rejections = 0;
    let initial_orders = core.engine().orders().len();
    let initial_fills = core.engine().fills().len();
    let mut clock = ReplayClock::new();
    for (received_at_ms, snapshot) in events {
        clock.advance_to(received_at_ms);
        
        let (result, _) = core.on_snapshot_at(
            snapshot,
            clock.now_ms(),
        )?;
    
        match result {
            SnapshotResult::Ignored => continue,
    
            SnapshotResult::NoSignal
            | SnapshotResult::SignalPassed
            | SnapshotResult::SignalPending => {
                updated_snapshots += 1;
            }
            SnapshotResult::LiquidityRejected => {
                updated_snapshots += 1;
                liquidity_rejections += 1;
            }
    
            SnapshotResult::RiskRejected => {
                updated_snapshots += 1;
                rejected_signals += 1;
            }
        }
        let equity = calculate_equity(core, initial_capital)?;
        equity_curve.push(EquityPoint {
            time: clock.now_ms(),
            equity,
        });

    }
    let  book =core.book();

    let engine =core.engine();

    let final_total_fees = engine.total_fees()
    .map_err(|err| format!("fee calculation failed: {err:?}"))?;


    let final_mark_price = match engine.position() {
        Position::Flat => None,
        Position::Long => book.best_bid().map(|(px, _)| px),
        Position::Short => book.best_ask().map(|(px, _)| px),
    };

    let final_unrealized_pnl = match engine.position() {
        Position::Flat => Some(Money::from_raw(0)),
        Position::Long | Position::Short => {
            match final_mark_price {
                Some(px) => {
                    let pnl = engine.unrealized_pnl(px)
                        .map_err(|err| {
                            format!("unrealized pnl failed: {err:?}")
                        })?;
    
                    Some(pnl)
                }
                None => None,
            }
        }
    };
    

    let max_drawdown =
    calculate_max_drawdown(initial_capital, &equity_curve)?;

    Ok(ReplayReport {
        updated_snapshots,
        rejected_signals,
        new_orders: engine.orders().len() - initial_orders,
        new_fills: engine.fills().len() - initial_fills,
        final_position: engine.position(),
        final_realized_pnl: engine.realized_pnl(),
        final_total_fees,
        final_mark_price,
        final_unrealized_pnl,
        equity_curve,
        max_drawdown,
        pending_signal_at_end: core.has_pending_signal(),
        liquidity_rejections,
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use crate::exchange_data::engine::Position;
    use crate::exchange_data::engine::order::OrderSide;
    use crate::exchange_data::replay::load_snapshots;
    use crate::exchange_data::risk::riskmanger::RiskConfig;
    use crate::exchange_data::types::{
        money::Money,
        quantity::Quantity,
    };

    #[test]
    fn replay_produces_expected_trades_and_pnl() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/replay_eth.jsonl");

        let snapshots = load_snapshots(&path)
            .expect("样本应读取成功");

        let risk = RiskManager::new(RiskConfig {
            max_order_size: Quantity::from_raw(1_000_000),
            max_position: Quantity::from_raw(1_000_000),
            max_notional: Money::from_raw(100_000_000_000),
        });

        let  engine = PaperEngine::new(
            Quantity::from_raw(10_000),
            5,
        )
        .expect("引擎配置合法");

        let config = ReplayConfig {
            coin: "ETH".to_string(),
            imbalance_threshold: 0.3,
            initial_capital:Money::from_raw(100_000_000_000), // 1000
        };
        let mut core = TradingCore::new(
            config.coin.clone(),
            engine,
            risk.clone(),
            ImbalanceStrategy::new(config.imbalance_threshold),
        );


        let report = run_replay(&snapshots,&mut  core,config.initial_capital)
            .expect("回放应完成");
        let engine = core.engine();
        assert_eq!(report.updated_snapshots, 2);
        assert_eq!(report.rejected_signals, 0);
        assert_eq!(report.new_orders, 3);
        assert_eq!(report.new_fills, 3);
        assert_eq!(report.final_total_fees.raw(), 4_510_500);
        assert_eq!(report.final_realized_pnl.raw(), 4_489_500);
        assert_eq!(
            report.final_mark_price.unwrap().ticks(),
            301_100,
        );
        
        assert_eq!(
            report.final_unrealized_pnl.unwrap().raw(),
            -1_000_000,
        );

        assert_eq!(report.equity_curve.len(), 2);

        assert_eq!(report.equity_curve[0].time, 1000);
        assert_eq!(
            report.equity_curve[0].equity.raw(),
            99_997_499_500,
        );

        assert_eq!(report.equity_curve[1].time, 2000);
        assert_eq!(
            report.equity_curve[1].equity.raw(),
            100_003_489_500,
        );

        println!("{report:#?}");

        assert_eq!(engine.position(), Position::Short);
        assert_eq!(engine.position_qty().raw(), 10_000);

        assert_eq!(
            engine.entry_price().unwrap().ticks(),
            301_000,
        );

        assert_eq!(engine.orders().len(), 3);
        assert_eq!(engine.fills().len(), 3);

        let fills = engine.fills();

        assert_eq!(fills[0].side, OrderSide::Buy);
        assert_eq!(fills[0].price.ticks(), 300_100);
        assert_eq!(fills[0].time, 1000);

        assert_eq!(fills[1].side, OrderSide::Sell);
        assert_eq!(fills[1].price.ticks(), 301_000);
        assert_eq!(fills[1].time, 2000);

        assert_eq!(fills[2].side, OrderSide::Sell);
        assert_eq!(fills[2].price.ticks(), 301_000);
        assert_eq!(fills[2].time, 2000);

        assert_eq!(
            core.engine().total_fees().unwrap().raw(),
            4_510_500,
        );
        assert_eq!(
            core.engine().realized_pnl().raw(),
            4_489_500,
        );

        // 新建引擎，从完全相同的初始状态再运行一次。
        let  second_engine = PaperEngine::new(
            Quantity::from_raw(10_000),
            5,
        )
        .expect("引擎配置合法");

        let mut   second_core = TradingCore::new(
            config.coin.clone(),
            second_engine,
            risk.clone(),
            ImbalanceStrategy::new(config.imbalance_threshold)
            ,
        );

        run_replay(
            &snapshots,
            &mut second_core,config.initial_capital
        )
        .expect("第二次回放应完成");
        let second_engine = second_core.engine();

        assert_eq!(engine.orders(), second_engine.orders());
        assert_eq!(engine.fills(), second_engine.fills());
        assert_eq!(engine.position(), second_engine.position());
        assert_eq!(
            engine.position_qty(),
            second_engine.position_qty(),
        );
        assert_eq!(engine.entry_price(), second_engine.entry_price());
        assert_eq!(engine.realized_pnl(), second_engine.realized_pnl());
        assert_eq!(
            engine.total_fees().unwrap(),
            second_engine.total_fees().unwrap(),
        );
    }
}