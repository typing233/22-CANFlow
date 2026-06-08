use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

pub fn monotonic_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts as *mut libc::timespec);
    }
    (ts.tv_sec as u64) * 1_000_000_000 + (ts.tv_nsec as u64)
}

pub fn ns_to_secs(ns: u64) -> f64 {
    ns as f64 / 1_000_000_000.0
}

pub fn format_timestamp(ns: u64) -> String {
    let secs = ns / 1_000_000_000;
    let subsec = ns % 1_000_000_000;
    let dt = chrono::DateTime::from_timestamp(secs as i64, subsec as u32)
        .unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M:%S%.6f").to_string()
}
