use rand::RngExt;
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
    /// Logistic regrowth rate of wood and fiber stands
    #[serde(default)]
    pub wood_regrowth: f64,
}

/// Round to an integer randomly, so the expected value equals `x`
pub fn random_round(x: f64, rng: &mut impl RngExt) -> u32 {
    let base = x.floor() as u32;
    if rng.random::<f64>() < x.fract() { base + 1 } else { base }
}

/// Food an agent can see from its position
#[derive(Debug, Clone, Default)]
pub struct LocalFood {
    /// Food on the agent's own cell
    pub here: u32,
    /// Adjacent cells with food: (x, y, amount)
    pub adjacent: Vec<(usize, usize, u32)>,
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
    /// Standing wood and fiber (trees and brush) on fertile land; regrows
    #[serde(default)]
    pub wood: u32,
    #[serde(default)]
    pub wood_capacity: u32,
    /// Stone (and flint) on barren land; finite, never comes back once quarried
    #[serde(default)]
    pub stone: u32,
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
    #[serde(default)]
    pub materials: MaterialsConfig,
}

/// Building materials in the landscape
#[derive(Debug, Clone, Deserialize)]
pub struct MaterialsConfig {
    /// Wood and fiber a fertile cell holds when fully grown
    #[serde(default = "default_wood_capacity")]
    pub wood_per_fertile: u32,
    /// Logistic regrowth rate r: growth = r * W * (1 - W / K), fastest at half capacity
    #[serde(default = "default_wood_regrowth")]
    pub wood_regrowth: f64,
    /// Stone a barren cell holds; it does not regrow
    #[serde(default = "default_stone")]
    pub stone_per_barren: u32,
}

impl Default for MaterialsConfig {
    fn default() -> Self {
        Self {
            wood_per_fertile: default_wood_capacity(),
            wood_regrowth: default_wood_regrowth(),
            stone_per_barren: default_stone(),
        }
    }
}

fn default_wood_capacity() -> u32 { 30 }
fn default_wood_regrowth() -> f64 { 0.1 }
fn default_stone() -> u32 { 40 }

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
                let (wood, stone) = match terrain {
                    Terrain::Fertile => (config.materials.wood_per_fertile, 0),
                    Terrain::Barren => (0, config.materials.stone_per_barren),
                };

                cells.push(Cell {
                    x,
                    y,
                    terrain,
                    food,
                    food_capacity,
                    structure: None,
                    territory: None,
                    wood,
                    wood_capacity: wood,
                    stone,
                });
            }
        }

        Self {
            width: config.width,
            height: config.height,
            cells,
            epoch: 0,
            wood_regrowth: config.materials.wood_regrowth,
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
        let mut rng = rand::rng();
        for cell in &mut self.cells {
            if cell.terrain == Terrain::Fertile && cell.food < cell.food_capacity {
                // Round randomly so the expected regrowth matches the rate exactly
                // (rounding up made any modifier > 0 yield at least 1 food per cell)
                let regen = random_round(cell.food_capacity as f64 * regen_rate * modifier, &mut rng);
                cell.food = (cell.food + regen).min(cell.food_capacity);
            }
        }
    }

    /// Wood and fiber stands regrow logistically (Verhulst): slowly when stripped bare, fastest
    /// at half capacity, levelling off when full. A bare stand resprouts as if one unit remained.
    /// Seasons scale growth like they do food
    pub fn regrow_wood(&mut self, modifier: f64) {
        let mut rng = rand::rng();
        let r = self.wood_regrowth * modifier;
        for cell in &mut self.cells {
            if cell.wood_capacity == 0 || cell.wood >= cell.wood_capacity {
                continue;
            }
            let k = cell.wood_capacity as f64;
            let w = (cell.wood as f64).max(1.0);
            let growth = random_round(r * w * (1.0 - w / k), &mut rng);
            cell.wood = (cell.wood + growth).min(cell.wood_capacity);
        }
    }

    /// Standing food dies back by the given fraction (e.g. in winter)
    pub fn decay_food(&mut self, fraction: f64) {
        if fraction <= 0.0 {
            return;
        }
        let mut rng = rand::rng();
        for cell in &mut self.cells {
            let lost = random_round(cell.food as f64 * fraction, &mut rng);
            cell.food = cell.food.saturating_sub(lost);
        }
    }

    /// Advance the world by one epoch
    ///
    /// The `food_regen_modifier` adjusts resource regeneration based on environmental
    /// conditions (1.0 = normal, < 1.0 = scarce, > 1.0 = abundant).
    pub fn tick(&mut self, regen_rate: f64, food_regen_modifier: f64, food_decay: f64) {
        self.epoch += 1;
        self.decay_food(food_decay);
        self.regenerate_resources(regen_rate, food_regen_modifier);
        self.regrow_wood(food_regen_modifier.min(1.0));
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
                let materials_desc = match cell.terrain {
                    Terrain::Fertile if cell.wood * 2 >= cell.wood_capacity => ", wooded",
                    Terrain::Fertile if cell.wood > 0 => ", sparse brush",
                    Terrain::Fertile => ", stripped of wood",
                    Terrain::Barren if cell.stone > 0 => ", stony",
                    Terrain::Barren => ", quarried out",
                };
                format!("{} with {}{}{}{}", terrain_desc, food_desc, materials_desc, structure_desc, territory_desc)
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

    /// Structured view of visible food (current cell + 8 adjacent), for non-LLM decision making
    pub fn local_food(&self, x: usize, y: usize) -> LocalFood {
        LocalFood {
            here: self.get(x, y).map(|c| c.food).unwrap_or(0),
            adjacent: self
                .adjacent(x, y)
                .iter()
                .filter(|c| c.food > 0)
                .map(|c| (c.x, c.y, c.food))
                .collect(),
        }
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
    fn test_food_decay_and_regen_rounding() {
        let config = WorldConfig {
            width: 10,
            height: 10,
            fertile_fraction: 1.0,
            initial_food_per_fertile: 10,
            food_regen_rate: 0.1,
            materials: MaterialsConfig::default(),
        };
        let mut world = World::new(&config);
        let total = |w: &World| w.cells.iter().map(|c| c.food).sum::<u32>();

        world.decay_food(0.0);
        assert_eq!(total(&world), 1000);
        world.decay_food(1.0);
        assert_eq!(total(&world), 0);

        // Capacity is 20, so expected regrowth is 0.6 per cell: random rounding keeps
        // the total near 60 (sd ~5), where rounding up would have given exactly 100
        world.regenerate_resources(0.1, 0.3);
        assert!((35..=85).contains(&total(&world)), "got {}", total(&world));
    }

    #[test]
    fn test_wood_regrows_logistically() {
        let config = WorldConfig {
            width: 10,
            height: 10,
            fertile_fraction: 1.0,
            initial_food_per_fertile: 10,
            food_regen_rate: 0.1,
            materials: MaterialsConfig::default(),
        };
        let mut world = World::new(&config);
        let total = |w: &World| w.cells.iter().map(|c| c.wood).sum::<u32>();
        assert_eq!(total(&world), 3000);
        world.regrow_wood(1.0);
        assert_eq!(total(&world), 3000, "full stands don't grow");

        // 100 stands at 15/30 grow ~0.75 each; at 1/30 (or bare) only ~0.1
        for c in &mut world.cells { c.wood = 15; }
        world.regrow_wood(1.0);
        let half = total(&world) - 1500;
        for c in &mut world.cells { c.wood = 0; }
        world.regrow_wood(1.0);
        let bare = total(&world);
        assert!((50..=100).contains(&half), "half-grown: {}", half);
        assert!(bare < 25, "bare: {}", bare);
        assert!(bare > 0, "bare stands resprout");
        for c in &mut world.cells { c.wood = 0; }
        world.regrow_wood(0.0);
        assert_eq!(total(&world), 0, "nothing grows when the season allows none");
    }

    #[test]
    fn test_world_creation() {
        let config = WorldConfig {
            width: 10,
            height: 10,
            fertile_fraction: 0.3,
            initial_food_per_fertile: 15,
            food_regen_rate: 0.1,
            materials: MaterialsConfig::default(),
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
            materials: MaterialsConfig::default(),
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
            materials: MaterialsConfig::default(),
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
