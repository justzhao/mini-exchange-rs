use serde::Serialize;

use crate::exchange_data::books::localbook::LocalBook;
use crate::exchange_data::features::orderbookfeature::OrderBookFeature;
use crate::exchange_data::signals::orderbooksignal::{OrderBookSignal, SignalSide};

pub type Price = f64;
pub type Quantity = f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    Buy,
    Sell,
    Holder,
}

impl From<SignalSide> for Signal {
    fn from(side: SignalSide) -> Self {
        match side {
            SignalSide::Buy => Self::Buy,
            SignalSide::Sell => Self::Sell,
            SignalSide::Holder => Self::Holder,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketRecord {
    pub timestamp: u64,
    pub symbol: String,
    pub best_bid: String,
    pub best_ask: String,
    pub bid_qty: String,
    pub ask_qty: String,
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
            best_bid: best_bid.to_string(),
            best_ask: best_ask.to_string(),
            bid_qty: bid_lvl.sz.to_string(),
            ask_qty: ask_lvl.sz.to_string(),
            spread: feat.spread,
            mid_price: feat.mid,
            imbalance: feat.imbalance,
            microprice: feat.microprice,
            signal: Signal::from(sig.side),
        })
    }
}
