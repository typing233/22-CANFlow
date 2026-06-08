use async_trait::async_trait;
use canflow_types::*;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{error, warn};

use crate::trait_def::CanAdapter;

pub struct ReconnectingAdapter {
    inner: Box<dyn CanAdapter>,
    policy: ReconnectPolicy,
}

impl ReconnectingAdapter {
    pub fn new(adapter: Box<dyn CanAdapter>, policy: ReconnectPolicy) -> Self {
        Self {
            inner: adapter,
            policy,
        }
    }
}

#[async_trait]
impl CanAdapter for ReconnectingAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    async fn run(&mut self, tx: mpsc::Sender<CanFrame>) -> Result<()> {
        let mut retries = 0u32;
        let mut delay_ms = self.policy.base_delay_ms;

        loop {
            match self.inner.run(tx.clone()).await {
                Ok(()) => return Ok(()),
                Err(CanFlowError::Shutdown) => return Ok(()),
                Err(CanFlowError::ChannelClosed) => return Err(CanFlowError::ChannelClosed),
                Err(e) => {
                    if self.policy.max_retries > 0 && retries >= self.policy.max_retries {
                        error!(adapter = %self.inner.name(), retries, "max retries reached");
                        return Err(e);
                    }

                    warn!(
                        adapter = %self.inner.name(),
                        error = %e,
                        retry_in_ms = delay_ms,
                        "adapter error, reconnecting"
                    );

                    sleep(Duration::from_millis(delay_ms)).await;
                    retries += 1;
                    delay_ms = (delay_ms * 2).min(self.policy.max_delay_ms);
                }
            }
        }
    }

    async fn send(&mut self, frame: &CanFrame) -> Result<()> {
        self.inner.send(frame).await
    }

    fn set_filters(&mut self, filters: Vec<FrameFilter>) {
        self.inner.set_filters(filters);
    }

    async fn shutdown(&mut self) {
        self.inner.shutdown().await;
    }
}
