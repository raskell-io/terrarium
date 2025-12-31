//! Group and alliance detection from shared beliefs.
//!
//! Groups are detected when 3+ agents have mutual trust above a threshold.
//! This module analyzes the social belief graph to find emergent alliances.
//! Inter-group rivalries are detected based on cross-group trust/distrust.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::agent::Agent;

/// Minimum trust for considering two agents as allies
const TRUST_THRESHOLD: f64 = 0.3;

/// Minimum group size
const MIN_GROUP_SIZE: usize = 3;

/// Thresholds for inter-group relationship classification
const HOSTILE_THRESHOLD: f64 = -0.3;
const TENSE_THRESHOLD: f64 = -0.1;
const FRIENDLY_THRESHOLD: f64 = 0.1;
const ALLIED_THRESHOLD: f64 = 0.3;

/// A detected group/alliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Stable group identifier
    pub id: Uuid,
    /// Member agent IDs
    pub members: HashSet<Uuid>,
    /// Epoch when first detected
    pub formed_epoch: usize,
    /// Average mutual trust within the group
    pub average_trust: f64,
    /// Average mutual sentiment within the group
    pub average_sentiment: f64,
    /// Agents that all members distrust (shared enemies)
    pub shared_enemies: Vec<Uuid>,
    /// Human-readable group name (generated)
    pub name: String,
    /// Detected leader (highest leadership score)
    pub leader: Option<Uuid>,
    /// Members ranked by leadership score (descending)
    pub hierarchy: Vec<(Uuid, f64)>,
}

/// Type of inter-group relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RivalryType {
    /// Groups are actively hostile (avg cross-trust < -0.3)
    Hostile,
    /// Groups have tension (avg cross-trust < -0.1)
    Tense,
    /// Groups are neutral (avg cross-trust between -0.1 and 0.1)
    Neutral,
    /// Groups are friendly (avg cross-trust > 0.1)
    Friendly,
    /// Groups are allied (avg cross-trust > 0.3, shared enemies)
    Allied,
}

impl RivalryType {
    pub fn describe(&self) -> &'static str {
        match self {
            RivalryType::Hostile => "hostile",
            RivalryType::Tense => "tense",
            RivalryType::Neutral => "neutral",
            RivalryType::Friendly => "friendly",
            RivalryType::Allied => "allied",
        }
    }

    pub fn is_conflict(&self) -> bool {
        matches!(self, RivalryType::Hostile | RivalryType::Tense)
    }
}

/// Relationship between two groups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rivalry {
    /// First group ID
    pub group_a: Uuid,
    /// Second group ID
    pub group_b: Uuid,
    /// Type of relationship
    pub rivalry_type: RivalryType,
    /// Average cross-group trust (members of A toward members of B, and vice versa)
    pub avg_cross_trust: f64,
    /// Average cross-group sentiment
    pub avg_cross_sentiment: f64,
    /// Whether groups share common enemies
    pub shared_enemies: bool,
    /// Epoch when this relationship was first detected
    pub since_epoch: usize,
}

/// Tracks groups over time
#[derive(Debug, Clone, Default)]
pub struct GroupTracker {
    /// Currently active groups
    pub groups: Vec<Group>,
    /// Groups that have dissolved
    pub dissolved: Vec<(Group, usize)>, // (group, dissolution_epoch)
    /// Next group number for naming
    next_group_num: usize,
    /// Current inter-group rivalries
    pub rivalries: Vec<Rivalry>,
}

/// Result of group detection for an epoch
#[derive(Debug, Clone, Default)]
pub struct GroupChanges {
    /// Newly formed groups
    pub formed: Vec<Group>,
    /// Groups that dissolved
    pub dissolved: Vec<Group>,
    /// Groups that changed membership
    pub changed: Vec<(Group, Vec<Uuid>, Vec<Uuid>)>, // (group, added, removed)
    /// Groups where leadership changed (group, old_leader, new_leader)
    pub leadership_changed: Vec<(Group, Option<Uuid>, Uuid)>,
    /// New rivalries detected
    pub rivalries_formed: Vec<Rivalry>,
    /// Rivalries that changed type (rivalry, old_type, new_type)
    pub rivalries_changed: Vec<(Rivalry, RivalryType, RivalryType)>,
    /// Rivalries that ended (groups no longer both exist)
    pub rivalries_ended: Vec<Rivalry>,
}

impl GroupTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Detect groups from current agent states
    /// Returns changes since last detection
    pub fn detect(&mut self, agents: &[Agent], epoch: usize) -> GroupChanges {
        let mut changes = GroupChanges::default();

        // Build the mutual trust graph
        let trust_graph = build_trust_graph(agents);

        // Find all cliques of size >= MIN_GROUP_SIZE
        let cliques = find_cliques(&trust_graph, MIN_GROUP_SIZE);

        // Convert cliques to groups
        let mut new_groups: Vec<Group> = cliques
            .into_iter()
            .map(|members| {
                let (avg_trust, avg_sentiment) = calculate_group_metrics(&members, agents);
                let shared_enemies = find_shared_enemies(&members, agents);
                let hierarchy = calculate_hierarchy(&members, agents);
                let leader = hierarchy.first().map(|(id, _)| *id);

                Group {
                    id: Uuid::new_v4(),
                    members,
                    formed_epoch: epoch,
                    average_trust: avg_trust,
                    average_sentiment: avg_sentiment,
                    shared_enemies,
                    name: String::new(), // Will be set below
                    leader,
                    hierarchy,
                }
            })
            .collect();

        // Match new groups to existing groups
        let mut matched_old: HashSet<Uuid> = HashSet::new();
        let mut matched_new: HashSet<usize> = HashSet::new();

        for (new_idx, new_group) in new_groups.iter_mut().enumerate() {
            // Find best matching old group (by member overlap)
            let mut best_match: Option<(usize, f64)> = None;

            for (old_idx, old_group) in self.groups.iter().enumerate() {
                if matched_old.contains(&old_group.id) {
                    continue;
                }

                let overlap = jaccard_similarity(&new_group.members, &old_group.members);
                if overlap > 0.5 {
                    // More than 50% overlap = same group
                    if best_match.is_none() || overlap > best_match.unwrap().1 {
                        best_match = Some((old_idx, overlap));
                    }
                }
            }

            if let Some((old_idx, _)) = best_match {
                let old_group = &self.groups[old_idx];

                // Keep the old ID and formation epoch
                new_group.id = old_group.id;
                new_group.formed_epoch = old_group.formed_epoch;
                new_group.name = old_group.name.clone();

                // Check for membership changes
                let added: Vec<Uuid> = new_group.members.difference(&old_group.members).copied().collect();
                let removed: Vec<Uuid> = old_group.members.difference(&new_group.members).copied().collect();

                if !added.is_empty() || !removed.is_empty() {
                    changes.changed.push((new_group.clone(), added, removed));
                }

                // Check for leadership changes
                if new_group.leader != old_group.leader {
                    if let Some(new_leader) = new_group.leader {
                        changes.leadership_changed.push((
                            new_group.clone(),
                            old_group.leader,
                            new_leader,
                        ));
                    }
                }

                matched_old.insert(old_group.id);
                matched_new.insert(new_idx);
            }
        }

        // New groups (not matched to existing)
        for (idx, group) in new_groups.iter_mut().enumerate() {
            if !matched_new.contains(&idx) {
                // Assign a name
                self.next_group_num += 1;
                group.name = format!("Alliance {}", self.next_group_num);
                changes.formed.push(group.clone());
            }
        }

        // Dissolved groups (old groups not matched to new)
        for old_group in &self.groups {
            if !matched_old.contains(&old_group.id) {
                changes.dissolved.push(old_group.clone());
                self.dissolved.push((old_group.clone(), epoch));
            }
        }

        // Update active groups
        self.groups = new_groups;

        // Detect inter-group rivalries
        self.detect_rivalries(agents, epoch, &mut changes);

        changes
    }

    /// Detect rivalries between groups
    fn detect_rivalries(&mut self, agents: &[Agent], epoch: usize, changes: &mut GroupChanges) {
        let mut new_rivalries: Vec<Rivalry> = Vec::new();

        // Compare each pair of groups
        for i in 0..self.groups.len() {
            for j in (i + 1)..self.groups.len() {
                let group_a = &self.groups[i];
                let group_b = &self.groups[j];

                // Calculate cross-group metrics
                let (avg_trust, avg_sentiment) = calculate_cross_group_metrics(
                    &group_a.members,
                    &group_b.members,
                    agents,
                );

                // Check for shared enemies
                let shared_enemies = !group_a
                    .shared_enemies
                    .iter()
                    .filter(|e| group_b.shared_enemies.contains(e))
                    .next()
                    .is_none();

                // Classify relationship type
                let rivalry_type = classify_rivalry(avg_trust, shared_enemies);

                // Only track non-neutral relationships or if shared enemies exist
                if rivalry_type != RivalryType::Neutral || shared_enemies {
                    new_rivalries.push(Rivalry {
                        group_a: group_a.id,
                        group_b: group_b.id,
                        rivalry_type,
                        avg_cross_trust: avg_trust,
                        avg_cross_sentiment: avg_sentiment,
                        shared_enemies,
                        since_epoch: epoch,
                    });
                }
            }
        }

        // Find new, changed, and ended rivalries
        let old_rivalries = std::mem::take(&mut self.rivalries);

        for new_rivalry in &mut new_rivalries {
            // Find matching old rivalry
            let old_match = old_rivalries.iter().find(|old| {
                (old.group_a == new_rivalry.group_a && old.group_b == new_rivalry.group_b)
                    || (old.group_a == new_rivalry.group_b && old.group_b == new_rivalry.group_a)
            });

            if let Some(old) = old_match {
                // Keep the original epoch
                new_rivalry.since_epoch = old.since_epoch;

                // Check if type changed
                if old.rivalry_type != new_rivalry.rivalry_type {
                    changes.rivalries_changed.push((
                        new_rivalry.clone(),
                        old.rivalry_type,
                        new_rivalry.rivalry_type,
                    ));
                }
            } else {
                // New rivalry
                changes.rivalries_formed.push(new_rivalry.clone());
            }
        }

        // Find ended rivalries (old rivalries not in new)
        for old in &old_rivalries {
            let still_exists = new_rivalries.iter().any(|new| {
                (new.group_a == old.group_a && new.group_b == old.group_b)
                    || (new.group_a == old.group_b && new.group_b == old.group_a)
            });
            if !still_exists {
                changes.rivalries_ended.push(old.clone());
            }
        }

        self.rivalries = new_rivalries;
    }

    /// Get rivalries involving a specific group
    pub fn rivalries_of(&self, group_id: Uuid) -> Vec<&Rivalry> {
        self.rivalries
            .iter()
            .filter(|r| r.group_a == group_id || r.group_b == group_id)
            .collect()
    }

    /// Get all current rivalries
    pub fn current_rivalries(&self) -> &[Rivalry] {
        &self.rivalries
    }

    /// Get group containing a specific agent
    pub fn group_of(&self, agent_id: Uuid) -> Option<&Group> {
        self.groups.iter().find(|g| g.members.contains(&agent_id))
    }

    /// Get all current groups
    pub fn current_groups(&self) -> &[Group] {
        &self.groups
    }
}

/// Build a graph of mutual trust relationships
fn build_trust_graph(agents: &[Agent]) -> HashMap<Uuid, HashSet<Uuid>> {
    let mut graph: HashMap<Uuid, HashSet<Uuid>> = HashMap::new();

    // Initialize empty sets for all living agents
    for agent in agents.iter().filter(|a| a.is_alive()) {
        graph.insert(agent.id, HashSet::new());
    }

    // Add edges for mutual trust
    for agent in agents.iter().filter(|a| a.is_alive()) {
        for (other_id, belief) in &agent.beliefs.social {
            if belief.trust > TRUST_THRESHOLD {
                // Check if other agent also trusts this agent
                if let Some(other) = agents.iter().find(|a| a.id == *other_id && a.is_alive()) {
                    if let Some(reverse_belief) = other.beliefs.social.get(&agent.id) {
                        if reverse_belief.trust > TRUST_THRESHOLD {
                            // Mutual trust - add edge
                            graph.entry(agent.id).or_default().insert(*other_id);
                            graph.entry(*other_id).or_default().insert(agent.id);
                        }
                    }
                }
            }
        }
    }

    graph
}

/// Find all cliques of at least min_size using Bron-Kerbosch algorithm
fn find_cliques(graph: &HashMap<Uuid, HashSet<Uuid>>, min_size: usize) -> Vec<HashSet<Uuid>> {
    let mut cliques = Vec::new();
    let mut r: HashSet<Uuid> = HashSet::new();
    let p: HashSet<Uuid> = graph.keys().copied().collect();
    let x: HashSet<Uuid> = HashSet::new();

    bron_kerbosch(graph, &mut r, p, x, min_size, &mut cliques);

    // Remove cliques that are subsets of larger cliques
    let mut maximal_cliques: Vec<HashSet<Uuid>> = Vec::new();
    for clique in cliques {
        let is_subset = maximal_cliques.iter().any(|other| clique.is_subset(other));
        if !is_subset {
            // Remove any existing cliques that are subsets of this one
            maximal_cliques.retain(|other| !other.is_subset(&clique));
            maximal_cliques.push(clique);
        }
    }

    maximal_cliques
}

/// Bron-Kerbosch algorithm for finding cliques
fn bron_kerbosch(
    graph: &HashMap<Uuid, HashSet<Uuid>>,
    r: &mut HashSet<Uuid>,
    mut p: HashSet<Uuid>,
    mut x: HashSet<Uuid>,
    min_size: usize,
    cliques: &mut Vec<HashSet<Uuid>>,
) {
    if p.is_empty() && x.is_empty() {
        if r.len() >= min_size {
            cliques.push(r.clone());
        }
        return;
    }

    // Choose pivot to minimize branching
    let pivot = p.union(&x).next().copied();

    let Some(pivot) = pivot else { return };
    let pivot_neighbors = graph.get(&pivot).cloned().unwrap_or_default();

    let candidates: Vec<Uuid> = p.difference(&pivot_neighbors).copied().collect();

    for v in candidates {
        let neighbors = graph.get(&v).cloned().unwrap_or_default();

        r.insert(v);
        let new_p: HashSet<Uuid> = p.intersection(&neighbors).copied().collect();
        let new_x: HashSet<Uuid> = x.intersection(&neighbors).copied().collect();

        bron_kerbosch(graph, r, new_p, new_x, min_size, cliques);

        r.remove(&v);
        p.remove(&v);
        x.insert(v);
    }
}

/// Calculate average trust and sentiment within a group
fn calculate_group_metrics(members: &HashSet<Uuid>, agents: &[Agent]) -> (f64, f64) {
    let mut total_trust = 0.0;
    let mut total_sentiment = 0.0;
    let mut count = 0;

    for &member_id in members {
        if let Some(member) = agents.iter().find(|a| a.id == member_id) {
            for &other_id in members {
                if member_id != other_id {
                    if let Some(belief) = member.beliefs.social.get(&other_id) {
                        total_trust += belief.trust;
                        total_sentiment += belief.sentiment;
                        count += 1;
                    }
                }
            }
        }
    }

    if count > 0 {
        (total_trust / count as f64, total_sentiment / count as f64)
    } else {
        (0.0, 0.0)
    }
}

/// Find agents that all group members distrust
fn find_shared_enemies(members: &HashSet<Uuid>, agents: &[Agent]) -> Vec<Uuid> {
    let mut enemy_counts: HashMap<Uuid, usize> = HashMap::new();

    for &member_id in members {
        if let Some(member) = agents.iter().find(|a| a.id == member_id) {
            for (other_id, belief) in &member.beliefs.social {
                if !members.contains(other_id) && belief.trust < -TRUST_THRESHOLD {
                    *enemy_counts.entry(*other_id).or_default() += 1;
                }
            }
        }
    }

    // Shared enemies = distrusted by all members
    enemy_counts
        .into_iter()
        .filter(|(_, count)| *count == members.len())
        .map(|(id, _)| id)
        .collect()
}

/// Calculate leadership hierarchy within a group
/// Leadership score = sum of incoming trust from other group members
/// Higher score = more trusted by peers = more likely to be leader
fn calculate_hierarchy(members: &HashSet<Uuid>, agents: &[Agent]) -> Vec<(Uuid, f64)> {
    let mut scores: Vec<(Uuid, f64)> = members
        .iter()
        .map(|&member_id| {
            // Calculate incoming trust from other group members
            let incoming_trust: f64 = members
                .iter()
                .filter(|&&other_id| other_id != member_id)
                .filter_map(|&other_id| {
                    agents
                        .iter()
                        .find(|a| a.id == other_id)
                        .and_then(|other| other.beliefs.social.get(&member_id))
                        .map(|belief| belief.trust)
                })
                .sum();

            // Optionally factor in personality (extraversion)
            let extraversion_bonus = agents
                .iter()
                .find(|a| a.id == member_id)
                .map(|a| a.identity.personality.extraversion * 0.2)
                .unwrap_or(0.0);

            (member_id, incoming_trust + extraversion_bonus)
        })
        .collect();

    // Sort by score descending
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scores
}

/// Calculate average trust and sentiment between two groups
fn calculate_cross_group_metrics(
    group_a: &HashSet<Uuid>,
    group_b: &HashSet<Uuid>,
    agents: &[Agent],
) -> (f64, f64) {
    let mut total_trust = 0.0;
    let mut total_sentiment = 0.0;
    let mut count = 0;

    // Members of A toward members of B
    for &member_a in group_a {
        if let Some(agent_a) = agents.iter().find(|a| a.id == member_a) {
            for &member_b in group_b {
                if let Some(belief) = agent_a.beliefs.social.get(&member_b) {
                    total_trust += belief.trust;
                    total_sentiment += belief.sentiment;
                    count += 1;
                }
            }
        }
    }

    // Members of B toward members of A
    for &member_b in group_b {
        if let Some(agent_b) = agents.iter().find(|a| a.id == member_b) {
            for &member_a in group_a {
                if let Some(belief) = agent_b.beliefs.social.get(&member_a) {
                    total_trust += belief.trust;
                    total_sentiment += belief.sentiment;
                    count += 1;
                }
            }
        }
    }

    if count > 0 {
        (total_trust / count as f64, total_sentiment / count as f64)
    } else {
        (0.0, 0.0)
    }
}

/// Classify the type of inter-group relationship based on cross-trust
fn classify_rivalry(avg_trust: f64, shared_enemies: bool) -> RivalryType {
    if avg_trust < HOSTILE_THRESHOLD {
        RivalryType::Hostile
    } else if avg_trust < TENSE_THRESHOLD {
        RivalryType::Tense
    } else if avg_trust > ALLIED_THRESHOLD || (avg_trust > FRIENDLY_THRESHOLD && shared_enemies) {
        RivalryType::Allied
    } else if avg_trust > FRIENDLY_THRESHOLD {
        RivalryType::Friendly
    } else {
        RivalryType::Neutral
    }
}

/// Jaccard similarity between two sets
fn jaccard_similarity(a: &HashSet<Uuid>, b: &HashSet<Uuid>) -> f64 {
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

impl Group {
    /// Get member names
    pub fn member_names<'a>(&self, agents: &'a [Agent]) -> Vec<&'a str> {
        self.members
            .iter()
            .filter_map(|id| agents.iter().find(|a| a.id == *id))
            .map(|a| a.name())
            .collect()
    }

    /// Get enemy names
    pub fn enemy_names<'a>(&self, agents: &'a [Agent]) -> Vec<&'a str> {
        self.shared_enemies
            .iter()
            .filter_map(|id| agents.iter().find(|a| a.id == *id))
            .map(|a| a.name())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaccard_similarity() {
        let a: HashSet<Uuid> = [Uuid::new_v4(), Uuid::new_v4()].into_iter().collect();
        let b = a.clone();
        assert_eq!(jaccard_similarity(&a, &b), 1.0);

        let c: HashSet<Uuid> = HashSet::new();
        assert_eq!(jaccard_similarity(&a, &c), 0.0);
    }
}
