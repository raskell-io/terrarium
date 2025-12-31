use serde::{Deserialize, Serialize};

/// Big Five personality model
/// All traits are normalized to 0.0 - 1.0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personality {
    /// Openness to experience: curiosity, creativity, preference for novelty
    pub openness: f64,
    /// Conscientiousness: organization, dependability, self-discipline
    pub conscientiousness: f64,
    /// Extraversion: sociability, assertiveness, positive emotions
    pub extraversion: f64,
    /// Agreeableness: cooperation, trust, altruism
    pub agreeableness: f64,
    /// Neuroticism: emotional instability, anxiety, moodiness
    pub neuroticism: f64,
}

impl Personality {
    pub fn random() -> Self {
        Self {
            openness: rand::random(),
            conscientiousness: rand::random(),
            extraversion: rand::random(),
            agreeableness: rand::random(),
            neuroticism: rand::random(),
        }
    }

    pub fn with_distribution(
        openness: (f64, f64),
        conscientiousness: (f64, f64),
        extraversion: (f64, f64),
        agreeableness: (f64, f64),
        neuroticism: (f64, f64),
    ) -> Self {
        Self {
            openness: sample_normal(openness.0, openness.1),
            conscientiousness: sample_normal(conscientiousness.0, conscientiousness.1),
            extraversion: sample_normal(extraversion.0, extraversion.1),
            agreeableness: sample_normal(agreeableness.0, agreeableness.1),
            neuroticism: sample_normal(neuroticism.0, neuroticism.1),
        }
    }

    /// Generate a human-readable description of this personality
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
            traits.push("calm and resilient");
        }

        if traits.is_empty() {
            "balanced temperament".to_string()
        } else {
            traits.join(", ")
        }
    }

    /// How likely is this agent to take risks?
    pub fn risk_factor(&self) -> f64 {
        // Low neuroticism + high openness = more risk taking
        (1.0 - self.neuroticism) * 0.5 + self.openness * 0.5
    }

    /// How likely is this agent to cooperate with others?
    pub fn cooperation_factor(&self) -> f64 {
        self.agreeableness * 0.6 + self.extraversion * 0.4
    }

    /// How likely is this agent to plan ahead?
    pub fn planning_factor(&self) -> f64 {
        self.conscientiousness * 0.7 + (1.0 - self.neuroticism) * 0.3
    }
}

/// Sample from a normal distribution, clamped to [0, 1]
fn sample_normal(mean: f64, std_dev: f64) -> f64 {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Box-Muller transform for normal distribution
    let u1: f64 = rng.gen();
    let u2: f64 = rng.gen();
    let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();

    (mean + z * std_dev).clamp(0.0, 1.0)
}
