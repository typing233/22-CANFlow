use canflow_types::*;
use canflow_adapter::build_adapter;
use tokio::sync::mpsc;

fn vcan_available() -> bool {
    std::path::Path::new("/sys/class/net/vcan0").exists()
}

#[tokio::test]
#[ignore = "requires vcan0 interface (sudo modprobe vcan && sudo ip link add vcan0 type vcan && sudo ip link set vcan0 up)"]
async fn vcan_send_receive_stability() {
    if !vcan_available() {
        eprintln!("vcan0 not available, skipping");
        return;
    }

    let config = AdapterConfig {
        name: "vcan0".to_string(),
        kind: AdapterKind::VirtualCan { interface: "vcan0".to_string() },
        filters: Vec::new(),
        reconnect: ReconnectPolicy::default(),
    };

    let mut adapter = build_adapter(&config, InterfaceId(0)).unwrap();
    let (tx, mut rx) = mpsc::channel(4096);

    let handle = tokio::spawn(async move {
        adapter.run(tx).await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    for i in 0..10u8 {
        let cmd = format!("cansend vcan0 7DF#{:02X}0102030405060708", i);
        let _ = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let mut received = 0;
    while rx.try_recv().is_ok() {
        received += 1;
    }

    assert!(received >= 10, "expected >=10 frames, got {}", received);
    handle.abort();
}

#[tokio::test]
#[ignore = "requires vcan0 + cangen tool"]
async fn vcan_high_throughput_no_loss() {
    if !vcan_available() {
        return;
    }

    let config = AdapterConfig {
        name: "vcan0".to_string(),
        kind: AdapterKind::VirtualCan { interface: "vcan0".to_string() },
        filters: Vec::new(),
        reconnect: ReconnectPolicy::default(),
    };

    let mut adapter = build_adapter(&config, InterfaceId(0)).unwrap();
    let (tx, mut rx) = mpsc::channel(16384);

    let handle = tokio::spawn(async move {
        adapter.run(tx).await
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let _ = tokio::process::Command::new("cangen")
        .args(["vcan0", "-n", "1000", "-g", "0", "-I", "r", "-D", "r", "-L", "8"])
        .output()
        .await;

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let mut received = 0;
    while rx.try_recv().is_ok() {
        received += 1;
    }

    let loss_pct = (1000.0 - received as f64) / 1000.0 * 100.0;
    println!("vcan throughput: received {}/1000, loss: {:.1}%", received, loss_pct);
    assert!(received >= 950, "too many lost frames: {}/1000", received);
    handle.abort();
}

#[tokio::test]
#[ignore = "requires real CAN hardware (can0)"]
async fn real_can_adapter_bind() {
    if !std::path::Path::new("/sys/class/net/can0").exists() {
        return;
    }

    let config = AdapterConfig {
        name: "can0".to_string(),
        kind: AdapterKind::SocketCan { interface: "can0".to_string() },
        filters: Vec::new(),
        reconnect: ReconnectPolicy::default(),
    };

    let result = build_adapter(&config, InterfaceId(0));
    assert!(result.is_ok(), "failed to build CAN adapter: {:?}", result.err());
}
