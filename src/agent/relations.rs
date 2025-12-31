use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// An agent's social network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialGraph {
    /// Known relationships
    pub relationships: HashMap<Uuid, Relationship>,
}

/// A relationship with another agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// The other agent's ID
    pub agent_id: Uuid,
    /// The other agent's name (as known)
    pub known_name: String,
    /// Type of relationship
    pub relationship_type: RelationshipType,
    /// Trust level (-1.0 = complete distrust, 1.0 = complete trust)
    pub trust: f64,
    /// How well we know them (0.0 = stranger, 1.0 = intimate)
    pub familiarity: f64,
    /// Overall sentiment (-1.0 = hate, 1.0 = love)
    pub sentiment: f64,
    /// Number of interactions
    pub interaction_count: usize,
    /// Last interaction epoch
    pub last_interaction: usize,
    /// Outstanding debts/obligations (positive = they owe us)
    pub debt: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationshipType {
    Stranger,
    Acquaintance,
    Friend,
    CloseFriend,
    Rival,
    Enemy,
    Family,
    TradePartner,
    Ally,
    Leader,
    Follower,
}

impl SocialGraph {
    pub fn new() -> Self {
        Self {
            relationships: HashMap::new(),
        }
    }

    /// Get or create a relationship with another agent
    pub fn get_or_create(&mut self, agent_id: Uuid, name: &str) -> &mut Relationship {
        self.relationships.entry(agent_id).or_insert_with(|| {
            Relationship {
                agent_id,
                known_name: name.to_string(),
                relationship_type: RelationshipType::Stranger,
                trust: 0.0,
                familiarity: 0.0,
                sentiment: 0.0,
                interaction_count: 0,
                last_interaction: 0,
                debt: 0,
            }
        })
    }

    /// Record an interaction
    pub fn record_interaction(
        &mut self,
        agent_id: Uuid,
        name: &str,
        epoch: usize,
        trust_delta: f64,
        sentiment_delta: f64,
    ) {
        let rel = self.get_or_create(agent_id, name);
        rel.interaction_count += 1;
        rel.last_interaction = epoch;
        rel.trust = (rel.trust + trust_delta).clamp(-1.0, 1.0);
        rel.sentiment = (rel.sentiment + sentiment_delta).clamp(-1.0, 1.0);
        rel.familiarity = (rel.familiarity + 0.1).min(1.0);

        // Update relationship type based on metrics
        rel.update_type();
    }

    /// Get agents we trust most
    pub fn trusted_agents(&self) -> Vec<Uuid> {
        let mut agents: Vec<_> = self.relationships.iter()
            .filter(|(_, r)| r.trust > 0.3)
            .collect();
        agents.sort_by(|a, b| b.1.trust.partial_cmp(&a.1.trust).unwrap());
        agents.into_iter().map(|(id, _)| *id).collect()
    }

    /// Get agents we distrust
    pub fn distrusted_agents(&self) -> Vec<Uuid> {
        self.relationships.iter()
            .filter(|(_, r)| r.trust < -0.3)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Generate a summary for LLM prompting
    pub fn summary(&self) -> String {
        if self.relationships.is_empty() {
            return "No known relationships.".to_string();
        }

        let mut summary = String::from("Known people:\n");
        for (_, rel) in &self.relationships {
            let trust_desc = if rel.trust > 0.5 {
                "trusted"
            } else if rel.trust < -0.5 {
                "distrusted"
            } else {
                "neutral"
            };

            let sentiment_desc = if rel.sentiment > 0.5 {
                "liked"
            } else if rel.sentiment < -0.5 {
                "disliked"
            } else {
                ""
            };

            summary.push_str(&format!(
                "- {} ({:?}): {} {}\n",
                rel.known_name,
                rel.relationship_type,
                trust_desc,
                sentiment_desc
            ));
        }

        summary
    }
}

impl Relationship {
    fn update_type(&mut self) {
        self.relationship_type = match (self.trust, self.sentiment, self.familiarity) {
            (t, _, f) if f < 0.2 => RelationshipType::Stranger,
            (t, s, f) if t < -0.5 && s < -0.5 => RelationshipType::Enemy,
            (t, s, _) if t < -0.3 && s < 0.0 => RelationshipType::Rival,
            (t, s, f) if t > 0.5 && s > 0.5 && f > 0.7 => RelationshipType::CloseFriend,
            (t, s, _) if t > 0.3 && s > 0.3 => RelationshipType::Friend,
            (t, _, _) if t > 0.3 => RelationshipType::Acquaintance,
            _ => RelationshipType::Acquaintance,
        };
    }
}

impl Default for SocialGraph {
    fn default() -> Self {
        Self::new()
    }
}
