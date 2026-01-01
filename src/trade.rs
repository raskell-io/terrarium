//! Trade and barter system.
//!
//! Enables multi-epoch negotiation between agents with support for:
//! - Physical item trades (food, materials, tools)
//! - Service promises (teaching, helping build, future gifts, alliances)
//! - Counter-offer chains with version tracking
//! - Promise enforcement with reneging penalties

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crafting::{MaterialType, ToolType};

/// Items that can be traded between agents
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TradeableItem {
    /// Food (amount)
    Food(u32),
    /// Materials (type, amount)
    Materials(MaterialType, u32),
    /// A specific tool instance (by tool ID)
    Tool(Uuid),
    /// A tool by type (resolved to specific tool during trade execution)
    ToolByType(ToolType),
    /// Promise to teach a skill (one session)
    TeachSkillPromise { skill: String },
    /// Promise to help build (contribute labor points)
    HelpBuildPromise { labor_points: u32 },
    /// Promise of future food gift (amount, deadline in epochs from now)
    FutureGiftPromise { amount: u32, deadline_epochs: usize },
    /// Alliance/protection promise (duration in epochs)
    AlliancePromise { duration_epochs: usize },
}

impl TradeableItem {
    /// Check if this is a promise (requires ServiceDebt tracking)
    pub fn is_promise(&self) -> bool {
        matches!(
            self,
            TradeableItem::TeachSkillPromise { .. }
                | TradeableItem::HelpBuildPromise { .. }
                | TradeableItem::FutureGiftPromise { .. }
                | TradeableItem::AlliancePromise { .. }
        )
    }

    /// Human-readable description of the item
    pub fn describe(&self) -> String {
        match self {
            TradeableItem::Food(amount) => format!("{} food", amount),
            TradeableItem::Materials(mat, amount) => format!("{} {}", amount, mat.display_name()),
            TradeableItem::Tool(id) => format!("tool {}", &id.to_string()[..8]),
            TradeableItem::ToolByType(tool_type) => tool_type.display_name().to_string(),
            TradeableItem::TeachSkillPromise { skill } => format!("teach {}", skill),
            TradeableItem::HelpBuildPromise { labor_points } => {
                format!("{} labor points", labor_points)
            }
            TradeableItem::FutureGiftPromise {
                amount,
                deadline_epochs,
            } => format!("{} food within {} days", amount, deadline_epochs),
            TradeableItem::AlliancePromise { duration_epochs } => {
                format!("alliance for {} days", duration_epochs)
            }
        }
    }
}

/// Status of a trade proposal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    /// Waiting for recipient response
    Pending,
    /// Trade was accepted and executed
    Accepted,
    /// Trade was declined
    Declined,
    /// Trade was counter-offered (superseded by new proposal)
    Countered,
    /// Trade expired without response
    Expired,
    /// Proposer cancelled the offer
    Cancelled,
}

/// A trade proposal between two agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeProposal {
    /// Unique proposal ID
    pub id: Uuid,
    /// Agent making the proposal
    pub proposer: Uuid,
    /// Agent receiving the proposal
    pub recipient: Uuid,
    /// What the proposer offers
    pub offering: Vec<TradeableItem>,
    /// What the proposer wants in return
    pub requesting: Vec<TradeableItem>,
    /// Epoch when proposal was created
    pub created_epoch: usize,
    /// Epoch when proposal expires
    pub expires_epoch: usize,
    /// If this is a counter-offer, the ID of the original proposal
    pub counter_to: Option<Uuid>,
    /// Version number for multi-round negotiation tracking (1 = original, 2 = first counter, etc.)
    pub version: u32,
    /// Current status
    pub status: ProposalStatus,
}

impl TradeProposal {
    /// Create a new trade proposal
    pub fn new(
        proposer: Uuid,
        recipient: Uuid,
        offering: Vec<TradeableItem>,
        requesting: Vec<TradeableItem>,
        epoch: usize,
        expiry_epochs: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            proposer,
            recipient,
            offering,
            requesting,
            created_epoch: epoch,
            expires_epoch: epoch + expiry_epochs,
            counter_to: None,
            version: 1,
            status: ProposalStatus::Pending,
        }
    }

    /// Create a counter-offer based on an existing proposal
    pub fn counter(
        original: &TradeProposal,
        offering: Vec<TradeableItem>,
        requesting: Vec<TradeableItem>,
        epoch: usize,
        expiry_epochs: usize,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            // Swap proposer/recipient for counter
            proposer: original.recipient,
            recipient: original.proposer,
            offering,
            requesting,
            created_epoch: epoch,
            expires_epoch: epoch + expiry_epochs,
            counter_to: Some(original.id),
            version: original.version + 1,
            status: ProposalStatus::Pending,
        }
    }

    /// Check if proposal is still valid
    pub fn is_pending(&self) -> bool {
        self.status == ProposalStatus::Pending
    }

    /// Check if proposal has expired
    pub fn is_expired(&self, epoch: usize) -> bool {
        epoch >= self.expires_epoch
    }

    /// Epochs remaining until expiry
    pub fn epochs_remaining(&self, epoch: usize) -> usize {
        self.expires_epoch.saturating_sub(epoch)
    }

    /// Human-readable description of what's offered
    pub fn offering_description(&self) -> String {
        self.offering
            .iter()
            .map(|i| i.describe())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Human-readable description of what's requested
    pub fn requesting_description(&self) -> String {
        self.requesting
            .iter()
            .map(|i| i.describe())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Types of services that can be promised
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServiceType {
    /// Promise to teach a skill
    TeachSkill { skill: String },
    /// Promise to help build (tracks progress toward labor goal)
    HelpBuild {
        labor_points: u32,
        labor_contributed: u32,
    },
    /// Promise of future food gift (tracks partial fulfillment)
    FutureGift { amount: u32, amount_given: u32 },
    /// Alliance/protection (passive, expires at epoch)
    Alliance { expires_epoch: usize },
}

impl ServiceType {
    /// Check if service is fulfilled
    pub fn is_fulfilled(&self) -> bool {
        match self {
            ServiceType::TeachSkill { .. } => false, // Requires explicit action
            ServiceType::HelpBuild {
                labor_points,
                labor_contributed,
            } => labor_contributed >= labor_points,
            ServiceType::FutureGift { amount, amount_given } => amount_given >= amount,
            ServiceType::Alliance { expires_epoch: _ } => false, // Never "fulfilled", just expires
        }
    }

    /// Human-readable description
    pub fn describe(&self) -> String {
        match self {
            ServiceType::TeachSkill { skill } => format!("teach {}", skill),
            ServiceType::HelpBuild {
                labor_points,
                labor_contributed,
            } => format!("{}/{} labor contributed", labor_contributed, labor_points),
            ServiceType::FutureGift { amount, amount_given } => {
                format!("{}/{} food given", amount_given, amount)
            }
            ServiceType::Alliance { expires_epoch } => {
                format!("alliance until day {}", expires_epoch)
            }
        }
    }
}

/// A promised service that must be fulfilled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDebt {
    /// Unique debt ID
    pub id: Uuid,
    /// The debtor (who owes the service)
    pub debtor: Uuid,
    /// The creditor (who is owed the service)
    pub creditor: Uuid,
    /// Type of service owed
    pub service: ServiceType,
    /// Epoch when promise was made
    pub created_epoch: usize,
    /// Epoch by which service must be fulfilled (None = no deadline, or alliance expiry)
    pub deadline_epoch: Option<usize>,
    /// Whether the service has been fulfilled
    pub fulfilled: bool,
    /// Whether reneging penalty has been applied
    pub reneged: bool,
    /// Original trade proposal that created this debt
    pub source_trade: Uuid,
}

impl ServiceDebt {
    /// Create a new service debt from a tradeable item promise
    pub fn from_promise(
        item: &TradeableItem,
        debtor: Uuid,
        creditor: Uuid,
        source_trade: Uuid,
        epoch: usize,
        default_deadline: usize,
    ) -> Option<Self> {
        let (service, deadline_epoch) = match item {
            TradeableItem::TeachSkillPromise { skill } => (
                ServiceType::TeachSkill {
                    skill: skill.clone(),
                },
                Some(epoch + default_deadline),
            ),
            TradeableItem::HelpBuildPromise { labor_points } => (
                ServiceType::HelpBuild {
                    labor_points: *labor_points,
                    labor_contributed: 0,
                },
                Some(epoch + default_deadline),
            ),
            TradeableItem::FutureGiftPromise {
                amount,
                deadline_epochs,
            } => (
                ServiceType::FutureGift {
                    amount: *amount,
                    amount_given: 0,
                },
                Some(epoch + deadline_epochs),
            ),
            TradeableItem::AlliancePromise { duration_epochs } => (
                ServiceType::Alliance {
                    expires_epoch: epoch + duration_epochs,
                },
                None, // Alliances don't have deadlines, they just expire
            ),
            _ => return None, // Not a promise
        };

        Some(Self {
            id: Uuid::new_v4(),
            debtor,
            creditor,
            service,
            created_epoch: epoch,
            deadline_epoch,
            fulfilled: false,
            reneged: false,
            source_trade,
        })
    }

    /// Check if deadline has passed
    pub fn is_overdue(&self, epoch: usize) -> bool {
        if let Some(deadline) = self.deadline_epoch {
            epoch > deadline && !self.fulfilled
        } else {
            false
        }
    }

    /// Check if alliance is still active
    pub fn is_alliance_active(&self, epoch: usize) -> bool {
        if let ServiceType::Alliance { expires_epoch } = &self.service {
            epoch < *expires_epoch
        } else {
            false
        }
    }

    /// Mark as fulfilled
    pub fn mark_fulfilled(&mut self) {
        self.fulfilled = true;
    }

    /// Mark as reneged
    pub fn mark_reneged(&mut self) {
        self.reneged = true;
    }

    /// Add progress to help build debt
    pub fn add_labor(&mut self, points: u32) {
        if let ServiceType::HelpBuild {
            labor_contributed, ..
        } = &mut self.service
        {
            *labor_contributed += points;
            if self.service.is_fulfilled() {
                self.fulfilled = true;
            }
        }
    }

    /// Add progress to future gift debt
    pub fn add_gift(&mut self, amount: u32) {
        if let ServiceType::FutureGift { amount_given, .. } = &mut self.service {
            *amount_given += amount;
            if self.service.is_fulfilled() {
                self.fulfilled = true;
            }
        }
    }
}

/// Trade system state held by the engine
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TradeState {
    /// All proposals indexed by ID
    pub proposals: HashMap<Uuid, TradeProposal>,
    /// Active service debts
    pub service_debts: Vec<ServiceDebt>,
}

impl TradeState {
    /// Create empty trade state
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new proposal
    pub fn add_proposal(&mut self, proposal: TradeProposal) {
        self.proposals.insert(proposal.id, proposal);
    }

    /// Get proposal by ID
    pub fn get_proposal(&self, id: Uuid) -> Option<&TradeProposal> {
        self.proposals.get(&id)
    }

    /// Get mutable proposal by ID
    pub fn get_proposal_mut(&mut self, id: Uuid) -> Option<&mut TradeProposal> {
        self.proposals.get_mut(&id)
    }

    /// Get pending proposals for a recipient
    pub fn pending_proposals_for(&self, recipient: Uuid) -> Vec<&TradeProposal> {
        self.proposals
            .values()
            .filter(|p| p.recipient == recipient && p.status == ProposalStatus::Pending)
            .collect()
    }

    /// Get pending proposals from a proposer
    pub fn pending_proposals_from(&self, proposer: Uuid) -> Vec<&TradeProposal> {
        self.proposals
            .values()
            .filter(|p| p.proposer == proposer && p.status == ProposalStatus::Pending)
            .collect()
    }

    /// Count pending proposals from an agent
    pub fn count_pending_from(&self, proposer: Uuid) -> usize {
        self.pending_proposals_from(proposer).len()
    }

    /// Add a service debt
    pub fn add_debt(&mut self, debt: ServiceDebt) {
        self.service_debts.push(debt);
    }

    /// Get debts owed by an agent
    pub fn debts_owed_by(&self, debtor: Uuid) -> Vec<&ServiceDebt> {
        self.service_debts
            .iter()
            .filter(|d| d.debtor == debtor && !d.fulfilled && !d.reneged)
            .collect()
    }

    /// Get debts owed to an agent
    pub fn debts_owed_to(&self, creditor: Uuid) -> Vec<&ServiceDebt> {
        self.service_debts
            .iter()
            .filter(|d| d.creditor == creditor && !d.fulfilled && !d.reneged)
            .collect()
    }

    /// Get mutable debt by ID
    pub fn get_debt_mut(&mut self, id: Uuid) -> Option<&mut ServiceDebt> {
        self.service_debts.iter_mut().find(|d| d.id == id)
    }

    /// Check if there's an active alliance between two agents
    pub fn has_alliance(&self, agent_a: Uuid, agent_b: Uuid, epoch: usize) -> bool {
        self.service_debts.iter().any(|d| {
            ((d.debtor == agent_a && d.creditor == agent_b)
                || (d.debtor == agent_b && d.creditor == agent_a))
                && d.is_alliance_active(epoch)
        })
    }

    /// Clean up old completed/expired proposals (keep last N for history)
    pub fn cleanup_old_proposals(&mut self, keep_count: usize) {
        let mut completed: Vec<_> = self
            .proposals
            .iter()
            .filter(|(_, p)| p.status != ProposalStatus::Pending)
            .map(|(id, p)| (*id, p.created_epoch))
            .collect();

        // Sort by epoch, oldest first
        completed.sort_by_key(|(_, epoch)| *epoch);

        // Remove oldest if over limit
        let to_remove = completed.len().saturating_sub(keep_count);
        for (id, _) in completed.into_iter().take(to_remove) {
            self.proposals.remove(&id);
        }
    }
}
