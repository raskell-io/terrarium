mod terrain;

pub use terrain::{Terrain, TerrainType, Cell};

use serde::{Deserialize, Serialize};
use crate::config::WorldConfig;

/// The world state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    /// Current epoch
    pub epoch: usize,
    /// The terrain grid
    pub terrain: Terrain,
    /// Current season (0-3: spring, summer, autumn, winter)
    pub season: usize,
    /// Global events active this epoch
    pub active_events: Vec<WorldEvent>,
    /// History of significant world events
    pub event_history: Vec<(usize, WorldEvent)>,
}

/// Events that affect the whole world or regions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorldEvent {
    /// Good harvest season
    Abundance { region: Option<(usize, usize)> },
    /// Poor harvest / drought
    Scarcity { region: Option<(usize, usize)> },
    /// Disease outbreak
    Plague { severity: f64 },
    /// Natural disaster
    Disaster { kind: DisasterKind, location: (usize, usize) },
    /// New resource discovered
    Discovery { resource: String, location: (usize, usize) },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DisasterKind {
    Flood,
    Fire,
    Earthquake,
    Storm,
}

impl World {
    pub fn new(config: &WorldConfig) -> Self {
        Self {
            epoch: 0,
            terrain: Terrain::generate(config),
            season: 0,
            active_events: Vec::new(),
            event_history: Vec::new(),
        }
    }

    /// Advance the world by one epoch
    pub fn tick(&mut self) {
        self.epoch += 1;

        // Update season every 4 epochs
        if self.epoch % 4 == 0 {
            self.season = (self.season + 1) % 4;
        }

        // Regenerate resources
        self.terrain.regenerate_resources(self.season);

        // Clear old events
        self.active_events.clear();

        // Possibly generate new events (low probability)
        self.maybe_generate_event();
    }

    fn maybe_generate_event(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // 5% chance of an event each epoch
        if rng.gen::<f64>() < 0.05 {
            let event = match rng.gen_range(0..4) {
                0 => WorldEvent::Abundance { region: None },
                1 => WorldEvent::Scarcity { region: None },
                2 => WorldEvent::Plague { severity: rng.gen_range(0.1..0.5) },
                _ => {
                    let loc = (
                        rng.gen_range(0..self.terrain.width),
                        rng.gen_range(0..self.terrain.height),
                    );
                    WorldEvent::Disaster {
                        kind: DisasterKind::Storm,
                        location: loc,
                    }
                }
            };
            self.event_history.push((self.epoch, event.clone()));
            self.active_events.push(event);
        }
    }

    /// Get season name
    pub fn season_name(&self) -> &'static str {
        match self.season {
            0 => "Spring",
            1 => "Summer",
            2 => "Autumn",
            3 => "Winter",
            _ => unreachable!(),
        }
    }

    /// Get resource multiplier based on season
    pub fn seasonal_multiplier(&self) -> f64 {
        match self.season {
            0 => 1.2, // Spring: growth
            1 => 1.5, // Summer: abundance
            2 => 1.0, // Autumn: harvest
            3 => 0.5, // Winter: scarcity
            _ => 1.0,
        }
    }

    /// Get a description of the current world state
    pub fn describe(&self) -> String {
        format!(
            "Epoch {}, {} (Season {})\nWorld size: {}x{}\nActive events: {}",
            self.epoch,
            self.season_name(),
            self.season,
            self.terrain.width,
            self.terrain.height,
            if self.active_events.is_empty() {
                "None".to_string()
            } else {
                format!("{:?}", self.active_events)
            }
        )
    }
}
