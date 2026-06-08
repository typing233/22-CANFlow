use canflow_types::CanFrame;
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct Subscriber {
    rx: broadcast::Receiver<Arc<CanFrame>>,
    lagged_total: u64,
}

impl Subscriber {
    pub fn new(rx: broadcast::Receiver<Arc<CanFrame>>) -> Self {
        Self {
            rx,
            lagged_total: 0,
        }
    }

    pub async fn recv(&mut self) -> Option<Arc<CanFrame>> {
        loop {
            match self.rx.recv().await {
                Ok(frame) => return Some(frame),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    self.lagged_total += n;
                    tracing::warn!(lagged = n, total_lagged = self.lagged_total, "subscriber lagged");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }

    pub fn lagged_total(&self) -> u64 {
        self.lagged_total
    }
}
