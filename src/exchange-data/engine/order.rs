use crate::exchange_data::signals::orderbooksignal::Side;

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub status: OrderStatus,
    pub side: Side,       // Buy/Sell；Holder 不要建单
    pub qty: f64,
    pub price: f64,       // 市价单：下单时用将要成交的 bid/ask 填上即可
    pub time: u64,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,
    Filled,
    Rejected, // 以后用
}
