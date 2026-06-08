use canflow_types::*;
use canflow_bus::FrameBus;
use std::time::Instant;
use tokio::sync::watch;

#[tokio::test]
async fn stress_8000_fps_throughput() {
    let mut bus = FrameBus::new(16384);
    let ingest_tx = bus.ingest_sender();
    let mut sub_rx = bus.subscribe();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        bus.run(shutdown_rx).await;
    });

    let total_frames: u64 = 80_000;
    let start = Instant::now();

    let tx = ingest_tx.clone();
    let producer = tokio::spawn(async move {
        for i in 0..total_frames {
            let frame = CanFrame {
                timestamp_ns: i * 125_000,
                id: CanId::standard((i % 0x7FF) as u16),
                dlc: 8,
                data: [
                    (i & 0xFF) as u8,
                    ((i >> 8) & 0xFF) as u8,
                    ((i >> 16) & 0xFF) as u8,
                    ((i >> 24) & 0xFF) as u8,
                    0xDE, 0xAD, 0xBE, 0xEF,
                ],
                is_error: false,
                is_remote: false,
                interface: InterfaceId(0),
            };
            tx.send(frame).await.unwrap();
        }
    });

    let consumer = tokio::spawn(async move {
        let mut received = 0u64;
        let mut lagged = 0u64;
        loop {
            match sub_rx.recv().await {
                Ok(_) => {
                    received += 1;
                    if received >= total_frames {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    lagged += n;
                    received += n;
                    if received >= total_frames {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        (received, lagged)
    });

    producer.await.unwrap();
    let (received, lagged) = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        consumer,
    ).await.unwrap().unwrap();

    let elapsed = start.elapsed();
    let fps = total_frames as f64 / elapsed.as_secs_f64();

    println!("Throughput: {:.0} fps, received: {}, lagged: {}", fps, received, lagged);
    let _ = shutdown_tx.send(true);

    assert!(fps > 50_000.0, "throughput too low: {:.0} fps", fps);
}

#[tokio::test]
async fn stress_backpressure_no_panic() {
    let mut bus = FrameBus::new(64);
    let ingest_tx = bus.ingest_sender();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        bus.run(shutdown_rx).await;
    });

    for i in 0..10_000u32 {
        let frame = CanFrame::new(CanId::standard(0x100), &[i as u8; 8]);
        let _ = ingest_tx.try_send(frame);
    }

    let _ = shutdown_tx.send(true);
}
