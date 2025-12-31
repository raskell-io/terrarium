//! Environment and scenario system.
//!
//! Supports diverse geographical scenarios from Earth to off-world colonies.
//! Environments define cycles (seasons), hazards, and resource dynamics.

use serde::{Deserialize, Serialize};

/// Environment configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnvironmentConfig {
    /// Environment type name (e.g., "earth_temperate", "mars", "moon")
    pub name: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Length of one full cycle in epochs (e.g., 100 epochs = 1 year)
    #[serde(default = "default_cycle_length")]
    pub cycle_length: usize,
    /// Phases within the cycle (seasons, day/night, etc.)
    #[serde(default = "default_phases")]
    pub phases: Vec<Phase>,
    /// Base hazard level (0.0 = safe, 1.0 = extremely hazardous)
    #[serde(default)]
    pub base_hazard: f64,
    /// Type of environmental hazard
    #[serde(default)]
    pub hazard_type: HazardType,
    /// Gravity modifier (1.0 = Earth, 0.16 = Moon, 0.38 = Mars)
    #[serde(default = "default_gravity")]
    pub gravity: f64,
    /// Whether there's breathable atmosphere
    #[serde(default = "default_atmosphere")]
    pub breathable_atmosphere: bool,
    /// Base temperature description
    #[serde(default)]
    pub base_temperature: Temperature,
    /// Day length in epochs (0 = no day/night cycle)
    #[serde(default)]
    pub day_length: usize,
}

/// A phase within an environmental cycle (like a season)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Phase {
    /// Phase name (e.g., "Spring", "Dust Storm Season", "Polar Night")
    pub name: String,
    /// Start position in cycle (0.0 to 1.0)
    pub start: f64,
    /// End position in cycle (0.0 to 1.0)
    pub end: f64,
    /// Food regeneration modifier (1.0 = normal)
    #[serde(default = "default_one")]
    pub food_regen_modifier: f64,
    /// Hazard modifier (1.0 = normal)
    #[serde(default = "default_one")]
    pub hazard_modifier: f64,
    /// Energy drain modifier (1.0 = normal)
    #[serde(default = "default_one")]
    pub energy_drain_modifier: f64,
    /// Movement cost modifier (1.0 = normal)
    #[serde(default = "default_one")]
    pub movement_cost_modifier: f64,
    /// Description for agents
    #[serde(default)]
    pub description: String,
}

/// Types of environmental hazards
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum HazardType {
    #[default]
    None,
    /// Extreme cold (Antarctica, Mars night, deep space)
    Cold,
    /// Extreme heat (desert, Venus)
    Heat,
    /// Radiation exposure (Moon, Mars, space)
    Radiation,
    /// Dust/sand storms (Mars, desert)
    DustStorm,
    /// Toxic atmosphere
    Toxic,
    /// Low oxygen / vacuum
    Vacuum,
    /// Combined multiple hazards
    Multiple,
}

impl HazardType {
    pub fn describe(&self) -> &'static str {
        match self {
            HazardType::None => "none",
            HazardType::Cold => "extreme cold",
            HazardType::Heat => "extreme heat",
            HazardType::Radiation => "radiation",
            HazardType::DustStorm => "dust storms",
            HazardType::Toxic => "toxic atmosphere",
            HazardType::Vacuum => "vacuum",
            HazardType::Multiple => "multiple hazards",
        }
    }
}

/// Temperature classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum Temperature {
    Freezing,   // < -20C
    Cold,       // -20C to 5C
    #[default]
    Temperate,  // 5C to 25C
    Hot,        // 25C to 40C
    Scorching,  // > 40C
}

impl Temperature {
    pub fn describe(&self) -> &'static str {
        match self {
            Temperature::Freezing => "freezing",
            Temperature::Cold => "cold",
            Temperature::Temperate => "temperate",
            Temperature::Hot => "hot",
            Temperature::Scorching => "scorching",
        }
    }
}

/// Current environmental state (computed from config + epoch)
#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentState {
    /// Current phase name
    pub current_phase: String,
    /// Current phase description
    pub phase_description: String,
    /// Effective food regeneration rate (base * modifier)
    pub food_regen_modifier: f64,
    /// Effective hazard level (base * modifier)
    pub hazard_level: f64,
    /// Hazard type
    pub hazard_type: HazardType,
    /// Energy drain per epoch from environment
    pub energy_drain: f64,
    /// Movement cost modifier
    pub movement_cost: f64,
    /// Position in cycle (0.0 to 1.0)
    pub cycle_position: f64,
    /// Current cycle number
    pub cycle_number: usize,
}

impl EnvironmentConfig {
    /// Get the current environmental state for a given epoch
    pub fn state_at(&self, epoch: usize) -> EnvironmentState {
        let cycle_number = epoch / self.cycle_length.max(1);
        let cycle_position = if self.cycle_length > 0 {
            (epoch % self.cycle_length) as f64 / self.cycle_length as f64
        } else {
            0.0
        };

        // Find current phase
        let current_phase = self.phases.iter()
            .find(|p| cycle_position >= p.start && cycle_position < p.end)
            .or_else(|| self.phases.first());

        match current_phase {
            Some(phase) => EnvironmentState {
                current_phase: phase.name.clone(),
                phase_description: phase.description.clone(),
                food_regen_modifier: phase.food_regen_modifier,
                hazard_level: self.base_hazard * phase.hazard_modifier,
                hazard_type: self.hazard_type,
                energy_drain: 0.05 * phase.energy_drain_modifier * (1.0 + self.base_hazard),
                movement_cost: phase.movement_cost_modifier,
                cycle_position,
                cycle_number,
            },
            None => EnvironmentState {
                current_phase: "Unknown".to_string(),
                phase_description: String::new(),
                food_regen_modifier: 1.0,
                hazard_level: self.base_hazard,
                hazard_type: self.hazard_type,
                energy_drain: 0.05,
                movement_cost: 1.0,
                cycle_position,
                cycle_number,
            },
        }
    }

    /// Get perception description for agents
    pub fn describe(&self, epoch: usize) -> String {
        let state = self.state_at(epoch);
        let mut desc = format!("Environment: {} ({})", self.name, state.current_phase);

        if !state.phase_description.is_empty() {
            desc.push_str(&format!(". {}", state.phase_description));
        }

        if state.hazard_level > 0.0 {
            desc.push_str(&format!(
                ". Hazard: {} ({:.0}%)",
                self.hazard_type.describe(),
                state.hazard_level * 100.0
            ));
        }

        desc
    }
}

// Default functions for serde
fn default_cycle_length() -> usize { 100 }
fn default_gravity() -> f64 { 1.0 }
fn default_atmosphere() -> bool { true }
fn default_one() -> f64 { 1.0 }

fn default_phases() -> Vec<Phase> {
    vec![Phase {
        name: "Default".to_string(),
        start: 0.0,
        end: 1.0,
        food_regen_modifier: 1.0,
        hazard_modifier: 1.0,
        energy_drain_modifier: 1.0,
        movement_cost_modifier: 1.0,
        description: String::new(),
    }]
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self::earth_temperate()
    }
}

// =============================================================================
// Predefined Environment Templates
// =============================================================================

impl EnvironmentConfig {
    /// Earth with temperate 4-season cycle
    pub fn earth_temperate() -> Self {
        Self {
            name: "Earth (Temperate)".to_string(),
            description: "A temperate Earth region with four distinct seasons.".to_string(),
            cycle_length: 100,
            phases: vec![
                Phase {
                    name: "Spring".to_string(),
                    start: 0.0,
                    end: 0.25,
                    food_regen_modifier: 1.2,
                    hazard_modifier: 0.5,
                    energy_drain_modifier: 0.9,
                    movement_cost_modifier: 1.0,
                    description: "Plants bloom and food becomes more abundant.".to_string(),
                },
                Phase {
                    name: "Summer".to_string(),
                    start: 0.25,
                    end: 0.5,
                    food_regen_modifier: 1.5,
                    hazard_modifier: 0.3,
                    energy_drain_modifier: 0.8,
                    movement_cost_modifier: 1.0,
                    description: "Warm weather and plentiful resources.".to_string(),
                },
                Phase {
                    name: "Autumn".to_string(),
                    start: 0.5,
                    end: 0.75,
                    food_regen_modifier: 0.8,
                    hazard_modifier: 0.5,
                    energy_drain_modifier: 1.0,
                    movement_cost_modifier: 1.0,
                    description: "Harvest time, but resources are dwindling.".to_string(),
                },
                Phase {
                    name: "Winter".to_string(),
                    start: 0.75,
                    end: 1.0,
                    food_regen_modifier: 0.2,
                    hazard_modifier: 2.0,
                    energy_drain_modifier: 1.5,
                    movement_cost_modifier: 1.3,
                    description: "Cold and harsh. Food is scarce.".to_string(),
                },
            ],
            base_hazard: 0.1,
            hazard_type: HazardType::Cold,
            gravity: 1.0,
            breathable_atmosphere: true,
            base_temperature: Temperature::Temperate,
            day_length: 0,
        }
    }

    /// Antarctica - extreme polar environment
    pub fn antarctica() -> Self {
        Self {
            name: "Antarctica".to_string(),
            description: "Earth's frozen continent. Extreme cold, limited resources.".to_string(),
            cycle_length: 100,
            phases: vec![
                Phase {
                    name: "Polar Summer".to_string(),
                    start: 0.0,
                    end: 0.25,
                    food_regen_modifier: 0.5,
                    hazard_modifier: 0.5,
                    energy_drain_modifier: 1.2,
                    movement_cost_modifier: 1.2,
                    description: "Endless daylight but still freezing. Brief window for resources.".to_string(),
                },
                Phase {
                    name: "Autumn Freeze".to_string(),
                    start: 0.25,
                    end: 0.4,
                    food_regen_modifier: 0.1,
                    hazard_modifier: 1.5,
                    energy_drain_modifier: 1.8,
                    movement_cost_modifier: 1.5,
                    description: "Temperatures plummet. Darkness approaches.".to_string(),
                },
                Phase {
                    name: "Polar Night".to_string(),
                    start: 0.4,
                    end: 0.75,
                    food_regen_modifier: 0.0,
                    hazard_modifier: 3.0,
                    energy_drain_modifier: 2.5,
                    movement_cost_modifier: 2.0,
                    description: "Months of total darkness. Extreme cold. Survival is paramount.".to_string(),
                },
                Phase {
                    name: "Spring Thaw".to_string(),
                    start: 0.75,
                    end: 1.0,
                    food_regen_modifier: 0.3,
                    hazard_modifier: 1.0,
                    energy_drain_modifier: 1.5,
                    movement_cost_modifier: 1.3,
                    description: "Light returns. Ice begins to soften.".to_string(),
                },
            ],
            base_hazard: 0.4,
            hazard_type: HazardType::Cold,
            gravity: 1.0,
            breathable_atmosphere: true,
            base_temperature: Temperature::Freezing,
            day_length: 0,
        }
    }

    /// Mars colony
    pub fn mars() -> Self {
        Self {
            name: "Mars".to_string(),
            description: "The red planet. Thin atmosphere, extreme cold, dust storms.".to_string(),
            cycle_length: 200, // Martian year is ~2 Earth years
            phases: vec![
                Phase {
                    name: "Calm Season".to_string(),
                    start: 0.0,
                    end: 0.4,
                    food_regen_modifier: 0.6, // Greenhouse farming
                    hazard_modifier: 0.8,
                    energy_drain_modifier: 1.3,
                    movement_cost_modifier: 1.1,
                    description: "Relatively calm. Dust levels low.".to_string(),
                },
                Phase {
                    name: "Dust Storm Season".to_string(),
                    start: 0.4,
                    end: 0.7,
                    food_regen_modifier: 0.2, // Dust blocks sunlight
                    hazard_modifier: 2.5,
                    energy_drain_modifier: 2.0,
                    movement_cost_modifier: 2.0,
                    description: "Global dust storms. Reduced visibility. Solar power limited.".to_string(),
                },
                Phase {
                    name: "Clearing".to_string(),
                    start: 0.7,
                    end: 1.0,
                    food_regen_modifier: 0.5,
                    hazard_modifier: 1.2,
                    energy_drain_modifier: 1.5,
                    movement_cost_modifier: 1.3,
                    description: "Dust settles. Recovery period.".to_string(),
                },
            ],
            base_hazard: 0.5,
            hazard_type: HazardType::Multiple, // Cold + radiation + dust
            gravity: 0.38,
            breathable_atmosphere: false,
            base_temperature: Temperature::Freezing,
            day_length: 1, // Sol is ~same as Earth day
        }
    }

    /// Lunar colony
    pub fn moon() -> Self {
        Self {
            name: "Moon".to_string(),
            description: "Earth's moon. No atmosphere, extreme temperature swings, radiation.".to_string(),
            cycle_length: 28, // Lunar day/night cycle
            phases: vec![
                Phase {
                    name: "Lunar Day".to_string(),
                    start: 0.0,
                    end: 0.5,
                    food_regen_modifier: 0.4, // Solar-powered hydroponics
                    hazard_modifier: 1.0, // Heat + radiation
                    energy_drain_modifier: 1.2,
                    movement_cost_modifier: 0.8, // Low gravity helps
                    description: "Two weeks of sunlight. Surface temperatures reach 120°C.".to_string(),
                },
                Phase {
                    name: "Lunar Night".to_string(),
                    start: 0.5,
                    end: 1.0,
                    food_regen_modifier: 0.1, // Limited power
                    hazard_modifier: 2.0, // Extreme cold
                    energy_drain_modifier: 2.0,
                    movement_cost_modifier: 1.0,
                    description: "Two weeks of darkness. Surface drops to -180°C.".to_string(),
                },
            ],
            base_hazard: 0.6,
            hazard_type: HazardType::Multiple, // Radiation + vacuum + temperature
            gravity: 0.16,
            breathable_atmosphere: false,
            base_temperature: Temperature::Freezing, // Average
            day_length: 14, // Half the cycle
        }
    }

    /// Generic Earth-like exoplanet
    pub fn exoplanet_earthlike() -> Self {
        Self {
            name: "Kepler-442b".to_string(),
            description: "An Earth-like exoplanet. New frontier for humanity.".to_string(),
            cycle_length: 120, // Slightly longer year
            phases: vec![
                Phase {
                    name: "Growing Season".to_string(),
                    start: 0.0,
                    end: 0.4,
                    food_regen_modifier: 1.3,
                    hazard_modifier: 0.5,
                    energy_drain_modifier: 0.9,
                    movement_cost_modifier: 1.0,
                    description: "Alien flora blooms. Resources are plentiful.".to_string(),
                },
                Phase {
                    name: "Storm Season".to_string(),
                    start: 0.4,
                    end: 0.6,
                    food_regen_modifier: 0.6,
                    hazard_modifier: 2.0,
                    energy_drain_modifier: 1.5,
                    movement_cost_modifier: 1.5,
                    description: "Violent weather patterns. Seek shelter.".to_string(),
                },
                Phase {
                    name: "Dormant Season".to_string(),
                    start: 0.6,
                    end: 1.0,
                    food_regen_modifier: 0.4,
                    hazard_modifier: 1.0,
                    energy_drain_modifier: 1.2,
                    movement_cost_modifier: 1.1,
                    description: "Native life hibernates. Quiet but lean times.".to_string(),
                },
            ],
            base_hazard: 0.2,
            hazard_type: HazardType::None,
            gravity: 1.1,
            breathable_atmosphere: true,
            base_temperature: Temperature::Temperate,
            day_length: 0,
        }
    }

    /// Hostile exoplanet with toxic atmosphere
    pub fn exoplanet_hostile() -> Self {
        Self {
            name: "Gliese 667Cc".to_string(),
            description: "A tidally-locked super-Earth with a toxic atmosphere.".to_string(),
            cycle_length: 50,
            phases: vec![
                Phase {
                    name: "Twilight Zone".to_string(),
                    start: 0.0,
                    end: 0.6,
                    food_regen_modifier: 0.5,
                    hazard_modifier: 1.0,
                    energy_drain_modifier: 1.3,
                    movement_cost_modifier: 1.2,
                    description: "The habitable band between eternal day and night.".to_string(),
                },
                Phase {
                    name: "Acid Rain".to_string(),
                    start: 0.6,
                    end: 1.0,
                    food_regen_modifier: 0.1,
                    hazard_modifier: 3.0,
                    energy_drain_modifier: 2.0,
                    movement_cost_modifier: 1.8,
                    description: "Toxic precipitation. Stay indoors.".to_string(),
                },
            ],
            base_hazard: 0.5,
            hazard_type: HazardType::Toxic,
            gravity: 1.5,
            breathable_atmosphere: false,
            base_temperature: Temperature::Hot,
            day_length: 0, // Tidally locked
        }
    }

    /// Desert environment (Earth)
    pub fn earth_desert() -> Self {
        Self {
            name: "Sahara Desert".to_string(),
            description: "Harsh desert environment. Extreme heat and scarce water.".to_string(),
            cycle_length: 100,
            phases: vec![
                Phase {
                    name: "Cool Season".to_string(),
                    start: 0.0,
                    end: 0.3,
                    food_regen_modifier: 0.4,
                    hazard_modifier: 0.5,
                    energy_drain_modifier: 1.0,
                    movement_cost_modifier: 1.2,
                    description: "Bearable temperatures. Best time for activity.".to_string(),
                },
                Phase {
                    name: "Hot Season".to_string(),
                    start: 0.3,
                    end: 0.8,
                    food_regen_modifier: 0.1,
                    hazard_modifier: 2.0,
                    energy_drain_modifier: 1.8,
                    movement_cost_modifier: 1.5,
                    description: "Scorching heat. Conserve energy and water.".to_string(),
                },
                Phase {
                    name: "Sandstorm Season".to_string(),
                    start: 0.8,
                    end: 1.0,
                    food_regen_modifier: 0.2,
                    hazard_modifier: 2.5,
                    energy_drain_modifier: 1.5,
                    movement_cost_modifier: 2.0,
                    description: "Blinding sandstorms sweep across the dunes.".to_string(),
                },
            ],
            base_hazard: 0.3,
            hazard_type: HazardType::Heat,
            gravity: 1.0,
            breathable_atmosphere: true,
            base_temperature: Temperature::Hot,
            day_length: 0,
        }
    }

    /// Space station / orbital habitat
    pub fn space_station() -> Self {
        Self {
            name: "Orbital Station".to_string(),
            description: "An artificial habitat in Earth orbit.".to_string(),
            cycle_length: 50,
            phases: vec![
                Phase {
                    name: "Normal Operations".to_string(),
                    start: 0.0,
                    end: 0.7,
                    food_regen_modifier: 0.5, // Hydroponics
                    hazard_modifier: 0.5,
                    energy_drain_modifier: 1.0,
                    movement_cost_modifier: 0.7, // Microgravity
                    description: "Systems nominal. Routine station life.".to_string(),
                },
                Phase {
                    name: "Solar Maximum".to_string(),
                    start: 0.7,
                    end: 0.85,
                    food_regen_modifier: 0.3,
                    hazard_modifier: 2.5,
                    energy_drain_modifier: 1.5,
                    movement_cost_modifier: 0.7,
                    description: "Increased solar radiation. Shelter in shielded areas.".to_string(),
                },
                Phase {
                    name: "Maintenance Cycle".to_string(),
                    start: 0.85,
                    end: 1.0,
                    food_regen_modifier: 0.4,
                    hazard_modifier: 1.0,
                    energy_drain_modifier: 1.2,
                    movement_cost_modifier: 0.8,
                    description: "Station maintenance and resupply.".to_string(),
                },
            ],
            base_hazard: 0.4,
            hazard_type: HazardType::Radiation,
            gravity: 0.0, // Microgravity
            breathable_atmosphere: true, // Artificial
            base_temperature: Temperature::Temperate,
            day_length: 0, // 90-minute orbits, abstracted away
        }
    }

    /// Get environment by name
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "earth" | "earth_temperate" | "temperate" => Some(Self::earth_temperate()),
            "antarctica" | "polar" | "arctic" => Some(Self::antarctica()),
            "mars" | "red_planet" => Some(Self::mars()),
            "moon" | "luna" | "lunar" => Some(Self::moon()),
            "exoplanet" | "exoplanet_earthlike" | "earthlike" | "kepler" => Some(Self::exoplanet_earthlike()),
            "hostile" | "exoplanet_hostile" | "toxic" | "gliese" => Some(Self::exoplanet_hostile()),
            "desert" | "earth_desert" | "sahara" => Some(Self::earth_desert()),
            "station" | "space_station" | "orbital" | "space" => Some(Self::space_station()),
            _ => None,
        }
    }

    /// List available preset environments
    pub fn available_presets() -> Vec<&'static str> {
        vec![
            "earth_temperate",
            "antarctica",
            "mars",
            "moon",
            "exoplanet_earthlike",
            "exoplanet_hostile",
            "earth_desert",
            "space_station",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_earth_temperate_phases() {
        let env = EnvironmentConfig::earth_temperate();

        // Start of year = Spring
        let state = env.state_at(0);
        assert_eq!(state.current_phase, "Spring");

        // Middle of year = Summer (epoch 25-49)
        let state = env.state_at(30);
        assert_eq!(state.current_phase, "Summer");

        // End of year = Winter
        let state = env.state_at(80);
        assert_eq!(state.current_phase, "Winter");
    }

    #[test]
    fn test_winter_scarcity() {
        let env = EnvironmentConfig::earth_temperate();

        let summer = env.state_at(30);
        let winter = env.state_at(80);

        // Winter should have lower food regen
        assert!(winter.food_regen_modifier < summer.food_regen_modifier);
        // Winter should have higher hazard
        assert!(winter.hazard_level > summer.hazard_level);
    }

    #[test]
    fn test_mars_dust_storm() {
        let env = EnvironmentConfig::mars();

        let calm = env.state_at(10);
        let storm = env.state_at(100);

        assert_eq!(calm.current_phase, "Calm Season");
        assert_eq!(storm.current_phase, "Dust Storm Season");
        assert!(storm.hazard_level > calm.hazard_level);
    }

    #[test]
    fn test_preset_lookup() {
        assert!(EnvironmentConfig::from_name("mars").is_some());
        assert!(EnvironmentConfig::from_name("moon").is_some());
        assert!(EnvironmentConfig::from_name("invalid").is_none());
    }
}
