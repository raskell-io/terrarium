use rand::prelude::IndexedRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Stable identity: personality, values, aspiration
/// Does not change during simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub personality: Personality,
    pub values: Vec<Value>,
    pub aspiration: Aspiration,
}

/// Big Five personality traits (simplified)
/// Each trait is 0.0 to 1.0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personality {
    /// Curiosity, creativity, openness to new experiences
    pub openness: f64,
    /// Organization, dependability, self-discipline
    pub conscientiousness: f64,
    /// Sociability, assertiveness, positive emotions
    pub extraversion: f64,
    /// Cooperation, trust, altruism
    pub agreeableness: f64,
    /// Emotional instability, anxiety, moodiness
    pub neuroticism: f64,
}

/// What the agent values most
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Value {
    Survival,
    Relationships,
    Status,
    Freedom,
    Knowledge,
    Comfort,
}

/// Life aspiration / long-term goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Aspiration {
    BeRespected,
    ProtectOthers,
    AccumulateResources,
    ExploreTheWorld,
    LivePeacefully,
    BecomePowerful,
}

impl Personality {
    /// Generate a random personality
    pub fn random() -> Self {
        let mut rng = rand::rng();
        Self {
            openness: rng.random(),
            conscientiousness: rng.random(),
            extraversion: rng.random(),
            agreeableness: rng.random(),
            neuroticism: rng.random(),
        }
    }

    /// Describe personality in natural language
    pub fn describe(&self) -> String {
        let mut traits = Vec::new();

        if self.openness > 0.7 {
            traits.push("curious and creative");
        } else if self.openness < 0.3 {
            traits.push("practical and conventional");
        }

        if self.conscientiousness > 0.7 {
            traits.push("organized and disciplined");
        } else if self.conscientiousness < 0.3 {
            traits.push("spontaneous and flexible");
        }

        if self.extraversion > 0.7 {
            traits.push("outgoing and energetic");
        } else if self.extraversion < 0.3 {
            traits.push("reserved and solitary");
        }

        if self.agreeableness > 0.7 {
            traits.push("cooperative and trusting");
        } else if self.agreeableness < 0.3 {
            traits.push("competitive and skeptical");
        }

        if self.neuroticism > 0.7 {
            traits.push("anxious and sensitive");
        } else if self.neuroticism < 0.3 {
            traits.push("calm and emotionally stable");
        }

        if traits.is_empty() {
            "balanced in temperament".to_string()
        } else {
            traits.join(", ")
        }
    }
}

impl Value {
    pub fn describe(&self) -> &'static str {
        match self {
            Value::Survival => "staying alive",
            Value::Relationships => "connections with others",
            Value::Status => "being respected and admired",
            Value::Freedom => "independence and autonomy",
            Value::Knowledge => "understanding the world",
            Value::Comfort => "safety and ease",
        }
    }
}

impl Aspiration {
    /// Generate a random aspiration
    pub fn random() -> Self {
        let mut rng = rand::rng();
        match rng.random_range(0..6) {
            0 => Aspiration::BeRespected,
            1 => Aspiration::ProtectOthers,
            2 => Aspiration::AccumulateResources,
            3 => Aspiration::ExploreTheWorld,
            4 => Aspiration::LivePeacefully,
            _ => Aspiration::BecomePowerful,
        }
    }

    pub fn describe(&self) -> &'static str {
        match self {
            Aspiration::BeRespected => "to be respected by others",
            Aspiration::ProtectOthers => "to protect those around me",
            Aspiration::AccumulateResources => "to accumulate resources and security",
            Aspiration::ExploreTheWorld => "to explore and understand the world",
            Aspiration::LivePeacefully => "to live a peaceful, quiet life",
            Aspiration::BecomePowerful => "to become powerful and influential",
        }
    }
}

impl Identity {
    /// Create a new identity by inheriting traits from two parents
    pub fn from_parents(name: String, parent_a: &Identity, parent_b: &Identity) -> Self {
        let mut rng = rand::rng();

        // Each Big Five trait randomly picked from one parent
        let personality = Personality {
            openness: if rng.random::<bool>() {
                parent_a.personality.openness
            } else {
                parent_b.personality.openness
            },
            conscientiousness: if rng.random::<bool>() {
                parent_a.personality.conscientiousness
            } else {
                parent_b.personality.conscientiousness
            },
            extraversion: if rng.random::<bool>() {
                parent_a.personality.extraversion
            } else {
                parent_b.personality.extraversion
            },
            agreeableness: if rng.random::<bool>() {
                parent_a.personality.agreeableness
            } else {
                parent_b.personality.agreeableness
            },
            neuroticism: if rng.random::<bool>() {
                parent_a.personality.neuroticism
            } else {
                parent_b.personality.neuroticism
            },
        };

        // Values: 2-3 from union of parent values
        let all_parent_values: HashSet<Value> = parent_a
            .values
            .iter()
            .chain(parent_b.values.iter())
            .copied()
            .collect();
        let all_values_vec: Vec<Value> = all_parent_values.into_iter().collect();
        let count = rng.random_range(2..=3.min(all_values_vec.len()));
        let values: Vec<Value> = all_values_vec
            .choose_multiple(&mut rng, count)
            .copied()
            .collect();

        // Aspiration: randomly from one parent
        let aspiration = if rng.random::<bool>() {
            parent_a.aspiration.clone()
        } else {
            parent_b.aspiration.clone()
        };

        Self {
            name,
            personality,
            values,
            aspiration,
        }
    }

    /// Create a new random identity with the given name
    pub fn new(name: String) -> Self {
        let mut rng = rand::rng();

        // Pick 2-3 values
        let all_values = [
            Value::Survival,
            Value::Relationships,
            Value::Status,
            Value::Freedom,
            Value::Knowledge,
            Value::Comfort,
        ];
        let count = rng.random_range(2..=3);
        let mut values: Vec<Value> = all_values
            .choose_multiple(&mut rng, count)
            .copied()
            .collect();

        // Survival is always important (but might not be #1)
        if !values.contains(&Value::Survival) && rng.random::<f64>() < 0.7 {
            values.insert(0, Value::Survival);
        }

        Self {
            name,
            personality: Personality::random(),
            values,
            aspiration: Aspiration::random(),
        }
    }

    /// Generate a full description for LLM prompting
    pub fn prompt_description(&self) -> String {
        let values_desc: Vec<&str> = self.values.iter().map(|v| v.describe()).collect();

        format!(
            "You are {}.\n\
             Personality: You are {}.\n\
             Values: You care most about {}.\n\
             Aspiration: Your life goal is {}.",
            self.name,
            self.personality.describe(),
            values_desc.join(", "),
            self.aspiration.describe()
        )
    }
}
