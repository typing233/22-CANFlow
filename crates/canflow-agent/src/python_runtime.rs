use canflow_types::{CanFlowError, CanFrame};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

use crate::sandbox::{SandboxConfig, apply_resource_limits, apply_seccomp_filter, apply_landlock};

pub struct PythonRuntime {
    name: String,
    script_path: String,
    child: Option<Child>,
    sandbox_config: SandboxConfig,
    timeout_secs: u64,
}

impl PythonRuntime {
    pub fn new(name: &str, script_path: &str, sandbox_config: SandboxConfig) -> Self {
        Self {
            name: name.to_string(),
            script_path: script_path.to_string(),
            child: None,
            sandbox_config,
            timeout_secs: 60,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn start(&mut self) -> Result<(), CanFlowError> {
        let mut cmd = Command::new("python3");
        cmd.arg("-u") // unbuffered
            .arg(&self.script_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply pre-exec sandbox
        let sandbox_config = self.sandbox_config.clone();
        unsafe {
            cmd.pre_exec(move || {
                apply_resource_limits(&sandbox_config)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                apply_seccomp_filter()?;
                apply_landlock(&sandbox_config.allow_filesystem)?;
                Ok(())
            });
        }

        let child = cmd.spawn().map_err(|e| CanFlowError::Adapter {
            interface: self.name.clone(),
            message: format!("failed to spawn python: {}", e),
        })?;

        self.child = Some(child);
        debug!(name = %self.name, "python agent started");
        Ok(())
    }

    pub async fn send_frame(&mut self, frame: &CanFrame) -> Result<Vec<String>, CanFlowError> {
        let child = self.child.as_mut().ok_or_else(|| CanFlowError::Config(
            "python process not started".to_string()
        ))?;

        let stdin = child.stdin.as_mut().ok_or_else(|| CanFlowError::Config(
            "stdin not available".to_string()
        ))?;

        let json = serde_json::to_string(frame).unwrap_or_default();
        stdin.write_all(json.as_bytes()).await.map_err(|e| CanFlowError::Io(e))?;
        stdin.write_all(b"\n").await.map_err(|e| CanFlowError::Io(e))?;

        // Read response lines
        let stdout = child.stdout.as_mut().ok_or_else(|| CanFlowError::Config(
            "stdout not available".to_string()
        ))?;

        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let mut results = Vec::new();

        match timeout(Duration::from_secs(5), reader.read_line(&mut line)).await {
            Ok(Ok(0)) => {}
            Ok(Ok(_)) => {
                results.push(line.trim().to_string());
            }
            Ok(Err(e)) => {
                warn!(name = %self.name, error = %e, "read error from python");
            }
            Err(_) => {
                warn!(name = %self.name, "timeout reading from python");
            }
        }

        Ok(results)
    }

    pub async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
    }
}

impl Drop for PythonRuntime {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}
