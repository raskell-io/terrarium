//! Structure types and instances.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crafting::{MaterialType, ToolQuality};

/// Types of structures that can be built
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StructureType {
    /// Basic shelter - quick to build, moderate protection
    LeanTo,
    /// Full shelter - better protection, rest bonus, safety boost
    Shelter,
    /// Storage - holds materials and food
    Storage,
    /// Workbench - crafting quality bonus
    Workbench,
    /// Farm - passive food production (fertile terrain only)
    Farm,
}

impl StructureType {
    /// Display name for the structure
    pub fn display_name(&self) -> &'static str {
        match self {
            StructureType::LeanTo => "lean-to",
            StructureType::Shelter => "shelter",
            StructureType::Storage => "storage",
            StructureType::Workbench => "workbench",
            StructureType::Farm => "farm",
        }
    }

    /// Base durability for this structure type
    pub fn base_durability(&self) -> u32 {
        match self {
            StructureType::LeanTo => 50,
            StructureType::Shelter => 100,
            StructureType::Storage => 80,
            StructureType::Workbench => 60,
            StructureType::Farm => 40,
        }
    }

    /// Hazard protection level (0.0 to 1.0)
    pub fn hazard_protection(&self) -> f64 {
        match self {
            StructureType::LeanTo => 0.5,
            StructureType::Shelter => 0.8,
            StructureType::Storage => 0.0,
            StructureType::Workbench => 0.0,
            StructureType::Farm => 0.0,
        }
    }

    /// Rest bonus when inside (added to base rest recovery)
    pub fn rest_bonus(&self) -> f64 {
        match self {
            StructureType::LeanTo => 0.1,
            StructureType::Shelter => 0.2,
            StructureType::Storage => 0.0,
            StructureType::Workbench => 0.0,
            StructureType::Farm => 0.0,
        }
    }

    /// Safety belief boost when sheltered
    pub fn safety_boost(&self) -> f64 {
        match self {
            StructureType::LeanTo => 0.05,
            StructureType::Shelter => 0.15,
            StructureType::Storage => 0.0,
            StructureType::Workbench => 0.0,
            StructureType::Farm => 0.0,
        }
    }

    /// Crafting quality bonus (percentage)
    pub fn crafting_bonus(&self) -> f64 {
        match self {
            StructureType::Workbench => 0.2,
            _ => 0.0,
        }
    }

    /// Food production per epoch (for farms)
    pub fn food_production(&self) -> u32 {
        match self {
            StructureType::Farm => 2,
            _ => 0,
        }
    }

    /// Whether this structure can be entered/sheltered in
    pub fn is_shelter(&self) -> bool {
        matches!(self, StructureType::LeanTo | StructureType::Shelter)
    }

    /// Whether this structure has storage capacity
    pub fn has_storage(&self) -> bool {
        matches!(self, StructureType::Storage)
    }

    /// Parse structure type from string
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.to_lowercase().replace('-', "_").replace(' ', "_");
        match s.as_str() {
            "lean_to" | "leanto" | "lean" => Some(StructureType::LeanTo),
            "shelter" => Some(StructureType::Shelter),
            "storage" => Some(StructureType::Storage),
            "workbench" | "bench" => Some(StructureType::Workbench),
            "farm" => Some(StructureType::Farm),
            _ => None,
        }
    }

    /// All structure types
    pub fn all() -> &'static [StructureType] {
        &[
            StructureType::LeanTo,
            StructureType::Shelter,
            StructureType::Storage,
            StructureType::Workbench,
            StructureType::Farm,
        ]
    }
}

/// Inventory for storage structures
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructureInventory {
    pub materials: HashMap<MaterialType, u32>,
    pub food: u32,
    pub max_materials: u32,
    pub max_food: u32,
}

impl StructureInventory {
    pub fn new(max_materials: u32, max_food: u32) -> Self {
        Self {
            materials: HashMap::new(),
            food: 0,
            max_materials,
            max_food,
        }
    }

    /// Total materials stored
    pub fn total_materials(&self) -> u32 {
        self.materials.values().sum()
    }

    /// Add material to storage (returns overflow)
    pub fn add_material(&mut self, material: MaterialType, amount: u32) -> u32 {
        let available_space = self.max_materials.saturating_sub(self.total_materials());
        let to_add = amount.min(available_space);
        *self.materials.entry(material).or_insert(0) += to_add;
        amount - to_add
    }

    /// Remove material from storage (returns actual removed)
    pub fn remove_material(&mut self, material: MaterialType, amount: u32) -> u32 {
        let current = self.materials.entry(material).or_insert(0);
        let to_remove = amount.min(*current);
        *current -= to_remove;
        if *current == 0 {
            self.materials.remove(&material);
        }
        to_remove
    }

    /// Add food to storage (returns overflow)
    pub fn add_food(&mut self, amount: u32) -> u32 {
        let available_space = self.max_food.saturating_sub(self.food);
        let to_add = amount.min(available_space);
        self.food += to_add;
        amount - to_add
    }

    /// Remove food from storage (returns actual removed)
    pub fn remove_food(&mut self, amount: u32) -> u32 {
        let to_remove = amount.min(self.food);
        self.food -= to_remove;
        to_remove
    }
}

/// A built structure in the world
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structure {
    pub id: Uuid,
    pub structure_type: StructureType,
    /// Who built/owns this structure
    pub owner: Uuid,
    /// Agents allowed to use this structure
    pub allowed_guests: Vec<Uuid>,
    /// Current build progress (0 = not started)
    pub build_progress: u32,
    /// Required progress to complete
    pub build_required: u32,
    /// Current durability
    pub durability: u32,
    /// Maximum durability
    pub max_durability: u32,
    /// Quality affects effectiveness
    pub quality: ToolQuality,
    /// Epoch when building started
    pub started_epoch: usize,
    /// Epoch when completed (None if still building)
    pub completed_epoch: Option<usize>,
    /// Storage inventory (for Storage type)
    pub inventory: Option<StructureInventory>,
}

impl Structure {
    /// Create a new structure (in-progress)
    pub fn new(
        structure_type: StructureType,
        owner: Uuid,
        build_required: u32,
        quality: ToolQuality,
        epoch: usize,
    ) -> Self {
        let base_dur = structure_type.base_durability();
        let max_durability = (base_dur as f64 * quality.durability_modifier()) as u32;

        let inventory = if structure_type.has_storage() {
            Some(StructureInventory::new(50, 20))
        } else {
            None
        };

        Self {
            id: Uuid::new_v4(),
            structure_type,
            owner,
            allowed_guests: Vec::new(),
            build_progress: 0,
            build_required,
            durability: max_durability,
            max_durability,
            quality,
            started_epoch: epoch,
            completed_epoch: None,
            inventory,
        }
    }

    /// Check if construction is complete
    pub fn is_complete(&self) -> bool {
        self.build_progress >= self.build_required
    }

    /// Add build progress
    pub fn add_progress(&mut self, amount: u32, epoch: usize) {
        self.build_progress = (self.build_progress + amount).min(self.build_required);
        if self.is_complete() && self.completed_epoch.is_none() {
            self.completed_epoch = Some(epoch);
        }
    }

    /// Check if an agent can use this structure
    pub fn can_use(&self, agent_id: Uuid) -> bool {
        if !self.is_complete() {
            return false;
        }
        agent_id == self.owner || self.allowed_guests.contains(&agent_id)
    }

    /// Grant access to an agent
    pub fn permit(&mut self, agent_id: Uuid) {
        if agent_id != self.owner && !self.allowed_guests.contains(&agent_id) {
            self.allowed_guests.push(agent_id);
        }
    }

    /// Revoke access from an agent
    pub fn deny(&mut self, agent_id: Uuid) {
        self.allowed_guests.retain(|&id| id != agent_id);
    }

    /// Get durability ratio (0.0 to 1.0)
    pub fn durability_ratio(&self) -> f64 {
        if self.max_durability == 0 {
            return 0.0;
        }
        self.durability as f64 / self.max_durability as f64
    }

    /// Get effective hazard protection (quality and durability adjusted)
    pub fn effective_protection(&self) -> f64 {
        if !self.is_complete() {
            return 0.0;
        }
        let base = self.structure_type.hazard_protection() * self.quality.effectiveness_modifier();
        // Durability affects effectiveness: at 50% durability, 75% effectiveness
        base * (0.5 + 0.5 * self.durability_ratio())
    }

    /// Get effective rest bonus (quality and durability adjusted)
    pub fn effective_rest_bonus(&self) -> f64 {
        if !self.is_complete() {
            return 0.0;
        }
        let base = self.structure_type.rest_bonus() * self.quality.effectiveness_modifier();
        base * (0.5 + 0.5 * self.durability_ratio())
    }

    /// Get effective crafting bonus (quality and durability adjusted)
    pub fn effective_crafting_bonus(&self) -> f64 {
        if !self.is_complete() {
            return 0.0;
        }
        let base = self.structure_type.crafting_bonus() * self.quality.effectiveness_modifier();
        base * (0.5 + 0.5 * self.durability_ratio())
    }

    /// Get effective food production (quality and durability adjusted, for farms)
    pub fn effective_food_production(&self) -> u32 {
        if !self.is_complete() {
            return 0;
        }
        let base = self.structure_type.food_production() as f64;
        let adjusted = base * self.quality.effectiveness_modifier() * (0.5 + 0.5 * self.durability_ratio());
        adjusted.round() as u32
    }

    /// Decay durability (called each epoch)
    pub fn decay(&mut self, amount: u32) {
        self.durability = self.durability.saturating_sub(amount);
    }

    /// Check if structure is destroyed
    pub fn is_destroyed(&self) -> bool {
        self.durability == 0
    }

    /// Get durability percentage for display
    pub fn durability_percent(&self) -> f64 {
        self.durability_ratio() * 100.0
    }

    /// Display name with quality
    pub fn display_name(&self) -> String {
        if self.is_complete() {
            format!("{} {}", self.quality.display_name(), self.structure_type.display_name())
        } else {
            format!("{} (building {:.0}%)",
                self.structure_type.display_name(),
                (self.build_progress as f64 / self.build_required as f64) * 100.0
            )
        }
    }
}
