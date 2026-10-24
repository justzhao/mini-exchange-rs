use crate::exchange_data::features::orderbookfeature::OrderBookFeature;
pub struct OrderBookSignal{
    pub side: SignalSide,
    pub strength: f64, // 可用 |imbalance|，方便以后过滤
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalSide {
    Buy,
    Sell,
    Holder,
}


impl OrderBookSignal {
 
    pub fn from_feature(feat: &OrderBookFeature, thr: f64) -> Self {

        let imb = feat.imbalance;

        let lift = feat.microprice - feat.mid; 

        let side = if imb > thr &&  lift > 0.0 {

            SignalSide::Buy

        } else if imb < -thr && lift < 0.0 {
            
            SignalSide::Sell
        } else {
            
            SignalSide::Holder
        };
        Self {
            side,
            strength: imb.abs(),
        }
    }
}