use std::fmt::Debug;
use std::time::Duration;

use tokio::sync::broadcast;
use tokio::task;

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
    fn decode(&self, raw: &[u8]) -> Result<Vec<Self::Event>, serde_json::Error>;

    fn run(
        &mut self,
        subs: &[Self::Sub],
        events: broadcast::Sender<Self::Event>,
    ) -> impl Future<Output = ()> + Send;
}

pub trait Handler<E>: Send {
    fn on_event(&mut self, event: E)-> impl Future<Output = ()> + Send;

    fn shutdown(
        self,
    ) -> impl Future<Output = Result<(), tokio::task::JoinError>> + Send
    where
        Self: Sized,
    {
        async move {
            drop(self);
            Ok(())
        }
    }
}

pub struct PrintHandler;

impl<E: Debug + Send> Handler<E> for PrintHandler {
    async fn on_event(&mut self, event: E) {
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
         // 不使用这个初始接收端。
         drop(_rx);

         let mut tasks= Vec::new();

        for mut handler in handlers {
            let mut rx = tx.subscribe();
            let task= tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            handler.on_event(event).await;
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            eprintln!("[警告] 某 Handler 处理太慢，漏掉了 {} 条事件", skipped);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }

                // 通道关闭，剩余事件读完后，再关闭 Handler。
                if let Err(err) = handler.shutdown().await {
                    eprintln!("[handler] shutdown failed: {err}");
                }

            });
            tasks.push(task);
        }

        // 等待交易所运行结束，或者用户按 Ctrl+C。
        tokio::select! {
            _ = self.exchange.run(&self.subscriptions, tx.clone()) => {
                eprintln!("[stream] exchange stopped");
            }

            result = tokio::signal::ctrl_c() => {
                match result {
                    Ok(()) => eprintln!("[stream] stopping"),
                    Err(err) => {
                        eprintln!("[stream] signal error: {err}");
                    }
                }
            }
        }
         // select! 结束后，交易所运行 Future 已结束或被丢弃，
        // 它持有的发送端也会释放。再释放这里的发送端。
        drop(tx);
          // 消费者读完缓冲事件、关闭 recorder 后，再退出。
        for task in tasks {
        if let Err(err) = task.await {
            eprintln!("[stream] handler task failed: {err}");
        }
    }


        eprintln!("[stream] shutdown complete");
    }
}
