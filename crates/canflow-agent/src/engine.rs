use canflow_types::{CanFlowError, CanFrame, AgentKind, AgentTaskConfig};
use std::path::Path;
use tokio::sync::mpsc;
use tracing::info;

use crate::builtins::fuzzer::{Fuzzer, FuzzConfig};
use crate::builtins::uds_probe::{UdsProbe, UdsProbeConfig};
use crate::builtins::state_walker::{StateWalker, StateWalkerConfig};
use crate::lua_runtime::LuaRuntime;
use crate::pipeline::Pipeline;
use crate::sandbox::SandboxConfig;
use crate::python_runtime::PythonRuntime;
use crate::process_runner::ProcessRunner;

pub struct AgentEngine {
    tx: mpsc::Sender<CanFrame>,
}

impl AgentEngine {
    pub fn new(tx: mpsc::Sender<CanFrame>) -> Self {
        Self { tx }
    }

    pub async fn run_script(&self, script: &Path) -> Result<String, CanFlowError> {
        let ext = script.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let sandbox = SandboxConfig::default();
        let mut runner = match ext {
            "lua" => ProcessRunner::lua(script, sandbox)?,
            "py" => ProcessRunner::python(script, sandbox)?,
            _ => return Err(CanFlowError::Config(
                format!("unsupported script extension: .{}", ext)
            )),
        };

        let frames = runner.run_to_completion().await?;
        for frame in &frames {
            if self.tx.send(frame.clone()).await.is_err() {
                return Err(CanFlowError::ChannelClosed);
            }
        }
        Ok(format!("{} agent produced {} frames", ext, frames.len()))
    }

    pub async fn run_task(&self, config: &AgentTaskConfig) -> Result<String, CanFlowError> {
        info!(name = %config.name, "running agent task");

        match &config.kind {
            AgentKind::Lua { script } => {
                let content = tokio::fs::read_to_string(script).await?;
                let runtime = LuaRuntime::new(&config.name, &content, true)?;
                let frames = runtime.execute()?;
                for frame in &frames {
                    if self.tx.send(frame.clone()).await.is_err() {
                        return Err(CanFlowError::ChannelClosed);
                    }
                }
                Ok(format!("lua agent produced {} frames", frames.len()))
            }
            AgentKind::Python { script } => {
                let sandbox = SandboxConfig::default();
                let mut runtime = PythonRuntime::new(
                    &config.name,
                    script.to_str().unwrap_or(""),
                    sandbox,
                );
                runtime.start().await?;
                Ok("python agent started".to_string())
            }
            AgentKind::Fuzz { target_ids, iterations } => {
                let fuzz_config = FuzzConfig {
                    target_ids: target_ids.clone(),
                    iterations: *iterations,
                    ..Default::default()
                };
                let fuzzer = Fuzzer::new(fuzz_config);
                let sent = fuzzer.run(self.tx.clone()).await?;
                Ok(format!("fuzzer sent {} frames", sent))
            }
            AgentKind::Replay { file } => {
                let replay_config = crate::builtins::replay::ReplayConfig {
                    file: file.clone(),
                    ..Default::default()
                };
                let task = crate::builtins::replay::ReplayTask::new(replay_config);
                let sent = task.run(self.tx.clone()).await?;
                Ok(format!("replay sent {} frames", sent))
            }
            AgentKind::UdsProbe { target_id, services } => {
                let probe_config = UdsProbeConfig {
                    target_id: *target_id,
                    services: services.clone(),
                    ..Default::default()
                };
                let probe = UdsProbe::new(probe_config);
                let results = probe.run(self.tx.clone()).await?;
                Ok(format!("UDS probe completed: {} probes", results.len()))
            }
            AgentKind::StateWalker { dbc_file: _ } => {
                let walker_config = StateWalkerConfig::default();
                let mut walker = StateWalker::new(walker_config);
                let transitions = walker.run(self.tx.clone()).await?;
                Ok(format!("state walker found {} transitions", transitions.len()))
            }
        }
    }

    pub async fn run_pipeline(&self, pipeline: &Pipeline) -> Result<Vec<String>, CanFlowError> {
        let order = pipeline.execution_order();
        let mut results = Vec::new();

        for idx in order {
            let stage = &pipeline.stages[idx];
            info!(stage = %stage.name, "executing pipeline stage");

            let result = match &stage.agent {
                crate::pipeline::StageAgent::Lua { script } => {
                    let sandbox = SandboxConfig::default();
                    let mut runner = ProcessRunner::lua(script, sandbox)?;
                    let frames = runner.run_to_completion().await?;
                    for frame in &frames {
                        let _ = self.tx.send(frame.clone()).await;
                    }
                    format!("stage '{}': {} frames", stage.name, frames.len())
                }
                crate::pipeline::StageAgent::Python { script } => {
                    let sandbox = SandboxConfig::default();
                    let mut runner = ProcessRunner::python(script, sandbox)?;
                    let frames = runner.run_to_completion().await?;
                    for frame in &frames {
                        let _ = self.tx.send(frame.clone()).await;
                    }
                    format!("stage '{}': {} frames", stage.name, frames.len())
                }
                crate::pipeline::StageAgent::Builtin { task } => {
                    format!("stage '{}': builtin '{}' executed", stage.name, task)
                }
            };

            results.push(result);
        }

        Ok(results)
    }
}
