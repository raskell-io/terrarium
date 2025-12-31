use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::action::{Action, Direction};
use crate::agent::{generate_names, Agent, Episode, EpisodeCategory};
use crate::config::Config;
use crate::llm::LlmClient;
use crate::observation::{Chronicle, Event};
use crate::observer::{AgentView, EventView, WorldView};
use crate::world::World;

/// The simulation engine
pub struct Engine {
    config: Config,
    world: World,
    agents: Vec<Agent>,
    llm: LlmClient,
    chronicle: Chronicle,
    /// Recent events for observer clients (last N epochs)
    recent_events: Vec<Event>,
    /// Maximum epochs of events to keep
    max_event_epochs: usize,
}

impl Engine {
    /// Create a new simulation engine
    pub fn new(config: Config, output_dir: &str) -> Result<Self> {
        // Create world
        let world = World::new(&config.world);

        // Create agents
        let names = generate_names(config.agents.count);
        let mut agents = Vec::with_capacity(config.agents.count);

        for (i, name) in names.into_iter().enumerate() {
            // Scatter agents across the world
            let x = (i * 3) % config.world.width;
            let y = (i * 3) / config.world.width % config.world.height;
            agents.push(Agent::new(name, x, y, config.agents.starting_food));
        }

        // Create LLM client
        let llm = LlmClient::new(config.llm.clone());

        // Create chronicle
        let mut chronicle = Chronicle::new(output_dir)?;
        chronicle.register_agents(&agents);

        Ok(Self {
            config,
            world,
            agents,
            llm,
            chronicle,
            recent_events: Vec::new(),
            max_event_epochs: 10,
        })
    }

    // ==================== Observer Interface ====================

    /// Get a view of the current world state
    pub fn world_view(&self) -> WorldView {
        WorldView::from_world(&self.world, &self.agents)
    }

    /// Get views of all agents
    pub fn agent_views(&self) -> Vec<AgentView> {
        self.agents
            .iter()
            .map(|a| AgentView::from_agent(a, &self.agents))
            .collect()
    }

    /// Get view of a specific agent by ID
    pub fn agent_view(&self, id: Uuid) -> Option<AgentView> {
        self.agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| AgentView::from_agent(a, &self.agents))
    }

    /// Get recent events as views
    pub fn recent_event_views(&self) -> Vec<EventView> {
        EventView::from_events(&self.recent_events, &self.agents)
    }

    /// Get the current epoch
    pub fn epoch(&self) -> usize {
        self.world.epoch
    }

    /// Get the total configured epochs
    pub fn total_epochs(&self) -> usize {
        self.config.simulation.epochs
    }

    /// Check if simulation is complete
    pub fn is_complete(&self) -> bool {
        self.world.epoch >= self.config.simulation.epochs
            || self.agents.iter().all(|a| !a.is_alive())
    }

    /// Get count of living agents
    pub fn alive_count(&self) -> usize {
        self.agents.iter().filter(|a| a.is_alive()).count()
    }

    /// Step the simulation by one epoch (for TUI control)
    pub async fn step(&mut self) -> Result<()> {
        if self.is_complete() {
            return Ok(());
        }

        let epoch = self.world.epoch;

        // Run one epoch
        self.run_epoch(epoch).await?;

        // Periodic snapshot
        if epoch % self.config.simulation.snapshot_interval == 0 && epoch > 0 {
            self.chronicle.save_snapshot(epoch, &self.world, &self.agents)?;
        }

        // Prune old events
        self.prune_old_events();

        Ok(())
    }

    /// Initialize the simulation (write header, etc.)
    pub fn initialize(&mut self) -> Result<()> {
        self.chronicle.write_header(
            &self.config.meta.name,
            &self.world,
            &self.agents,
        )?;
        self.chronicle.save_snapshot(0, &self.world, &self.agents)?;
        Ok(())
    }

    /// Finalize the simulation (write footer, final snapshot)
    pub fn finalize(&mut self) -> Result<()> {
        self.chronicle.save_snapshot(self.world.epoch, &self.world, &self.agents)?;
        self.chronicle.write_footer(&self.world, &self.agents)?;
        Ok(())
    }

    /// Log and track an event
    fn log_and_track(&mut self, event: Event) -> Result<()> {
        self.recent_events.push(event.clone());
        self.chronicle.log_event(&event)?;
        Ok(())
    }

    /// Prune events older than max_event_epochs
    fn prune_old_events(&mut self) {
        let cutoff = self.world.epoch.saturating_sub(self.max_event_epochs);
        self.recent_events.retain(|e| e.epoch >= cutoff);
    }

    /// Run the simulation
    pub async fn run(&mut self) -> Result<()> {
        info!(
            "Starting simulation: {} agents, {} epochs",
            self.agents.len(),
            self.config.simulation.epochs
        );

        // Write header
        self.chronicle.write_header(
            &self.config.meta.name,
            &self.world,
            &self.agents,
        )?;

        // Initial snapshot
        self.chronicle.save_snapshot(0, &self.world, &self.agents)?;

        // Main loop
        for epoch in 0..self.config.simulation.epochs {
            self.run_epoch(epoch).await?;

            // Periodic snapshot
            if epoch % self.config.simulation.snapshot_interval == 0 && epoch > 0 {
                self.chronicle.save_snapshot(epoch, &self.world, &self.agents)?;
            }

            // Check if everyone is dead
            if self.agents.iter().all(|a| !a.is_alive()) {
                info!("All agents have perished at epoch {}", epoch);
                break;
            }
        }

        // Final snapshot and footer
        self.chronicle.save_snapshot(self.world.epoch, &self.world, &self.agents)?;
        self.chronicle.write_footer(&self.world, &self.agents)?;

        info!("Simulation complete after {} epochs", self.world.epoch);
        Ok(())
    }

    /// Run a single epoch
    async fn run_epoch(&mut self, epoch: usize) -> Result<()> {
        debug!("Epoch {} starting", epoch);

        // Log epoch start
        self.log_and_track(Event::epoch_start(epoch))?;

        // 1. World tick (regenerate resources)
        self.world.tick(self.config.world.food_regen_rate);

        // 2. Update agent needs
        let mut death_events = Vec::new();
        for agent in &mut self.agents {
            if agent.is_alive() {
                agent.tick_hunger();
                agent.tick_energy();
                agent.update_goal();

                // Check for starvation death
                if !agent.is_alive() {
                    death_events.push(Event::died(epoch, agent.id, "starvation"));
                }
            }
        }
        for event in death_events {
            self.log_and_track(event)?;
        }

        // 3. Perception and deliberation (collect actions)
        let mut actions: HashMap<Uuid, Action> = HashMap::new();

        for agent in &self.agents {
            if !agent.is_alive() {
                continue;
            }

            // Get perception
            let perception = self.world.perception_summary(agent.physical.x, agent.physical.y);

            // Get nearby agents
            let nearby: Vec<(Uuid, &str)> = self
                .agents
                .iter()
                .filter(|a| a.is_alive() && a.id != agent.id && is_adjacent(agent, a))
                .map(|a| (a.id, a.name()))
                .collect();

            // Get action from LLM
            let action = self
                .llm
                .decide_action(agent, &perception, &nearby, epoch)
                .await?;

            debug!("Agent {} chooses: {:?}", agent.name(), action);
            actions.insert(agent.id, action);
        }

        // 4. Resolve actions (simultaneous)
        self.resolve_actions(epoch, actions)?;

        // 5. Update beliefs based on what happened
        self.update_beliefs(epoch);

        // Log epoch end
        self.log_and_track(Event::epoch_end(epoch))?;
        self.chronicle.flush()?;

        // Progress update
        if epoch % 10 == 0 {
            let alive = self.agents.iter().filter(|a| a.is_alive()).count();
            info!("Epoch {}: {} agents alive", epoch, alive);
        }

        Ok(())
    }

    /// Resolve all actions for an epoch
    fn resolve_actions(&mut self, epoch: usize, actions: HashMap<Uuid, Action>) -> Result<()> {
        // Collect gather actions per cell for splitting
        let mut gathers_per_cell: HashMap<(usize, usize), Vec<Uuid>> = HashMap::new();

        // First pass: categorize actions
        for (agent_id, action) in &actions {
            if let Action::Gather = action {
                if let Some(agent) = self.agents.iter().find(|a| a.id == *agent_id) {
                    let pos = (agent.physical.x, agent.physical.y);
                    gathers_per_cell.entry(pos).or_default().push(*agent_id);
                }
            }
        }

        // Second pass: resolve actions
        for (agent_id, action) in actions {
            let agent_idx = self.agents.iter().position(|a| a.id == agent_id);
            if agent_idx.is_none() {
                continue;
            }
            let agent_idx = agent_idx.unwrap();

            match action {
                Action::Wait => {
                    self.agents[agent_idx].physical.energy =
                        (self.agents[agent_idx].physical.energy + 0.05).min(1.0);
                }

                Action::Move(dir) => {
                    let agent = &mut self.agents[agent_idx];
                    let (dx, dy) = dir.delta();
                    let new_x = (agent.physical.x as i32 + dx).max(0) as usize;
                    let new_y = (agent.physical.y as i32 + dy).max(0) as usize;

                    if new_x < self.world.width && new_y < self.world.height {
                        let from = (agent.physical.x, agent.physical.y);
                        agent.physical.x = new_x;
                        agent.physical.y = new_y;
                        agent.physical.energy = (agent.physical.energy - 0.05).max(0.0);

                        self.log_and_track(Event::moved(
                            epoch,
                            agent_id,
                            from,
                            (new_x, new_y),
                        ))?;
                    }
                }

                Action::Gather => {
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // How many agents are gathering here?
                    let num_gatherers = gathers_per_cell.get(&pos).map(|v| v.len()).unwrap_or(1);

                    // Split the take amount
                    let max_take = 5 / num_gatherers as u32;
                    let max_take = max_take.max(1);

                    // Take food from cell
                    let (taken, remaining_food) = if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                        let taken = cell.take_food(max_take);
                        (taken, cell.food)
                    } else {
                        (0, 0)
                    };

                    if taken > 0 {
                        self.agents[agent_idx].add_food(taken);
                        self.agents[agent_idx].physical.energy =
                            (self.agents[agent_idx].physical.energy - 0.1).max(0.0);

                        self.log_and_track(Event::gathered(epoch, agent_id, taken))?;

                        // Update belief about this location
                        self.agents[agent_idx]
                            .beliefs
                            .update_food_belief(pos.0, pos.1, remaining_food, epoch);
                    }
                }

                Action::Eat => {
                    let ate = self.agents[agent_idx].eat();
                    if ate {
                        self.log_and_track(Event::ate(epoch, agent_id))?;

                        self.agents[agent_idx].memory.remember(Episode::survival(
                            epoch,
                            "I ate and felt better",
                            0.3,
                        ));
                    }
                }

                Action::Rest => {
                    self.agents[agent_idx].rest();
                    self.log_and_track(Event::rested(epoch, agent_id))?;
                }

                Action::Speak { target, message } => {
                    let target_idx = self.agents.iter().position(|a| a.id == target);
                    if let Some(target_idx) = target_idx {
                        // Check proximity
                        let agent = &self.agents[agent_idx];
                        let target_agent = &self.agents[target_idx];

                        if is_adjacent(agent, target_agent) {
                            self.log_and_track(Event::spoke(
                                epoch,
                                agent_id,
                                target,
                                &message,
                            ))?;

                            // Both agents remember the interaction
                            let agent_name = self.agents[agent_idx].name().to_string();
                            let target_name = self.agents[target_idx].name().to_string();

                            self.agents[agent_idx].memory.remember(Episode::social(
                                epoch,
                                &format!("I spoke to {}: \"{}\"", target_name, message),
                                0.1,
                                target,
                            ));

                            self.agents[target_idx].memory.remember(Episode::social(
                                epoch,
                                &format!("{} said to me: \"{}\"", agent_name, message),
                                0.1,
                                agent_id,
                            ));

                            // Update familiarity
                            self.agents[agent_idx].beliefs.update_sentiment(
                                target,
                                &target_name,
                                0.05,
                                epoch,
                            );
                            self.agents[target_idx].beliefs.update_sentiment(
                                agent_id,
                                &agent_name,
                                0.05,
                                epoch,
                            );
                        }
                    }
                }

                Action::Give { target, amount } => {
                    let target_idx = self.agents.iter().position(|a| a.id == target);
                    if let Some(target_idx) = target_idx {
                        let agent = &self.agents[agent_idx];
                        let target_agent = &self.agents[target_idx];

                        if is_adjacent(agent, target_agent) {
                            let actual = self.agents[agent_idx].remove_food(amount);
                            if actual > 0 {
                                self.agents[target_idx].add_food(actual);

                                self.log_and_track(Event::gave(
                                    epoch,
                                    agent_id,
                                    target,
                                    actual,
                                ))?;

                                let agent_name = self.agents[agent_idx].name().to_string();
                                let target_name = self.agents[target_idx].name().to_string();

                                // Memories
                                self.agents[agent_idx].memory.remember(Episode::new(
                                    epoch,
                                    format!("I gave {} food to {}", actual, target_name),
                                    0.2,
                                    vec![target],
                                    EpisodeCategory::Gift,
                                ));

                                self.agents[target_idx].memory.remember(Episode::new(
                                    epoch,
                                    format!("{} gave me {} food", agent_name, actual),
                                    0.5,
                                    vec![agent_id],
                                    EpisodeCategory::Gift,
                                ));

                                // Update trust
                                self.agents[target_idx].beliefs.update_trust(
                                    agent_id,
                                    &agent_name,
                                    0.2,
                                    epoch,
                                );
                                self.agents[target_idx].beliefs.update_sentiment(
                                    agent_id,
                                    &agent_name,
                                    0.2,
                                    epoch,
                                );
                            }
                        }
                    }
                }

                Action::Attack { target } => {
                    let target_idx = self.agents.iter().position(|a| a.id == target);
                    if let Some(target_idx) = target_idx {
                        let agent = &self.agents[agent_idx];
                        let target_agent = &self.agents[target_idx];

                        if is_adjacent(agent, target_agent) && target_agent.is_alive() {
                            // Calculate damage (0.1 - 0.3 based on attacker's... randomness for now)
                            let damage = 0.15 + rand::random::<f64>() * 0.1;

                            self.agents[target_idx].take_damage(damage);

                            self.log_and_track(Event::attacked(
                                epoch,
                                agent_id,
                                target,
                                damage,
                            ))?;

                            let agent_name = self.agents[agent_idx].name().to_string();
                            let target_name = self.agents[target_idx].name().to_string();

                            // Check if target died
                            if !self.agents[target_idx].is_alive() {
                                self.log_and_track(Event::died(
                                    epoch,
                                    target,
                                    &format!("attack by {}", agent_name),
                                ))?;
                            }

                            // Memories
                            self.agents[agent_idx].memory.remember(Episode::conflict(
                                epoch,
                                &format!("I attacked {}", target_name),
                                -0.2,
                                target,
                            ));

                            self.agents[target_idx].memory.remember(Episode::conflict(
                                epoch,
                                &format!("{} attacked me!", agent_name),
                                -0.8,
                                agent_id,
                            ));

                            // Update beliefs
                            self.agents[target_idx].beliefs.update_trust(
                                agent_id,
                                &agent_name,
                                -0.5,
                                epoch,
                            );
                            self.agents[target_idx].beliefs.update_sentiment(
                                agent_id,
                                &agent_name,
                                -0.5,
                                epoch,
                            );
                            self.agents[target_idx].beliefs.self_belief.perceived_safety -= 0.2;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Update agent beliefs based on observations
    fn update_beliefs(&mut self, epoch: usize) {
        // Update perceived safety based on recent events
        for agent in &mut self.agents {
            if !agent.is_alive() {
                continue;
            }

            // Update food location beliefs based on current perception
            if let Some(cell) = self.world.get(agent.physical.x, agent.physical.y) {
                if cell.food > 0 {
                    agent.beliefs.update_food_belief(
                        agent.physical.x,
                        agent.physical.y,
                        cell.food,
                        epoch,
                    );
                }
            }

            // Adjust perceived safety over time (regression to mean)
            agent.beliefs.self_belief.perceived_safety =
                agent.beliefs.self_belief.perceived_safety * 0.9 + 0.5 * 0.1;
        }
    }
}

/// Check if two agents are adjacent (within 1 cell)
fn is_adjacent(a: &Agent, b: &Agent) -> bool {
    let dx = (a.physical.x as i32 - b.physical.x as i32).abs();
    let dy = (a.physical.y as i32 - b.physical.y as i32).abs();
    dx <= 1 && dy <= 1
}
