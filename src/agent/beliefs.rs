use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Agent's belief system: what they think they know (can be wrong)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Beliefs {
    /// Beliefs about the world (locations, resources)
    pub world: WorldBeliefs,
    /// Beliefs about other agents
    pub social: HashMap<Uuid, SocialBelief>,
    /// Beliefs about self
    pub self_belief: SelfBelief,
}

/// Beliefs about the physical world
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldBeliefs {
    /// Known locations with food - stored as Vec for JSON compatibility
    pub food_locations: Vec<FoodLocationBelief>,
    /// Locations believed to be dangerous
    pub dangerous_locations: Vec<(usize, usize)>,
}

/// Belief about food at a specific location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodLocationBelief {
    pub x: usize,
    pub y: usize,
    pub belief: FoodBelief,
}

/// Belief about food at a location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoodBelief {
    pub amount: u32,
    pub last_seen_epoch: usize,
}

/// Belief about another agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialBelief {
    pub name: String,
    /// -1.0 (complete distrust) to 1.0 (complete trust)
    pub trust: f64,
    /// -1.0 (hate) to 1.0 (love)
    pub sentiment: f64,
    /// How many times we've interacted
    pub interaction_count: u32,
    /// Last epoch we saw them
    pub last_seen_epoch: usize,
    /// Brief impression
    pub impression: Option<String>,
}

/// Beliefs about self
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelfBelief {
    /// How capable do I think I am? (0-1)
    pub perceived_competence: f64,
    /// How safe do I feel? (0-1)
    pub perceived_safety: f64,
    /// How connected do I feel to others? (0-1)
    pub perceived_belonging: f64,
}

impl Beliefs {
    pub fn new() -> Self {
        Self {
            world: WorldBeliefs::default(),
            social: HashMap::new(),
            self_belief: SelfBelief {
                perceived_competence: 0.5,
                perceived_safety: 0.5,
                perceived_belonging: 0.0,
            },
        }
    }

    /// Update belief about food at a location
    pub fn update_food_belief(&mut self, x: usize, y: usize, amount: u32, epoch: usize) {
        // Find existing belief or add new one
        if let Some(existing) = self.world.food_locations.iter_mut().find(|b| b.x == x && b.y == y) {
            existing.belief.amount = amount;
            existing.belief.last_seen_epoch = epoch;
        } else {
            self.world.food_locations.push(FoodLocationBelief {
                x,
                y,
                belief: FoodBelief {
                    amount,
                    last_seen_epoch: epoch,
                },
            });
        }
    }

    /// Get or create social belief about another agent
    pub fn get_or_create_social(&mut self, agent_id: Uuid, name: &str) -> &mut SocialBelief {
        self.social.entry(agent_id).or_insert_with(|| SocialBelief {
            name: name.to_string(),
            trust: 0.0,      // neutral
            sentiment: 0.0,  // neutral
            interaction_count: 0,
            last_seen_epoch: 0,
            impression: None,
        })
    }

    /// Update trust based on an interaction
    pub fn update_trust(&mut self, agent_id: Uuid, name: &str, delta: f64, epoch: usize) {
        let belief = self.get_or_create_social(agent_id, name);
        belief.trust = (belief.trust + delta).clamp(-1.0, 1.0);
        belief.interaction_count += 1;
        belief.last_seen_epoch = epoch;
    }

    /// Update sentiment based on an interaction
    pub fn update_sentiment(&mut self, agent_id: Uuid, name: &str, delta: f64, epoch: usize) {
        let belief = self.get_or_create_social(agent_id, name);
        belief.sentiment = (belief.sentiment + delta).clamp(-1.0, 1.0);
        belief.last_seen_epoch = epoch;
    }

    /// Receive gossip about a third party
    /// The influence is weighted by trust in the gossiper
    /// Returns the sentiment description for logging
    pub fn receive_gossip(
        &mut self,
        gossiper_id: Uuid,
        about_id: Uuid,
        about_name: &str,
        gossiper_trust: f64,
        gossiper_sentiment: f64,
        epoch: usize,
    ) -> String {
        // How much we're influenced by the gossiper (max 30%)
        let gossiper_trust_in_us = self.social.get(&gossiper_id).map(|b| b.trust).unwrap_or(0.0);
        let influence = ((gossiper_trust_in_us + 1.0) / 2.0) * 0.3; // 0-30% based on trust

        let belief = self.get_or_create_social(about_id, about_name);

        // Blend our belief with the gossiper's belief
        let old_trust = belief.trust;
        let old_sentiment = belief.sentiment;

        belief.trust = (belief.trust * (1.0 - influence) + gossiper_trust * influence).clamp(-1.0, 1.0);
        belief.sentiment = (belief.sentiment * (1.0 - influence) + gossiper_sentiment * influence).clamp(-1.0, 1.0);
        belief.last_seen_epoch = epoch;

        // Return a description of the gossip sentiment
        if gossiper_trust > 0.3 && gossiper_sentiment > 0.3 {
            "positive".to_string()
        } else if gossiper_trust < -0.3 || gossiper_sentiment < -0.3 {
            "negative".to_string()
        } else {
            "neutral".to_string()
        }
    }

    /// Get social belief about an agent (if exists)
    pub fn get_social(&self, agent_id: Uuid) -> Option<&SocialBelief> {
        self.social.get(&agent_id)
    }

    /// Generate a summary for LLM prompting
    pub fn prompt_summary(&self, current_epoch: usize) -> String {
        let mut parts = Vec::new();

        // World beliefs
        let food_beliefs: Vec<String> = self
            .world
            .food_locations
            .iter()
            .filter(|loc| current_epoch.saturating_sub(loc.belief.last_seen_epoch) < 10) // Recent beliefs
            .map(|loc| {
                let freshness = if current_epoch == loc.belief.last_seen_epoch {
                    "just saw"
                } else {
                    "remember"
                };
                format!("I {} food at ({}, {})", freshness, loc.x, loc.y)
            })
            .collect();

        if !food_beliefs.is_empty() {
            parts.push(format!("World knowledge: {}", food_beliefs.join("; ")));
        }

        // Social beliefs
        let social_beliefs: Vec<String> = self
            .social
            .values()
            .map(|belief| {
                let trust_desc = if belief.trust > 0.5 {
                    "trust"
                } else if belief.trust < -0.5 {
                    "distrust"
                } else {
                    "am unsure about"
                };
                let sentiment_desc = if belief.sentiment > 0.5 {
                    "like"
                } else if belief.sentiment < -0.5 {
                    "dislike"
                } else {
                    ""
                };

                if sentiment_desc.is_empty() {
                    format!("I {} {}", trust_desc, belief.name)
                } else {
                    format!("I {} and {} {}", trust_desc, sentiment_desc, belief.name)
                }
            })
            .collect();

        if !social_beliefs.is_empty() {
            parts.push(format!("Social beliefs: {}", social_beliefs.join("; ")));
        }

        // Self beliefs
        let safety_desc = if self.self_belief.perceived_safety > 0.7 {
            "I feel safe"
        } else if self.self_belief.perceived_safety < 0.3 {
            "I feel unsafe"
        } else {
            "I'm uncertain about my safety"
        };
        parts.push(format!("Self: {}", safety_desc));

        if parts.is_empty() {
            "I don't know much about this world yet.".to_string()
        } else {
            parts.join("\n")
        }
    }
}

impl SocialBelief {
    /// Update impression from an observation or interaction
    pub fn set_impression(&mut self, impression: &str) {
        self.impression = Some(impression.to_string());
    }
}
