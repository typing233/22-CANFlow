use canflow_types::{CanFrame, CanId};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;

pub struct LiveStats {
    start_time: Instant,
    per_id_count: Arc<DashMap<u32, AtomicU64>>,
    window_frames: Arc<AtomicU64>,
    window_start: Arc<std::sync::Mutex<Instant>>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LiveStatsSnapshot {
    pub uptime_secs: f64,
    pub current_fps: f64,
    pub total_frames: u64,
    pub unique_ids: usize,
    pub top_ids: Vec<(u32, u64)>,
}

impl LiveStats {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            per_id_count: Arc::new(DashMap::new()),
            window_frames: Arc::new(AtomicU64::new(0)),
            window_start: Arc::new(std::sync::Mutex::new(Instant::now())),
        }
    }

    pub async fn run(&self, mut rx: broadcast::Receiver<Arc<CanFrame>>) {
        loop {
            match rx.recv().await {
                Ok(frame) => {
                    self.per_id_count
                        .entry(frame.id.raw_id())
                        .or_insert_with(|| AtomicU64::new(0))
                        .fetch_add(1, Ordering::Relaxed);
                    self.window_frames.fetch_add(1, Ordering::Relaxed);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }

    pub fn snapshot(&self) -> LiveStatsSnapshot {
        let uptime = self.start_time.elapsed().as_secs_f64();

        let window_frames = self.window_frames.swap(0, Ordering::Relaxed);
        let mut window_start = self.window_start.lock().unwrap();
        let window_elapsed = window_start.elapsed().as_secs_f64();
        *window_start = Instant::now();

        let current_fps = if window_elapsed > 0.0 {
            window_frames as f64 / window_elapsed
        } else {
            0.0
        };

        let mut total_frames = 0u64;
        let mut id_counts: Vec<(u32, u64)> = self
            .per_id_count
            .iter()
            .map(|entry| {
                let count = entry.value().load(Ordering::Relaxed);
                total_frames += count;
                (*entry.key(), count)
            })
            .collect();

        id_counts.sort_by(|a, b| b.1.cmp(&a.1));
        let top_ids: Vec<(u32, u64)> = id_counts.into_iter().take(10).collect();

        LiveStatsSnapshot {
            uptime_secs: uptime,
            current_fps,
            total_frames,
            unique_ids: self.per_id_count.len(),
            top_ids,
        }
    }
}
