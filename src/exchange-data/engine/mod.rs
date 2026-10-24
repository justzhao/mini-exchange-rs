use crate::exchange_data::types::price::Price;
use crate::Quantity;
pub mod paperengine;

pub mod order;

pub mod fill;

use crate::exchange_data::signals::orderbooksignal::SignalSide;
pub trait ExecutionEngine: Send {
    fn on_signal(&mut self, side: SignalSide, best_bid: Price, best_ask: Price, time: u64);

    // fn order_qty(&self) -> f64;
    // fn position_qty(&self) -> f64;

    fn order_qty(&self) -> Quantity;
    fn position(&self) -> Position;
    fn position_qty(&self) -> Quantity;


    // 以后可加：on_fill、cancel_all、position() …
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Flat,
    Long,
    Short,
}
