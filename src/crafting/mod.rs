//! Crafting system for tools and materials.

pub mod materials;
pub mod recipes;
pub mod tools;

pub use materials::MaterialType;
pub use recipes::{Recipe, RecipeRegistry};
pub use tools::{Tool, ToolQuality, ToolType};
