#[path = "exchange-data/mod.rs"]
mod exchange_data;

use exchange_data::hyperliquid::Hyperliquid;
use exchange_data::stream::{PrintHandler, Stream};
use exchange_data::books::bookhandler::BookHandler;
use exchange_data::apphandler::AppHandler;
#[tokio::main]
async fn main() {
    println!("exchange start");

    Stream::new(Hyperliquid::mainnet(),  vec![ 
      //  AppHandler::Print(PrintHandler),
        AppHandler::Book(BookHandler::new("ETH"))
        ],
    )
        .subscribe( [("ETH", "l2Book")])
        .run()
        .await;
}
