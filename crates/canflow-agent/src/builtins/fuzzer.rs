use canflow_types::{CanFlowError, CanFrame, CanId};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::debug;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FuzzConfig {
    pub target_ids: Vec<u32>,
    pub iterations: u64,
    pub min_dlc: u8,
    pub max_dlc: u8,
    pub delay_us: u64,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            target_ids: vec![0x7DF],
            iterations: 1000,
            min_dlc: 1,
            max_dlc: 8,
            delay_us: 100,
        }
    }
}

pub struct Fuzzer {
    config: FuzzConfig,
}

impl Fuzzer {
    pub fn new(config: FuzzConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self, tx: mpsc::Sender<CanFrame>) -> Result<u64, CanFlowError> {
        let mut rng = rand::thread_rng();
        let mut sent = 0u64;

        debug!(
            targets = ?self.config.target_ids,
            iterations = self.config.iterations,
            "fuzzer started"
        );

        for i in 0..self.config.iterations {
            let target_id = self.config.target_ids[i as usize % self.config.target_ids.len()];
            let dlc = rng.gen_range(self.config.min_dlc..=self.config.max_dlc);
            let mut data = [0u8; 8];
            for b in data[..dlc as usize].iter_mut() {
                *b = rng.gen();
            }

            let id = if target_id > 0x7FF {
                CanId::extended(target_id)
            } else {
                CanId::standard(target_id as u16)
            };

            let frame = CanFrame::new(id, &data[..dlc as usize])
                .with_timestamp(canflow_types::timestamp::monotonic_ns());

            if tx.send(frame).await.is_err() {
                return Err(CanFlowError::ChannelClosed);
            }
            sent += 1;

            if self.config.delay_us > 0 {
                tokio::time::sleep(tokio::time::Duration::from_micros(self.config.delay_us)).await;
            }
        }

        debug!(sent, "fuzzer completed");
        Ok(sent)
    }
}
