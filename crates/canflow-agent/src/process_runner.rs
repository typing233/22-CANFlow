use canflow_types::{CanFlowError, CanFrame};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};
use tracing::{debug, warn};

use crate::sandbox::{SandboxConfig, apply_resource_limits, apply_seccomp_filter, apply_landlock};

pub struct ProcessRunner {
    child: Child,
    timeout_secs: u64,
}

impl ProcessRunner {
    pub fn lua(script: &Path, sandbox: SandboxConfig) -> Result<Self, CanFlowError> {
        let script_path = script.to_str().unwrap_or("").to_string();
        let wrapper = lua_wrapper_code(&script_path);

        let mut cmd = Command::new("lua5.4");
        cmd.arg("-e")
            .arg(&wrapper)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let sandbox_clone = sandbox.clone();
        unsafe {
            cmd.pre_exec(move || {
                apply_resource_limits(&sandbox_clone)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                apply_seccomp_filter()?;
                apply_landlock(&sandbox_clone.allow_filesystem)?;
                Ok(())
            });
        }

        let child = cmd.spawn().map_err(|e| CanFlowError::Adapter {
            interface: "lua".to_string(),
            message: format!("failed to spawn lua5.4: {}", e),
        })?;

        Ok(Self { child, timeout_secs: 60 })
    }

    pub fn python(script: &Path, sandbox: SandboxConfig) -> Result<Self, CanFlowError> {
        let script_path = script.to_str().unwrap_or("").to_string();

        let mut cmd = Command::new("python3");
        cmd.arg("-u")
            .arg(&script_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let sandbox_clone = sandbox.clone();
        unsafe {
            cmd.pre_exec(move || {
                apply_resource_limits(&sandbox_clone)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                apply_seccomp_filter()?;
                apply_landlock(&sandbox_clone.allow_filesystem)?;
                Ok(())
            });
        }

        let child = cmd.spawn().map_err(|e| CanFlowError::Adapter {
            interface: "python".to_string(),
            message: format!("failed to spawn python3: {}", e),
        })?;

        Ok(Self { child, timeout_secs: 60 })
    }

    pub async fn run_to_completion(&mut self) -> Result<Vec<CanFrame>, CanFlowError> {
        let stdout = self.child.stdout.take().ok_or_else(|| {
            CanFlowError::Config("stdout not captured".to_string())
        })?;

        let mut reader = BufReader::new(stdout);
        let mut frames = Vec::new();

        let result = timeout(Duration::from_secs(self.timeout_secs), async {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<CanFrame>(trimmed) {
                            Ok(frame) => frames.push(frame),
                            Err(_) => {
                                debug!(line = trimmed, "non-frame output from script");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "read error from child process");
                        break;
                    }
                }
            }
        }).await;

        if result.is_err() {
            warn!("script timed out after {}s, killing", self.timeout_secs);
            let _ = self.child.kill().await;
        }

        let _ = self.child.wait().await;
        Ok(frames)
    }
}

impl Drop for ProcessRunner {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn lua_wrapper_code(script_path: &str) -> String {
    format!(r#"
local json = {{}}
function json.encode_frame(id, is_ext, dlc, data, ts)
    local hex_data = ""
    for i = 1, #data do
        hex_data = hex_data .. string.format("%02x", string.byte(data, i))
    end
    local ext_flag = "false"
    if is_ext then ext_flag = "true" end
    io.write(string.format(
        '{{"id":{{"raw_id":%d,"is_extended":%s}},"dlc":%d,"data":[%s],"timestamp_ns":%d}}\n',
        id, ext_flag, dlc,
        table.concat({{string.byte(data, 1, #data)}}, ","),
        ts or 0
    ))
    io.flush()
end

-- Provide can_frame helper
function can_frame(id, data)
    local dlc = #data
    local is_ext = id > 0x7FF
    json.encode_frame(id, is_ext, dlc, string.char(table.unpack(data)), 0)
end

-- Provide random_bytes helper
function random_bytes(n)
    local t = {{}}
    for i = 1, math.min(n, 8) do
        t[i] = math.random(0, 255)
    end
    return t
end

math.randomseed(os.time())

-- Remove dangerous globals
os.execute = nil
os.exit = nil
os.remove = nil
os.rename = nil
io.open = nil
io.popen = nil
loadfile = nil
dofile = nil

-- Execute the user script
dofile("{}")
"#, script_path.replace('\\', "\\\\").replace('"', "\\\""))
}
