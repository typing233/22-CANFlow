mod commands;
mod output;

use canflow_adapter::{build_adapter, PrivilegeLevel};
use canflow_analysis::AnalysisEngine;
use canflow_bus::{FrameBus, LiveStats, SessionRecorder};
use canflow_plugin::PluginRegistry;
use canflow_types::*;
use clap::Parser;
use commands::*;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tracing::error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Status => {
            let level = PrivilegeLevel::detect();
            println!("CANFlow Status");
            println!("  Privilege: {:?}", level);
            println!("  Real CAN:  {}", level.can_use_real_can());
            println!("  Virtual:   {}", level.can_use_vcan());
        }

        Commands::Capture { interface, replay, record, analyze } => {
            let app_config = load_config(&cli.config).await;
            run_capture(app_config, interface, replay, record, analyze, &cli.format).await?;
        }

        Commands::Analyze { input, modules } => {
            let content = tokio::fs::read_to_string(&input).await?;
            let config = AnalysisConfig {
                enabled: if modules.is_empty() {
                    vec!["entropy".into(), "period".into(), "burst".into(), "uds".into()]
                } else {
                    modules
                },
                ..Default::default()
            };

            let (alert_tx, mut alert_rx) = mpsc::channel(1024);
            let mut engine = AnalysisEngine::new(&config, alert_tx);

            for line in content.lines() {
                if let Ok(frame) = serde_json::from_str::<CanFrame>(line) {
                    let alerts = engine.ingest_frame(&frame);
                    for alert in alerts {
                        output::print_alert(&alert, &cli.format);
                    }
                }
            }

            while let Ok(alert) = alert_rx.try_recv() {
                output::print_alert(&alert, &cli.format);
            }
        }

        Commands::Replay { file, format: fmt, speed, interface: _ } => {
            let (tx, mut rx) = mpsc::channel(8192);
            let replay_config = canflow_agent::builtins::replay::ReplayConfig {
                file,
                format: match fmt.as_str() {
                    "asc" => ReplayFormat::Asc,
                    _ => ReplayFormat::Candump,
                },
                speed_multiplier: speed,
                loop_count: 1,
            };

            let task = canflow_agent::builtins::replay::ReplayTask::new(replay_config);
            let output_format = cli.format.clone();
            let print_handle = tokio::spawn(async move {
                while let Some(frame) = rx.recv().await {
                    output::print_frame(&frame, &output_format);
                }
            });

            let result = task.run(tx).await;
            // tx is dropped here, print_handle will see channel closed
            print_handle.await?;
            result?;
        }

        Commands::Fuzz { task: _, ids, iterations, interface: _ } => {
            let target_ids: Vec<u32> = ids.iter()
                .filter_map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                .collect();

            let target_ids = if target_ids.is_empty() { vec![0x7DF] } else { target_ids };

            let (tx, mut rx) = mpsc::channel(8192);
            let fuzz_config = canflow_agent::builtins::fuzzer::FuzzConfig {
                target_ids,
                iterations,
                ..Default::default()
            };

            let output_format = cli.format.clone();
            let print_handle = tokio::spawn(async move {
                while let Some(frame) = rx.recv().await {
                    output::print_frame(&frame, &output_format);
                }
            });

            let fuzzer = canflow_agent::builtins::fuzzer::Fuzzer::new(fuzz_config);
            let sent = fuzzer.run(tx).await?;
            println!("Fuzzer complete: {} frames sent", sent);
            print_handle.await?;
        }

        Commands::Agent { action } => {
            match action {
                AgentAction::Run { script, interface: _ } => {
                    let (tx, mut rx) = mpsc::channel(8192);
                    let output_format = cli.format.clone();
                    let print_handle = tokio::spawn(async move {
                        while let Some(frame) = rx.recv().await {
                            output::print_frame(&frame, &output_format);
                        }
                    });

                    let engine = canflow_agent::AgentEngine::new(tx.clone());
                    let result = engine.run_script(&script).await?;
                    println!("{}", result);

                    drop(tx);
                    print_handle.await?;
                }
                AgentAction::Pipeline { config } => {
                    let content = tokio::fs::read_to_string(&config).await?;
                    let pipeline: canflow_agent::Pipeline = toml::from_str(&content)
                        .map_err(|e| canflow_types::CanFlowError::Config(e.to_string()))?;
                    println!("Pipeline '{}' loaded with {} stages", pipeline.name, pipeline.stages.len());

                    let (tx, mut rx) = mpsc::channel(8192);
                    let output_format = cli.format.clone();
                    let print_handle = tokio::spawn(async move {
                        while let Some(frame) = rx.recv().await {
                            output::print_frame(&frame, &output_format);
                        }
                    });

                    let engine = canflow_agent::AgentEngine::new(tx.clone());
                    let results = engine.run_pipeline(&pipeline).await?;
                    for r in results {
                        println!("  {}", r);
                    }

                    drop(tx);
                    print_handle.await?;
                }
                AgentAction::Probe { target, interface: _ } => {
                    let target_id = u32::from_str_radix(target.trim_start_matches("0x"), 16)
                        .unwrap_or(0x7DF);
                    let probe_config = canflow_agent::builtins::uds_probe::UdsProbeConfig {
                        target_id,
                        ..Default::default()
                    };

                    let (tx, _rx) = mpsc::channel(8192);
                    let probe = canflow_agent::builtins::uds_probe::UdsProbe::new(probe_config);
                    let results = probe.run(tx).await?;
                    println!("UDS Probe Results ({} probes):", results.len());
                    for r in &results {
                        println!("  SID=0x{:02X} sub={:?}", r.service_id, r.sub_function);
                    }
                }
            }
        }

        Commands::Plugin { action } => {
            match action {
                PluginAction::List => {
                    let registry = PluginRegistry::new("./plugins");
                    let plugins = registry.list();
                    if plugins.is_empty() {
                        println!("No plugins loaded");
                    } else {
                        for name in plugins {
                            println!("  {}", name);
                        }
                    }
                }
                PluginAction::Load { path } => {
                    let mut registry = PluginRegistry::new("./plugins");
                    registry.load(&path, "{}")?;
                    println!("Plugin loaded from: {}", path.display());
                }
                PluginAction::Reload { name } => {
                    let mut registry = PluginRegistry::new("./plugins");
                    registry.reload(&name, "{}")?;
                    println!("Plugin '{}' reloaded", name);
                }
            }
        }
    }

    Ok(())
}

async fn load_config(path: &PathBuf) -> Option<AppConfig> {
    match tokio::fs::read_to_string(path).await {
        Ok(content) => match toml::from_str::<AppConfig>(&content) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                eprintln!("[WARN] failed to parse config {}: {}", path.display(), e);
                None
            }
        },
        Err(_) => None,
    }
}

async fn run_capture(
    app_config: Option<AppConfig>,
    cli_interface: Option<String>,
    cli_replay: Option<PathBuf>,
    cli_record: Option<PathBuf>,
    cli_analyze: bool,
    format: &OutputFormat,
) -> anyhow::Result<()> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Resolve bus config
    let bus_config = app_config.as_ref().map(|c| &c.bus).cloned().unwrap_or_default();
    let analysis_config = app_config.as_ref().map(|c| &c.analysis).cloned().unwrap_or_default();
    let log_config = app_config.as_ref().map(|c| &c.logging).cloned().unwrap_or_default();

    // Build bus from config
    let mut bus = FrameBus::new(bus_config.channel_capacity);
    let ingest_tx = bus.ingest_sender();
    let _stats = bus.stats();
    let live_stats = Arc::new(LiveStats::new());

    // Apply fault injection from config
    if let Some(ref fault) = bus_config.fault_injection {
        bus.set_fault_config(Some(fault.clone()));
    }

    // Determine adapter config: CLI flags override config file
    let adapter_configs: Vec<AdapterConfig> = if let Some(ref file) = cli_replay {
        vec![AdapterConfig {
            name: "replay".to_string(),
            kind: AdapterKind::LogReplay {
                path: file.clone(),
                format: ReplayFormat::Candump,
                loop_forever: false,
            },
            filters: Vec::new(),
            reconnect: ReconnectPolicy::default(),
        }]
    } else if let Some(ref iface) = cli_interface {
        vec![AdapterConfig {
            name: iface.clone(),
            kind: AdapterKind::VirtualCan { interface: iface.clone() },
            filters: Vec::new(),
            reconnect: ReconnectPolicy::default(),
        }]
    } else if let Some(ref cfg) = app_config {
        if cfg.adapters.is_empty() {
            vec![AdapterConfig {
                name: "vcan0".to_string(),
                kind: AdapterKind::VirtualCan { interface: "vcan0".to_string() },
                filters: Vec::new(),
                reconnect: ReconnectPolicy::default(),
            }]
        } else {
            cfg.adapters.clone()
        }
    } else {
        vec![AdapterConfig {
            name: "vcan0".to_string(),
            kind: AdapterKind::VirtualCan { interface: "vcan0".to_string() },
            filters: Vec::new(),
            reconnect: ReconnectPolicy::default(),
        }]
    };

    // Subscribe BEFORE moving bus
    let broadcast_tx = bus.broadcast_sender();

    // Spawn bus
    let bus_shutdown_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        bus.run(bus_shutdown_rx).await;
    });

    // Spawn adapters — track handles so we know when replay finishes
    let mut adapter_handles = Vec::new();
    for (i, adapter_cfg) in adapter_configs.iter().enumerate() {
        let mut adapter = build_adapter(adapter_cfg, InterfaceId(i as u16))?;
        let tx = ingest_tx.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = adapter.run(tx).await {
                match e {
                    CanFlowError::ChannelClosed | CanFlowError::Shutdown => {}
                    _ => error!(error = %e, "adapter error"),
                }
            }
        });
        adapter_handles.push(handle);
    }

    // When all adapters finish (e.g. replay completes), signal shutdown
    let shutdown_on_adapter_done = shutdown_tx.clone();
    tokio::spawn(async move {
        for h in adapter_handles {
            let _ = h.await;
        }
        // Small grace period to let buffered frames drain through the bus
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let _ = shutdown_on_adapter_done.send(true);
    });

    // Spawn analysis if enabled (via CLI flag or config has modules)
    let (alert_tx, alert_rx) = mpsc::channel(1024);
    if cli_analyze || !analysis_config.enabled.is_empty() {
        let mut engine = AnalysisEngine::new(&analysis_config, alert_tx.clone());
        let analysis_rx = broadcast_tx.subscribe();
        tokio::spawn(async move {
            engine.run(analysis_rx).await;
        });
    }

    // Spawn plugin host if plugins directory exists
    let plugin_dir = PathBuf::from("./plugins");
    if plugin_dir.is_dir() {
        let plugin_alert_tx = alert_tx.clone();
        let plugin_rx = broadcast_tx.subscribe();
        tokio::spawn(async move {
            run_plugin_stream(plugin_dir, plugin_rx, plugin_alert_tx).await;
        });
    }

    // Spawn recording if requested
    if let Some(ref path) = cli_record {
        let mut recorder = SessionRecorder::new(path).await?;
        let record_rx = broadcast_tx.subscribe();
        tokio::spawn(async move {
            recorder.run(record_rx).await;
        });
    }

    // Spawn stats collector
    let stats_rx = broadcast_tx.subscribe();
    let live_stats_clone = live_stats.clone();
    tokio::spawn(async move {
        live_stats_clone.run(stats_rx).await;
    });

    // Handle Ctrl+C
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let _ = shutdown_tx_clone.send(true);
    });

    // Run TUI or stream mode
    match format {
        OutputFormat::Tui => {
            let frame_rx = broadcast_tx.subscribe();
            canflow_tui::run_tui(frame_rx, alert_rx, live_stats, shutdown_rx).await?;
        }
        _ => {
            let mut frame_rx = broadcast_tx.subscribe();
            let mut alert_rx = alert_rx;
            let format = format.clone();
            let mut shutdown_watch = shutdown_rx.clone();
            loop {
                tokio::select! {
                    biased;
                    _ = shutdown_watch.changed() => break,
                    result = frame_rx.recv() => {
                        match result {
                            Ok(frame) => output::print_frame(&frame, &format),
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                eprintln!("[WARN] lagged {} frames", n);
                            }
                            Err(_) => break,
                        }
                    }
                    Some(alert) = alert_rx.recv() => {
                        output::print_alert(&alert, &format);
                    }
                }
            }
        }
    }

    let _ = shutdown_tx.send(true);
    // Allow spawned tasks to wind down
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    Ok(())
}

async fn run_plugin_stream(
    plugin_dir: PathBuf,
    mut rx: tokio::sync::broadcast::Receiver<Arc<CanFrame>>,
    alert_tx: mpsc::Sender<canflow_analysis::Alert>,
) {
    let mut registry = PluginRegistry::new(&plugin_dir);

    // Auto-load any .so files in the plugin directory
    if let Ok(mut entries) = tokio::fs::read_dir(&plugin_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "so") {
                if let Err(e) = registry.load(&path, "{}") {
                    eprintln!("[WARN] failed to load plugin {}: {}", path.display(), e);
                }
            }
        }
    }

    if registry.list().is_empty() {
        return;
    }

    let mut tick_interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(frame) => {
                        let alerts = registry.ingest_all(&frame);
                        for alert in alerts {
                            let _ = alert_tx.send(alert).await;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            _ = tick_interval.tick() => {
                let alerts = registry.tick_all();
                for alert in alerts {
                    let _ = alert_tx.send(alert).await;
                }
            }
        }
    }
}
