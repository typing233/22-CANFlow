use canflow_types::*;
use canflow_bus::FrameBus;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch;

#[tokio::test]
async fn stress_8000_fps_throughput() {
    let mut bus = FrameBus::new(16384);
    let ingest_tx = bus.ingest_sender();
    let mut sub_rx = bus.subscribe();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Start bus
    tokio::spawn(async move {
        bus.run(shutdown_rx).await;
    });

    let total_frames: u64 = 80_000;
    let start = Instant::now();

    // Producer: send 80k frames (simulating 8000fps for 10 seconds)
    let tx = ingest_tx.clone();
    let producer = tokio::spawn(async move {
        for i in 0..total_frames {
            let frame = CanFrame {
                timestamp_ns: i * 125_000, // 125us between frames = 8000fps
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

    // Consumer: count received frames
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

    println!("Throughput test results:");
    println!("  Total frames:  {}", total_frames);
    println!("  Received:      {}", received);
    println!("  Lagged:        {}", lagged);
    println!("  Time:          {:.3}s", elapsed.as_secs_f64());
    println!("  Throughput:    {:.0} fps", fps);
    println!("  Loss rate:     {:.4}%", lagged as f64 / total_frames as f64 * 100.0);

    let _ = shutdown_tx.send(true);

    // At 8000fps target, we should process much faster in-memory
    assert!(fps > 50_000.0, "throughput too low: {} fps", fps);
}

#[tokio::test]
async fn stress_backpressure_no_panic() {
    // Push at max speed with tiny buffer to test backpressure
    let mut bus = FrameBus::new(64); // Very small buffer
    let ingest_tx = bus.ingest_sender();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        bus.run(shutdown_rx).await;
    });

    // Rapid-fire without consumer (tests that bus doesn't OOM)
    for i in 0..10_000u32 {
        let frame = CanFrame::new(CanId::standard(0x100), &[i as u8; 8]);
        // This may block on backpressure, but should never panic
        let _ = ingest_tx.try_send(frame);
    }

    let _ = shutdown_tx.send(true);
}
