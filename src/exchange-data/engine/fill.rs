use crate::exchange_data::engine::order::OrderSide;
use crate::exchange_data::types::money::Money;
use crate::exchange_data::types::price::Price;
use crate::exchange_data::types::quantity::Quantity;
// fill.rs
#[derive(Debug, Clone)]
pub struct Fill {
    pub order_id: u64,
    pub price: Price,
    pub qty: Quantity,
    pub fee: Money,
    pub time: u64,
    pub side: OrderSide, // 冗余一份，apply 时省查单
}
