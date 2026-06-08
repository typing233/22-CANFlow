use crate::host::LoadedPlugin;
use canflow_analysis::Alert;
use canflow_types::{CanFlowError, CanFrame};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub struct PluginRegistry {
    plugins: HashMap<String, LoadedPlugin>,
    plugin_dir: PathBuf,
}

impl PluginRegistry {
    pub fn new(plugin_dir: impl AsRef<Path>) -> Self {
        Self {
            plugins: HashMap::new(),
            plugin_dir: plugin_dir.as_ref().to_path_buf(),
        }
    }

    pub fn load(&mut self, path: &Path, config_json: &str) -> Result<(), CanFlowError> {
        let plugin = LoadedPlugin::load(path, config_json)?;
        let name = plugin.name().to_string();
        self.plugins.insert(name, plugin);
        Ok(())
    }

    pub fn unload(&mut self, name: &str) -> bool {
        self.plugins.remove(name).is_some()
    }

    pub fn reload(&mut self, name: &str, config_json: &str) -> Result<(), CanFlowError> {
        let path = match self.plugins.get(name) {
            Some(p) => p.path().to_path_buf(),
            None => return Err(CanFlowError::PluginLoad {
                path: name.to_string(),
                reason: "plugin not found".to_string(),
            }),
        };

        // Load new instance first
        let new_plugin = LoadedPlugin::load(&path, config_json)?;
        // Then swap (old is dropped)
        self.plugins.insert(name.to_string(), new_plugin);
        info!(plugin = %name, "plugin reloaded");
        Ok(())
    }

    pub fn ingest_all(&mut self, frame: &CanFrame) -> Vec<Alert> {
        let mut alerts = Vec::new();
        for plugin in self.plugins.values_mut() {
            alerts.extend(plugin.ingest(frame));
        }
        alerts
    }

    pub fn tick_all(&mut self) -> Vec<Alert> {
        let mut alerts = Vec::new();
        for plugin in self.plugins.values_mut() {
            alerts.extend(plugin.tick());
        }
        alerts
    }

    pub fn list(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }

    pub fn plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }
}
