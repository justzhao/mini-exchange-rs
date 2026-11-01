use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::exchange_data::hyperliquid::Event;
use crate::exchange_data::stream::Handler;

use super::bookrecord::BookRecord;
use super::bookrecorder;

pub struct BookRecordHandler {
    tx: mpsc::Sender<BookRecord>,
    task: JoinHandle<std::io::Result<()>>,
    sequence: u64,
}

impl BookRecordHandler {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        let (tx, task) = bookrecorder::spawn(path)?;

        Ok(Self {
            tx,
            task,
            sequence: 0,
        })
    }
}

impl Handler<Event> for BookRecordHandler {
    async fn on_event(&mut self, event: Event) {
        // 只录制完整盘口，忽略成交事件。
        let Event::L2Book(snapshot) = event else {
            return;
        };

        let received_at_ms = match SystemTime::now()
            .duration_since(UNIX_EPOCH)
        {
            Ok(duration) => duration.as_millis() as u64,
            Err(err) => {
                eprintln!("[book-recorder] clock failed: {err}");
                return;
            }
        };

        let record = BookRecord {
            version: 1,
            sequence: self.sequence,
            received_at_ms,
            snapshot,
        };

        if let Err(err) = self.tx.send(record).await {
            eprintln!("[book-recorder] send failed: {err}");
            return;
        }

        self.sequence += 1;
    }

    async fn shutdown(self) -> Result<(), tokio::task::JoinError> {
        // 关闭通道，让写盘任务排空队列并刷新。
        drop(self.tx);

        match self.task.await? {
            Ok(()) => {
                eprintln!("[book-recorder] recording complete");
            }
            Err(err) => {
                eprintln!("[book-recorder] write failed: {err}");
            }
        }

        Ok(())
    }
}