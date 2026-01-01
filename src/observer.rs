//! Observer protocol types for viewing simulation state.
//!
//! This module defines the view types that clients use to observe the simulation.
//! The views are read-only snapshots that decouple clients from engine internals.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::{Agent, Goal};
use crate::config::AgingConfig;
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

/// View of a structure for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureView {
    pub structure_type: String,
    pub display_name: String,
    pub is_complete: bool,
    pub build_percent: f64,
    pub owner_name: Option<String>,
}

/// View of a territory claim for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerritoryView {
    pub owner_id: Uuid,
    pub owner_name: String,
    pub strength: f64,
    pub guest_count: usize,
}

/// View of a single cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellView {
    pub x: usize,
    pub y: usize,
    pub terrain: Terrain,
    pub food: u32,
    pub occupants: Vec<Uuid>,
    pub structure: Option<StructureView>,
    pub territory: Option<TerritoryView>,
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

    // Aging
    pub age: usize,
    pub life_stage: String,
    pub generation: usize,

    // Identity
    pub personality_summary: String,
    pub aspiration: String,

    // Cognitive
    pub current_goal: Option<String>,
    pub recent_memories: Vec<String>,
    pub social_beliefs: Vec<SocialBeliefView>,

    // Reproduction
    pub reproduction: ReproductionView,

    // Skills
    pub skills: Vec<SkillView>,
}

/// View of a social belief
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialBeliefView {
    pub about: String,
    pub trust: f64,
    pub sentiment: f64,
}

/// View of reproduction state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReproductionView {
    pub is_gestating: bool,
    pub expected_birth: Option<usize>,
    pub num_children: usize,
    pub parent_names: Vec<String>,
    pub courtships: Vec<(String, f64)>,
    pub on_cooldown: bool,
}

/// View of a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillView {
    pub name: String,
    pub level: f64,
}

/// View of a trade proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeProposalView {
    pub id: Uuid,
    pub proposer_name: String,
    pub recipient_name: String,
    pub offering: String,
    pub requesting: String,
    pub expires_in: usize,
    pub status: String,
}

/// View of a service debt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDebtView {
    pub debtor_name: String,
    pub creditor_name: String,
    pub service: String,
    pub deadline_in: Option<i64>,  // Negative = overdue
    pub fulfilled: bool,
    pub is_alliance: bool,
}

/// Combined view of all trade state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeStateView {
    pub pending_proposals: Vec<TradeProposalView>,
    pub service_debts: Vec<ServiceDebtView>,
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
    AllyIntervened,
    Death,
    Gossip,
    GroupFormed,
    GroupDissolved,
    GroupChanged,
    LeadershipChanged,
    RivalryFormed,
    RivalryChanged,
    RivalryEnded,
    Courtship,
    Conception,
    Birth,
    SkillTaught,
    // Crafting
    MaterialGathering,
    Crafting,
    Hunting,
    Fishing,
    Chopping,
    ToolBroke,
    // Territory
    TerritoryMarked,
    TerritoryChallenged,
    TerritorySubmitted,
    TerritoryFight,
    TerritoryLost,
    // Structures
    FarmProduced,
    StructureDestroyed,
    // Trade
    TradeProposed,
    TradeAccepted,
    TradeDeclined,
    TradeCountered,
    TradeExpired,
    TradeCancelled,
    TradeReneged,
    ServiceFulfilled,
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

            // Build structure view if present
            let structure = cell.structure.as_ref().map(|s| {
                let owner_name = agents
                    .iter()
                    .find(|a| a.id == s.owner)
                    .map(|a| a.name().to_string());

                StructureView {
                    structure_type: format!("{:?}", s.structure_type),
                    display_name: s.display_name(),
                    is_complete: s.is_complete(),
                    build_percent: if s.build_required > 0 {
                        (s.build_progress as f64 / s.build_required as f64) * 100.0
                    } else {
                        100.0
                    },
                    owner_name,
                }
            });

            // Build territory view if present
            let territory = cell.territory.as_ref().map(|t| {
                let owner_name = agents
                    .iter()
                    .find(|a| a.id == t.owner)
                    .map(|a| a.name().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                TerritoryView {
                    owner_id: t.owner,
                    owner_name,
                    strength: t.strength,
                    guest_count: t.allowed_guests.len(),
                }
            });

            cells.push(CellView {
                x: cell.x,
                y: cell.y,
                terrain: cell.terrain,
                food: cell.food,
                occupants,
                structure,
                territory,
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
    pub fn from_agent(agent: &Agent, agents: &[Agent], aging_config: &AgingConfig) -> Self {
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

        // Build reproduction view
        let parent_names: Vec<String> = agent
            .reproduction
            .family
            .parents
            .iter()
            .filter_map(|id| agents.iter().find(|a| a.id == *id))
            .map(|a| a.name().to_string())
            .collect();

        let courtships: Vec<(String, f64)> = agent
            .reproduction
            .courtship_progress
            .iter()
            .filter_map(|(id, score)| {
                agents
                    .iter()
                    .find(|a| a.id == *id)
                    .map(|a| (a.name().to_string(), *score))
            })
            .collect();

        let reproduction = ReproductionView {
            is_gestating: agent.reproduction.gestation.is_some(),
            expected_birth: agent
                .reproduction
                .gestation
                .as_ref()
                .map(|g| g.expected_birth_epoch),
            num_children: agent.reproduction.family.children.len(),
            parent_names,
            courtships,
            on_cooldown: agent.reproduction.mating_cooldown > 0,
        };

        // Build skills view (sorted by level, highest first)
        let mut skills: Vec<SkillView> = agent
            .skills
            .levels
            .iter()
            .filter(|(_, level)| **level > 0.0)
            .map(|(name, level)| SkillView {
                name: name.clone(),
                level: *level,
            })
            .collect();
        skills.sort_by(|a, b| b.level.partial_cmp(&a.level).unwrap_or(std::cmp::Ordering::Equal));

        Self {
            id: agent.id,
            name: agent.name().to_string(),
            position: (agent.physical.x, agent.physical.y),
            health: agent.physical.health,
            hunger: agent.physical.hunger,
            energy: agent.physical.energy,
            food: agent.physical.food,
            alive: agent.is_alive(),
            age: agent.physical.age,
            life_stage: agent.life_stage(aging_config).to_string(),
            generation: agent.reproduction.family.generation,
            personality_summary,
            aspiration: agent.identity.aspiration.describe().to_string(),
            current_goal: agent.active_goal.as_ref().map(|g| g.describe().to_string()),
            recent_memories,
            social_beliefs,
            reproduction,
            skills,
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
            EventType::AllyIntervened => {
                let target_name = agent_name(event.target?);
                let ally_id = event.data.ally?;
                let ally_name = agent_name(ally_id);
                let reduction = event.data.damage_reduction.unwrap_or(0.0) * 100.0;
                (
                    format!("{} defended {} ({:.0}% damage reduction)", ally_name, target_name, reduction),
                    EventViewType::AllyIntervened,
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
            EventType::LeadershipChanged => {
                let group_name = event.data.group_name.as_deref().unwrap_or("Unknown");
                let new_leader_name = event
                    .data
                    .new_leader
                    .map(agent_name)
                    .unwrap_or_else(|| "Unknown".to_string());
                let old_leader_name = event.data.old_leader.map(agent_name);

                let description = if let Some(old_name) = old_leader_name {
                    format!("{}: {} succeeded {} as leader", group_name, new_leader_name, old_name)
                } else {
                    format!("{}: {} became leader", group_name, new_leader_name)
                };
                (description, EventViewType::LeadershipChanged)
            }
            EventType::RivalryFormed => {
                let group_a = event.data.group_name.as_deref().unwrap_or("Unknown");
                let group_b = event.data.group_b_name.as_deref().unwrap_or("Unknown");
                let rivalry_type = event.data.rivalry_type.as_deref().unwrap_or("neutral");
                (
                    format!("{} and {} are now {}", group_a, group_b, rivalry_type),
                    EventViewType::RivalryFormed,
                )
            }
            EventType::RivalryChanged => {
                let group_a = event.data.group_name.as_deref().unwrap_or("Unknown");
                let group_b = event.data.group_b_name.as_deref().unwrap_or("Unknown");
                let old_type = event.data.old_rivalry_type.as_deref().unwrap_or("neutral");
                let new_type = event.data.rivalry_type.as_deref().unwrap_or("neutral");
                (
                    format!("{} and {}: {} → {}", group_a, group_b, old_type, new_type),
                    EventViewType::RivalryChanged,
                )
            }
            EventType::RivalryEnded => {
                let group_a = event.data.group_name.as_deref().unwrap_or("Unknown");
                let group_b = event.data.group_b_name.as_deref().unwrap_or("Unknown");
                (
                    format!("{} and {} no longer rivals", group_a, group_b),
                    EventViewType::RivalryEnded,
                )
            }
            EventType::Courted => {
                let name = agent_name(event.agent?);
                let target_name = agent_name(event.target?);
                let score = event.data.courtship_score.unwrap_or(0.0);
                (
                    format!("{} courted {} ({:.0}%)", name, target_name, score * 100.0),
                    EventViewType::Courtship,
                )
            }
            EventType::Conceived => {
                let parent_a = event.data.parent_a.map(agent_name).unwrap_or_else(|| "Unknown".to_string());
                let parent_b = event.data.parent_b.map(agent_name).unwrap_or_else(|| "Unknown".to_string());
                (
                    format!("{} and {} conceived", parent_a, parent_b),
                    EventViewType::Conception,
                )
            }
            EventType::BirthOccurred => {
                let parent_a = event.data.parent_a.map(agent_name).unwrap_or_else(|| "Unknown".to_string());
                let parent_b = event.data.parent_b.map(agent_name).unwrap_or_else(|| "Unknown".to_string());
                let child_name = event.data.child_name.as_deref().unwrap_or("Unknown");
                (
                    format!("{} was born to {} and {}", child_name, parent_a, parent_b),
                    EventViewType::Birth,
                )
            }
            EventType::SkillTaught => {
                let teacher = agent_name(event.agent?);
                let student = agent_name(event.target?);
                let skill = event.data.skill_name.as_deref().unwrap_or("unknown");
                let level = event.data.skill_level.unwrap_or(0.0);
                (
                    format!("{} taught {} to {} ({:.0}%)", teacher, skill, student, level * 100.0),
                    EventViewType::SkillTaught,
                )
            }
            EventType::GatheredMaterials => {
                let name = agent_name(event.agent?);
                let materials = event.data.materials.as_ref()
                    .map(|m| {
                        m.iter()
                            .map(|(mat, amt)| format!("{} {}", amt, mat))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_else(|| "materials".to_string());
                (
                    format!("{} gathered {}", name, materials),
                    EventViewType::MaterialGathering,
                )
            }
            EventType::Crafted => {
                let name = agent_name(event.agent?);
                let tool = event.data.tool_name.as_deref().unwrap_or("tool");
                let quality = event.data.tool_quality.as_deref().unwrap_or("");
                (
                    format!("{} crafted a {} {}", name, quality, tool),
                    EventViewType::Crafting,
                )
            }
            EventType::Hunted => {
                let name = agent_name(event.agent?);
                let success = event.data.success.unwrap_or(false);
                if success {
                    let amount = event.data.amount.unwrap_or(0);
                    (
                        format!("{} hunted successfully ({} food)", name, amount),
                        EventViewType::Hunting,
                    )
                } else {
                    (
                        format!("{} failed to catch prey", name),
                        EventViewType::Hunting,
                    )
                }
            }
            EventType::Fished => {
                let name = agent_name(event.agent?);
                let success = event.data.success.unwrap_or(false);
                if success {
                    let amount = event.data.amount.unwrap_or(0);
                    (
                        format!("{} caught {} fish", name, amount),
                        EventViewType::Fishing,
                    )
                } else {
                    (
                        format!("{} failed to catch any fish", name),
                        EventViewType::Fishing,
                    )
                }
            }
            EventType::Chopped => {
                let name = agent_name(event.agent?);
                let amount = event.data.amount.unwrap_or(0);
                (
                    format!("{} chopped {} wood", name, amount),
                    EventViewType::Chopping,
                )
            }
            EventType::ToolBroke => {
                let name = agent_name(event.agent?);
                let tool = event.data.tool_name.as_deref().unwrap_or("tool");
                (
                    format!("{}'s {} broke", name, tool),
                    EventViewType::ToolBroke,
                )
            }
            EventType::TerritoryMarked => {
                let name = agent_name(event.agent?);
                let x = event.data.territory_x.unwrap_or(0);
                let y = event.data.territory_y.unwrap_or(0);
                (
                    format!("{} marked territory at ({}, {})", name, x, y),
                    EventViewType::TerritoryMarked,
                )
            }
            EventType::TerritoryChallenged => {
                let name = agent_name(event.agent?);
                let target_name = agent_name(event.target?);
                (
                    format!("{} challenged {} for trespassing", name, target_name),
                    EventViewType::TerritoryChallenged,
                )
            }
            EventType::TerritorySubmitted => {
                let trespasser = agent_name(event.target?);
                (
                    format!("{} submitted and left the territory", trespasser),
                    EventViewType::TerritorySubmitted,
                )
            }
            EventType::TerritoryFight => {
                let challenger = agent_name(event.agent?);
                let trespasser = agent_name(event.target?);
                let winner = event.data.winner.map(agent_name).unwrap_or_else(|| "Unknown".to_string());
                (
                    format!("{} and {} fought over territory - {} won", challenger, trespasser, winner),
                    EventViewType::TerritoryFight,
                )
            }
            EventType::TerritoryLost => {
                let name = agent_name(event.agent?);
                let x = event.data.territory_x.unwrap_or(0);
                let y = event.data.territory_y.unwrap_or(0);
                (
                    format!("{} lost territory at ({}, {})", name, x, y),
                    EventViewType::TerritoryLost,
                )
            }
            EventType::FarmProduced => {
                let name = agent_name(event.agent?);
                let amount = event.data.amount.unwrap_or(0);
                (
                    format!("{}'s farm produced {} food", name, amount),
                    EventViewType::FarmProduced,
                )
            }
            EventType::StructureDestroyed => {
                let name = agent_name(event.agent?);
                let structure_type = event.data.description.as_deref().unwrap_or("structure");
                (
                    format!("{}'s {} collapsed", name, structure_type),
                    EventViewType::StructureDestroyed,
                )
            }
            // Trade events
            EventType::TradeProposed => {
                let proposer = agent_name(event.agent?);
                let recipient = agent_name(event.target?);
                let offering = event.data.trade_offering.as_deref().unwrap_or("items");
                let requesting = event.data.trade_requesting.as_deref().unwrap_or("items");
                (
                    format!("{} offers {} to {} for {}", proposer, offering, recipient, requesting),
                    EventViewType::TradeProposed,
                )
            }
            EventType::TradeAccepted => {
                let accepter = agent_name(event.agent?);
                let proposer = agent_name(event.target?);
                (
                    format!("{} accepted trade from {}", accepter, proposer),
                    EventViewType::TradeAccepted,
                )
            }
            EventType::TradeDeclined => {
                let decliner = agent_name(event.agent?);
                let proposer = agent_name(event.target?);
                (
                    format!("{} declined trade from {}", decliner, proposer),
                    EventViewType::TradeDeclined,
                )
            }
            EventType::TradeCountered => {
                let counter = agent_name(event.agent?);
                let original = agent_name(event.target?);
                let offering = event.data.trade_offering.as_deref().unwrap_or("items");
                let requesting = event.data.trade_requesting.as_deref().unwrap_or("items");
                (
                    format!("{} counter-offers {} for {}", counter, offering, requesting),
                    EventViewType::TradeCountered,
                )
            }
            EventType::TradeExpired => {
                let proposer = agent_name(event.agent?);
                (
                    format!("{}'s trade offer expired", proposer),
                    EventViewType::TradeExpired,
                )
            }
            EventType::TradeCancelled => {
                let proposer = agent_name(event.agent?);
                (
                    format!("{} cancelled their trade offer", proposer),
                    EventViewType::TradeCancelled,
                )
            }
            EventType::TradeReneged => {
                let debtor = agent_name(event.agent?);
                let creditor = agent_name(event.target?);
                let service = event.data.service_type.as_deref().unwrap_or("promise");
                (
                    format!("{} broke their {} promise to {}", debtor, service, creditor),
                    EventViewType::TradeReneged,
                )
            }
            EventType::ServiceFulfilled => {
                let debtor = agent_name(event.agent?);
                let creditor = agent_name(event.target?);
                let service = event.data.service_type.as_deref().unwrap_or("promise");
                (
                    format!("{} fulfilled their {} to {}", debtor, service, creditor),
                    EventViewType::ServiceFulfilled,
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
