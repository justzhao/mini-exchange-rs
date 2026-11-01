use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tokio::time::{timeout};
use super::stream::{Encoding, Exchange, PingPong};

use crate::exchange_data::books::localbook::BookSnapshot;
use crate::exchange_data::books::bookhandler::BookEvent;
use std::collections::HashSet;
use crate::exchange_data::types::{
    price::Price,
    quantity::Quantity,
};


type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsError = Box<dyn std::error::Error + Send + Sync>;

const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Trade {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub time: u64,
}

#[derive(Debug, Clone, serde::Deserialize,serde::Serialize)]
pub struct BookLevel {
    pub px: String,
    pub sz: String,
    pub n: u32,
}

#[derive(Debug, Clone, serde::Deserialize,serde::Serialize)]
pub struct L2Book {
    pub coin: String,
    pub time: u64,
    pub levels: [Vec<BookLevel>; 2],
}


impl BookSnapshot for L2Book { 
    fn coin(&self) -> &str {
        &self.coin
    }
    fn time(&self) -> u64 {
        self.time
    }
    fn bids(&self) -> Result<Vec<(Price, Quantity, u32)>, String>{
        
        self.levels[0].iter().map(parse_level).collect()
    
    }
    fn asks(&self) -> Result<Vec<(Price, Quantity, u32)>, String>{
        self.levels[1].iter().map(parse_level).collect()
    }

    
}

fn parse_level(level: &BookLevel) -> Result<(Price, Quantity, u32), String> {

    let price = level.px.parse::<Price>()
        .map_err(|err| {
            format!("invalid price {:?}: {err:?}", level.px)
        })?;

    let quantity = level.sz.parse::<Quantity>()
        .map_err(|err| {
            format!("invalid quantity {:?}: {err:?}", level.sz)
        })?;

    Ok((price, quantity, level.n))

    // Some((
    //     level.px.parse().ok()?,
    //     level.sz.parse().ok()?,
    //     level.n,
    // ))
}

#[derive(Debug, Clone, serde::Deserialize)]
pub enum Event {
    Trade(Trade),
    L2Book(L2Book),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct HlSub {
    #[serde(rename = "type")]
    pub kind: String,
    pub coin: String,
}
#[derive(Debug, serde::Deserialize)]
struct SubscriptionResponse {
    method: String,
    subscription: HlSub,
}

impl From<(&str, &str)> for HlSub {
    fn from((coin, kind): (&str, &str)) -> Self {
        Self {
            coin: coin.into(),
            kind: kind.into(),
        }
    }
}

impl BookEvent for Event {
    type Snap = L2Book;
    fn as_book(&self) -> Option<&L2Book> {
        match self {
            Event::L2Book(book) => Some(book),
            Event::Trade(_) => None,
        }
    }
}

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

    fn subscribe_payload(sub_type: &str, coin: &str) -> String {
        serde_json::json!({
            "method": "subscribe",
            "subscription": {
                "type": sub_type,
                "coin": coin,
            }
        })
        .to_string()
    }

    async fn connect(&mut self) -> Result<(), WsError> {
        self.ws = None;
       // let (ws, _) = connect_async(&self.ws_url).await?;
       let (ws, _) = timeout(
        Duration::from_secs(10),
        connect_async(&self.ws_url),
        )
         .await??;
        
        self.ws = Some(ws);
        Ok(())
    }

    async fn send_subs(&mut self, subs: &[HlSub]) -> Result<(), WsError> {
        self.connect().await?;
        let ws = self.ws.as_mut().ok_or("websocket missing")?;
        for sub in subs {
            
            let message = Message::Text(
                Self::subscribe_payload(&sub.kind, &sub.coin).into(),
            );
            timeout(
                Duration::from_secs(10),
                ws.send(message),
            )
            .await??;

        }
        Ok(())
    }

    async fn read_loop(&mut self, subs: &[HlSub], events: &broadcast::Sender<Event>) -> Result<(), WsError> {
        let mut pending: HashSet<HlSub> =subs.iter().cloned().collect();
        let ping = self.ping_pong();
        let mut ticker = tokio::time::interval(ping.interval);
        let ws = self.ws.take().ok_or("websocket missing")?;
        let (mut write, mut read) = ws.split();
        let idle_timeout = Duration::from_secs(60);
        let idle_timer = tokio::time::sleep(idle_timeout);
        tokio::pin!(idle_timer);

        let subscription_timer =
        tokio::time::sleep(Duration::from_secs(10));
        tokio::pin!(subscription_timer);
       

        loop {
            tokio::select! {
                _ = &mut idle_timer => {
                    return Err("60 秒未收到消息".into());
                }
                _ = &mut subscription_timer, if !pending.is_empty() => {
                    return Err(
                        format!(
                            "subscription confirmation timed out; pending: {pending:?}"
                        )
                        .into()
                    );
                }
                _ = ticker.tick() => {
                    timeout(
                        Duration::from_secs(10),
                        write.send(Message::Text(ping.ping_payload.into())),
                    )
                    .await??;
                }
                msg = read.next() => {
                    //读取超时
                    idle_timer.as_mut().reset(
                        tokio::time::Instant::now() + idle_timeout
                    );
                    match msg {
                        Some(Ok(Message::Text(text))) => {

                                                        // 先处理订阅确认消息。
                            if let Ok(envelope) = serde_json::from_str::<Envelope>(&text) {
                                if envelope.channel == "subscriptionResponse" {
                                    match serde_json::from_value::<SubscriptionResponse>(
                                        envelope.data,
                                    ) {
                                        Ok(response) => {
                                            if response.method == "subscribe" {
                                                let removed =
                                                    pending.remove(&response.subscription);

                                                if removed {
                                                    println!(
                                                        "[subscription] confirmed: {}/{}",
                                                        response.subscription.coin,
                                                        response.subscription.kind,
                                                    );

                                                    if pending.is_empty() {
                                                        println!("[subscription] all confirmed");
                                                    }
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            eprintln!("[subscription] invalid response: {err}");
                                        }
                                    }

                                    continue;
                                }
                            }

                            match   self.decode(text.as_bytes()){
                                 Ok(deocdes)=>{

                                    for event in deocdes {
                                        events.send(event).map_err(|err| {
                                            format!("event broadcast failed: {err}")
                                        })?;
                                    }

                                 } ,
                                 Err(err) =>{
                                    eprintln!("decode error {err:?}")
                                 }  

                            }  
                        }
                        Some(Ok(Message::Ping(payload))) => {
                            timeout(
                                Duration::from_secs(10),
                                write.send(Message::Pong(payload)),
                            )
                            .await??;
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
    type Sub = HlSub;
    type Event = Event;

    fn ws_url(&self) -> &str {
        &self.ws_url
    }

    fn encoding(&self) -> Encoding {
        self.encoding
    }

    fn ping_pong(&self) -> PingPong {
        self.ping_pong
    }

    fn decode(&self, raw: &[u8]) -> Result<Vec<Self::Event>, serde_json::Error>{
        
        match self.encoding {
            Encoding::Json =>{
                 decode_json(raw)
             }


        }
    }

    async fn run(
        &mut self,
        subs: &[Self::Sub],
        events: broadcast::Sender<Self::Event>,
    ) {
        let mut backoff = INITIAL_BACKOFF;

        loop {
            match self.send_subs(subs).await {
                Ok(()) => backoff = INITIAL_BACKOFF,
                Err(err) => {
                    eprintln!("websocket connect failed: {err}");
                    self.ws = None;
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                    continue;
                }
            }

            if let Err(err) = self.read_loop(subs,&events).await {
                eprintln!("websocket disconnected: {err}");
            }
            self.ws = None;
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }
}

fn decode_json(raw: &[u8]) -> Result<Vec<Event>, serde_json::Error>{
    let envelope: Envelope = serde_json::from_slice(raw)?;
    match envelope.channel.as_str() {
        "trades" => {
            let trades: Vec<Trade> = serde_json::from_value(envelope.data)?;
         
            let events=  trades.into_iter().map(Event::Trade).collect::<Vec<Event>>();

            Ok(events)
        }
        "l2Book" => {
            let book: L2Book = serde_json::from_value(envelope.data)?;
            Ok(vec![Event::L2Book(book)])
        }
        "subscriptionResponse" => {
            let response: SubscriptionResponse =
            serde_json::from_value(envelope.data)?;
    
        println!(
            "[subscription] method={} coin={} kind={}",
            response.method,
            response.subscription.coin,
            response.subscription.kind,
        );

            Ok(vec![])
        }
        _ => Ok(vec![]),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_all_trades_in_order() {
        let raw = br#"{
            "channel": "trades",
            "data": [
                {"coin":"ETH","side":"B","px":"3000","sz":"0.01","time":1},
                {"coin":"ETH","side":"A","px":"3001","sz":"0.02","time":2},
                {"coin":"ETH","side":"B","px":"3002","sz":"0.03","time":3}
            ]
        }"#;

        let events = decode_json(raw).expect("成交消息应解析成功");

        assert_eq!(events.len(), 3);

        for (event, expected_price) in
            events.iter().zip(["3000", "3001", "3002"])
        {
            match event {
                Event::Trade(trade) => {
                    assert_eq!(trade.px.as_str(), expected_price);
                }
                other => panic!("预期 Trade，实际为 {other:?}"),
            }
        }
    }
}