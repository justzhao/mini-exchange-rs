use std::collections::BTreeMap;
use ordered_float::OrderedFloat;

pub struct LocalBook {
    coin: String,
    time: u64,
    // 买：价格从高到低，BTreeMap 默认升序，查询时 rev()
    bids: BTreeMap<OrderedFloat<f64>, Level>, // 或用字符串价格 + 自己的序
    asks: BTreeMap<OrderedFloat<f64>, Level>,
}

pub trait BookSnapshot {
    fn coin(&self) -> &str;
    fn time(&self) -> u64;
    // (px, sz, n)
    fn bids(&self) -> impl Iterator<Item = (f64, f64, u32)>;
    fn asks(&self) -> impl Iterator<Item = (f64, f64, u32)>;
}



pub struct Level {
    sz: f64,
    n: u32,
}

impl LocalBook {

    pub fn new(coin: impl Into<String>) -> Self { 
        Self {
            coin: coin.into(),
            time: 0,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }
    
    pub fn rebuild_book(&mut self, snap: &impl BookSnapshot) {
        if snap.coin() != self.coin || snap.time() < self.time {
            return;
        }
        self.time = snap.time();
        self.bids.clear();
        self.asks.clear();
        // 填档 ...
        self.bids.extend(snap.bids().map(|(px, sz, n)| {
            (OrderedFloat(px), Level { sz, n })
        }));
        self.asks.extend(snap.asks().map(|(px, sz, n)| {
            (OrderedFloat(px), Level { sz, n })
        }));
    }


    pub fn best_bid(&self) -> Option<(f64, &Level)> {
        self.bids
            .iter()
            .next_back()
            .map(|(px, lvl)| (px.into_inner(), lvl))
    }
    pub fn best_ask(&self) -> Option<(f64, &Level)> {
        self.asks
            .iter()
            .next()
            .map(|(px, lvl)| (px.into_inner(), lvl))
    }
    pub fn spread(&self) -> Option<f64> {
        let (bid, _) = self.best_bid()?;
        let (ask, _) = self.best_ask()?;
        Some(ask - bid)
    }

    pub fn mid(&self) -> Option<f64> {
        let (bid, _) = self.best_bid()?;
        let (ask, _) = self.best_ask()?;
        Some((bid + ask) / 2.0)
    }
    
}
