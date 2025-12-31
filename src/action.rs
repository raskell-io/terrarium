use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        }
    }

    /// Get the list of available actions for prompting
    /// teachable_skills: list of skill names this agent can teach (level >= 0.5)
    pub fn available_actions_prompt(nearby_agents: &[(Uuid, &str)], teachable_skills: &[&String]) -> String {
        let mut actions: Vec<String> = vec![
            "WAIT - do nothing, recover energy".to_string(),
            "MOVE <direction> - move (north/south/east/west/ne/nw/se/sw)".to_string(),
            "GATHER - collect food from current location".to_string(),
            "EAT - eat food from your inventory".to_string(),
            "REST - rest to recover energy".to_string(),
        ];

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
