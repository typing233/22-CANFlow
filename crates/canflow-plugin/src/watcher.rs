use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

pub struct PluginWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<PathBuf>,
}

impl PluginWatcher {
    pub fn new(watch_dir: &Path) -> std::io::Result<Self> {
        let (tx, rx) = mpsc::channel(32);

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        for path in event.paths {
                            if path.extension().map_or(false, |e| e == "so" || e == "dylib") {
                                let _ = tx.blocking_send(path);
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        watcher
            .watch(watch_dir, RecursiveMode::NonRecursive)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        info!(dir = %watch_dir.display(), "plugin watcher started");

        Ok(Self {
            _watcher: watcher,
            rx,
        })
    }

    pub async fn next_change(&mut self) -> Option<PathBuf> {
        self.rx.recv().await
    }
}
