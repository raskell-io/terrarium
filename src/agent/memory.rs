use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An agent's memory system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Significant life events (episodic memory)
    pub episodes: Vec<Episode>,
    /// Learned facts about the world (semantic memory)
    pub knowledge: Vec<Knowledge>,
    /// Maximum number of episodes to retain
    pub max_episodes: usize,
}

/// A memorable event in an agent's life
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    /// When this happened
    pub epoch: usize,
    /// What happened (narrative description)
    pub description: String,
    /// Emotional valence (-1.0 = very negative, 1.0 = very positive)
    pub emotional_valence: f64,
    /// How significant this event was (affects retention)
    pub significance: f64,
    /// Other agents involved
    pub participants: Vec<Uuid>,
    /// Tags for categorization
    pub tags: Vec<EpisodeTag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EpisodeTag {
    Trade,
    Conflict,
    Cooperation,
    Discovery,
    Loss,
    Gain,
    Social,
    Survival,
    Betrayal,
    Kindness,
}

/// Learned information about the world
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Knowledge {
    /// What is known
    pub fact: String,
    /// When this was learned
    pub learned_epoch: usize,
    /// Confidence in this knowledge (0.0 - 1.0)
    pub confidence: f64,
    /// Source of this knowledge
    pub source: KnowledgeSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KnowledgeSource {
    /// Observed directly
    Direct,
    /// Told by another agent
    Hearsay(Uuid),
    /// Inferred from other knowledge
    Inference,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            episodes: Vec::new(),
            knowledge: Vec::new(),
            max_episodes: 50,
        }
    }

    /// Record a new episode
    pub fn remember(&mut self, episode: Episode) {
        self.episodes.push(episode);
        self.compress_if_needed();
    }

    /// Add new knowledge
    pub fn learn(&mut self, knowledge: Knowledge) {
        // Check if we already know this (update confidence if so)
        if let Some(existing) = self.knowledge.iter_mut().find(|k| k.fact == knowledge.fact) {
            existing.confidence = (existing.confidence + knowledge.confidence) / 2.0;
        } else {
            self.knowledge.push(knowledge);
        }
    }

    /// Compress memories if we have too many
    fn compress_if_needed(&mut self) {
        if self.episodes.len() > self.max_episodes {
            // Keep the most significant and most recent episodes
            self.episodes.sort_by(|a, b| {
                let a_score = a.significance + (a.epoch as f64 * 0.01);
                let b_score = b.significance + (b.epoch as f64 * 0.01);
                b_score.partial_cmp(&a_score).unwrap()
            });
            self.episodes.truncate(self.max_episodes);
        }
    }

    /// Get episodes involving a specific agent
    pub fn episodes_with(&self, agent_id: Uuid) -> Vec<&Episode> {
        self.episodes
            .iter()
            .filter(|e| e.participants.contains(&agent_id))
            .collect()
    }

    /// Get recent episodes
    pub fn recent(&self, n: usize) -> Vec<&Episode> {
        let mut sorted: Vec<_> = self.episodes.iter().collect();
        sorted.sort_by(|a, b| b.epoch.cmp(&a.epoch));
        sorted.into_iter().take(n).collect()
    }

    /// Generate a narrative summary of memories for LLM prompting
    pub fn narrative_summary(&self, current_epoch: usize) -> String {
        let recent = self.recent(5);
        if recent.is_empty() {
            return "No significant memories yet.".to_string();
        }

        let mut summary = String::from("Recent memories:\n");
        for episode in recent {
            let epochs_ago = current_epoch - episode.epoch;
            let time_desc = if epochs_ago == 0 {
                "Just now".to_string()
            } else if epochs_ago == 1 {
                "Last epoch".to_string()
            } else {
                format!("{} epochs ago", epochs_ago)
            };

            let emotional = if episode.emotional_valence > 0.3 {
                "(positive)"
            } else if episode.emotional_valence < -0.3 {
                "(negative)"
            } else {
                "(neutral)"
            };

            summary.push_str(&format!("- {}: {} {}\n", time_desc, episode.description, emotional));
        }

        summary
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
