use arc_swap::ArcSwap;
use canflow_types::*;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, watch};
use tracing::{debug, trace, warn};

pub struct FrameBus {
    ingest_tx: mpsc::Sender<CanFrame>,
    ingest_rx: Option<mpsc::Receiver<CanFrame>>,
    broadcast_tx: broadcast::Sender<Arc<CanFrame>>,
    stats: Arc<BusStats>,
    fault: Arc<ArcSwap<Option<FaultConfig>>>,
    capacity: usize,
}

impl FrameBus {
    pub fn new(capacity: usize) -> Self {
        let (ingest_tx, ingest_rx) = mpsc::channel(capacity);
        let (broadcast_tx, _) = broadcast::channel(capacity);
        Self {
            ingest_tx,
            ingest_rx: Some(ingest_rx),
            broadcast_tx,
            stats: Arc::new(BusStats::default()),
            fault: Arc::new(ArcSwap::new(Arc::new(None))),
            capacity,
        }
    }

    pub fn ingest_sender(&self) -> mpsc::Sender<CanFrame> {
        self.ingest_tx.clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<CanFrame>> {
        self.broadcast_tx.subscribe()
    }

    pub fn stats(&self) -> Arc<BusStats> {
        self.stats.clone()
    }

    pub fn set_fault_config(&self, config: Option<FaultConfig>) {
        self.fault.store(Arc::new(config));
    }

    pub fn broadcast_sender(&self) -> broadcast::Sender<Arc<CanFrame>> {
        self.broadcast_tx.clone()
    }

    pub async fn run(&mut self, mut shutdown: watch::Receiver<bool>) {
        let mut rx = self.ingest_rx.take().expect("bus already running");
        debug!(capacity = self.capacity, "frame bus started");

        loop {
            tokio::select! {
                biased;
                _ = shutdown.changed() => {
                    debug!("frame bus shutdown");
                    break;
                }
                Some(frame) = rx.recv() => {
                    self.stats.record_frame(frame.dlc);

                    if let Some(frame) = self.apply_fault(frame) {
                        self.stats.record_forward();
                        let shared = Arc::new(frame);
                        let _ = self.broadcast_tx.send(shared);
                    }
                }
            }
        }
    }

    fn apply_fault(&self, mut frame: CanFrame) -> Option<CanFrame> {
        let fault_guard = self.fault.load();
        let config = match fault_guard.as_ref() {
            Some(c) => c,
            None => return Some(frame),
        };

        if config.drop_rate > 0.0 && rand::random::<f64>() < config.drop_rate {
            self.stats.record_drop(1);
            return None;
        }

        if config.corrupt_rate > 0.0 && rand::random::<f64>() < config.corrupt_rate {
            let idx = (rand::random::<u8>() % frame.dlc.max(1)) as usize;
            if idx < 8 {
                frame.data[idx] ^= rand::random::<u8>();
            }
        }

        Some(frame)
    }
}
