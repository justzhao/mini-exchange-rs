use crate::exchange_data::signals::orderbooksignal::Side;
#[derive(Clone, Copy)]
struct PendingEval {
    t0_ms: u64,      // 信号时的 book.time（HL 是交易所时间）
    mid0: f64,
    side: Side,
    // 哪些 horizon 已经结算过
    done_100ms: bool,
    done_1s: bool,
    done_5s: bool,
}
pub struct SignalEvaluator {
    pending: Vec<PendingEval>,
    // 可选：累计统计
}
const H_100MS: u64 = 100;
const H_1S: u64 = 1_000;
const H_5S: u64 = 5_000;

impl SignalEvaluator {

    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    pub fn push(&mut self, t0_ms: u64, mid0: f64, side: Side) {
        if matches!(side, Side::Holder) {
            return; // 观望不算信号，不评估
        }
        self.pending.push(PendingEval {
            t0_ms,
            mid0,
            side,
            done_100ms: false,
            done_1s: false,
            done_5s: false,
        });
    }

    pub fn on_mid(&mut self, now_ms: u64, mid: f64) {
        self.pending.retain_mut(|p| {
            let dt = now_ms.saturating_sub(p.t0_ms);
            if !p.done_100ms && dt >= H_100MS {
                let ret = mid - p.mid0;
                println!("t0={} side={:?} +100ms mid={} return={:+.2}", p.t0_ms, p.side, mid, ret);
                p.done_100ms = true;
            }

            if !p.done_1s && dt >= H_1S {
                let ret = mid - p.mid0;
                println!("t0={} side={:?} +1s mid={} return={:+.2}", p.t0_ms, p.side, mid, ret);
                p.done_1s = true;
            }

            if !p.done_5s && dt >= H_5S {
                let ret = mid - p.mid0;
                println!("t0={} side={:?} +5s mid={} return={:+.2}", p.t0_ms, p.side, mid, ret);
                p.done_5s = true;
            }

            
            // 同理 1s、5s …
            // 5s 也做完就可以丢掉
            !(p.done_100ms && p.done_1s && p.done_5s)
        });
    }
}