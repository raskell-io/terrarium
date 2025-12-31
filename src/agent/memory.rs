use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Agent's memory system
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Memory {
    /// Recent episodes (last N events)
    pub recent: Vec<Episode>,
    /// Maximum recent episodes to keep
    pub max_recent: usize,
}

/// A single memorable event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub epoch: usize,
    pub description: String,
    /// Emotional valence: -1.0 (very negative) to 1.0 (very positive)
    pub valence: f64,
    /// Other agents involved
    pub participants: Vec<Uuid>,
    /// Category for filtering
    pub category: EpisodeCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EpisodeCategory {
    Survival,    // Food, health, danger
    Social,      // Interaction with others
    Conflict,    // Attacks, disputes
    Discovery,   // Found something new
    Gift,        // Received or gave something
}

impl Memory {
    pub fn new() -> Self {
        Self {
            recent: Vec::new(),
            max_recent: 10,
        }
    }

    /// Add an episode to memory
    pub fn remember(&mut self, episode: Episode) {
        self.recent.push(episode);

        // Keep only the most recent
        if self.recent.len() > self.max_recent {
            self.recent.remove(0);
        }
    }

    /// Get episodes involving a specific agent
    pub fn episodes_with(&self, agent_id: Uuid) -> Vec<&Episode> {
        self.recent
            .iter()
            .filter(|e| e.participants.contains(&agent_id))
            .collect()
    }

    /// Get episodes of a specific category
    pub fn episodes_of_category(&self, category: EpisodeCategory) -> Vec<&Episode> {
        self.recent
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Generate a summary for LLM prompting
    pub fn prompt_summary(&self, current_epoch: usize) -> String {
        if self.recent.is_empty() {
            return "No significant recent events.".to_string();
        }

        let summaries: Vec<String> = self
            .recent
            .iter()
            .rev() // Most recent first
            .take(5)
            .map(|e| {
                let ago = current_epoch - e.epoch;
                let time_desc = if ago == 0 {
                    "Just now".to_string()
                } else if ago == 1 {
                    "Yesterday".to_string()
                } else {
                    format!("{} days ago", ago)
                };
                format!("{}: {}", time_desc, e.description)
            })
            .collect();

        format!("Recent memories:\n{}", summaries.join("\n"))
    }
}

impl Episode {
    pub fn new(
        epoch: usize,
        description: String,
        valence: f64,
        participants: Vec<Uuid>,
        category: EpisodeCategory,
    ) -> Self {
        Self {
            epoch,
            description,
            valence,
            participants,
            category,
        }
    }

    /// Create a survival-related episode
    pub fn survival(epoch: usize, description: &str, valence: f64) -> Self {
        Self::new(
            epoch,
            description.to_string(),
            valence,
            Vec::new(),
            EpisodeCategory::Survival,
        )
    }

    /// Create a social episode
    pub fn social(epoch: usize, description: &str, valence: f64, other: Uuid) -> Self {
        Self::new(
            epoch,
            description.to_string(),
            valence,
            vec![other],
            EpisodeCategory::Social,
        )
    }

    /// Create a conflict episode
    pub fn conflict(epoch: usize, description: &str, valence: f64, other: Uuid) -> Self {
        Self::new(
            epoch,
            description.to_string(),
            valence,
            vec![other],
            EpisodeCategory::Conflict,
        )
    }
}
