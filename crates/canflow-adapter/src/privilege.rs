use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeLevel {
    RawSocket,
    VirtualOnly,
    Unprivileged,
}

impl PrivilegeLevel {
    pub fn detect() -> Self {
        // Try creating a raw CAN socket
        let fd = unsafe { libc::socket(libc::AF_CAN, libc::SOCK_RAW, libc::CAN_RAW) };
        if fd >= 0 {
            unsafe { libc::close(fd); }
            info!("privilege check: CAP_NET_RAW available");
            return Self::RawSocket;
        }

        // Check if vcan module is available
        if std::path::Path::new("/sys/module/vcan").exists()
            || std::path::Path::new("/proc/net/can").exists()
        {
            warn!("no CAP_NET_RAW, falling back to virtual CAN only");
            return Self::VirtualOnly;
        }

        warn!("no CAN capabilities detected");
        Self::Unprivileged
    }

    pub fn can_use_real_can(&self) -> bool {
        matches!(self, Self::RawSocket)
    }

    pub fn can_use_vcan(&self) -> bool {
        matches!(self, Self::RawSocket | Self::VirtualOnly)
    }
}
