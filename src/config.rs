use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::environment::EnvironmentConfig;
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
    #[serde(default)]
    pub environment: Option<EnvironmentConfig>,
    #[serde(default)]
    pub reproduction: ReproductionConfig,
    #[serde(default)]
    pub aging: AgingConfig,
    #[serde(default)]
    pub skills: SkillsConfig,
    #[serde(default)]
    pub trade: TradeConfig,
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

/// Reproduction system configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ReproductionConfig {
    /// Whether reproduction is enabled
    #[serde(default = "default_reproduction_enabled")]
    pub enabled: bool,
    /// Epochs required for gestation
    #[serde(default = "default_gestation_period")]
    pub gestation_period: usize,
    /// Food cost to initiate mating (per parent)
    #[serde(default = "default_mating_food_cost")]
    pub mating_food_cost: u32,
    /// Energy cost during gestation (per epoch)
    #[serde(default = "default_gestation_energy_drain")]
    pub gestation_energy_drain: f64,
    /// Courtship threshold to allow mating (0.0-1.0)
    #[serde(default = "default_courtship_threshold")]
    pub courtship_threshold: f64,
    /// Courtship score increase per successful court action
    #[serde(default = "default_courtship_increment")]
    pub courtship_increment: f64,
    /// Courtship decay per epoch
    #[serde(default = "default_courtship_decay")]
    pub courtship_decay: f64,
    /// Minimum epochs between mating for an agent
    #[serde(default = "default_mating_cooldown")]
    pub mating_cooldown: usize,
    /// Starting food for newborn
    #[serde(default = "default_offspring_starting_food")]
    pub offspring_starting_food: u32,
    /// Minimum health to reproduce
    #[serde(default = "default_min_health_to_reproduce")]
    pub min_health_to_reproduce: f64,
    /// Minimum energy to reproduce
    #[serde(default = "default_min_energy_to_reproduce")]
    pub min_energy_to_reproduce: f64,
}

impl Default for ReproductionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gestation_period: 10,
            mating_food_cost: 5,
            gestation_energy_drain: 0.02,
            courtship_threshold: 0.7,
            courtship_increment: 0.15,
            courtship_decay: 0.05,
            mating_cooldown: 20,
            offspring_starting_food: 5,
            min_health_to_reproduce: 0.5,
            min_energy_to_reproduce: 0.4,
        }
    }
}

fn default_reproduction_enabled() -> bool { true }
fn default_gestation_period() -> usize { 10 }
fn default_mating_food_cost() -> u32 { 5 }
fn default_gestation_energy_drain() -> f64 { 0.02 }
fn default_courtship_threshold() -> f64 { 0.7 }
fn default_courtship_increment() -> f64 { 0.15 }
fn default_courtship_decay() -> f64 { 0.05 }
fn default_mating_cooldown() -> usize { 20 }
fn default_offspring_starting_food() -> u32 { 5 }
fn default_min_health_to_reproduce() -> f64 { 0.5 }
fn default_min_energy_to_reproduce() -> f64 { 0.4 }

/// Aging system configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AgingConfig {
    /// Whether aging is enabled
    #[serde(default = "default_aging_enabled")]
    pub enabled: bool,
    /// End of youth period (still developing)
    #[serde(default = "default_youth_end")]
    pub youth_end: usize,
    /// End of prime period (peak performance)
    #[serde(default = "default_prime_end")]
    pub prime_end: usize,
    /// Start of elderly period (when death probability begins)
    #[serde(default = "default_elderly_start")]
    pub elderly_start: usize,
    /// Maximum lifespan (certain death)
    #[serde(default = "default_max_lifespan")]
    pub max_lifespan: usize,
    /// Base probability of death per epoch after elderly_start
    #[serde(default = "default_death_probability_rate")]
    pub death_probability_rate: f64,
    /// Whether age affects action effectiveness
    #[serde(default = "default_capability_affects_actions")]
    pub capability_affects_actions: bool,
}

impl Default for AgingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            youth_end: 15,
            prime_end: 60,
            elderly_start: 60,
            max_lifespan: 150,
            death_probability_rate: 0.02,
            capability_affects_actions: true,
        }
    }
}

fn default_aging_enabled() -> bool { true }
fn default_youth_end() -> usize { 15 }
fn default_prime_end() -> usize { 60 }
fn default_elderly_start() -> usize { 60 }
fn default_max_lifespan() -> usize { 150 }
fn default_death_probability_rate() -> f64 { 0.02 }
fn default_capability_affects_actions() -> bool { true }

/// Skills system configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SkillsConfig {
    /// Whether skills are enabled
    #[serde(default = "default_skills_enabled")]
    pub enabled: bool,
    /// Base learning rate when taught
    #[serde(default = "default_learning_rate")]
    pub learning_rate: f64,
    /// Multiplier for teaching effectiveness
    #[serde(default = "default_teaching_multiplier")]
    pub teaching_multiplier: f64,
    /// Improvement per practice action
    #[serde(default = "default_practice_improvement")]
    pub practice_improvement: f64,
    /// Decay rate for unused skills
    #[serde(default = "default_decay_rate")]
    pub decay_rate: f64,
    /// Epochs before skill decay begins
    #[serde(default = "default_decay_threshold_epochs")]
    pub decay_threshold_epochs: usize,
    /// Minimum skill level to teach
    #[serde(default = "default_min_level_to_teach")]
    pub min_level_to_teach: f64,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            learning_rate: 0.1,
            teaching_multiplier: 1.5,
            practice_improvement: 0.02,
            decay_rate: 0.005,
            decay_threshold_epochs: 30,
            min_level_to_teach: 0.5,
        }
    }
}

fn default_skills_enabled() -> bool { true }
fn default_learning_rate() -> f64 { 0.1 }
fn default_teaching_multiplier() -> f64 { 1.5 }
fn default_practice_improvement() -> f64 { 0.02 }
fn default_decay_rate() -> f64 { 0.005 }
fn default_decay_threshold_epochs() -> usize { 30 }
fn default_min_level_to_teach() -> f64 { 0.5 }

/// Trade system configuration
#[derive(Debug, Clone, Deserialize)]
pub struct TradeConfig {
    /// Whether trading is enabled
    #[serde(default = "default_trade_enabled")]
    pub enabled: bool,
    /// Default epochs until proposal expires
    #[serde(default = "default_proposal_expiry")]
    pub proposal_expiry_epochs: usize,
    /// Maximum concurrent proposals per agent
    #[serde(default = "default_max_proposals")]
    pub max_pending_proposals: usize,
    /// Trust penalty for declining trades
    #[serde(default = "default_decline_trust_penalty")]
    pub decline_trust_penalty: f64,
    /// Trust penalty for reneging on promises
    #[serde(default = "default_renege_trust_penalty")]
    pub renege_trust_penalty: f64,
    /// Trust bonus for fulfilling promises
    #[serde(default = "default_fulfill_trust_bonus")]
    pub fulfill_trust_bonus: f64,
    /// Default deadline for promises (epochs from trade)
    #[serde(default = "default_promise_deadline")]
    pub default_promise_deadline: usize,
}

impl Default for TradeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            proposal_expiry_epochs: 5,
            max_pending_proposals: 3,
            decline_trust_penalty: 0.05,
            renege_trust_penalty: 0.5,
            fulfill_trust_bonus: 0.15,
            default_promise_deadline: 20,
        }
    }
}

fn default_trade_enabled() -> bool { true }
fn default_proposal_expiry() -> usize { 5 }
fn default_max_proposals() -> usize { 3 }
fn default_decline_trust_penalty() -> f64 { 0.05 }
fn default_renege_trust_penalty() -> f64 { 0.5 }
fn default_fulfill_trust_bonus() -> f64 { 0.15 }
fn default_promise_deadline() -> usize { 20 }

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
            environment: None,
            reproduction: ReproductionConfig::default(),
            aging: AgingConfig::default(),
            skills: SkillsConfig::default(),
            trade: TradeConfig::default(),
        }
    }
}
