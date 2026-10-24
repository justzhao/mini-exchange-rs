use crate::exchange_data::engine::fill::Fill;
use crate::exchange_data::engine::order::{Order, OrderSide, OrderStatus};
use crate::exchange_data::engine::{ExecutionEngine, Position};
use crate::exchange_data::signals::orderbooksignal::SignalSide;
use crate::exchange_data::types::{money::Money, price::Price, quantity::Quantity};

pub struct PaperEngine {
    qty: Quantity,
    fee_bps: u32,
    position: Position,
    position_qty: Quantity,
    entry_px: Option<Price>,
    realized_pnl: Money,
    next_order_id: u64,
    fills: Vec<Fill>,
    orders: Vec<Order>,
}

#[derive(Debug)]
pub enum PaperEngineConfigError {
    InvalidQuantity,
}
#[derive(Debug)]
pub enum QuoteError {
    NoCross,
}
#[derive(Debug)]
pub enum CalculationError {
    Overflow,
}

fn money_from_raw(raw: i128) -> Result<Money, CalculationError> {
    i64::try_from(raw)
        .map(Money::from_raw)
        .map_err(|_| CalculationError::Overflow)
}

// Price has 2 decimals, Quantity has 6, so their product has Money's 8 decimals.
fn calculate_fee(price: Price, qty: Quantity, fee_bps: u32) -> Result<Money, CalculationError> {
    let amount = i128::from(price.ticks()) * i128::from(qty.raw());
    let numerator = amount
        .checked_mul(i128::from(fee_bps))
        .ok_or(CalculationError::Overflow)?;
    // Nonnegative fees are rounded down to the smallest Money unit.
    money_from_raw(numerator / 10_000)
}

impl SignalSide {
    pub fn to_order_side(self) -> Option<OrderSide> {
        match self {
            Self::Buy => Some(OrderSide::Buy),
            Self::Sell => Some(OrderSide::Sell),
            Self::Holder => None,
        }
    }
}

impl ExecutionEngine for PaperEngine {
    fn on_signal(&mut self, side: SignalSide, best_bid: Price, best_ask: Price, time: u64) {
        let Some(side) = side.to_order_side() else {
            return;
        };
        if let Err(err) = self.execute_market(side, best_bid, best_ask, time) {
            eprintln!("[paper] execution failed: {err:?}");
        }
    }

    // Compatibility with the existing floating-point risk interface only.
    fn order_qty(&self) -> Quantity {
        self.qty
    }
    fn position(&self) -> Position {
        self.position
    }
    fn position_qty(&self) -> Quantity {

        self.position_qty
        // let qty = self.position_qty.raw() as f64 / 1_000_000.0;
        // match self.position {
        //     Position::Flat => 0.0,
        //     Position::Long => qty,
        //     Position::Short => -qty,
        // }
    }
}

impl PaperEngine {
    pub fn new(qty: Quantity, fee_bps: u32) -> Result<Self, PaperEngineConfigError> {
        if qty.raw() <= 0 {
            return Err(PaperEngineConfigError::InvalidQuantity);
        }
        Ok(Self {
            qty,
            fee_bps,
            position: Position::Flat,
            position_qty: Quantity::from_raw(0),
            entry_px: None,
            realized_pnl: Money::from_raw(0),
            next_order_id: 0,
            fills: Vec::new(),
            orders: Vec::new(),
        })
    }

    fn execute_market(
        &mut self,
        side: OrderSide,
        bid: Price,
        ask: Price,
        time: u64,
    ) -> Result<(), CalculationError> {
        if let Err(err) = Self::validate_quote(bid, ask) {
            eprintln!("[paper] invalid quote: {err:?}");
            return Ok(());
        }
        match (self.position, side) {
            // 空仓买入：开多
            (Position::Flat, OrderSide::Buy) => {
                self.open_position(OrderSide::Buy, bid, ask, time)?;
            }

            // 空仓卖出：开空
            (Position::Flat, OrderSide::Sell) => {
                self.open_position(OrderSide::Sell, bid, ask, time)?;
            }

            // 已有多仓，再买：忽略
            (Position::Long, OrderSide::Buy) => {}

            // 已有空仓，再卖：忽略
            (Position::Short, OrderSide::Sell) => {}

            // 多仓卖出：先平多，再开空
            (Position::Long, OrderSide::Sell) => {
                self.close_current_position(bid, ask, time)?;
                self.open_position(side, bid, ask, time)?;
        
            }

            // 空仓买入：先平空，再开多
            (Position::Short, OrderSide::Buy) => {
                self.close_current_position(bid, ask, time)?;
                self.open_position(side, bid, ask, time)?;
            }
        }
        Ok(())
    }

    fn submit_market(
        &mut self,
        side: OrderSide,
        bid: Price,
        ask: Price,
        time: u64,
    ) -> Result<(), CalculationError> {
        let px = match side {
            OrderSide::Buy => ask,
            OrderSide::Sell => bid,
        };
        let mut order = self.create_order(side, px, time);
        let fill = self.fill_market(&order, bid, ask, time)?;
        self.apply_fill(&fill)?;
        order.status = OrderStatus::Filled;
        println!(
            "[paper] ORDER {} {:?} qty={} px={} fee={}",
            order.id, side, order.qty, fill.price, fill.fee
        );
        self.orders.push(order);
        self.fills.push(fill);
        Ok(())
    }
    //开仓，只有空仓的时候才能做
    fn open_position(&mut self,
        side: OrderSide,
        bid: Price,
        ask: Price,
        time: u64,) -> Result<(), CalculationError>{

        if self.position != Position::Flat {
            return Ok(());
        }
        self.submit_market(side, bid, ask, time)
    }

    // 这个是  平仓：根据当前持仓选相反方向。
    fn close_current_position(&mut self,
        bid: Price,
        ask: Price,
        time: u64,) -> Result<(), CalculationError>{

        let side = match self.position {
            Position::Flat => return Ok(()),
            Position::Long => OrderSide::Sell,
            Position::Short => OrderSide::Buy,
        };
        
        self.submit_market(side, bid, ask, time)    
        
    }


    fn create_order(&mut self, side: OrderSide, price: Price, time: u64) -> Order {
        let id = self.next_order_id;
        self.next_order_id += 1;
        let qty = match (self.position, side) {
            (Position::Long, OrderSide::Sell) | (Position::Short, OrderSide::Buy) => {
                self.position_qty
            }
            _ => self.qty,
        };
        Order {
            id,
            status: OrderStatus::New,
            side,
            qty,
            price,
            time,
        }
    }

    fn fill_market(
        &self,
        order: &Order,
        bid: Price,
        ask: Price,
        time: u64,
    ) -> Result<Fill, CalculationError> {
        let price = match order.side {
            OrderSide::Buy => ask,
            OrderSide::Sell => bid,
        };
        let fee = calculate_fee(price, order.qty, self.fee_bps)?;
        Ok(Fill {
            order_id: order.id,
            price,
            qty: order.qty,
            fee,
            time,
            side: order.side,
        })
    }

    fn apply_fill(&mut self, fill: &Fill) -> Result<(), CalculationError> {
        if self.position == Position::Flat {
            let pnl =
                money_from_raw(i128::from(self.realized_pnl.raw()) - i128::from(fill.fee.raw()))?;
            self.position = match fill.side {
                OrderSide::Buy => Position::Long,
                OrderSide::Sell => Position::Short,
            };
            self.entry_px = Some(fill.price);
            self.position_qty = fill.qty;
            self.realized_pnl = pnl;
        } else {
            // Current execution model only generates full closes, no partial fills.
            let gross = self.unrealized_pnl(fill.price)?;
            let pnl = money_from_raw(
                i128::from(self.realized_pnl.raw()) + i128::from(gross.raw())
                    - i128::from(fill.fee.raw()),
            )?;
            self.realized_pnl = pnl;
            self.position = Position::Flat;
            self.entry_px = None;
            self.position_qty = Quantity::from_raw(0);
        }
        Ok(())
    }

    pub fn close_position(&mut self, bid: Price, ask: Price, time: u64) {
        if self.position == Position::Flat {
            return;
        }
        if let Err(err) = Self::validate_quote(bid, ask) {
            eprintln!("[paper] invalid quote: {err:?}");
            return;
        }

        if let Err(err) = self.close_current_position(bid, ask, time) {
            eprintln!("[paper] close failed: {err:?}");
        }
    }

    pub fn orders(&self) -> &[Order] {
        &self.orders
    }
    pub fn fills(&self) -> &[Fill] {
        &self.fills
    }
    pub fn position(&self) -> Position {
        self.position
    }
    pub fn entry_price(&self) -> Option<Price> {
        self.entry_px
    }
    pub fn realized_pnl(&self) -> Money {
        self.realized_pnl
    }
    pub fn unrealized_pnl(&self, mark_price: Price) -> Result<Money, CalculationError> {
        let Some(entry) = self.entry_px else {
            return Ok(Money::from_raw(0));
        };
        let delta = match self.position {
            Position::Long => i128::from(mark_price.ticks()) - i128::from(entry.ticks()),
            Position::Short => i128::from(entry.ticks()) - i128::from(mark_price.ticks()),
            Position::Flat => 0,
        };
        money_from_raw(delta * i128::from(self.position_qty.raw()))
    }
    pub fn validate_quote(bid: Price, ask: Price) -> Result<(), QuoteError> {
        if bid > ask {
            return Err(QuoteError::NoCross);
        }
        Ok(())
    }
    pub fn total_fees(&self) -> Result<Money, CalculationError> {
        let total = self.fills.iter().try_fold(0_i128, |sum, fill| {
            sum.checked_add(i128::from(fill.fee.raw()))
                .ok_or(CalculationError::Overflow)
        })?;
        money_from_raw(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price(s: &str) -> Price {
        s.parse().unwrap()
    }

    #[test]
    fn full_long_trade_keeps_exact_fees_and_pnl() {
        let mut engine = PaperEngine::new(Quantity::from_raw(10_000), 5).unwrap();
        engine.on_signal(SignalSide::Buy, price("2999"), price("3000"), 1);
        engine.on_signal(SignalSide::Buy, price("2999"), price("3000"), 1);
        assert_eq!(
            engine.unrealized_pnl(price("3010")).unwrap().raw(),
            10_000_000
        );
        engine.close_position(price("3010"), price("3011"), 2);
        engine.close_position(price("3010"), price("3011"), 3);
        assert_eq!(engine.position(), Position::Flat);
        assert_eq!(engine.entry_price(), None);
        assert_eq!(engine.orders().len(), 2);
        assert_eq!(engine.fills().len(), 2);
        assert_eq!(engine.total_fees().unwrap().raw(), 3_005_000);
        assert_eq!(engine.realized_pnl().raw(), 6_995_000);
    }

    #[test]
    fn short_trade_can_close_at_a_loss() {
        let mut engine = PaperEngine::new(Quantity::from_raw(10_000), 5).unwrap();
        engine.on_signal(SignalSide::Sell, price("3000"), price("3001"), 1);
        engine.close_position(price("3009"), price("3010"), 2);
        assert_eq!(engine.realized_pnl().raw(), -13_005_000);
        assert_eq!(engine.position(), Position::Flat);
    }

    #[test]
    fn fee_overflow_does_not_create_a_fill_or_position() {
        let mut engine = PaperEngine::new(Quantity::from_raw(i64::MAX), u32::MAX).unwrap();
        let max_price = Price::from_ticks(i64::MAX).unwrap();
        engine.on_signal(SignalSide::Buy, max_price, max_price, 1);
        assert!(engine.fills().is_empty());
        assert!(engine.orders().is_empty());
        assert_eq!(engine.realized_pnl().raw(), 0);
        assert_eq!(engine.position(), Position::Flat);
    }
}
