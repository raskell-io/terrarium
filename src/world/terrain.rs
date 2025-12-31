use serde::{Deserialize, Serialize};
use crate::config::WorldConfig;

/// The terrain grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Terrain {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<Vec<Cell>>,
}

/// A single cell in the world
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub terrain_type: TerrainType,
    /// Food resources available (0-100)
    pub food: u32,
    /// Material resources available (0-100)
    pub materials: u32,
    /// Maximum food this cell can hold
    pub food_capacity: u32,
    /// Maximum materials this cell can hold
    pub material_capacity: u32,
    /// Movement cost multiplier (1.0 = normal)
    pub movement_cost: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TerrainType {
    /// Fertile land, good for food
    Plains,
    /// Difficult terrain, rich in materials
    Mountains,
    /// Mixed resources
    Forest,
    /// Fast movement, poor resources
    Grassland,
    /// Impassable
    Water,
    /// Very poor resources
    Desert,
}

impl Terrain {
    pub fn generate(config: &WorldConfig) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let mut cells = Vec::with_capacity(config.height);

        for y in 0..config.height {
            let mut row = Vec::with_capacity(config.width);
            for x in 0..config.width {
                // Simple terrain generation based on config
                let roll: f64 = rng.gen();
                let terrain_type = if roll < config.terrain.fertile_fraction {
                    TerrainType::Plains
                } else if roll < config.terrain.fertile_fraction + config.terrain.resource_fraction {
                    TerrainType::Mountains
                } else if roll < 0.7 {
                    TerrainType::Forest
                } else if roll < 0.85 {
                    TerrainType::Grassland
                } else if roll < 0.95 {
                    TerrainType::Desert
                } else {
                    TerrainType::Water
                };

                let cell = Cell::new(terrain_type);
                row.push(cell);
            }
            cells.push(row);
        }

        Self {
            width: config.width,
            height: config.height,
            cells,
        }
    }

    /// Get a cell at coordinates
    pub fn get(&self, x: usize, y: usize) -> Option<&Cell> {
        self.cells.get(y).and_then(|row| row.get(x))
    }

    /// Get a mutable cell at coordinates
    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut Cell> {
        self.cells.get_mut(y).and_then(|row| row.get_mut(x))
    }

    /// Regenerate resources across the terrain
    pub fn regenerate_resources(&mut self, season: usize) {
        let multiplier = match season {
            0 => 0.15, // Spring
            1 => 0.20, // Summer
            2 => 0.10, // Autumn
            3 => 0.05, // Winter
            _ => 0.10,
        };

        for row in &mut self.cells {
            for cell in row {
                cell.regenerate(multiplier);
            }
        }
    }

    /// Get adjacent positions (including diagonals)
    pub fn adjacent_positions(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut positions = Vec::new();
        let x = x as i32;
        let y = y as i32;

        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x + dx;
                let ny = y + dy;
                if nx >= 0 && ny >= 0 && (nx as usize) < self.width && (ny as usize) < self.height {
                    positions.push((nx as usize, ny as usize));
                }
            }
        }

        positions
    }
}

impl Cell {
    pub fn new(terrain_type: TerrainType) -> Self {
        let (food_cap, material_cap, movement) = match terrain_type {
            TerrainType::Plains => (100, 20, 1.0),
            TerrainType::Mountains => (10, 100, 2.0),
            TerrainType::Forest => (60, 60, 1.5),
            TerrainType::Grassland => (40, 10, 0.8),
            TerrainType::Water => (0, 0, f64::INFINITY),
            TerrainType::Desert => (5, 30, 1.2),
        };

        Self {
            terrain_type,
            food: food_cap / 2,
            materials: material_cap / 2,
            food_capacity: food_cap,
            material_capacity: material_cap,
            movement_cost: movement,
        }
    }

    /// Regenerate resources up to capacity
    pub fn regenerate(&mut self, rate: f64) {
        let food_regen = (self.food_capacity as f64 * rate) as u32;
        let material_regen = (self.material_capacity as f64 * rate * 0.5) as u32;

        self.food = (self.food + food_regen).min(self.food_capacity);
        self.materials = (self.materials + material_regen).min(self.material_capacity);
    }

    /// Can this cell be traversed?
    pub fn is_passable(&self) -> bool {
        self.terrain_type != TerrainType::Water
    }

    /// Describe this cell
    pub fn describe(&self) -> String {
        let resource_desc = match (self.food, self.materials) {
            (f, m) if f > 70 && m > 70 => "abundant resources",
            (f, _) if f > 70 => "good food sources",
            (_, m) if m > 70 => "rich in materials",
            (f, m) if f < 20 && m < 20 => "barren",
            _ => "moderate resources",
        };

        format!("{:?} ({})", self.terrain_type, resource_desc)
    }
}
