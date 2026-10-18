use crate::exchange_data::signals::orderbooksignal::Side;
pub struct RiskManager {
    cfg: RiskConfig,
}

pub struct RiskConfig {
    pub max_order_size: f64,  // 单笔 qty 上限，如 0.1
    pub max_position: f64,    // 成交后 |仓位数量| 上限，如 0.1
    pub max_notional: f64,    // qty * px 上限（报价币），如 500.0
}

#[derive(Debug)]
pub enum RiskReject {
    OrderSize { qty: f64, max: f64 },
    Position { after: f64, max: f64 },
    Notional { notional: f64, max: f64 },
}
impl RiskManager {
    pub fn new(cfg: RiskConfig) -> Self { Self { cfg } }
    /// `order_qty`: 本笔打算交易的数量（你现在就是 PaperEngine 的 qty）
    /// `pos_qty`: 当前仓位，带符号：Long=+qty, Short=-qty, Flat=0
    /// `px`: 用来估名义价值，可用 mid 或将要成交的 ask/bid
    pub fn check(
        &self,
        side: Side,
        order_qty: f64,
        pos_qty: f64,
        px: f64,
    ) -> Result<(), RiskReject> {
        if matches!(side, Side::Holder) {
            return Ok(()); // 或不调用 risk
        }
        // 1) 单笔过大
        if order_qty > self.cfg.max_order_size {
            return Err(RiskReject::OrderSize {
                qty: order_qty,
                max: self.cfg.max_order_size,
            });
        }
        // 2) 名义价值：本笔 order_qty * px
        let notional = order_qty * px;
        if notional > self.cfg.max_notional {
            return Err(RiskReject::Notional {
                notional,
                max: self.cfg.max_notional,
            });
        }
        // 3) 成交后仓位（按你现在的逻辑：开仓才会增仓；平仓先忽略复杂路径）
        // 简单版：Buy 在 Flat 时 → +order_qty；Sell 在 Flat 时 → -order_qty
        // 已有同向仓则 after = pos（你目前同向不开）；反向会先平再开，after ≈ ±order_qty
        let after = projected_position(side, pos_qty, order_qty);
        if after.abs() > self.cfg.max_position {
            return Err(RiskReject::Position {
                after,
                max: self.cfg.max_position,
            });
        }
        Ok(())
    }
}
fn projected_position(side: Side, pos: f64, order_qty: f64) -> f64 {
    match side {
        Side::Holder => pos,
        // 与 PaperEngine 一致的粗投影：最终若开仓，仓位落到 ±order_qty；平仓后 Flat=0 再开也是 ±order_qty
        Side::Buy => {
            if pos < 0.0 {
                order_qty // 平空再开多
            } else if pos == 0.0 {
                order_qty
            } else {
                pos // 已有多，同向忽略
            }
        }
        Side::Sell => {
            if pos > 0.0 {
                -order_qty
            } else if pos == 0.0 {
                -order_qty
            } else {
                pos
            }
        }
    }
}