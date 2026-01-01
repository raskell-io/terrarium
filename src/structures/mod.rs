//! Shelter and construction system.
//!
//! This module implements structures that agents can build in the world:
//! - **Lean-to**: Quick shelter with basic protection
//! - **Shelter**: Full shelter with better protection and rest bonus
//! - **Storage**: Shared inventory for materials and food
//! - **Workbench**: Crafting quality bonus
//! - **Farm**: Passive food production on fertile terrain

mod types;
mod recipes;

pub use types::{Structure, StructureInventory, StructureType};
pub use recipes::{StructureRecipe, StructureRecipeRegistry};
