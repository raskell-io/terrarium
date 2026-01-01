//! Material types for crafting.

use serde::{Deserialize, Serialize};

use crate::world::Terrain;

/// Types of materials that can be gathered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialType {
    Wood,
    Stone,
    Fiber,
    Flint,
    Hide,
    Bone,
}

impl MaterialType {
    /// How common is this material? (affects gather rates)
    pub fn rarity(&self) -> f64 {
        match self {
            MaterialType::Wood => 0.8,
            MaterialType::Stone => 0.6,
            MaterialType::Fiber => 0.7,
            MaterialType::Flint => 0.15,
            MaterialType::Hide => 0.3,
            MaterialType::Bone => 0.25,
        }
    }

    /// What terrain type yields this material?
    pub fn source_terrain(&self) -> Option<Terrain> {
        match self {
            MaterialType::Wood => Some(Terrain::Fertile),
            MaterialType::Stone => Some(Terrain::Barren),
            MaterialType::Fiber => Some(Terrain::Fertile),
            MaterialType::Flint => Some(Terrain::Barren),
            MaterialType::Hide | MaterialType::Bone => None, // From hunting
        }
    }

    /// Display name for the material
    pub fn display_name(&self) -> &'static str {
        match self {
            MaterialType::Wood => "wood",
            MaterialType::Stone => "stone",
            MaterialType::Fiber => "fiber",
            MaterialType::Flint => "flint",
            MaterialType::Hide => "hide",
            MaterialType::Bone => "bone",
        }
    }

    /// Parse material from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "wood" => Some(MaterialType::Wood),
            "stone" => Some(MaterialType::Stone),
            "fiber" => Some(MaterialType::Fiber),
            "flint" => Some(MaterialType::Flint),
            "hide" => Some(MaterialType::Hide),
            "bone" => Some(MaterialType::Bone),
            _ => None,
        }
    }

    /// All material types that can be gathered from terrain
    pub fn gatherable() -> &'static [MaterialType] {
        &[
            MaterialType::Wood,
            MaterialType::Stone,
            MaterialType::Fiber,
            MaterialType::Flint,
        ]
    }
}
