use nix::sys::resource::{setrlimit, Resource};
use std::io;

#[derive(Clone, Debug)]
pub struct ResourceLimits {
    pub cpu_time_secs: u64,
    pub memory_bytes: u64,
    pub max_fds: u64,
    pub max_pids: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_time_secs: 60,
            memory_bytes: 256 * 1024 * 1024,
            max_fds: 64,
            max_pids: 10,
        }
    }
}

impl ResourceLimits {
    pub fn apply(&self) -> io::Result<()> {
        setrlimit(Resource::RLIMIT_CPU, self.cpu_time_secs, self.cpu_time_secs)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        setrlimit(Resource::RLIMIT_AS, self.memory_bytes, self.memory_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        setrlimit(Resource::RLIMIT_NOFILE, self.max_fds, self.max_fds)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        setrlimit(Resource::RLIMIT_NPROC, self.max_pids, self.max_pids)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(())
    }
}
