use canflow_types::FaultConfig;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaultInjector {
    config: FaultConfig,
    dropped: u64,
    corrupted: u64,
    delayed: u64,
}

impl FaultInjector {
    pub fn new(config: FaultConfig) -> Self {
        Self {
            config,
            dropped: 0,
            corrupted: 0,
            delayed: 0,
        }
    }

    pub fn config(&self) -> &FaultConfig {
        &self.config
    }

    pub fn stats(&self) -> (u64, u64, u64) {
        (self.dropped, self.corrupted, self.delayed)
    }
}
