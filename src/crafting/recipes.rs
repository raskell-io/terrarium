//! Crafting recipes.

use std::collections::HashMap;

use super::materials::MaterialType;
use super::tools::{Tool, ToolType};

/// A crafting recipe
#[derive(Debug, Clone)]
pub struct Recipe {
    pub output: ToolType,
    pub ingredients: Vec<(MaterialType, u32)>,
    pub required_tool: Option<ToolType>,
    pub min_crafting_skill: f64,
}

/// Registry of all recipes
pub struct RecipeRegistry {
    recipes: HashMap<ToolType, Recipe>,
}

impl RecipeRegistry {
    /// Create registry with all recipes
    pub fn new() -> Self {
        let mut recipes = HashMap::new();

        // Basic tools (no tool required)
        recipes.insert(
            ToolType::StoneAxe,
            Recipe {
                output: ToolType::StoneAxe,
                ingredients: vec![(MaterialType::Stone, 2), (MaterialType::Wood, 1)],
                required_tool: None,
                min_crafting_skill: 0.0,
            },
        );

        recipes.insert(
            ToolType::StoneKnife,
            Recipe {
                output: ToolType::StoneKnife,
                ingredients: vec![(MaterialType::Stone, 1), (MaterialType::Flint, 1)],
                required_tool: None,
                min_crafting_skill: 0.1,
            },
        );

        recipes.insert(
            ToolType::WoodenSpear,
            Recipe {
                output: ToolType::WoodenSpear,
                ingredients: vec![(MaterialType::Wood, 2), (MaterialType::Stone, 1)],
                required_tool: None,
                min_crafting_skill: 0.0,
            },
        );

        recipes.insert(
            ToolType::Rope,
            Recipe {
                output: ToolType::Rope,
                ingredients: vec![(MaterialType::Fiber, 3)],
                required_tool: None,
                min_crafting_skill: 0.0,
            },
        );

        recipes.insert(
            ToolType::Basket,
            Recipe {
                output: ToolType::Basket,
                ingredients: vec![(MaterialType::Fiber, 4), (MaterialType::Wood, 1)],
                required_tool: None,
                min_crafting_skill: 0.1,
            },
        );

        // Advanced tools (require tools)
        recipes.insert(
            ToolType::FlintAxe,
            Recipe {
                output: ToolType::FlintAxe,
                ingredients: vec![
                    (MaterialType::Flint, 2),
                    (MaterialType::Wood, 1),
                    (MaterialType::Fiber, 1),
                ],
                required_tool: Some(ToolType::StoneKnife),
                min_crafting_skill: 0.3,
            },
        );

        recipes.insert(
            ToolType::FlintKnife,
            Recipe {
                output: ToolType::FlintKnife,
                ingredients: vec![(MaterialType::Flint, 2), (MaterialType::Hide, 1)],
                required_tool: Some(ToolType::StoneKnife),
                min_crafting_skill: 0.3,
            },
        );

        recipes.insert(
            ToolType::Bow,
            Recipe {
                output: ToolType::Bow,
                ingredients: vec![(MaterialType::Wood, 2), (MaterialType::Fiber, 2)],
                required_tool: Some(ToolType::StoneKnife),
                min_crafting_skill: 0.4,
            },
        );

        recipes.insert(
            ToolType::FishingPole,
            Recipe {
                output: ToolType::FishingPole,
                ingredients: vec![
                    (MaterialType::Wood, 2),
                    (MaterialType::Fiber, 1),
                    (MaterialType::Bone, 1),
                ],
                required_tool: Some(ToolType::StoneKnife),
                min_crafting_skill: 0.2,
            },
        );

        Self { recipes }
    }

    /// Get a recipe by tool type
    pub fn get(&self, tool_type: &ToolType) -> Option<&Recipe> {
        self.recipes.get(tool_type)
    }

    /// Get all recipes
    pub fn all_recipes(&self) -> impl Iterator<Item = &Recipe> {
        self.recipes.values()
    }

    /// Get recipes an agent can craft with their current resources
    pub fn available_recipes(
        &self,
        materials: &HashMap<MaterialType, u32>,
        tools: &[Tool],
        crafting_skill: f64,
    ) -> Vec<&Recipe> {
        self.recipes
            .values()
            .filter(|recipe| {
                // Check skill requirement
                if crafting_skill < recipe.min_crafting_skill {
                    return false;
                }

                // Check material requirements
                for (mat_type, amount) in &recipe.ingredients {
                    let available = materials.get(mat_type).copied().unwrap_or(0);
                    if available < *amount {
                        return false;
                    }
                }

                // Check tool requirement
                if let Some(required) = recipe.required_tool {
                    if !tools.iter().any(|t| t.tool_type == required && !t.is_broken()) {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    /// Get tool types that can be crafted
    pub fn craftable_tools(
        &self,
        materials: &HashMap<MaterialType, u32>,
        tools: &[Tool],
        crafting_skill: f64,
    ) -> Vec<ToolType> {
        self.available_recipes(materials, tools, crafting_skill)
            .into_iter()
            .map(|r| r.output)
            .collect()
    }
}

impl Default for RecipeRegistry {
    fn default() -> Self {
        Self::new()
    }
}
