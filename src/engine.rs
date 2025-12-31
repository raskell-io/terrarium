use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::action::{Action, Direction};
use crate::agent::{generate_names, generate_offspring_name, Agent, Episode, EpisodeCategory, Identity};
use crate::config::Config;
use crate::environment::{EnvironmentConfig, EnvironmentState};
use crate::groups::{GroupTracker, Group};
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
    /// Group/alliance tracker
    group_tracker: GroupTracker,
    /// Environment configuration (seasons, hazards, etc.)
    environment: EnvironmentConfig,
    /// Pending births to be processed at end of epoch
    pending_births: Vec<Agent>,
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

        // Get environment config (use from config or default to earth temperate)
        let environment = config
            .environment
            .clone()
            .unwrap_or_else(EnvironmentConfig::default);

        info!("Environment: {} (cycle: {} epochs)", environment.name, environment.cycle_length);

        Ok(Self {
            config,
            world,
            agents,
            llm,
            chronicle,
            recent_events: Vec::new(),
            max_event_epochs: 10,
            group_tracker: GroupTracker::new(),
            environment,
            pending_births: Vec::new(),
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
            .map(|a| AgentView::from_agent(a, &self.agents, &self.config.aging))
            .collect()
    }

    /// Get view of a specific agent by ID
    pub fn agent_view(&self, id: Uuid) -> Option<AgentView> {
        self.agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| AgentView::from_agent(a, &self.agents, &self.config.aging))
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

    /// Get current groups/alliances
    pub fn current_groups(&self) -> &[Group] {
        self.group_tracker.current_groups()
    }

    /// Get the current environment state
    pub fn environment_state(&self) -> EnvironmentState {
        self.environment.state_at(self.world.epoch)
    }

    /// Get the environment configuration
    pub fn environment_config(&self) -> &EnvironmentConfig {
        &self.environment
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

        // Get current environment state
        let env_state = self.environment.state_at(epoch);

        // Log epoch start
        self.log_and_track(Event::epoch_start(epoch))?;

        // 1. World tick (regenerate resources with environmental modifier)
        self.world.tick(self.config.world.food_regen_rate, env_state.food_regen_modifier);

        // 2. Update agent needs (with environmental effects)
        let mut death_events = Vec::new();
        for agent in &mut self.agents {
            if agent.is_alive() {
                agent.tick_hunger();
                agent.tick_energy();

                // Apply environmental hazard effects
                if env_state.hazard_level > 0.0 {
                    // Extra energy drain from harsh environment
                    let extra_drain = env_state.energy_drain * env_state.hazard_level;
                    agent.physical.energy = (agent.physical.energy - extra_drain).max(0.0);

                    // High hazard can cause health damage
                    if env_state.hazard_level > 0.5 {
                        let health_damage = (env_state.hazard_level - 0.5) * 0.02;
                        agent.physical.health = (agent.physical.health - health_damage).max(0.0);
                    }
                }

                agent.update_goal();

                // Check for death (starvation or environmental)
                if !agent.is_alive() {
                    let cause = if agent.physical.hunger >= 1.0 {
                        "starvation"
                    } else if env_state.hazard_level > 0.5 {
                        env_state.hazard_type.describe()
                    } else {
                        "exhaustion"
                    };
                    death_events.push(Event::died(epoch, agent.id, cause));
                }
            }
        }
        for event in death_events {
            self.log_and_track(event)?;
        }

        // 3. Perception and deliberation (collect actions)
        let mut actions: HashMap<Uuid, Action> = HashMap::new();

        // Build environment perception
        let env_perception = self.environment.describe(epoch);

        for agent in &self.agents {
            if !agent.is_alive() {
                continue;
            }

            // Get perception (world + environment)
            let world_perception = self.world.perception_summary(agent.physical.x, agent.physical.y);
            let perception = format!("{}\n{}", env_perception, world_perception);

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
        self.resolve_actions(epoch, actions.clone())?;

        // 5. Resolve mating (requires mutual consent check)
        self.resolve_mating(epoch, &actions)?;

        // 6. Tick reproduction systems
        self.tick_gestations(epoch)?;
        self.tick_courtship_decay();
        self.process_births();

        // 7. Tick aging (after reproduction so newborns get their first epoch)
        self.tick_aging(epoch)?;

        // 8. Update beliefs based on what happened
        self.update_beliefs(epoch);

        // 9. Detect groups/alliances
        self.detect_groups(epoch)?;

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
        // Get environment state for movement cost
        let env_state = self.environment.state_at(epoch);
        let base_movement_cost = 0.05 * env_state.movement_cost;
        let aging_config = self.config.aging.clone();

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
                    let age_mod = self.agents[agent_idx].age_modifier(&aging_config);
                    let recovery = 0.05 * age_mod;
                    self.agents[agent_idx].physical.energy =
                        (self.agents[agent_idx].physical.energy + recovery).min(1.0);
                }

                Action::Move(dir) => {
                    let age_mod = self.agents[agent_idx].age_modifier(&aging_config);
                    let agent = &mut self.agents[agent_idx];
                    let (dx, dy) = dir.delta();
                    let new_x = (agent.physical.x as i32 + dx).max(0) as usize;
                    let new_y = (agent.physical.y as i32 + dy).max(0) as usize;

                    if new_x < self.world.width && new_y < self.world.height {
                        let from = (agent.physical.x, agent.physical.y);
                        agent.physical.x = new_x;
                        agent.physical.y = new_y;
                        // Movement cost affected by environment and age (elderly use more energy)
                        let movement_cost = base_movement_cost / age_mod;
                        agent.physical.energy = (agent.physical.energy - movement_cost).max(0.0);

                        self.log_and_track(Event::moved(
                            epoch,
                            agent_id,
                            from,
                            (new_x, new_y),
                        ))?;
                    }
                }

                Action::Gather => {
                    let age_mod = self.agents[agent_idx].age_modifier(&aging_config);
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Calculate skill bonus: hunting +50% at max, foraging +30% at max
                    let hunting_level = agent.skills.level("hunting");
                    let foraging_level = agent.skills.level("foraging");
                    let skill_bonus = 1.0 + hunting_level * 0.5 + foraging_level * 0.3;

                    // How many agents are gathering here?
                    let num_gatherers = gathers_per_cell.get(&pos).map(|v| v.len()).unwrap_or(1);

                    // Split the take amount, modified by age and skills
                    let base_max = 5 / num_gatherers as u32;
                    let max_take = ((base_max as f64 * age_mod * skill_bonus).round() as u32).max(1);

                    // Take food from cell
                    let (taken, remaining_food) = if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                        let taken = cell.take_food(max_take);
                        (taken, cell.food)
                    } else {
                        (0, 0)
                    };

                    if taken > 0 {
                        self.agents[agent_idx].add_food(taken);
                        // Gathering energy cost affected by age (elderly use more energy)
                        let gather_cost = 0.1 / age_mod;
                        self.agents[agent_idx].physical.energy =
                            (self.agents[agent_idx].physical.energy - gather_cost).max(0.0);

                        // Practice foraging skill when gathering
                        self.agents[agent_idx].skills.practice("foraging", epoch);

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
                    // Rest recovery affected by age
                    let age_mod = self.agents[agent_idx].age_modifier(&aging_config);
                    let recovery = 0.3 * age_mod;
                    self.agents[agent_idx].physical.energy =
                        (self.agents[agent_idx].physical.energy + recovery).min(1.0);
                    self.log_and_track(Event::rested(epoch, agent_id))?;
                }

                Action::Speak { target, message } => {
                    let target_idx = self.agents.iter().position(|a| a.id == target);
                    if let Some(target_idx) = target_idx {
                        // Check proximity
                        let agent = &self.agents[agent_idx];
                        let target_agent = &self.agents[target_idx];

                        if is_adjacent(agent, target_agent) {
                            // Leadership bonus: +50% sentiment gain at max level
                            let leadership_bonus = 1.0 + agent.skills.level("leadership") * 0.5;

                            self.log_and_track(Event::spoke(
                                epoch,
                                agent_id,
                                target,
                                &message,
                            ))?;

                            // Practice leadership when speaking
                            self.agents[agent_idx].skills.practice("leadership", epoch);

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

                            // Update familiarity (speaker gets leadership bonus for target's sentiment)
                            self.agents[agent_idx].beliefs.update_sentiment(
                                target,
                                &target_name,
                                0.05,
                                epoch,
                            );
                            self.agents[target_idx].beliefs.update_sentiment(
                                agent_id,
                                &agent_name,
                                0.05 * leadership_bonus,
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
                            // Leadership bonus: +50% trust/sentiment gain at max level
                            let leadership_bonus = 1.0 + agent.skills.level("leadership") * 0.5;

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

                                // Practice leadership when giving
                                self.agents[agent_idx].skills.practice("leadership", epoch);

                                // Update trust (giver gets leadership bonus)
                                self.agents[target_idx].beliefs.update_trust(
                                    agent_id,
                                    &agent_name,
                                    0.2 * leadership_bonus,
                                    epoch,
                                );
                                self.agents[target_idx].beliefs.update_sentiment(
                                    agent_id,
                                    &agent_name,
                                    0.2 * leadership_bonus,
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

                Action::Gossip { target, about } => {
                    let target_idx = self.agents.iter().position(|a| a.id == target);
                    let about_idx = self.agents.iter().position(|a| a.id == about);

                    if let (Some(target_idx), Some(about_idx)) = (target_idx, about_idx) {
                        let agent = &self.agents[agent_idx];
                        let target_agent = &self.agents[target_idx];

                        if is_adjacent(agent, target_agent) && target_agent.is_alive() {
                            // Diplomacy bonus: gossip is 2x as influential at max level
                            let diplomacy_bonus = 1.0 + agent.skills.level("diplomacy");

                            // Get the gossiper's beliefs about the subject
                            let (gossiper_trust, gossiper_sentiment) = self.agents[agent_idx]
                                .beliefs
                                .get_social(about)
                                .map(|b| (b.trust, b.sentiment))
                                .unwrap_or((0.0, 0.0));

                            // Apply diplomacy bonus to influence
                            let effective_trust = gossiper_trust * diplomacy_bonus;
                            let effective_sentiment = gossiper_sentiment * diplomacy_bonus;

                            let agent_name = self.agents[agent_idx].name().to_string();
                            let target_name = self.agents[target_idx].name().to_string();
                            let about_name = self.agents[about_idx].name().to_string();

                            // Practice diplomacy when gossiping
                            self.agents[agent_idx].skills.practice("diplomacy", epoch);

                            // Target receives the gossip and updates their belief
                            let sentiment_desc = self.agents[target_idx].beliefs.receive_gossip(
                                agent_id,
                                about,
                                &about_name,
                                effective_trust,
                                effective_sentiment,
                                epoch,
                            );

                            // Log the gossip event
                            self.log_and_track(Event::gossiped(
                                epoch,
                                agent_id,
                                target,
                                about,
                                &sentiment_desc,
                            ))?;

                            // Both agents remember the gossip
                            self.agents[agent_idx].memory.remember(Episode::social(
                                epoch,
                                &format!("I told {} about {}", target_name, about_name),
                                0.1,
                                target,
                            ));

                            self.agents[target_idx].memory.remember(Episode::social(
                                epoch,
                                &format!("{} told me about {}", agent_name, about_name),
                                0.1,
                                agent_id,
                            ));

                            // Gossiping increases familiarity
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

                Action::Court { target } => {
                    if !self.config.reproduction.enabled {
                        continue;
                    }
                    let target_idx = self.agents.iter().position(|a| a.id == target);
                    if let Some(target_idx) = target_idx {
                        let agent = &self.agents[agent_idx];
                        let target_agent = &self.agents[target_idx];

                        if is_adjacent(agent, target_agent) && target_agent.is_alive() {
                            let agent_name = self.agents[agent_idx].name().to_string();
                            let target_name = self.agents[target_idx].name().to_string();

                            // Increase courtship score for both parties
                            let increment = self.config.reproduction.courtship_increment;

                            let new_score_a = self.agents[agent_idx]
                                .reproduction
                                .courtship_progress
                                .entry(target)
                                .or_insert(0.0);
                            *new_score_a = (*new_score_a + increment).min(1.0);
                            let score_from_agent = *new_score_a;

                            let new_score_b = self.agents[target_idx]
                                .reproduction
                                .courtship_progress
                                .entry(agent_id)
                                .or_insert(0.0);
                            *new_score_b = (*new_score_b + increment * 0.5).min(1.0); // Recipient gains less
                            let score_from_target = *new_score_b;

                            // Log courtship event
                            self.log_and_track(Event::courted(
                                epoch,
                                agent_id,
                                target,
                                score_from_agent,
                            ))?;

                            // Create memories
                            self.agents[agent_idx].memory.remember(Episode::social(
                                epoch,
                                &format!("I courted {}", target_name),
                                0.2,
                                target,
                            ));

                            self.agents[target_idx].memory.remember(Episode::social(
                                epoch,
                                &format!("{} courted me", agent_name),
                                0.15,
                                agent_id,
                            ));

                            // Boost sentiment
                            self.agents[agent_idx].beliefs.update_sentiment(
                                target,
                                &target_name,
                                0.1,
                                epoch,
                            );
                            self.agents[target_idx].beliefs.update_sentiment(
                                agent_id,
                                &agent_name,
                                0.08,
                                epoch,
                            );

                            debug!(
                                "{} courted {} (courtship: {:.2} / {:.2})",
                                agent_name, target_name, score_from_agent, score_from_target
                            );
                        }
                    }
                }

                Action::Mate { target: _ } => {
                    // Mate actions are handled separately after all actions are collected
                    // to check for mutual consent
                }

                Action::Teach { target, skill } => {
                    if !self.config.skills.enabled {
                        continue;
                    }

                    let target_idx = self.agents.iter().position(|a| a.id == target);
                    if let Some(target_idx) = target_idx {
                        let agent = &self.agents[agent_idx];
                        let target_agent = &self.agents[target_idx];

                        // Check: adjacent, target alive, teacher has skill at teachable level
                        let teacher_level = agent.skills.level(&skill);
                        let min_level = self.config.skills.min_level_to_teach;

                        if is_adjacent(agent, target_agent)
                            && target_agent.is_alive()
                            && teacher_level >= min_level
                        {
                            let agent_name = self.agents[agent_idx].name().to_string();
                            let target_name = self.agents[target_idx].name().to_string();

                            // Calculate skill improvement
                            // Base: teacher_level * teaching_multiplier * learning_rate
                            // Bonus from target's openness (learning aptitude)
                            let learning_rate = self.config.skills.learning_rate;
                            let teaching_mult = self.config.skills.teaching_multiplier;
                            let teacher_teaching_skill = self.agents[agent_idx].skills.level("teaching");
                            let target_openness = self.agents[target_idx].identity.personality.openness;

                            let improvement = teacher_level
                                * learning_rate
                                * teaching_mult
                                * (1.0 + teacher_teaching_skill * 0.5)
                                * (1.0 + target_openness * 0.3);

                            // Target can't exceed teacher's level
                            let target_current = self.agents[target_idx].skills.level(&skill);
                            let max_new_level = teacher_level.min(1.0);
                            let new_level = (target_current + improvement).min(max_new_level);

                            if new_level > target_current {
                                self.agents[target_idx].skills.improve(&skill, improvement, epoch);

                                // Teacher practices teaching skill
                                self.agents[agent_idx].skills.practice("teaching", epoch);
                                let practice_imp = self.config.skills.practice_improvement;
                                self.agents[agent_idx].skills.improve("teaching", practice_imp * 0.5, epoch);

                                // Energy cost for teaching
                                self.agents[agent_idx].physical.energy =
                                    (self.agents[agent_idx].physical.energy - 0.1).max(0.0);

                                // Log event
                                self.log_and_track(Event::skill_taught(
                                    epoch,
                                    agent_id,
                                    target,
                                    &skill,
                                    new_level,
                                ))?;

                                // Create memories
                                self.agents[agent_idx].memory.remember(Episode::social(
                                    epoch,
                                    &format!("I taught {} about {}", target_name, skill),
                                    0.2,
                                    target,
                                ));

                                self.agents[target_idx].memory.remember(Episode::social(
                                    epoch,
                                    &format!("{} taught me {}", agent_name, skill),
                                    0.3,
                                    agent_id,
                                ));

                                // Boost trust and sentiment
                                self.agents[target_idx].beliefs.update_trust(
                                    agent_id,
                                    &agent_name,
                                    0.1,
                                    epoch,
                                );
                                self.agents[target_idx].beliefs.update_sentiment(
                                    agent_id,
                                    &agent_name,
                                    0.1,
                                    epoch,
                                );

                                debug!(
                                    "{} taught {} to {} (now at {:.2})",
                                    agent_name, skill, target_name, new_level
                                );
                            }
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

    /// Detect and log group/alliance changes
    fn detect_groups(&mut self, epoch: usize) -> Result<()> {
        let changes = self.group_tracker.detect(&self.agents, epoch);

        // Log new groups
        for group in &changes.formed {
            let members: Vec<_> = group.members.iter().copied().collect();
            self.log_and_track(Event::group_formed(epoch, &group.name, members))?;
            info!(
                "Group formed: {} with {} members",
                group.name,
                group.members.len()
            );
        }

        // Log dissolved groups
        for group in &changes.dissolved {
            let members: Vec<_> = group.members.iter().copied().collect();
            self.log_and_track(Event::group_dissolved(epoch, &group.name, members))?;
            info!("Group dissolved: {}", group.name);
        }

        // Log membership changes
        for (group, added, removed) in &changes.changed {
            let added_names: Vec<_> = added
                .iter()
                .filter_map(|id| self.agents.iter().find(|a| a.id == *id))
                .map(|a| a.name())
                .collect();
            let removed_names: Vec<_> = removed
                .iter()
                .filter_map(|id| self.agents.iter().find(|a| a.id == *id))
                .map(|a| a.name())
                .collect();

            let description = if !added.is_empty() && !removed.is_empty() {
                format!(
                    "{} joined, {} left",
                    added_names.join(", "),
                    removed_names.join(", ")
                )
            } else if !added.is_empty() {
                format!("{} joined", added_names.join(", "))
            } else {
                format!("{} left", removed_names.join(", "))
            };

            self.log_and_track(Event::group_changed(epoch, &group.name, &description))?;
            debug!("Group {} changed: {}", group.name, description);
        }

        // Log leadership changes
        for (group, old_leader, new_leader) in &changes.leadership_changed {
            self.log_and_track(Event::leadership_changed(
                epoch,
                &group.name,
                *old_leader,
                *new_leader,
            ))?;

            let new_leader_name = self
                .agents
                .iter()
                .find(|a| a.id == *new_leader)
                .map(|a| a.name())
                .unwrap_or("Unknown");

            let old_leader_name = old_leader
                .and_then(|id| self.agents.iter().find(|a| a.id == id))
                .map(|a| a.name());

            if let Some(old_name) = old_leader_name {
                info!(
                    "{}: {} succeeded {} as leader",
                    group.name, new_leader_name, old_name
                );
            } else {
                info!("{}: {} became leader", group.name, new_leader_name);
            }
        }

        // Log new rivalries
        for rivalry in &changes.rivalries_formed {
            let group_a_name = self.group_tracker.groups
                .iter()
                .find(|g| g.id == rivalry.group_a)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let group_b_name = self.group_tracker.groups
                .iter()
                .find(|g| g.id == rivalry.group_b)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let rivalry_desc = rivalry.rivalry_type.describe();

            self.log_and_track(Event::rivalry_formed(
                epoch,
                &group_a_name,
                &group_b_name,
                rivalry_desc,
            ))?;

            if rivalry.rivalry_type.is_conflict() {
                info!(
                    "Rivalry: {} and {} are now {}",
                    group_a_name, group_b_name, rivalry_desc
                );
            }
        }

        // Log rivalry changes
        for (rivalry, old_type, new_type) in &changes.rivalries_changed {
            let group_a_name = self.group_tracker.groups
                .iter()
                .find(|g| g.id == rivalry.group_a)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let group_b_name = self.group_tracker.groups
                .iter()
                .find(|g| g.id == rivalry.group_b)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let old_desc = old_type.describe();
            let new_desc = new_type.describe();

            self.log_and_track(Event::rivalry_changed(
                epoch,
                &group_a_name,
                &group_b_name,
                old_desc,
                new_desc,
            ))?;

            info!(
                "Relations: {} and {} changed from {} to {}",
                group_a_name, group_b_name, old_desc, new_desc
            );
        }

        // Log ended rivalries
        for rivalry in &changes.rivalries_ended {
            // Look up names from dissolved groups if needed
            let group_a_name = self.group_tracker.groups
                .iter()
                .find(|g| g.id == rivalry.group_a)
                .map(|g| g.name.clone())
                .or_else(|| {
                    self.group_tracker.dissolved
                        .iter()
                        .find(|(g, _)| g.id == rivalry.group_a)
                        .map(|(g, _)| g.name.clone())
                })
                .unwrap_or_else(|| "Unknown".to_string());
            let group_b_name = self.group_tracker.groups
                .iter()
                .find(|g| g.id == rivalry.group_b)
                .map(|g| g.name.clone())
                .or_else(|| {
                    self.group_tracker.dissolved
                        .iter()
                        .find(|(g, _)| g.id == rivalry.group_b)
                        .map(|(g, _)| g.name.clone())
                })
                .unwrap_or_else(|| "Unknown".to_string());

            self.log_and_track(Event::rivalry_ended(
                epoch,
                &group_a_name,
                &group_b_name,
            ))?;
        }

        Ok(())
    }

    // ==================== Aging System ====================

    /// Tick aging: increment age for all agents and check for natural death
    fn tick_aging(&mut self, epoch: usize) -> Result<()> {
        if !self.config.aging.enabled {
            return Ok(());
        }

        use rand::Rng;
        let mut rng = rand::rng();
        let aging_config = &self.config.aging;

        let mut death_events = Vec::new();

        for agent in &mut self.agents {
            if !agent.is_alive() {
                continue;
            }

            // Increment age
            agent.physical.age += 1;
            let age = agent.physical.age;

            // Check for natural death
            if age >= aging_config.max_lifespan {
                // Certain death at max lifespan
                agent.physical.health = 0.0;
                death_events.push(Event::died(epoch, agent.id, "old age"));
            } else if age >= aging_config.elderly_start {
                // Probabilistic death after elderly_start
                let age_factor = (age - aging_config.elderly_start) as f64
                    / (aging_config.max_lifespan - aging_config.elderly_start) as f64;
                let death_probability = aging_config.death_probability_rate * age_factor;

                if rng.random::<f64>() < death_probability {
                    agent.physical.health = 0.0;
                    death_events.push(Event::died(epoch, agent.id, "old age"));
                }
            }
        }

        for event in death_events {
            self.log_and_track(event)?;
        }

        Ok(())
    }

    // ==================== Reproduction System ====================

    /// Tick gestations: energy drain during pregnancy, check for births
    fn tick_gestations(&mut self, epoch: usize) -> Result<()> {
        if !self.config.reproduction.enabled {
            return Ok(());
        }

        let energy_drain = self.config.reproduction.gestation_energy_drain;
        let starting_food = self.config.reproduction.offspring_starting_food;

        // Collect births to process
        let mut births: Vec<(Uuid, Uuid, Uuid, Identity, String)> = Vec::new();

        for agent in &mut self.agents {
            if !agent.is_alive() {
                continue;
            }

            if let Some(gestation) = &agent.reproduction.gestation {
                // Energy drain during pregnancy
                agent.physical.energy = (agent.physical.energy - energy_drain).max(0.0);

                // Check if birth is due
                if epoch >= gestation.expected_birth_epoch {
                    births.push((
                        agent.id,
                        agent.id,
                        gestation.partner_id,
                        gestation.offspring_identity.clone(),
                        gestation.offspring_name.clone(),
                    ));
                }
            }
        }

        // Process births
        for (_agent_id, carrier_id, partner_id, offspring_identity, _offspring_name) in births {
            let carrier_idx = match self.agents.iter().position(|a| a.id == carrier_id) {
                Some(idx) => idx,
                None => continue,
            };
            let carrier = &self.agents[carrier_idx];
            let spawn_pos = self.find_adjacent_spawn(carrier.physical.x, carrier.physical.y);

            // Calculate generation (max of parents + 1)
            let carrier_gen = self.agents[carrier_idx].reproduction.family.generation;
            let partner_idx = self.agents.iter().position(|a| a.id == partner_id);
            let partner_gen = partner_idx
                .map(|idx| self.agents[idx].reproduction.family.generation)
                .unwrap_or(0);
            let offspring_generation = carrier_gen.max(partner_gen) + 1;

            // Get parent skills for inheritance
            let parent_skills = partner_idx.map(|idx| {
                (&self.agents[carrier_idx].skills, &self.agents[idx].skills)
            });

            // Create the child
            let child = Agent::new_with_identity(
                offspring_identity,
                spawn_pos.0,
                spawn_pos.1,
                starting_food,
                vec![carrier_id, partner_id],
                offspring_generation,
                parent_skills,
            );
            let child_id = child.id;
            let child_name = child.name().to_string();

            // Log birth event
            self.log_and_track(Event::birth_occurred(
                epoch,
                carrier_id,
                partner_id,
                child_id,
                &child_name,
            ))?;

            info!("{} was born to the family!", child_name);

            // Queue the child to be added
            self.pending_births.push(child);

            // Clear gestation and update family records for carrier
            self.agents[carrier_idx].reproduction.gestation = None;
            self.agents[carrier_idx].reproduction.family.children.push(child_id);

            // Update family records for partner (if alive)
            if let Some(partner_idx) = self.agents.iter().position(|a| a.id == partner_id) {
                self.agents[partner_idx].reproduction.family.children.push(child_id);
            }

            // Create memories for parents
            self.agents[carrier_idx].memory.remember(Episode::social(
                epoch,
                &format!("I gave birth to {}", child_name),
                0.8,
                child_id,
            ));

            if let Some(partner_idx) = self.agents.iter().position(|a| a.id == partner_id) {
                let carrier_name = self.agents[carrier_idx].name().to_string();
                self.agents[partner_idx].memory.remember(Episode::social(
                    epoch,
                    &format!("{} and I had a child named {}", carrier_name, child_name),
                    0.7,
                    child_id,
                ));
            }
        }

        Ok(())
    }

    /// Decay courtship scores each epoch
    fn tick_courtship_decay(&mut self) {
        if !self.config.reproduction.enabled {
            return;
        }

        let decay = self.config.reproduction.courtship_decay;

        for agent in &mut self.agents {
            if !agent.is_alive() {
                continue;
            }

            // Decay courtship scores
            agent.reproduction.courtship_progress.retain(|_, score| {
                *score -= decay;
                *score > 0.0
            });

            // Decrement mating cooldown
            if agent.reproduction.mating_cooldown > 0 {
                agent.reproduction.mating_cooldown -= 1;
            }
        }
    }

    /// Resolve mating actions - requires mutual consent
    fn resolve_mating(&mut self, epoch: usize, actions: &HashMap<Uuid, Action>) -> Result<()> {
        if !self.config.reproduction.enabled {
            return Ok(());
        }

        // Find all Mate actions
        let mate_actions: Vec<(Uuid, Uuid)> = actions
            .iter()
            .filter_map(|(agent_id, action)| {
                if let Action::Mate { target } = action {
                    Some((*agent_id, *target))
                } else {
                    None
                }
            })
            .collect();

        // Check for mutual consent pairs
        let mut processed: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

        for (agent_a, target_a) in &mate_actions {
            if processed.contains(agent_a) {
                continue;
            }

            // Check if target is also trying to mate with agent_a
            let mutual = mate_actions
                .iter()
                .any(|(agent_b, target_b)| agent_b == target_a && target_b == agent_a);

            if mutual {
                self.attempt_mating(epoch, *agent_a, *target_a)?;
                processed.insert(*agent_a);
                processed.insert(*target_a);
            } else {
                // One-sided - rejection
                if let Some(agent_idx) = self.agents.iter().position(|a| a.id == *agent_a) {
                    let target_name = self.agents
                        .iter()
                        .find(|a| a.id == *target_a)
                        .map(|a| a.name().to_string())
                        .unwrap_or_else(|| "someone".to_string());

                    self.agents[agent_idx].memory.remember(Episode::social(
                        epoch,
                        &format!("{} wasn't interested in mating", target_name),
                        -0.1,
                        *target_a,
                    ));
                }
            }
        }

        Ok(())
    }

    /// Attempt mating between two agents
    fn attempt_mating(&mut self, epoch: usize, agent_a: Uuid, agent_b: Uuid) -> Result<()> {
        let idx_a = self.agents.iter().position(|a| a.id == agent_a);
        let idx_b = self.agents.iter().position(|a| a.id == agent_b);

        let (idx_a, idx_b) = match (idx_a, idx_b) {
            (Some(a), Some(b)) => (a, b),
            _ => return Ok(()),
        };

        // Validate mating conditions
        let config = &self.config.reproduction;

        // Check adjacency
        if !is_adjacent(&self.agents[idx_a], &self.agents[idx_b]) {
            return Ok(());
        }

        // Check if both are alive
        if !self.agents[idx_a].is_alive() || !self.agents[idx_b].is_alive() {
            return Ok(());
        }

        // Check health requirements
        if self.agents[idx_a].physical.health < config.min_health_to_reproduce
            || self.agents[idx_b].physical.health < config.min_health_to_reproduce
        {
            return Ok(());
        }

        // Check energy requirements
        if self.agents[idx_a].physical.energy < config.min_energy_to_reproduce
            || self.agents[idx_b].physical.energy < config.min_energy_to_reproduce
        {
            return Ok(());
        }

        // Check food requirements
        if self.agents[idx_a].physical.food < config.mating_food_cost
            || self.agents[idx_b].physical.food < config.mating_food_cost
        {
            return Ok(());
        }

        // Check mating cooldowns
        if self.agents[idx_a].reproduction.mating_cooldown > 0
            || self.agents[idx_b].reproduction.mating_cooldown > 0
        {
            return Ok(());
        }

        // Check if either is already gestating
        if self.agents[idx_a].reproduction.gestation.is_some()
            || self.agents[idx_b].reproduction.gestation.is_some()
        {
            return Ok(());
        }

        // Check courtship threshold (average of both scores)
        let score_a = self.agents[idx_a]
            .reproduction
            .courtship_progress
            .get(&agent_b)
            .copied()
            .unwrap_or(0.0);
        let score_b = self.agents[idx_b]
            .reproduction
            .courtship_progress
            .get(&agent_a)
            .copied()
            .unwrap_or(0.0);
        let avg_score = (score_a + score_b) / 2.0;

        if avg_score < config.courtship_threshold {
            return Ok(());
        }

        // All checks passed - proceed with mating!
        let name_a = self.agents[idx_a].name().to_string();
        let name_b = self.agents[idx_b].name().to_string();

        info!("{} and {} are mating!", name_a, name_b);

        // Deduct food cost
        self.agents[idx_a].remove_food(config.mating_food_cost);
        self.agents[idx_b].remove_food(config.mating_food_cost);

        // Set mating cooldowns
        self.agents[idx_a].reproduction.mating_cooldown = config.mating_cooldown;
        self.agents[idx_b].reproduction.mating_cooldown = config.mating_cooldown;

        // Update mate history
        self.agents[idx_a].reproduction.family.mate_history.push(agent_b);
        self.agents[idx_b].reproduction.family.mate_history.push(agent_a);

        // Randomly select carrier (who gestates)
        let carrier_idx = if rand::random::<bool>() { idx_a } else { idx_b };
        let partner_idx = if carrier_idx == idx_a { idx_b } else { idx_a };
        let carrier_id = self.agents[carrier_idx].id;
        let partner_id = self.agents[partner_idx].id;

        // Generate offspring identity
        let existing_names: Vec<String> = self.agents.iter().map(|a| a.name().to_string()).collect();
        let offspring_name = generate_offspring_name(
            &self.agents[idx_a].name(),
            &self.agents[idx_b].name(),
            &existing_names,
        );
        let offspring_identity = Identity::from_parents(
            offspring_name.clone(),
            &self.agents[idx_a].identity,
            &self.agents[idx_b].identity,
        );

        // Create gestation
        let gestation = crate::agent::Gestation {
            partner_id,
            conception_epoch: epoch,
            expected_birth_epoch: epoch + config.gestation_period,
            offspring_identity,
            offspring_name,
        };

        self.agents[carrier_idx].reproduction.gestation = Some(gestation);

        // Log conception event
        self.log_and_track(Event::conceived(epoch, carrier_id, partner_id))?;

        // Create memories
        let carrier_name = self.agents[carrier_idx].name().to_string();
        let partner_name = self.agents[partner_idx].name().to_string();

        self.agents[carrier_idx].memory.remember(Episode::social(
            epoch,
            &format!("{} and I conceived a child", partner_name),
            0.6,
            partner_id,
        ));

        self.agents[partner_idx].memory.remember(Episode::social(
            epoch,
            &format!("{} and I conceived a child", carrier_name),
            0.6,
            carrier_id,
        ));

        // Boost relationship
        self.agents[carrier_idx].beliefs.update_sentiment(partner_id, &partner_name, 0.2, epoch);
        self.agents[partner_idx].beliefs.update_sentiment(carrier_id, &carrier_name, 0.2, epoch);
        self.agents[carrier_idx].beliefs.update_trust(partner_id, &partner_name, 0.15, epoch);
        self.agents[partner_idx].beliefs.update_trust(carrier_id, &carrier_name, 0.15, epoch);

        Ok(())
    }

    /// Add pending births to the simulation
    fn process_births(&mut self) {
        let births = std::mem::take(&mut self.pending_births);
        // Register and add each new birth
        for child in births {
            // Register the child's name in the chronicle
            self.chronicle.register_agents(std::slice::from_ref(&child));
            self.agents.push(child);
        }
    }

    /// Find an adjacent spawn position for a newborn
    fn find_adjacent_spawn(&self, x: usize, y: usize) -> (usize, usize) {
        // Try adjacent cells first
        let deltas = [
            (0, 1), (1, 0), (0, -1), (-1, 0),
            (1, 1), (1, -1), (-1, 1), (-1, -1),
        ];

        for (dx, dy) in deltas {
            let nx = (x as i32 + dx).max(0) as usize;
            let ny = (y as i32 + dy).max(0) as usize;
            if nx < self.world.width && ny < self.world.height {
                return (nx, ny);
            }
        }

        // Fallback to same position
        (x, y)
    }
}

/// Check if two agents are adjacent (within 1 cell)
fn is_adjacent(a: &Agent, b: &Agent) -> bool {
    let dx = (a.physical.x as i32 - b.physical.x as i32).abs();
    let dy = (a.physical.y as i32 - b.physical.y as i32).abs();
    dx <= 1 && dy <= 1
}
