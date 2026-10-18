pub mod paperengine;

pub mod order;

pub mod fill;

use crate::exchange_data::signals::orderbooksignal::Side;
pub trait ExecutionEngine: Send {

    fn on_signal(
        &mut self,
        side: Side,
        best_bid: f64,
        best_ask: f64,
        time: u64,
    );

    fn order_qty(&self) -> f64;
    fn position_qty(&self) -> f64;
    // 以后可加：on_fill、cancel_all、position() …
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Flat,
    Long,
    Short,
}
