use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pipeline {
    pub name: String,
    pub stages: Vec<PipelineStage>,
    pub edges: Vec<(usize, usize)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineStage {
    pub name: String,
    pub agent: StageAgent,
    #[serde(default = "default_stage_config")]
    pub config: toml::Value,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

fn default_stage_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StageAgent {
    Lua { script: PathBuf },
    Python { script: PathBuf },
    Builtin { task: String },
}

impl Pipeline {
    pub fn execution_order(&self) -> Vec<usize> {
        // Topological sort based on edges
        let n = self.stages.len();
        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

        for &(from, to) in &self.edges {
            if from < n && to < n {
                adj[from].push(to);
                in_degree[to] += 1;
            }
        }

        let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut order = Vec::new();

        while let Some(node) = queue.pop() {
            order.push(node);
            for &next in &adj[node] {
                in_degree[next] -= 1;
                if in_degree[next] == 0 {
                    queue.push(next);
                }
            }
        }

        order
    }
}
