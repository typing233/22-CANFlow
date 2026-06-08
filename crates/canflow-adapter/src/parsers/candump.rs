use canflow_types::{CanFrame, CanId, InterfaceId};

/// Parse candump log format:
/// (1234567890.123456) vcan0 1A3#DEADBEEF
/// (1234567890.123456) vcan0 1A3#R (remote frame)
pub fn parse(content: &str, interface_id: InterfaceId) -> Vec<CanFrame> {
    let mut frames = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(frame) = parse_line(line, interface_id) {
            frames.push(frame);
        }
    }

    frames
}

fn parse_line(line: &str, interface_id: InterfaceId) -> Option<CanFrame> {
    // Format: (timestamp) interface id#data
    let line = line.strip_prefix('(')?;
    let (ts_str, rest) = line.split_once(')')?;
    let timestamp_ns = parse_timestamp(ts_str.trim())?;

    let rest = rest.trim();
    // Skip interface name
    let (_iface, frame_part) = rest.split_once(' ')?;
    let frame_part = frame_part.trim();

    let (id_str, data_str) = frame_part.split_once('#')?;

    let id_val = u32::from_str_radix(id_str, 16).ok()?;
    let id = if id_val > 0x7FF {
        CanId::extended(id_val)
    } else {
        CanId::standard(id_val as u16)
    };

    let is_remote = data_str == "R" || data_str == "r";
    let data = if is_remote {
        [0u8; 8]
    } else {
        parse_hex_data(data_str)
    };
    let dlc = if is_remote { 0 } else { data_str.len() / 2 };

    Some(CanFrame {
        timestamp_ns,
        id,
        dlc: dlc.min(8) as u8,
        data,
        is_error: false,
        is_remote,
        interface: interface_id,
    })
}

fn parse_timestamp(s: &str) -> Option<u64> {
    let (secs_str, frac_str) = s.split_once('.')?;
    let secs: u64 = secs_str.parse().ok()?;
    let frac: u64 = frac_str.parse().ok()?;
    let frac_digits = frac_str.len() as u32;
    let nanos = frac * 10u64.pow(9u32.saturating_sub(frac_digits));
    Some(secs * 1_000_000_000 + nanos)
}

fn parse_hex_data(s: &str) -> [u8; 8] {
    let mut data = [0u8; 8];
    let bytes: Vec<u8> = (0..s.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 2 <= s.len() {
                u8::from_str_radix(&s[i..i + 2], 16).ok()
            } else {
                None
            }
        })
        .collect();

    let copy_len = bytes.len().min(8);
    data[..copy_len].copy_from_slice(&bytes[..copy_len]);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_candump_line() {
        let line = "(1609459200.000000) vcan0 1A3#DEADBEEF";
        let frame = parse_line(line, InterfaceId(0)).unwrap();
        assert_eq!(frame.id.raw_id(), 0x1A3);
        assert_eq!(frame.dlc, 4);
        assert_eq!(&frame.data[..4], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_parse_extended_id() {
        let line = "(1609459200.100000) can0 1FFFFFFF#0102030405060708";
        let frame = parse_line(line, InterfaceId(0)).unwrap();
        assert!(frame.id.is_extended());
        assert_eq!(frame.id.raw_id(), 0x1FFFFFFF);
        assert_eq!(frame.dlc, 8);
    }

    #[test]
    fn test_parse_remote_frame() {
        let line = "(1609459200.200000) vcan0 100#R";
        let frame = parse_line(line, InterfaceId(0)).unwrap();
        assert!(frame.is_remote);
        assert_eq!(frame.dlc, 0);
    }
}
