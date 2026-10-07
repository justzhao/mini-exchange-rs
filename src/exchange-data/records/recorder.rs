use std::fs::{self, File};
use std::path::PathBuf;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::marketrecord::MarketRecord;

const DATA_DIR: &str = "data";

pub fn spawn(mut rx: mpsc::Receiver<MarketRecord>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut writer: Option<csv::Writer<File>> = None;

        while let Some(record) = rx.recv().await {
            if writer.is_none() {
                if let Err(err) = fs::create_dir_all(DATA_DIR) {
                    eprintln!("[recorder] create data dir failed: {err}");
                    continue;
                }
                let path = PathBuf::from(DATA_DIR)
                    .join(format!("{}_{}.csv", record.symbol, record.timestamp));
                match File::create(&path) {
                    Ok(file) => {
                        eprintln!("[recorder] writing {}", path.display());
                        writer = Some(csv::Writer::from_writer(file));
                    }
                    Err(err) => {
                        eprintln!("[recorder] open csv failed: {err}");
                        continue;
                    }
                }
            }

            if let Some(w) = writer.as_mut() {
                if let Err(err) = w.serialize(&record) {
                    eprintln!("[recorder] serialize failed: {err}");
                    continue;
                }
                if let Err(err) = w.flush() {
                    eprintln!("[recorder] flush failed: {err}");
                }
            }
        }
    })
}
