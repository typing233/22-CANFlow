use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "canflow", version, about = "Production-grade CAN communication & security analysis platform")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Configuration file path
    #[arg(short, long, global = true, default_value = "config/default.toml")]
    pub config: PathBuf,

    /// Output format: json, table, or tui
    #[arg(short, long, global = true, default_value = "tui")]
    pub format: OutputFormat,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum OutputFormat {
    Tui,
    Json,
    Table,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Capture CAN frames from interfaces
    Capture {
        /// CAN interface name (e.g., can0, vcan0)
        #[arg(short, long)]
        interface: Option<String>,

        /// Log file to replay
        #[arg(short, long)]
        replay: Option<PathBuf>,

        /// Enable recording to file
        #[arg(short = 'R', long)]
        record: Option<PathBuf>,

        /// Enable all analyzers
        #[arg(short, long)]
        analyze: bool,
    },

    /// Run security analysis on captured traffic
    Analyze {
        /// Input file (recorded session)
        #[arg(short, long)]
        input: PathBuf,

        /// Analyzers to enable
        #[arg(short, long, value_delimiter = ',')]
        modules: Vec<String>,
    },

    /// Replay a captured log file
    Replay {
        /// Log file path
        file: PathBuf,

        /// Replay format (candump, asc)
        #[arg(short = 'F', long, default_value = "candump")]
        format: String,

        /// Speed multiplier
        #[arg(short, long, default_value = "1.0")]
        speed: f64,

        /// Output interface
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Run fuzzing tasks
    Fuzz {
        /// Task configuration file
        #[arg(short, long)]
        task: Option<PathBuf>,

        /// Target CAN IDs
        #[arg(short = 'I', long, value_delimiter = ',')]
        ids: Vec<String>,

        /// Number of iterations
        #[arg(short = 'n', long, default_value = "1000")]
        iterations: u64,

        /// Output interface
        #[arg(short = 'o', long)]
        interface: Option<String>,
    },

    /// Run agent scripts
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },

    /// Manage analysis plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Show system status and privilege level
    Status,
}

#[derive(Subcommand)]
pub enum AgentAction {
    /// Run a Lua or Python agent script
    Run {
        /// Script file path
        script: PathBuf,
        /// Output interface
        #[arg(short, long)]
        interface: Option<String>,
    },
    /// Run a pipeline definition
    Pipeline {
        /// Pipeline config file (TOML)
        config: PathBuf,
    },
    /// Run UDS probe
    Probe {
        /// Target ECU CAN ID
        #[arg(short, long, default_value = "0x7DF")]
        target: String,
        /// Output interface
        #[arg(short, long)]
        interface: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PluginAction {
    /// List loaded plugins
    List,
    /// Load a plugin
    Load {
        /// Path to .so plugin file
        path: PathBuf,
    },
    /// Reload a plugin by name
    Reload { name: String },
}
