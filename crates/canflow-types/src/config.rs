use crate::filter::FrameFilter;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub adapters: Vec<AdapterConfig>,
    pub bus: BusConfig,
    pub analysis: AnalysisConfig,
    #[serde(default)]
    pub agents: Vec<AgentTaskConfig>,
    pub logging: LogConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdapterConfig {
    pub name: String,
    pub kind: AdapterKind,
    #[serde(default)]
    pub filters: Vec<FrameFilter>,
    #[serde(default)]
    pub reconnect: ReconnectPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AdapterKind {
    SocketCan { interface: String },
    VirtualCan { interface: String },
    LogReplay { path: PathBuf, format: ReplayFormat, #[serde(default)] loop_forever: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReplayFormat {
    Candump,
    Asc,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReconnectPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_retries: 0,
            base_delay_ms: 100,
            max_delay_ms: 5000,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BusConfig {
    #[serde(default = "default_channel_capacity")]
    pub channel_capacity: usize,
    pub fault_injection: Option<FaultConfig>,
}

impl Default for BusConfig {
    fn default() -> Self {
        Self {
            channel_capacity: default_channel_capacity(),
            fault_injection: None,
        }
    }
}

fn default_channel_capacity() -> usize {
    16384
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaultConfig {
    #[serde(default)]
    pub drop_rate: f64,
    pub delay_ms: Option<u64>,
    #[serde(default)]
    pub corrupt_rate: f64,
    pub reorder_window: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
    #[serde(default)]
    pub entropy: EntropyConfig,
    #[serde(default)]
    pub period: PeriodConfig,
    #[serde(default)]
    pub burst: BurstConfig,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            enabled: vec![
                "entropy".into(),
                "period".into(),
                "burst".into(),
                "uds".into(),
            ],
            entropy: EntropyConfig::default(),
            period: PeriodConfig::default(),
            burst: BurstConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntropyConfig {
    pub window_size: usize,
    pub threshold: f64,
}

impl Default for EntropyConfig {
    fn default() -> Self {
        Self {
            window_size: 64,
            threshold: 6.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeriodConfig {
    pub jitter_threshold_pct: f64,
    pub learning_samples: usize,
}

impl Default for PeriodConfig {
    fn default() -> Self {
        Self {
            jitter_threshold_pct: 25.0,
            learning_samples: 100,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BurstConfig {
    pub rate_multiplier: f64,
    pub window_secs: f64,
}

impl Default for BurstConfig {
    fn default() -> Self {
        Self {
            rate_multiplier: 3.0,
            window_secs: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentTaskConfig {
    pub name: String,
    pub kind: AgentKind,
    #[serde(default = "default_agent_config")]
    pub config: toml::Value,
}

fn default_agent_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentKind {
    Lua { script: PathBuf },
    Python { script: PathBuf },
    Fuzz { target_ids: Vec<u32>, iterations: u64 },
    Replay { file: PathBuf },
    UdsProbe { target_id: u32, services: Vec<u8> },
    StateWalker { dbc_file: Option<PathBuf> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogConfig {
    #[serde(default = "default_log_format")]
    pub format: String,
    #[serde(default = "default_log_dir")]
    pub directory: PathBuf,
    #[serde(default = "default_rotation")]
    pub rotation: String,
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            format: default_log_format(),
            directory: default_log_dir(),
            rotation: default_rotation(),
            max_files: default_max_files(),
        }
    }
}

fn default_log_format() -> String { "json".into() }
fn default_log_dir() -> PathBuf { PathBuf::from("./logs") }
fn default_rotation() -> String { "hourly".into() }
fn default_max_files() -> u32 { 168 }
