use canflow_types::CanFrame;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::broadcast;
use tracing::{debug, error};

pub struct SessionRecorder {
    path: PathBuf,
    writer: Option<BufWriter<File>>,
    frames_written: u64,
}

impl SessionRecorder {
    pub async fn new(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let file = File::create(&path).await?;
        let writer = BufWriter::with_capacity(64 * 1024, file);
        Ok(Self {
            path,
            writer: Some(writer),
            frames_written: 0,
        })
    }

    pub async fn run(&mut self, mut rx: broadcast::Receiver<Arc<CanFrame>>) {
        debug!(path = %self.path.display(), "session recorder started");

        loop {
            match rx.recv().await {
                Ok(frame) => {
                    if let Some(writer) = &mut self.writer {
                        let json = serde_json::to_string(frame.as_ref()).unwrap_or_default();
                        if let Err(e) = writer.write_all(json.as_bytes()).await {
                            error!(error = %e, "recording write failed");
                            break;
                        }
                        if let Err(e) = writer.write_all(b"\n").await {
                            error!(error = %e, "recording write failed");
                            break;
                        }
                        self.frames_written += 1;

                        // Flush every 1000 frames
                        if self.frames_written % 1000 == 0 {
                            let _ = writer.flush().await;
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(lagged = n, "recorder lagged, frames lost in recording");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }

        if let Some(mut writer) = self.writer.take() {
            let _ = writer.flush().await;
        }
        debug!(frames = self.frames_written, "session recorder stopped");
    }

    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }
}
