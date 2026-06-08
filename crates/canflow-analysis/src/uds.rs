use crate::alert::Alert;
use canflow_types::CanFrame;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UdsState {
    Idle,
    DiagSession(u8),
    SecurityAccess(u8),
    Authenticated,
    Programming,
}

const UDS_DIAG_SESSION_CONTROL: u8 = 0x10;
const UDS_ECU_RESET: u8 = 0x11;
const UDS_SECURITY_ACCESS: u8 = 0x27;
const UDS_TESTER_PRESENT: u8 = 0x3E;
const UDS_READ_DATA_BY_ID: u8 = 0x22;
const UDS_WRITE_DATA_BY_ID: u8 = 0x2E;
const UDS_REQUEST_DOWNLOAD: u8 = 0x34;
const UDS_TRANSFER_DATA: u8 = 0x36;
const UDS_REQUEST_TRANSFER_EXIT: u8 = 0x37;
const UDS_ROUTINE_CONTROL: u8 = 0x31;

const UDS_POSITIVE_RESPONSE_OFFSET: u8 = 0x40;
const UDS_NEGATIVE_RESPONSE: u8 = 0x7F;

struct UdsSession {
    state: UdsState,
    last_request_ns: u64,
    request_count: u64,
    failed_auth_attempts: u32,
}

impl UdsSession {
    fn new() -> Self {
        Self {
            state: UdsState::Idle,
            last_request_ns: 0,
            request_count: 0,
            failed_auth_attempts: 0,
        }
    }
}

pub struct UdsAnalyzer {
    sessions: HashMap<u32, UdsSession>,
    max_failed_auth: u32,
}

impl UdsAnalyzer {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            max_failed_auth: 3,
        }
    }

    pub fn name(&self) -> &str {
        "uds"
    }

    pub fn ingest(&mut self, frame: &CanFrame) -> Vec<Alert> {
        if frame.dlc < 2 {
            return Vec::new();
        }

        let id = frame.id.raw_id();
        let service_id = frame.data[1];
        let mut alerts = Vec::new();

        // Detect UDS request patterns (typically on IDs 0x700-0x7FF)
        let is_uds_range = id >= 0x600 && id <= 0x7FF;
        if !is_uds_range {
            return alerts;
        }

        let session = self.sessions.entry(id).or_insert_with(UdsSession::new);
        session.last_request_ns = frame.timestamp_ns;
        session.request_count += 1;

        // Check for negative responses (auth failures)
        if service_id == UDS_NEGATIVE_RESPONSE && frame.dlc >= 4 {
            let rejected_service = frame.data[2];
            let nrc = frame.data[3];

            if rejected_service == UDS_SECURITY_ACCESS {
                session.failed_auth_attempts += 1;
                if session.failed_auth_attempts >= self.max_failed_auth {
                    alerts.push(
                        Alert::critical("uds", Some(id), format!(
                            "repeated security access failures: {} attempts",
                            session.failed_auth_attempts
                        ))
                        .with_details(serde_json::json!({
                            "failed_attempts": session.failed_auth_attempts,
                            "nrc": format!("0x{:02X}", nrc)
                        })),
                    );
                }
            }
            return alerts;
        }

        // State machine transitions
        match service_id {
            UDS_DIAG_SESSION_CONTROL => {
                let sub = if frame.dlc >= 3 { frame.data[2] } else { 0 };
                match session.state {
                    UdsState::Idle => {
                        session.state = UdsState::DiagSession(sub);
                    }
                    _ => {
                        if sub == 0x02 || sub == 0x03 {
                            // Jump to extended/programming without proper auth
                            if session.state != UdsState::Authenticated {
                                alerts.push(Alert::warning(
                                    "uds", Some(id),
                                    format!("session escalation without auth: sub=0x{:02X}", sub),
                                ));
                            }
                        }
                        session.state = UdsState::DiagSession(sub);
                    }
                }
            }
            UDS_SECURITY_ACCESS => {
                session.state = UdsState::SecurityAccess(
                    if frame.dlc >= 3 { frame.data[2] } else { 0 },
                );
            }
            s if s == UDS_SECURITY_ACCESS + UDS_POSITIVE_RESPONSE_OFFSET => {
                session.state = UdsState::Authenticated;
                session.failed_auth_attempts = 0;
            }
            UDS_REQUEST_DOWNLOAD | UDS_TRANSFER_DATA => {
                if session.state != UdsState::Authenticated
                    && session.state != UdsState::Programming
                {
                    alerts.push(Alert::critical(
                        "uds", Some(id),
                        "firmware transfer without authentication".to_string(),
                    ));
                }
                session.state = UdsState::Programming;
            }
            UDS_ECU_RESET => {
                alerts.push(Alert::info(
                    "uds", Some(id),
                    "ECU reset requested".to_string(),
                ));
                session.state = UdsState::Idle;
            }
            _ => {}
        }

        alerts
    }

    pub fn tick(&mut self) -> Vec<Alert> {
        Vec::new()
    }

    pub fn reset(&mut self) {
        self.sessions.clear();
    }
}
