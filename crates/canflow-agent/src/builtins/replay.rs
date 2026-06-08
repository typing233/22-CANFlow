use canflow_types::{CanFlowError, CanFrame, InterfaceId, ReplayFormat};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, Instant};
use tracing::debug;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayConfig {
    pub file: PathBuf,
    pub format: ReplayFormat,
    pub speed_multiplier: f64,
    pub loop_count: u32,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            file: PathBuf::from("capture.log"),
            format: ReplayFormat::Candump,
            speed_multiplier: 1.0,
            loop_count: 1,
        }
    }
}

pub struct ReplayTask {
    config: ReplayConfig,
}

impl ReplayTask {
    pub fn new(config: ReplayConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self, tx: mpsc::Sender<CanFrame>) -> Result<u64, CanFlowError> {
        let content = tokio::fs::read_to_string(&self.config.file).await?;
        let frames = match self.config.format {
            ReplayFormat::Candump => {
                canflow_adapter::parsers::candump::parse(&content, InterfaceId(0))
            }
            ReplayFormat::Asc => {
                canflow_adapter::parsers::asc::parse(&content, InterfaceId(0))
            }
        };

        if frames.is_empty() {
            return Err(CanFlowError::Config("no frames in replay file".to_string()));
        }

        let mut total_sent = 0u64;

        for _loop_idx in 0..self.config.loop_count {
            let start = Instant::now();
            let base_ts = frames[0].timestamp_ns;

            for frame in &frames {
                let relative_ns = frame.timestamp_ns.saturating_sub(base_ts);
                let adjusted_ns = (relative_ns as f64 / self.config.speed_multiplier) as u64;
                let target_elapsed = Duration::from_nanos(adjusted_ns);
                let actual_elapsed = start.elapsed();

                if target_elapsed > actual_elapsed {
                    sleep(target_elapsed - actual_elapsed).await;
                }

                let mut f = frame.clone();
                f.timestamp_ns = canflow_types::timestamp::monotonic_ns();
                if tx.send(f).await.is_err() {
                    return Err(CanFlowError::ChannelClosed);
                }
                total_sent += 1;
            }
        }

        debug!(total_sent, "replay completed");
        Ok(total_sent)
    }
}
