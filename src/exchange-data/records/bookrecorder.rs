use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use super::bookrecord::BookRecord;

pub struct BookRecorder {
    writer: BufWriter<File>,
}

impl BookRecorder {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;

        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn write_record(
        &mut self,
        record: &BookRecord,
    ) -> std::io::Result<()> {
        serde_json::to_writer(&mut self.writer, record)
            .map_err(std::io::Error::other)?;

        self.writer.write_all(b"\n")?;

        Ok(())
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

pub fn spawn(
    path: &Path,
) -> std::io::Result<(
    mpsc::Sender<BookRecord>,
    JoinHandle<std::io::Result<()>>,
)> {
    // 启动时创建文件，失败直接交给调用方。
    let mut recorder = BookRecorder::new(path)?;

    let (tx, mut rx) = mpsc::channel::<BookRecord>(4096);

    let task = tokio::task::spawn_blocking(move || {
        let mut pending = 0;

        while let Some(record) = rx.blocking_recv() {
            recorder.write_record(&record)?;

            pending += 1;
            if pending >= 256 {
                recorder.flush()?;
                pending = 0;
            }
        }

        // 所有发送端关闭、队列排空后，刷新尾部数据。
        recorder.flush()
    });

    Ok((tx, task))
}