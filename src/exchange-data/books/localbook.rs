use std::collections::BTreeMap;
use ordered_float::OrderedFloat;
use crate::Price;
use crate::Quantity;
pub struct LocalBook {
    coin: String,
    time: u64,
    // 买：价格从高到低，BTreeMap 默认升序，查询时 rev()
    bids: BTreeMap<Price, Level>, // 或用字符串价格 + 自己的序
    asks: BTreeMap<Price, Level>,
}
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub  enum RebuildBookResult {
    
    Updated,
    InvalidCoin,
    StaleTime,
    InvalidSnapshot,


}

pub trait BookSnapshot {
    fn coin(&self) -> &str;
    fn time(&self) -> u64;
    // (px, sz, n)
    fn bids(&self) -> Result<Vec<(Price, Quantity, u32)>, String>;
    fn asks(&self) -> Result<Vec<(Price, Quantity, u32)>, String>;
}



pub struct Level {
    pub(crate) sz: Quantity,
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

    pub fn coin(&self) -> &str {
        &self.coin
    }

    pub fn time(&self) -> u64 {
        self.time
    }
    
    pub fn rebuild_book(&mut self, snap: &impl BookSnapshot) -> RebuildBookResult{
        if snap.coin() != self.coin {
            return RebuildBookResult::InvalidCoin;
        }
        if  snap.time() <= self.time {
            return RebuildBookResult::StaleTime;
        }

        let bids = match snap.bids() {
            Ok(levels) => levels,
            Err(err) => {
                eprintln!("[book] invalid bids: {err}");
                return RebuildBookResult::InvalidSnapshot;
            }
        };

        let asks = match snap.asks() {
            Ok(levels) => levels,
            Err(err) => {
                eprintln!("[book] invalid asks: {err}");
                return RebuildBookResult::InvalidSnapshot;
            }
        };

        let new_bids = bids
        .into_iter()
        .map(|(px, sz, n)| (px, Level { sz, n }))
        .collect();

        let new_asks = asks
        .into_iter()
        .map(|(px, sz, n)| (px, Level { sz, n }))
        .collect();

        self.bids = new_bids;
        self.asks = new_asks;
        self.time = snap.time();

        RebuildBookResult::Updated
    }


    pub fn best_bid(&self) -> Option<(Price, &Level)> {
        self.bids
            .iter()
            .next_back()
             .map(|(px, lvl)| (*px, lvl))
    }
    pub fn best_ask(&self) -> Option<(Price, &Level)> {
        self.asks
            .iter()
            .next()
            .map(|(px, lvl)| (*px, lvl))
    }
    pub fn spread(&self) -> Option<f64> {
        let (bid, _) = self.best_bid()?;
        let (ask, _) = self.best_ask()?;

        // 先用整数计算价差，再换算成实际价格单位。
        let difference = i128::from(ask.ticks()) - i128::from(bid.ticks());

        Some(difference as f64 / 100.0)
    }

    pub fn mid(&self) -> Option<f64> {
        let (bid, _) = self.best_bid()?;
        let (ask, _) = self.best_ask()?;
    
        let sum = i128::from(bid.ticks()) + i128::from(ask.ticks());
    
        // 除以 100 换算价格，再除以 2 取平均。
        Some(sum as f64 / 200.0)
    }
    pub fn imbalance(&self) -> Option<f64>{

        let(_,bid) =self.best_bid()?;
        let(_,ask) =self.best_ask()?;
        let qb = bid.sz.raw() as f64 ;
        let qa = ask.sz.raw() as f64;
        let qsum =  qa+qb;
        if qsum == 0.0{
            return None;
        }
        Some( (qb-qa)/qsum )        
    }

    pub fn microprice(&self) -> Option<f64>{
        let(pb, bid) = self.best_bid()?;
        let(pa, ask) = self.best_ask()?;
        let qb = bid.sz.raw() as f64;
        let qa = ask.sz.raw() as f64;

        let qsum =  qa + qb;


        let pb = pb.ticks() as f64 / 100.0;
        let pa = pa.ticks() as f64 / 100.0;


        if qsum == 0.0{
            return None;
        }
        Some( (qa*pb + qb*pa )/ qsum )
    }
    
}
