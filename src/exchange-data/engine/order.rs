use crate::exchange_data::types::price::Price;
use crate::exchange_data::types::quantity::Quantity;

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub status: OrderStatus,
    pub side: OrderSide, // Buy/Sell；Holder 不要建单
    pub qty: Quantity,
    pub price: Price, // 市价单：下单时用将要成交的 bid/ask 填上即可
    pub time: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,
    Filled,
    Rejected, // 以后用
}

// 执行层（engine/order）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell, // 只有这两种能进 Order / Fill
}
