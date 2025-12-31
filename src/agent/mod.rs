pub mod beliefs;
pub mod identity;
pub mod memory;

pub use beliefs::Beliefs;
pub use identity::{Aspiration, Identity, Personality, Value};
pub use memory::{Episode, EpisodeCategory, Memory};

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::config::AgingConfig;

/// A single agent in the simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub identity: Identity,
    pub beliefs: Beliefs,
    pub memory: Memory,
    pub physical: PhysicalState,
    pub active_goal: Option<Goal>,
    pub reproduction: ReproductionState,
    pub skills: Skills,
}

/// Reproduction state for an agent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReproductionState {
    /// Courtship progress with other agents: partner_id -> score (0.0-1.0)
    pub courtship_progress: HashMap<Uuid, f64>,
    /// Current gestation (if pregnant/expecting)
    pub gestation: Option<Gestation>,
    /// Family relationships
    pub family: FamilyRelations,
    /// Cooldown epochs before can mate again
    pub mating_cooldown: usize,
}

/// Active gestation state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gestation {
    /// Partner who contributed to conception
    pub partner_id: Uuid,
    /// Epoch when conception occurred
    pub conception_epoch: usize,
    /// Epoch when birth will occur
    pub expected_birth_epoch: usize,
    /// Pre-determined offspring identity (computed at conception)
    pub offspring_identity: Identity,
    /// Pre-determined offspring name
    pub offspring_name: String,
}

/// Family relationship tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FamilyRelations {
    /// Parent UUIDs (0, 1, or 2 parents)
    pub parents: Vec<Uuid>,
    /// Children UUIDs
    pub children: Vec<Uuid>,
    /// Mate history
    pub mate_history: Vec<Uuid>,
    /// Generation number (0 for originals, increments for offspring)
    pub generation: usize,
}

/// Skills and proficiencies for an agent
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Skills {
    /// Skill name -> level (0.0 to 1.0)
    pub levels: HashMap<String, f64>,
    /// Last epoch each skill was practiced
    pub last_practiced: HashMap<String, usize>,
}

impl Skills {
    /// Create skills based on personality traits
    pub fn from_personality(personality: &Personality) -> Self {
        let mut levels = HashMap::new();
        let mut rng = rand::rng();

        // High openness → foraging (curiosity, exploration)
        if personality.openness > 0.6 {
            let level = 0.15 + rng.random::<f64>() * 0.1;
            levels.insert("foraging".to_string(), level);
        }

        // High conscientiousness → crafting (discipline, patience)
        if personality.conscientiousness > 0.6 {
            let level = 0.15 + rng.random::<f64>() * 0.1;
            levels.insert("crafting".to_string(), level);
        }

        // High extraversion → leadership (social confidence)
        if personality.extraversion > 0.6 {
            let level = 0.15 + rng.random::<f64>() * 0.1;
            levels.insert("leadership".to_string(), level);
        }

        // High agreeableness → teaching (empathy, patience)
        if personality.agreeableness > 0.6 {
            let level = 0.15 + rng.random::<f64>() * 0.1;
            levels.insert("teaching".to_string(), level);
        }

        // Low neuroticism → hunting (calm under pressure)
        if personality.neuroticism < 0.4 {
            let level = 0.15 + rng.random::<f64>() * 0.1;
            levels.insert("hunting".to_string(), level);
        }

        // Also give a small diplomacy bonus if agreeable or extraverted
        if personality.agreeableness > 0.5 || personality.extraversion > 0.5 {
            let level = 0.1 + rng.random::<f64>() * 0.1;
            levels.insert("diplomacy".to_string(), level);
        }

        Self {
            levels,
            last_practiced: HashMap::new(),
        }
    }

    /// Inherit skills from parents (average * 0.3) plus personality bonus
    pub fn from_parents(parent_a: &Skills, parent_b: &Skills, personality: &Personality) -> Self {
        let mut skills = Skills::from_personality(personality);

        // Collect all skill names from both parents
        let mut all_skills: std::collections::HashSet<String> = std::collections::HashSet::new();
        for name in parent_a.levels.keys() {
            all_skills.insert(name.clone());
        }
        for name in parent_b.levels.keys() {
            all_skills.insert(name.clone());
        }

        // Inherit at 30% of parent average
        for name in all_skills {
            let level_a = parent_a.levels.get(&name).copied().unwrap_or(0.0);
            let level_b = parent_b.levels.get(&name).copied().unwrap_or(0.0);
            let inherited = (level_a + level_b) / 2.0 * 0.3;

            // Add to existing personality-based skill or set new
            let current = skills.levels.get(&name).copied().unwrap_or(0.0);
            skills.levels.insert(name, (current + inherited).min(1.0));
        }

        skills
    }

    /// Get skill level (0.0 if not known)
    pub fn level(&self, skill: &str) -> f64 {
        self.levels.get(skill).copied().unwrap_or(0.0)
    }

    /// Check if agent can teach a skill (level >= 0.5)
    pub fn can_teach(&self, skill: &str) -> bool {
        self.level(skill) >= 0.5
    }

    /// Get all teachable skills
    pub fn teachable_skills(&self) -> Vec<&String> {
        self.levels
            .iter()
            .filter(|(_, level)| **level >= 0.5)
            .map(|(name, _)| name)
            .collect()
    }

    /// Improve a skill (capped at 1.0)
    pub fn improve(&mut self, skill: &str, amount: f64, epoch: usize) {
        let current = self.levels.get(skill).copied().unwrap_or(0.0);
        self.levels.insert(skill.to_string(), (current + amount).min(1.0));
        self.last_practiced.insert(skill.to_string(), epoch);
    }

    /// Mark a skill as practiced this epoch
    pub fn practice(&mut self, skill: &str, epoch: usize) {
        if self.levels.contains_key(skill) {
            self.last_practiced.insert(skill.to_string(), epoch);
        }
    }

    /// Get skill level description
    pub fn level_description(level: f64) -> &'static str {
        if level >= 0.9 {
            "master"
        } else if level >= 0.7 {
            "expert"
        } else if level >= 0.5 {
            "skilled"
        } else if level >= 0.2 {
            "competent"
        } else {
            "novice"
        }
    }
}

/// Physical state of an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalState {
    pub x: usize,
    pub y: usize,
    /// 0.0 (dead) to 1.0 (healthy)
    pub health: f64,
    /// 0.0 (full) to 1.0 (starving)
    pub hunger: f64,
    /// 0.0 (exhausted) to 1.0 (rested)
    pub energy: f64,
    /// Food carried
    pub food: u32,
    /// Age in epochs
    pub age: usize,
}

/// Current active goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Goal {
    FindFood,
    Eat,
    Rest,
    Explore,
    Socialize,
    Flee,
    Custom(String),
}

impl Agent {
    /// Create a new agent with random identity at the given position
    pub fn new(name: String, x: usize, y: usize, starting_food: u32) -> Self {
        let identity = Identity::new(name);
        let skills = Skills::from_personality(&identity.personality);
        Self {
            id: Uuid::new_v4(),
            identity,
            beliefs: Beliefs::new(),
            memory: Memory::new(),
            physical: PhysicalState {
                x,
                y,
                health: 1.0,
                hunger: 0.3, // Slightly hungry to start
                energy: 1.0,
                food: starting_food,
                age: 0,
            },
            active_goal: Some(Goal::Explore),
            reproduction: ReproductionState::default(),
            skills,
        }
    }

    /// Create a new agent with a pre-determined identity (for offspring)
    pub fn new_with_identity(
        identity: Identity,
        x: usize,
        y: usize,
        starting_food: u32,
        parents: Vec<Uuid>,
        generation: usize,
        parent_skills: Option<(&Skills, &Skills)>,
    ) -> Self {
        // Skills: inherit from parents if available, otherwise from personality
        let skills = match parent_skills {
            Some((parent_a, parent_b)) => {
                Skills::from_parents(parent_a, parent_b, &identity.personality)
            }
            None => Skills::from_personality(&identity.personality),
        };

        Self {
            id: Uuid::new_v4(),
            identity,
            beliefs: Beliefs::new(),
            memory: Memory::new(),
            physical: PhysicalState {
                x,
                y,
                health: 1.0,
                hunger: 0.2, // Newborns start less hungry
                energy: 0.8,
                food: starting_food,
                age: 0,
            },
            active_goal: Some(Goal::Explore),
            reproduction: ReproductionState {
                family: FamilyRelations {
                    parents,
                    children: Vec::new(),
                    mate_history: Vec::new(),
                    generation,
                },
                ..Default::default()
            },
            skills,
        }
    }

    /// Check if agent is alive
    pub fn is_alive(&self) -> bool {
        self.physical.health > 0.0
    }

    /// Get agent's name
    pub fn name(&self) -> &str {
        &self.identity.name
    }

    /// Get agent's age
    pub fn age(&self) -> usize {
        self.physical.age
    }

    /// Calculate age-based capability modifier (0.5 to 1.0)
    /// Youth: 0.7 to 1.0, Prime: 1.0, Elderly/Ancient: 1.0 to 0.5
    pub fn age_modifier(&self, config: &AgingConfig) -> f64 {
        if !config.enabled || !config.capability_affects_actions {
            return 1.0;
        }

        let age = self.physical.age;

        if age < config.youth_end {
            // Youth: starts at 0.7, grows to 1.0 by end of youth
            0.7 + 0.3 * (age as f64 / config.youth_end as f64)
        } else if age < config.prime_end {
            // Prime: 100% capability
            1.0
        } else if age < config.max_lifespan {
            // Elderly/Ancient: linear decline from 1.0 to 0.5
            let decline_progress = (age - config.prime_end) as f64
                / (config.max_lifespan - config.prime_end) as f64;
            1.0 - (decline_progress * 0.5)
        } else {
            // Beyond max lifespan (shouldn't happen, but cap at 0.5)
            0.5
        }
    }

    /// Get the current life stage as a string
    pub fn life_stage(&self, config: &AgingConfig) -> &'static str {
        if !config.enabled {
            return "ageless";
        }

        let age = self.physical.age;

        if age < config.youth_end {
            "youth"
        } else if age < config.prime_end {
            "prime"
        } else if age < config.elderly_start + (config.max_lifespan - config.elderly_start) / 2 {
            "elderly"
        } else {
            "ancient"
        }
    }

    /// Update hunger (called each epoch)
    pub fn tick_hunger(&mut self) {
        // Hunger increases by 0.1 per epoch
        self.physical.hunger = (self.physical.hunger + 0.1).min(1.0);

        // High hunger damages health
        if self.physical.hunger > 0.8 {
            self.physical.health -= 0.1;
        }
    }

    /// Update energy (slight natural drain)
    pub fn tick_energy(&mut self) {
        self.physical.energy = (self.physical.energy - 0.05).max(0.0);
    }

    /// Eat food from inventory
    pub fn eat(&mut self) -> bool {
        if self.physical.food > 0 {
            self.physical.food -= 1;
            self.physical.hunger = (self.physical.hunger - 0.3).max(0.0);
            self.physical.health = (self.physical.health + 0.05).min(1.0);
            true
        } else {
            false
        }
    }

    /// Rest to recover energy
    pub fn rest(&mut self) {
        self.physical.energy = (self.physical.energy + 0.3).min(1.0);
    }

    /// Take damage
    pub fn take_damage(&mut self, amount: f64) {
        self.physical.health = (self.physical.health - amount).max(0.0);
    }

    /// Add food to inventory
    pub fn add_food(&mut self, amount: u32) {
        self.physical.food += amount;
    }

    /// Remove food from inventory (returns actual amount removed)
    pub fn remove_food(&mut self, amount: u32) -> u32 {
        let removed = amount.min(self.physical.food);
        self.physical.food -= removed;
        removed
    }

    /// Generate the full state summary for LLM prompting
    pub fn prompt_state(&self, epoch: usize) -> String {
        // Physical state
        let health_desc = if self.physical.health > 0.8 {
            "healthy"
        } else if self.physical.health > 0.5 {
            "somewhat injured"
        } else if self.physical.health > 0.2 {
            "badly hurt"
        } else {
            "near death"
        };

        let hunger_desc = if self.physical.hunger < 0.2 {
            "well-fed"
        } else if self.physical.hunger < 0.5 {
            "slightly hungry"
        } else if self.physical.hunger < 0.8 {
            "hungry"
        } else {
            "starving"
        };

        let energy_desc = if self.physical.energy > 0.7 {
            "energetic"
        } else if self.physical.energy > 0.4 {
            "a bit tired"
        } else if self.physical.energy > 0.2 {
            "exhausted"
        } else {
            "barely able to move"
        };

        // Age description
        let aging_config = AgingConfig::default();
        let life_stage = self.life_stage(&aging_config);
        let age_desc = match life_stage {
            "youth" => format!("You are young ({} days old), still developing your strength", self.physical.age),
            "prime" => format!("You are in your prime ({} days old), at peak capability", self.physical.age),
            "elderly" => format!("You are elderly ({} days old), feeling your age", self.physical.age),
            "ancient" => format!("You are ancient ({} days old), your body is failing", self.physical.age),
            _ => format!("You are {} days old", self.physical.age),
        };

        let physical = format!(
            "Physical state: {}. You are {}, {}, and {}. You carry {} food.",
            age_desc, health_desc, hunger_desc, energy_desc, self.physical.food
        );

        let goal = match &self.active_goal {
            Some(g) => format!("Current focus: {}", g.describe()),
            None => "You have no particular goal right now.".to_string(),
        };

        // Reproduction state
        let mut reproduction_parts = Vec::new();

        if let Some(gestation) = &self.reproduction.gestation {
            let days_left = gestation.expected_birth_epoch.saturating_sub(epoch);
            reproduction_parts.push(format!(
                "You are expecting a child in {} days",
                days_left
            ));
        }

        if !self.reproduction.courtship_progress.is_empty() {
            let courtships: Vec<String> = self
                .reproduction
                .courtship_progress
                .iter()
                .map(|(_, score)| format!("{:.0}%", score * 100.0))
                .collect();
            reproduction_parts.push(format!(
                "You have {} active courtships",
                courtships.len()
            ));
        }

        if self.reproduction.mating_cooldown > 0 {
            reproduction_parts.push(format!(
                "You need {} days before you can mate again",
                self.reproduction.mating_cooldown
            ));
        }

        if self.reproduction.family.children.len() > 0 {
            reproduction_parts.push(format!(
                "You have {} children",
                self.reproduction.family.children.len()
            ));
        }

        let reproduction = if reproduction_parts.is_empty() {
            String::new()
        } else {
            format!("\nReproduction: {}", reproduction_parts.join(". "))
        };

        // Skills summary
        let skills = self.skills_prompt_summary();

        format!(
            "{}\n\n{}{}\n\n{}\n\n{}\n\n{}\n\n{}",
            self.identity.prompt_description(),
            physical,
            reproduction,
            skills,
            goal,
            self.beliefs.prompt_summary(epoch),
            self.memory.prompt_summary(epoch),
        )
    }

    /// Generate skills summary for LLM prompting
    fn skills_prompt_summary(&self) -> String {
        let skill_tiers: Vec<String> = self
            .skills
            .levels
            .iter()
            .filter(|(_, level)| **level >= 0.2)
            .map(|(name, level)| {
                let tier = if *level >= 0.9 {
                    "master"
                } else if *level >= 0.7 {
                    "expert"
                } else if *level >= 0.5 {
                    "skilled"
                } else if *level >= 0.2 {
                    "competent"
                } else {
                    "novice"
                };
                format!("{} ({})", name, tier)
            })
            .collect();

        if skill_tiers.is_empty() {
            "You have no developed skills yet.".to_string()
        } else {
            format!("Your skills: {}", skill_tiers.join(", "))
        }
    }

    /// Determine a new goal based on current state
    pub fn update_goal(&mut self) {
        // Priority: survival first
        if self.physical.hunger > 0.7 {
            if self.physical.food > 0 {
                self.active_goal = Some(Goal::Eat);
            } else {
                self.active_goal = Some(Goal::FindFood);
            }
        } else if self.physical.energy < 0.2 {
            self.active_goal = Some(Goal::Rest);
        } else if self.physical.health < 0.3 {
            self.active_goal = Some(Goal::Rest);
        } else {
            // Non-urgent: based on personality
            let mut rng = rand::rng();
            if self.identity.personality.extraversion > 0.6 && rng.random::<f64>() < 0.3 {
                self.active_goal = Some(Goal::Socialize);
            } else {
                self.active_goal = Some(Goal::Explore);
            }
        }
    }
}

impl Goal {
    pub fn describe(&self) -> &str {
        match self {
            Goal::FindFood => "finding food",
            Goal::Eat => "eating",
            Goal::Rest => "resting",
            Goal::Explore => "exploring",
            Goal::Socialize => "meeting others",
            Goal::Flee => "escaping danger",
            Goal::Custom(s) => s,
        }
    }
}

/// Names for generating agents
const NAMES: &[&str] = &[
    "Aric", "Bria", "Corin", "Dara", "Elwyn", "Faye", "Garen", "Hana", "Isen", "Jora",
    "Kael", "Lira", "Maren", "Niko", "Orin", "Petra", "Quinn", "Rhea", "Soren", "Talia",
];

/// Generate N unique agent names
pub fn generate_names(count: usize) -> Vec<String> {
    let mut names: Vec<String> = NAMES.iter().map(|s| s.to_string()).collect();
    let mut rng = rand::rng();

    // Shuffle
    for i in (1..names.len()).rev() {
        let j = rng.random_range(0..=i);
        names.swap(i, j);
    }

    names.into_iter().take(count).collect()
}

/// Generate a unique offspring name based on parents
pub fn generate_offspring_name(parent_a_name: &str, parent_b_name: &str, existing_names: &[String]) -> String {
    let mut rng = rand::rng();

    // First try: unused names from the pool
    let unused: Vec<_> = NAMES
        .iter()
        .filter(|n| !existing_names.iter().any(|e| e.eq_ignore_ascii_case(n)))
        .collect();

    if !unused.is_empty() {
        return unused[rng.random_range(0..unused.len())].to_string();
    }

    // Fallback: blend parent names (first 2-3 chars of each)
    let prefix_len_a = parent_a_name.len().min(2);
    let prefix_len_b = parent_b_name.len().min(3);
    let prefix_a = &parent_a_name[..prefix_len_a];
    let suffix_b = &parent_b_name[parent_b_name.len().saturating_sub(prefix_len_b)..];

    let mut blended = format!("{}{}", prefix_a, suffix_b.to_lowercase());

    // Capitalize first letter
    if let Some(first) = blended.get_mut(0..1) {
        first.make_ascii_uppercase();
    }

    // Ensure uniqueness with suffix if needed
    if existing_names.iter().any(|e| e.eq_ignore_ascii_case(&blended)) {
        blended = format!("{}-{}", blended, rng.random_range(1..100));
    }

    blended
}
