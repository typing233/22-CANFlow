use canflow_types::{CanFlowError, CanFrame, CanId};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{debug, info};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UdsProbeConfig {
    pub target_id: u32,
    pub response_id: u32,
    pub services: Vec<u8>,
    pub sub_functions: bool,
    pub timeout_ms: u64,
}

impl Default for UdsProbeConfig {
    fn default() -> Self {
        Self {
            target_id: 0x7DF,
            response_id: 0x7E8,
            services: vec![0x10, 0x11, 0x22, 0x27, 0x2E, 0x31, 0x34, 0x36, 0x37, 0x3E],
            sub_functions: true,
            timeout_ms: 100,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ProbeResult {
    pub service_id: u8,
    pub sub_function: Option<u8>,
    pub response: ProbeResponse,
}

#[derive(Clone, Debug, Serialize)]
pub enum ProbeResponse {
    Positive { data: Vec<u8> },
    Negative { nrc: u8 },
    Timeout,
}

pub struct UdsProbe {
    config: UdsProbeConfig,
}

impl UdsProbe {
    pub fn new(config: UdsProbeConfig) -> Self {
        Self { config }
    }

    pub async fn run(&self, tx: mpsc::Sender<CanFrame>) -> Result<Vec<ProbeResult>, CanFlowError> {
        let mut results = Vec::new();

        let target_id = if self.config.target_id > 0x7FF {
            CanId::extended(self.config.target_id)
        } else {
            CanId::standard(self.config.target_id as u16)
        };

        debug!(target = %self.config.target_id, services = ?self.config.services, "UDS probe started");

        for &service in &self.config.services {
            // Send UDS request: [length, service_id]
            let data = [0x02, service, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
            let frame = CanFrame {
                timestamp_ns: canflow_types::timestamp::monotonic_ns(),
                id: target_id,
                dlc: 8,
                data,
                is_error: false,
                is_remote: false,
                interface: canflow_types::InterfaceId(0),
            };

            if tx.send(frame).await.is_err() {
                return Err(CanFlowError::ChannelClosed);
            }

            results.push(ProbeResult {
                service_id: service,
                sub_function: None,
                response: ProbeResponse::Timeout,
            });

            sleep(Duration::from_millis(self.config.timeout_ms)).await;

            // Probe sub-functions if enabled
            if self.config.sub_functions {
                for sub in 0x01..=0x03u8 {
                    let data = [0x02, service, sub, 0x00, 0x00, 0x00, 0x00, 0x00];
                    let frame = CanFrame {
                        timestamp_ns: canflow_types::timestamp::monotonic_ns(),
                        id: target_id,
                        dlc: 8,
                        data,
                        is_error: false,
                        is_remote: false,
                        interface: canflow_types::InterfaceId(0),
                    };

                    if tx.send(frame).await.is_err() {
                        return Err(CanFlowError::ChannelClosed);
                    }

                    results.push(ProbeResult {
                        service_id: service,
                        sub_function: Some(sub),
                        response: ProbeResponse::Timeout,
                    });

                    sleep(Duration::from_millis(self.config.timeout_ms)).await;
                }
            }
        }

        info!(probes = results.len(), "UDS probe completed");
        Ok(results)
    }
}
