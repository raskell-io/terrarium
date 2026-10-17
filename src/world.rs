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
    /// Wild game: logistic growth rate and the share of each herd that wanders off daily
    #[serde(default)]
    pub game_growth: f64,
    #[serde(default)]
    pub game_dispersal: f64,
    #[serde(default)]
    pub fish_growth: f64,
}

/// Days of undisturbed breeding and wandering simulated before a world begins
const WILDLIFE_SETTLING_DAYS: usize = 500;

/// Round to an integer randomly, so the expected value equals `x`
pub fn random_round(x: f64, rng: &mut impl RngExt) -> u32 {
    let base = x.floor() as u32;
    if rng.random::<f64>() < x.fract() { base + 1 } else { base }
}

/// Which land cells are fertile, and how high each cell lies. A random fertility field is
/// smoothed `clustering` times (each cell averaged with its neighbours), land beside water gets
/// a boost, and the most fertile `fertile_fraction` of land becomes fertile, so the fraction is
/// exact and patchy. The same field gives the relief: fertile valleys lie low, barren uplands
/// high (elevation is the cell's rank, 0 lowest land to 1 highest; water is 0)
fn fertile_land(config: &WorldConfig, water: &[bool], rng: &mut impl RngExt) -> (Vec<bool>, Vec<f64>) {
    let (w, h) = (config.width, config.height);
    let neighbours = |i: usize| {
        let (x, y) = ((i % w) as i64, (i / w) as i64);
        (-1..=1)
            .flat_map(move |dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
            .filter(move |&(nx, ny)| (nx, ny) != (x, y) && nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64)
            .map(move |(nx, ny)| ny as usize * w + nx as usize)
    };
    // Smooth on a field that wraps around the edges, so every cell averages the same number of
    // values; otherwise edge cells vary more and crowd the extremes (land would hug the border)
    let wrapped = |i: usize| {
        let (x, y) = ((i % w) as i64, (i / w) as i64);
        (-1..=1).flat_map(move |dy| {
            (-1..=1).map(move |dx| ((y + dy).rem_euclid(h as i64) as usize) * w + (x + dx).rem_euclid(w as i64) as usize)
        })
    };
    let mut field: Vec<f64> = (0..w * h).map(|_| rng.random::<f64>()).collect();
    for _ in 0..config.clustering {
        field = (0..w * h).map(|i| wrapped(i).map(|j| field[j]).sum::<f64>() / 9.0).collect();
    }
    // Shores are greener
    for i in 0..w * h {
        if !water[i] && neighbours(i).any(|j| water[j]) {
            field[i] += 0.2;
        }
    }
    let mut land: Vec<usize> = (0..w * h).filter(|&i| !water[i]).collect();
    land.sort_by(|&a, &b| field[b].total_cmp(&field[a]));
    let count = (land.len() as f64 * config.fertile_fraction).round() as usize;
    let mut fertile = vec![false; w * h];
    for &i in land.iter().take(count) {
        fertile[i] = true;
    }
    let mut elevation = vec![0.0; w * h];
    let span = land.len().saturating_sub(1).max(1) as f64;
    for (rank, &i) in land.iter().enumerate() {
        elevation[i] = rank as f64 / span;
    }
    (fertile, elevation)
}

/// Flood a lake of `size` orthogonally connected cells outward from a random dry cell
fn dig_lake(water: &mut [bool], width: usize, height: usize, size: usize, rng: &mut impl RngExt) {
    let dry: Vec<usize> = (0..water.len()).filter(|&i| !water[i]).collect();
    if dry.is_empty() {
        return;
    }
    let mut lake = vec![dry[rng.random_range(0..dry.len())]];
    while lake.len() < size {
        let shore: Vec<usize> = lake
            .iter()
            .flat_map(|&i| {
                let (x, y) = ((i % width) as i32, (i / width) as i32);
                [(0, -1), (0, 1), (-1, 0), (1, 0)].map(|(dx, dy)| (x + dx, y + dy))
            })
            .filter(|&(x, y)| x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height)
            .map(|(x, y)| y as usize * width + x as usize)
            .filter(|i| !lake.contains(i) && !water[*i])
            .collect();
        if shore.is_empty() {
            break;
        }
        lake.push(shore[rng.random_range(0..shore.len())]);
    }
    for i in lake {
        water[i] = true;
    }
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
    /// Wild game living here (animals), and how many the land here can support
    #[serde(default)]
    pub game: f64,
    #[serde(default)]
    pub game_capacity: f64,
    /// Fish in a lake cell (food units), and how many it supports
    #[serde(default)]
    pub fish: f64,
    #[serde(default)]
    pub fish_capacity: f64,
    /// Height of the land, 0 (lowest) to 1 (highest)
    #[serde(default)]
    pub elevation: f64,
}

/// Extra energy per unit of elevation climbed (a climb from the lowest to the highest land in
/// one step costs three times a level step)
const CLIMB_COST: f64 = 2.0;
/// Energy multiplier for stepping into thick forest
const FOREST_COST: f64 = 1.5;

/// Land this high counts as high ground: one can see further from it
pub const HIGH_GROUND: f64 = 0.8;

impl Cell {
    /// Thick forest: a stand at least two-thirds grown
    pub fn is_forest(&self) -> bool {
        self.wood_capacity > 0 && self.wood * 3 >= self.wood_capacity * 2
    }
}

/// Terrain types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Terrain {
    Fertile,
    Barren,
    /// Lakes: no one walks into them; fish live here
    Water,
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
    #[serde(default)]
    pub wildlife: WildlifeConfig,
    #[serde(default)]
    pub water: WaterConfig,
    /// Smoothing passes over the fertility field: 0 scatters fertile cells at random,
    /// more passes gather them into larger patches (valleys) with barren uplands between
    #[serde(default = "default_clustering")]
    pub clustering: usize,
}

fn default_clustering() -> usize { 2 }

/// Lakes and their fish
#[derive(Debug, Clone, Deserialize)]
pub struct WaterConfig {
    /// Number of lakes
    #[serde(default = "default_lakes")]
    pub lakes: usize,
    /// Cells per lake
    #[serde(default = "default_lake_size")]
    pub lake_size: usize,
    /// Fish (in food units) a water cell supports
    #[serde(default = "default_fish_per_water")]
    pub fish_per_water: f64,
    /// Logistic growth rate of fish per day, scaled by season
    #[serde(default = "default_fish_growth")]
    pub fish_growth: f64,
}

impl Default for WaterConfig {
    fn default() -> Self {
        Self {
            lakes: default_lakes(),
            lake_size: default_lake_size(),
            fish_per_water: default_fish_per_water(),
            fish_growth: default_fish_growth(),
        }
    }
}

fn default_lakes() -> usize { 1 }
fn default_lake_size() -> usize { 6 }
fn default_fish_per_water() -> f64 { 15.0 }
fn default_fish_growth() -> f64 { 0.05 }

/// Wild game: herds that breed, wander and can be hunted out
#[derive(Debug, Clone, Deserialize)]
pub struct WildlifeConfig {
    /// Animals a fertile cell supports
    #[serde(default = "default_game_fertile")]
    pub game_per_fertile: f64,
    /// Animals a barren cell supports
    #[serde(default = "default_game_barren")]
    pub game_per_barren: f64,
    /// Logistic growth rate per day in the breeding seasons
    #[serde(default = "default_game_growth")]
    pub growth: f64,
    /// Share of each herd that moves to neighbouring cells each day
    #[serde(default = "default_game_dispersal")]
    pub dispersal: f64,
}

impl Default for WildlifeConfig {
    fn default() -> Self {
        Self {
            game_per_fertile: default_game_fertile(),
            game_per_barren: default_game_barren(),
            growth: default_game_growth(),
            dispersal: default_game_dispersal(),
        }
    }
}

fn default_game_fertile() -> f64 { 2.0 }
fn default_game_barren() -> f64 { 0.3 }
fn default_game_growth() -> f64 { 0.03 }
fn default_game_dispersal() -> f64 { 0.1 }

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
// A meadow takes about a year (100 days) to grow into thick forest
fn default_wood_regrowth() -> f64 { 0.02 }
fn default_stone() -> u32 { 40 }

impl World {
    /// Create a new world from configuration
    pub fn new(config: &WorldConfig) -> Self {
        let mut rng = rand::rng();
        let mut cells = Vec::with_capacity(config.width * config.height);

        // Lakes first, so the fertile fraction applies to the land that remains
        let mut water = vec![false; config.width * config.height];
        for _ in 0..config.water.lakes {
            dig_lake(&mut water, config.width, config.height, config.water.lake_size, &mut rng);
        }

        let (fertile, elevation) = fertile_land(config, &water, &mut rng);

        for y in 0..config.height {
            for x in 0..config.width {
                let terrain = if water[y * config.width + x] {
                    Terrain::Water
                } else if fertile[y * config.width + x] {
                    Terrain::Fertile
                } else {
                    Terrain::Barren
                };

                let (food, food_capacity) = match terrain {
                    Terrain::Fertile => (config.initial_food_per_fertile, 20),
                    Terrain::Barren | Terrain::Water => (0, 0),
                };
                let (wood, stone) = match terrain {
                    Terrain::Fertile => (config.materials.wood_per_fertile, 0),
                    Terrain::Barren => (0, config.materials.stone_per_barren),
                    Terrain::Water => (0, 0),
                };
                let game_capacity = match terrain {
                    Terrain::Fertile => config.wildlife.game_per_fertile,
                    Terrain::Barren => config.wildlife.game_per_barren,
                    Terrain::Water => 0.0,
                };
                let fish = if terrain == Terrain::Water { config.water.fish_per_water } else { 0.0 };

                cells.push(Cell {
                    x,
                    y,
                    terrain,
                    food,
                    food_capacity,
                    structure: None,
                    territory: None,
                    // A mix of meadow and woodland: stands start 20-100% grown
                    wood: (wood as f64 * rng.random_range(0.2..=1.0)).round() as u32,
                    wood_capacity: wood,
                    stone,
                    game: game_capacity,
                    game_capacity,
                    fish,
                    fish_capacity: fish,
                    elevation: elevation[y * config.width + x],
                });
            }
        }

        let mut world = Self {
            width: config.width,
            height: config.height,
            cells,
            epoch: 0,
            wood_regrowth: config.materials.wood_regrowth,
            game_growth: config.wildlife.growth,
            game_dispersal: config.wildlife.dispersal,
            fish_growth: config.water.fish_growth,
        };

        // Herds wander from rich land into poor land that can't sustain them (sources and sinks),
        // so the steady state sits below capacity. Start the world there rather than at capacity
        if world.game_growth > 0.0 {
            for _ in 0..WILDLIFE_SETTLING_DAYS {
                world.update_wildlife(1.0);
            }
        }
        world
    }

    /// Whether agents can stand here (not water, inside the world)
    pub fn walkable(&self, x: usize, y: usize) -> bool {
        self.get(x, y).is_some_and(|c| c.terrain != Terrain::Water)
    }

    /// The walkable cell closest to (x, y)
    pub fn nearest_land(&self, x: usize, y: usize) -> (usize, usize) {
        self.cells
            .iter()
            .filter(|c| c.terrain != Terrain::Water)
            .min_by_key(|c| c.x.abs_diff(x).max(c.y.abs_diff(y)))
            .map_or((x, y), |c| (c.x, c.y))
    }

    /// Fish within reach of an angler at (x, y): the water cells next to them
    pub fn fish_around(&self, x: usize, y: usize) -> f64 {
        self.adjacent(x, y).iter().filter(|c| c.terrain == Terrain::Water).map(|c| c.fish).sum()
    }

    /// Take a catch of `amount` from the best-stocked water next to (x, y); returns what was caught
    pub fn take_fish(&mut self, x: usize, y: usize, amount: u32) -> u32 {
        let best = self
            .adjacent(x, y)
            .iter()
            .filter(|c| c.terrain == Terrain::Water)
            .max_by(|a, b| a.fish.total_cmp(&b.fish))
            .map(|c| (c.x, c.y));
        match best.and_then(|(bx, by)| self.get_mut(bx, by)) {
            Some(cell) => {
                let caught = (amount as f64).min(cell.fish.floor()) as u32;
                cell.fish -= caught as f64;
                caught
            }
            None => 0,
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

    /// Wild game breeds logistically (r * N * (1 - N / K)) in the seasons that allow it; herds
    /// above what their land supports shrink. Then a share of every herd wanders to the
    /// neighbouring cells, so hunted-out ground slowly refills from around it
    pub fn update_wildlife(&mut self, breeding: f64) {
        let r = self.game_growth * breeding;
        for cell in &mut self.cells {
            let k = cell.game_capacity;
            if k > 0.0 {
                cell.game = (cell.game + r * cell.game * (1.0 - cell.game / k)).max(0.0);
            } else {
                cell.game *= 1.0 - self.game_growth;
            }
        }
        let rf = self.fish_growth * breeding;
        for cell in self.cells.iter_mut().filter(|c| c.fish_capacity > 0.0) {
            let n = cell.fish.max(0.5);
            cell.fish = (cell.fish + rf * n * (1.0 - n / cell.fish_capacity)).clamp(0.0, cell.fish_capacity);
        }
        if self.game_dispersal <= 0.0 {
            return;
        }
        let mut inflow = vec![0.0; self.cells.len()];
        for i in 0..self.cells.len() {
            let (x, y) = (self.cells[i].x, self.cells[i].y);
            let targets: Vec<usize> = self
                .adjacent(x, y)
                .iter()
                .filter(|c| c.terrain != Terrain::Water)
                .map(|c| c.y * self.width + c.x)
                .collect();
            if targets.is_empty() {
                continue;
            }
            let leaving = self.cells[i].game * self.game_dispersal;
            inflow[i] -= leaving;
            for t in &targets {
                inflow[*t] += leaving / targets.len() as f64;
            }
        }
        for (cell, delta) in self.cells.iter_mut().zip(inflow) {
            cell.game = (cell.game + delta).max(0.0);
        }
    }

    /// Animals within a hunter's range: this cell and the eight around it
    pub fn game_around(&self, x: usize, y: usize) -> f64 {
        self.get(x, y).map_or(0.0, |c| c.game) + self.adjacent(x, y).iter().map(|c| c.game).sum::<f64>()
    }

    /// Remove one animal killed by a hunter at (x, y), from the cell in range with the most game
    pub fn take_game(&mut self, x: usize, y: usize) {
        let mut range: Vec<(usize, usize)> = self.adjacent(x, y).iter().map(|c| (c.x, c.y)).collect();
        range.push((x, y));
        if let Some(&(bx, by)) = range
            .iter()
            .max_by(|a, b| self.get(a.0, a.1).unwrap().game.total_cmp(&self.get(b.0, b.1).unwrap().game))
        {
            let cell = self.get_mut(bx, by).unwrap();
            cell.game = (cell.game - 1.0).max(0.0);
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
        self.update_wildlife(food_regen_modifier);
    }

    /// Describe a cell for agent perception
    pub fn describe_cell(&self, x: usize, y: usize) -> String {
        match self.get(x, y) {
            Some(cell) => {
                let terrain_desc = match cell.terrain {
                    Terrain::Fertile => "fertile ground",
                    Terrain::Barren => "barren land",
                    Terrain::Water => {
                        let fish = if cell.fish >= cell.fish_capacity * 0.5 {
                            "plenty of fish"
                        } else if cell.fish >= 1.0 {
                            "a few fish"
                        } else {
                            "no fish to be seen"
                        };
                        return format!("a lake with {}", fish);
                    }
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
                    Terrain::Fertile if cell.is_forest() => ", thick forest",
                    Terrain::Fertile if cell.wood * 2 >= cell.wood_capacity => ", wooded",
                    Terrain::Fertile if cell.wood > 0 => ", sparse brush",
                    Terrain::Fertile => ", stripped of wood",
                    Terrain::Barren if cell.stone > 0 => ", stony",
                    Terrain::Barren => ", quarried out",
                    Terrain::Water => "",
                };
                let relief_desc = if cell.terrain != Terrain::Water && cell.elevation >= HIGH_GROUND {
                    ", high ground"
                } else {
                    ""
                };
                let game_desc = if cell.game >= 1.5 {
                    ", tracks of game"
                } else if cell.game >= 0.5 {
                    ", a few animal tracks"
                } else {
                    ""
                };
                format!("{} with {}{}{}{}{}{}", terrain_desc, food_desc, materials_desc, relief_desc, game_desc, structure_desc, territory_desc)
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

    /// How far one can see from (x, y): further from high ground, only to the next cell from
    /// inside thick forest
    pub fn sight_from(&self, x: usize, y: usize, base: usize) -> usize {
        match self.get(x, y) {
            Some(c) if c.is_forest() => base.min(1),
            Some(c) if c.elevation >= HIGH_GROUND => base + 1,
            _ => base,
        }
    }

    /// Energy multiplier for stepping from one cell to another: climbing costs more, and so
    /// does pushing into thick forest
    pub fn step_cost(&self, from: (usize, usize), to: (usize, usize)) -> f64 {
        let (Some(a), Some(b)) = (self.get(from.0, from.1), self.get(to.0, to.1)) else {
            return 1.0;
        };
        let climb = (b.elevation - a.elevation).max(0.0);
        (1.0 + CLIMB_COST * climb) * if b.is_forest() { FOREST_COST } else { 1.0 }
    }

    /// Cells within `radius` steps of (x, y), including it
    pub fn visible_cells(&self, x: usize, y: usize, radius: usize) -> impl Iterator<Item = &Cell> {
        self.cells.iter().filter(move |c| c.x.abs_diff(x) <= radius && c.y.abs_diff(y) <= radius)
    }

    /// What an agent sees: its own and adjacent cells in full, and anything notable further out
    /// within `radius` (good food, water, structures, game, claimed land)
    pub fn perception_summary(&self, x: usize, y: usize, radius: usize) -> String {
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
        let distant: Vec<String> = self
            .visible_cells(x, y, radius)
            .filter(|c| c.x.abs_diff(x).max(c.y.abs_diff(y)) >= 2)
            .filter(|c| {
                c.food > 5 || c.terrain == Terrain::Water || c.structure.is_some() || c.territory.is_some() || c.game >= 1.5
            })
            .map(|c| format!("{} {}: {}", c.x.abs_diff(x).max(c.y.abs_diff(y)), compass(x, y, c.x, c.y), self.describe_cell(c.x, c.y)))
            .collect();

        let mut summary = format!("You are at ({}, {}): {}\nNearby: {}", x, y, current, adjacent.join("; "));
        if !distant.is_empty() {
            summary.push_str(&format!("\nFurther off (cells away, direction): {}", distant.join("; ")));
        }
        summary
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

/// Compass direction from one cell toward another at any distance
pub fn compass(from_x: usize, from_y: usize, to_x: usize, to_y: usize) -> &'static str {
    let step = |a: usize, b: usize| (1 + (b as i64 - a as i64).signum()) as usize;
    direction_name(1, 1, step(from_x, to_x), step(from_y, to_y))
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
            wildlife: WildlifeConfig::default(),
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
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
            wildlife: WildlifeConfig::default(),
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
        };
        let mut world = World::new(&config);
        let total = |w: &World| w.cells.iter().map(|c| c.wood).sum::<u32>();
        assert!((600..=3000).contains(&total(&world)), "stands start part-grown");
        world.wood_regrowth = 0.1;
        for c in &mut world.cells { c.wood = 30; }
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
    fn test_game_breeds_wanders_and_can_be_hunted_out() {
        let config = WorldConfig {
            width: 5,
            height: 5,
            fertile_fraction: 1.0,
            initial_food_per_fertile: 10,
            food_regen_rate: 0.1,
            materials: MaterialsConfig::default(),
            wildlife: WildlifeConfig { growth: 0.0, ..WildlifeConfig::default() },
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
        };
        let mut world = World::new(&config);
        let total = |w: &World| w.cells.iter().map(|c| c.game).sum::<f64>();
        assert!((total(&world) - 50.0).abs() < 1e-9, "without breeding there is nothing to settle");

        // Wandering moves animals around but never creates or destroys them
        world.get_mut(2, 2).unwrap().game = 20.0;
        world.update_wildlife(1.0);
        assert!((total(&world) - 68.0).abs() < 1e-9);
        assert!(world.get(2, 2).unwrap().game < 20.0);

        // Hunting takes animals out of the range
        let before = world.game_around(2, 2);
        world.take_game(2, 2);
        assert!((world.game_around(2, 2) - (before - 1.0)).abs() < 1e-9);

        // With breeding, a depleted herd recovers toward what the land supports, not beyond
        world.game_growth = 0.1;
        world.game_dispersal = 0.0;
        for c in &mut world.cells { c.game = 0.5; }
        for _ in 0..200 { world.update_wildlife(1.0); }
        assert!(world.cells.iter().all(|c| (c.game - 2.0).abs() < 0.05), "herds settle at capacity");
        // ...and not at all outside the breeding seasons
        for c in &mut world.cells { c.game = 0.5; }
        world.update_wildlife(0.0);
        assert!(world.cells.iter().all(|c| (c.game - 0.5).abs() < 1e-9));
    }

    #[test]
    fn test_lakes_are_connected_and_stocked() {
        let config = WorldConfig {
            width: 10,
            height: 10,
            fertile_fraction: 0.5,
            initial_food_per_fertile: 10,
            food_regen_rate: 0.1,
            materials: MaterialsConfig::default(),
            wildlife: WildlifeConfig::default(),
            water: WaterConfig { lakes: 1, lake_size: 6, ..WaterConfig::default() },
            clustering: 2,
        };
        for _ in 0..20 {
            let world = World::new(&config);
            let lake: Vec<&Cell> = world.cells.iter().filter(|c| c.terrain == Terrain::Water).collect();
            assert_eq!(lake.len(), 6);
            assert!(lake.iter().all(|c| c.food == 0 && c.wood == 0 && c.game == 0.0 && c.fish == 15.0));
            // Every lake cell touches another one (orthogonally)
            assert!(lake.iter().all(|a| lake.iter().any(|b| a.x.abs_diff(b.x) + a.y.abs_diff(b.y) == 1)));
            let (x, y) = (lake[0].x, lake[0].y);
            assert!(!world.walkable(x, y));
            let (lx, ly) = world.nearest_land(x, y);
            assert!(world.walkable(lx, ly));
        }
    }

    #[test]
    fn test_agents_see_notable_things_further_off() {
        let config = WorldConfig {
            width: 10,
            height: 10,
            fertile_fraction: 0.0,
            initial_food_per_fertile: 10,
            food_regen_rate: 0.1,
            materials: MaterialsConfig::default(),
            wildlife: WildlifeConfig { game_per_barren: 0.0, ..WildlifeConfig::default() },
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
        };
        let mut world = World::new(&config);
        world.get_mut(7, 3).unwrap().terrain = Terrain::Water;
        world.get_mut(5, 7).unwrap().food = 12;

        let seen = world.perception_summary(5, 5, 2);
        assert!(seen.contains("2 NE: a lake"), "{}", seen);
        assert!(seen.contains("2 S: barren land with abundant food"), "{}", seen);
        assert!(!world.perception_summary(5, 5, 1).contains("Further off"));
        assert_eq!(compass(5, 5, 2, 9), "SW");
        assert_eq!(compass(5, 5, 5, 0), "N");
    }

    #[test]
    fn test_fertile_land_comes_in_patches() {
        // Share of a fertile cell's neighbours that are fertile too, averaged over many worlds
        let patchiness = |clustering: usize| {
            let config = WorldConfig {
                width: 20,
                height: 20,
                fertile_fraction: 0.3,
                initial_food_per_fertile: 10,
                food_regen_rate: 0.1,
                materials: MaterialsConfig::default(),
                wildlife: WildlifeConfig { growth: 0.0, ..WildlifeConfig::default() },
                water: WaterConfig { lakes: 0, ..WaterConfig::default() },
                clustering,
            };
            let mut shares = Vec::new();
            for _ in 0..20 {
                let world = World::new(&config);
                let fertile: Vec<&Cell> = world.cells.iter().filter(|c| c.terrain == Terrain::Fertile).collect();
                assert_eq!(fertile.len(), 120, "the fertile fraction is exact");
                for c in fertile {
                    let adj = world.adjacent(c.x, c.y);
                    shares.push(adj.iter().filter(|a| a.terrain == Terrain::Fertile).count() as f64 / adj.len() as f64);
                }
            }
            shares.iter().sum::<f64>() / shares.len() as f64
        };
        let scattered = patchiness(0);
        let patchy = patchiness(2);

        // Patches don't favour the map's edges
        let config = WorldConfig {
            width: 20, height: 20, fertile_fraction: 0.3, initial_food_per_fertile: 10, food_regen_rate: 0.1,
            materials: MaterialsConfig::default(),
            wildlife: WildlifeConfig { growth: 0.0, ..WildlifeConfig::default() },
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
        };
        let (mut edge, mut edge_fertile) = (0, 0);
        for _ in 0..50 {
            for c in World::new(&config).cells {
                if c.x == 0 || c.y == 0 || c.x == 19 || c.y == 19 {
                    edge += 1;
                    edge_fertile += (c.terrain == Terrain::Fertile) as usize;
                }
            }
        }
        let edge_share = edge_fertile as f64 / edge as f64;
        assert!((0.24..0.36).contains(&edge_share), "edge cells fertile: {}", edge_share);
        assert!((0.25..0.35).contains(&scattered), "scattered: {}", scattered);
        assert!(patchy > 0.5, "patchy: {}", patchy);
    }

    #[test]
    fn test_relief_and_forest_shape_movement_and_sight() {
        let config = WorldConfig {
            width: 10, height: 10, fertile_fraction: 0.3, initial_food_per_fertile: 10, food_regen_rate: 0.1,
            materials: MaterialsConfig::default(),
            wildlife: WildlifeConfig { growth: 0.0, ..WildlifeConfig::default() },
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
        };
        let mut world = World::new(&config);
        // Fertile valleys lie lower than barren uplands
        let mean = |t: Terrain| {
            let e: Vec<f64> = world.cells.iter().filter(|c| c.terrain == t).map(|c| c.elevation).collect();
            e.iter().sum::<f64>() / e.len() as f64
        };
        assert!(mean(Terrain::Fertile) < mean(Terrain::Barren));

        for c in &mut world.cells { c.elevation = 0.5; c.wood = 0; c.wood_capacity = 0; }
        world.get_mut(1, 0).unwrap().elevation = 1.0;
        assert!((world.step_cost((0, 0), (1, 0)) - 2.0).abs() < 1e-9, "climbing half the relief doubles the cost");
        assert!((world.step_cost((1, 0), (0, 0)) - 1.0).abs() < 1e-9, "going down is level cost");
        assert_eq!(world.sight_from(1, 0, 2), 3, "high ground sees further");
        let forest = world.get_mut(5, 5).unwrap();
        forest.wood_capacity = 30;
        forest.wood = 25;
        assert_eq!(world.sight_from(5, 5, 2), 1, "thick forest closes the view");
        assert!((world.step_cost((4, 5), (5, 5)) - 1.5).abs() < 1e-9);
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
            wildlife: WildlifeConfig::default(),
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
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
            wildlife: WildlifeConfig::default(),
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
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
            wildlife: WildlifeConfig::default(),
            water: WaterConfig { lakes: 0, ..WaterConfig::default() },
            clustering: 2,
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
