use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::action::{Action, ActionContext};
use crate::agent::Agent;
use crate::trade::TradeableItem;
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
        // If no API key, use heuristic
        if !self.is_available() {
            return Ok(heuristic_action(agent, local_food, context, nearby_agents, pending_trades, debts_owed, mate_proposals));
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
                warn!("LLM call failed for {}: {}. Using heuristic.", agent.name(), e);
                return Ok(heuristic_action(agent, local_food, context, nearby_agents, pending_trades, debts_owed, mate_proposals));
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
        let nearby_list: Vec<String> = nearby_agents
            .iter()
            .map(|(_, name)| name.to_string())
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

/// Heuristic action when no LLM available
/// pending_trades: Vec of (proposal_index, proposer_id, proposer_name, offering_desc, requesting_desc, expires_in)
/// debts_owed: Vec of (creditor_id, creditor_name, service_description, deadline_in) for debts the agent owes
fn heuristic_action(
    agent: &Agent,
    local_food: &LocalFood,
    context: &ActionContext,
    nearby_agents: &[(uuid::Uuid, &str)],
    pending_trades: &[(usize, uuid::Uuid, &str, String, String, Option<usize>)],
    debts_owed: &[(uuid::Uuid, &str, String, Option<usize>)],
    mate_proposals: &[(uuid::Uuid, &str)],
) -> Action {
    use rand::RngExt;
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

    // Priority 3: Hungry, disagreeable agents with no food take it by force when none is in sight,
    // from whoever nearby they like least (never a friend)
    if agent.physical.food == 0
        && agent.physical.hunger > 0.5
        && local_food.here == 0
        && local_food.adjacent.is_empty()
        && agent.identity.personality.agreeableness < 0.4
    {
        let sentiment = |id: &uuid::Uuid| agent.beliefs.social.get(id).map(|b| b.sentiment).unwrap_or(0.0);
        let victim = nearby_agents
            .iter()
            .filter(|(id, _)| sentiment(id) < 0.3)
            .min_by(|(a, _), (b, _)| sentiment(a).partial_cmp(&sentiment(b)).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((target, name)) = victim {
            debug!("Heuristic: desperate, attacking {} for food", name);
            return Action::Attack { target: *target };
        }
    }

    let pos = (agent.physical.x, agent.physical.y);
    let toward = |target: (usize, usize)| crate::action::Direction::toward(pos, target).map(Action::Move);

    // Priority 4: Find food if running low, falling back on stored food when none is in reach
    if agent.physical.food < 3 {
        if let Some((x, y, stored)) = context.storage {
            if stored > 0 && local_food.here == 0 {
                if (x, y) == pos {
                    return Action::WithdrawFood { amount: 5 };
                }
                if local_food.adjacent.is_empty() {
                    if let Some(step) = toward((x, y)) {
                        return step;
                    }
                }
            }
        }
        return seek_food(agent, local_food, &mut rng);
    }

    // Priority 5: Shelter, storage, building and territory
    if context.has_shelter && !context.is_sheltered {
        return Action::EnterShelter;
    }
    // With a storehouse, stock up: gather a surplus and carry it home before it spoils,
    // keeping a few days' worth in hand
    if let Some((x, y, _)) = context.storage {
        if agent.physical.food > 4 && (x, y) == pos {
            return Action::DepositFood { amount: agent.physical.food - 4 };
        }
        if agent.physical.food >= 10 && rng.random::<f64>() < 0.6 {
            if let Some(step) = toward((x, y)) {
                return step;
            }
        }
        if agent.physical.food < 10 && local_food.here > 0 && rng.random::<f64>() < 0.4 {
            return Action::Gather;
        }
    }
    // Defend our land: disagreeable owners challenge trespassers
    if let Some((target, _)) = context.trespassers.first() {
        if agent.identity.personality.agreeableness < 0.5 && rng.random::<f64>() < 0.6 {
            return Action::Challenge { target: *target };
        }
    }
    // Finish what we started: work on our project here, or walk back to it when fed and rested
    if let Some((x, y, structure_type, _)) = context.unfinished_project {
        if (x, y) == pos {
            if agent.physical.energy > 0.3 {
                return Action::Build { structure_type };
            }
        } else if agent.physical.hunger < 0.5 && agent.physical.energy > 0.4 && rng.random::<f64>() < 0.8 {
            if let Some(step) = toward((x, y)) {
                return step;
            }
        }
    }
    // Lend a hand on a friend's unfinished structure
    if let Some((owner, name)) = &context.unfinished_here_owner {
        let sentiment = agent.beliefs.social.get(owner).map(|b| b.sentiment).unwrap_or(0.0);
        if agent.physical.energy > 0.4 && (sentiment > 0.3 || agent.identity.personality.agreeableness > 0.6) && rng.random::<f64>() < 0.5 {
            debug!("Heuristic: helping {} build", name);
            return Action::Build { structure_type: context.buildable_structures[0] };
        }
    }
    // Settle down around home: claim the land next to it
    if let Some(home) = context.home {
        let near_home = home.0.abs_diff(pos.0) <= 1 && home.1.abs_diff(pos.1) <= 1;
        if near_home && context.can_mark_territory && !context.here_barren && rng.random::<f64>() < 0.3 {
            return Action::Mark;
        }
    }
    // Conscientious agents without a project work toward a home, then a storehouse,
    // gathering the materials for it first (wood and fiber on fertile land, stone on barren)
    if context.unfinished_project.is_none()
        && agent.identity.personality.conscientiousness > 0.5
        && rng.random::<f64>() < 0.5
    {
        use crate::structures::StructureType;
        let goal = if context.home.is_none() {
            Some([StructureType::Shelter, StructureType::LeanTo])
        } else if context.storage.is_none() {
            Some([StructureType::Storage, StructureType::Storage])
        } else {
            None
        };
        if let Some(goal) = goal {
            if let Some(structure_type) = goal.iter().find(|s| context.buildable_structures.contains(s)) {
                return Action::Build { structure_type: *structure_type };
            }
            return gather_materials_for(agent, goal[1], context, &mut rng);
        }
    }
    // Craft a tool we don't have yet, preferring ones that unlock new ways to get food
    if rng.random::<f64>() < 0.3 {
        use crate::crafting::ToolType;
        let preferred = [ToolType::WoodenSpear, ToolType::StoneAxe, ToolType::Rope];
        let wanted = preferred
            .iter()
            .chain(context.craftable_tools.iter())
            .find(|t| context.craftable_tools.contains(t) && !agent.physical.has_tool(**t));
        if let Some(tool) = wanted {
            return Action::Craft { tool: *tool };
        }
    }

    // Priority 6: Give food to nearby hungry agent if agreeable
    if agent.identity.personality.agreeableness > 0.7
        && agent.physical.food > 5
        && !nearby_agents.is_empty()
    {
        let (target, _) = nearby_agents[rng.random_range(0..nearby_agents.len())];
        return Action::Give { target, amount: 1 };
    }

    // Priority 7: Gossip if extraverted and have opinions to share
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

    // Courting is free; mating additionally needs energy and food
    let can_reproduce = agent.reproduction.mating_cooldown == 0
        && agent.reproduction.gestation.is_none()
        && agent.physical.health > 0.5;
    let ready_to_mate = can_reproduce && agent.physical.food >= 5;
    let rested = agent.physical.energy > 0.4;

    // Priority 8: Accept a mating proposal from someone we don't dislike
    // (resting first if too tired - proposals stay open for a few days)
    if ready_to_mate {
        let accepted = mate_proposals.iter().find(|(id, _)| {
            agent.beliefs.social.get(id).map(|b| b.sentiment >= 0.0).unwrap_or(true)
        });
        if let Some((proposer, name)) = accepted {
            if !rested {
                return Action::Rest;
            }
            debug!("Heuristic: accepting mating proposal from {}", name);
            return Action::Mate { target: *proposer };
        }
    }

    // Priority 9: Propose mating once courtship with a nearby partner is strong
    if ready_to_mate {
        let mate_candidate = agent.reproduction.courtship_progress.iter()
            .filter(|(id, score)| {
                **score >= 0.7 && nearby_agents.iter().any(|(nid, _)| nid == *id)
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id);

        if let Some(target) = mate_candidate {
            return if rested { Action::Mate { target } } else { Action::Rest };
        }
    }

    // Priority 10: Court - keep up an existing courtship (ours or one directed at us),
    // occasionally start a new one if sociable
    if can_reproduce && !nearby_agents.is_empty() {
        let current_partner = agent.reproduction.courtship_progress.iter()
            .filter(|(id, _)| nearby_agents.iter().any(|(nid, _)| nid == *id))
            .filter(|(id, _)| agent.beliefs.social.get(id).map(|b| b.sentiment >= 0.0).unwrap_or(true))
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| *id);

        if let Some(target) = current_partner {
            if rng.random::<f64>() < 0.6 {
                return Action::Court { target };
            }
        } else if (agent.identity.personality.extraversion > 0.5
            || agent.identity.personality.agreeableness > 0.5)
            && rng.random::<f64>() < 0.2
        {
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
    }

    // Priority 11: Teach if skilled and agreeable
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

    // Priority 12: Trade if we have materials and could use food (or vice versa)
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
        5..=6 => seek_food(agent, local_food, &mut rng),
        7..=8 => Action::GatherMaterials, // Gather wood, stone, etc.
        9 => Action::Rest,
        _ => Action::Wait,
    }
}

/// Gather materials for a structure, walking to the right terrain for whatever is still missing
fn gather_materials_for(
    agent: &Agent,
    structure_type: crate::structures::StructureType,
    context: &ActionContext,
    rng: &mut impl rand::RngExt,
) -> Action {
    use crate::crafting::MaterialType;
    let registry = crate::structures::StructureRecipeRegistry::new();
    let missing: Vec<MaterialType> = registry
        .get(structure_type)
        .map(|r| {
            r.materials
                .iter()
                .filter(|(m, n)| agent.physical.materials.get(m).copied().unwrap_or(0) < **n)
                .map(|(m, _)| *m)
                .collect()
        })
        .unwrap_or_default();
    if missing.is_empty() {
        // We have everything; this cell just isn't free to build on
        return random_move(rng);
    }
    let needs_stone = missing.iter().any(|m| matches!(m, MaterialType::Stone | MaterialType::Flint));
    let needs_plants = missing.iter().any(|m| matches!(m, MaterialType::Wood | MaterialType::Fiber));
    let pos = (agent.physical.x, agent.physical.y);
    let (wanted_here, elsewhere) = if needs_plants && (!needs_stone || !context.here_barren) {
        (!context.here_barren, context.adjacent_fertile)
    } else {
        (context.here_barren, context.adjacent_barren)
    };
    if wanted_here {
        return Action::GatherMaterials;
    }
    match elsewhere.and_then(|target| crate::action::Direction::toward(pos, target)) {
        Some(dir) => Action::Move(dir),
        None => random_move(rng),
    }
}

fn random_move(rng: &mut impl rand::RngExt) -> Action {
    use crate::action::Direction;
    let directions = [
        Direction::North, Direction::South, Direction::East, Direction::West,
        Direction::NorthEast, Direction::NorthWest, Direction::SouthEast, Direction::SouthWest,
    ];
    Action::Move(directions[rng.random_range(0..8)])
}

/// Gather if there is food here; otherwise step toward the richest visible cell,
/// then toward the most recently remembered food location, else hunt (if armed) or explore
fn seek_food(agent: &Agent, local_food: &LocalFood, rng: &mut impl rand::RngExt) -> Action {
    use crate::action::Direction;

    if local_food.here > 0 {
        return Action::Gather;
    }

    let pos = (agent.physical.x, agent.physical.y);
    let target = local_food
        .adjacent
        .iter()
        .max_by_key(|(_, _, amount)| *amount)
        .map(|(x, y, _)| (*x, *y))
        .or_else(|| {
            agent
                .beliefs
                .world
                .food_locations
                .iter()
                .filter(|loc| loc.belief.amount > 0 && (loc.x, loc.y) != pos)
                .max_by_key(|loc| loc.belief.last_seen_epoch)
                .map(|loc| (loc.x, loc.y))
        });

    if let Some(dir) = target.and_then(|t| Direction::toward(pos, t)) {
        return Action::Move(dir);
    }

    // Nothing in sight or remembered: hunt if armed, otherwise explore
    if agent.physical.has_tool(crate::crafting::ToolType::WoodenSpear) && rng.random::<f64>() < 0.5 {
        return Action::Hunt;
    }

    let directions = [
        Direction::North,
        Direction::South,
        Direction::East,
        Direction::West,
        Direction::NorthEast,
        Direction::NorthWest,
        Direction::SouthEast,
        Direction::SouthWest,
    ];
    Action::Move(directions[rng.random_range(0..8)])
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
