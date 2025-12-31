use crate::agent::{Action, Agent, Direction};
use crate::config::LlmConfig;
use crate::engine::AgentContext;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use uuid::Uuid;

/// Client for LLM interactions
pub struct LlmClient {
    client: Client,
    config: LlmConfig,
    api_key: String,
}

impl LlmClient {
    pub fn new(config: &LlmConfig) -> anyhow::Result<Self> {
        let api_key = std::env::var(&config.api_key_env)
            .unwrap_or_else(|_| {
                warn!("API key not found in {}. Using mock responses.", config.api_key_env);
                String::new()
            });

        Ok(Self {
            client: Client::new(),
            config: config.clone(),
            api_key,
        })
    }

    /// Get an action decision from the LLM
    pub async fn decide_action(&self, agent: &Agent, context: &AgentContext) -> anyhow::Result<Action> {
        // If no API key, use simple heuristic behavior
        if self.api_key.is_empty() {
            return Ok(self.heuristic_action(agent, context));
        }

        // Build the prompt
        let prompt = self.build_prompt(agent, context);

        // Call the LLM
        let response = self.call_llm(&prompt).await?;

        // Parse the response into an action
        let action = self.parse_action(&response, agent, context)?;

        Ok(action)
    }

    fn build_prompt(&self, agent: &Agent, context: &AgentContext) -> String {
        format!(
            r#"You are {name}, living in a simulated world. You must decide what to do this epoch.

## Your Current State
{state}

## Your Memories
{memories}

## Your Relationships
{relations}

## What You See
Current location: {current_cell}
Nearby terrain: {terrain}
Nearby people: {nearby}
Season: {season}
World events: {events}

## Available Actions
- WAIT: Do nothing, recover some energy
- MOVE [direction]: Move to adjacent cell (NORTH, SOUTH, EAST, WEST, NORTHEAST, NORTHWEST, SOUTHEAST, SOUTHWEST)
- GATHER: Collect resources from current location
- EAT: Consume food to reduce hunger
- REST: Recover energy
- TRADE [person] [offer_type] [offer_amount] [request_type] [request_amount]: Propose a trade
- SPEAK [person] [message]: Say something to someone nearby
- GIVE [person] [resource] [amount]: Give resources to someone
- ATTACK [person]: Attack someone nearby
- BUILD [item]: Create something from materials

## Instructions
Think about your personality, your needs, and your goals. What would you do?
Respond with exactly one action in the format shown above.
If speaking, keep messages brief (under 20 words).

Your action:"#,
            name = agent.name,
            state = agent.state_summary(),
            memories = agent.memory.narrative_summary(context.epoch),
            relations = agent.relations.summary(),
            current_cell = context.current_cell,
            terrain = format_terrain(&context.visible_cells),
            nearby = format_nearby(&context.nearby_agents),
            season = context.season,
            events = if context.world_events.is_empty() {
                "None".to_string()
            } else {
                context.world_events.join(", ")
            },
        )
    }

    async fn call_llm(&self, prompt: &str) -> anyhow::Result<String> {
        #[derive(Serialize)]
        struct Request {
            model: String,
            max_tokens: usize,
            messages: Vec<Message>,
        }

        #[derive(Serialize)]
        struct Message {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct Response {
            content: Vec<Content>,
        }

        #[derive(Deserialize)]
        struct Content {
            text: String,
        }

        let request = Request {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?
            .json::<Response>()
            .await?;

        Ok(response.content.first()
            .map(|c| c.text.clone())
            .unwrap_or_default())
    }

    fn parse_action(&self, response: &str, agent: &Agent, context: &AgentContext) -> anyhow::Result<Action> {
        let response = response.trim().to_uppercase();
        let parts: Vec<&str> = response.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(Action::Wait);
        }

        match parts[0] {
            "WAIT" => Ok(Action::Wait),
            "REST" => Ok(Action::Rest),
            "GATHER" => Ok(Action::Gather),
            "EAT" => Ok(Action::Eat),
            "MOVE" if parts.len() > 1 => {
                let direction = match parts[1] {
                    "NORTH" => Direction::North,
                    "SOUTH" => Direction::South,
                    "EAST" => Direction::East,
                    "WEST" => Direction::West,
                    "NORTHEAST" => Direction::NorthEast,
                    "NORTHWEST" => Direction::NorthWest,
                    "SOUTHEAST" => Direction::SouthEast,
                    "SOUTHWEST" => Direction::SouthWest,
                    _ => Direction::North,
                };
                Ok(Action::Move { direction })
            }
            "SPEAK" if parts.len() > 2 => {
                let target_name = parts[1].to_lowercase();
                let message = parts[2..].join(" ");

                if let Some((id, _, _)) = context.nearby_agents.iter()
                    .find(|(_, name, _)| name.to_lowercase().contains(&target_name))
                {
                    Ok(Action::Speak { target: *id, message })
                } else {
                    Ok(Action::Wait)
                }
            }
            "ATTACK" if parts.len() > 1 => {
                let target_name = parts[1].to_lowercase();

                if let Some((id, _, _)) = context.nearby_agents.iter()
                    .find(|(_, name, _)| name.to_lowercase().contains(&target_name))
                {
                    Ok(Action::Attack { target: *id })
                } else {
                    Ok(Action::Wait)
                }
            }
            "BUILD" if parts.len() > 1 => {
                Ok(Action::Build { item: parts[1].to_string() })
            }
            _ => Ok(Action::Wait),
        }
    }

    /// Simple heuristic behavior when no LLM is available
    fn heuristic_action(&self, agent: &Agent, context: &AgentContext) -> Action {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Priority 1: Eat if very hungry and have food
        if agent.body.hunger > 0.7 && agent.body.inventory.food > 0 {
            return Action::Eat;
        }

        // Priority 2: Rest if very tired
        if agent.body.energy < 0.2 {
            return Action::Rest;
        }

        // Priority 3: Gather if low on food
        if agent.body.inventory.food < 5 {
            return Action::Gather;
        }

        // Random action otherwise
        match rng.gen_range(0..10) {
            0..=3 => Action::Move {
                direction: match rng.gen_range(0..8) {
                    0 => Direction::North,
                    1 => Direction::South,
                    2 => Direction::East,
                    3 => Direction::West,
                    4 => Direction::NorthEast,
                    5 => Direction::NorthWest,
                    6 => Direction::SouthEast,
                    _ => Direction::SouthWest,
                }
            },
            4..=5 => Action::Gather,
            6 => Action::Rest,
            7 if agent.body.inventory.food > 0 => Action::Eat,
            _ => Action::Wait,
        }
    }
}

fn format_terrain(cells: &[((usize, usize), String)]) -> String {
    if cells.is_empty() {
        return "Nothing visible".to_string();
    }

    cells.iter()
        .map(|((x, y), desc)| format!("({},{}): {}", x, y, desc))
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_nearby(agents: &[(Uuid, String, (usize, usize))]) -> String {
    if agents.is_empty() {
        return "No one nearby".to_string();
    }

    agents.iter()
        .map(|(_, name, (x, y))| format!("{} at ({},{})", name, x, y))
        .collect::<Vec<_>>()
        .join("; ")
}
