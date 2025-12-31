use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::action::Action;
use crate::agent::Agent;

/// LLM client configuration
#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    pub max_tokens: usize,
    pub temperature: f64,
}

/// LLM client for agent deliberation
pub struct LlmClient {
    client: reqwest::Client,
    config: LlmConfig,
    api_key: Option<String>,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: usize,
    temperature: f64,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(config: LlmConfig) -> Self {
        let api_key = std::env::var(&config.api_key_env).ok();

        if api_key.is_none() {
            warn!(
                "API key not found in {}. Will use heuristic fallback.",
                config.api_key_env
            );
        }

        Self {
            client: reqwest::Client::new(),
            config,
            api_key,
        }
    }

    /// Check if LLM is available
    pub fn is_available(&self) -> bool {
        self.api_key.is_some()
    }

    /// Get an action from the LLM
    pub async fn decide_action(
        &self,
        agent: &Agent,
        world_perception: &str,
        nearby_agents: &[(uuid::Uuid, &str)],
        epoch: usize,
    ) -> Result<Action> {
        // If no API key, use heuristic
        if !self.is_available() {
            return Ok(heuristic_action(agent, nearby_agents));
        }

        let prompt = self.build_prompt(agent, world_perception, nearby_agents, epoch);
        let response = self.call_api(&prompt).await?;

        debug!("Agent {} reasoning: {}", agent.name(), response);

        // Parse action from response
        let action = Action::parse(&response, nearby_agents).unwrap_or_else(|| {
            warn!(
                "Could not parse action from: {}. Defaulting to WAIT",
                response
            );
            Action::Wait
        });

        Ok(action)
    }

    fn build_prompt(
        &self,
        agent: &Agent,
        world_perception: &str,
        nearby_agents: &[(uuid::Uuid, &str)],
        epoch: usize,
    ) -> String {
        let nearby_list: Vec<String> = nearby_agents
            .iter()
            .map(|(_, name)| name.to_string())
            .collect();

        let nearby_desc = if nearby_list.is_empty() {
            "No one else is nearby.".to_string()
        } else {
            format!("Nearby: {}", nearby_list.join(", "))
        };

        format!(
            r#"{}

## Current Situation (Day {})
{}

{}

## Available Actions
{}

## Instructions
Think about your current needs, your personality, and your goals.
Decide what to do. Respond with your reasoning (1-2 sentences) then your chosen action.

Format your response like this:
REASONING: [your thinking]
ACTION: [one action from the list above]

Example:
REASONING: I am hungry and there is food here. I should gather some.
ACTION: GATHER"#,
            agent.prompt_state(epoch),
            epoch,
            world_perception,
            nearby_desc,
            Action::available_actions_prompt(nearby_agents),
        )
    }

    async fn call_api(&self, prompt: &str) -> Result<String> {
        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("No API key"))?;

        let request = AnthropicRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            system: "You are a person living in a small world. You make decisions based on your personality, needs, and goals. Be consistent with your character. Respond concisely.".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error {}: {}", status, text));
        }

        let response: AnthropicResponse = response.json().await?;

        response
            .content
            .first()
            .map(|c| c.text.clone())
            .ok_or_else(|| anyhow!("Empty response"))
    }
}

/// Heuristic action when no LLM available
fn heuristic_action(agent: &Agent, nearby_agents: &[(uuid::Uuid, &str)]) -> Action {
    use rand::Rng;
    let mut rng = rand::rng();

    // Priority 1: Eat if hungry and have food
    if agent.physical.hunger > 0.6 && agent.physical.food > 0 {
        return Action::Eat;
    }

    // Priority 2: Rest if exhausted
    if agent.physical.energy < 0.2 {
        return Action::Rest;
    }

    // Priority 3: Gather if low on food
    if agent.physical.food < 3 {
        return Action::Gather;
    }

    // Priority 4: Give food to nearby hungry agent if agreeable
    if agent.identity.personality.agreeableness > 0.7
        && agent.physical.food > 5
        && !nearby_agents.is_empty()
    {
        let (target, _) = nearby_agents[rng.random_range(0..nearby_agents.len())];
        return Action::Give { target, amount: 1 };
    }

    // Priority 5: Gossip if extraverted and have opinions to share
    if agent.identity.personality.extraversion > 0.5
        && nearby_agents.len() >= 2
        && rng.random::<f64>() < 0.3  // 30% chance
    {
        // Find someone we have strong feelings about
        let gossip_subject = agent.beliefs.social.iter()
            .filter(|(id, belief)| {
                // Strong feelings (positive or negative)
                (belief.trust.abs() > 0.3 || belief.sentiment.abs() > 0.3)
                    // Subject is nearby
                    && nearby_agents.iter().any(|(nid, _)| nid == *id)
            })
            .max_by(|(_, a), (_, b)| {
                // Prefer strongest feelings
                let a_strength = a.trust.abs() + a.sentiment.abs();
                let b_strength = b.trust.abs() + b.sentiment.abs();
                a_strength.partial_cmp(&b_strength).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| *id);

        if let Some(about) = gossip_subject {
            // Find a target to gossip to (not the subject)
            let potential_targets: Vec<_> = nearby_agents.iter()
                .filter(|(id, _)| *id != about)
                .collect();

            if !potential_targets.is_empty() {
                let (target, _) = potential_targets[rng.random_range(0..potential_targets.len())];
                return Action::Gossip { target: *target, about };
            }
        }
    }

    // Otherwise: random action
    match rng.random_range(0..10) {
        0..=4 => {
            // Move in random direction
            let directions = [
                crate::action::Direction::North,
                crate::action::Direction::South,
                crate::action::Direction::East,
                crate::action::Direction::West,
                crate::action::Direction::NorthEast,
                crate::action::Direction::NorthWest,
                crate::action::Direction::SouthEast,
                crate::action::Direction::SouthWest,
            ];
            Action::Move(directions[rng.random_range(0..8)])
        }
        5..=6 => Action::Gather,
        7 => Action::Rest,
        _ => Action::Wait,
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            max_tokens: 500,
            temperature: 0.7,
        }
    }
}
