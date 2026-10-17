//! Terrarium in the browser: the simulation engine behind a small WebAssembly API.
//!
//! The page creates a `Simulation` from one of the bundled scenarios (or its own TOML),
//! advances it a day at a time with `step`, and reads `world` once and `frame` every day
//! as JSON. Agents act on instinct; there is no LLM in the browser build.

use std::collections::HashMap;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use serde_json::{Value, json};
use terrarium::config::Config;
use terrarium::engine::Engine;
use terrarium::observation::EventType;
use terrarium::world::Terrain;
use uuid::Uuid;
use wasm_bindgen::prelude::*;

/// Scenarios shipped with the page: (id, TOML)
const SCENARIOS: &[(&str, &str)] = &[
    ("first_winter", include_str!("../../../scenarios/first_winter.toml")),
    ("great_valley", include_str!("../../../scenarios/great_valley.toml")),
    ("exoplanet", include_str!("../../../scenarios/exoplanet.toml")),
    ("antarctica", include_str!("../../../scenarios/antarctica.toml")),
    ("mars_colony", include_str!("../../../scenarios/mars_colony.toml")),
    ("space_station", include_str!("../../../scenarios/space_station.toml")),
];

/// Days to run when the page doesn't say: effectively until everyone is gone
const OPEN_ENDED_DAYS: usize = 1_000_000;

/// Drive a future to completion. Without an LLM nothing in the engine ever waits, so one
/// poll is enough; a future that does wait is a bug in this build
fn run_now<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    match future.as_mut().poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the browser build has nothing to wait for"),
    }
}

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// The bundled scenarios as JSON: [{id, name, description, agents, width, height}]
#[wasm_bindgen]
pub fn scenarios() -> String {
    let list: Vec<Value> = SCENARIOS
        .iter()
        .filter_map(|(id, toml)| {
            let config: Config = ::toml::from_str(toml).ok()?;
            Some(json!({
                "id": id,
                "name": config.meta.name,
                "description": config.meta.description.trim(),
                "agents": config.agents.count,
                "width": config.world.width,
                "height": config.world.height,
            }))
        })
        .collect();
    Value::Array(list).to_string()
}

#[wasm_bindgen]
pub struct Simulation {
    engine: Engine,
}

#[wasm_bindgen]
impl Simulation {
    /// Start a bundled scenario by id
    #[wasm_bindgen(constructor)]
    pub fn new(scenario: &str) -> Result<Simulation, JsError> {
        let toml = SCENARIOS
            .iter()
            .find(|(id, _)| *id == scenario)
            .map(|(_, toml)| *toml)
            .ok_or_else(|| err(format!("unknown scenario {scenario}")))?;
        Self::from_toml(toml)
    }

    /// Start a scenario from TOML text
    #[wasm_bindgen(js_name = fromToml)]
    pub fn from_toml(toml: &str) -> Result<Simulation, JsError> {
        let mut config: Config = ::toml::from_str(toml).map_err(err)?;
        config.simulation.epochs = OPEN_ENDED_DAYS;
        let mut engine = Engine::in_memory(config).map_err(err)?;
        engine.initialize().map_err(err)?;
        Ok(Simulation { engine })
    }

    /// Advance one day
    pub fn step(&mut self) -> Result<(), JsError> {
        run_now(self.engine.step()).map_err(err)
    }

    /// Days elapsed
    pub fn day(&self) -> usize {
        self.engine.epoch()
    }

    /// Whether anyone is still alive
    pub fn populated(&self) -> bool {
        self.engine.alive_count() > 0
    }

    /// What doesn't change: size, terrain and relief, as JSON
    pub fn world(&self) -> String {
        let world = self.engine.world();
        let terrain: String = world
            .cells
            .iter()
            .map(|c| match c.terrain {
                Terrain::Fertile => 'F',
                Terrain::Barren => 'B',
                Terrain::Water => 'W',
            })
            .collect();
        let elevation: Vec<u8> = world.cells.iter().map(|c| (c.elevation * 99.0).round() as u8).collect();
        let config = self.engine.config();
        json!({
            "name": config.meta.name,
            "description": config.meta.description.trim(),
            "environment": self.engine.environment_config().name,
            "cycle": self.engine.environment_config().cycle_length,
            "width": world.width,
            "height": world.height,
            "terrain": terrain,
            "elevation": elevation,
        })
        .to_string()
    }

    /// Everything that changes, as JSON: the land, the people, conditions, and what happened
    /// on the last day
    pub fn frame(&self) -> String {
        let engine = self.engine.config();
        let world = self.engine.world();
        let agents = self.engine.agents();
        let index: HashMap<Uuid, usize> = agents.iter().enumerate().map(|(i, a)| (a.id, i)).collect();
        let owner = |id: &Uuid| index.get(id).copied();

        let cells: Vec<Value> = world
            .cells
            .iter()
            .map(|c| {
                let forest = if c.wood_capacity == 0 { 0 } else { (c.wood * 3 / c.wood_capacity).min(3) };
                json!([c.food, forest, (c.game * 10.0).round() as u32, c.fish.round() as u32])
            })
            .collect();
        let structures: Vec<Value> = world
            .cells
            .iter()
            .filter_map(|c| c.structure.as_ref().map(|s| (c, s)))
            .map(|(c, s)| {
                json!({
                    "x": c.x, "y": c.y,
                    "kind": format!("{:?}", s.structure_type),
                    "name": s.structure_type.display_name(),
                    "complete": s.is_complete(),
                    "progress": (s.build_progress * 100 / s.build_required.max(1)),
                    "owner": owner(&s.owner),
                })
            })
            .collect();
        let land: Vec<Value> = world
            .cells
            .iter()
            .filter_map(|c| c.territory.as_ref().map(|t| json!([c.x, c.y, owner(&t.owner)])))
            .collect();

        // What each person did on the last day, from the event log
        let last_day = world.epoch.saturating_sub(1);
        let today: Vec<_> = self.engine.recent_events().iter().filter(|e| e.epoch == last_day).collect();
        let mut doing: HashMap<Uuid, &'static str> = HashMap::new();
        for e in &today {
            if let Some(a) = e.agent {
                if let Some(verb) = verb(&e.event_type) {
                    doing.insert(a, verb);
                }
            }
        }

        let people: Vec<Value> = agents
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let p = &a.physical;
                json!({
                    "id": i,
                    "name": a.name(),
                    "alive": a.is_alive(),
                    "x": p.x, "y": p.y,
                    "health": p.health.max(0.0), "hunger": p.hunger, "energy": p.energy,
                    "food": p.food,
                    "age": p.age,
                    "stage": a.life_stage(&engine.aging),
                    "generation": a.reproduction.family.generation,
                    "sick": p.sick_days > 0,
                    "sheltered": p.sheltered_at.is_some(),
                    "expecting": a.reproduction.gestation.is_some(),
                    "doing": doing.get(&a.id).copied(),
                    "personality": a.identity.personality.describe(),
                    "aspiration": a.identity.aspiration.describe(),
                })
            })
            .collect();

        let events: Vec<Value> = self
            .engine
            .recent_event_views()
            .into_iter()
            .filter(|e| e.epoch == last_day)
            .map(|e| json!({ "kind": format!("{:?}", e.event_type), "text": e.description }))
            .collect();

        let env = self.engine.environment_state();
        let cycle = self.engine.environment_config().cycle_length.max(1);
        json!({
            "day": world.epoch,
            "year": world.epoch / cycle + 1,
            "dayOfYear": world.epoch % cycle + 1,
            "season": env.current_phase,
            "seasonText": env.phase_description,
            "weather": format!("{:?}", env.weather).to_lowercase(),
            "hazard": env.hazard_level,
            "cells": cells,
            "structures": structures,
            "land": land,
            "people": people,
            "events": events,
        })
        .to_string()
    }
}

/// A short present-tense description of what an event shows someone doing
fn verb(event: &EventType) -> Option<&'static str> {
    Some(match event {
        EventType::Moved => "walking",
        EventType::Rested => "resting",
        EventType::Ate => "eating",
        EventType::Gathered => "foraging",
        EventType::GatheredMaterials => "gathering materials",
        EventType::Courted => "courting",
        EventType::Gave => "sharing food",
        EventType::Crafted => "crafting",
        EventType::Hunted => "hunting",
        EventType::Fished => "fishing",
        EventType::Chopped => "chopping wood",
        EventType::Attacked => "fighting",
        EventType::Raided => "raiding a store",
        EventType::TerritoryMarked => "claiming land",
        EventType::TerritoryChallenged => "warding off a trespasser",
        EventType::TerritorySubmitted => "backing off",
        EventType::Gossiped => "talking",
        EventType::SkillTaught => "teaching",
        _ => return None,
    })
}
