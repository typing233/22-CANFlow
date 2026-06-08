use canflow_types::*;
use canflow_bus::FrameBus;
use canflow_analysis::AnalysisEngine;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

#[tokio::test]
async fn test_end_to_end_pipeline() {
    let mut bus = FrameBus::new(1024);
    let ingest_tx = bus.ingest_sender();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Start bus
    tokio::spawn(async move {
        bus.run(shutdown_rx).await;
    });

    // Send some frames
    for i in 0..100u32 {
        let frame = CanFrame::new(CanId::standard(0x100), &[i as u8, 0xAA, 0xBB, 0xCC]);
        ingest_tx.send(frame).await.unwrap();
    }

    // Let it process
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let _ = shutdown_tx.send(true);
}

#[tokio::test]
async fn test_analysis_engine_entropy() {
    let config = AnalysisConfig::default();
    let (alert_tx, mut alert_rx) = mpsc::channel(1024);
    let mut engine = AnalysisEngine::new(&config, alert_tx);

    // Feed frames with varying entropy
    for i in 0..200u64 {
        let data = [(i % 256) as u8; 8];
        let frame = CanFrame {
            timestamp_ns: i * 1_000_000,
            id: CanId::standard(0x100),
            dlc: 8,
            data,
            is_error: false,
            is_remote: false,
            interface: InterfaceId(0),
        };
        engine.ingest_frame(&frame);
    }

    // Should not panic, analysis should complete
}

#[tokio::test]
async fn test_adapter_replay_candump() {
    let log = "(1609459200.000000) vcan0 123#DEADBEEF\n\
               (1609459200.001000) vcan0 456#0102030405060708\n\
               (1609459200.002000) vcan0 789#AA\n";

    let tmpfile = "/tmp/canflow_test_replay.log";
    tokio::fs::write(tmpfile, log).await.unwrap();

    let adapter = canflow_adapter::ReplayAdapter::new(
        tmpfile.into(),
        ReplayFormat::Candump,
        false,
        InterfaceId(0),
    );

    // Adapter should be constructable
    assert_eq!(adapter.name(), format!("replay:{}", tmpfile));

    tokio::fs::remove_file(tmpfile).await.ok();
}
