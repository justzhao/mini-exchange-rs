use std::f32::consts::E;

use tokio::sync::mpsc;

use crate::exchange_data::books::localbook::BookSnapshot;
use crate::exchange_data::books::localbook::LocalBook;
use crate::exchange_data::evals::pendingeval::SignalEvaluator;
use crate::exchange_data::features::orderbookfeature::OrderBookFeature;
use crate::exchange_data::records::marketrecord::MarketRecord;
use crate::exchange_data::records::recorder;
use crate::exchange_data::signals::orderbooksignal::OrderBookSignal;
use crate::exchange_data::stream::Handler;
use crate::exchange_data::engine::ExecutionEngine;

const RECORD_QUEUE_SIZE: usize = 4096;

pub struct BookHandler<E: ExecutionEngine> {
    book: LocalBook,
    eval: SignalEvaluator,
    record_tx: mpsc::Sender<MarketRecord>,
    exec:E 
}

impl<E: ExecutionEngine> BookHandler<E> {
    pub fn new(coin: impl Into<String>,exec: E) -> Self {
        let (record_tx, record_rx) = mpsc::channel(RECORD_QUEUE_SIZE);
        recorder::spawn(record_rx);
        Self {
            book: LocalBook::new(coin),
            eval: SignalEvaluator::new(),
            record_tx,
            exec,
        }
    } 
}

pub trait BookEvent {
    type Snap: BookSnapshot + std::fmt::Debug;
    fn as_book(&self) -> Option<&Self::Snap>;
}

//impl<E: BookEvent> Handler<E> for BookHandler {
impl<Ev:BookEvent, Eng:ExecutionEngine> Handler<Ev> for BookHandler<Eng> {
    fn on_event(&mut self, event: Ev) {
        if let Some(snap) = event.as_book() {
            self.book.rebuild_book(snap);

            let Some(feat) = OrderBookFeature::new(&self.book) else {
                return;
            };
            let sig = OrderBookSignal::from_feature(&feat, 0.3);

          //  self.eval.push(self.book.time(), feat.mid, sig.side);
           // self.eval.on_mid(self.book.time(), feat.mid);

            if let Some(rec) = MarketRecord::from_book_feat_sig(&self.book, &feat, &sig) {
                if let Err(err) = self.record_tx.try_send(rec) {
                    eprintln!("[警告] record queue full or closed: {err}");
                }
            }
            if let (Some((bid, _)), Some((ask, _))) = (self.book.best_bid(), self.book.best_ask()) {
                self.exec.on_signal(sig.side, bid, ask, self.book.time());
            }
        }
    }
}
