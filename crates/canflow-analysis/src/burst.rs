use crate::alert::Alert;
use canflow_types::{CanFrame, BurstConfig};
use std::collections::HashMap;

struct BurstTracker {
    timestamps: Vec<u64>,
    learned_rate: Option<f64>,
    window_ns: u64,
}

impl BurstTracker {
    fn new(window_secs: f64) -> Self {
        Self {
            timestamps: Vec::new(),
            learned_rate: None,
            window_ns: (window_secs * 1_000_000_000.0) as u64,
        }
    }

    fn record(&mut self, timestamp_ns: u64) -> f64 {
        self.timestamps.push(timestamp_ns);

        // Evict old timestamps outside window
        let cutoff = timestamp_ns.saturating_sub(self.window_ns);
        self.timestamps.retain(|&t| t >= cutoff);

        let current_rate = self.timestamps.len() as f64;

        if self.learned_rate.is_none() && self.timestamps.len() > 10 {
            self.learned_rate = Some(current_rate);
        } else if let Some(baseline) = self.learned_rate {
            // Exponential moving average for baseline
            self.learned_rate = Some(baseline * 0.99 + current_rate * 0.01);
        }

        current_rate
    }

    fn baseline_rate(&self) -> f64 {
        self.learned_rate.unwrap_or(0.0)
    }
}

pub struct BurstAnalyzer {
    trackers: HashMap<u32, BurstTracker>,
    rate_multiplier: f64,
    window_secs: f64,
}

impl BurstAnalyzer {
    pub fn new(config: &BurstConfig) -> Self {
        Self {
            trackers: HashMap::new(),
            rate_multiplier: config.rate_multiplier,
            window_secs: config.window_secs,
        }
    }

    pub fn name(&self) -> &str {
        "burst"
    }

    pub fn ingest(&mut self, frame: &CanFrame) -> Vec<Alert> {
        let id = frame.id.raw_id();
        let tracker = self.trackers
            .entry(id)
            .or_insert_with(|| BurstTracker::new(self.window_secs));

        let current_rate = tracker.record(frame.timestamp_ns);
        let baseline = tracker.baseline_rate();

        let mut alerts = Vec::new();
        if baseline > 0.0 && current_rate > baseline * self.rate_multiplier {
            alerts.push(
                Alert::warning("burst", Some(id), format!(
                    "burst detected: {:.0} fps vs baseline {:.0} fps ({:.1}x)",
                    current_rate, baseline, current_rate / baseline
                ))
                .with_details(serde_json::json!({
                    "current_rate": current_rate,
                    "baseline_rate": baseline,
                    "multiplier": current_rate / baseline
                })),
            );
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
