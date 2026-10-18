#[path = "exchange-data/mod.rs"]
mod exchange_data;

use exchange_data::hyperliquid::Hyperliquid;
use exchange_data::stream::{PrintHandler, Stream};
use exchange_data::books::bookhandler::BookHandler;
use exchange_data::apphandler::AppHandler;

use crate::exchange_data::engine::paperengine::PaperEngine;
use crate::exchange_data::risk::riskmanger::RiskManager;
use crate::exchange_data::risk::riskmanger::RiskConfig;
#[tokio::main]
async fn main() {
    println!("exchange start");

    Stream::new(Hyperliquid::mainnet(),  vec![ 
      //  AppHandler::Print(PrintHandler),
        AppHandler::Book(BookHandler::new("ETH",PaperEngine::new(0.01),RiskManager::new(
          RiskConfig {
            max_order_size: 0.10,
            max_position: 0.10,
            max_notional: 300.0,
        }
        )))
        ],
    )
        .subscribe( [("ETH", "l2Book")])
        .run()
        .await;
}
