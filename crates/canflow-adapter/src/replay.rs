use async_trait::async_trait;
use canflow_types::*;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, Instant};
use tracing::debug;

use crate::parsers;
use crate::trait_def::CanAdapter;

pub struct ReplayAdapter {
    name: String,
    path: std::path::PathBuf,
    format: ReplayFormat,
    loop_forever: bool,
    filters: Vec<FrameFilter>,
    interface_id: InterfaceId,
    shutdown: bool,
}

impl ReplayAdapter {
    pub fn new(
        path: std::path::PathBuf,
        format: ReplayFormat,
        loop_forever: bool,
        interface_id: InterfaceId,
    ) -> Self {
        let name = format!("replay:{}", path.display());
        Self {
            name,
            path,
            format,
            loop_forever,
            filters: Vec::new(),
            interface_id,
            shutdown: false,
        }
    }
}

#[async_trait]
impl CanAdapter for ReplayAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run(&mut self, tx: mpsc::Sender<CanFrame>) -> Result<()> {
        debug!(path = %self.path.display(), "Replay adapter started");

        loop {
            let content = tokio::fs::read_to_string(&self.path).await?;
            let frames = match self.format {
                ReplayFormat::Candump => parsers::candump::parse(&content, self.interface_id),
                ReplayFormat::Asc => parsers::asc::parse(&content, self.interface_id),
            };

            if frames.is_empty() {
                return Err(CanFlowError::Adapter {
                    interface: self.name.clone(),
                    message: "no frames parsed from log file".to_string(),
                });
            }

            let start = Instant::now();
            let base_ts = frames[0].timestamp_ns;

            for frame in &frames {
                if self.shutdown {
                    return Ok(());
                }

                let relative_ns = frame.timestamp_ns.saturating_sub(base_ts);
                let target_elapsed = Duration::from_nanos(relative_ns);
                let actual_elapsed = start.elapsed();

                if target_elapsed > actual_elapsed {
                    sleep(target_elapsed - actual_elapsed).await;
                }

                let passes = self.filters.is_empty()
                    || self.filters.iter().any(|f| f.matches(frame));

                if passes {
                    let mut f = frame.clone();
                    f.timestamp_ns = timestamp::monotonic_ns();
                    if tx.send(f).await.is_err() {
                        return Err(CanFlowError::ChannelClosed);
                    }
                }
            }

            if !self.loop_forever {
                break;
            }
        }

        Ok(())
    }

    async fn send(&mut self, _frame: &CanFrame) -> Result<()> {
        Err(CanFlowError::Adapter {
            interface: self.name.clone(),
            message: "replay adapter does not support sending".to_string(),
        })
    }

    fn set_filters(&mut self, filters: Vec<FrameFilter>) {
        self.filters = filters;
    }

    async fn shutdown(&mut self) {
        self.shutdown = true;
    }
}
