use canflow_analysis::Alert;
use canflow_bus::LiveStatsSnapshot;
use canflow_types::CanFrame;
use std::sync::Arc;

use crate::commands::OutputFormat;

pub fn print_frame(frame: &CanFrame, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string(frame).unwrap_or_default();
            println!("{}", json);
        }
        OutputFormat::Table => {
            let ts = format!("{:.6}", frame.timestamp_ns as f64 / 1_000_000_000.0);
            let data: String = frame.payload().iter().map(|b| format!("{:02X} ", b)).collect();
            println!("{} {} [{}] {}", ts, frame.id, frame.dlc, data.trim());
        }
        OutputFormat::Tui => {} // handled by TUI
    }
}

pub fn print_alert(alert: &Alert, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string(alert).unwrap_or_default();
            println!("{}", json);
        }
        OutputFormat::Table => {
            let id_str = alert.frame_id.map_or("-".to_string(), |id| format!("0x{:03X}", id));
            println!("[{}] {} ({}) {}", alert.severity, alert.analyzer, id_str, alert.message);
        }
        OutputFormat::Tui => {}
    }
}

pub fn print_stats(stats: &LiveStatsSnapshot, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string(stats).unwrap_or_default();
            println!("{}", json);
        }
        OutputFormat::Table => {
            println!("--- Stats ---");
            println!("  Uptime:    {:.1}s", stats.uptime_secs);
            println!("  FPS:       {:.0}", stats.current_fps);
            println!("  Total:     {}", stats.total_frames);
            println!("  Unique:    {}", stats.unique_ids);
            for (id, count) in &stats.top_ids {
                println!("  0x{:03X}:    {}", id, count);
            }
        }
        OutputFormat::Tui => {}
    }
}
