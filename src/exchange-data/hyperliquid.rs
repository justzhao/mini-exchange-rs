use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use super::stream::{Encoding, Event, Exchange, PingPong, Trade};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsError = Box<dyn std::error::Error + Send + Sync>;

const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

pub struct Hyperliquid {
    pub ws_url: String,
    pub encoding: Encoding,
    pub ping_pong: PingPong,
    ws: Option<Ws>,
}

impl Hyperliquid {
    pub fn mainnet() -> Self {
        Self {
            ws_url: "wss://api.hyperliquid.xyz/ws".to_string(),
            encoding: Encoding::Json,
            ping_pong: PingPong {
                interval: Duration::from_secs(30),
                ping_payload: r#"{"method":"ping"}"#,
            },
            ws: None,
        }
    }

    fn trades_payload(coin: &str) -> String {
        serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": "trades",
                "coin": coin,
            }
        })
        .to_string()
    }

    async fn connect(&mut self) -> Result<(), WsError> {
        self.ws = None;
        let (ws, _) = connect_async(&self.ws_url).await?;
        self.ws = Some(ws);
        Ok(())
    }

    async fn send_trades_subs(&mut self, coins: &[String]) -> Result<(), WsError> {
        self.connect().await?;
        let ws = self.ws.as_mut().ok_or("websocket missing")?;
        for coin in coins {
            ws.send(Message::Text(Self::trades_payload(coin).into()))
                .await?;
        }
        Ok(())
    }

    async fn read_loop(&mut self, events: &mpsc::Sender<Event>) -> Result<(), WsError> {
        let ping = self.ping_pong();
        let mut ticker = tokio::time::interval(ping.interval);
        let ws = self.ws.take().ok_or("websocket missing")?;
        let (mut write, mut read) = ws.split();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    write
                        .send(Message::Text(ping.ping_payload.into()))
                        .await?;
                }
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Some(event) = self.decode(text.as_bytes()) {
                                match events.try_send(event) {
                                    Ok(()) => {}
                                    Err(mpsc::error::TrySendError::Full(_)) => {
                                        eprintln!("event queue full, dropping trade");
                                    }
                                    Err(mpsc::error::TrySendError::Closed(_)) => {
                                        return Err("event handler stopped".into());
                                    }
                                }
                            }
                        }
                        Some(Ok(Message::Ping(payload))) => {
                            write.send(Message::Pong(payload)).await?;
                        }
                        Some(Ok(Message::Close(frame))) => {
                            return Err(format!("server closed: {frame:?}").into());
                        }
                        Some(Ok(_)) => {}
                        Some(Err(err)) => return Err(err.into()),
                        None => return Err("websocket stream ended".into()),
                    }
                }
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct Envelope {
    channel: String,
    #[serde(default)]
    data: serde_json::Value,
}

impl Exchange for Hyperliquid {
    fn ws_url(&self) -> &str {
        &self.ws_url
    }

    fn encoding(&self) -> Encoding {
        self.encoding
    }

    fn ping_pong(&self) -> PingPong {
        self.ping_pong
    }

    fn decode(&self, raw: &[u8]) -> Option<Event> {
        match self.encoding {
            Encoding::Json => decode_json(raw),
        }
    }

    async fn run(&mut self, coins: &[String], events: tokio::sync::mpsc::Sender<Event>) {
        let mut backoff = INITIAL_BACKOFF;

        loop {
            match self.send_trades_subs(coins).await {
                Ok(()) => backoff = INITIAL_BACKOFF,
                Err(err) => {
                    eprintln!("websocket connect failed: {err}");
                    self.ws = None;
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                    continue;
                }
            }

            if let Err(err) = self.read_loop(&events).await {
                eprintln!("websocket disconnected: {err}");
            }
            self.ws = None;
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }
}

fn decode_json(raw: &[u8]) -> Option<Event> {
    let envelope: Envelope = serde_json::from_slice(raw).ok()?;
    if envelope.channel != "trades" {
        return None;
    }

    let trades: Vec<Trade> = serde_json::from_value(envelope.data).ok()?;
    trades.into_iter().next().map(Event::Trade)
}
