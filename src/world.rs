use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::structures::Structure;

/// A territorial claim on a cell
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerritoryClaim {
    /// The agent who owns this territory
    pub owner: Uuid,
    /// Agents allowed to use this territory
    pub allowed_guests: Vec<Uuid>,
    /// Epoch when the territory was claimed
    pub claimed_epoch: usize,
    /// Last epoch the owner was present (within 2 cells)
    pub last_presence_epoch: usize,
    /// Claim strength (1.0 = full, decays without presence)
    pub strength: f64,
}

/// Terrain type alias for external modules
pub type TerrainType = Terrain;

/// The world: a grid of cells with terrain and resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Cell>,
    pub epoch: usize,
}

/// A single cell in the grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub x: usize,
    pub y: usize,
    pub terrain: Terrain,
    pub food: u32,
    pub food_capacity: u32,
    /// Structure built in this cell (if any)
    pub structure: Option<Structure>,
    /// Territory claim on this cell (if any)
    pub territory: Option<TerritoryClaim>,
}

/// Terrain types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terrain {
    Fertile,
    Barren,
}

/// Configuration for world generation
#[derive(Debug, Clone, Deserialize)]
pub struct WorldConfig {
    pub width: usize,
    pub height: usize,
    pub fertile_fraction: f64,
    pub initial_food_per_fertile: u32,
    pub food_regen_rate: f64,
}

impl World {
    /// Create a new world from configuration
    pub fn new(config: &WorldConfig) -> Self {
        let mut rng = rand::rng();
        let mut cells = Vec::with_capacity(config.width * config.height);

        for y in 0..config.height {
            for x in 0..config.width {
                let terrain = if rng.random::<f64>() < config.fertile_fraction {
                    Terrain::Fertile
                } else {
                    Terrain::Barren
                };

                let (food, food_capacity) = match terrain {
                    Terrain::Fertile => (config.initial_food_per_fertile, 20),
                    Terrain::Barren => (0, 0),
                };

                cells.push(Cell {
                    x,
                    y,
                    terrain,
                    food,
                    food_capacity,
                    structure: None,
                    territory: None,
                });
            }
        }

        Self {
            width: config.width,
            height: config.height,
            cells,
            epoch: 0,
        }
    }

    /// Get cell at coordinates
    pub fn get(&self, x: usize, y: usize) -> Option<&Cell> {
        if x < self.width && y < self.height {
            Some(&self.cells[y * self.width + x])
        } else {
            None
        }
    }

    /// Get mutable cell at coordinates
    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut Cell> {
        if x < self.width && y < self.height {
            Some(&mut self.cells[y * self.width + x])
        } else {
            None
        }
    }

    /// Get cells adjacent to a position (8 directions)
    pub fn adjacent(&self, x: usize, y: usize) -> Vec<&Cell> {
        let mut result = Vec::new();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 {
                    if let Some(cell) = self.get(nx as usize, ny as usize) {
                        result.push(cell);
                    }
                }
            }
        }
        result
    }

    /// Regenerate resources across the world
    ///
    /// The `modifier` parameter adjusts the regeneration rate based on environmental
    /// conditions (seasons, weather, etc.). A modifier of 1.0 is normal, < 1.0 reduces
    /// regeneration, > 1.0 increases it.
    pub fn regenerate_resources(&mut self, regen_rate: f64, modifier: f64) {
        for cell in &mut self.cells {
            if cell.terrain == Terrain::Fertile && cell.food < cell.food_capacity {
                let effective_rate = regen_rate * modifier;
                let regen = (cell.food_capacity as f64 * effective_rate).ceil() as u32;
                cell.food = (cell.food + regen).min(cell.food_capacity);
            }
        }
    }

    /// Advance the world by one epoch
    ///
    /// The `food_regen_modifier` adjusts resource regeneration based on environmental
    /// conditions (1.0 = normal, < 1.0 = scarce, > 1.0 = abundant).
    pub fn tick(&mut self, regen_rate: f64, food_regen_modifier: f64) {
        self.epoch += 1;
        self.regenerate_resources(regen_rate, food_regen_modifier);
    }

    /// Describe a cell for agent perception
    pub fn describe_cell(&self, x: usize, y: usize) -> String {
        match self.get(x, y) {
            Some(cell) => {
                let terrain_desc = match cell.terrain {
                    Terrain::Fertile => "fertile ground",
                    Terrain::Barren => "barren land",
                };
                let food_desc = if cell.food > 10 {
                    "abundant food"
                } else if cell.food > 5 {
                    "some food"
                } else if cell.food > 0 {
                    "scarce food"
                } else {
                    "no food"
                };
                let structure_desc = cell.structure.as_ref().map(|s| {
                    format!(", {}", s.display_name())
                }).unwrap_or_default();
                let territory_desc = cell.territory.as_ref().map(|_| {
                    " (claimed territory)"
                }).unwrap_or_default();
                format!("{} with {}{}{}", terrain_desc, food_desc, structure_desc, territory_desc)
            }
            None => "unknown".to_string(),
        }
    }

    /// Describe a cell's territory for agent perception (with owner name lookup)
    pub fn describe_cell_territory(&self, x: usize, y: usize, agent_id: Uuid, get_name: impl Fn(Uuid) -> String) -> Option<String> {
        self.get(x, y).and_then(|cell| {
            cell.territory.as_ref().map(|t| {
                let owner_name = get_name(t.owner);
                if t.owner == agent_id {
                    format!("your territory (strength: {:.0}%)", t.strength * 100.0)
                } else if t.allowed_guests.contains(&agent_id) {
                    format!("{}'s territory (you are a guest)", owner_name)
                } else {
                    format!("{}'s territory", owner_name)
                }
            })
        })
    }

    /// Get a summary of visible area for an agent
    pub fn perception_summary(&self, x: usize, y: usize) -> String {
        let current = self.describe_cell(x, y);
        let adjacent: Vec<String> = self
            .adjacent(x, y)
            .iter()
            .map(|c| {
                let dir = direction_name(x, y, c.x, c.y);
                let desc = self.describe_cell(c.x, c.y);
                format!("{}: {}", dir, desc)
            })
            .collect();

        format!(
            "You are at ({}, {}): {}\nNearby: {}",
            x,
            y,
            current,
            adjacent.join("; ")
        )
    }
}

impl Cell {
    /// Take food from this cell (returns amount actually taken)
    pub fn take_food(&mut self, amount: u32) -> u32 {
        let taken = amount.min(self.food);
        self.food -= taken;
        taken
    }
}

fn direction_name(from_x: usize, from_y: usize, to_x: usize, to_y: usize) -> &'static str {
    let dx = to_x as i32 - from_x as i32;
    let dy = to_y as i32 - from_y as i32;
    match (dx, dy) {
        (0, -1) => "N",
        (0, 1) => "S",
        (1, 0) => "E",
        (-1, 0) => "W",
        (1, -1) => "NE",
        (-1, -1) => "NW",
        (1, 1) => "SE",
        (-1, 1) => "SW",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_creation() {
        let config = WorldConfig {
            width: 10,
            height: 10,
            fertile_fraction: 0.3,
            initial_food_per_fertile: 15,
            food_regen_rate: 0.1,
        };
        let world = World::new(&config);
        assert_eq!(world.cells.len(), 100);
        assert_eq!(world.width, 10);
        assert_eq!(world.height, 10);
    }

    #[test]
    fn test_cell_access() {
        let config = WorldConfig {
            width: 5,
            height: 5,
            fertile_fraction: 1.0,
            initial_food_per_fertile: 10,
            food_regen_rate: 0.1,
        };
        let world = World::new(&config);
        assert!(world.get(0, 0).is_some());
        assert!(world.get(4, 4).is_some());
        assert!(world.get(5, 5).is_none());
    }

    #[test]
    fn test_adjacent_cells() {
        let config = WorldConfig {
            width: 5,
            height: 5,
            fertile_fraction: 1.0,
            initial_food_per_fertile: 10,
            food_regen_rate: 0.1,
        };
        let world = World::new(&config);

        // Corner cell should have 3 neighbors
        let adj = world.adjacent(0, 0);
        assert_eq!(adj.len(), 3);

        // Center cell should have 8 neighbors
        let adj = world.adjacent(2, 2);
        assert_eq!(adj.len(), 8);
    }
}
