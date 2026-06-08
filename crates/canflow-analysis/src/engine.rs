use crate::alert::Alert;
use crate::burst::BurstAnalyzer;
use crate::entropy::EntropyAnalyzer;
use crate::period::PeriodAnalyzer;
use crate::uds::UdsAnalyzer;
use canflow_types::{AnalysisConfig, CanFrame};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::debug;

pub struct AnalysisEngine {
    entropy: Option<EntropyAnalyzer>,
    period: Option<PeriodAnalyzer>,
    burst: Option<BurstAnalyzer>,
    uds: Option<UdsAnalyzer>,
    alert_tx: mpsc::Sender<Alert>,
}

impl AnalysisEngine {
    pub fn new(config: &AnalysisConfig, alert_tx: mpsc::Sender<Alert>) -> Self {
        let entropy = if config.enabled.contains(&"entropy".to_string()) {
            Some(EntropyAnalyzer::new(&config.entropy))
        } else {
            None
        };

        let period = if config.enabled.contains(&"period".to_string()) {
            Some(PeriodAnalyzer::new(&config.period))
        } else {
            None
        };

        let burst = if config.enabled.contains(&"burst".to_string()) {
            Some(BurstAnalyzer::new(&config.burst))
        } else {
            None
        };

        let uds = if config.enabled.contains(&"uds".to_string()) {
            Some(UdsAnalyzer::new())
        } else {
            None
        };

        Self {
            entropy,
            period,
            burst,
            uds,
            alert_tx,
        }
    }

    pub fn ingest_frame(&mut self, frame: &CanFrame) -> Vec<Alert> {
        let mut alerts = Vec::new();

        if let Some(ref mut a) = self.entropy {
            alerts.extend(a.ingest(frame));
        }
        if let Some(ref mut a) = self.period {
            alerts.extend(a.ingest(frame));
        }
        if let Some(ref mut a) = self.burst {
            alerts.extend(a.ingest(frame));
        }
        if let Some(ref mut a) = self.uds {
            alerts.extend(a.ingest(frame));
        }

        alerts
    }

    pub fn tick(&mut self) -> Vec<Alert> {
        let mut alerts = Vec::new();

        if let Some(ref mut a) = self.entropy {
            alerts.extend(a.tick());
        }
        if let Some(ref mut a) = self.period {
            alerts.extend(a.tick());
        }
        if let Some(ref mut a) = self.burst {
            alerts.extend(a.tick());
        }
        if let Some(ref mut a) = self.uds {
            alerts.extend(a.tick());
        }

        alerts
    }

    pub async fn run(&mut self, mut rx: broadcast::Receiver<Arc<CanFrame>>) {
        debug!("analysis engine started");
        let mut tick_interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(frame) => {
                            let alerts = self.ingest_frame(&frame);
                            for alert in alerts {
                                let _ = self.alert_tx.send(alert).await;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = tick_interval.tick() => {
                    let alerts = self.tick();
                    for alert in alerts {
                        let _ = self.alert_tx.send(alert).await;
                    }
                }
            }
        }
    }

    pub fn reset(&mut self) {
        if let Some(ref mut a) = self.entropy {
            a.reset();
        }
        if let Some(ref mut a) = self.period {
            a.reset();
        }
        if let Some(ref mut a) = self.burst {
            a.reset();
        }
        if let Some(ref mut a) = self.uds {
            a.reset();
        }
    }
}
