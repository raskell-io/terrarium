//! Observer protocol types for viewing simulation state.
//!
//! This module defines the view types that clients use to observe the simulation.
//! The views are read-only snapshots that decouple clients from engine internals.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::{Agent, Goal};
use crate::observation::{Event, EventType};
use crate::world::{Terrain, World};

/// View of the entire world state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldView {
    pub epoch: usize,
    pub width: usize,
    pub height: usize,
    pub cells: Vec<CellView>,
}

/// View of a single cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellView {
    pub x: usize,
    pub y: usize,
    pub terrain: Terrain,
    pub food: u32,
    pub occupants: Vec<Uuid>,
}

/// View of an agent's state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentView {
    pub id: Uuid,
    pub name: String,
    pub position: (usize, usize),

    // Physical
    pub health: f64,
    pub hunger: f64,
    pub energy: f64,
    pub food: u32,
    pub alive: bool,

    // Identity
    pub personality_summary: String,
    pub aspiration: String,

    // Cognitive
    pub current_goal: Option<String>,
    pub recent_memories: Vec<String>,
    pub social_beliefs: Vec<SocialBeliefView>,
}

/// View of a social belief
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialBeliefView {
    pub about: String,
    pub trust: f64,
    pub sentiment: f64,
}

/// View of an event for display
#[derive(Debug, Clone)]
pub struct EventView {
    pub epoch: usize,
    pub description: String,
    pub event_type: EventViewType,
}

/// Simplified event types for display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventViewType {
    Movement,
    Gathering,
    Eating,
    Resting,
    Speech,
    Gift,
    Attack,
    Death,
    Gossip,
    GroupFormed,
    GroupDissolved,
    GroupChanged,
    Meta,
}

impl WorldView {
    /// Create a world view from the world and agents
    pub fn from_world(world: &World, agents: &[Agent]) -> Self {
        let mut cells = Vec::with_capacity(world.cells.len());

        for cell in &world.cells {
            let occupants: Vec<Uuid> = agents
                .iter()
                .filter(|a| a.is_alive() && a.physical.x == cell.x && a.physical.y == cell.y)
                .map(|a| a.id)
                .collect();

            cells.push(CellView {
                x: cell.x,
                y: cell.y,
                terrain: cell.terrain,
                food: cell.food,
                occupants,
            });
        }

        Self {
            epoch: world.epoch,
            width: world.width,
            height: world.height,
            cells,
        }
    }

    /// Get cell at coordinates
    pub fn get(&self, x: usize, y: usize) -> Option<&CellView> {
        if x < self.width && y < self.height {
            Some(&self.cells[y * self.width + x])
        } else {
            None
        }
    }
}

impl AgentView {
    /// Create an agent view from an agent
    pub fn from_agent(agent: &Agent, agents: &[Agent]) -> Self {
        // Build personality summary
        let p = &agent.identity.personality;
        let mut traits = Vec::new();
        if p.openness > 0.6 {
            traits.push("curious");
        } else if p.openness < 0.4 {
            traits.push("practical");
        }
        if p.conscientiousness > 0.6 {
            traits.push("disciplined");
        } else if p.conscientiousness < 0.4 {
            traits.push("spontaneous");
        }
        if p.extraversion > 0.6 {
            traits.push("outgoing");
        } else if p.extraversion < 0.4 {
            traits.push("reserved");
        }
        if p.agreeableness > 0.6 {
            traits.push("cooperative");
        } else if p.agreeableness < 0.4 {
            traits.push("competitive");
        }
        if p.neuroticism > 0.6 {
            traits.push("anxious");
        } else if p.neuroticism < 0.4 {
            traits.push("calm");
        }
        let personality_summary = if traits.is_empty() {
            "balanced temperament".to_string()
        } else {
            traits.join(", ")
        };

        // Build social beliefs
        let social_beliefs: Vec<SocialBeliefView> = agent
            .beliefs
            .social
            .iter()
            .map(|(id, belief)| {
                let about = agents
                    .iter()
                    .find(|a| a.id == *id)
                    .map(|a| a.name().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                SocialBeliefView {
                    about,
                    trust: belief.trust,
                    sentiment: belief.sentiment,
                }
            })
            .collect();

        // Get recent memories
        let recent_memories: Vec<String> = agent
            .memory
            .recent
            .iter()
            .rev()
            .take(5)
            .map(|e| format!("Day {}: {}", e.epoch, e.description))
            .collect();

        Self {
            id: agent.id,
            name: agent.name().to_string(),
            position: (agent.physical.x, agent.physical.y),
            health: agent.physical.health,
            hunger: agent.physical.hunger,
            energy: agent.physical.energy,
            food: agent.physical.food,
            alive: agent.is_alive(),
            personality_summary,
            aspiration: agent.identity.aspiration.describe().to_string(),
            current_goal: agent.active_goal.as_ref().map(|g| g.describe().to_string()),
            recent_memories,
            social_beliefs,
        }
    }
}

impl EventView {
    /// Create event views from raw events, resolving agent names
    pub fn from_events(events: &[Event], agents: &[Agent]) -> Vec<Self> {
        events
            .iter()
            .filter_map(|e| Self::from_event(e, agents))
            .collect()
    }

    fn from_event(event: &Event, agents: &[Agent]) -> Option<Self> {
        let agent_name = |id: Uuid| {
            agents
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.name().to_string())
                .unwrap_or_else(|| "Unknown".to_string())
        };

        let (description, event_type) = match &event.event_type {
            EventType::EpochStart => return None,
            EventType::EpochEnd => return None,
            EventType::Moved => {
                let name = agent_name(event.agent?);
                let to = event.data.to?;
                (
                    format!("{} moved to ({}, {})", name, to.0, to.1),
                    EventViewType::Movement,
                )
            }
            EventType::Gathered => {
                let name = agent_name(event.agent?);
                let amount = event.data.amount?;
                (
                    format!("{} gathered {} food", name, amount),
                    EventViewType::Gathering,
                )
            }
            EventType::Ate => {
                let name = agent_name(event.agent?);
                (format!("{} ate", name), EventViewType::Eating)
            }
            EventType::Rested => {
                let name = agent_name(event.agent?);
                (format!("{} rested", name), EventViewType::Resting)
            }
            EventType::Spoke => {
                let name = agent_name(event.agent?);
                let target_name = agent_name(event.target?);
                let message = event.data.message.as_deref().unwrap_or("");
                (
                    format!("{} to {}: \"{}\"", name, target_name, message),
                    EventViewType::Speech,
                )
            }
            EventType::Gave => {
                let name = agent_name(event.agent?);
                let target_name = agent_name(event.target?);
                let amount = event.data.amount?;
                (
                    format!("{} gave {} food to {}", name, amount, target_name),
                    EventViewType::Gift,
                )
            }
            EventType::Attacked => {
                let name = agent_name(event.agent?);
                let target_name = agent_name(event.target?);
                (
                    format!("{} attacked {}!", name, target_name),
                    EventViewType::Attack,
                )
            }
            EventType::Died => {
                let name = agent_name(event.agent?);
                let cause = event.data.description.as_deref().unwrap_or("unknown causes");
                (
                    format!("{} died from {}", name, cause),
                    EventViewType::Death,
                )
            }
            EventType::Gossiped => {
                let name = agent_name(event.agent?);
                let target_name = agent_name(event.target?);
                let about_name = event.data.about.map(agent_name).unwrap_or_else(|| "someone".to_string());
                let sentiment = event.data.description.as_deref().unwrap_or("neutral");
                (
                    format!("{} told {} ({}) things about {}", name, target_name, sentiment, about_name),
                    EventViewType::Gossip,
                )
            }
            EventType::HealthChanged => {
                return None;
            }
            EventType::GroupFormed => {
                let group_name = event.data.group_name.as_deref().unwrap_or("Unknown");
                let member_count = event.data.members.as_ref().map(|m| m.len()).unwrap_or(0);
                (
                    format!("{} formed with {} members", group_name, member_count),
                    EventViewType::GroupFormed,
                )
            }
            EventType::GroupDissolved => {
                let group_name = event.data.group_name.as_deref().unwrap_or("Unknown");
                (
                    format!("{} dissolved", group_name),
                    EventViewType::GroupDissolved,
                )
            }
            EventType::GroupChanged => {
                let group_name = event.data.group_name.as_deref().unwrap_or("Unknown");
                let description = event.data.description.as_deref().unwrap_or("membership changed");
                (
                    format!("{}: {}", group_name, description),
                    EventViewType::GroupChanged,
                )
            }
        };

        Some(Self {
            epoch: event.epoch,
            description,
            event_type,
        })
    }
}

/// Simulation control commands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationCommand {
    Pause,
    Resume,
    Step,
    SetSpeed(u32), // ms per epoch
    Stop,
}
