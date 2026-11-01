

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use crate::exchange_data::hyperliquid::L2Book;
use crate::exchange_data::books::localbook::{
    LocalBook, RebuildBookResult,
};
pub fn load_snapshots(
    path: &Path,
) -> Result<Vec<L2Book>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut snapshots = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let snapshot: L2Book = serde_json::from_str(&line)?;
        snapshots.push(snapshot);
    }

    Ok(snapshots)
}

use crate::exchange_data::records::bookrecord::BookRecord;

pub fn load_book_records(
    path: &Path,
) -> Result<Vec<BookRecord>, Box<dyn std::error::Error>>{
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    let mut expected_sequence = 0_u64;

    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        let record: BookRecord = serde_json::from_str(&line)?;

        if record.version != 1 {
            return Err(format!(
                "第 {} 行不支持的数据版本：{}",
                index + 1,
                record.version,
            ).into());
        }
        if record.sequence != expected_sequence {
            return Err(format!(
                "第 {} 行顺序号异常：预期 {}，实际 {}",
                index + 1,
                expected_sequence,
                record.sequence,
            ).into());
        }
        expected_sequence = expected_sequence .checked_add(1).ok_or("录制顺序号溢出")?;

        records.push(record);
    }

    Ok(records)
}
pub fn load_recorded_snapshots(
    path: &Path,
) -> Result<Vec<L2Book>, Box<dyn std::error::Error>> {
    let records = load_book_records(path)?;

    Ok(records
        .into_iter()
        .map(|record| record.snapshot)
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_snapshots_in_file_order() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/replay_eth.jsonl");

        let snapshots = load_snapshots(&path)
            .expect("回放样本应读取成功");

        assert_eq!(snapshots.len(), 2);

        assert_eq!(snapshots[0].coin, "ETH");
        assert_eq!(snapshots[0].time, 1000);

        assert_eq!(snapshots[1].coin, "ETH");
        assert_eq!(snapshots[1].time, 2000);

        assert_eq!(snapshots[0].levels[0][0].px, "3000.00");
        assert_eq!(snapshots[1].levels[0][0].px, "3010.00");
    }

    #[test]
    fn replays_snapshots_into_local_book() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/replay_eth.jsonl");

        let snapshots = load_snapshots(&path)
            .expect("回放样本应读取成功");

        let mut book = LocalBook::new("ETH");

        for snapshot in &snapshots {
            assert_eq!(
                book.rebuild_book(snapshot),
                RebuildBookResult::Updated,
            );
        }

        assert_eq!(book.time(), 2000);

        let (bid, bid_level) = book.best_bid().expect("应有买盘");
        let (ask, ask_level) = book.best_ask().expect("应有卖盘");

        assert_eq!(bid.ticks(), 301000);
        assert_eq!(ask.ticks(), 301100);
        assert_eq!(bid_level.sz.raw(), 1_000_000);
        assert_eq!(ask_level.sz.raw(), 10_000_000);
    }
}