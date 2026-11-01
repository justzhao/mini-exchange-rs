use crate::exchange_data::hyperliquid::L2Book;



#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BookRecord {
    pub version: u32,
    pub sequence: u64,
    pub received_at_ms: u64,
    pub snapshot: L2Book,
}

