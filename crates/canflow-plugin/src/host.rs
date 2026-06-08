use crate::abi::*;
use canflow_analysis::{Alert, Severity};
use canflow_types::{CanFlowError, CanFrame};
use libloading::{Library, Symbol};
use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

const MAX_ALERTS_PER_CALL: usize = 64;

pub struct LoadedPlugin {
    _lib: Library,
    vtable: *const PluginVtable,
    state: *mut std::ffi::c_void,
    name: String,
    path: PathBuf,
}

unsafe impl Send for LoadedPlugin {}

impl LoadedPlugin {
    pub fn load(path: &Path, config_json: &str) -> Result<Self, CanFlowError> {
        unsafe {
            let lib = Library::new(path).map_err(|e| CanFlowError::PluginLoad {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;

            let init_fn: Symbol<PluginInitFn> =
                lib.get(PLUGIN_INIT_SYMBOL).map_err(|e| CanFlowError::PluginLoad {
                    path: path.display().to_string(),
                    reason: format!("symbol not found: {}", e),
                })?;

            let vtable = init_fn();
            if vtable.is_null() {
                return Err(CanFlowError::PluginLoad {
                    path: path.display().to_string(),
                    reason: "init returned null".to_string(),
                });
            }

            if (*vtable).abi_version != CANFLOW_PLUGIN_ABI_VERSION {
                return Err(CanFlowError::PluginLoad {
                    path: path.display().to_string(),
                    reason: format!(
                        "ABI version mismatch: plugin={}, host={}",
                        (*vtable).abi_version,
                        CANFLOW_PLUGIN_ABI_VERSION
                    ),
                });
            }

            let name = if (*vtable).name.is_null() {
                "unknown".to_string()
            } else {
                CStr::from_ptr((*vtable).name).to_string_lossy().into_owned()
            };

            let config_c = CString::new(config_json).unwrap_or_default();
            let state = ((*vtable).create)(config_c.as_ptr());

            info!(plugin = %name, path = %path.display(), "plugin loaded");

            Ok(Self {
                _lib: lib,
                vtable,
                state,
                name,
                path: path.to_path_buf(),
            })
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn ingest(&mut self, frame: &CanFrame) -> Vec<Alert> {
        let plugin_frame = PluginFrame::from(frame);
        let mut out_alerts: Vec<PluginAlert> = Vec::with_capacity(MAX_ALERTS_PER_CALL);
        unsafe {
            out_alerts.set_len(MAX_ALERTS_PER_CALL);
        }

        let count = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            ((*self.vtable).ingest)(
                self.state,
                &plugin_frame,
                out_alerts.as_mut_ptr(),
                MAX_ALERTS_PER_CALL as u32,
            )
        }));

        match count {
            Ok(n) => self.convert_alerts(&out_alerts, n as usize),
            Err(_) => {
                error!(plugin = %self.name, "plugin panicked in ingest");
                Vec::new()
            }
        }
    }

    pub fn tick(&mut self) -> Vec<Alert> {
        let mut out_alerts: Vec<PluginAlert> = Vec::with_capacity(MAX_ALERTS_PER_CALL);
        unsafe {
            out_alerts.set_len(MAX_ALERTS_PER_CALL);
        }

        let count = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            ((*self.vtable).tick)(self.state, out_alerts.as_mut_ptr(), MAX_ALERTS_PER_CALL as u32)
        }));

        match count {
            Ok(n) => self.convert_alerts(&out_alerts, n as usize),
            Err(_) => {
                error!(plugin = %self.name, "plugin panicked in tick");
                Vec::new()
            }
        }
    }

    pub fn reset(&mut self) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            ((*self.vtable).reset)(self.state);
        }));
    }

    fn convert_alerts(&self, raw: &[PluginAlert], count: usize) -> Vec<Alert> {
        let mut alerts = Vec::new();
        for i in 0..count.min(MAX_ALERTS_PER_CALL) {
            let raw_alert = &raw[i];
            let severity = match raw_alert.severity {
                PluginSeverity::Info => Severity::Info,
                PluginSeverity::Warning => Severity::Warning,
                PluginSeverity::Critical => Severity::Critical,
            };

            let message = if raw_alert.message.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(raw_alert.message) }
                    .to_string_lossy()
                    .into_owned()
            };

            let details = if raw_alert.details_json.is_null() {
                serde_json::Value::Null
            } else {
                let json_str = unsafe { CStr::from_ptr(raw_alert.details_json) }
                    .to_string_lossy();
                serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null)
            };

            alerts.push(Alert {
                timestamp_ns: canflow_types::timestamp::monotonic_ns(),
                severity,
                analyzer: format!("plugin:{}", self.name),
                frame_id: None,
                message,
                details,
            });
        }
        alerts
    }
}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        if !self.state.is_null() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                ((*self.vtable).destroy)(self.state);
            }));
            self.state = std::ptr::null_mut();
        }
    }
}
