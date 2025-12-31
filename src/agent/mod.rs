mod memory;
mod personality;
mod relations;

pub use memory::{Memory, Episode};
pub use personality::Personality;
pub use relations::{Relationship, RelationshipType, SocialGraph};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single agent in the simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub body: Body,
    pub mind: Mind,
    pub memory: Memory,
    pub relations: SocialGraph,
    pub created_epoch: usize,
}

/// Physical characteristics and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Body {
    /// Current position in the world
    pub position: (usize, usize),
    /// Age in epochs
    pub age: usize,
    /// Health (0.0 - 1.0)
    pub health: f64,
    /// Energy/stamina (0.0 - 1.0)
    pub energy: f64,
    /// Hunger level (0.0 = full, 1.0 = starving)
    pub hunger: f64,
    /// Physical strength (affects carrying capacity, combat)
    pub strength: f64,
    /// Movement speed
    pub speed: f64,
    /// Resources currently held
    pub inventory: Inventory,
}

/// The agent's mind: personality, values, goals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mind {
    /// Big Five personality traits
    pub personality: Personality,
    /// Current primary goal
    pub current_goal: Option<Goal>,
    /// Risk tolerance (0.0 = very risk averse, 1.0 = risk seeking)
    pub risk_tolerance: f64,
    /// Time preference (0.0 = long-term thinking, 1.0 = immediate gratification)
    pub time_preference: f64,
    /// What this agent values most
    pub values: Vec<Value>,
}

/// Things an agent might value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    Survival,
    Wealth,
    Status,
    Relationships,
    Knowledge,
    Freedom,
    Power,
    Comfort,
}

/// Possible goals an agent might pursue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Goal {
    Survive,
    AccumulateResource(String),
    FindFood,
    FindShelter,
    FormAlliance(Uuid),
    Trade,
    Explore,
    Rest,
    Socialize,
    Dominate,
    Help(Uuid),
}

/// What an agent is carrying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub food: u32,
    pub materials: u32,
    pub tools: u32,
    pub other: Vec<(String, u32)>,
}

impl Inventory {
    pub fn total_weight(&self) -> u32 {
        self.food + self.materials * 2 + self.tools * 3
    }
}

/// Actions an agent can take
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Do nothing this epoch
    Wait,
    /// Move to an adjacent cell
    Move { direction: Direction },
    /// Gather resources from current location
    Gather,
    /// Consume food to reduce hunger
    Eat,
    /// Rest to recover energy
    Rest,
    /// Attempt to trade with another agent
    Trade {
        target: Uuid,
        offer: (String, u32),
        request: (String, u32)
    },
    /// Communicate with another agent
    Speak {
        target: Uuid,
        message: String
    },
    /// Attack another agent
    Attack { target: Uuid },
    /// Give resources to another agent
    Give {
        target: Uuid,
        resource: String,
        amount: u32
    },
    /// Build or craft something
    Build { item: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Direction {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Agent {
    pub fn new(name: String, position: (usize, usize), personality: Personality, epoch: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            body: Body {
                position,
                age: 0,
                health: 1.0,
                energy: 1.0,
                hunger: 0.3,
                strength: 0.5 + rand::random::<f64>() * 0.3,
                speed: 0.5 + rand::random::<f64>() * 0.3,
                inventory: Inventory::default(),
            },
            mind: Mind {
                personality,
                current_goal: Some(Goal::Survive),
                risk_tolerance: rand::random(),
                time_preference: rand::random(),
                values: vec![Value::Survival, Value::Relationships],
            },
            memory: Memory::new(),
            relations: SocialGraph::new(),
            created_epoch: epoch,
        }
    }

    /// Check if the agent is still alive
    pub fn is_alive(&self) -> bool {
        self.body.health > 0.0
    }

    /// Get a summary of the agent's current state for LLM prompting
    pub fn state_summary(&self) -> String {
        format!(
            "Name: {}\nAge: {} epochs\nHealth: {:.0}%\nEnergy: {:.0}%\nHunger: {:.0}%\n\
             Position: ({}, {})\nInventory: {} food, {} materials, {} tools\n\
             Personality: {}\nCurrent goal: {:?}",
            self.name,
            self.body.age,
            self.body.health * 100.0,
            self.body.energy * 100.0,
            self.body.hunger * 100.0,
            self.body.position.0,
            self.body.position.1,
            self.body.inventory.food,
            self.body.inventory.materials,
            self.body.inventory.tools,
            self.mind.personality.describe(),
            self.mind.current_goal,
        )
    }
}
