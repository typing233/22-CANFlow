use crate::alert::Alert;
use crate::window::SlidingWindow;
use canflow_types::{CanFrame, EntropyConfig};
use std::collections::HashMap;

pub struct EntropyAnalyzer {
    windows: HashMap<u32, SlidingWindow<[u8; 8]>>,
    window_size: usize,
    threshold: f64,
    baseline: HashMap<u32, f64>,
    learning: bool,
}

impl EntropyAnalyzer {
    pub fn new(config: &EntropyConfig) -> Self {
        Self {
            windows: HashMap::new(),
            window_size: config.window_size,
            threshold: config.threshold,
            baseline: HashMap::new(),
            learning: true,
        }
    }

    pub fn name(&self) -> &str {
        "entropy"
    }

    pub fn ingest(&mut self, frame: &CanFrame) -> Vec<Alert> {
        let id = frame.id.raw_id();
        let window = self.windows
            .entry(id)
            .or_insert_with(|| SlidingWindow::new(self.window_size));

        window.push(frame.data);

        if !window.is_full() {
            return Vec::new();
        }

        let entropy = self.compute_entropy(id);

        if self.learning {
            self.baseline.insert(id, entropy);
            return Vec::new();
        }

        let mut alerts = Vec::new();
        if entropy > self.threshold {
            let baseline = self.baseline.get(&id).copied().unwrap_or(0.0);
            let deviation = entropy - baseline;

            if deviation > 1.0 {
                alerts.push(
                    Alert::warning("entropy", Some(id), format!(
                        "high entropy {:.2} bits (baseline: {:.2}, threshold: {:.2})",
                        entropy, baseline, self.threshold
                    ))
                    .with_details(serde_json::json!({
                        "entropy": entropy,
                        "baseline": baseline,
                        "deviation": deviation
                    })),
                );
            }
        }

        alerts
    }

    pub fn tick(&mut self) -> Vec<Alert> {
        Vec::new()
    }

    pub fn finish_learning(&mut self) {
        self.learning = false;
    }

    pub fn reset(&mut self) {
        self.windows.clear();
        self.baseline.clear();
        self.learning = true;
    }

    fn compute_entropy(&self, id: u32) -> f64 {
        let window = match self.windows.get(&id) {
            Some(w) => w,
            None => return 0.0,
        };

        let mut byte_counts = [0u64; 256];
        let mut total_bytes = 0u64;

        for payload in window.iter() {
            for &byte in payload.iter() {
                byte_counts[byte as usize] += 1;
                total_bytes += 1;
            }
        }

        if total_bytes == 0 {
            return 0.0;
        }

        let mut entropy = 0.0f64;
        for &count in &byte_counts {
            if count > 0 {
                let p = count as f64 / total_bytes as f64;
                entropy -= p * p.log2();
            }
        }

        entropy
    }
}
