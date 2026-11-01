use crate::exchange_data::features::orderbookfeature::OrderBookFeature;
use super::orderbooksignal::OrderBookSignal;

pub trait Strategy: Send {
    fn on_feature(
        &mut self,
        feature: &OrderBookFeature,
    ) -> OrderBookSignal;
}

pub struct ImbalanceStrategy {
    threshold: f64,
}

impl ImbalanceStrategy {
    pub fn new(threshold: f64) -> Self {
        Self { threshold }
    }
}

impl Strategy for ImbalanceStrategy {
    fn on_feature(
        &mut self,
        feature: &OrderBookFeature,
    ) -> OrderBookSignal {
        OrderBookSignal::from_feature(feature, self.threshold)
    }
}