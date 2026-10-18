
use crate::exchange_data::signals::orderbooksignal::Side;

// fill.rs
#[derive(Debug, Clone)]
pub struct Fill {
    pub order_id: u64,
    pub price: f64,
    pub qty: f64,
    pub fee: f64,
    pub time: u64,
    pub side: Side, // 冗余一份，apply 时省查单
}