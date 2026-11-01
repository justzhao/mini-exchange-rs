use crate::exchange_data::types::money::Money;
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
use core::f64;
use std::path::Path;
use crate::exchange_data::replay::ReplayConfig;
use crate::exchange_data::replay::{
    load_snapshots, run_replay,load_recorded_snapshots
};
use crate::exchange_data::trading::tradingcore::TradingCore;
use crate::exchange_data::signals::strategy::ImbalanceStrategy;
use crate::exchange_data::records::bookrecordhandler::BookRecordHandler;
use crate::exchange_data::replay::{
    load_book_records,
    run_recorded_replay,
};
#[tokio::main]
async fn main() {
    println!("exchange start");

    let mut args = std::env::args_os().skip(1);

    if let Some(mode) = args.next() {

        if mode == "record" {
            let Some(path) = args.next() else {
                eprintln!("用法：cargo run -- record <文件路径>");
                return;
            };
        
            let handler = match BookRecordHandler::new(Path::new(&path)) {
                Ok(handler) => handler,
                Err(err) => {
                    eprintln!("[record] 创建录制文件失败: {err}");
                    return;
                }
            };
        
            eprintln!("[record] 开始录制 ETH 盘口，按 Ctrl+C 停止");
        
            Stream::new(
                Hyperliquid::mainnet(),
                vec![handler],
            )
            .subscribe([("ETH", "l2Book")])
            .run()
            .await;
        
            return;
        }


        if mode != "replay" && mode != "replay-recorded" {
            eprintln!(
                "用法：cargo run -- <replay|replay-recorded> <文件路径> [阈值]"
            );
            return;
        }

        let Some(path) = args.next() else {
            eprintln!("缺少回放文件路径");
            return;
        };
        let threshold = match args.next() {
            Some(value) => {
                let parsed = value.to_str()
                    .unwrap_or("")
                    .parse::<f64>();
        
                match parsed {
                    Ok(value) if value.is_finite()
                        && (0.0..=1.0).contains(&value) => value,
        
                    _ => {
                        eprintln!("阈值必须是 0 到 1 之间的有限数");
                        return;
                    }
                }
            }
            None => 0.3,
        };

        if let Err(err) = replay_file(Path::new(&path),threshold,mode == "replay-recorded",) {
            eprintln!("[replay] failed: {err}");
            std::process::exit(1);
        }

        return;
    }


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

    Stream::new(Hyperliquid::mainnet(),  vec![
      //  AppHandler::Print(PrintHandler),
        AppHandler::Book(
            BookHandler::new("ETH",engine,RiskManager::new(
          RiskConfig {
            max_order_size: Quantity::from_raw( 2),
            max_position: Quantity::from_raw(10),
            max_notional: Money::from_raw(10000),
        }
        ), ImbalanceStrategy::new(0.3)))
        ],
    )
        .subscribe( [("ETH", "l2Book")])
        .run()
        .await;
}



fn replay_file(
  path: &Path,threshold:f64, recorded: bool

) -> Result<(), Box<dyn std::error::Error>> {
    

  let  engine = PaperEngine::new(
      Quantity::from_raw(10_000), // 0.01
      5,                        // 5 bps
  )
  .expect("固定的演示配置应合法")
  .with_slippage(if recorded { 2 } else { 0 });

  let risk = RiskManager::new(RiskConfig {
      max_order_size: Quantity::from_raw(1_000_000),
      max_position: Quantity::from_raw(1_000_000),
      max_notional: Money::from_raw(100_000_000_000),
  });

  let config = ReplayConfig {
    coin: "ETH".to_string(),
    imbalance_threshold: threshold,
    initial_capital: Money::from_raw(100_000_000_000), // 1000
    };
    let mut core = TradingCore::new(
        config.coin.clone(),
        engine,
        risk,
        ImbalanceStrategy::new(config.imbalance_threshold),
    ).with_latency(if recorded { 100 } else { 0 });;
    let report = if recorded {
        let records = load_book_records(path)?;
    
        run_recorded_replay(
            &records,
            &mut core,
            config.initial_capital,
        )
    } else {
        let snapshots = load_snapshots(path)?;
    
        run_replay(
            &snapshots,
            &mut core,
            config.initial_capital,
        )
    }
    .map_err(std::io::Error::other)?;
    println!("{report:#?}");
    
    Ok(())
}