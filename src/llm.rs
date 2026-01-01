use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::action::Action;
use crate::agent::Agent;
use crate::trade::TradeableItem;

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
    /// pending_trades: Vec of (proposal_index, proposer_id, proposer_name, offering_desc, requesting_desc, expires_in)
    /// debts_owed: Vec of (creditor_id, creditor_name, service_description, deadline_in) for debts the agent owes
    /// credits_owed: Vec of (debtor_id, debtor_name, service_description, deadline_in) for debts others owe this agent
    /// my_proposals: number of pending trade proposals this agent has made
    pub async fn decide_action(
        &self,
        agent: &Agent,
        world_perception: &str,
        nearby_agents: &[(uuid::Uuid, &str)],
        epoch: usize,
        pending_trades: &[(usize, uuid::Uuid, &str, String, String, Option<usize>)],
        debts_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        credits_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        my_proposals: usize,
    ) -> Result<Action> {
        // If no API key, use heuristic
        if !self.is_available() {
            return Ok(heuristic_action(agent, nearby_agents, pending_trades, debts_owed));
        }

        let prompt = self.build_prompt(
            agent,
            world_perception,
            nearby_agents,
            epoch,
            pending_trades,
            debts_owed,
            credits_owed,
            my_proposals,
        );
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
        pending_trades: &[(usize, uuid::Uuid, &str, String, String, Option<usize>)],
        debts_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        credits_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
        my_proposals: usize,
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

        // Build trade context section
        let trade_context = self.build_trade_context(
            pending_trades,
            debts_owed,
            credits_owed,
            epoch,
        );

        // Get teachable skills for action prompt
        let teachable_skills = agent.skills.teachable_skills();

        // Get tool-unlocked actions (already lowercase from unlocked_actions)
        let unlocked_actions = agent.physical.unlocked_actions();

        // For now, empty craftable tools and structures (would need Registry access)
        let craftable_tools = Vec::new();
        let buildable_structures = Vec::new();

        // Structure-related context (set during engine run)
        let has_shelter = false;
        let has_storage = false;
        let owns_structure = false;
        let is_sheltered = agent.physical.sheltered_at.is_some();

        // Territory-related context (would be computed by engine)
        let can_mark_territory = false; // Placeholder - engine should compute this
        let trespassers: Vec<(uuid::Uuid, &str)> = Vec::new(); // Placeholder
        let is_challenged = false; // Placeholder - engine should track pending challenges

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
                &craftable_tools,
                &buildable_structures,
                has_shelter,
                has_storage,
                owns_structure,
                is_sheltered,
                can_mark_territory,
                &trespassers,
                is_challenged,
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
/// pending_trades: Vec of (proposal_index, proposer_id, proposer_name, offering_desc, requesting_desc, expires_in)
/// debts_owed: Vec of (creditor_id, creditor_name, service_description, deadline_in) for debts the agent owes
fn heuristic_action(
    agent: &Agent,
    nearby_agents: &[(uuid::Uuid, &str)],
    pending_trades: &[(usize, uuid::Uuid, &str, String, String, Option<usize>)],
    debts_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
) -> Action {
    use rand::Rng;
    let mut rng = rand::rng();

    // Priority 0a: Fulfill debts to nearby creditors
    for (creditor_id, creditor_name, service_desc, _deadline) in debts_owed {
        // Check if creditor is nearby
        if !nearby_agents.iter().any(|(id, _)| id == creditor_id) {
            continue;
        }

        // TeachSkill debt - teach the skill if we can
        if service_desc.starts_with("teach ") {
            let skill = service_desc.strip_prefix("teach ").unwrap_or("");
            if agent.skills.level(skill) >= 0.3 {
                debug!("Heuristic: fulfilling teach debt to {} - teaching {}", creditor_name, skill);
                return Action::Teach {
                    target: *creditor_id,
                    skill: skill.to_string(),
                };
            }
        }

        // FutureGift debt - give food if we have enough
        if service_desc.contains("food given") {
            if agent.physical.food > 3 {
                let amount = (agent.physical.food / 2).max(1);
                debug!("Heuristic: fulfilling gift debt to {} - giving {} food", creditor_name, amount);
                return Action::Give {
                    target: *creditor_id,
                    amount,
                };
            }
        }

        // HelpBuild debt - we'd need to move to their structure location
        // For now, just note this in debug
        if service_desc.contains("labor") {
            debug!("Heuristic: has HelpBuild debt to {}, need to find their structure", creditor_name);
        }
    }

    // Priority 0b: Respond to pending trade offers
    if !pending_trades.is_empty() {
        // Evaluate each offer
        for (idx, _proposer_id, proposer_name, offering, requesting, _expires) in pending_trades {
            // Simple heuristic: accept if they're offering food and we need it
            // or if they're offering materials and we have excess food
            // Promises contain "within" (e.g., "5 food within 15 days")
            let is_promise = offering.contains("within") || offering.contains("teach") || offering.contains("labor");
            let offering_immediate_food = offering.contains("food") && !is_promise;
            let need_food = agent.physical.food < 5;
            let offering_materials = offering.contains("wood") || offering.contains("stone");
            let have_excess_food = agent.physical.food > 8;

            // Check for tool trades
            let offering_tool = offering.contains("axe") || offering.contains("bow")
                || offering.contains("spear") || offering.contains("knife")
                || offering.contains("pole") || offering.contains("rope") || offering.contains("basket");
            let requesting_tool = requesting.contains("axe") || requesting.contains("bow")
                || requesting.contains("spear") || requesting.contains("knife")
                || requesting.contains("pole") || requesting.contains("rope") || requesting.contains("basket");
            let have_duplicate_tools = agent.physical.tools.len() > 1;

            // Check what they're requesting
            let requesting_wood = requesting.contains("wood");
            let requesting_food = requesting.contains("food");
            let have_wood = agent.physical.materials.get(&crate::crafting::MaterialType::Wood).copied().unwrap_or(0) >= 2;
            let have_stone = agent.physical.materials.get(&crate::crafting::MaterialType::Stone).copied().unwrap_or(0) >= 2;

            // Accept if:
            // 1. They're offering immediate food and we need it
            // 2. They're offering materials and we have excess food
            // 3. They're promising future food and we have the requested materials
            // 4. They're offering a tool and we don't have many tools
            // 5. They're requesting a tool and we have duplicates
            let favorable = (offering_immediate_food && need_food)
                || (offering_materials && have_excess_food)
                || (is_promise && requesting_wood && have_wood)
                || (offering_tool && agent.physical.tools.is_empty())
                || (requesting_tool && have_duplicate_tools && (offering_immediate_food || offering_materials));

            if favorable {
                // 70% chance to accept a favorable trade
                if rng.random::<f64>() < 0.7 {
                    debug!("Heuristic: accepting trade {} from {}", idx, proposer_name);
                    return Action::TradeAccept { proposal_index: *idx };
                }
            }

            // Consider counter-offer if we want what they're offering but can't meet their request
            // Or if we think we can get a better deal
            let could_counter = !favorable && rng.random::<f64>() < 0.25;

            if could_counter {
                // Build a counter-offer based on what we have
                // Key rule: don't offer the same type we're requesting!
                let mut counter_offering = Vec::new();
                let mut counter_requesting = Vec::new();

                // Determine what we want from them (what they originally offered)
                let want_food = offering_immediate_food && need_food;
                let want_wood = offering.contains("wood");
                let want_stone = offering.contains("stone");

                // Determine what we can offer (different from what we want)
                if want_food {
                    // We want food, so offer materials
                    if have_wood {
                        counter_offering.push(TradeableItem::Materials(crate::crafting::MaterialType::Wood, 1));
                    } else if have_stone {
                        counter_offering.push(TradeableItem::Materials(crate::crafting::MaterialType::Stone, 1));
                    }
                    counter_requesting.push(TradeableItem::Food(3));
                } else if want_wood {
                    // We want wood, so offer food or stone
                    if agent.physical.food > 4 {
                        counter_offering.push(TradeableItem::Food(3));
                    } else if have_stone {
                        counter_offering.push(TradeableItem::Materials(crate::crafting::MaterialType::Stone, 2));
                    }
                    counter_requesting.push(TradeableItem::Materials(crate::crafting::MaterialType::Wood, 1));
                } else if want_stone {
                    // We want stone, so offer food or wood
                    if agent.physical.food > 4 {
                        counter_offering.push(TradeableItem::Food(3));
                    } else if have_wood {
                        counter_offering.push(TradeableItem::Materials(crate::crafting::MaterialType::Wood, 2));
                    }
                    counter_requesting.push(TradeableItem::Materials(crate::crafting::MaterialType::Stone, 1));
                } else if requesting_wood && !have_wood && have_stone {
                    // They want wood we don't have, offer stone instead for food
                    counter_offering.push(TradeableItem::Materials(crate::crafting::MaterialType::Stone, 2));
                    counter_requesting.push(TradeableItem::Food(3));
                } else if requesting_food && agent.physical.food > 4 && (have_wood || have_stone) {
                    // They want food, we have some - ask for materials
                    counter_offering.push(TradeableItem::Food(2));
                    if !have_wood {
                        counter_requesting.push(TradeableItem::Materials(crate::crafting::MaterialType::Wood, 1));
                    } else if !have_stone {
                        counter_requesting.push(TradeableItem::Materials(crate::crafting::MaterialType::Stone, 1));
                    }
                }

                // Only counter if we have a valid, sensible offer
                if !counter_offering.is_empty() && !counter_requesting.is_empty() {
                    debug!("Heuristic: counter-offering trade {} to {}", idx, proposer_name);
                    return Action::TradeCounter {
                        proposal_index: *idx,
                        offering: counter_offering,
                        requesting: counter_requesting,
                    };
                }
            }

            // Decline unfavorable trades with some probability
            if rng.random::<f64>() < 0.3 {
                debug!("Heuristic: declining trade {} from {}", idx, proposer_name);
                return Action::TradeDecline { proposal_index: *idx };
            }
        }
    }

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

    // Priority 6: Court if extraverted/agreeable and conditions are right
    if (agent.identity.personality.extraversion > 0.5 || agent.identity.personality.agreeableness > 0.5)
        && !nearby_agents.is_empty()
        && agent.reproduction.mating_cooldown == 0
        && agent.reproduction.gestation.is_none()
        && agent.physical.health > 0.5
        && agent.physical.energy > 0.4
        && rng.random::<f64>() < 0.2  // 20% chance
    {
        // Find someone we have positive feelings about or a stranger
        let potential_partners: Vec<_> = nearby_agents.iter()
            .filter(|(id, _)| {
                agent.beliefs.social.get(id)
                    .map(|b| b.sentiment >= 0.0)  // Not hostile
                    .unwrap_or(true)  // Or unknown
            })
            .collect();

        if !potential_partners.is_empty() {
            let (target, _) = potential_partners[rng.random_range(0..potential_partners.len())];
            return Action::Court { target: *target };
        }
    }

    // Priority 7: Try to mate if courtship is high enough
    if !nearby_agents.is_empty()
        && agent.reproduction.mating_cooldown == 0
        && agent.reproduction.gestation.is_none()
        && agent.physical.health > 0.5
        && agent.physical.energy > 0.4
        && agent.physical.food >= 5  // Need food for mating
    {
        // Find someone with high courtship score
        let mate_candidate = agent.reproduction.courtship_progress.iter()
            .filter(|(id, score)| {
                **score >= 0.7  // Threshold met
                    && nearby_agents.iter().any(|(nid, _)| nid == *id)  // Is nearby
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id);

        if let Some(target) = mate_candidate {
            return Action::Mate { target };
        }
    }

    // Priority 8: Teach if skilled and agreeable
    let teachable = agent.skills.teachable_skills();
    if !teachable.is_empty()
        && !nearby_agents.is_empty()
        && agent.physical.energy > 0.3
        && (agent.identity.personality.agreeableness > 0.5
            || agent.skills.level("teaching") > 0.5)
        && rng.random::<f64>() < 0.15  // 15% chance
    {
        // Pick a random skill to teach and a random nearby agent
        let skill = teachable[rng.random_range(0..teachable.len())].clone();
        let (target, _) = nearby_agents[rng.random_range(0..nearby_agents.len())];
        return Action::Teach { target, skill };
    }

    // Priority 9: Trade if we have materials and could use food (or vice versa)
    if !nearby_agents.is_empty() && rng.random::<f64>() < 0.25 {
        // Check if we have excess materials to trade for food
        let total_materials: u32 = agent.physical.materials.values().sum();

        if total_materials > 3 && agent.physical.food < 5 {
            // Offer materials for food
            if let Some((mat_type, amount)) = agent.physical.materials.iter()
                .filter(|(_, amt)| **amt >= 2)
                .map(|(m, a)| (*m, *a))
                .next()
            {
                let (target, _) = nearby_agents[rng.random_range(0..nearby_agents.len())];
                let trade_amount = amount.min(3);
                return Action::TradeOffer {
                    target,
                    offering: vec![TradeableItem::Materials(mat_type, trade_amount)],
                    requesting: vec![TradeableItem::Food(trade_amount * 2)],
                };
            }
        }

        // Or if we have excess food and need materials
        if agent.physical.food > 8 && total_materials < 5 {
            let (target, _) = nearby_agents[rng.random_range(0..nearby_agents.len())];
            return Action::TradeOffer {
                target,
                offering: vec![TradeableItem::Food(3)],
                requesting: vec![TradeableItem::Materials(crate::crafting::MaterialType::Wood, 2)],
            };
        }

        // Sometimes offer a future gift promise for immediate materials
        if total_materials < 3 && agent.physical.food < 8 && rng.random::<f64>() < 0.3 {
            let (target, _) = nearby_agents[rng.random_range(0..nearby_agents.len())];
            // Promise 5 food in exchange for 2 wood now
            return Action::TradeOffer {
                target,
                offering: vec![TradeableItem::FutureGiftPromise { amount: 5, deadline_epochs: 15 }],
                requesting: vec![TradeableItem::Materials(crate::crafting::MaterialType::Wood, 2)],
            };
        }
    }

    // Otherwise: random action
    match rng.random_range(0..12) {
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
        7..=8 => Action::GatherMaterials, // Gather wood, stone, etc.
        9 => Action::Rest,
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
