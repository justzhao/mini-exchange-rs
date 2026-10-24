use crate::exchange_data::types::{price::Price, quantity::Quantity};
#[path = "exchange-data/mod.rs"]
mod exchange_data;

use exchange_data::apphandler::AppHandler;
use exchange_data::books::bookhandler::BookHandler;
use exchange_data::hyperliquid::Hyperliquid;
use exchange_data::stream::Stream;

use crate::exchange_data::engine::ExecutionEngine;
use crate::exchange_data::engine::paperengine::PaperEngine;
use crate::exchange_data::risk::riskmanger::RiskConfig;
use crate::exchange_data::risk::riskmanger::RiskManager;
use crate::exchange_data::signals::orderbooksignal::SignalSide;
#[tokio::main]
async fn main() {
    println!("exchange start");

    let mut engine = PaperEngine::new(Quantity::from_raw(10_000), 5).expect("模拟引擎配置应合法");

    let bid = "2999.00".parse::<Price>().expect("买价合法");
    let ask = "3000.00".parse::<Price>().expect("卖价合法");
    engine.on_signal(SignalSide::Buy, bid, ask, 1);
    engine.on_signal(SignalSide::Buy, bid, ask, 1);
    engine.close_position("3010.00".parse().unwrap(), "3011.00".parse().unwrap(), 2);

    println!("position     = {:?}", engine.position());
    println!("entry_price  = {:?}", engine.entry_price());
    println!("realized_pnl = {}", engine.realized_pnl());
    println!(
        "unrealized   = {}",
        engine
            .unrealized_pnl("3010.00".parse().unwrap())
            .expect("浮盈计算成功")
    ); // 传入标记价
    println!("orders       = {:?}", engine.orders());
    println!("fills        = {:?}", engine.fills());
    println!(
        "total_fees = {}",
        engine.total_fees().expect("手续费汇总成功")
    );

    // Stream::new(Hyperliquid::mainnet(),  vec![
    //   //  AppHandler::Print(PrintHandler),
    //     AppHandler::Book(BookHandler::new("ETH",engine,RiskManager::new(
    //       RiskConfig {
    //         max_order_size: 0.10,
    //         max_position: 0.10,
    //         max_notional: 300.0,
    //     }
    //     )))
    //     ],
    // )
    //     .subscribe( [("ETH", "l2Book")])
    //     .run()
    //     .await;
}
