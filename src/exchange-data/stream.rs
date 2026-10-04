use std::time::Duration;

use tokio::sync::mpsc;

const EVENT_QUEUE_SIZE: usize = 4096;

#[derive(Clone, Copy)]
pub enum Encoding {
    Json,
}

#[derive(Clone, Copy)]
pub struct PingPong {
    pub interval: Duration,
    pub ping_payload: &'static str,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Trade {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub time: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub enum Event {
    Trade(Trade),
}

pub trait Exchange: Send {
    fn ws_url(&self) -> &str;
    fn encoding(&self) -> Encoding;
    fn ping_pong(&self) -> PingPong;
    fn decode(&self, raw: &[u8]) -> Option<Event>;

    fn run(
        &mut self,
        coins: &[String],
        events: mpsc::Sender<Event>,
    ) -> impl Future<Output = ()> + Send;
}

pub trait Handler: Send {
    fn on_event(&mut self, event: Event);
}

pub struct PrintHandler;

impl Handler for PrintHandler {
    fn on_event(&mut self, event: Event) {
        println!("{event:?}");
    }
}

pub struct Stream<E, H> {
    exchange: E,
    handler: H,
    trades: Vec<String>,
}

impl<E: Exchange, H: Handler + 'static> Stream<E, H> {
    pub fn new(exchange: E, handler: H) -> Self {
        Self {
            exchange,
            handler,
            trades: Vec::new(),
        }
    }

    pub fn subscribe_trades(mut self, coin: impl Into<String>) -> Self {
        self.trades.push(coin.into());
        self
    }

    pub async fn run(mut self) {
        let (tx, mut rx) = mpsc::channel(EVENT_QUEUE_SIZE);
        let mut handler = self.handler;
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                handler.on_event(event);
            }
        });
        self.exchange.run(&self.trades, tx).await;
    }
}
