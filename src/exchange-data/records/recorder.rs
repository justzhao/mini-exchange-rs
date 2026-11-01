use std::fs::{self, File};
use std::path::PathBuf;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::marketrecord::MarketRecord;

const DATA_DIR: &str = "data";
const FLUSH_BATCH_SIZE: usize = 256;

pub fn spawn(mut rx: mpsc::Receiver<MarketRecord>) -> JoinHandle<()> {
    tokio::task::spawn_blocking( move|| {
        let mut writer: Option<csv::Writer<File>> = None;
        let mut pending_count = 0;
        while let Some(record) = rx.blocking_recv() {
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
                pending_count += 1;
                if pending_count >= FLUSH_BATCH_SIZE {
                    if let Err(err) = w.flush() {
                        eprintln!("[recorder] flush failed: {err}");
                        return;
                    }
    
                    pending_count = 0;
                }
            }
        }

        if let Some(w) = writer.as_mut() {
            if let Err(err) = w.flush() {
                eprintln!("[recorder] final flush failed: {err}");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange_data::records::marketrecord::Signal;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    // Remove only this test's reserved file, including when an assertion fails.
    struct TestFile(PathBuf);

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[tokio::test]
    async fn drains_records_before_exit() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let symbol = format!("ETH_RECORDER_TEST_{}_{unique}", std::process::id());
        let first_timestamp = 1_000;
        let path = PathBuf::from(DATA_DIR)
            .join(format!("{symbol}_{first_timestamp}.csv"));

        fs::create_dir_all(DATA_DIR).expect("测试数据目录应能创建");
        // Reserve a new filename so the recorder cannot overwrite an existing file.
        let reserved = File::options()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("测试文件名应唯一");
        let output = TestFile(path);
        drop(reserved);

        let (tx, rx) = mpsc::channel(16);
        let task = spawn(rx);
        let mut expected = Vec::new();

        // Three records are below the flush threshold; shutdown must write the tail.
        for i in 0..3_u64 {
            let record = MarketRecord {
                timestamp: first_timestamp + i,
                symbol: symbol.clone(),
                best_bid: format!("{}.00", 3_000 + i),
                best_ask: format!("{}.00", 3_001 + i),
                bid_qty: "0.010000".into(),
                ask_qty: "0.020000".into(),
                spread: 1.0,
                mid_price: 3_000.5 + i as f64,
                imbalance: -1.0 / 3.0,
                microprice: 3_000.0 + i as f64 + 1.0 / 3.0,
                signal: Signal::Holder,
            };

            // Serialize independently in memory to compare every field in order.
            let mut encoder = csv::WriterBuilder::new()
                .has_headers(false)
                .from_writer(Vec::new());
            encoder.serialize(&record).unwrap();
            let bytes = encoder.into_inner().unwrap();
            let row = csv::ReaderBuilder::new()
                .has_headers(false)
                .from_reader(bytes.as_slice())
                .records()
                .next()
                .unwrap()
                .unwrap();
            expected.push(row);

            tx.send(record).await.expect("录制任务应接收记录");
        }

        // No sleeps: close the input, then explicitly wait for draining and flushing.
        drop(tx);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("关闭通道后录制任务应及时结束")
            .expect("录制任务不应 panic");

        let mut reader = csv::Reader::from_path(&output.0)
            .expect("录制文件应可读取");
        assert_eq!(reader.headers().unwrap().get(0), Some("timestamp"));
        let actual = reader.records()
            .collect::<Result<Vec<_>, _>>()
            .expect("CSV 数据应完整且可解析");

        assert_eq!(actual.len(), 3);
        assert_eq!(actual, expected, "记录内容和顺序应保持不变");
    }
}
