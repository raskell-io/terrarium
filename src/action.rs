use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crafting::{MaterialType, ToolType};
use crate::structures::StructureType;
use crate::trade::TradeableItem;

/// Actions an agent can take
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Do nothing, recover a bit of energy
    Wait,
    /// Move in a direction
    Move(Direction),
    /// Gather food from current location
    Gather,
    /// Consume food from inventory
    Eat,
    /// Rest to recover energy
    Rest,
    /// Say something to a nearby agent
    Speak { target: Uuid, message: String },
    /// Give food to a nearby agent
    Give { target: Uuid, amount: u32 },
    /// Attack a nearby agent
    Attack { target: Uuid },
    /// Share opinion about another agent (gossip)
    Gossip { target: Uuid, about: Uuid },
    /// Court a nearby agent (advance courtship)
    Court { target: Uuid },
    /// Attempt to mate with a nearby agent (requires mutual consent and courtship threshold)
    Mate { target: Uuid },
    /// Teach a skill to a nearby agent
    Teach { target: Uuid, skill: String },
    /// Gather materials (wood, stone, fiber) from current location
    GatherMaterials,
    /// Craft a tool from materials
    Craft { tool: ToolType },
    /// Hunt for food and materials (requires spear or bow)
    Hunt,
    /// Fish for food (requires fishing pole)
    Fish,
    /// Chop wood efficiently (requires axe)
    Chop,
    // Structure actions
    /// Build or continue building a structure at current location
    Build { structure_type: StructureType },
    /// Enter a shelter at current location
    EnterShelter,
    /// Leave the current shelter
    LeaveShelter,
    /// Deposit materials into a storage structure
    Deposit { material: MaterialType, amount: u32 },
    /// Withdraw materials from a storage structure
    Withdraw { material: MaterialType, amount: u32 },
    /// Grant access to your structure
    Permit { target: Uuid },
    /// Revoke access from your structure
    Deny { target: Uuid },
    // Territory actions
    /// Mark current cell as your territory
    Mark,
    /// Challenge a trespasser on your territory
    Challenge { target: Uuid },
    /// Submit to a territorial challenge and leave peacefully
    Submit,
    /// Fight back against a territorial challenge
    Fight,
    // Trade actions
    /// Propose a trade to a nearby agent
    TradeOffer {
        target: Uuid,
        offering: Vec<TradeableItem>,
        requesting: Vec<TradeableItem>,
    },
    /// Accept a pending trade proposal
    TradeAccept { proposal_index: usize },
    /// Decline a pending trade proposal
    TradeDecline { proposal_index: usize },
    /// Counter a trade proposal with modified terms
    TradeCounter {
        proposal_index: usize,
        offering: Vec<TradeableItem>,
        requesting: Vec<TradeableItem>,
    },
    /// Cancel your own pending proposal
    TradeCancel { proposal_index: usize },
}

/// Movement directions (8-directional)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl Direction {
    /// Get the delta for this direction
    pub fn delta(&self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
            Direction::NorthEast => (1, -1),
            Direction::NorthWest => (-1, -1),
            Direction::SouthEast => (1, 1),
            Direction::SouthWest => (-1, 1),
        }
    }

    /// Get direction name for display
    pub fn name(&self) -> &'static str {
        match self {
            Direction::North => "north",
            Direction::South => "south",
            Direction::East => "east",
            Direction::West => "west",
            Direction::NorthEast => "northeast",
            Direction::NorthWest => "northwest",
            Direction::SouthEast => "southeast",
            Direction::SouthWest => "southwest",
        }
    }

    /// Parse direction from string
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.to_lowercase();
        match s.as_str() {
            "n" | "north" => Some(Direction::North),
            "s" | "south" => Some(Direction::South),
            "e" | "east" => Some(Direction::East),
            "w" | "west" => Some(Direction::West),
            "ne" | "northeast" => Some(Direction::NorthEast),
            "nw" | "northwest" => Some(Direction::NorthWest),
            "se" | "southeast" => Some(Direction::SouthEast),
            "sw" | "southwest" => Some(Direction::SouthWest),
            _ => None,
        }
    }
}

impl Action {
    /// Parse an action from LLM response text
    pub fn parse(text: &str, nearby_agents: &[(Uuid, &str)]) -> Option<Self> {
        let text = text.to_uppercase();
        let words: Vec<&str> = text.split_whitespace().collect();

        if words.is_empty() {
            return Some(Action::Wait);
        }

        match words[0] {
            "WAIT" => Some(Action::Wait),
            "MOVE" => {
                if words.len() > 1 {
                    Direction::parse(words[1]).map(Action::Move)
                } else {
                    None
                }
            }
            "GATHER" => Some(Action::Gather),
            "EAT" => Some(Action::Eat),
            "REST" => Some(Action::Rest),
            "SPEAK" => {
                if words.len() >= 3 {
                    let target_name = words[1].to_lowercase();
                    let message = words[2..].join(" ");
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Speak { target, message })
                } else {
                    None
                }
            }
            "GIVE" => {
                if words.len() >= 3 {
                    let target_name = words[1].to_lowercase();
                    let amount = words[2].parse().unwrap_or(1);
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Give { target, amount })
                } else {
                    None
                }
            }
            "ATTACK" => {
                if words.len() >= 2 {
                    let target_name = words[1].to_lowercase();
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Attack { target })
                } else {
                    None
                }
            }
            "GOSSIP" => {
                // GOSSIP <target> <about>
                if words.len() >= 3 {
                    let target_name = words[1].to_lowercase();
                    let about_name = words[2].to_lowercase();
                    let target = find_agent_by_name(&target_name, nearby_agents);
                    let about = find_agent_by_name(&about_name, nearby_agents);
                    match (target, about) {
                        (Some(t), Some(a)) if t != a => Some(Action::Gossip { target: t, about: a }),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            "COURT" => {
                if words.len() >= 2 {
                    let target_name = words[1].to_lowercase();
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Court { target })
                } else {
                    None
                }
            }
            "MATE" => {
                if words.len() >= 2 {
                    let target_name = words[1].to_lowercase();
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Mate { target })
                } else {
                    None
                }
            }
            "TEACH" => {
                // TEACH <target> <skill>
                if words.len() >= 3 {
                    let target_name = words[1].to_lowercase();
                    let skill = words[2].to_lowercase();
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Teach { target, skill })
                } else {
                    None
                }
            }
            "GATHER_MATERIALS" | "GATHER_MAT" | "COLLECT" => Some(Action::GatherMaterials),
            "CRAFT" => {
                if words.len() >= 2 {
                    let tool_name = words[1..].join("_").to_lowercase();
                    ToolType::parse(&tool_name).map(|tool| Action::Craft { tool })
                } else {
                    None
                }
            }
            "HUNT" => Some(Action::Hunt),
            "FISH" => Some(Action::Fish),
            "CHOP" => Some(Action::Chop),
            // Structure actions
            "BUILD" => {
                if words.len() >= 2 {
                    let structure_name = words[1..].join("_").to_lowercase();
                    StructureType::parse(&structure_name).map(|structure_type| Action::Build { structure_type })
                } else {
                    None
                }
            }
            "ENTER" | "ENTER_SHELTER" => Some(Action::EnterShelter),
            "LEAVE" | "LEAVE_SHELTER" => Some(Action::LeaveShelter),
            "DEPOSIT" => {
                // DEPOSIT <material> [amount]
                if words.len() >= 2 {
                    let material_name = words[1].to_lowercase();
                    let amount = if words.len() >= 3 {
                        words[2].parse().unwrap_or(1)
                    } else {
                        1
                    };
                    MaterialType::parse(&material_name).map(|material| Action::Deposit { material, amount })
                } else {
                    None
                }
            }
            "WITHDRAW" => {
                // WITHDRAW <material> [amount]
                if words.len() >= 2 {
                    let material_name = words[1].to_lowercase();
                    let amount = if words.len() >= 3 {
                        words[2].parse().unwrap_or(1)
                    } else {
                        1
                    };
                    MaterialType::parse(&material_name).map(|material| Action::Withdraw { material, amount })
                } else {
                    None
                }
            }
            "PERMIT" => {
                if words.len() >= 2 {
                    let target_name = words[1].to_lowercase();
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Permit { target })
                } else {
                    None
                }
            }
            "DENY" => {
                if words.len() >= 2 {
                    let target_name = words[1].to_lowercase();
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Deny { target })
                } else {
                    None
                }
            }
            // Territory actions
            "MARK" | "CLAIM" => Some(Action::Mark),
            "CHALLENGE" => {
                if words.len() >= 2 {
                    let target_name = words[1].to_lowercase();
                    find_agent_by_name(&target_name, nearby_agents)
                        .map(|target| Action::Challenge { target })
                } else {
                    None
                }
            }
            "SUBMIT" | "YIELD" | "LEAVE_TERRITORY" => Some(Action::Submit),
            "FIGHT" | "RESIST" | "DEFEND" => Some(Action::Fight),
            // Trade actions
            "TRADE" | "OFFER" => {
                // TRADE <name> OFFER <items> FOR <items>
                // or TRADE <name> <items> FOR <items>
                if words.len() >= 5 {
                    let target_name = words[1].to_lowercase();
                    let target = find_agent_by_name(&target_name, nearby_agents)?;

                    // Find OFFER and FOR keywords
                    let offer_start = words.iter().position(|&w| w == "OFFER").unwrap_or(2);
                    let for_pos = words.iter().position(|&w| w == "FOR")?;

                    let offer_words = &words[offer_start + 1..for_pos];
                    let request_words = &words[for_pos + 1..];

                    let offering = parse_tradeable_items(offer_words);
                    let requesting = parse_tradeable_items(request_words);

                    if offering.is_empty() || requesting.is_empty() {
                        return None;
                    }

                    Some(Action::TradeOffer { target, offering, requesting })
                } else {
                    None
                }
            }
            "ACCEPT" => {
                // ACCEPT TRADE <number> or ACCEPT <number>
                let num_pos = if words.len() >= 3 && words[1] == "TRADE" { 2 } else { 1 };
                if words.len() > num_pos {
                    words[num_pos].parse::<usize>().ok()
                        .map(|n| Action::TradeAccept { proposal_index: n.saturating_sub(1) })
                } else {
                    None
                }
            }
            "DECLINE" | "REJECT" => {
                // DECLINE TRADE <number> or DECLINE <number>
                let num_pos = if words.len() >= 3 && words[1] == "TRADE" { 2 } else { 1 };
                if words.len() > num_pos {
                    words[num_pos].parse::<usize>().ok()
                        .map(|n| Action::TradeDecline { proposal_index: n.saturating_sub(1) })
                } else {
                    None
                }
            }
            "COUNTER" => {
                // COUNTER <number> OFFER <items> FOR <items>
                if words.len() >= 6 {
                    let proposal_num = words[1].parse::<usize>().ok()?;
                    let proposal_index = proposal_num.saturating_sub(1);

                    let offer_start = words.iter().position(|&w| w == "OFFER")?;
                    let for_pos = words.iter().position(|&w| w == "FOR")?;

                    let offer_words = &words[offer_start + 1..for_pos];
                    let request_words = &words[for_pos + 1..];

                    let offering = parse_tradeable_items(offer_words);
                    let requesting = parse_tradeable_items(request_words);

                    if offering.is_empty() || requesting.is_empty() {
                        return None;
                    }

                    Some(Action::TradeCounter { proposal_index, offering, requesting })
                } else {
                    None
                }
            }
            "CANCEL" => {
                // CANCEL TRADE <number> or CANCEL <number>
                let num_pos = if words.len() >= 3 && words[1] == "TRADE" { 2 } else { 1 };
                if words.len() > num_pos {
                    words[num_pos].parse::<usize>().ok()
                        .map(|n| Action::TradeCancel { proposal_index: n.saturating_sub(1) })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Describe the action for logging
    pub fn describe(&self, agent_name: &str, agents: &[(Uuid, &str)]) -> String {
        match self {
            Action::Wait => format!("{} waits", agent_name),
            Action::Move(dir) => format!("{} moves {}", agent_name, dir.name()),
            Action::Gather => format!("{} gathers food", agent_name),
            Action::Eat => format!("{} eats", agent_name),
            Action::Rest => format!("{} rests", agent_name),
            Action::Speak { target, message } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} says to {}: \"{}\"", agent_name, target_name, message)
            }
            Action::Give { target, amount } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} gives {} food to {}", agent_name, amount, target_name)
            }
            Action::Attack { target } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} attacks {}", agent_name, target_name)
            }
            Action::Gossip { target, about } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                let about_name = find_name_by_id(*about, agents).unwrap_or("someone");
                format!("{} gossips to {} about {}", agent_name, target_name, about_name)
            }
            Action::Court { target } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} courts {}", agent_name, target_name)
            }
            Action::Mate { target } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} attempts to mate with {}", agent_name, target_name)
            }
            Action::Teach { target, skill } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} teaches {} to {}", agent_name, skill, target_name)
            }
            Action::GatherMaterials => format!("{} gathers materials", agent_name),
            Action::Craft { tool } => format!("{} crafts a {}", agent_name, tool.display_name()),
            Action::Hunt => format!("{} hunts for prey", agent_name),
            Action::Fish => format!("{} fishes", agent_name),
            Action::Chop => format!("{} chops wood", agent_name),
            Action::Build { structure_type } => {
                format!("{} builds a {}", agent_name, structure_type.display_name())
            }
            Action::EnterShelter => format!("{} enters shelter", agent_name),
            Action::LeaveShelter => format!("{} leaves shelter", agent_name),
            Action::Deposit { material, amount } => {
                format!("{} deposits {} {}", agent_name, amount, material.display_name())
            }
            Action::Withdraw { material, amount } => {
                format!("{} withdraws {} {}", agent_name, amount, material.display_name())
            }
            Action::Permit { target } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} permits {} to use their structure", agent_name, target_name)
            }
            Action::Deny { target } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} denies {} access to their structure", agent_name, target_name)
            }
            Action::Mark => format!("{} marks territory", agent_name),
            Action::Challenge { target } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                format!("{} challenges {} for trespassing", agent_name, target_name)
            }
            Action::Submit => format!("{} submits and leaves the territory", agent_name),
            Action::Fight => format!("{} fights to defend their position", agent_name),
            Action::TradeOffer { target, offering, requesting } => {
                let target_name = find_name_by_id(*target, agents).unwrap_or("someone");
                let offer_str: Vec<_> = offering.iter().map(|i| i.describe()).collect();
                let request_str: Vec<_> = requesting.iter().map(|i| i.describe()).collect();
                format!("{} offers {} to {} for {}", agent_name, offer_str.join(", "), target_name, request_str.join(", "))
            }
            Action::TradeAccept { proposal_index } => {
                format!("{} accepts trade offer #{}", agent_name, proposal_index + 1)
            }
            Action::TradeDecline { proposal_index } => {
                format!("{} declines trade offer #{}", agent_name, proposal_index + 1)
            }
            Action::TradeCounter { proposal_index, offering, requesting } => {
                let offer_str: Vec<_> = offering.iter().map(|i| i.describe()).collect();
                let request_str: Vec<_> = requesting.iter().map(|i| i.describe()).collect();
                format!("{} counter-offers trade #{}: {} for {}", agent_name, proposal_index + 1, offer_str.join(", "), request_str.join(", "))
            }
            Action::TradeCancel { proposal_index } => {
                format!("{} cancels their trade offer #{}", agent_name, proposal_index + 1)
            }
        }
    }

    /// Get the list of available actions for prompting
    /// teachable_skills: list of skill names this agent can teach (level >= 0.5)
    /// unlocked_actions: actions unlocked by having specific tools (e.g., "hunt", "fish", "chop")
    /// craftable_tools: tool names that can currently be crafted
    /// buildable_structures: structures that can be built at current location
    /// has_shelter: whether there's an accessible shelter at current location
    /// has_storage: whether there's an accessible storage at current location
    /// owns_structure: whether agent owns a structure at current location
    /// is_sheltered: whether agent is currently sheltered
    /// can_mark_territory: whether agent can claim current cell as territory
    /// trespassers: list of trespassers the agent can challenge
    /// is_challenged: whether the agent has been challenged on someone's territory
    /// pending_trade_offers: list of (index, proposer_name, offer_desc, request_desc) for received proposals
    /// my_pending_proposals: count of proposals this agent has sent
    #[allow(clippy::too_many_arguments)]
    pub fn available_actions_prompt(
        nearby_agents: &[(Uuid, &str)],
        teachable_skills: &[&String],
        unlocked_actions: &[&str],
        craftable_tools: &[ToolType],
        buildable_structures: &[StructureType],
        has_shelter: bool,
        has_storage: bool,
        owns_structure: bool,
        is_sheltered: bool,
        can_mark_territory: bool,
        trespassers: &[(Uuid, &str)],
        is_challenged: bool,
        pending_trade_offers: &[(usize, &str, String, String)],
        my_pending_proposals: usize,
    ) -> String {
        let mut actions: Vec<String> = vec![
            "WAIT - do nothing, recover energy".to_string(),
            "MOVE <direction> - move (north/south/east/west/ne/nw/se/sw)".to_string(),
            "GATHER - collect food from current location".to_string(),
            "EAT - eat food from your inventory".to_string(),
            "REST - rest to recover energy".to_string(),
            "GATHER_MATERIALS - collect wood, stone, fiber, or flint from the terrain".to_string(),
        ];

        // Tool-unlocked actions
        if unlocked_actions.contains(&"hunt") {
            actions.push("HUNT - hunt for food and materials (requires spear or bow)".to_string());
        }
        if unlocked_actions.contains(&"fish") {
            actions.push("FISH - fish for food (requires fishing pole)".to_string());
        }
        if unlocked_actions.contains(&"chop") {
            actions.push("CHOP - efficiently chop wood (requires axe)".to_string());
        }

        // Crafting
        if !craftable_tools.is_empty() {
            let tools_list = craftable_tools
                .iter()
                .map(|t| t.display_name())
                .collect::<Vec<_>>()
                .join(", ");
            actions.push(format!("CRAFT <tool> - craft a tool (available: {})", tools_list));
        }

        // Structure actions
        if !buildable_structures.is_empty() {
            let structures_list = buildable_structures
                .iter()
                .map(|s| s.display_name())
                .collect::<Vec<_>>()
                .join(", ");
            actions.push(format!("BUILD <structure> - build a structure (available: {})", structures_list));
        }

        if has_shelter && !is_sheltered {
            actions.push("ENTER - enter a shelter for protection".to_string());
        }
        if is_sheltered {
            actions.push("LEAVE - leave the shelter".to_string());
        }

        if has_storage {
            actions.push("DEPOSIT <material> [amount] - deposit materials into storage".to_string());
            actions.push("WITHDRAW <material> [amount] - withdraw materials from storage".to_string());
        }

        if owns_structure && !nearby_agents.is_empty() {
            actions.push("PERMIT <name> - grant someone access to your structure".to_string());
            actions.push("DENY <name> - revoke someone's access to your structure".to_string());
        }

        // Territory actions
        if can_mark_territory {
            actions.push("MARK - claim this cell as your territory (max 4 cells)".to_string());
        }

        if !trespassers.is_empty() {
            let names = trespassers.iter().map(|(_, n)| *n).collect::<Vec<_>>().join(", ");
            actions.push(format!("CHALLENGE <name> - challenge a trespasser on your territory ({})", names));
        }

        if is_challenged {
            actions.push("SUBMIT - leave the territory peacefully".to_string());
            actions.push("FIGHT - fight to stay on the territory".to_string());
        }

        if !nearby_agents.is_empty() {
            actions.push("SPEAK <name> <message> - say something to someone nearby".to_string());
            actions.push("GIVE <name> <amount> - give food to someone nearby".to_string());
            actions.push("ATTACK <name> - attack someone nearby".to_string());
            if nearby_agents.len() >= 2 {
                actions.push("GOSSIP <name> <about> - share your opinion about <about> with <name>".to_string());
            }
            actions.push("COURT <name> - court someone nearby (builds courtship over time)".to_string());
            actions.push("MATE <name> - attempt to mate with someone (requires mutual consent and sufficient courtship)".to_string());

            // Show TEACH if agent has teachable skills
            if !teachable_skills.is_empty() {
                let skills_list = teachable_skills.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("/");
                actions.push(format!("TEACH <name> <skill> - teach a skill ({}) to someone nearby", skills_list));
            }

            // Trade actions
            actions.push("TRADE <name> OFFER <items> FOR <items> - propose a trade".to_string());
            actions.push("  Items: food amount, material amount (wood/stone/etc), tool name (stone axe/bow/etc), TEACH <skill>, labor <amount>".to_string());
        }

        // Trade response actions (only if there are pending offers)
        if !pending_trade_offers.is_empty() {
            for (idx, proposer, offer, request) in pending_trade_offers {
                actions.push(format!("  #{}: {} offers {} for {}", idx + 1, proposer, offer, request));
            }
            actions.push("ACCEPT TRADE <#> - accept a pending trade offer".to_string());
            actions.push("DECLINE TRADE <#> - decline a pending trade offer".to_string());
            actions.push("COUNTER TRADE <#> OFFER <items> FOR <items> - counter-offer".to_string());
        }

        // Cancel own proposals
        if my_pending_proposals > 0 {
            actions.push(format!("CANCEL TRADE <#> - cancel one of your {} pending proposals", my_pending_proposals));
        }

        actions.join("\n")
    }
}

/// Find agent UUID by name
fn find_agent_by_name(name: &str, agents: &[(Uuid, &str)]) -> Option<Uuid> {
    agents
        .iter()
        .find(|(_, n)| n.to_lowercase().starts_with(name))
        .map(|(id, _)| *id)
}

/// Find agent name by UUID
fn find_name_by_id<'a>(id: Uuid, agents: &'a [(Uuid, &'a str)]) -> Option<&'a str> {
    agents.iter().find(|(i, _)| *i == id).map(|(_, n)| *n)
}

/// Parse tradeable items from a list of words
/// Supports formats like:
/// - "5 FOOD" or "FOOD 5"
/// - "3 WOOD" or "WOOD 3"
/// - "TEACH HUNTING"
/// - "5 LABOR" or "LABOR 5"
/// - "10 FOOD WITHIN 20" (future gift)
/// - "ALLIANCE 30" (alliance for 30 epochs)
fn parse_tradeable_items(words: &[&str]) -> Vec<TradeableItem> {
    let mut items = Vec::new();
    let mut i = 0;

    while i < words.len() {
        let word = words[i].to_uppercase();

        // Try to parse as amount + type or type + amount
        if let Ok(amount) = word.parse::<u32>() {
            // Amount first: "5 FOOD", "3 WOOD"
            if i + 1 < words.len() {
                let type_word = words[i + 1].to_uppercase();
                if let Some(item) = parse_item_type(&type_word, amount, &words[i + 2..]) {
                    items.push(item.0);
                    i += 2 + item.1; // Skip amount, type, and any extra words consumed
                    continue;
                }
            }
        } else if let Some(item) = parse_item_type_first(&word, &words[i + 1..]) {
            // Type first with amount: "WOOD 3", "FOOD 5"
            // Or promise types: "TEACH HUNTING", "ALLIANCE 30"
            items.push(item.0);
            i += 1 + item.1;
            continue;
        }

        i += 1;
    }

    items
}

/// Parse an item type with amount coming after the type name
/// Returns (item, words_consumed)
fn parse_item_type_first(type_word: &str, rest: &[&str]) -> Option<(TradeableItem, usize)> {
    match type_word {
        "FOOD" => {
            if let Some(amount) = rest.first().and_then(|w| w.parse::<u32>().ok()) {
                Some((TradeableItem::Food(amount), 1))
            } else {
                Some((TradeableItem::Food(1), 0))
            }
        }
        "WOOD" | "STONE" | "FIBER" | "FLINT" | "HIDE" | "BONE" => {
            let mat = parse_material_type(type_word)?;
            if let Some(amount) = rest.first().and_then(|w| w.parse::<u32>().ok()) {
                Some((TradeableItem::Materials(mat, amount), 1))
            } else {
                Some((TradeableItem::Materials(mat, 1), 0))
            }
        }
        "TEACH" => {
            if let Some(skill) = rest.first() {
                Some((TradeableItem::TeachSkillPromise { skill: skill.to_lowercase() }, 1))
            } else {
                None
            }
        }
        "LABOR" => {
            if let Some(amount) = rest.first().and_then(|w| w.parse::<u32>().ok()) {
                Some((TradeableItem::HelpBuildPromise { labor_points: amount }, 1))
            } else {
                Some((TradeableItem::HelpBuildPromise { labor_points: 5 }, 0))
            }
        }
        "ALLIANCE" | "PROTECTION" => {
            if let Some(duration) = rest.first().and_then(|w| w.parse::<usize>().ok()) {
                Some((TradeableItem::AlliancePromise { duration_epochs: duration }, 1))
            } else {
                Some((TradeableItem::AlliancePromise { duration_epochs: 30 }, 0))
            }
        }
        // Tool types - try parsing as tool name
        _ => {
            // Handle multi-word tool names (e.g., "STONE AXE" -> first word is "STONE", rest[0] is "AXE")
            let mut tool_name = type_word.to_lowercase();
            let mut consumed = 0;

            // Check if next word completes a tool name
            if !rest.is_empty() {
                let combined = format!("{}_{}", type_word.to_lowercase(), rest[0].to_lowercase());
                if ToolType::parse(&combined).is_some() {
                    tool_name = combined;
                    consumed = 1;
                }
            }

            ToolType::parse(&tool_name).map(|t| (TradeableItem::ToolByType(t), consumed))
        }
    }
}

/// Parse an item type with amount provided
/// Returns (item, extra_words_consumed)
fn parse_item_type(type_word: &str, amount: u32, rest: &[&str]) -> Option<(TradeableItem, usize)> {
    match type_word {
        "FOOD" => Some((TradeableItem::Food(amount), 0)),
        "WOOD" | "STONE" | "FIBER" | "FLINT" | "HIDE" | "BONE" => {
            let mat = parse_material_type(type_word)?;
            Some((TradeableItem::Materials(mat, amount), 0))
        }
        "LABOR" => Some((TradeableItem::HelpBuildPromise { labor_points: amount }, 0)),
        // Future gift: "10 FOOD WITHIN 20" (amount already parsed as 10)
        // Actually this is complex, need to handle differently
        // For now, just treat FOOD/WITHIN as future gift pattern
        _ if type_word == "FOOD" && rest.len() >= 2 && rest[0] == "WITHIN" => {
            if let Ok(deadline) = rest[1].parse::<usize>() {
                Some((TradeableItem::FutureGiftPromise { amount, deadline_epochs: deadline }, 2))
            } else {
                Some((TradeableItem::Food(amount), 0))
            }
        }
        _ => None,
    }
}

/// Parse material type from string
fn parse_material_type(s: &str) -> Option<MaterialType> {
    match s {
        "WOOD" => Some(MaterialType::Wood),
        "STONE" => Some(MaterialType::Stone),
        "FIBER" => Some(MaterialType::Fiber),
        "FLINT" => Some(MaterialType::Flint),
        "HIDE" => Some(MaterialType::Hide),
        "BONE" => Some(MaterialType::Bone),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_actions() {
        assert!(matches!(Action::parse("WAIT", &[]), Some(Action::Wait)));
        assert!(matches!(Action::parse("GATHER", &[]), Some(Action::Gather)));
        assert!(matches!(Action::parse("EAT", &[]), Some(Action::Eat)));
        assert!(matches!(Action::parse("REST", &[]), Some(Action::Rest)));
    }

    #[test]
    fn test_parse_move() {
        assert!(matches!(
            Action::parse("MOVE NORTH", &[]),
            Some(Action::Move(Direction::North))
        ));
        assert!(matches!(
            Action::parse("MOVE SE", &[]),
            Some(Action::Move(Direction::SouthEast))
        ));
    }

    #[test]
    fn test_direction_delta() {
        assert_eq!(Direction::North.delta(), (0, -1));
        assert_eq!(Direction::SouthEast.delta(), (1, 1));
    }
}
