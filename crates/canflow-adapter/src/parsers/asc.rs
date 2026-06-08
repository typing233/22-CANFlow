use canflow_types::{CanFrame, CanId, InterfaceId};

/// Parse Vector ASC log format:
///    0.000100 1  1A3       Rx  d 4 DE AD BE EF
///    0.001000 1  1FFFFFFF  Rx  d 8 01 02 03 04 05 06 07 08
pub fn parse(content: &str, interface_id: InterfaceId) -> Vec<CanFrame> {
    let mut frames = Vec::new();
    let mut in_header = true;

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        // Skip header lines (date, base, etc.)
        if in_header {
            if line.starts_with("date")
                || line.starts_with("base")
                || line.starts_with("internal")
                || line.starts_with("Begin")
                || line.starts_with("Start")
            {
                continue;
            }
            // First line that looks like data
            if line.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                in_header = false;
            } else {
                continue;
            }
        }

        if let Some(frame) = parse_line(line, interface_id) {
            frames.push(frame);
        }
    }

    frames
}

fn parse_line(line: &str, interface_id: InterfaceId) -> Option<CanFrame> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 {
        return None;
    }

    // timestamp (seconds as float)
    let ts_secs: f64 = parts[0].parse().ok()?;
    let timestamp_ns = (ts_secs * 1_000_000_000.0) as u64;

    // channel (skip)
    // id
    let id_str = parts[2];
    let id_val = u32::from_str_radix(id_str.trim_end_matches('x'), 16).ok()?;
    let is_extended = id_val > 0x7FF || id_str.ends_with('x');

    let id = if is_extended {
        CanId::extended(id_val)
    } else {
        CanId::standard(id_val as u16)
    };

    // direction (Rx/Tx) - skip
    // d/r flag
    let frame_type = parts[4];
    let is_remote = frame_type == "r";

    // DLC
    let dlc: u8 = parts[5].parse().ok()?;
    let dlc = dlc.min(8);

    // Data bytes
    let mut data = [0u8; 8];
    for i in 0..dlc as usize {
        if let Some(byte_str) = parts.get(6 + i) {
            if let Ok(b) = u8::from_str_radix(byte_str, 16) {
                data[i] = b;
            }
        }
    }

    Some(CanFrame {
        timestamp_ns,
        id,
        dlc,
        data,
        is_error: false,
        is_remote,
        interface: interface_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_asc_line() {
        let line = "0.000100 1  1A3       Rx  d 4 DE AD BE EF";
        let frame = parse_line(line, InterfaceId(0)).unwrap();
        assert_eq!(frame.id.raw_id(), 0x1A3);
        assert_eq!(frame.dlc, 4);
        assert_eq!(&frame.data[..4], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_parse_asc_extended() {
        let line = "0.001000 1  1FFFFFFF  Rx  d 8 01 02 03 04 05 06 07 08";
        let frame = parse_line(line, InterfaceId(0)).unwrap();
        assert!(frame.id.is_extended());
        assert_eq!(frame.dlc, 8);
    }
}
