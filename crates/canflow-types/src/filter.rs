use crate::frame::{CanFrame, CanId};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FrameFilter {
    IdExact(CanId),
    IdMask { id: u32, mask: u32 },
    IdRange { start: u32, end: u32 },
    PayloadHex(String),
    Not(Box<FrameFilter>),
    And(Vec<FrameFilter>),
    Or(Vec<FrameFilter>),
    Any,
}

impl FrameFilter {
    pub fn matches(&self, frame: &CanFrame) -> bool {
        match self {
            Self::IdExact(id) => frame.id == *id,
            Self::IdMask { id, mask } => (frame.id.raw_id() & mask) == (id & mask),
            Self::IdRange { start, end } => {
                let raw = frame.id.raw_id();
                raw >= *start && raw <= *end
            }
            Self::PayloadHex(pattern) => {
                let hex: String = frame.payload().iter().map(|b| format!("{:02X}", b)).collect();
                Regex::new(pattern)
                    .map(|re| re.is_match(&hex))
                    .unwrap_or(false)
            }
            Self::Not(inner) => !inner.matches(frame),
            Self::And(filters) => filters.iter().all(|f| f.matches(frame)),
            Self::Or(filters) => filters.iter().any(|f| f.matches(frame)),
            Self::Any => true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CompiledFilter {
    filter: FrameFilter,
    payload_regex: Option<Regex>,
}

impl CompiledFilter {
    pub fn compile(filter: FrameFilter) -> Result<Self, String> {
        let payload_regex = match &filter {
            FrameFilter::PayloadHex(pattern) => Some(
                Regex::new(pattern).map_err(|e| format!("invalid regex: {}", e))?,
            ),
            _ => None,
        };
        Ok(Self {
            filter,
            payload_regex,
        })
    }

    pub fn matches(&self, frame: &CanFrame) -> bool {
        if let Some(re) = &self.payload_regex {
            let hex: String = frame.payload().iter().map(|b| format!("{:02X}", b)).collect();
            re.is_match(&hex)
        } else {
            self.filter.matches(frame)
        }
    }
}
