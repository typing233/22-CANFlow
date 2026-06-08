use canflow_types::{CanFlowError, CanFrame, CanId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{debug, info};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateWalkerConfig {
    pub target_id: u32,
    pub initial_state: Vec<u8>,
    pub max_depth: usize,
    pub timeout_ms: u64,
}

impl Default for StateWalkerConfig {
    fn default() -> Self {
        Self {
            target_id: 0x7DF,
            initial_state: vec![0x02, 0x10, 0x01], // DiagSessionControl defaultSession
            max_depth: 5,
            timeout_ms: 200,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct StateTransition {
    pub from_state: Vec<u8>,
    pub input: Vec<u8>,
    pub to_state: Vec<u8>,
    pub depth: usize,
}

pub struct StateWalker {
    config: StateWalkerConfig,
    visited: HashSet<Vec<u8>>,
    transitions: Vec<StateTransition>,
}

impl StateWalker {
    pub fn new(config: StateWalkerConfig) -> Self {
        Self {
            config,
            visited: HashSet::new(),
            transitions: Vec::new(),
        }
    }

    pub async fn run(&mut self, tx: mpsc::Sender<CanFrame>) -> Result<Vec<StateTransition>, CanFlowError> {
        let target_id = if self.config.target_id > 0x7FF {
            CanId::extended(self.config.target_id)
        } else {
            CanId::standard(self.config.target_id as u16)
        };

        debug!(target = %self.config.target_id, max_depth = self.config.max_depth, "state walker started");

        // BFS through state space
        let mut queue: Vec<(Vec<u8>, usize)> = vec![(self.config.initial_state.clone(), 0)];
        self.visited.insert(self.config.initial_state.clone());

        // UDS session transitions to explore
        let transitions_to_try: Vec<Vec<u8>> = vec![
            vec![0x02, 0x10, 0x01], // Default session
            vec![0x02, 0x10, 0x02], // Programming session
            vec![0x02, 0x10, 0x03], // Extended diagnostic
            vec![0x02, 0x27, 0x01], // Security access seed
            vec![0x02, 0x27, 0x02], // Security access key
            vec![0x02, 0x11, 0x01], // ECU reset hard
            vec![0x02, 0x11, 0x03], // ECU reset soft
            vec![0x01, 0x3E],       // Tester present
        ];

        while let Some((current_state, depth)) = queue.pop() {
            if depth >= self.config.max_depth {
                continue;
            }

            for transition_input in &transitions_to_try {
                let mut data = [0u8; 8];
                let len = transition_input.len().min(8);
                data[..len].copy_from_slice(&transition_input[..len]);

                let frame = CanFrame {
                    timestamp_ns: canflow_types::timestamp::monotonic_ns(),
                    id: target_id,
                    dlc: 8,
                    data,
                    is_error: false,
                    is_remote: false,
                    interface: canflow_types::InterfaceId(0),
                };

                if tx.send(frame).await.is_err() {
                    return Err(CanFlowError::ChannelClosed);
                }

                sleep(Duration::from_millis(self.config.timeout_ms)).await;

                // Record transition (in real implementation we'd observe response)
                let new_state = transition_input.clone();
                if !self.visited.contains(&new_state) {
                    self.visited.insert(new_state.clone());
                    self.transitions.push(StateTransition {
                        from_state: current_state.clone(),
                        input: transition_input.clone(),
                        to_state: new_state.clone(),
                        depth: depth + 1,
                    });
                    queue.push((new_state, depth + 1));
                }
            }
        }

        info!(transitions = self.transitions.len(), states = self.visited.len(), "state walker completed");
        Ok(self.transitions.clone())
    }
}
