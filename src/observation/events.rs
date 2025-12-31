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
        }
    }
}
