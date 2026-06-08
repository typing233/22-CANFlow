use canflow_types::CanFrame;
use chrono::Utc;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::broadcast;
use tracing::{debug, error};

#[derive(Serialize)]
struct AuditEntry {
    timestamp: String,
    frame_id: String,
    dlc: u8,
    data_hex: String,
    interface: u16,
    is_error: bool,
}

pub struct AuditLogger {
    dir: PathBuf,
    writer: Option<BufWriter<File>>,
    entries_in_file: u64,
    max_entries_per_file: u64,
}

impl AuditLogger {
    pub async fn new(dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        tokio::fs::create_dir_all(&dir).await?;
        let mut logger = Self {
            dir,
            writer: None,
            entries_in_file: 0,
            max_entries_per_file: 100_000,
        };
        logger.rotate().await?;
        Ok(logger)
    }

    async fn rotate(&mut self) -> std::io::Result<()> {
        if let Some(mut w) = self.writer.take() {
            w.flush().await?;
        }
        let filename = format!("audit_{}.jsonl", Utc::now().format("%Y%m%d_%H%M%S"));
        let path = self.dir.join(filename);
        let file = File::create(&path).await?;
        self.writer = Some(BufWriter::with_capacity(32 * 1024, file));
        self.entries_in_file = 0;
        debug!(path = %path.display(), "audit log rotated");
        Ok(())
    }

    pub async fn run(&mut self, mut rx: broadcast::Receiver<Arc<CanFrame>>) {
        debug!("audit logger started");

        loop {
            match rx.recv().await {
                Ok(frame) => {
                    if self.entries_in_file >= self.max_entries_per_file {
                        if let Err(e) = self.rotate().await {
                            error!(error = %e, "audit log rotation failed");
                            break;
                        }
                    }

                    let entry = AuditEntry {
                        timestamp: canflow_types::timestamp::format_timestamp(frame.timestamp_ns),
                        frame_id: format!("{}", frame.id),
                        dlc: frame.dlc,
                        data_hex: frame.payload().iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(""),
                        interface: frame.interface.0,
                        is_error: frame.is_error,
                    };

                    if let Some(writer) = &mut self.writer {
                        let json = serde_json::to_string(&entry).unwrap_or_default();
                        if writer.write_all(json.as_bytes()).await.is_err() {
                            break;
                        }
                        if writer.write_all(b"\n").await.is_err() {
                            break;
                        }
                        self.entries_in_file += 1;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }

        if let Some(mut w) = self.writer.take() {
            let _ = w.flush().await;
        }
    }
}
