use std::fmt::Debug;
use std::time::Duration;

use tokio::sync::broadcast;

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

pub trait Exchange: Send {
    type Sub;
    type Event: Clone + Send + 'static;

    fn ws_url(&self) -> &str;
    fn encoding(&self) -> Encoding;
    fn ping_pong(&self) -> PingPong;
    fn decode(&self, raw: &[u8]) -> Option<Self::Event>;

    fn run(
        &mut self,
        subs: &[Self::Sub],
        events: broadcast::Sender<Self::Event>,
    ) -> impl Future<Output = ()> + Send;
}

pub trait Handler<E>: Send {
    fn on_event(&mut self, event: E);
}

pub struct PrintHandler;

impl<E: Debug + Send> Handler<E> for PrintHandler {
    fn on_event(&mut self, event: E) {
        println!("{event:?}");
    }
}

pub struct Stream<E: Exchange, H> {
    exchange: E,
    handler: Vec<H>,
    subscriptions: Vec<E::Sub>,
}

impl<E, H> Stream<E, H>
where
    E: Exchange,
    H: Handler<E::Event> + 'static,
{
    pub fn new(exchange: E, handler: Vec<H>) -> Self {
        Self {
            exchange,
            handler,
            subscriptions: Vec::new(),
        }
    }

    pub fn subscribe<I, S>(mut self, subscriptions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<E::Sub>,
    {
        self.subscriptions
            .extend(subscriptions.into_iter().map(Into::into));
        self
    }

    pub async fn run(mut self) {
        let (tx, _rx) = broadcast::channel::<E::Event>(EVENT_QUEUE_SIZE);
        let handlers = self.handler;

        for mut handler in handlers {
            let mut rx = tx.subscribe();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            handler.on_event(event);
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            eprintln!("[警告] 某 Handler 处理太慢，漏掉了 {} 条事件", skipped);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            });
        }

        self.exchange.run(&self.subscriptions, tx).await;
    }
}
