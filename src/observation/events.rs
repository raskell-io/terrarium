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
        }
    }
}
