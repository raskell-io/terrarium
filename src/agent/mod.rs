pub mod beliefs;
pub mod identity;
pub mod memory;

pub use beliefs::Beliefs;
pub use identity::{Aspiration, Identity, Personality, Value};
pub use memory::{Episode, EpisodeCategory, Memory};

use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single agent in the simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub identity: Identity,
    pub beliefs: Beliefs,
    pub memory: Memory,
    pub physical: PhysicalState,
    pub active_goal: Option<Goal>,
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
        Self {
            id: Uuid::new_v4(),
            identity: Identity::new(name),
            beliefs: Beliefs::new(),
            memory: Memory::new(),
            physical: PhysicalState {
                x,
                y,
                health: 1.0,
                hunger: 0.3, // Slightly hungry to start
                energy: 1.0,
                food: starting_food,
            },
            active_goal: Some(Goal::Explore),
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

        let physical = format!(
            "Physical state: You are {}, {}, and {}. You carry {} food.",
            health_desc, hunger_desc, energy_desc, self.physical.food
        );

        let goal = match &self.active_goal {
            Some(g) => format!("Current focus: {}", g.describe()),
            None => "You have no particular goal right now.".to_string(),
        };

        format!(
            "{}\n\n{}\n\n{}\n\n{}\n\n{}",
            self.identity.prompt_description(),
            physical,
            goal,
            self.beliefs.prompt_summary(epoch),
            self.memory.prompt_summary(epoch),
        )
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
