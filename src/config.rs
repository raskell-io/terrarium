use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub name: String,
    pub agents: AgentConfig,
    pub world: WorldConfig,
    pub simulation: SimulationParams,
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Number of agents to create
    pub count: usize,
    /// Distribution of personality traits (if None, randomized)
    pub personality_distribution: Option<PersonalityDistribution>,
    /// Initial resource distribution
    pub starting_resources: ResourceDistribution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityDistribution {
    /// Big Five trait distributions (mean, std_dev)
    pub openness: (f64, f64),
    pub conscientiousness: (f64, f64),
    pub extraversion: (f64, f64),
    pub agreeableness: (f64, f64),
    pub neuroticism: (f64, f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDistribution {
    /// How resources are distributed at start
    pub mode: DistributionMode,
    /// Total resources in the system
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DistributionMode {
    /// Everyone gets equal share
    Equal,
    /// Random distribution
    Random,
    /// Pareto distribution (wealth inequality)
    Pareto { alpha: f64 },
    /// Some agents start with nothing
    SomeHaveNone { fraction_with_none: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    /// World dimensions
    pub width: usize,
    pub height: usize,
    /// Terrain generation parameters
    pub terrain: TerrainConfig,
    /// Resource regeneration rate per epoch
    pub resource_regen_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainConfig {
    /// Fraction of land that is fertile
    pub fertile_fraction: f64,
    /// Fraction that has resources (minerals, etc)
    pub resource_fraction: f64,
    /// Whether to use geographical barriers
    pub barriers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationParams {
    /// Number of epochs to run
    pub epochs: usize,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
    /// How many epochs between full snapshots
    pub snapshot_interval: usize,
    /// Whether to log agent internal monologues
    pub log_thoughts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Which provider to use
    pub provider: LlmProvider,
    /// Model to use
    pub model: String,
    /// API key (or env var name)
    pub api_key_env: String,
    /// Max tokens for agent responses
    pub max_tokens: usize,
    /// Temperature for agent decisions
    pub temperature: f64,
    /// Whether to cache LLM responses
    pub cache_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    Local { endpoint: String },
}

impl SimulationConfig {
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn default_with(agent_count: usize, epochs: usize, seed: Option<u64>) -> Self {
        Self {
            name: "Untitled Terrarium".to_string(),
            agents: AgentConfig {
                count: agent_count,
                personality_distribution: None,
                starting_resources: ResourceDistribution {
                    mode: DistributionMode::Equal,
                    total: agent_count as u64 * 100,
                },
            },
            world: WorldConfig {
                width: 20,
                height: 20,
                terrain: TerrainConfig {
                    fertile_fraction: 0.3,
                    resource_fraction: 0.1,
                    barriers: false,
                },
                resource_regen_rate: 0.05,
            },
            simulation: SimulationParams {
                epochs,
                seed,
                snapshot_interval: 10,
                log_thoughts: true,
            },
            llm: LlmConfig {
                provider: LlmProvider::Anthropic,
                model: "claude-sonnet-4-20250514".to_string(),
                api_key_env: "ANTHROPIC_API_KEY".to_string(),
                max_tokens: 500,
                temperature: 0.7,
                cache_enabled: true,
            },
        }
    }
}

impl Default for PersonalityDistribution {
    fn default() -> Self {
        Self {
            openness: (0.5, 0.15),
            conscientiousness: (0.5, 0.15),
            extraversion: (0.5, 0.15),
            agreeableness: (0.5, 0.15),
            neuroticism: (0.5, 0.15),
        }
    }
}
