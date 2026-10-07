pub mod paperengine;

use crate::exchange_data::signals::orderbooksignal::Side;
pub trait ExecutionEngine: Send {

    fn on_signal(
        &mut self,
        side: Side,
        best_bid: f64,
        best_ask: f64,
        time: u64,
    );
    // 以后可加：on_fill、cancel_all、position() …
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    Flat,
    Long,
    Short,
}
