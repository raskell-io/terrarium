use crate::agent::{Agent, Action, Episode, EpisodeTag, Personality};
use crate::chronicle::Chronicle;
use crate::config::SimulationConfig;
use crate::llm::LlmClient;
use crate::world::World;
use std::collections::HashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// The main simulation engine
pub struct Engine {
    config: SimulationConfig,
    world: World,
    agents: HashMap<Uuid, Agent>,
    chronicle: Chronicle,
    llm: LlmClient,
}

impl Engine {
    pub async fn new(config: SimulationConfig, output_dir: &str) -> anyhow::Result<Self> {
        // Initialize world
        let world = World::new(&config.world);

        // Initialize LLM client
        let llm = LlmClient::new(&config.llm)?;

        // Initialize chronicle
        let chronicle = Chronicle::new(output_dir, &config.name)?;

        // Create agents
        let mut agents = HashMap::new();
        let names = generate_names(config.agents.count);

        for (i, name) in names.into_iter().enumerate() {
            // Distribute agents across the world
            let x = i % config.world.width;
            let y = i / config.world.width % config.world.height;

            let personality = Personality::random();
            let agent = Agent::new(name, (x, y), personality, 0);

            agents.insert(agent.id, agent);
        }

        info!("Created {} agents in a {}x{} world",
              agents.len(), config.world.width, config.world.height);

        Ok(Self {
            config,
            world,
            agents,
            chronicle,
            llm,
        })
    }

    /// Run the simulation for the configured number of epochs
    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.chronicle.write_header(&self.config, &self.world, &self.agents)?;

        for epoch in 0..self.config.simulation.epochs {
            self.run_epoch(epoch).await?;

            // Save snapshot periodically
            if epoch % self.config.simulation.snapshot_interval == 0 {
                self.save_snapshot(epoch)?;
            }
        }

        self.chronicle.write_footer(&self.world, &self.agents)?;
        Ok(())
    }

    /// Run a single epoch
    async fn run_epoch(&mut self, epoch: usize) -> anyhow::Result<()> {
        debug!("Beginning epoch {}", epoch);

        // 1. World tick
        self.world.tick();

        // 2. Agent perception and deliberation
        let mut actions: Vec<(Uuid, Action)> = Vec::new();

        for (id, agent) in &self.agents {
            if !agent.is_alive() {
                continue;
            }

            // Build context for this agent
            let context = self.build_agent_context(agent);

            // Get action from LLM
            let action = self.llm.decide_action(agent, &context).await?;

            debug!("Agent {} chooses: {:?}", agent.name, action);
            actions.push((*id, action));
        }

        // 3. Resolve actions
        let events = self.resolve_actions(actions)?;

        // 4. Update agent states
        self.update_agents(epoch);

        // 5. Chronicle significant events
        for event in &events {
            self.chronicle.record_event(epoch, event)?;
        }

        // Log epoch summary
        if epoch % 10 == 0 {
            info!("Epoch {} complete. {} agents alive. Season: {}",
                  epoch,
                  self.agents.values().filter(|a| a.is_alive()).count(),
                  self.world.season_name());
        }

        Ok(())
    }

    /// Build the context an agent perceives
    fn build_agent_context(&self, agent: &Agent) -> AgentContext {
        let (x, y) = agent.body.position;

        // Get visible terrain
        let visible_cells: Vec<_> = self.world.terrain
            .adjacent_positions(x, y)
            .into_iter()
            .filter_map(|(px, py)| {
                self.world.terrain.get(px, py).map(|c| ((px, py), c.describe()))
            })
            .collect();

        // Get nearby agents
        let nearby_agents: Vec<_> = self.agents.values()
            .filter(|a| a.id != agent.id && a.is_alive())
            .filter(|a| {
                let (ax, ay) = a.body.position;
                let dx = (ax as i32 - x as i32).abs();
                let dy = (ay as i32 - y as i32).abs();
                dx <= 2 && dy <= 2
            })
            .map(|a| (a.id, a.name.clone(), a.body.position))
            .collect();

        // Current cell
        let current_cell = self.world.terrain.get(x, y)
            .map(|c| c.describe())
            .unwrap_or_else(|| "unknown".to_string());

        AgentContext {
            epoch: self.world.epoch,
            season: self.world.season_name().to_string(),
            current_cell,
            visible_cells,
            nearby_agents,
            world_events: self.world.active_events.iter()
                .map(|e| format!("{:?}", e))
                .collect(),
        }
    }

    /// Resolve all actions and return events
    fn resolve_actions(&mut self, actions: Vec<(Uuid, Action)>) -> anyhow::Result<Vec<SimulationEvent>> {
        let mut events = Vec::new();

        for (agent_id, action) in actions {
            let event = self.resolve_single_action(agent_id, action)?;
            if let Some(e) = event {
                events.push(e);
            }
        }

        Ok(events)
    }

    fn resolve_single_action(&mut self, agent_id: Uuid, action: Action) -> anyhow::Result<Option<SimulationEvent>> {
        let agent = match self.agents.get_mut(&agent_id) {
            Some(a) => a,
            None => return Ok(None),
        };

        match action {
            Action::Wait => {
                agent.body.energy = (agent.body.energy + 0.1).min(1.0);
                Ok(None)
            }

            Action::Move { direction } => {
                let (x, y) = agent.body.position;
                let (dx, dy) = direction_delta(&direction);
                let new_x = (x as i32 + dx).max(0) as usize;
                let new_y = (y as i32 + dy).max(0) as usize;

                if new_x < self.world.terrain.width && new_y < self.world.terrain.height {
                    if let Some(cell) = self.world.terrain.get(new_x, new_y) {
                        if cell.is_passable() {
                            agent.body.position = (new_x, new_y);
                            agent.body.energy = (agent.body.energy - 0.05 * cell.movement_cost).max(0.0);
                        }
                    }
                }
                Ok(None)
            }

            Action::Gather => {
                let (x, y) = agent.body.position;
                if let Some(cell) = self.world.terrain.get_mut(x, y) {
                    let food_gathered = cell.food.min(10);
                    let materials_gathered = cell.materials.min(5);

                    cell.food -= food_gathered;
                    cell.materials -= materials_gathered;

                    agent.body.inventory.food += food_gathered;
                    agent.body.inventory.materials += materials_gathered;
                    agent.body.energy = (agent.body.energy - 0.1).max(0.0);

                    if food_gathered > 5 || materials_gathered > 3 {
                        return Ok(Some(SimulationEvent {
                            description: format!("{} gathered {} food and {} materials",
                                                 agent.name, food_gathered, materials_gathered),
                            participants: vec![agent_id],
                            tags: vec![EpisodeTag::Gain],
                        }));
                    }
                }
                Ok(None)
            }

            Action::Eat => {
                if agent.body.inventory.food > 0 {
                    agent.body.inventory.food -= 1;
                    agent.body.hunger = (agent.body.hunger - 0.3).max(0.0);
                    agent.body.health = (agent.body.health + 0.05).min(1.0);
                }
                Ok(None)
            }

            Action::Rest => {
                agent.body.energy = (agent.body.energy + 0.3).min(1.0);
                Ok(None)
            }

            Action::Trade { target, offer, request } => {
                // Simplified trade resolution
                let agent_name = agent.name.clone();
                let agent_food = agent.body.inventory.food;

                if let Some(target_agent) = self.agents.get(&target) {
                    let target_name = target_agent.name.clone();

                    return Ok(Some(SimulationEvent {
                        description: format!("{} attempted to trade with {}", agent_name, target_name),
                        participants: vec![agent_id, target],
                        tags: vec![EpisodeTag::Trade],
                    }));
                }
                Ok(None)
            }

            Action::Speak { target, message } => {
                let agent_name = agent.name.clone();

                if let Some(target_agent) = self.agents.get(&target) {
                    let target_name = target_agent.name.clone();

                    return Ok(Some(SimulationEvent {
                        description: format!("{} said to {}: \"{}\"", agent_name, target_name, message),
                        participants: vec![agent_id, target],
                        tags: vec![EpisodeTag::Social],
                    }));
                }
                Ok(None)
            }

            Action::Give { target, resource, amount } => {
                let agent_name = agent.name.clone();
                // Simplified giving
                if let Some(target_agent) = self.agents.get(&target) {
                    let target_name = target_agent.name.clone();

                    return Ok(Some(SimulationEvent {
                        description: format!("{} gave {} {} to {}", agent_name, amount, resource, target_name),
                        participants: vec![agent_id, target],
                        tags: vec![EpisodeTag::Kindness],
                    }));
                }
                Ok(None)
            }

            Action::Attack { target } => {
                let agent_name = agent.name.clone();

                if let Some(target_agent) = self.agents.get(&target) {
                    let target_name = target_agent.name.clone();

                    return Ok(Some(SimulationEvent {
                        description: format!("{} attacked {}", agent_name, target_name),
                        participants: vec![agent_id, target],
                        tags: vec![EpisodeTag::Conflict],
                    }));
                }
                Ok(None)
            }

            Action::Build { item } => {
                let agent_name = agent.name.clone();

                if agent.body.inventory.materials >= 5 {
                    agent.body.inventory.materials -= 5;
                    agent.body.inventory.tools += 1;

                    return Ok(Some(SimulationEvent {
                        description: format!("{} built a {}", agent_name, item),
                        participants: vec![agent_id],
                        tags: vec![EpisodeTag::Discovery],
                    }));
                }
                Ok(None)
            }
        }
    }

    /// Update agent states at end of epoch
    fn update_agents(&mut self, epoch: usize) {
        for agent in self.agents.values_mut() {
            if !agent.is_alive() {
                continue;
            }

            // Age
            agent.body.age += 1;

            // Hunger increases each epoch
            agent.body.hunger = (agent.body.hunger + 0.1).min(1.0);

            // High hunger damages health
            if agent.body.hunger > 0.8 {
                agent.body.health -= 0.1;
            }

            // Energy slowly depletes
            agent.body.energy = (agent.body.energy - 0.05).max(0.0);

            // Check for death
            if agent.body.health <= 0.0 {
                info!("Agent {} has died at epoch {} (age: {} epochs)",
                      agent.name, epoch, agent.body.age);
            }
        }
    }

    fn save_snapshot(&self, epoch: usize) -> anyhow::Result<()> {
        debug!("Saving snapshot at epoch {}", epoch);
        // TODO: Implement snapshot saving
        Ok(())
    }
}

/// Context visible to an agent
pub struct AgentContext {
    pub epoch: usize,
    pub season: String,
    pub current_cell: String,
    pub visible_cells: Vec<((usize, usize), String)>,
    pub nearby_agents: Vec<(Uuid, String, (usize, usize))>,
    pub world_events: Vec<String>,
}

/// A significant event in the simulation
pub struct SimulationEvent {
    pub description: String,
    pub participants: Vec<Uuid>,
    pub tags: Vec<EpisodeTag>,
}

fn direction_delta(dir: &crate::agent::Direction) -> (i32, i32) {
    use crate::agent::Direction::*;
    match dir {
        North => (0, -1),
        South => (0, 1),
        East => (1, 0),
        West => (-1, 0),
        NorthEast => (1, -1),
        NorthWest => (-1, -1),
        SouthEast => (1, 1),
        SouthWest => (-1, 1),
    }
}

fn generate_names(count: usize) -> Vec<String> {
    // Simple name generation - can be expanded
    let prefixes = ["Al", "Bri", "Cor", "Dan", "El", "Fay", "Gor", "Hel", "Iri", "Jon",
                    "Kel", "Lor", "Mar", "Ned", "Oli", "Per", "Qui", "Ros", "Sam", "Tor"];
    let suffixes = ["a", "an", "en", "ia", "is", "on", "or", "us", "wyn", "ax"];

    let mut names = Vec::with_capacity(count);
    for i in 0..count {
        let prefix = prefixes[i % prefixes.len()];
        let suffix = suffixes[i / prefixes.len() % suffixes.len()];
        names.push(format!("{}{}", prefix, suffix));
    }
    names
}
