use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CanId(u32);

impl CanId {
    const EXTENDED_FLAG: u32 = 1 << 31;
    const STANDARD_MASK: u32 = 0x7FF;
    const EXTENDED_MASK: u32 = 0x1FFFFFFF;

    pub fn standard(id: u16) -> Self {
        Self((id as u32) & Self::STANDARD_MASK)
    }

    pub fn extended(id: u32) -> Self {
        Self((id & Self::EXTENDED_MASK) | Self::EXTENDED_FLAG)
    }

    pub fn raw_id(self) -> u32 {
        self.0 & Self::EXTENDED_MASK
    }

    pub fn is_extended(self) -> bool {
        self.0 & Self::EXTENDED_FLAG != 0
    }

    pub fn raw_with_flags(self) -> u32 {
        self.0
    }
}

impl fmt::Display for CanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_extended() {
            write!(f, "{:08X}", self.raw_id())
        } else {
            write!(f, "{:03X}", self.raw_id())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InterfaceId(pub u16);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanFrame {
    pub timestamp_ns: u64,
    pub id: CanId,
    pub dlc: u8,
    pub data: [u8; 8],
    pub is_error: bool,
    pub is_remote: bool,
    pub interface: InterfaceId,
}

impl CanFrame {
    pub fn new(id: CanId, data: &[u8]) -> Self {
        let mut frame_data = [0u8; 8];
        let len = data.len().min(8);
        frame_data[..len].copy_from_slice(&data[..len]);
        Self {
            timestamp_ns: 0,
            id,
            dlc: len as u8,
            data: frame_data,
            is_error: false,
            is_remote: false,
            interface: InterfaceId(0),
        }
    }

    pub fn payload(&self) -> &[u8] {
        &self.data[..self.dlc as usize]
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp_ns = ts;
        self
    }

    pub fn with_interface(mut self, iface: InterfaceId) -> Self {
        self.interface = iface;
        self
    }
}

impl fmt::Display for CanFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.id, self.dlc)?;
        for i in 0..self.dlc as usize {
            write!(f, " {:02X}", self.data[i])?;
        }
        Ok(())
    }
}
