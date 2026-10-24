use crate::exchange_data::signals::orderbooksignal::SignalSide;
use crate::exchange_data::types::{
    money::Money,
    price::Price,
    quantity::Quantity,
};
use crate::exchange_data::engine::Position;
pub struct RiskManager {
    cfg: RiskConfig,
}

pub struct RiskConfig {
    pub max_order_size: Quantity,  // 单笔 qty 上限，如 0.1
    pub max_position: Quantity,    // 成交后 |仓位数量| 上限，如 0.1
    pub max_notional: Money,    // qty * px 上限（报价币），如 500.0
}

#[derive(Debug)]
pub enum RiskReject {
    OrderSize {
        qty: Quantity,
        max: Quantity,
    },
    Position {
        after: Quantity,
        max: Quantity,
    },
    Notional {
        notional: Money,
        max: Money,
    },
    Overflow,
}
impl RiskManager {
    pub fn new(cfg: RiskConfig) -> Self { Self { cfg } }
    /// `order_qty`: 本笔打算交易的数量（你现在就是 PaperEngine 的 qty）
    /// `pos_qty`: 当前仓位，带符号：Long=+qty, Short=-qty, Flat=0
    /// `px`: 用来估名义价值，可用 mid 或将要成交的 ask/bid
    pub fn check(
        &self,
        side: SignalSide,
        order_qty: Quantity,
        position: Position,
        position_qty: Quantity,
        px: Price,
    ) -> Result<(), RiskReject> {

           // 观望和同向信号不会产生订单。
        match (position, side) {
            (_, SignalSide::Holder)
            | (Position::Long, SignalSide::Buy)
            | (Position::Short, SignalSide::Sell) => {
                return Ok(());
            }
            _ => {}
        }
        if order_qty.raw() > self.cfg.max_order_size.raw() {
            return Err(RiskReject::OrderSize {
                qty: order_qty,
                max: self.cfg.max_order_size,
            });
        }

        // Price 两位、Quantity 六位，乘积就是 Money 八位。
        let notional_raw =
            i128::from(px.ticks()) * i128::from(order_qty.raw());

        let notional_raw = i64::try_from(notional_raw)
            .map_err(|_| RiskReject::Overflow)?;

        let notional = Money::from_raw(notional_raw);

        if notional.raw() > self.cfg.max_notional.raw() {
            return Err(RiskReject::Notional {
                notional,
                max: self.cfg.max_notional,
            });
        }

        let after = projected_position(
            side,
            position,
            position_qty,
            order_qty,
        );

        if after.raw() > self.cfg.max_position.raw() {
            return Err(RiskReject::Position {
                after,
                max: self.cfg.max_position,
            });
        }

        Ok(())
        
    }
}
fn projected_position(side: SignalSide, position: Position, position_qty: Quantity, order_qty: Quantity) -> Quantity {
    match (position, side) {
        (_, SignalSide::Holder)
        | (Position::Long, SignalSide::Buy)
        | (Position::Short, SignalSide::Sell) => position_qty,

        _ => order_qty,
    }
}