use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    pub timestamp_ns: u64,
    pub severity: Severity,
    pub analyzer: String,
    pub frame_id: Option<u32>,
    pub message: String,
    pub details: serde_json::Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Critical => write!(f, "CRIT"),
        }
    }
}

impl Alert {
    pub fn info(analyzer: &str, frame_id: Option<u32>, message: impl Into<String>) -> Self {
        Self {
            timestamp_ns: canflow_types::timestamp::monotonic_ns(),
            severity: Severity::Info,
            analyzer: analyzer.to_string(),
            frame_id,
            message: message.into(),
            details: serde_json::Value::Null,
        }
    }

    pub fn warning(analyzer: &str, frame_id: Option<u32>, message: impl Into<String>) -> Self {
        Self {
            timestamp_ns: canflow_types::timestamp::monotonic_ns(),
            severity: Severity::Warning,
            analyzer: analyzer.to_string(),
            frame_id,
            message: message.into(),
            details: serde_json::Value::Null,
        }
    }

    pub fn critical(analyzer: &str, frame_id: Option<u32>, message: impl Into<String>) -> Self {
        Self {
            timestamp_ns: canflow_types::timestamp::monotonic_ns(),
            severity: Severity::Critical,
            analyzer: analyzer.to_string(),
            frame_id,
            message: message.into(),
            details: serde_json::Value::Null,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }
}
