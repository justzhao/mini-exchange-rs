use crate::exchange_data::engine::ExecutionEngine;
use crate::exchange_data::signals::orderbooksignal::Side;
use crate::exchange_data::engine::Position;
use crate::exchange_data::engine::order::Order;
use crate::exchange_data::engine::order::OrderStatus;
use crate::exchange_data::engine::fill::Fill;


pub struct PaperEngine{
    qty: f64, //每次开/平的数量（如 0.01 ETH）
    fee_bps: f64,       // 手续费（basis points），先 0
    position: Position, //当前仓位：Flat / Long / Short
    entry_px: f64,  //开仓成交价，算平仓盈亏用
    realized_pnl: f64, //已实现盈亏（平仓后累加；未平仓浮盈不算在内）
    next_order_id: u64,
    
}

impl PaperEngine {
    pub fn new(qty: f64) -> Self {
        Self {
            qty,
            fee_bps: 0.0,
            position: Position::Flat,
            entry_px: 0.0,
            realized_pnl: 0.0,
            next_order_id:0
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
            Side::Buy | Side::Sell => {
                // 仍按你现在的仓位规则：需要平仓时先下一笔反向单并成交，再视情况开仓
                // 每一笔都是：create_order → fill_market → order.status=Filled → apply_fill
                self.execute_market(side, best_bid, best_ask, time);
            }
        }

    }

    fn order_qty(&self) -> f64 {
        self.qty
    }

    fn position_qty(&self) -> f64 {
        match self.position {
            Position::Flat => 0.0,
            Position::Long => self.qty,
            Position::Short => -self.qty,
        }
    }


}

impl PaperEngine {
    fn execute_market(&mut self, side: Side, best_bid: f64, best_ask: f64, time: u64) {
        match side {
            Side::Holder => {}
            Side::Buy => {
                // 有空：先买平
                if self.position == Position::Short {
                    self.submit_market(Side::Buy, best_bid, best_ask, time);
                }
                // 空仓：再买开多
                if self.position == Position::Flat {
                    self.submit_market(Side::Buy, best_bid, best_ask, time);
                }
                // 已有多：同向忽略
            }
            Side::Sell => {
                if self.position == Position::Long {
                    self.submit_market(Side::Sell, best_bid, best_ask, time);
                }
                if self.position == Position::Flat {
                    self.submit_market(Side::Sell, best_bid, best_ask, time);
                }
            }
        }

    }

    fn submit_market(&mut self, side: Side, best_bid: f64, best_ask: f64, time: u64) {
        let px = match side {
            Side::Buy => best_ask,
            Side::Sell => best_bid,
            Side::Holder => return,
        };
        let mut order = self.create_order(side, px, time);
        let fill = self.fill_market(&order, best_bid, best_ask, time);
        order.status = OrderStatus::Filled;
        println!(
            "[paper] ORDER {} {:?} qty={} px={} -> FILL fee={}",
            order.id, order.side, order.qty, fill.price, fill.fee
        );
        self.apply_fill(&fill);
    }

    fn create_order(&mut self, side: Side, price: f64, time: u64) -> Order {
        let id = self.next_order_id;
        self.next_order_id += 1;
        Order {
            id,
            status: OrderStatus::New,
            side,
            qty: self.qty,
            price,
            time,
        }
    }
    fn fill_market(&self, order: &Order, best_bid: f64, best_ask: f64, time: u64) -> Fill {
        let price = match order.side {
            Side::Buy => best_ask,
            Side::Sell => best_bid,
            Side::Holder => unreachable!(),
        };
        let fee = price * order.qty * self.fee_bps / 10_000.0;
        Fill {
            order_id: order.id,
            price,
            qty: order.qty,
            fee,
            time,
            side: order.side,
        }
    }

    fn apply_fill(&mut self, fill: &Fill) {
        // 开/平由「当前仓位 + 成交方向」共同决定
        match (self.position, fill.side) {
            (Position::Flat, Side::Buy) => {
                self.open(Position::Long, fill.price, fill.fee, fill.time);
            }
            (Position::Flat, Side::Sell) => {
                self.open(Position::Short, fill.price, fill.fee, fill.time);
            }
            (Position::Short, Side::Buy) => {
                self.close_at(fill.price, fill.fee, fill.time); // 买平空
            }
            (Position::Long, Side::Sell) => {
                self.close_at(fill.price, fill.fee, fill.time); // 卖平多
            }
            (pos, side) => {
                eprintln!("[paper] ignore fill: pos={pos:?} side={side:?}");
            }
        }
    }

    fn open(&mut self, pos: Position, px: f64, fee: f64, time: u64) {
        self.realized_pnl -= fee;
        self.position = pos;
        self.entry_px = px;
        println!("[paper] {time} OPEN {pos:?} qty={} px={px} fee={fee}", self.qty);
    }

    fn close_at(&mut self, px: f64, fee: f64, time: u64) {
        let gross = match self.position {
            Position::Long => (px - self.entry_px) * self.qty,
            Position::Short => (self.entry_px - px) * self.qty,
            Position::Flat => return,
        };
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