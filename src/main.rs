#[path = "exchange-data/mod.rs"]
mod exchange_data;

use exchange_data::hyperliquid::Hyperliquid;
use exchange_data::stream::{PrintHandler, Stream};

#[tokio::main]
async fn main() {
    println!("exchange start");

    Stream::new(Hyperliquid::mainnet(),  vec![PrintHandler])
        .subscribe( [("ETH", "l2Book")])
        .run()
        .await;
}
