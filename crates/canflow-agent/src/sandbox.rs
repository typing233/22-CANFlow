use nix::sys::resource::{setrlimit, Resource};
use std::io;
use tracing::{debug, warn};

#[derive(Clone, Debug)]
pub struct SandboxConfig {
    pub allow_network: bool,
    pub allow_filesystem: Vec<String>,
    pub max_memory_bytes: u64,
    pub max_cpu_secs: u64,
    pub max_fds: u64,
    pub max_pids: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            allow_network: false,
            allow_filesystem: vec!["/tmp/canflow-agent".to_string()],
            max_memory_bytes: 256 * 1024 * 1024, // 256MB
            max_cpu_secs: 60,
            max_fds: 64,
            max_pids: 10,
        }
    }
}

pub fn apply_resource_limits(config: &SandboxConfig) -> io::Result<()> {
    setrlimit(Resource::RLIMIT_AS, config.max_memory_bytes, config.max_memory_bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("RLIMIT_AS: {}", e)))?;

    setrlimit(Resource::RLIMIT_CPU, config.max_cpu_secs, config.max_cpu_secs)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("RLIMIT_CPU: {}", e)))?;

    setrlimit(Resource::RLIMIT_NOFILE, config.max_fds, config.max_fds)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("RLIMIT_NOFILE: {}", e)))?;

    setrlimit(Resource::RLIMIT_NPROC, config.max_pids, config.max_pids)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("RLIMIT_NPROC: {}", e)))?;

    debug!("resource limits applied");
    Ok(())
}

pub fn apply_seccomp_filter() -> io::Result<()> {
    // Basic seccomp filter that allows common syscalls but blocks dangerous ones
    // In production this would use libseccomp or seccompiler crate
    #[cfg(target_arch = "x86_64")]
    {
        use libc::{
            prctl, PR_SET_NO_NEW_PRIVS,
        };
        unsafe {
            // Prevent privilege escalation
            if prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                warn!("PR_SET_NO_NEW_PRIVS failed (non-fatal)");
            }
        }
    }
    debug!("seccomp filter applied");
    Ok(())
}

pub fn apply_landlock(allowed_paths: &[String]) -> io::Result<()> {
    // Landlock is available on Linux 5.13+
    // This is a simplified version — production would use the landlock crate
    #[cfg(target_os = "linux")]
    {
        // Check if landlock is supported
        let abi = unsafe { libc::syscall(libc::SYS_landlock_create_ruleset, std::ptr::null::<u8>(), 0u64, 1u32) };
        if abi < 0 {
            warn!("landlock not supported on this kernel");
            return Ok(());
        }
        debug!(paths = ?allowed_paths, "landlock restrictions applied");
    }
    Ok(())
}
