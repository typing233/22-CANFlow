use async_trait::async_trait;
use canflow_types::*;
use std::os::unix::io::{AsRawFd, RawFd};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;
use tracing::debug;

use crate::trait_def::CanAdapter;

const CAN_MTU: usize = 16;

#[repr(C)]
#[derive(Clone, Copy)]
struct CanSocketFrame {
    can_id: u32,
    can_dlc: u8,
    _pad: u8,
    _res0: u8,
    _res1: u8,
    data: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SockAddrCan {
    can_family: u16,
    ifindex: i32,
    rx_id: u32,
    tx_id: u32,
}

pub struct SocketCanAdapter {
    name: String,
    interface: String,
    fd: Option<AsyncFd<OwnedFd>>,
    filters: Vec<FrameFilter>,
    interface_id: InterfaceId,
    shutdown: bool,
}

struct OwnedFd(RawFd);

impl AsRawFd for OwnedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl Drop for OwnedFd {
    fn drop(&mut self) {
        unsafe { libc::close(self.0); }
    }
}

impl SocketCanAdapter {
    pub fn new(interface: &str, interface_id: InterfaceId) -> Result<Self> {
        Ok(Self {
            name: format!("socketcan:{}", interface),
            interface: interface.to_string(),
            fd: None,
            filters: Vec::new(),
            interface_id,
            shutdown: false,
        })
    }

    fn open_socket(interface: &str) -> Result<RawFd> {
        unsafe {
            let fd = libc::socket(libc::AF_CAN, libc::SOCK_RAW, libc::CAN_RAW);
            if fd < 0 {
                return Err(CanFlowError::Adapter {
                    interface: interface.to_string(),
                    message: format!("socket() failed: {}", std::io::Error::last_os_error()),
                });
            }

            let mut ifr: libc::ifreq = std::mem::zeroed();
            let name_bytes = interface.as_bytes();
            let copy_len = name_bytes.len().min(libc::IFNAMSIZ - 1);
            std::ptr::copy_nonoverlapping(
                name_bytes.as_ptr(),
                ifr.ifr_name.as_mut_ptr() as *mut u8,
                copy_len,
            );

            if libc::ioctl(fd, libc::SIOCGIFINDEX as _, &ifr) < 0 {
                libc::close(fd);
                return Err(CanFlowError::Adapter {
                    interface: interface.to_string(),
                    message: format!("ioctl(SIOCGIFINDEX) failed: {}", std::io::Error::last_os_error()),
                });
            }

            let addr = SockAddrCan {
                can_family: libc::AF_CAN as u16,
                ifindex: ifr.ifr_ifru.ifru_ifindex,
                rx_id: 0,
                tx_id: 0,
            };

            if libc::bind(
                fd,
                &addr as *const SockAddrCan as *const libc::sockaddr,
                std::mem::size_of::<SockAddrCan>() as u32,
            ) < 0
            {
                libc::close(fd);
                return Err(CanFlowError::Adapter {
                    interface: interface.to_string(),
                    message: format!("bind() failed: {}", std::io::Error::last_os_error()),
                });
            }

            // Enable timestamps
            let enable: i32 = 1;
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_TIMESTAMPNS,
                &enable as *const i32 as *const libc::c_void,
                std::mem::size_of::<i32>() as u32,
            );

            // Set non-blocking
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);

            Ok(fd)
        }
    }

    fn read_frame(fd: RawFd, interface_id: InterfaceId) -> Option<CanFrame> {
        let mut raw = CanSocketFrame {
            can_id: 0,
            can_dlc: 0,
            _pad: 0,
            _res0: 0,
            _res1: 0,
            data: [0; 8],
        };

        let n = unsafe {
            libc::read(fd, &mut raw as *mut CanSocketFrame as *mut libc::c_void, CAN_MTU)
        };

        if n < CAN_MTU as isize {
            return None;
        }

        let is_extended = raw.can_id & 0x80000000 != 0;
        let is_remote = raw.can_id & 0x40000000 != 0;
        let is_error = raw.can_id & 0x20000000 != 0;

        let id = if is_extended {
            CanId::extended(raw.can_id & 0x1FFFFFFF)
        } else {
            CanId::standard((raw.can_id & 0x7FF) as u16)
        };

        Some(CanFrame {
            timestamp_ns: timestamp::monotonic_ns(),
            id,
            dlc: raw.can_dlc.min(8),
            data: raw.data,
            is_error,
            is_remote,
            interface: interface_id,
        })
    }

    fn write_frame(fd: RawFd, frame: &CanFrame) -> Result<()> {
        let mut can_id = frame.id.raw_id();
        if frame.id.is_extended() {
            can_id |= 0x80000000;
        }
        if frame.is_remote {
            can_id |= 0x40000000;
        }

        let raw = CanSocketFrame {
            can_id,
            can_dlc: frame.dlc,
            _pad: 0,
            _res0: 0,
            _res1: 0,
            data: frame.data,
        };

        let n = unsafe {
            libc::write(fd, &raw as *const CanSocketFrame as *const libc::c_void, CAN_MTU)
        };

        if n < 0 {
            Err(CanFlowError::Io(std::io::Error::last_os_error()))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl CanAdapter for SocketCanAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run(&mut self, tx: mpsc::Sender<CanFrame>) -> Result<()> {
        let raw_fd = Self::open_socket(&self.interface)?;
        let owned = OwnedFd(raw_fd);
        let async_fd = AsyncFd::new(owned).map_err(|e| CanFlowError::Adapter {
            interface: self.interface.clone(),
            message: format!("AsyncFd creation failed: {}", e),
        })?;
        self.fd = Some(async_fd);

        debug!(interface = %self.interface, "SocketCAN adapter started");

        loop {
            if self.shutdown {
                return Ok(());
            }

            let fd_ref = self.fd.as_ref().unwrap();
            let mut guard = fd_ref.readable().await.map_err(|e| CanFlowError::Adapter {
                interface: self.interface.clone(),
                message: format!("readable() failed: {}", e),
            })?;

            loop {
                match Self::read_frame(fd_ref.as_raw_fd(), self.interface_id) {
                    Some(frame) => {
                        let passes = self.filters.is_empty()
                            || self.filters.iter().any(|f| f.matches(&frame));
                        if passes {
                            if tx.send(frame).await.is_err() {
                                return Err(CanFlowError::ChannelClosed);
                            }
                        }
                    }
                    None => {
                        guard.clear_ready();
                        break;
                    }
                }
            }
        }
    }

    async fn send(&mut self, frame: &CanFrame) -> Result<()> {
        if let Some(fd) = &self.fd {
            Self::write_frame(fd.as_raw_fd(), frame)
        } else {
            Err(CanFlowError::Adapter {
                interface: self.interface.clone(),
                message: "socket not open".to_string(),
            })
        }
    }

    fn set_filters(&mut self, filters: Vec<FrameFilter>) {
        self.filters = filters;
    }

    async fn shutdown(&mut self) {
        self.shutdown = true;
        self.fd.take();
    }
}
