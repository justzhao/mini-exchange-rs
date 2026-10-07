use serde::Serialize;

use crate::exchange_data::books::localbook::LocalBook;
use crate::exchange_data::features::orderbookfeature::OrderBookFeature;
use crate::exchange_data::signals::orderbooksignal::{OrderBookSignal, Side};

pub type Price = f64;
pub type Quantity = f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    Buy,
    Sell,
    Holder,
}

impl From<Side> for Signal {
    fn from(side: Side) -> Self {
        match side {
            Side::Buy => Self::Buy,
            Side::Sell => Self::Sell,
            Side::Holder => Self::Holder,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketRecord {
    pub timestamp: u64,
    pub symbol: String,
    pub best_bid: Price,
    pub best_ask: Price,
    pub bid_qty: Quantity,
    pub ask_qty: Quantity,
    pub spread: Price,
    pub mid_price: Price,
    pub imbalance: f64,
    pub microprice: Price,
    pub signal: Signal,
}

impl MarketRecord {
    pub fn from_book_feat_sig(
        book: &LocalBook,
        feat: &OrderBookFeature,
        sig: &OrderBookSignal,
    ) -> Option<Self> {
        let (best_bid, bid_lvl) = book.best_bid()?;
        let (best_ask, ask_lvl) = book.best_ask()?;
        Some(Self {
            timestamp: book.time(),
            symbol: book.coin().to_string(),
            best_bid,
            best_ask,
            bid_qty: bid_lvl.sz,
            ask_qty: ask_lvl.sz,
            spread: feat.spread,
            mid_price: feat.mid,
            imbalance: feat.imbalance,
            microprice: feat.microprice,
            signal: Signal::from(sig.side),
        })
    }
}
