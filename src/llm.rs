use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::action::{Action, ActionContext};
use crate::agent::Agent;
use crate::world::LocalFood;

/// LLM client configuration
#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    pub max_tokens: usize,
    /// Sampling temperature. Leave unset for current models, which reject non-default values.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Thinking effort: low, medium, high, xhigh, max (unset = model default)
    #[serde(default)]
    pub effort: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig>,
    /// Server-side retry on another model if a safety classifier declines
    fallbacks: &'static str,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct OutputConfig {
    effort: String,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    stop_reason: Option<String>,
}

/// Responses may include thinking blocks before the text block
#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(config: LlmConfig) -> Self {
        let api_key = std::env::var(&config.api_key_env).ok();

        if api_key.is_none() {
            warn!(
                "API key not found in {}. Agents will act on instinct (see docs/INSTINCTS.md).",
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
    /// pending_trades: Vec of (proposal_index, proposer_id, proposer_name, offering_desc, requesting_desc, expires_in)
    /// debts_owed: Vec of (creditor_id, creditor_name, service_description, deadline_in) for debts the agent owes
    /// credits_owed: Vec of (debtor_id, debtor_name, service_description, deadline_in) for debts others owe this agent
    /// my_proposals: number of pending trade proposals this agent has made
    pub async fn decide_action(
        &self,
        agent: &Agent,
        world_perception: &str,
        local_food: &LocalFood,
        context: &ActionContext,
        nearby_agents: &[(uuid::Uuid, &str)],
        epoch: usize,
        pending_trades: &[(usize, uuid::Uuid, &str, String, String, Option<usize>)],
        debts_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        credits_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        my_proposals: usize,
        mate_proposals: &[(uuid::Uuid, &str)],
    ) -> Result<Action> {
        // Without an API key, agents act on instinct
        if !self.is_available() {
            return Ok(crate::instinct::decide(agent, local_food, context, pending_trades, debts_owed, mate_proposals, epoch));
        }

        let prompt = self.build_prompt(
            agent,
            world_perception,
            context,
            nearby_agents,
            epoch,
            pending_trades,
            debts_owed,
            credits_owed,
            my_proposals,
            mate_proposals,
        );
        let response = match self.call_api(&prompt).await {
            Ok(response) => response,
            Err(e) => {
                warn!("LLM call failed for {}: {}. Acting on instinct.", agent.name(), e);
                return Ok(crate::instinct::decide(agent, local_food, context, pending_trades, debts_owed, mate_proposals, epoch));
            }
        };

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
        context: &ActionContext,
        nearby_agents: &[(uuid::Uuid, &str)],
        epoch: usize,
        pending_trades: &[(usize, uuid::Uuid, &str, String, String, Option<usize>)],
        debts_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        credits_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        my_proposals: usize,
        mate_proposals: &[(uuid::Uuid, &str)],
    ) -> String {
        // Describe others as they look: hunger, injuries, what they carry, and known kinship
        let nearby_list: Vec<String> = nearby_agents
            .iter()
            .map(|(id, name)| match context.neighbours.iter().find(|n| n.id == *id) {
                Some(n) => {
                    let mut looks = Vec::new();
                    if n.hunger > 0.7 {
                        looks.push("looks starving".to_string());
                    } else if n.hunger > 0.5 {
                        looks.push("looks hungry".to_string());
                    }
                    if n.health < 0.5 {
                        looks.push("injured".to_string());
                    }
                    looks.push(format!("carrying {} food", n.food));
                    if n.relatedness > 0.0 {
                        looks.push("your kin".to_string());
                    }
                    format!("{} ({})", name, looks.join(", "))
                }
                None => name.to_string(),
            })
            .collect();

        let mut nearby_desc = if nearby_list.is_empty() {
            "No one else is nearby.".to_string()
        } else {
            format!("Nearby: {}", nearby_list.join(", "))
        };
        if let Some((x, y, structure_type, pct)) = context.unfinished_project {
            nearby_desc.push_str(&format!(
                "\nYour unfinished {} at ({}, {}) is {}% built. Stand on it and BUILD to keep working.",
                structure_type.display_name(), x, y, pct
            ));
        }
        if let Some((_, owner)) = &context.unfinished_here_owner {
            nearby_desc.push_str(&format!("\n{}'s unfinished structure is here; you could help BUILD it.", owner));
        }

        // Build trade context section
        let mut trade_context = self.build_trade_context(
            pending_trades,
            debts_owed,
            credits_owed,
            epoch,
        );
        if !mate_proposals.is_empty() {
            trade_context.push_str("\n## Proposals\n");
            for (_, name) in mate_proposals {
                trade_context.push_str(&format!(
                    "- **{}** wants to have a child with you. Reply MATE {} to accept within a few days, or ignore it to decline.\n",
                    name, name
                ));
            }
        }

        // Get teachable skills for action prompt
        let teachable_skills = agent.skills.teachable_skills();

        // Get tool-unlocked actions (already lowercase from unlocked_actions)
        let unlocked_actions = agent.physical.unlocked_actions();

        let trespassers: Vec<(uuid::Uuid, &str)> = context
            .trespassers
            .iter()
            .map(|(id, name)| (*id, name.as_str()))
            .collect();

        // Build pending trade offers for action prompt (index, proposer_name, offer, request)
        let pending_offer_descs: Vec<(usize, &str, String, String)> = pending_trades
            .iter()
            .map(|(idx, _, name, offering, requesting, _)| {
                (*idx, *name, offering.clone(), requesting.clone())
            })
            .collect();

        format!(
            r#"{}

## Current Situation (Day {})
{}

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
            trade_context,
            Action::available_actions_prompt(
                nearby_agents,
                &teachable_skills,
                &unlocked_actions,
                &context.craftable_tools,
                &context.buildable_structures,
                context.has_shelter,
                context.has_storage,
                context.owns_structure,
                context.is_sheltered,
                context.can_mark_territory,
                &trespassers,
                context.is_trespassing,
                context
                    .raid_target
                    .as_ref()
                    .filter(|(x, y, ..)| (*x, *y) == (agent.physical.x, agent.physical.y))
                    .map(|(_, _, _, owner, food)| (owner.as_str(), *food)),
                &pending_offer_descs,
                my_proposals,
            ),
        )
    }

    /// Build trade context section for prompt
    fn build_trade_context(
        &self,
        pending_trades: &[(usize, uuid::Uuid, &str, String, String, Option<usize>)],
        debts_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        credits_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        epoch: usize,
    ) -> String {
        let mut sections = Vec::new();

        // Pending trade offers to this agent
        if !pending_trades.is_empty() {
            let mut offers = String::from("\n## Pending Trade Offers\n");
            for (idx, _, name, offering, requesting, expires) in pending_trades {
                let expiry_str = expires
                    .map(|e| {
                        if e > epoch {
                            format!(" (expires in {} days)", e - epoch)
                        } else {
                            " (expiring soon!)".to_string()
                        }
                    })
                    .unwrap_or_default();
                offers.push_str(&format!(
                    "{}. **{}** offers {} for {}{}\n",
                    idx + 1,
                    name,
                    offering,
                    requesting,
                    expiry_str
                ));
            }
            sections.push(offers);
        }

        // Debts this agent owes
        if !debts_owed.is_empty() {
            let mut debts = String::from("\n## Your Obligations\n");
            debts.push_str("*You owe these services to others:*\n");
            for (_, name, service_desc, deadline) in debts_owed {
                let deadline_str = deadline
                    .map(|d| {
                        if d > epoch {
                            format!(" (due in {} days)", d - epoch)
                        } else {
                            " (OVERDUE!)".to_string()
                        }
                    })
                    .unwrap_or_default();
                debts.push_str(&format!("- {} to **{}**{}\n", service_desc, name, deadline_str));
            }
            sections.push(debts);
        }

        // Credits owed to this agent
        if !credits_owed.is_empty() {
            let mut credits = String::from("\n## Owed to You\n");
            credits.push_str("*Others owe you these services:*\n");
            for (_, name, service_desc, deadline) in credits_owed {
                let deadline_str = deadline
                    .map(|d| {
                        if d > epoch {
                            format!(" (due in {} days)", d - epoch)
                        } else {
                            " (overdue)".to_string()
                        }
                    })
                    .unwrap_or_default();
                credits.push_str(&format!("- **{}** owes you {}{}\n", name, service_desc, deadline_str));
            }
            sections.push(credits);
        }

        sections.join("")
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
            output_config: self.config.effort.clone().map(|effort| OutputConfig { effort }),
            fallbacks: "default",
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
            .header("anthropic-beta", "server-side-fallback-2026-07-01")
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

        match response.stop_reason.as_deref() {
            Some("refusal") => return Err(anyhow!("Request declined (refusal)")),
            Some("max_tokens") => warn!("LLM response hit max_tokens; consider raising it"),
            _ => {}
        }

        response
            .content
            .into_iter()
            .find(|c| c.kind == "text" && !c.text.is_empty())
            .map(|c| c.text)
            .ok_or_else(|| anyhow!("Empty response"))
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-5-5".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            max_tokens: 16000,
            temperature: None,
            effort: Some("low".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_omits_unset_temperature() {
        let config = LlmConfig::default();
        let request = AnthropicRequest {
            model: config.model,
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            output_config: config.effort.map(|effort| OutputConfig { effort }),
            fallbacks: "default",
            system: String::new(),
            messages: vec![],
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("temperature").is_none());
        assert_eq!(json["output_config"]["effort"], "low");
        assert_eq!(json["fallbacks"], "default");
    }

    #[test]
    fn test_response_skips_thinking_block() {
        let body = r#"{"content":[{"type":"thinking","thinking":"","signature":"x"},
                                  {"type":"text","text":"ACTION: GATHER"}],
                       "stop_reason":"end_turn"}"#;
        let response: AnthropicResponse = serde_json::from_str(body).unwrap();
        let text = response.content.into_iter().find(|c| c.kind == "text").map(|c| c.text);
        assert_eq!(text.as_deref(), Some("ACTION: GATHER"));
    }
}
