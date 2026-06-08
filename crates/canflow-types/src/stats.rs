use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct BusStats {
    pub frames_received: AtomicU64,
    pub frames_forwarded: AtomicU64,
    pub frames_dropped: AtomicU64,
    pub bytes_total: AtomicU64,
    pub errors: AtomicU64,
    pub reconnects: AtomicU64,
}

impl BusStats {
    pub fn record_frame(&self, dlc: u8) {
        self.frames_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_total.fetch_add(dlc as u64, Ordering::Relaxed);
    }

    pub fn record_forward(&self) {
        self.frames_forwarded.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_drop(&self, count: u64) {
        self.frames_dropped.fetch_add(count, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_reconnect(&self) {
        self.reconnects.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            frames_received: self.frames_received.load(Ordering::Relaxed),
            frames_forwarded: self.frames_forwarded.load(Ordering::Relaxed),
            frames_dropped: self.frames_dropped.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            reconnects: self.reconnects.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StatsSnapshot {
    pub frames_received: u64,
    pub frames_forwarded: u64,
    pub frames_dropped: u64,
    pub bytes_total: u64,
    pub errors: u64,
    pub reconnects: u64,
}

impl StatsSnapshot {
    pub fn loss_rate(&self) -> f64 {
        if self.frames_received == 0 {
            0.0
        } else {
            self.frames_dropped as f64 / self.frames_received as f64
        }
    }

    pub fn throughput_bps(&self, elapsed_secs: f64) -> f64 {
        if elapsed_secs <= 0.0 {
            0.0
        } else {
            (self.bytes_total as f64 * 8.0) / elapsed_secs
        }
    }
}
