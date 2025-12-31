use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::llm::LlmConfig;
use crate::world::WorldConfig;

/// Top-level configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub meta: MetaConfig,
    pub world: WorldConfig,
    pub agents: AgentsConfig,
    pub simulation: SimulationConfig,
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetaConfig {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentsConfig {
    pub count: usize,
    pub starting_food: u32,
    #[serde(default = "default_personality")]
    pub personality: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SimulationConfig {
    pub epochs: usize,
    #[serde(default = "default_snapshot_interval")]
    pub snapshot_interval: usize,
    #[serde(default = "default_log_thoughts")]
    pub log_thoughts: bool,
}

fn default_personality() -> String {
    "random".to_string()
}

fn default_snapshot_interval() -> usize {
    10
}

fn default_log_thoughts() -> bool {
    true
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            meta: MetaConfig {
                name: "Untitled Terrarium".to_string(),
                description: String::new(),
            },
            world: WorldConfig {
                width: 10,
                height: 10,
                fertile_fraction: 0.3,
                initial_food_per_fertile: 15,
                food_regen_rate: 0.1,
            },
            agents: AgentsConfig {
                count: 10,
                starting_food: 10,
                personality: "random".to_string(),
            },
            simulation: SimulationConfig {
                epochs: 100,
                snapshot_interval: 10,
                log_thoughts: true,
            },
            llm: LlmConfig::default(),
        }
    }
}
