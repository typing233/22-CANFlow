use canflow_types::*;
use canflow_adapter::CanAdapter;
use canflow_bus::FrameBus;
use canflow_analysis::AnalysisEngine;
use tokio::sync::{mpsc, watch};

#[tokio::test]
async fn end_to_end_bus_pipeline() {
    let mut bus = FrameBus::new(1024);
    let ingest_tx = bus.ingest_sender();
    let broadcast_tx = bus.broadcast_sender();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        bus.run(shutdown_rx).await;
    });

    let (alert_tx, mut alert_rx) = mpsc::channel(1024);
    let mut config = AnalysisConfig::default();
    config.entropy.learning_frames = 50;

    let mut engine = AnalysisEngine::new(&config, alert_tx.clone());
    let analysis_rx = broadcast_tx.subscribe();
    tokio::spawn(async move {
        engine.run(analysis_rx).await;
    });

    for i in 0..100u32 {
        let frame = CanFrame::new(CanId::standard(0x100), &[i as u8, 0xAA, 0xBB, 0xCC]);
        ingest_tx.send(frame).await.unwrap();
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let mut alerts = Vec::new();
    while let Ok(alert) = alert_rx.try_recv() {
        alerts.push(alert);
    }

    let has_learning_complete = alerts.iter().any(|a| a.message.contains("learning complete"));
    assert!(has_learning_complete, "entropy learning phase did not complete");

    let _ = shutdown_tx.send(true);
}

#[tokio::test]
async fn entropy_detects_anomaly_after_learning() {
    let mut config = AnalysisConfig::default();
    config.entropy.learning_frames = 80;
    config.entropy.window_size = 16;
    let (alert_tx, _alert_rx) = mpsc::channel(1024);
    let mut engine = AnalysisEngine::new(&config, alert_tx);

    // Learning phase: uniform data
    for i in 0..80u64 {
        let frame = CanFrame {
            timestamp_ns: i * 1_000_000,
            id: CanId::standard(0x200),
            dlc: 8,
            data: [0x42; 8],
            is_error: false,
            is_remote: false,
            interface: InterfaceId(0),
        };
        engine.ingest_frame(&frame);
    }

    // Detection phase: random high-entropy data
    let mut got_alert = false;
    for i in 80..200u64 {
        let data: [u8; 8] = [
            rand::random(), rand::random(), rand::random(), rand::random(),
            rand::random(), rand::random(), rand::random(), rand::random(),
        ];
        let frame = CanFrame {
            timestamp_ns: i * 1_000_000,
            id: CanId::standard(0x200),
            dlc: 8,
            data,
            is_error: false,
            is_remote: false,
            interface: InterfaceId(0),
        };
        let alerts = engine.ingest_frame(&frame);
        if alerts.iter().any(|a| a.message.contains("high entropy")) {
            got_alert = true;
            break;
        }
    }

    assert!(got_alert, "entropy analyzer failed to detect anomaly after learning");
}

#[tokio::test]
async fn replay_adapter_parses_candump() {
    let log = "(1609459200.000000) vcan0 123#DEADBEEF\n\
               (1609459200.001000) vcan0 456#0102030405060708\n\
               (1609459200.002000) vcan0 789#AA\n";

    let tmpfile = "/tmp/canflow_integ_replay.log";
    tokio::fs::write(tmpfile, log).await.unwrap();

    let (tx, mut rx) = mpsc::channel(64);
    let mut adapter = canflow_adapter::ReplayAdapter::new(
        tmpfile.into(),
        ReplayFormat::Candump,
        false,
        InterfaceId(0),
    );
    adapter.run(tx).await.unwrap();

    let mut frames = Vec::new();
    while let Ok(f) = rx.try_recv() {
        frames.push(f);
    }

    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].id.raw_id(), 0x123);
    assert_eq!(frames[1].id.raw_id(), 0x456);
    assert_eq!(frames[2].id.raw_id(), 0x789);

    tokio::fs::remove_file(tmpfile).await.ok();
}
