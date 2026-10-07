use crate::exchange_data::engine::ExecutionEngine;
use crate::exchange_data::signals::orderbooksignal::Side;
use crate::exchange_data::engine::Position;
pub struct PaperEngine{
    qty: f64, //每次开/平的数量（如 0.01 ETH）
    fee_bps: f64,       // 手续费（basis points），先 0
    position: Position, //当前仓位：Flat / Long / Short
    entry_px: f64,  //开仓成交价，算平仓盈亏用
    realized_pnl: f64, //已实现盈亏（平仓后累加；未平仓浮盈不算在内）
    
}

impl PaperEngine {
    pub fn new(qty: f64) -> Self {
        Self {
            qty,
            fee_bps: 0.0,
            position: Position::Flat,
            entry_px: 0.0,
            realized_pnl: 0.0,
        }
    }
}

impl ExecutionEngine for PaperEngine {
    fn on_signal(
        &mut self,
        side: Side,
        best_bid: f64,
        best_ask: f64,
        time: u64,
    ){

        match side {
            Side::Holder => {}
            Side::Buy => {
                // 有空仓先平
                if self.position == Position::Short {
                    self.close_at(best_ask, time);
                }
                // 空仓则开多
                if self.position == Position::Flat {
                    self.open(Position::Long, best_ask, time);
                }
            }
            Side::Sell => {
                if self.position == Position::Long {
                    self.close_at(best_bid, time);
                }
                if self.position == Position::Flat {
                    self.open(Position::Short, best_bid, time);
                }
            }
        }

    }



}

impl PaperEngine {
    fn open(&mut self, pos: Position, px: f64, time: u64) {
        let fee = px * self.qty * self.fee_bps / 10_000.0;
        self.realized_pnl -= fee;
        self.position = pos;
        self.entry_px = px;
        println!("[paper] {time} OPEN {pos:?} qty={} px={px}", self.qty);
    }
    fn close_at(&mut self, px: f64, time: u64) {
        let gross = match self.position {
            Position::Long => (px - self.entry_px) * self.qty,
            Position::Short => (self.entry_px - px) * self.qty,
            Position::Flat => return,
        };
        let fee = px * self.qty * self.fee_bps / 10_000.0;
        let net = gross - fee;
        self.realized_pnl += net;
        println!(
            "[paper] {time} CLOSE {:?} entry={} exit={px} pnl={net:+.4} total={:+.4}",
            self.position, self.entry_px, self.realized_pnl
        );
        self.position = Position::Flat;
        self.entry_px = 0.0;
    }
}