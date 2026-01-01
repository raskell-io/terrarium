//! Structure building recipes.

use std::collections::HashMap;

use crate::crafting::{MaterialType, ToolType};
use crate::world::TerrainType;

use super::StructureType;

/// A recipe for building a structure
#[derive(Debug, Clone)]
pub struct StructureRecipe {
    /// What structure this creates
    pub output: StructureType,
    /// Required materials
    pub materials: HashMap<MaterialType, u32>,
    /// Required tool (if any)
    pub required_tool: Option<ToolType>,
    /// Build progress required to complete
    pub build_required: u32,
    /// Terrain restrictions (empty = any terrain)
    pub allowed_terrain: Vec<TerrainType>,
}

impl StructureRecipe {
    /// Create a new structure recipe
    pub fn new(output: StructureType, build_required: u32) -> Self {
        Self {
            output,
            materials: HashMap::new(),
            required_tool: None,
            build_required,
            allowed_terrain: Vec::new(),
        }
    }

    /// Add a material requirement
    pub fn with_material(mut self, material: MaterialType, amount: u32) -> Self {
        self.materials.insert(material, amount);
        self
    }

    /// Add a tool requirement
    pub fn with_tool(mut self, tool: ToolType) -> Self {
        self.required_tool = Some(tool);
        self
    }

    /// Restrict to specific terrain types
    pub fn on_terrain(mut self, terrain: TerrainType) -> Self {
        self.allowed_terrain.push(terrain);
        self
    }

    /// Check if agent has required materials
    pub fn can_afford(&self, inventory: &HashMap<MaterialType, u32>) -> bool {
        self.materials.iter().all(|(mat, required)| {
            inventory.get(mat).copied().unwrap_or(0) >= *required
        })
    }

    /// Check if terrain is valid for this structure
    pub fn valid_terrain(&self, terrain: TerrainType) -> bool {
        self.allowed_terrain.is_empty() || self.allowed_terrain.contains(&terrain)
    }

    /// Get total material cost for display
    pub fn material_cost_string(&self) -> String {
        let mut parts: Vec<String> = self.materials
            .iter()
            .map(|(mat, amt)| format!("{} {}", amt, mat.display_name()))
            .collect();
        parts.sort();
        parts.join(", ")
    }
}

/// Registry of all structure recipes
pub struct StructureRecipeRegistry {
    recipes: HashMap<StructureType, StructureRecipe>,
}

impl StructureRecipeRegistry {
    /// Create registry with default recipes
    pub fn new() -> Self {
        let mut recipes = HashMap::new();

        // Lean-to: 3 wood + 2 fiber, 1 epoch worth of progress
        recipes.insert(
            StructureType::LeanTo,
            StructureRecipe::new(StructureType::LeanTo, 10)
                .with_material(MaterialType::Wood, 3)
                .with_material(MaterialType::Fiber, 2),
        );

        // Shelter: 6 wood + 4 fiber + 2 stone, 2 epochs
        recipes.insert(
            StructureType::Shelter,
            StructureRecipe::new(StructureType::Shelter, 20)
                .with_material(MaterialType::Wood, 6)
                .with_material(MaterialType::Fiber, 4)
                .with_material(MaterialType::Stone, 2),
        );

        // Storage: 4 wood + 4 stone + 2 fiber, 2 epochs
        recipes.insert(
            StructureType::Storage,
            StructureRecipe::new(StructureType::Storage, 20)
                .with_material(MaterialType::Wood, 4)
                .with_material(MaterialType::Stone, 4)
                .with_material(MaterialType::Fiber, 2),
        );

        // Workbench: 5 wood + 3 stone + 1 flint, needs knife, 2 epochs
        recipes.insert(
            StructureType::Workbench,
            StructureRecipe::new(StructureType::Workbench, 20)
                .with_material(MaterialType::Wood, 5)
                .with_material(MaterialType::Stone, 3)
                .with_material(MaterialType::Flint, 1)
                .with_tool(ToolType::StoneKnife),
        );

        // Farm: 8 wood + 4 fiber + 2 stone, fertile terrain only, 3 epochs
        recipes.insert(
            StructureType::Farm,
            StructureRecipe::new(StructureType::Farm, 30)
                .with_material(MaterialType::Wood, 8)
                .with_material(MaterialType::Fiber, 4)
                .with_material(MaterialType::Stone, 2)
                .on_terrain(TerrainType::Fertile),
        );

        Self { recipes }
    }

    /// Get recipe for a structure type
    pub fn get(&self, structure_type: StructureType) -> Option<&StructureRecipe> {
        self.recipes.get(&structure_type)
    }

    /// Get all recipes
    pub fn all(&self) -> impl Iterator<Item = &StructureRecipe> {
        self.recipes.values()
    }

    /// Get buildable structures given inventory and terrain
    pub fn buildable(
        &self,
        inventory: &HashMap<MaterialType, u32>,
        terrain: TerrainType,
        has_tool: impl Fn(ToolType) -> bool,
    ) -> Vec<StructureType> {
        self.recipes
            .values()
            .filter(|recipe| {
                recipe.can_afford(inventory)
                    && recipe.valid_terrain(terrain)
                    && recipe.required_tool.map(|t| has_tool(t)).unwrap_or(true)
            })
            .map(|recipe| recipe.output)
            .collect()
    }
}

impl Default for StructureRecipeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
