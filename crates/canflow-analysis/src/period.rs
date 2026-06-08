use crate::alert::Alert;
use canflow_types::{CanFrame, PeriodConfig};
use std::collections::HashMap;

struct PeriodTracker {
    last_timestamp_ns: u64,
    learned_period_ns: Option<u64>,
    samples: Vec<u64>,
    learning_samples: usize,
}

impl PeriodTracker {
    fn new(learning_samples: usize) -> Self {
        Self {
            last_timestamp_ns: 0,
            learned_period_ns: None,
            samples: Vec::new(),
            learning_samples,
        }
    }

    fn record(&mut self, timestamp_ns: u64) -> Option<f64> {
        if self.last_timestamp_ns == 0 {
            self.last_timestamp_ns = timestamp_ns;
            return None;
        }

        let delta = timestamp_ns.saturating_sub(self.last_timestamp_ns);
        self.last_timestamp_ns = timestamp_ns;

        if delta == 0 {
            return None;
        }

        if self.learned_period_ns.is_none() {
            self.samples.push(delta);
            if self.samples.len() >= self.learning_samples {
                let sum: u64 = self.samples.iter().sum();
                self.learned_period_ns = Some(sum / self.samples.len() as u64);
                self.samples.clear();
            }
            return None;
        }

        let expected = self.learned_period_ns.unwrap() as f64;
        let deviation_pct = ((delta as f64 - expected) / expected).abs() * 100.0;
        Some(deviation_pct)
    }
}

pub struct PeriodAnalyzer {
    trackers: HashMap<u32, PeriodTracker>,
    jitter_threshold_pct: f64,
    learning_samples: usize,
}

impl PeriodAnalyzer {
    pub fn new(config: &PeriodConfig) -> Self {
        Self {
            trackers: HashMap::new(),
            jitter_threshold_pct: config.jitter_threshold_pct,
            learning_samples: config.learning_samples,
        }
    }

    pub fn name(&self) -> &str {
        "period"
    }

    pub fn ingest(&mut self, frame: &CanFrame) -> Vec<Alert> {
        let id = frame.id.raw_id();
        let tracker = self.trackers
            .entry(id)
            .or_insert_with(|| PeriodTracker::new(self.learning_samples));

        let mut alerts = Vec::new();
        if let Some(deviation_pct) = tracker.record(frame.timestamp_ns) {
            if deviation_pct > self.jitter_threshold_pct {
                let expected_ns = tracker.learned_period_ns.unwrap_or(0);
                alerts.push(
                    Alert::warning("period", Some(id), format!(
                        "period drift {:.1}% (expected period: {:.2}ms)",
                        deviation_pct,
                        expected_ns as f64 / 1_000_000.0
                    ))
                    .with_details(serde_json::json!({
                        "deviation_pct": deviation_pct,
                        "expected_period_ns": expected_ns,
                        "threshold_pct": self.jitter_threshold_pct
                    })),
                );
            }
        }

        alerts
    }

    pub fn tick(&mut self) -> Vec<Alert> {
        Vec::new()
    }

    pub fn reset(&mut self) {
        self.trackers.clear();
    }
}
