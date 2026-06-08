mod commands;
mod output;

use canflow_adapter::{build_adapter, PrivilegeLevel};
use canflow_analysis::AnalysisEngine;
use canflow_bus::{AuditLogger, FrameBus, LiveStats, SessionRecorder};
use canflow_types::*;
use clap::Parser;
use commands::*;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Init tracing
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
            run_capture(interface, replay, record, analyze, &cli.format).await?;
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

            // Parse frames from recorded session (JSONL)
            for line in content.lines() {
                if let Ok(frame) = serde_json::from_str::<CanFrame>(line) {
                    let alerts = engine.ingest_frame(&frame);
                    for alert in alerts {
                        output::print_alert(&alert, &cli.format);
                    }
                }
            }

            // Drain remaining alerts
            while let Ok(alert) = alert_rx.try_recv() {
                output::print_alert(&alert, &cli.format);
            }
        }

        Commands::Replay { file, format: fmt, speed, interface } => {
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

            task.run(tx).await?;
            print_handle.await?;
        }

        Commands::Fuzz { task, ids, iterations, interface } => {
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
                AgentAction::Run { script, interface } => {
                    let content = tokio::fs::read_to_string(&script).await?;
                    let ext = script.extension().and_then(|e| e.to_str()).unwrap_or("");

                    let (tx, mut rx) = mpsc::channel(8192);
                    let output_format = cli.format.clone();
                    let print_handle = tokio::spawn(async move {
                        while let Some(frame) = rx.recv().await {
                            output::print_frame(&frame, &output_format);
                        }
                    });

                    if ext == "lua" {
                        let runtime = canflow_agent::LuaRuntime::new("cli-agent", &content, true)?;
                        let frames = runtime.execute()?;
                        for frame in frames {
                            let _ = tx.send(frame).await;
                        }
                    } else {
                        println!("Unsupported script format: {}", ext);
                    }

                    drop(tx);
                    print_handle.await?;
                }
                AgentAction::Pipeline { config } => {
                    let content = tokio::fs::read_to_string(&config).await?;
                    let pipeline: canflow_agent::Pipeline = toml::from_str(&content)
                        .map_err(|e| canflow_types::CanFlowError::Config(e.to_string()))?;
                    println!("Pipeline '{}' loaded with {} stages", pipeline.name, pipeline.stages.len());

                    let (tx, _rx) = mpsc::channel(8192);
                    let engine = canflow_agent::AgentEngine::new(tx);
                    let results = engine.run_pipeline(&pipeline).await?;
                    for r in results {
                        println!("  {}", r);
                    }
                }
                AgentAction::Probe { target, interface } => {
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
                    let registry = canflow_plugin::PluginRegistry::new("./plugins");
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
                    let mut registry = canflow_plugin::PluginRegistry::new("./plugins");
                    registry.load(&path, "{}")?;
                    println!("Plugin loaded from: {}", path.display());
                }
                PluginAction::Reload { name } => {
                    let mut registry = canflow_plugin::PluginRegistry::new("./plugins");
                    registry.reload(&name, "{}")?;
                    println!("Plugin '{}' reloaded", name);
                }
            }
        }
    }

    Ok(())
}

async fn run_capture(
    interface: Option<String>,
    replay: Option<PathBuf>,
    record: Option<PathBuf>,
    analyze: bool,
    format: &OutputFormat,
) -> anyhow::Result<()> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Build bus
    let mut bus = FrameBus::new(16384);
    let ingest_tx = bus.ingest_sender();
    let stats = bus.stats();
    let live_stats = Arc::new(LiveStats::new());

    // Setup adapter
    let adapter_config = if let Some(ref file) = replay {
        AdapterConfig {
            name: "replay".to_string(),
            kind: AdapterKind::LogReplay {
                path: file.clone(),
                format: ReplayFormat::Candump,
                loop_forever: false,
            },
            filters: Vec::new(),
            reconnect: ReconnectPolicy::default(),
        }
    } else {
        let iface = interface.unwrap_or_else(|| "vcan0".to_string());
        AdapterConfig {
            name: iface.clone(),
            kind: AdapterKind::VirtualCan { interface: iface },
            filters: Vec::new(),
            reconnect: ReconnectPolicy::default(),
        }
    };

    let mut adapter = build_adapter(&adapter_config, InterfaceId(0))?;

    // Subscribe BEFORE moving bus (bus is consumed by run)
    let broadcast_tx = bus.broadcast_sender();

    // Spawn bus
    let bus_shutdown_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        bus.run(bus_shutdown_rx).await;
    });

    // Spawn adapter
    let adapter_tx = ingest_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = adapter.run(adapter_tx).await {
            error!(error = %e, "adapter error");
        }
    });

    // Spawn analysis if enabled
    let (alert_tx, alert_rx) = mpsc::channel(1024);
    if analyze {
        let analysis_config = AnalysisConfig::default();
        let mut engine = AnalysisEngine::new(&analysis_config, alert_tx.clone());
        let analysis_rx = broadcast_tx.subscribe();
        tokio::spawn(async move {
            engine.run(analysis_rx).await;
        });
    }

    // Spawn recording if requested
    if let Some(ref path) = record {
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
            let format = format.clone();
            let mut shutdown_watch = shutdown_rx.clone();
            loop {
                tokio::select! {
                    result = frame_rx.recv() => {
                        match result {
                            Ok(frame) => output::print_frame(&frame, &format),
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                eprintln!("[WARN] lagged {} frames", n);
                            }
                            Err(_) => break,
                        }
                    }
                    _ = shutdown_watch.changed() => break,
                }
            }
        }
    }

    let _ = shutdown_tx.send(true);
    Ok(())
}
