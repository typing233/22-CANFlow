use thiserror::Error;

#[derive(Error, Debug)]
pub enum CanFlowError {
    #[error("adapter error on {interface}: {message}")]
    Adapter { interface: String, message: String },

    #[error("bus overrun: {dropped} frames dropped")]
    BusOverrun { dropped: u64 },

    #[error("filter compilation failed: {0}")]
    FilterCompile(String),

    #[error("plugin load failed: {path}: {reason}")]
    PluginLoad { path: String, reason: String },

    #[error("sandbox violation: {0}")]
    SandboxViolation(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("channel closed")]
    ChannelClosed,

    #[error("shutdown")]
    Shutdown,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, CanFlowError>;
