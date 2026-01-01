use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A simulation event for logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub epoch: usize,
    pub event_type: EventType,
    pub agent: Option<Uuid>,
    pub target: Option<Uuid>,
    pub data: EventData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventType {
    // Physical
    Moved,
    Gathered,
    Ate,
    Rested,
    HealthChanged,
    Died,

    // Social
    Spoke,
    Gave,
    Gossiped,

    // Conflict
    Attacked,
    AllyIntervened,

    // Groups
    GroupFormed,
    GroupDissolved,
    GroupChanged,
    LeadershipChanged,

    // Inter-group relations
    RivalryFormed,
    RivalryChanged,
    RivalryEnded,

    // Reproduction
    Courted,
    Conceived,
    BirthOccurred,

    // Skills
    SkillTaught,

    // Crafting
    GatheredMaterials,
    Crafted,
    Hunted,
    Fished,
    Chopped,
    ToolBroke,

    // Territory
    TerritoryMarked,
    TerritoryChallenged,
    TerritorySubmitted,
    TerritoryFight,
    TerritoryLost,

    // Structures
    FarmProduced,
    StructureDestroyed,

    // Trade
    TradeProposed,
    TradeAccepted,
    TradeDeclined,
    TradeCountered,
    TradeExpired,
    TradeCancelled,
    TradeReneged,
    ServiceFulfilled,

    // Meta
    EpochStart,
    EpochEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Third party in gossip events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub about: Option<Uuid>,
    /// Group name for group events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    /// Member IDs for group events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub members: Option<Vec<Uuid>>,
    /// New leader for leadership change events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_leader: Option<Uuid>,
    /// Old leader for leadership change events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_leader: Option<Uuid>,
    /// Second group name for rivalry events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_b_name: Option<String>,
    /// Rivalry type (hostile, tense, neutral, friendly, allied)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rivalry_type: Option<String>,
    /// Previous rivalry type for rivalry change events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_rivalry_type: Option<String>,
    /// Courtship score for courted events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub courtship_score: Option<f64>,
    /// Parent A for birth events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_a: Option<Uuid>,
    /// Parent B for birth events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_b: Option<Uuid>,
    /// Child for birth events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child: Option<Uuid>,
    /// Child name for birth events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_name: Option<String>,
    /// Skill name for skill events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_name: Option<String>,
    /// Skill level for skill events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_level: Option<f64>,
    /// Materials gathered (name, amount)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub materials: Option<Vec<(String, u32)>>,
    /// Tool name for crafting/tool broke events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Tool quality for crafted events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_quality: Option<String>,
    /// Success for hunt/fish events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,
    /// Territory x coordinate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub territory_x: Option<usize>,
    /// Territory y coordinate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub territory_y: Option<usize>,
    /// Territory strength for territory events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub territory_strength: Option<f64>,
    /// Winner of a fight
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner: Option<Uuid>,
    /// Trade proposal ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_proposal_id: Option<Uuid>,
    /// Items offered in trade (serialized summary)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_offering: Option<String>,
    /// Items requested in trade (serialized summary)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_requesting: Option<String>,
    /// Service type for service events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_type: Option<String>,
    /// Ally who intervened in combat
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ally: Option<Uuid>,
    /// Damage reduction from ally intervention
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_reduction: Option<f64>,
}

impl Event {
    pub fn epoch_start(epoch: usize) -> Self {
        Self {
            epoch,
            event_type: EventType::EpochStart,
            agent: None,
            target: None,
            data: EventData::empty(),
        }
    }

    pub fn epoch_end(epoch: usize) -> Self {
        Self {
            epoch,
            event_type: EventType::EpochEnd,
            agent: None,
            target: None,
            data: EventData::empty(),
        }
    }

    pub fn moved(epoch: usize, agent: Uuid, from: (usize, usize), to: (usize, usize)) -> Self {
        Self {
            epoch,
            event_type: EventType::Moved,
            agent: Some(agent),
            target: None,
            data: EventData {
                from: Some(from),
                to: Some(to),
                ..EventData::empty()
            },
        }
    }

    pub fn gathered(epoch: usize, agent: Uuid, amount: u32) -> Self {
        Self {
            epoch,
            event_type: EventType::Gathered,
            agent: Some(agent),
            target: None,
            data: EventData {
                amount: Some(amount),
                ..EventData::empty()
            },
        }
    }

    pub fn ate(epoch: usize, agent: Uuid) -> Self {
        Self {
            epoch,
            event_type: EventType::Ate,
            agent: Some(agent),
            target: None,
            data: EventData::empty(),
        }
    }

    pub fn rested(epoch: usize, agent: Uuid) -> Self {
        Self {
            epoch,
            event_type: EventType::Rested,
            agent: Some(agent),
            target: None,
            data: EventData::empty(),
        }
    }

    pub fn spoke(epoch: usize, agent: Uuid, target: Uuid, message: &str) -> Self {
        Self {
            epoch,
            event_type: EventType::Spoke,
            agent: Some(agent),
            target: Some(target),
            data: EventData {
                message: Some(message.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn gave(epoch: usize, agent: Uuid, target: Uuid, amount: u32) -> Self {
        Self {
            epoch,
            event_type: EventType::Gave,
            agent: Some(agent),
            target: Some(target),
            data: EventData {
                amount: Some(amount),
                ..EventData::empty()
            },
        }
    }

    pub fn attacked(epoch: usize, agent: Uuid, target: Uuid, damage: f64) -> Self {
        Self {
            epoch,
            event_type: EventType::Attacked,
            agent: Some(agent),
            target: Some(target),
            data: EventData {
                damage: Some(damage),
                ..EventData::empty()
            },
        }
    }

    pub fn ally_intervened(
        epoch: usize,
        attacker: Uuid,
        target: Uuid,
        ally: Uuid,
        damage_reduction: f64,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::AllyIntervened,
            agent: Some(attacker),
            target: Some(target),
            data: EventData {
                ally: Some(ally),
                damage_reduction: Some(damage_reduction),
                ..EventData::empty()
            },
        }
    }

    pub fn died(epoch: usize, agent: Uuid, cause: &str) -> Self {
        Self {
            epoch,
            event_type: EventType::Died,
            agent: Some(agent),
            target: None,
            data: EventData {
                description: Some(cause.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn gossiped(epoch: usize, agent: Uuid, target: Uuid, about: Uuid, sentiment: &str) -> Self {
        Self {
            epoch,
            event_type: EventType::Gossiped,
            agent: Some(agent),
            target: Some(target),
            data: EventData {
                about: Some(about),
                description: Some(sentiment.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn group_formed(epoch: usize, group_name: &str, members: Vec<Uuid>) -> Self {
        Self {
            epoch,
            event_type: EventType::GroupFormed,
            agent: None,
            target: None,
            data: EventData {
                group_name: Some(group_name.to_string()),
                members: Some(members),
                ..EventData::empty()
            },
        }
    }

    pub fn group_dissolved(epoch: usize, group_name: &str, members: Vec<Uuid>) -> Self {
        Self {
            epoch,
            event_type: EventType::GroupDissolved,
            agent: None,
            target: None,
            data: EventData {
                group_name: Some(group_name.to_string()),
                members: Some(members),
                ..EventData::empty()
            },
        }
    }

    pub fn group_changed(epoch: usize, group_name: &str, description: &str) -> Self {
        Self {
            epoch,
            event_type: EventType::GroupChanged,
            agent: None,
            target: None,
            data: EventData {
                group_name: Some(group_name.to_string()),
                description: Some(description.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn leadership_changed(
        epoch: usize,
        group_name: &str,
        old_leader: Option<Uuid>,
        new_leader: Uuid,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::LeadershipChanged,
            agent: None,
            target: None,
            data: EventData {
                group_name: Some(group_name.to_string()),
                old_leader,
                new_leader: Some(new_leader),
                ..EventData::empty()
            },
        }
    }

    pub fn rivalry_formed(
        epoch: usize,
        group_a_name: &str,
        group_b_name: &str,
        rivalry_type: &str,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::RivalryFormed,
            agent: None,
            target: None,
            data: EventData {
                group_name: Some(group_a_name.to_string()),
                group_b_name: Some(group_b_name.to_string()),
                rivalry_type: Some(rivalry_type.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn rivalry_changed(
        epoch: usize,
        group_a_name: &str,
        group_b_name: &str,
        old_type: &str,
        new_type: &str,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::RivalryChanged,
            agent: None,
            target: None,
            data: EventData {
                group_name: Some(group_a_name.to_string()),
                group_b_name: Some(group_b_name.to_string()),
                old_rivalry_type: Some(old_type.to_string()),
                rivalry_type: Some(new_type.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn rivalry_ended(
        epoch: usize,
        group_a_name: &str,
        group_b_name: &str,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::RivalryEnded,
            agent: None,
            target: None,
            data: EventData {
                group_name: Some(group_a_name.to_string()),
                group_b_name: Some(group_b_name.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn courted(epoch: usize, agent: Uuid, target: Uuid, courtship_score: f64) -> Self {
        Self {
            epoch,
            event_type: EventType::Courted,
            agent: Some(agent),
            target: Some(target),
            data: EventData {
                courtship_score: Some(courtship_score),
                ..EventData::empty()
            },
        }
    }

    pub fn conceived(epoch: usize, parent_a: Uuid, parent_b: Uuid) -> Self {
        Self {
            epoch,
            event_type: EventType::Conceived,
            agent: Some(parent_a),
            target: Some(parent_b),
            data: EventData {
                parent_a: Some(parent_a),
                parent_b: Some(parent_b),
                ..EventData::empty()
            },
        }
    }

    pub fn birth_occurred(
        epoch: usize,
        parent_a: Uuid,
        parent_b: Uuid,
        child: Uuid,
        child_name: &str,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::BirthOccurred,
            agent: None,
            target: None,
            data: EventData {
                parent_a: Some(parent_a),
                parent_b: Some(parent_b),
                child: Some(child),
                child_name: Some(child_name.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn skill_taught(
        epoch: usize,
        teacher: Uuid,
        student: Uuid,
        skill_name: &str,
        new_level: f64,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::SkillTaught,
            agent: Some(teacher),
            target: Some(student),
            data: EventData {
                skill_name: Some(skill_name.to_string()),
                skill_level: Some(new_level),
                ..EventData::empty()
            },
        }
    }

    pub fn gathered_materials(epoch: usize, agent: Uuid, materials: Vec<(String, u32)>) -> Self {
        Self {
            epoch,
            event_type: EventType::GatheredMaterials,
            agent: Some(agent),
            target: None,
            data: EventData {
                materials: Some(materials),
                ..EventData::empty()
            },
        }
    }

    pub fn crafted(epoch: usize, agent: Uuid, tool_name: &str, tool_quality: &str) -> Self {
        Self {
            epoch,
            event_type: EventType::Crafted,
            agent: Some(agent),
            target: None,
            data: EventData {
                tool_name: Some(tool_name.to_string()),
                tool_quality: Some(tool_quality.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn hunted(epoch: usize, agent: Uuid, food_gained: u32, success: bool) -> Self {
        Self {
            epoch,
            event_type: EventType::Hunted,
            agent: Some(agent),
            target: None,
            data: EventData {
                amount: Some(food_gained),
                success: Some(success),
                ..EventData::empty()
            },
        }
    }

    pub fn fished(epoch: usize, agent: Uuid, food_gained: u32, success: bool) -> Self {
        Self {
            epoch,
            event_type: EventType::Fished,
            agent: Some(agent),
            target: None,
            data: EventData {
                amount: Some(food_gained),
                success: Some(success),
                ..EventData::empty()
            },
        }
    }

    pub fn chopped(epoch: usize, agent: Uuid, wood_gathered: u32) -> Self {
        Self {
            epoch,
            event_type: EventType::Chopped,
            agent: Some(agent),
            target: None,
            data: EventData {
                amount: Some(wood_gathered),
                ..EventData::empty()
            },
        }
    }

    pub fn tool_broke(epoch: usize, agent: Uuid, tool_name: &str) -> Self {
        Self {
            epoch,
            event_type: EventType::ToolBroke,
            agent: Some(agent),
            target: None,
            data: EventData {
                tool_name: Some(tool_name.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn territory_marked(epoch: usize, agent: Uuid, x: usize, y: usize) -> Self {
        Self {
            epoch,
            event_type: EventType::TerritoryMarked,
            agent: Some(agent),
            target: None,
            data: EventData {
                territory_x: Some(x),
                territory_y: Some(y),
                ..EventData::empty()
            },
        }
    }

    pub fn territory_challenged(
        epoch: usize,
        challenger: Uuid,
        trespasser: Uuid,
        x: usize,
        y: usize,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TerritoryChallenged,
            agent: Some(challenger),
            target: Some(trespasser),
            data: EventData {
                territory_x: Some(x),
                territory_y: Some(y),
                ..EventData::empty()
            },
        }
    }

    pub fn territory_submitted(epoch: usize, challenger: Uuid, trespasser: Uuid) -> Self {
        Self {
            epoch,
            event_type: EventType::TerritorySubmitted,
            agent: Some(challenger),
            target: Some(trespasser),
            data: EventData::empty(),
        }
    }

    pub fn territory_fight(
        epoch: usize,
        challenger: Uuid,
        trespasser: Uuid,
        winner: Uuid,
        x: usize,
        y: usize,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TerritoryFight,
            agent: Some(challenger),
            target: Some(trespasser),
            data: EventData {
                territory_x: Some(x),
                territory_y: Some(y),
                winner: Some(winner),
                ..EventData::empty()
            },
        }
    }

    pub fn territory_lost(epoch: usize, former_owner: Uuid, x: usize, y: usize) -> Self {
        Self {
            epoch,
            event_type: EventType::TerritoryLost,
            agent: Some(former_owner),
            target: None,
            data: EventData {
                territory_x: Some(x),
                territory_y: Some(y),
                ..EventData::empty()
            },
        }
    }

    pub fn farm_produced(epoch: usize, owner: Uuid, x: usize, y: usize, amount: u32) -> Self {
        Self {
            epoch,
            event_type: EventType::FarmProduced,
            agent: Some(owner),
            target: None,
            data: EventData {
                amount: Some(amount),
                territory_x: Some(x),
                territory_y: Some(y),
                ..EventData::empty()
            },
        }
    }

    pub fn structure_destroyed(epoch: usize, owner: Uuid, x: usize, y: usize, structure_type: &str) -> Self {
        Self {
            epoch,
            event_type: EventType::StructureDestroyed,
            agent: Some(owner),
            target: None,
            data: EventData {
                description: Some(structure_type.to_string()),
                territory_x: Some(x),
                territory_y: Some(y),
                ..EventData::empty()
            },
        }
    }

    // Trade events

    pub fn trade_proposed(
        epoch: usize,
        proposer: Uuid,
        recipient: Uuid,
        proposal_id: Uuid,
        offering: &str,
        requesting: &str,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TradeProposed,
            agent: Some(proposer),
            target: Some(recipient),
            data: EventData {
                trade_proposal_id: Some(proposal_id),
                trade_offering: Some(offering.to_string()),
                trade_requesting: Some(requesting.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn trade_accepted(
        epoch: usize,
        proposer: Uuid,
        accepter: Uuid,
        proposal_id: Uuid,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TradeAccepted,
            agent: Some(accepter),
            target: Some(proposer),
            data: EventData {
                trade_proposal_id: Some(proposal_id),
                ..EventData::empty()
            },
        }
    }

    pub fn trade_declined(
        epoch: usize,
        proposer: Uuid,
        decliner: Uuid,
        proposal_id: Uuid,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TradeDeclined,
            agent: Some(decliner),
            target: Some(proposer),
            data: EventData {
                trade_proposal_id: Some(proposal_id),
                ..EventData::empty()
            },
        }
    }

    pub fn trade_countered(
        epoch: usize,
        proposer: Uuid,
        counter_proposer: Uuid,
        original_proposal_id: Uuid,
        new_proposal_id: Uuid,
        offering: &str,
        requesting: &str,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TradeCountered,
            agent: Some(counter_proposer),
            target: Some(proposer),
            data: EventData {
                trade_proposal_id: Some(new_proposal_id),
                trade_offering: Some(offering.to_string()),
                trade_requesting: Some(requesting.to_string()),
                description: Some(format!("counter to {}", original_proposal_id)),
                ..EventData::empty()
            },
        }
    }

    pub fn trade_expired(
        epoch: usize,
        proposal_id: Uuid,
        proposer: Uuid,
        recipient: Uuid,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TradeExpired,
            agent: Some(proposer),
            target: Some(recipient),
            data: EventData {
                trade_proposal_id: Some(proposal_id),
                ..EventData::empty()
            },
        }
    }

    pub fn trade_cancelled(
        epoch: usize,
        proposer: Uuid,
        recipient: Uuid,
        proposal_id: Uuid,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TradeCancelled,
            agent: Some(proposer),
            target: Some(recipient),
            data: EventData {
                trade_proposal_id: Some(proposal_id),
                ..EventData::empty()
            },
        }
    }

    pub fn trade_reneged(
        epoch: usize,
        debtor: Uuid,
        creditor: Uuid,
        service_type: &str,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::TradeReneged,
            agent: Some(debtor),
            target: Some(creditor),
            data: EventData {
                service_type: Some(service_type.to_string()),
                ..EventData::empty()
            },
        }
    }

    pub fn service_fulfilled(
        epoch: usize,
        debtor: Uuid,
        creditor: Uuid,
        service_type: &str,
    ) -> Self {
        Self {
            epoch,
            event_type: EventType::ServiceFulfilled,
            agent: Some(debtor),
            target: Some(creditor),
            data: EventData {
                service_type: Some(service_type.to_string()),
                ..EventData::empty()
            },
        }
    }
}

impl EventData {
    pub fn empty() -> Self {
        Self {
            from: None,
            to: None,
            amount: None,
            message: None,
            damage: None,
            description: None,
            about: None,
            group_name: None,
            members: None,
            new_leader: None,
            old_leader: None,
            group_b_name: None,
            rivalry_type: None,
            old_rivalry_type: None,
            courtship_score: None,
            parent_a: None,
            parent_b: None,
            child: None,
            child_name: None,
            skill_name: None,
            skill_level: None,
            materials: None,
            tool_name: None,
            tool_quality: None,
            success: None,
            territory_x: None,
            territory_y: None,
            territory_strength: None,
            winner: None,
            trade_proposal_id: None,
            trade_offering: None,
            trade_requesting: None,
            service_type: None,
            ally: None,
            damage_reduction: None,
        }
    }
}
