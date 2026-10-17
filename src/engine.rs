use anyhow::Result;
use std::collections::HashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::action::{Action, ActionContext, Direction, Neighbour};
use crate::agent::{generate_names, generate_offspring_name, Agent, Episode, EpisodeCategory, Identity};
use crate::config::Config;
use crate::crafting::{MaterialType, RecipeRegistry, Tool, ToolQuality, ToolType};
use crate::environment::{EnvironmentConfig, EnvironmentState, Weather, WeatherState};
use crate::groups::{GroupTracker, Group};
use crate::llm::LlmClient;
use crate::observation::{Chronicle, Event};
use crate::observer::{AgentView, EventView, ServiceDebtView, TradeProposalView, TradeStateView, WorldView};
use crate::trade::{ProposalStatus, ServiceDebt, ServiceType, TradeableItem, TradeProposal, TradeState};
use crate::world::{Terrain, World};

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
    /// Crafting recipe registry
    recipe_registry: RecipeRegistry,
    /// Trade system state
    trade_state: TradeState,
    /// Open mating proposals: proposer -> (target, last epoch the target can accept)
    mate_proposals: HashMap<Uuid, (Uuid, usize)>,
    /// Droughts and storms on top of the seasons
    weather: WeatherState,
}

/// Days a mating proposal stays open for the other agent to accept
const MATE_PROPOSAL_DAYS: usize = 3;

impl Engine {
    /// Create a new simulation engine
    pub fn new(config: Config, output_dir: &str) -> Result<Self> {
        // Create world
        let world = World::new(&config.world);

        // Create agents
        let names = generate_names(config.agents.count);
        let mut agents = Vec::with_capacity(config.agents.count);
        let starting_age = config.agents.starting_age.unwrap_or(config.aging.youth_end);

        // Scatter founders evenly across the world on a lattice, each on the nearest land
        let (w, h) = (config.world.width, config.world.height);
        let count = names.len().max(1);
        let cols = ((count as f64 * w as f64 / h as f64).sqrt().ceil() as usize).max(1);
        let rows = count.div_ceil(cols);
        for (i, name) in names.into_iter().enumerate() {
            let (col, row) = (i % cols, i / cols);
            let x = ((col as f64 + 0.5) * w as f64 / cols as f64) as usize;
            let y = ((row as f64 + 0.5) * h as f64 / rows as f64) as usize;
            let (x, y) = world.nearest_land(x.min(w - 1), y.min(h - 1));
            let mut agent = Agent::new(name, x, y, config.agents.starting_food);
            agent.physical.age = starting_age;
            agents.push(agent);
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
            recipe_registry: RecipeRegistry::new(),
            trade_state: TradeState::new(),
            mate_proposals: HashMap::new(),
            weather: WeatherState::default(),
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

    /// Get trade state as view
    pub fn trade_views(&self) -> TradeStateView {
        let epoch = self.world.epoch;

        // Build proposal views
        let pending_proposals: Vec<TradeProposalView> = self
            .trade_state
            .proposals
            .values()
            .filter(|p| p.status == ProposalStatus::Pending)
            .filter_map(|p| {
                let proposer_name = self.agents.iter()
                    .find(|a| a.id == p.proposer)
                    .map(|a| a.name().to_string())?;
                let recipient_name = self.agents.iter()
                    .find(|a| a.id == p.recipient)
                    .map(|a| a.name().to_string())?;

                Some(TradeProposalView {
                    id: p.id,
                    proposer_name,
                    recipient_name,
                    offering: p.offering_description(),
                    requesting: p.requesting_description(),
                    expires_in: p.expires_epoch.saturating_sub(epoch),
                    status: format!("{:?}", p.status),
                })
            })
            .collect();

        // Build service debt views
        let service_debts: Vec<ServiceDebtView> = self
            .trade_state
            .service_debts
            .iter()
            .filter(|d| !d.fulfilled && !d.reneged)
            .filter_map(|d| {
                let debtor_name = self.agents.iter()
                    .find(|a| a.id == d.debtor)
                    .map(|a| a.name().to_string())?;
                let creditor_name = self.agents.iter()
                    .find(|a| a.id == d.creditor)
                    .map(|a| a.name().to_string())?;

                let deadline_in = d.deadline_epoch.map(|dl| dl as i64 - epoch as i64);
                let is_alliance = matches!(d.service, ServiceType::Alliance { .. });

                Some(ServiceDebtView {
                    debtor_name,
                    creditor_name,
                    service: d.service.describe(),
                    deadline_in,
                    fulfilled: d.fulfilled,
                    is_alliance,
                })
            })
            .collect();

        TradeStateView {
            pending_proposals,
            service_debts,
        }
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
        self.conditions(self.world.epoch)
    }

    /// The season's conditions with today's weather applied
    fn conditions(&self, epoch: usize) -> EnvironmentState {
        let mut state = self.environment.state_at(epoch);
        self.weather.apply(&self.environment.weather, &mut state);
        state
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

        // Log epoch start
        self.log_and_track(Event::epoch_start(epoch))?;

        // Weather turns, unpredictably, on top of the season
        if let Some(weather) = self.weather.advance(&self.environment.weather, &mut rand::rng()) {
            debug!("Weather: {}", weather.describe());
            self.log_and_track(Event::weather_changed(epoch, weather.describe()))?;
        }
        let env_state = self.conditions(epoch);
        if env_state.weather == Weather::Storm && self.environment.weather.storm_damage > 0 {
            self.storm_damage(epoch)?;
        }

        // 1. World tick (regenerate resources with environmental modifier)
        self.world.tick(self.config.world.food_regen_rate, env_state.food_regen_modifier, env_state.food_decay);

        // 1b. Structure production (farms produce food)
        self.process_structure_production(epoch)?;

        // 2. Update agent needs (with environmental effects)
        let mut death_events = Vec::new();
        let spoilage = self.config.agents.food_spoilage;
        let mut rng = rand::rng();
        for agent in &mut self.agents {
            if agent.is_alive() {
                agent.tick_hunger();
                agent.tick_energy();

                // Carried food spoils; food in storage keeps
                let spoiled = crate::world::random_round(agent.physical.food as f64 * spoilage, &mut rng);
                agent.physical.food -= spoiled.min(agent.physical.food);

                // Apply environmental hazard effects (reduced by shelter)
                if env_state.hazard_level > 0.0 {
                    // Calculate shelter protection
                    let shelter_protection = if let Some((sx, sy)) = agent.physical.sheltered_at {
                        self.world.get(sx, sy)
                            .and_then(|c| c.structure.as_ref())
                            .map(|s| s.effective_protection())
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    };

                    let effective_hazard = env_state.hazard_level * (1.0 - shelter_protection);

                    // Extra energy drain from harsh environment
                    let extra_drain = env_state.energy_drain * effective_hazard;
                    agent.physical.energy = (agent.physical.energy - extra_drain).max(0.0);

                    // High hazard can cause health damage (only if effective hazard > 0.5)
                    if effective_hazard > 0.5 {
                        let health_damage = (effective_hazard - 0.5) * 0.02;
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

        // 2b. Illness spreads between people who are close
        self.tick_disease(epoch)?;

        // 3. Perception and deliberation (collect actions)
        let mut actions: HashMap<Uuid, Action> = HashMap::new();

        // Build environment perception
        let env_perception = self.environment.describe(&env_state);

        for agent in &self.agents {
            if !agent.is_alive() {
                continue;
            }

            // Get perception (world + environment)
            let radius = self.world.sight_from(agent.physical.x, agent.physical.y, self.config.agents.view_radius);
            let mut world_perception = self.world.perception_summary(agent.physical.x, agent.physical.y, radius);
            // People are visible from afar; talking, giving or fighting still needs adjacency
            let distant: Vec<String> = self
                .agents
                .iter()
                .filter(|a| a.is_alive() && !is_adjacent(a, agent))
                .filter(|a| a.physical.x.abs_diff(agent.physical.x).max(a.physical.y.abs_diff(agent.physical.y)) <= radius)
                .map(|a| {
                    let (ax, ay) = (a.physical.x, a.physical.y);
                    let d = ax.abs_diff(agent.physical.x).max(ay.abs_diff(agent.physical.y));
                    format!("{} ({} {})", a.name(), d, crate::world::compass(agent.physical.x, agent.physical.y, ax, ay))
                })
                .collect();
            if !distant.is_empty() {
                world_perception.push_str(&format!("\nIn the distance: {}", distant.join(", ")));
            }
            let perception = format!("{}\n{}", env_perception, world_perception);
            let local_food = self.world.local_food(agent.physical.x, agent.physical.y);
            let action_context = self.action_context(agent);

            // Get nearby agents
            let nearby: Vec<(Uuid, &str)> = self
                .agents
                .iter()
                .filter(|a| a.is_alive() && a.id != agent.id && is_adjacent(agent, a))
                .map(|a| (a.id, a.name()))
                .collect();

            // Get pending trade proposals for this agent (offers from others)
            let pending_trades: Vec<(usize, Uuid, &str, String, String, Option<usize>)> = self
                .trade_state
                .pending_proposals_for(agent.id)
                .into_iter()
                .enumerate()
                .filter_map(|(idx, proposal)| {
                    // Find proposer name
                    let proposer_name = self.agents.iter()
                        .find(|a| a.id == proposal.proposer)
                        .map(|a| a.name())?;
                    Some((
                        idx,
                        proposal.proposer,
                        proposer_name,
                        proposal.offering_description(),
                        proposal.requesting_description(),
                        Some(proposal.expires_epoch),
                    ))
                })
                .collect();

            // Get unfulfilled service debts this agent owes (their obligations)
            let debts_owed: Vec<(Uuid, &str, String, Option<usize>)> = self
                .trade_state
                .service_debts
                .iter()
                .filter(|d| d.debtor == agent.id && !d.fulfilled && !d.reneged)
                .filter_map(|d| {
                    let creditor_name = self.agents.iter()
                        .find(|a| a.id == d.creditor)
                        .map(|a| a.name())?;
                    Some((d.creditor, creditor_name, d.service.describe(), d.deadline_epoch))
                })
                .collect();

            // Get unfulfilled service debts owed TO this agent (credits)
            let credits_owed: Vec<(Uuid, &str, String, Option<usize>)> = self
                .trade_state
                .service_debts
                .iter()
                .filter(|d| d.creditor == agent.id && !d.fulfilled && !d.reneged)
                .filter_map(|d| {
                    let debtor_name = self.agents.iter()
                        .find(|a| a.id == d.debtor)
                        .map(|a| a.name())?;
                    Some((d.debtor, debtor_name, d.service.describe(), d.deadline_epoch))
                })
                .collect();

            // Nearby agents with an open mating proposal to this agent
            let mate_proposals: Vec<(Uuid, &str)> = self
                .mate_proposals
                .iter()
                .filter(|(_, (target, expires))| *target == agent.id && *expires >= epoch)
                .filter_map(|(proposer, _)| nearby.iter().find(|(id, _)| id == proposer).copied())
                .collect();

            // Count this agent's pending proposals (to show in actions prompt)
            let my_proposals = self
                .trade_state
                .proposals
                .values()
                .filter(|p| p.proposer == agent.id && p.status == ProposalStatus::Pending)
                .count();

            // Get action from LLM
            let action = self
                .llm
                .decide_action(
                    agent,
                    &perception,
                    &local_food,
                    &action_context,
                    &nearby,
                    epoch,
                    &pending_trades,
                    &debts_owed,
                    &credits_owed,
                    my_proposals,
                    &mate_proposals,
                )
                .await?;

            debug!("Agent {} chooses: {:?}", agent.name(), action);
            actions.insert(agent.id, action);
        }

        // 4. Resolve actions (simultaneous)
        self.resolve_actions(epoch, actions.clone())?;

        // 4b. Trade maintenance (expiry, deadline checking)
        self.expire_trade_proposals(epoch)?;
        self.check_service_deadlines(epoch)?;

        // 5. Resolve mating (requires mutual consent check)
        self.resolve_mating(epoch, &actions)?;

        // 6. Tick reproduction systems
        self.tick_gestations(epoch)?;
        self.tick_courtship_decay();
        self.tick_skill_decay(epoch);
        self.process_births();

        // 7. Tick aging (after reproduction so newborns get their first epoch)
        self.tick_aging(epoch)?;

        // 8. Update beliefs based on what happened
        self.update_beliefs(epoch);

        // 9. Update territories (decay, group sharing)
        self.update_territories(epoch)?;

        // 10. Structure decay
        self.decay_structures(epoch)?;

        // 11. Detect groups/alliances
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
        let env_state = self.conditions(epoch);
        let base_movement_cost = 0.05 * env_state.movement_cost;
        // Game and fish follow the seasons like plants do (scarce in winter)
        let game_availability = env_state.food_regen_modifier.min(1.0);
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

        // Second pass: resolve actions. Everyone acts from where they stood at the start of the
        // day, so leaving (moving or backing down) happens after every interaction: walking away
        // doesn't dodge an attack, gift or theft made the same day
        let mut ordered: Vec<(Uuid, Action)> = actions.into_iter().collect();
        ordered.sort_by_key(|(_, action)| matches!(action, Action::Move(_) | Action::Submit));
        for (agent_id, action) in ordered {
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

                    // Nobody walks into a lake
                    if self.world.walkable(new_x, new_y) {
                        let from = (agent.physical.x, agent.physical.y);
                        agent.physical.x = new_x;
                        agent.physical.y = new_y;
                        // Walking away leaves the shelter (protection is tied to being there)
                        agent.physical.leave_shelter();
                        // Movement cost affected by terrain (climbing, forest), environment and age
                        let movement_cost = base_movement_cost * self.world.step_cost(from, (new_x, new_y)) / age_mod;
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

                    // Territory is a belief, not a wall: anyone can gather here. An owner who is
                    // nearby sees it happen, and what they do about it is their choice
                    let watching_owner = self
                        .world
                        .get(pos.0, pos.1)
                        .and_then(|c| c.territory.as_ref())
                        .filter(|t| t.owner != agent_id && !t.allowed_guests.contains(&agent_id))
                        .and_then(|t| self.agents.iter().position(|a| a.id == t.owner && a.is_alive()))
                        .filter(|&o| is_adjacent(&self.agents[o], agent));

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

                    if taken == 0 {
                        // Log failed attempts too, so the event stream reflects every action
                        self.log_and_track(Event::gathered(epoch, agent_id, 0))?;
                    } else {
                        self.agents[agent_idx].add_food(taken);
                        // Gathering energy cost affected by age (elderly use more energy)
                        let gather_cost = 0.1 / age_mod;
                        self.agents[agent_idx].physical.energy =
                            (self.agents[agent_idx].physical.energy - gather_cost).max(0.0);

                        // Practice foraging skill when gathering
                        self.agents[agent_idx].skills.practice("foraging", self.config.skills.practice_gain(), epoch);

                        self.log_and_track(Event::gathered(epoch, agent_id, taken))?;

                        if let Some(o) = watching_owner {
                            let name = self.agents[agent_idx].name().to_string();
                            self.agents[o].memory.remember(Episode::conflict(
                                epoch,
                                &format!("I saw {} take {} food from my land", name, taken),
                                -0.4,
                                agent_id,
                            ));
                            self.agents[o].beliefs.update_trust(agent_id, &name, -0.1, epoch);
                        }

                        // Update belief about this location
                        self.agents[agent_idx]
                            .beliefs
                            .update_food_belief(pos.0, pos.1, remaining_food, epoch);
                    }
                }

                Action::Eat => {
                    let eaten = self.agents[agent_idx].eat();
                    if eaten > 0 {
                        self.log_and_track(Event::ate(epoch, agent_id, eaten))?;

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
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Shelter rest bonus
                    let shelter_bonus = if let Some((sx, sy)) = agent.physical.sheltered_at {
                        if let Some(cell) = self.world.get(sx, sy) {
                            if let Some(ref structure) = cell.structure {
                                structure.effective_rest_bonus()
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };

                    // Territory rest bonus (resting on own territory feels safer)
                    let territory_bonus = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        if let Some(ref territory) = cell.territory {
                            if territory.owner == agent_id || territory.allowed_guests.contains(&agent_id) {
                                0.1 // Bonus for resting on owned/friendly territory
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };

                    let recovery = (0.5 + shelter_bonus + territory_bonus) * age_mod;
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
                            self.agents[agent_idx].skills.practice("leadership", self.config.skills.practice_gain(), epoch);

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

                                self.agents[agent_idx].beliefs.get_or_create_social(target, &target_name).food_given += actual;
                                self.agents[target_idx].beliefs.get_or_create_social(agent_id, &agent_name).food_received += actual;

                                // Practice leadership when giving
                                self.agents[agent_idx].skills.practice("leadership", self.config.skills.practice_gain(), epoch);

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

                                // Check if this contributes to a FutureGift debt
                                self.check_give_fulfills_debt(agent_id, target, actual, epoch);
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
                            // Allies don't step in automatically: bystanders see the attack and
                            // choose for themselves whether to help, retaliate or stay out of it
                            let damage = 0.15 + rand::random::<f64>() * 0.1;
                            self.agents[target_idx].take_damage(damage);

                            // Violence is the arbiter of property: the attacker seizes food in
                            // proportion to how hard the blow landed; a killing blow takes it all
                            let share = if self.agents[target_idx].is_alive() { (damage * 2.0).min(1.0) } else { 1.0 };
                            let target_food = self.agents[target_idx].physical.food;
                            let seized = self.agents[target_idx].remove_food((target_food as f64 * share).round() as u32);
                            self.agents[agent_idx].add_food(seized);

                            self.log_and_track(Event::attacked(
                                epoch,
                                agent_id,
                                target,
                                damage,
                                seized,
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
                            let (attacker_memory, victim_memory) = if seized > 0 {
                                (
                                    format!("I attacked {} and took {} food", target_name, seized),
                                    format!("{} attacked me and took {} of my food!", agent_name, seized),
                                )
                            } else {
                                (format!("I attacked {}", target_name), format!("{} attacked me!", agent_name))
                            };
                            self.agents[agent_idx].memory.remember(Episode::conflict(
                                epoch,
                                &attacker_memory,
                                -0.2,
                                target,
                            ));

                            self.agents[target_idx].memory.remember(Episode::conflict(
                                epoch,
                                &victim_memory,
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

                            let bystanders: Vec<usize> = (0..self.agents.len())
                                .filter(|&i| i != agent_idx && i != target_idx && self.agents[i].is_alive())
                                .filter(|&i| is_adjacent(&self.agents[i], &self.agents[target_idx]))
                                .collect();
                            for i in bystanders {
                                self.agents[i].memory.remember(Episode::conflict(
                                    epoch,
                                    &format!("I saw {} attack {}", agent_name, target_name),
                                    -0.4,
                                    agent_id,
                                ));
                                self.agents[i].beliefs.update_trust(agent_id, &agent_name, -0.1, epoch);
                            }
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
                            self.agents[agent_idx].skills.practice("diplomacy", self.config.skills.practice_gain(), epoch);

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
                                self.agents[agent_idx].skills.practice("teaching", self.config.skills.practice_gain(), epoch);

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

                                // Check if this fulfills a TeachSkill debt
                                self.check_teach_fulfills_debt(agent_id, target, &skill, epoch);

                                debug!(
                                    "{} taught {} to {} (now at {:.2})",
                                    agent_name, skill, target_name, new_level
                                );
                            }
                        }
                    }
                }

                Action::GatherMaterials => {
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Get terrain at current position
                    if let Some(cell) = self.world.get(pos.0, pos.1) {
                        let terrain = cell.terrain;
                        let foraging_skill = agent.skills.level("foraging");
                        let tool_bonus = agent.physical.tool_bonus_for_skill("foraging");

                        // Base materials based on terrain
                        let mut gathered: Vec<(MaterialType, u32)> = Vec::new();

                        // What the land here still holds limits what can be taken
                        let (wood_stock, stone_stock) = (cell.wood, cell.stone);
                        match terrain {
                            Terrain::Fertile => {
                                // Wood and fiber from the stand on fertile terrain
                                let wood_amount = ((1.0 + foraging_skill * 2.0 + tool_bonus).round() as u32).min(wood_stock);
                                let fiber_amount = ((1.0 + foraging_skill * 2.0).round() as u32).min(wood_stock - wood_amount);
                                gathered.push((MaterialType::Wood, wood_amount));
                                gathered.push((MaterialType::Fiber, fiber_amount));
                            }
                            Terrain::Water => {}
                            Terrain::Barren => {
                                // Stone and occasionally flint from barren terrain
                                let stone_amount = ((2.0 + foraging_skill).round() as u32).min(stone_stock);
                                gathered.push((MaterialType::Stone, stone_amount));

                                // 20% chance for flint while there is stone to work
                                if stone_stock > stone_amount && rand::random::<f64>() < 0.2 + foraging_skill * 0.1 {
                                    gathered.push((MaterialType::Flint, 1));
                                }
                            }
                        }
                        gathered.retain(|(_, amount)| *amount > 0);
                        let taken: u32 = gathered.iter().map(|(_, a)| a).sum();
                        if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                            match terrain {
                                Terrain::Fertile => cell.wood -= taken,
                                Terrain::Barren => cell.stone -= taken,
                                Terrain::Water => {}
                            }
                        }

                        // Add materials to inventory
                        for (mat_type, amount) in &gathered {
                            self.agents[agent_idx].physical.add_material(*mat_type, *amount);
                        }

                        // Practice foraging
                        self.agents[agent_idx].skills.practice("foraging", self.config.skills.practice_gain(), epoch);

                        // Energy cost
                        self.agents[agent_idx].physical.energy =
                            (self.agents[agent_idx].physical.energy - 0.15).max(0.0);

                        // Log event
                        self.log_and_track(Event::gathered_materials(
                            epoch,
                            agent_id,
                            gathered.iter().map(|(m, a)| (m.display_name().to_string(), *a)).collect(),
                        ))?;

                        debug!(
                            "{} gathered materials: {:?}",
                            self.agents[agent_idx].name(),
                            gathered
                        );
                    }
                }

                Action::Craft { tool } => {
                    let agent = &self.agents[agent_idx];
                    let crafting_skill = agent.skills.level("crafting");

                    // Check if we have the recipe
                    if let Some(recipe) = self.recipe_registry.get(&tool) {
                        // Check skill requirement
                        if crafting_skill < recipe.min_crafting_skill {
                            continue;
                        }

                        // Check material requirements
                        let mut can_craft = true;
                        for (mat_type, amount) in &recipe.ingredients {
                            if agent.physical.material_count(*mat_type) < *amount {
                                can_craft = false;
                                break;
                            }
                        }

                        // Check tool requirement
                        if let Some(required_tool) = recipe.required_tool {
                            if !agent.physical.has_tool(required_tool) {
                                can_craft = false;
                            }
                        }

                        if can_craft {
                            // Consume materials
                            for (mat_type, amount) in &recipe.ingredients {
                                self.agents[agent_idx].physical.remove_material(*mat_type, *amount);
                            }

                            // Determine quality based on crafting skill
                            let quality = ToolQuality::from_skill(crafting_skill);

                            // Create the tool
                            let new_tool = Tool::new(tool, quality, Some(agent_id), epoch);
                            let tool_name = new_tool.display_name();
                            self.agents[agent_idx].physical.tools.push(new_tool);

                            // Practice crafting
                            self.agents[agent_idx].skills.practice("crafting", self.config.skills.practice_gain(), epoch);
                            let improvement = 0.02 + recipe.min_crafting_skill * 0.05;
                            self.agents[agent_idx].skills.improve("crafting", improvement, epoch);

                            // Energy cost
                            self.agents[agent_idx].physical.energy =
                                (self.agents[agent_idx].physical.energy - 0.2).max(0.0);

                            // Log event
                            self.log_and_track(Event::crafted(
                                epoch,
                                agent_id,
                                tool.display_name(),
                                quality.name(),
                            ))?;

                            // Memory
                            self.agents[agent_idx].memory.remember(Episode::survival(
                                epoch,
                                &format!("I crafted a {}", tool_name),
                                0.3,
                            ));

                            debug!(
                                "{} crafted a {} {}",
                                self.agents[agent_idx].name(),
                                quality.name(),
                                tool.display_name()
                            );
                        }
                    }
                }

                Action::Hunt => {
                    let agent = &self.agents[agent_idx];

                    // Check for hunting weapon
                    let has_weapon = agent.physical.has_tool(ToolType::WoodenSpear)
                        || agent.physical.has_tool(ToolType::Bow);

                    if !has_weapon {
                        continue;
                    }

                    let hunting_skill = agent.skills.level("hunting");
                    let tool_bonus = agent.physical.tool_bonus_for_skill("hunting");
                    let pos = (agent.physical.x, agent.physical.y);

                    // Finding prey depends on how many animals range nearby: encounters rise with
                    // density and saturate (Holling's functional response); skill and weapon
                    // decide whether an encounter becomes a kill
                    let game = self.world.game_around(pos.0, pos.1);
                    let encounter = 1.0 - (-HUNT_SEARCH_EFFICIENCY * game).exp();
                    let success_chance = encounter * (0.4 + hunting_skill * 0.3 + tool_bonus * 0.2).min(1.0);

                    if game >= 1.0 && rand::random::<f64>() < success_chance {
                        self.world.take_game(pos.0, pos.1);
                        // Successful hunt!
                        let food_gained = (3.0 + hunting_skill * 4.0 + tool_bonus * 2.0).round() as u32;
                        self.agents[agent_idx].add_food(food_gained);

                        // Chance to get hide and bone
                        if rand::random::<f64>() < 0.7 {
                            self.agents[agent_idx].physical.add_material(MaterialType::Hide, 1);
                        }
                        if rand::random::<f64>() < 0.5 {
                            self.agents[agent_idx].physical.add_material(MaterialType::Bone, 1);
                        }

                        // Practice hunting
                        self.agents[agent_idx].skills.practice("hunting", self.config.skills.practice_gain(), epoch);
                        self.agents[agent_idx].skills.improve("hunting", 0.03, epoch);

                        // Use tool durability
                        self.agents[agent_idx].physical.use_tool_for_action("hunt");

                        // Log event
                        self.log_and_track(Event::hunted(epoch, agent_id, food_gained, true))?;

                        self.agents[agent_idx].memory.remember(Episode::survival(
                            epoch,
                            &format!("I hunted successfully and got {} food", food_gained),
                            0.4,
                        ));
                    } else {
                        // Failed hunt
                        self.agents[agent_idx].skills.practice("hunting", self.config.skills.practice_gain(), epoch);

                        // Still uses energy and tool durability
                        self.agents[agent_idx].physical.use_tool_for_action("hunt");

                        self.log_and_track(Event::hunted(epoch, agent_id, 0, false))?;
                    }

                    // Energy cost
                    self.agents[agent_idx].physical.energy =
                        (self.agents[agent_idx].physical.energy - 0.25).max(0.0);
                }

                Action::Fish => {
                    let agent = &self.agents[agent_idx];

                    // Check for fishing pole
                    if !agent.physical.has_tool(ToolType::FishingPole) {
                        continue;
                    }

                    let foraging_skill = agent.skills.level("foraging");
                    let tool_bonus = agent.physical.tool_bonus_for_skill("foraging");

                    // Fishing needs water next to the angler; a bite is likelier the more fish are
                    // in reach, saturating like a hunter's encounters
                    let pos = (agent.physical.x, agent.physical.y);
                    let fish = self.world.fish_around(pos.0, pos.1);
                    if fish == 0.0 && !self.world.adjacent(pos.0, pos.1).iter().any(|c| c.terrain == Terrain::Water) {
                        debug!("{} has no water to fish in", agent.name());
                        continue;
                    }
                    let bite = 1.0 - (-FISH_SEARCH_EFFICIENCY * fish).exp();
                    let success_chance = bite * (0.5 + foraging_skill * 0.25 + tool_bonus * 0.15).min(1.0);

                    let wanted = (2.0 + foraging_skill * 3.0 + tool_bonus).round() as u32;
                    let food_gained = if rand::random::<f64>() < success_chance {
                        self.world.take_fish(pos.0, pos.1, wanted)
                    } else {
                        0
                    };
                    if food_gained > 0 {
                        // Successful fishing!
                        self.agents[agent_idx].add_food(food_gained);

                        // Practice foraging
                        self.agents[agent_idx].skills.practice("foraging", self.config.skills.practice_gain(), epoch);

                        // Use tool durability
                        self.agents[agent_idx].physical.use_tool_for_action("fish");

                        self.log_and_track(Event::fished(epoch, agent_id, food_gained, true))?;

                        self.agents[agent_idx].memory.remember(Episode::survival(
                            epoch,
                            &format!("I caught {} fish", food_gained),
                            0.3,
                        ));
                    } else {
                        // Failed to catch anything
                        self.agents[agent_idx].physical.use_tool_for_action("fish");
                        self.log_and_track(Event::fished(epoch, agent_id, 0, false))?;
                    }

                    // Energy cost (fishing is less tiring)
                    self.agents[agent_idx].physical.energy =
                        (self.agents[agent_idx].physical.energy - 0.1).max(0.0);
                }

                Action::Chop => {
                    let agent = &self.agents[agent_idx];

                    // Check for axe
                    let has_axe = agent.physical.has_tool(ToolType::StoneAxe)
                        || agent.physical.has_tool(ToolType::FlintAxe);

                    if !has_axe {
                        continue;
                    }

                    let foraging_skill = agent.skills.level("foraging");
                    let tool_bonus = agent.physical.tool_bonus_for_skill("foraging");
                    let pos = (agent.physical.x, agent.physical.y);

                    // Chopping is efficient wood gathering, from the stand that grows here
                    let wanted = (3.0 + foraging_skill * 3.0 + tool_bonus * 2.0).round() as u32;
                    let wood_amount = match self.world.get_mut(pos.0, pos.1) {
                        Some(cell) => {
                            let taken = wanted.min(cell.wood);
                            cell.wood -= taken;
                            taken
                        }
                        None => 0,
                    };
                    if wood_amount == 0 {
                        debug!("{} found no wood to chop", self.agents[agent_idx].name());
                        continue;
                    }
                    self.agents[agent_idx].physical.add_material(MaterialType::Wood, wood_amount);

                    // Practice foraging
                    self.agents[agent_idx].skills.practice("foraging", self.config.skills.practice_gain(), epoch);

                    // Use tool durability
                    self.agents[agent_idx].physical.use_tool_for_action("chop");

                    // Energy cost
                    self.agents[agent_idx].physical.energy =
                        (self.agents[agent_idx].physical.energy - 0.15).max(0.0);

                    self.log_and_track(Event::chopped(epoch, agent_id, wood_amount))?;

                    debug!(
                        "{} chopped {} wood",
                        self.agents[agent_idx].name(),
                        wood_amount
                    );
                }

                // ==================== Structure Actions ====================

                Action::Build { structure_type } => {
                    use crate::structures::{Structure, StructureRecipeRegistry};

                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);
                    let registry = StructureRecipeRegistry::new();

                    // Get recipe for this structure type
                    let recipe = match registry.get(structure_type) {
                        Some(r) => r,
                        None => continue,
                    };

                    // Check terrain requirements
                    let cell_terrain = self.world.get(pos.0, pos.1).map(|c| c.terrain);
                    if let Some(terrain) = cell_terrain {
                        if !recipe.valid_terrain(terrain) {
                            debug!("{} cannot build {} on this terrain", self.agents[agent_idx].name(), structure_type.display_name());
                            continue;
                        }
                    }

                    // Check if there's already a structure at this location
                    let existing_structure = self.world.get(pos.0, pos.1).and_then(|c| c.structure.as_ref());

                    if let Some(structure) = existing_structure {
                        // Continue building an existing structure
                        if structure.is_complete() {
                            debug!("{} - structure already complete", self.agents[agent_idx].name());
                            continue;
                        }

                        // Track owner before we lose reference
                        let structure_owner = structure.owner;

                        // Add progress
                        let crafting_skill = self.agents[agent_idx].skills.level("crafting");
                        let progress = build_progress(crafting_skill);

                        let mut completed = None;
                        if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                            if let Some(ref mut s) = cell.structure {
                                let was_complete = s.is_complete();
                                s.add_progress(progress, epoch);

                                // Practice crafting
                                self.agents[agent_idx].skills.practice("crafting", self.config.skills.practice_gain(), epoch);
                                self.agents[agent_idx].physical.energy =
                                    (self.agents[agent_idx].physical.energy - 0.15).max(0.0);

                                if !was_complete && s.is_complete() {
                                    completed = Some(s.structure_type.display_name());
                                    debug!("{} completed building {}", self.agents[agent_idx].name(), s.display_name());
                                    self.agents[agent_idx].memory.remember(Episode::survival(
                                        epoch,
                                        &format!("I completed building a {}", s.structure_type.display_name()),
                                        0.5,
                                    ));
                                }
                            }
                        }

                        if let Some(name) = completed {
                            self.log_and_track(Event::structure_completed(epoch, structure_owner, pos.0, pos.1, name))?;
                        }

                        // If helping someone else's structure, check for HelpBuild debt fulfillment
                        if structure_owner != agent_id {
                            self.check_build_fulfills_debt(agent_id, structure_owner, progress, epoch);
                        }
                    } else {
                        // Start a new structure - check if agent can afford materials
                        let can_afford = recipe.can_afford(&self.agents[agent_idx].physical.materials);
                        if !can_afford {
                            debug!("{} cannot afford to build {}", self.agents[agent_idx].name(), structure_type.display_name());
                            continue;
                        }

                        // Check tool requirement
                        if let Some(tool_type) = recipe.required_tool {
                            if !self.agents[agent_idx].physical.has_tool(tool_type) {
                                debug!("{} needs {} to build {}", self.agents[agent_idx].name(), tool_type.display_name(), structure_type.display_name());
                                continue;
                            }
                        }

                        // Consume materials
                        for (material, amount) in &recipe.materials {
                            self.agents[agent_idx].physical.remove_material(*material, *amount);
                        }

                        // Create the structure
                        let crafting_skill = self.agents[agent_idx].skills.level("crafting");
                        let quality = ToolQuality::from_skill(crafting_skill);
                        let new_structure = Structure::new(
                            structure_type,
                            agent_id,
                            recipe.build_required,
                            quality,
                            epoch,
                        );

                        // Add initial progress
                        let progress = build_progress(crafting_skill);
                        if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                            let mut s = new_structure;
                            s.add_progress(progress, epoch);
                            cell.structure = Some(s);
                        }

                        // Practice crafting
                        self.agents[agent_idx].skills.practice("crafting", self.config.skills.practice_gain(), epoch);
                        self.agents[agent_idx].physical.energy =
                            (self.agents[agent_idx].physical.energy - 0.2).max(0.0);

                        debug!("{} started building a {}", self.agents[agent_idx].name(), structure_type.display_name());

                        self.agents[agent_idx].memory.remember(Episode::survival(
                            epoch,
                            &format!("I started building a {}", structure_type.display_name()),
                            0.3,
                        ));
                    }
                }

                Action::EnterShelter => {
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Check if there's a shelter at this location that the agent can use
                    let can_enter = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        if let Some(ref structure) = cell.structure {
                            structure.structure_type.is_shelter() && structure.can_use(agent_id)
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if can_enter {
                        self.agents[agent_idx].physical.enter_shelter(pos.0, pos.1);
                        debug!("{} entered shelter at {:?}", self.agents[agent_idx].name(), pos);
                    }
                }

                Action::LeaveShelter => {
                    if self.agents[agent_idx].physical.is_sheltered() {
                        self.agents[agent_idx].physical.leave_shelter();
                        debug!("{} left shelter", self.agents[agent_idx].name());
                    }
                }

                Action::Deposit { material, amount } => {
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Check for accessible storage
                    let can_deposit = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        if let Some(ref structure) = cell.structure {
                            structure.structure_type.has_storage()
                                && structure.is_complete()
                                && structure.can_use(agent_id)
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if can_deposit {
                        // Remove from agent inventory
                        let actual = self.agents[agent_idx].physical.remove_material(material, amount);

                        if actual > 0 {
                            // Add to storage
                            if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                                if let Some(ref mut structure) = cell.structure {
                                    if let Some(ref mut inv) = structure.inventory {
                                        let _overflow = inv.add_material(material, actual);
                                    }
                                }
                            }
                            debug!("{} deposited {} {}", self.agents[agent_idx].name(), actual, material.display_name());
                        }
                    }
                }

                Action::Withdraw { material, amount } => {
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Check for accessible storage
                    let can_withdraw = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        if let Some(ref structure) = cell.structure {
                            structure.structure_type.has_storage()
                                && structure.is_complete()
                                && structure.can_use(agent_id)
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if can_withdraw {
                        // Remove from storage
                        let mut withdrawn = 0;
                        if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                            if let Some(ref mut structure) = cell.structure {
                                if let Some(ref mut inv) = structure.inventory {
                                    withdrawn = inv.remove_material(material, amount);
                                }
                            }
                        }

                        if withdrawn > 0 {
                            // Add to agent inventory
                            self.agents[agent_idx].physical.add_material(material, withdrawn);
                            debug!("{} withdrew {} {}", self.agents[agent_idx].name(), withdrawn, material.display_name());
                        }
                    }
                }

                Action::Permit { target } => {
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Check if agent owns a structure at this location
                    let is_owner = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        if let Some(ref structure) = cell.structure {
                            structure.owner == agent_id
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if is_owner {
                        if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                            if let Some(ref mut structure) = cell.structure {
                                structure.permit(target);
                                debug!("{} permitted access to structure", self.agents[agent_idx].name());

                                // Update trust between agents
                                if let Some(target_idx) = self.agents.iter().position(|a| a.id == target) {
                                    let agent_name = self.agents[agent_idx].name().to_string();
                                    self.agents[target_idx].beliefs.update_trust(agent_id, &agent_name, 0.2, epoch);
                                }
                            }
                        }
                    }
                }

                Action::Deny { target } => {
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Check if agent owns a structure at this location
                    let is_owner = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        if let Some(ref structure) = cell.structure {
                            structure.owner == agent_id
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if is_owner {
                        if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                            if let Some(ref mut structure) = cell.structure {
                                structure.deny(target);
                                debug!("{} denied access to structure", self.agents[agent_idx].name());
                            }
                        }
                    }
                }

                // ==================== Territory Actions ====================

                Action::DepositFood { amount } | Action::WithdrawFood { amount } => {
                    let depositing = matches!(action, Action::DepositFood { .. });
                    let pos = (self.agents[agent_idx].physical.x, self.agents[agent_idx].physical.y);
                    let carried = self.agents[agent_idx].physical.food;
                    let mut moved = 0;
                    if let Some(inv) = self
                        .world
                        .get_mut(pos.0, pos.1)
                        .and_then(|c| c.structure.as_mut())
                        .filter(|s| s.structure_type.has_storage() && s.can_use(agent_id))
                        .and_then(|s| s.inventory.as_mut())
                    {
                        moved = if depositing {
                            let offered = amount.min(carried);
                            offered - inv.add_food(offered)
                        } else {
                            inv.remove_food(amount)
                        };
                    }
                    if moved > 0 {
                        let agent = &mut self.agents[agent_idx];
                        if depositing {
                            agent.physical.food -= moved;
                        } else {
                            agent.physical.food += moved;
                        }
                        debug!("{} {} {} food", agent.name(), if depositing { "stored" } else { "took out" }, moved);
                    }
                }

                Action::Raid => {
                    let pos = (self.agents[agent_idx].physical.x, self.agents[agent_idx].physical.y);
                    let Some((owner, stored)) = self
                        .world
                        .get(pos.0, pos.1)
                        .and_then(|c| c.structure.as_ref())
                        .filter(|s| s.structure_type.has_storage() && s.is_complete() && !s.can_use(agent_id))
                        .map(|s| (s.owner, s.inventory.as_ref().map_or(0, |i| i.food)))
                    else {
                        continue;
                    };
                    if stored == 0 {
                        continue;
                    }
                    let raider_name = self.agents[agent_idx].name().to_string();
                    let owner_idx = self.agents.iter().position(|a| a.id == owner && a.is_alive());
                    let owner_present = owner_idx.is_some_and(|o| is_adjacent(&self.agents[agent_idx], &self.agents[o]));

                    // The engine never defends a store on the owner's behalf: an owner who is
                    // present sees the theft and decides what to do about it (e.g. attack to take it back)
                    let stolen = self
                        .world
                        .get_mut(pos.0, pos.1)
                        .and_then(|c| c.structure.as_mut())
                        .and_then(|s| s.inventory.as_mut())
                        .map_or(0, |inv| inv.remove_food(6));
                    self.agents[agent_idx].add_food(stolen);
                    self.log_and_track(Event::raided(epoch, agent_id, owner, stolen))?;
                    debug!("{} raided a storehouse and stole {} food", raider_name, stolen);

                    let owner_name = self.agent_name(owner);
                    self.agents[agent_idx].memory.remember(Episode::conflict(
                        epoch, &format!("I stole {} food from {}'s storehouse", stolen, owner_name), 0.2, owner,
                    ));
                    // The owner knows who did it if they saw it; otherwise only that food is gone.
                    // Anyone else nearby saw it too.
                    if let Some(o) = owner_idx {
                        if owner_present {
                            self.agents[o].memory.remember(Episode::conflict(
                                epoch, &format!("{} stole {} food from my storehouse!", raider_name, stolen), -0.8, agent_id,
                            ));
                            self.agents[o].beliefs.update_trust(agent_id, &raider_name, -0.5, epoch);
                            self.agents[o].beliefs.update_sentiment(agent_id, &raider_name, -0.5, epoch);
                        } else {
                            self.agents[o].memory.remember(Episode::survival(
                                epoch, &format!("Someone stole {} food from my storehouse", stolen), -0.6,
                            ));
                        }
                    }
                    let witnesses: Vec<usize> = (0..self.agents.len())
                        .filter(|&i| i != agent_idx && Some(i) != owner_idx)
                        .filter(|&i| self.agents[i].is_alive() && is_adjacent(&self.agents[i], &self.agents[agent_idx]))
                        .collect();
                    for w in witnesses {
                        self.agents[w].memory.remember(Episode::social(
                            epoch, &format!("I saw {} steal from {}'s storehouse", raider_name, owner_name), -0.3, agent_id,
                        ));
                        self.agents[w].beliefs.update_trust(agent_id, &raider_name, -0.2, epoch);
                    }
                }

                Action::Mark => {
                    use crate::world::TerritoryClaim;

                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Count how many territories this agent already owns (max 4)
                    let claim_count = self.count_agent_territories(agent_id);
                    if claim_count >= 4 {
                        debug!("{} already has max territories (4)", agent.name());
                        continue;
                    }

                    // Check if cell can be claimed and get old owner if overriding
                    let (can_claim, old_owner) = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        match &cell.territory {
                            None => (true, None),
                            Some(claim) if claim.owner == agent_id => (true, None),
                            Some(claim) if claim.strength < 0.3 => (true, Some(claim.owner)),
                            _ => (false, None),
                        }
                    } else {
                        (false, None)
                    };

                    if can_claim {
                        // First log the territory lost event if overriding
                        if let Some(old_owner_id) = old_owner {
                            self.log_and_track(Event::territory_lost(
                                epoch,
                                old_owner_id,
                                pos.0,
                                pos.1,
                            ))?;
                        }

                        // Now update the cell
                        if let Some(cell) = self.world.get_mut(pos.0, pos.1) {
                            cell.territory = Some(TerritoryClaim {
                                owner: agent_id,
                                allowed_guests: vec![],
                                claimed_epoch: epoch,
                                last_presence_epoch: epoch,
                                strength: 1.0,
                            });
                        }

                        self.log_and_track(Event::territory_marked(epoch, agent_id, pos.0, pos.1))?;
                        debug!("{} marked territory at ({}, {})", self.agents[agent_idx].name(), pos.0, pos.1);
                    }
                }

                Action::Challenge { target } => {
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Check if agent owns this territory
                    let is_owner = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        if let Some(ref claim) = cell.territory {
                            claim.owner == agent_id
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if !is_owner {
                        debug!("{} cannot challenge - not territory owner", agent.name());
                        continue;
                    }

                    // Check if target is present and not a guest
                    let target_idx = self.agents.iter().position(|a| a.id == target);
                    let is_trespasser = if let Some(t_idx) = target_idx {
                        let target_agent = &self.agents[t_idx];
                        let same_pos = target_agent.physical.x == pos.0 && target_agent.physical.y == pos.1;
                        let is_guest = if let Some(cell) = self.world.get(pos.0, pos.1) {
                            if let Some(ref claim) = cell.territory {
                                claim.allowed_guests.contains(&target)
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        same_pos && !is_guest && target_agent.is_alive()
                    } else {
                        false
                    };

                    if is_trespasser {
                        self.log_and_track(Event::territory_challenged(
                            epoch,
                            agent_id,
                            target,
                            pos.0,
                            pos.1,
                        ))?;
                        debug!("{} challenged {} for trespassing", self.agents[agent_idx].name(),
                            target_idx.map(|i| self.agents[i].name()).unwrap_or("unknown"));

                        // Update beliefs - trust penalty
                        if let Some(t_idx) = target_idx {
                            let agent_name = self.agents[agent_idx].name().to_string();
                            let target_name = self.agents[t_idx].name().to_string();
                            self.agents[agent_idx].beliefs.update_trust(target, &target_name, -0.1, epoch);
                            self.agents[t_idx].beliefs.update_trust(agent_id, &agent_name, -0.1, epoch);
                        }
                    }
                }

                Action::Submit => {
                    // Agent submits to a challenge and leaves territory
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Check if on someone else's territory
                    let territory_owner = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        cell.territory.as_ref().and_then(|claim| {
                            if claim.owner != agent_id && !claim.allowed_guests.contains(&agent_id) {
                                Some(claim.owner)
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };

                    if let Some(owner_id) = territory_owner {
                        // Move agent away (random adjacent cell)
                        let directions = [
                            Direction::North, Direction::South, Direction::East, Direction::West,
                            Direction::NorthEast, Direction::NorthWest, Direction::SouthEast, Direction::SouthWest,
                        ];
                        use rand::RngExt;
                        let mut rng = rand::rng();
                        let dir = directions[rng.random_range(0..8)];
                        let (dx, dy) = dir.delta();
                        let new_x = (pos.0 as i32 + dx).max(0) as usize;
                        let new_y = (pos.1 as i32 + dy).max(0) as usize;

                        if self.world.walkable(new_x, new_y) {
                            self.agents[agent_idx].physical.x = new_x;
                            self.agents[agent_idx].physical.y = new_y;
                        }

                        self.log_and_track(Event::territory_submitted(epoch, owner_id, agent_id))?;

                        // Update trust between agents (-0.2 mutual)
                        if let Some(owner_idx) = self.agents.iter().position(|a| a.id == owner_id) {
                            let agent_name = self.agents[agent_idx].name().to_string();
                            let owner_name = self.agents[owner_idx].name().to_string();
                            self.agents[agent_idx].beliefs.update_trust(owner_id, &owner_name, -0.2, epoch);
                            self.agents[owner_idx].beliefs.update_trust(agent_id, &agent_name, -0.2, epoch);
                        }

                        debug!("{} submitted and left territory", self.agents[agent_idx].name());
                    }
                }

                Action::Fight => {
                    // Agent fights back against territory owner
                    let agent = &self.agents[agent_idx];
                    let pos = (agent.physical.x, agent.physical.y);

                    // Check if on someone else's territory
                    let territory_info = if let Some(cell) = self.world.get(pos.0, pos.1) {
                        cell.territory.as_ref().and_then(|claim| {
                            if claim.owner != agent_id && !claim.allowed_guests.contains(&agent_id) {
                                Some((claim.owner, pos.0, pos.1))
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };

                    if let Some((owner_id, x, y)) = territory_info {
                        // Combat resolution
                        let owner_idx = self.agents.iter().position(|a| a.id == owner_id);
                        if let Some(o_idx) = owner_idx {
                            // Combat damage; the trespasser fights on unfamiliar ground
                            let base_damage = 0.15;
                            let trespasser_damage = base_damage * 1.2;
                            let owner_damage = base_damage;

                            self.agents[agent_idx].physical.health -= trespasser_damage;
                            self.agents[o_idx].physical.health -= owner_damage;

                            // Determine winner (whoever has more health remaining)
                            let winner = if self.agents[agent_idx].physical.health > self.agents[o_idx].physical.health {
                                agent_id
                            } else {
                                owner_id
                            };

                            self.log_and_track(Event::territory_fight(
                                epoch,
                                owner_id,
                                agent_id,
                                winner,
                                x,
                                y,
                            ))?;

                            // If trespasser wins, they claim the territory
                            if winner == agent_id {
                                if let Some(cell) = self.world.get_mut(x, y) {
                                    if let Some(ref mut claim) = cell.territory {
                                        claim.owner = agent_id;
                                        claim.strength = 0.8;
                                        claim.allowed_guests.clear();
                                    }
                                }
                                debug!("{} won territory fight and claimed territory", self.agents[agent_idx].name());
                            } else {
                                // Loser moves away
                                let directions = [
                                    Direction::North, Direction::South, Direction::East, Direction::West,
                                ];
                                use rand::RngExt;
                                let mut rng = rand::rng();
                                let dir = directions[rng.random_range(0..4)];
                                let (dx, dy) = dir.delta();
                                let new_x = (pos.0 as i32 + dx).max(0) as usize;
                                let new_y = (pos.1 as i32 + dy).max(0) as usize;

                                if self.world.walkable(new_x, new_y) {
                                    self.agents[agent_idx].physical.x = new_x;
                                    self.agents[agent_idx].physical.y = new_y;
                                }
                                debug!("{} lost territory fight", self.agents[agent_idx].name());
                            }

                            // Major trust damage
                            let agent_name = self.agents[agent_idx].name().to_string();
                            let owner_name = self.agents[o_idx].name().to_string();
                            self.agents[agent_idx].beliefs.update_trust(owner_id, &owner_name, -0.5, epoch);
                            self.agents[o_idx].beliefs.update_trust(agent_id, &agent_name, -0.5, epoch);
                        }
                    }
                }

                // Trade actions
                Action::TradeOffer { target, offering, requesting } => {
                    let trade_config = &self.config.trade;
                    if !trade_config.enabled {
                        continue;
                    }

                    // Check if target is nearby
                    let agent = &self.agents[agent_idx];
                    let target_idx = self.agents.iter().position(|a| a.id == target && a.is_alive());
                    if target_idx.is_none() {
                        continue;
                    }
                    let target_idx = target_idx.unwrap();

                    let agent_pos = (agent.physical.x, agent.physical.y);
                    let target_pos = (self.agents[target_idx].physical.x, self.agents[target_idx].physical.y);
                    if manhattan_distance(agent_pos, target_pos) > 1 {
                        continue;
                    }

                    // Check proposal limit
                    if self.trade_state.count_pending_from(agent_id) >= trade_config.max_pending_proposals {
                        debug!("{} has too many pending proposals", agent_id);
                        continue;
                    }

                    // Validate agent has the items they're offering (except promises)
                    if !self.agent_has_items(agent_idx, &offering) {
                        debug!("{} doesn't have items to offer", agent_id);
                        continue;
                    }

                    // Create proposal
                    let proposal = TradeProposal::new(
                        agent_id,
                        target,
                        offering.clone(),
                        requesting.clone(),
                        epoch,
                        trade_config.proposal_expiry_epochs,
                    );
                    let proposal_id = proposal.id;
                    let offer_str = proposal.offering_description();
                    let request_str = proposal.requesting_description();

                    self.trade_state.add_proposal(proposal);

                    // Log event
                    self.log_and_track(Event::trade_proposed(
                        epoch, agent_id, target, proposal_id, &offer_str, &request_str,
                    ))?;

                    // Add memories
                    let target_name = self.agents[target_idx].name().to_string();
                    let agent_name = self.agents[agent_idx].name().to_string();
                    self.agents[agent_idx].memory.remember(Episode::new(
                        epoch,
                        format!("I offered {} to {} for {}", offer_str, target_name, request_str),
                        0.1,
                        vec![target],
                        EpisodeCategory::Social,
                    ));
                    self.agents[target_idx].memory.remember(Episode::new(
                        epoch,
                        format!("{} offered me {} for {}", agent_name, offer_str, request_str),
                        0.2,
                        vec![agent_id],
                        EpisodeCategory::Social,
                    ));

                    debug!("{} proposes trade to {}", agent_id, target);
                }

                Action::TradeAccept { proposal_index } => {
                    let trade_config = self.config.trade.clone();
                    if !trade_config.enabled {
                        continue;
                    }

                    // Get pending proposals for this agent
                    let pending: Vec<_> = self.trade_state.pending_proposals_for(agent_id)
                        .into_iter()
                        .map(|p| p.id)
                        .collect();

                    if proposal_index >= pending.len() {
                        continue;
                    }
                    let proposal_id = pending[proposal_index];

                    // Get proposal details
                    let proposal = match self.trade_state.get_proposal(proposal_id) {
                        Some(p) if p.is_pending() => p.clone(),
                        _ => continue,
                    };

                    let proposer_idx = self.agents.iter().position(|a| a.id == proposal.proposer);
                    if proposer_idx.is_none() {
                        continue;
                    }
                    let proposer_idx = proposer_idx.unwrap();

                    // Validate both parties still have items
                    if !self.agent_has_items(proposer_idx, &proposal.offering) {
                        debug!("Proposer no longer has offered items");
                        continue;
                    }
                    if !self.agent_has_items(agent_idx, &proposal.requesting) {
                        debug!("Accepter doesn't have requested items");
                        continue;
                    }

                    // Execute the trade - transfer physical items
                    self.transfer_items(proposer_idx, agent_idx, &proposal.offering);
                    self.transfer_items(agent_idx, proposer_idx, &proposal.requesting);

                    // Create service debts for promises
                    for item in &proposal.offering {
                        if let Some(debt) = ServiceDebt::from_promise(
                            item, proposal.proposer, agent_id, proposal_id, epoch,
                            trade_config.default_promise_deadline,
                        ) {
                            self.trade_state.add_debt(debt);
                        }
                    }
                    for item in &proposal.requesting {
                        if let Some(debt) = ServiceDebt::from_promise(
                            item, agent_id, proposal.proposer, proposal_id, epoch,
                            trade_config.default_promise_deadline,
                        ) {
                            self.trade_state.add_debt(debt);
                        }
                    }

                    // Mark proposal as accepted
                    if let Some(p) = self.trade_state.get_proposal_mut(proposal_id) {
                        p.status = ProposalStatus::Accepted;
                    }

                    // Log event
                    self.log_and_track(Event::trade_accepted(
                        epoch, proposal.proposer, agent_id, proposal_id,
                    ))?;

                    // Trust boost for both parties
                    let proposer_name = self.agents[proposer_idx].name().to_string();
                    let agent_name = self.agents[agent_idx].name().to_string();
                    self.agents[agent_idx].beliefs.update_trust(proposal.proposer, &proposer_name, 0.1, epoch);
                    self.agents[proposer_idx].beliefs.update_trust(agent_id, &agent_name, 0.1, epoch);

                    debug!("{} accepts trade from {}", agent_id, proposal.proposer);
                }

                Action::TradeDecline { proposal_index } => {
                    let trade_config = self.config.trade.clone();
                    if !trade_config.enabled {
                        continue;
                    }

                    let pending: Vec<_> = self.trade_state.pending_proposals_for(agent_id)
                        .into_iter()
                        .map(|p| p.id)
                        .collect();

                    if proposal_index >= pending.len() {
                        continue;
                    }
                    let proposal_id = pending[proposal_index];

                    let proposal = match self.trade_state.get_proposal(proposal_id) {
                        Some(p) if p.is_pending() => p.clone(),
                        _ => continue,
                    };

                    // Mark as declined
                    if let Some(p) = self.trade_state.get_proposal_mut(proposal_id) {
                        p.status = ProposalStatus::Declined;
                    }

                    // Log event
                    self.log_and_track(Event::trade_declined(
                        epoch, proposal.proposer, agent_id, proposal_id,
                    ))?;

                    // Minor sentiment penalty
                    let proposer_idx = self.agents.iter().position(|a| a.id == proposal.proposer);
                    if let Some(p_idx) = proposer_idx {
                        let agent_name = self.agents[agent_idx].name().to_string();
                        self.agents[p_idx].beliefs.update_sentiment(agent_id, &agent_name, -trade_config.decline_trust_penalty, epoch);
                    }

                    debug!("{} declines trade from {}", agent_id, proposal.proposer);
                }

                Action::TradeCounter { proposal_index, offering, requesting } => {
                    let trade_config = self.config.trade.clone();
                    if !trade_config.enabled {
                        continue;
                    }

                    let pending: Vec<_> = self.trade_state.pending_proposals_for(agent_id)
                        .into_iter()
                        .map(|p| p.id)
                        .collect();

                    if proposal_index >= pending.len() {
                        continue;
                    }
                    let original_id = pending[proposal_index];

                    let original = match self.trade_state.get_proposal(original_id) {
                        Some(p) if p.is_pending() => p.clone(),
                        _ => continue,
                    };

                    // Validate agent has items they're offering
                    if !self.agent_has_items(agent_idx, &offering) {
                        continue;
                    }

                    // Mark original as countered
                    if let Some(p) = self.trade_state.get_proposal_mut(original_id) {
                        p.status = ProposalStatus::Countered;
                    }

                    // Create counter proposal
                    let counter = TradeProposal::counter(
                        &original,
                        offering.clone(),
                        requesting.clone(),
                        epoch,
                        trade_config.proposal_expiry_epochs,
                    );
                    let counter_id = counter.id;
                    let offer_str = counter.offering_description();
                    let request_str = counter.requesting_description();

                    self.trade_state.add_proposal(counter);

                    // Log event
                    self.log_and_track(Event::trade_countered(
                        epoch, original.proposer, agent_id, original_id, counter_id, &offer_str, &request_str,
                    ))?;

                    debug!("{} counter-offers trade to {}", agent_id, original.proposer);
                }

                Action::TradeCancel { proposal_index } => {
                    if !self.config.trade.enabled {
                        continue;
                    }

                    let pending: Vec<_> = self.trade_state.pending_proposals_from(agent_id)
                        .into_iter()
                        .map(|p| p.id)
                        .collect();

                    if proposal_index >= pending.len() {
                        continue;
                    }
                    let proposal_id = pending[proposal_index];

                    let proposal = match self.trade_state.get_proposal(proposal_id) {
                        Some(p) if p.is_pending() => p.clone(),
                        _ => continue,
                    };

                    // Mark as cancelled
                    if let Some(p) = self.trade_state.get_proposal_mut(proposal_id) {
                        p.status = ProposalStatus::Cancelled;
                    }

                    // Log event
                    self.log_and_track(Event::trade_cancelled(
                        epoch, agent_id, proposal.recipient, proposal_id,
                    ))?;

                    debug!("{} cancels their trade offer", agent_id);
                }
            }
        }

        // Clean up broken tools at end of action resolution
        let mut tool_break_events = Vec::new();
        for agent in &mut self.agents {
            if agent.is_alive() {
                let broken = agent.physical.cleanup_broken_tools();
                for tool in broken {
                    tool_break_events.push((agent.id, tool.display_name()));
                }
            }
        }
        for (agent_id, tool_name) in tool_break_events {
            self.log_and_track(Event::tool_broke(epoch, agent_id, &tool_name))?;
        }

        Ok(())
    }

    /// Update agent beliefs based on observations
    fn update_beliefs(&mut self, epoch: usize) {
        // Collect agent names for territory belief updates
        let agent_names: HashMap<Uuid, String> = self.agents
            .iter()
            .map(|a| (a.id, a.name().to_string()))
            .collect();

        // Update perceived safety based on recent events
        for agent in &mut self.agents {
            if !agent.is_alive() {
                continue;
            }

            let pos = (agent.physical.x, agent.physical.y);

            // Everything in view updates what the agent believes about food there, including
            // that it has run out; places out of sight keep their (possibly stale) beliefs
            let sight = self.world.sight_from(pos.0, pos.1, self.config.agents.view_radius);
            for seen in self.world.visible_cells(pos.0, pos.1, sight) {
                if seen.food > 0 || agent.beliefs.world.food_locations.iter().any(|b| (b.x, b.y) == (seen.x, seen.y)) {
                    agent.beliefs.update_food_belief(seen.x, seen.y, seen.food, epoch);
                }
            }

            if let Some(cell) = self.world.get(pos.0, pos.1) {

                // Update territory beliefs based on current perception
                if let Some(ref territory) = cell.territory {
                    let owner_name = agent_names.get(&territory.owner)
                        .cloned()
                        .unwrap_or_else(|| "Unknown".to_string());
                    let is_allowed = territory.owner == agent.id ||
                        territory.allowed_guests.contains(&agent.id);
                    agent.beliefs.update_territory_belief(
                        pos.0,
                        pos.1,
                        territory.owner,
                        &owner_name,
                        is_allowed,
                        epoch,
                    );
                } else {
                    // No territory here - remove stale belief if any
                    agent.beliefs.remove_territory_belief(pos.0, pos.1);
                }
            }

            // Shelter safety boost
            let shelter_boost = if let Some((sx, sy)) = agent.physical.sheltered_at {
                if let Some(cell) = self.world.get(sx, sy) {
                    if let Some(ref structure) = cell.structure {
                        structure.structure_type.safety_boost()
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Territory safety boost (being on own territory feels safe)
            let territory_boost = if let Some(cell) = self.world.get(pos.0, pos.1) {
                if let Some(ref territory) = cell.territory {
                    if territory.owner == agent.id {
                        0.1 // Own territory
                    } else if territory.allowed_guests.contains(&agent.id) {
                        0.05 // Guest on friendly territory
                    } else {
                        -0.1 // Trespassing - feels unsafe
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Adjust perceived safety over time (regression to mean) + shelter boost + territory boost
            let base_safety = agent.beliefs.self_belief.perceived_safety * 0.9 + 0.5 * 0.1;
            agent.beliefs.self_belief.perceived_safety = (base_safety + shelter_boost + territory_boost).clamp(0.0, 1.0);
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

        use rand::RngExt;
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

    /// Unused skills fade over time
    fn tick_skill_decay(&mut self, epoch: usize) {
        if !self.config.skills.enabled {
            return;
        }
        let (rate, threshold) = (self.config.skills.decay_rate, self.config.skills.decay_threshold_epochs);
        for agent in self.agents.iter_mut().filter(|a| a.is_alive()) {
            agent.skills.decay_unused(rate, threshold, epoch);
        }
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

    /// What this agent can craft, build and do with structures and territory here
    fn action_context(&self, agent: &Agent) -> ActionContext {
        let pos = (agent.physical.x, agent.physical.y);
        let cell = self.world.get(pos.0, pos.1);
        let structure = cell.and_then(|c| c.structure.as_ref());
        let territory = cell.and_then(|c| c.territory.as_ref());

        let craftable_tools = self.recipe_registry.craftable_tools(
            &agent.physical.materials,
            &agent.physical.tools,
            agent.skills.level("crafting"),
        );

        // New structures need an empty cell; an unfinished one can be worked on further
        let buildable_structures = match (cell, structure) {
            (Some(cell), None) => crate::structures::StructureRecipeRegistry::new().buildable(
                &agent.physical.materials,
                cell.terrain,
                |tool| agent.physical.has_tool(tool),
            ),
            (_, Some(s)) if !s.is_complete() => vec![s.structure_type],
            _ => vec![],
        };

        let owns_territory = territory.is_some_and(|t| t.owner == agent.id);
        let can_mark_territory = self.count_agent_territories(agent.id) < 4
            && match territory {
                None => true,
                Some(t) => t.owner == agent.id || t.strength < 0.3,
            }
            && !owns_territory;
        let trespassers = if owns_territory {
            let guests = &territory.unwrap().allowed_guests;
            self.agents
                .iter()
                .filter(|a| a.is_alive() && a.id != agent.id && !guests.contains(&a.id))
                .filter(|a| (a.physical.x, a.physical.y) == pos)
                .map(|a| (a.id, a.name().to_string()))
                .collect()
        } else {
            vec![]
        };

        ActionContext {
            craftable_tools,
            buildable_structures,
            has_shelter: structure.is_some_and(|s| s.structure_type.is_shelter() && s.can_use(agent.id)),
            has_storage: structure.is_some_and(|s| s.structure_type.has_storage() && s.can_use(agent.id)),
            owns_structure: structure.is_some_and(|s| s.owner == agent.id),
            is_sheltered: agent.physical.sheltered_at.is_some(),
            can_mark_territory,
            trespassers,
            is_trespassing: territory
                .is_some_and(|t| t.owner != agent.id && !t.allowed_guests.contains(&agent.id)),
            unfinished_project: self
                .world
                .cells
                .iter()
                .filter_map(|c| c.structure.as_ref().map(|s| (c, s)))
                .filter(|(_, s)| s.owner == agent.id && !s.is_complete())
                .min_by_key(|(c, _)| c.x.abs_diff(pos.0).max(c.y.abs_diff(pos.1)))
                .map(|(c, s)| (c.x, c.y, s.structure_type, s.build_progress * 100 / s.build_required.max(1))),
            home: self.nearest_structure(pos, |s| s.owner == agent.id && s.is_complete() && s.structure_type.is_shelter())
                .map(|(x, y, _)| (x, y)),
            storage: self.nearest_structure(pos, |s| s.structure_type.has_storage() && s.can_use(agent.id))
                .map(|(x, y, s)| (x, y, s.inventory.as_ref().map_or(0, |i| i.food))),
            here_barren: cell.is_some_and(|c| c.terrain == Terrain::Barren),
            materials_here: cell.is_some_and(|c| c.wood > 0 || c.stone > 0),
            adjacent_barren: self.world.adjacent(pos.0, pos.1).iter()
                .find(|c| c.terrain == Terrain::Barren && c.stone > 0).map(|c| (c.x, c.y)),
            adjacent_fertile: self.world.adjacent(pos.0, pos.1).iter()
                .find(|c| c.terrain == Terrain::Fertile && c.wood > 0).map(|c| (c.x, c.y)),
            raid_target: self
                .world
                .cells
                .iter()
                .filter(|c| c.x.abs_diff(pos.0) <= 1 && c.y.abs_diff(pos.1) <= 1)
                .filter_map(|c| c.structure.as_ref().map(|s| (c, s)))
                .filter(|(_, s)| s.structure_type.has_storage() && s.is_complete() && !s.can_use(agent.id))
                .filter_map(|(c, s)| s.inventory.as_ref().filter(|i| i.food > 0).map(|i| (c, s, i.food)))
                .min_by_key(|(c, ..)| (c.x, c.y) != pos)
                .map(|(c, s, food)| (c.x, c.y, s.owner, self.agent_name(s.owner), food)),
            neighbours: self
                .agents
                .iter()
                .filter(|a| a.is_alive() && a.id != agent.id && is_adjacent(a, agent))
                .map(|a| Neighbour {
                    id: a.id,
                    name: a.name().to_string(),
                    x: a.physical.x,
                    y: a.physical.y,
                    health: a.physical.health,
                    hunger: a.physical.hunger,
                    food: a.physical.food,
                    strength: a.physical.health * a.age_modifier(&self.config.aging),
                    relatedness: relatedness(agent, a),
                    sick: a.physical.sick_days > 0,
                })
                .collect(),
            strength: agent.physical.health * agent.age_modifier(&self.config.aging),
            mating_food_cost: if self.config.reproduction.enabled { self.config.reproduction.mating_food_cost } else { 0 },
            fertile: self.config.reproduction.enabled && self.config.aging.is_fertile(agent.physical.age),
            claimed_cells: self
                .world
                .cells
                .iter()
                .filter(|c| c.x.abs_diff(pos.0) <= 1 && c.y.abs_diff(pos.1) <= 1)
                .filter_map(|c| c.territory.as_ref().map(|t| (c, t)))
                .filter(|(_, t)| t.owner != agent.id && !t.allowed_guests.contains(&agent.id))
                .map(|(c, t)| (c.x, c.y, t.owner))
                .collect(),
            territory_owner_nearby: territory
                .filter(|t| t.owner != agent.id && !t.allowed_guests.contains(&agent.id))
                .filter(|t| self.agents.iter().any(|a| a.id == t.owner && a.is_alive() && is_adjacent(a, agent)))
                .map(|t| t.owner),
            unfinished_here_owner: structure
                .filter(|s| s.owner != agent.id && !s.is_complete())
                .map(|s| (s.owner, self.agent_name(s.owner))),
        }
    }

    /// Nearest structure (by steps) matching the predicate: (x, y, structure)
    fn nearest_structure(
        &self,
        pos: (usize, usize),
        pred: impl Fn(&crate::structures::Structure) -> bool,
    ) -> Option<(usize, usize, &crate::structures::Structure)> {
        self.world
            .cells
            .iter()
            .filter_map(|c| c.structure.as_ref().map(|s| (c, s)))
            .filter(|(_, s)| pred(s))
            .min_by_key(|(c, _)| c.x.abs_diff(pos.0).max(c.y.abs_diff(pos.1)))
            .map(|(c, s)| (c.x, c.y, s))
    }

    fn agent_name(&self, id: Uuid) -> String {
        self.agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name().to_string())
            .unwrap_or_else(|| "someone".to_string())
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

        // Mating needs consent from both: either both choose MATE this epoch, or
        // one accepts the other's still-open proposal from an earlier epoch
        let mut matched: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

        for (agent_a, target_a) in &mate_actions {
            if matched.contains(agent_a) {
                continue;
            }

            let same_epoch = mate_actions
                .iter()
                .any(|(agent_b, target_b)| agent_b == target_a && target_b == agent_a);
            let accepts_open_proposal = self
                .mate_proposals
                .get(target_a)
                .is_some_and(|(target, expires)| target == agent_a && *expires >= epoch);

            if same_epoch || accepts_open_proposal {
                matched.insert(*agent_a);
                matched.insert(*target_a);
                self.mate_proposals.remove(agent_a);
                self.mate_proposals.remove(target_a);
                self.attempt_mating(epoch, *agent_a, *target_a)?;
                continue;
            }

            // Otherwise this is a proposal the target can accept in the coming days
            let is_new = self
                .mate_proposals
                .get(agent_a)
                .is_none_or(|(target, _)| target != target_a);
            self.mate_proposals
                .insert(*agent_a, (*target_a, epoch + MATE_PROPOSAL_DAYS));

            if is_new {
                let proposer_name = self.agent_name(*agent_a);
                let target_name = self.agent_name(*target_a);
                if let Some(idx) = self.agents.iter().position(|a| a.id == *agent_a) {
                    self.agents[idx].memory.remember(Episode::social(
                        epoch,
                        &format!("I asked {} to have a child with me", target_name),
                        0.1,
                        *target_a,
                    ));
                }
                if let Some(idx) = self.agents.iter().position(|a| a.id == *target_a) {
                    self.agents[idx].memory.remember(Episode::social(
                        epoch,
                        &format!("{} asked me to have a child with them", proposer_name),
                        0.15,
                        *agent_a,
                    ));
                }
            }
        }

        // Expire proposals that went unanswered
        let expired: Vec<(Uuid, Uuid)> = self
            .mate_proposals
            .iter()
            .filter(|(_, (_, expires))| *expires <= epoch)
            .map(|(proposer, (target, _))| (*proposer, *target))
            .collect();
        for (proposer, target) in expired {
            self.mate_proposals.remove(&proposer);
            let target_name = self.agent_name(target);
            if let Some(idx) = self.agents.iter().position(|a| a.id == proposer) {
                self.agents[idx].memory.remember(Episode::social(
                    epoch,
                    &format!("{} wasn't interested in having a child with me", target_name),
                    -0.1,
                    target,
                ));
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
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "not adjacent");
        }

        // Check if both are alive
        if !self.agents[idx_a].is_alive() || !self.agents[idx_b].is_alive() {
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "partner not alive");
        }

        // Check health requirements
        if self.agents[idx_a].physical.health < config.min_health_to_reproduce
            || self.agents[idx_b].physical.health < config.min_health_to_reproduce
        {
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "health too low");
        }

        // Check energy requirements
        if self.agents[idx_a].physical.energy < config.min_energy_to_reproduce
            || self.agents[idx_b].physical.energy < config.min_energy_to_reproduce
        {
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "energy too low");
        }

        // Check food requirements
        if self.agents[idx_a].physical.food < config.mating_food_cost
            || self.agents[idx_b].physical.food < config.mating_food_cost
        {
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "not enough food");
        }

        // Check mating cooldowns
        if !self.config.aging.is_fertile(self.agents[idx_a].physical.age)
            || !self.config.aging.is_fertile(self.agents[idx_b].physical.age)
        {
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "not of fertile age");
        }

        if self.agents[idx_a].reproduction.mating_cooldown > 0
            || self.agents[idx_b].reproduction.mating_cooldown > 0
        {
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "mating cooldown");
        }

        // Check if either is already gestating
        if self.agents[idx_a].reproduction.gestation.is_some()
            || self.agents[idx_b].reproduction.gestation.is_some()
        {
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "already expecting");
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
            return mating_failed(&self.agents[idx_a], &self.agents[idx_b], "courtship below threshold");
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

    /// Count how many territories an agent owns
    fn count_agent_territories(&self, agent_id: Uuid) -> usize {
        let mut count = 0;
        for cell in &self.world.cells {
            if let Some(ref claim) = cell.territory {
                if claim.owner == agent_id {
                    count += 1;
                }
            }
        }
        count
    }

    /// Process structure production (farms produce food each epoch)
    fn process_structure_production(&mut self, epoch: usize) -> Result<()> {
        use crate::structures::StructureType;

        // Collect production info first to avoid borrow issues
        let mut productions: Vec<(usize, usize, Uuid, u32)> = Vec::new();

        for y in 0..self.world.height {
            for x in 0..self.world.width {
                if let Some(cell) = self.world.get(x, y) {
                    if let Some(ref structure) = cell.structure {
                        if structure.structure_type == StructureType::Farm && structure.is_complete() {
                            let production = structure.effective_food_production();
                            if production > 0 {
                                productions.push((x, y, structure.owner, production));
                            }
                        }
                    }
                }
            }
        }

        // Apply productions and log events
        for (x, y, owner, amount) in productions {
            // Add food directly to the cell
            if let Some(cell) = self.world.get_mut(x, y) {
                cell.food = cell.food.saturating_add(amount);
                // Cap at cell capacity
                cell.food = cell.food.min(cell.food_capacity);
            }

            self.log_and_track(Event::farm_produced(epoch, owner, x, y, amount))?;
            debug!("Farm at ({}, {}) produced {} food", x, y, amount);
        }

        Ok(())
    }

    /// Decay structures each epoch and remove destroyed ones
    /// Contagion (SIR): the sick lose health and energy until they recover with immunity; the
    /// well catch it from each sick person within one step, or rarely on their own
    fn tick_disease(&mut self, epoch: usize) -> Result<()> {
        let config = self.config.disease.clone();
        if !config.enabled {
            return Ok(());
        }
        use rand::RngExt;
        let mut rng = rand::rng();
        // Who is infectious today (as of the start of the day)
        let sick: Vec<(usize, usize, Uuid)> = self
            .agents
            .iter()
            .filter(|a| a.is_alive() && a.physical.sick_days > 0)
            .map(|a| (a.physical.x, a.physical.y, a.id))
            .collect();
        let mut events = Vec::new();
        for agent in self.agents.iter_mut().filter(|a| a.is_alive()) {
            let p = &mut agent.physical;
            if p.sick_days > 0 {
                p.health -= config.health_damage;
                p.energy = (p.energy - config.energy_drain).max(0.0);
                p.sick_days -= 1;
                if !agent.is_alive() {
                    events.push(Event::died(epoch, agent.id, "illness"));
                } else if agent.physical.sick_days == 0 {
                    agent.physical.immune_until = epoch + config.immunity_days;
                    agent.memory.remember(Episode::survival(epoch, "I recovered from my illness", 0.5));
                    events.push(Event::recovered(epoch, agent.id));
                }
                continue;
            }
            if epoch < p.immune_until {
                continue;
            }
            let contacts = sick
                .iter()
                .filter(|(x, y, id)| *id != agent.id && x.abs_diff(p.x) <= 1 && y.abs_diff(p.y) <= 1)
                .count();
            let escape = (1.0 - config.transmission).powi(contacts as i32) * (1.0 - config.spontaneous_chance);
            if rng.random::<f64>() >= escape {
                let (lo, hi) = config.sick_days;
                p.sick_days = rng.random_range(lo.max(1)..=hi.max(lo.max(1)));
                agent.memory.remember(Episode::survival(epoch, "I fell ill", -0.6));
                events.push(Event::fell_ill(epoch, agent.id, contacts > 0));
            }
        }
        for event in events {
            self.log_and_track(event)?;
        }
        Ok(())
    }

    /// Storms batter every structure; ones worn down to nothing are destroyed
    fn storm_damage(&mut self, epoch: usize) -> Result<()> {
        let damage = self.environment.weather.storm_damage;
        let mut destroyed = Vec::new();
        for cell in &mut self.world.cells {
            if let Some(structure) = cell.structure.as_mut().filter(|s| s.is_complete()) {
                structure.decay(damage);
                if structure.is_destroyed() {
                    destroyed.push((cell.x, cell.y, structure.owner, structure.display_name()));
                    cell.structure = None;
                }
            }
        }
        for (x, y, owner, name) in destroyed {
            self.log_and_track(Event::structure_destroyed(epoch, owner, x, y, &name))?;
        }
        Ok(())
    }

    fn decay_structures(&mut self, epoch: usize) -> Result<()> {
        // Collect structures to decay and check for destruction
        let mut destroyed: Vec<(usize, usize, Uuid, String)> = Vec::new();

        for y in 0..self.world.height {
            for x in 0..self.world.width {
                if let Some(cell) = self.world.get_mut(x, y) {
                    if let Some(ref mut structure) = cell.structure {
                        // Only decay complete structures
                        if structure.is_complete() {
                            structure.decay(1);

                            if structure.is_destroyed() {
                                let owner = structure.owner;
                                let name = structure.display_name();
                                destroyed.push((x, y, owner, name));
                            }
                        }
                    }
                }
            }
        }

        // Remove destroyed structures and log events
        for (x, y, owner, name) in destroyed {
            if let Some(cell) = self.world.get_mut(x, y) {
                cell.structure = None;
            }

            self.log_and_track(Event::structure_destroyed(epoch, owner, x, y, &name))?;
            debug!("Structure '{}' at ({}, {}) was destroyed from decay", name, x, y);
        }

        Ok(())
    }

    // ==================== Trade Maintenance ====================

    /// Expire trade proposals that have passed their expiry epoch
    fn expire_trade_proposals(&mut self, epoch: usize) -> Result<()> {
        // Collect proposals to expire
        let to_expire: Vec<_> = self
            .trade_state
            .proposals
            .iter()
            .filter(|(_, p)| p.is_pending() && p.is_expired(epoch))
            .map(|(id, p)| (*id, p.proposer, p.recipient))
            .collect();

        for (proposal_id, proposer, recipient) in to_expire {
            // Mark as expired
            if let Some(proposal) = self.trade_state.get_proposal_mut(proposal_id) {
                proposal.status = ProposalStatus::Expired;
            }

            // Log event
            self.log_and_track(Event::trade_expired(epoch, proposal_id, proposer, recipient))?;

            debug!("Trade proposal {} expired", proposal_id);
        }

        // Cleanup old completed proposals (keep last 50)
        self.trade_state.cleanup_old_proposals(50);

        Ok(())
    }

    /// Check service debt deadlines and apply renege penalties
    fn check_service_deadlines(&mut self, epoch: usize) -> Result<()> {
        let trade_config = self.config.trade.clone();

        // Collect overdue debts
        let overdue: Vec<_> = self
            .trade_state
            .service_debts
            .iter()
            .enumerate()
            .filter(|(_, d)| d.is_overdue(epoch) && !d.reneged)
            .map(|(idx, d)| (idx, d.debtor, d.creditor, d.service.describe()))
            .collect();

        // Process in reverse to avoid index shifting
        for (debt_idx, debtor, creditor, service_desc) in overdue.into_iter().rev() {
            // Mark as reneged
            self.trade_state.service_debts[debt_idx].mark_reneged();

            // Find agent indices
            let debtor_idx = self.agents.iter().position(|a| a.id == debtor);
            let creditor_idx = self.agents.iter().position(|a| a.id == creditor);

            // Apply penalties
            if let (Some(d_idx), Some(c_idx)) = (debtor_idx, creditor_idx) {
                let debtor_name = self.agents[d_idx].name().to_string();

                // Creditor loses trust and sentiment toward debtor
                self.agents[c_idx].beliefs.update_trust(
                    debtor,
                    &debtor_name,
                    -trade_config.renege_trust_penalty,
                    epoch,
                );
                self.agents[c_idx].beliefs.update_sentiment(
                    debtor,
                    &debtor_name,
                    -trade_config.renege_trust_penalty * 0.6,
                    epoch,
                );

                // Add memory to creditor
                self.agents[c_idx].memory.remember(Episode::new(
                    epoch,
                    format!("{} broke their promise to {}", debtor_name, service_desc),
                    -0.5,
                    vec![debtor],
                    EpisodeCategory::Social,
                ));
            }

            // Log event
            self.log_and_track(Event::trade_reneged(epoch, debtor, creditor, &service_desc))?;

            debug!("{} reneged on promise: {}", debtor, service_desc);
        }

        // Also clean up fulfilled/expired alliance debts
        self.trade_state.service_debts.retain(|d| {
            // Keep if not fulfilled and not reneged
            // OR if it's an active alliance
            if d.fulfilled || d.reneged {
                return false;
            }
            if let ServiceType::Alliance { expires_epoch } = &d.service {
                // Remove expired alliances
                epoch < *expires_epoch
            } else {
                true
            }
        });

        Ok(())
    }

    // ==================== Trade Helpers ====================

    /// Check if an agent has the items required for a trade (excluding promises)
    fn agent_has_items(&self, agent_idx: usize, items: &[TradeableItem]) -> bool {
        let agent = &self.agents[agent_idx];

        for item in items {
            match item {
                TradeableItem::Food(amount) => {
                    if agent.physical.food < *amount {
                        return false;
                    }
                }
                TradeableItem::Materials(mat, amount) => {
                    let has = agent.physical.materials.get(mat).copied().unwrap_or(0);
                    if has < *amount {
                        return false;
                    }
                }
                TradeableItem::Tool(tool_id) => {
                    if !agent.physical.tools.iter().any(|t| t.id == *tool_id) {
                        return false;
                    }
                }
                TradeableItem::ToolByType(tool_type) => {
                    if !agent.physical.tools.iter().any(|t| t.tool_type == *tool_type) {
                        return false;
                    }
                }
                // Promises don't require having anything
                TradeableItem::TeachSkillPromise { .. }
                | TradeableItem::HelpBuildPromise { .. }
                | TradeableItem::FutureGiftPromise { .. }
                | TradeableItem::AlliancePromise { .. } => {}
            }
        }

        true
    }

    /// Transfer items from one agent to another
    fn transfer_items(&mut self, from_idx: usize, to_idx: usize, items: &[TradeableItem]) {
        for item in items {
            match item {
                TradeableItem::Food(amount) => {
                    let actual = self.agents[from_idx].physical.food.min(*amount);
                    self.agents[from_idx].physical.food -= actual;
                    self.agents[to_idx].physical.food += actual;
                }
                TradeableItem::Materials(mat, amount) => {
                    let has = self.agents[from_idx].physical.materials.entry(*mat).or_insert(0);
                    let actual = (*has).min(*amount);
                    *has -= actual;
                    *self.agents[to_idx].physical.materials.entry(*mat).or_insert(0) += actual;
                }
                TradeableItem::Tool(tool_id) => {
                    if let Some(pos) = self.agents[from_idx].physical.tools.iter().position(|t| t.id == *tool_id) {
                        let tool = self.agents[from_idx].physical.tools.remove(pos);
                        self.agents[to_idx].physical.tools.push(tool);
                    }
                }
                TradeableItem::ToolByType(tool_type) => {
                    // Find first tool of this type and transfer it
                    if let Some(pos) = self.agents[from_idx].physical.tools.iter().position(|t| t.tool_type == *tool_type) {
                        let tool = self.agents[from_idx].physical.tools.remove(pos);
                        self.agents[to_idx].physical.tools.push(tool);
                    }
                }
                // Promises don't transfer anything physical
                TradeableItem::TeachSkillPromise { .. }
                | TradeableItem::HelpBuildPromise { .. }
                | TradeableItem::FutureGiftPromise { .. }
                | TradeableItem::AlliancePromise { .. } => {}
            }
        }
    }

    // ==================== Service Debt Fulfillment ====================

    /// Check if a TEACH action fulfills a TeachSkill debt
    /// Returns the debt ID if fulfilled, None otherwise
    fn check_teach_fulfills_debt(
        &mut self,
        teacher: Uuid,
        student: Uuid,
        skill: &str,
        epoch: usize,
    ) -> Option<Uuid> {
        let trade_config = self.config.trade.clone();

        // Find a matching unfulfilled debt
        let debt_idx = self.trade_state.service_debts.iter().position(|d| {
            d.debtor == teacher
                && d.creditor == student
                && !d.fulfilled
                && !d.reneged
                && matches!(&d.service, ServiceType::TeachSkill { skill: s } if s.to_lowercase() == skill.to_lowercase())
        })?;

        let debt_id = self.trade_state.service_debts[debt_idx].id;
        let service_desc = self.trade_state.service_debts[debt_idx].service.describe();

        // Mark as fulfilled
        self.trade_state.service_debts[debt_idx].fulfilled = true;

        // Apply trust bonus
        let teacher_idx = self.agents.iter().position(|a| a.id == teacher);
        let student_idx = self.agents.iter().position(|a| a.id == student);

        if let (Some(t_idx), Some(s_idx)) = (teacher_idx, student_idx) {
            let teacher_name = self.agents[t_idx].name().to_string();

            // Student trusts teacher more for fulfilling promise
            self.agents[s_idx].beliefs.update_trust(
                teacher,
                &teacher_name,
                trade_config.fulfill_trust_bonus,
                epoch,
            );

            // Add memory
            self.agents[s_idx].memory.remember(Episode::new(
                epoch,
                format!("{} fulfilled their promise to {}", teacher_name, service_desc),
                0.3,
                vec![teacher],
                EpisodeCategory::Social,
            ));
        }

        // Log event
        if let Err(e) = self.log_and_track(Event::service_fulfilled(
            epoch,
            teacher,
            student,
            &service_desc,
        )) {
            debug!("Failed to log service fulfillment: {}", e);
        }

        debug!("TeachSkill debt {} fulfilled: {} taught {} to creditor", debt_id, teacher, skill);

        Some(debt_id)
    }

    /// Check if a GIVE action contributes to a FutureGift debt
    /// Returns the debt ID and whether it's now fully fulfilled
    fn check_give_fulfills_debt(
        &mut self,
        giver: Uuid,
        receiver: Uuid,
        amount: u32,
        epoch: usize,
    ) -> Option<(Uuid, bool)> {
        let trade_config = self.config.trade.clone();

        // Find a matching unfulfilled FutureGift debt
        let debt_idx = self.trade_state.service_debts.iter().position(|d| {
            d.debtor == giver
                && d.creditor == receiver
                && !d.fulfilled
                && !d.reneged
                && matches!(&d.service, ServiceType::FutureGift { .. })
        })?;

        let debt_id = self.trade_state.service_debts[debt_idx].id;

        // Add to the gift progress
        self.trade_state.service_debts[debt_idx].add_gift(amount);

        let is_fulfilled = self.trade_state.service_debts[debt_idx].fulfilled;
        let service_desc = self.trade_state.service_debts[debt_idx].service.describe();

        // If fully fulfilled, apply trust bonus and log event
        if is_fulfilled {
            let giver_idx = self.agents.iter().position(|a| a.id == giver);
            let receiver_idx = self.agents.iter().position(|a| a.id == receiver);

            if let (Some(g_idx), Some(r_idx)) = (giver_idx, receiver_idx) {
                let giver_name = self.agents[g_idx].name().to_string();

                // Receiver trusts giver more for fulfilling promise
                self.agents[r_idx].beliefs.update_trust(
                    giver,
                    &giver_name,
                    trade_config.fulfill_trust_bonus,
                    epoch,
                );

                // Add memory
                self.agents[r_idx].memory.remember(Episode::new(
                    epoch,
                    format!("{} fulfilled their promise: {}", giver_name, service_desc),
                    0.3,
                    vec![giver],
                    EpisodeCategory::Social,
                ));
            }

            // Log event
            if let Err(e) = self.log_and_track(Event::service_fulfilled(
                epoch,
                giver,
                receiver,
                &service_desc,
            )) {
                debug!("Failed to log service fulfillment: {}", e);
            }

            debug!("FutureGift debt {} fully fulfilled", debt_id);
        } else {
            debug!("FutureGift debt {} progress: {}", debt_id, service_desc);
        }

        Some((debt_id, is_fulfilled))
    }

    /// Check if a BUILD action contributes to a HelpBuild debt
    /// Returns the debt ID and whether it's now fully fulfilled
    fn check_build_fulfills_debt(
        &mut self,
        builder: Uuid,
        structure_owner: Uuid,
        labor_points: u32,
        epoch: usize,
    ) -> Option<(Uuid, bool)> {
        let trade_config = self.config.trade.clone();

        // Find a matching unfulfilled HelpBuild debt
        let debt_idx = self.trade_state.service_debts.iter().position(|d| {
            d.debtor == builder
                && d.creditor == structure_owner
                && !d.fulfilled
                && !d.reneged
                && matches!(&d.service, ServiceType::HelpBuild { .. })
        })?;

        let debt_id = self.trade_state.service_debts[debt_idx].id;

        // Add labor progress
        self.trade_state.service_debts[debt_idx].add_labor(labor_points);

        let is_fulfilled = self.trade_state.service_debts[debt_idx].fulfilled;
        let service_desc = self.trade_state.service_debts[debt_idx].service.describe();

        // If fully fulfilled, apply trust bonus and log event
        if is_fulfilled {
            let builder_idx = self.agents.iter().position(|a| a.id == builder);
            let owner_idx = self.agents.iter().position(|a| a.id == structure_owner);

            if let (Some(b_idx), Some(o_idx)) = (builder_idx, owner_idx) {
                let builder_name = self.agents[b_idx].name().to_string();

                // Owner trusts builder more
                self.agents[o_idx].beliefs.update_trust(
                    builder,
                    &builder_name,
                    trade_config.fulfill_trust_bonus,
                    epoch,
                );

                // Add memory
                self.agents[o_idx].memory.remember(Episode::new(
                    epoch,
                    format!("{} fulfilled their promise: {}", builder_name, service_desc),
                    0.3,
                    vec![builder],
                    EpisodeCategory::Social,
                ));
            }

            // Log event
            if let Err(e) = self.log_and_track(Event::service_fulfilled(
                epoch,
                builder,
                structure_owner,
                &service_desc,
            )) {
                debug!("Failed to log service fulfillment: {}", e);
            }

            debug!("HelpBuild debt {} fully fulfilled", debt_id);
        } else {
            debug!("HelpBuild debt {} progress: {}", debt_id, service_desc);
        }

        Some((debt_id, is_fulfilled))
    }

    /// Update territory decay each epoch
    fn update_territories(&mut self, epoch: usize) -> Result<()> {
        // First, update territory strength/decay
        let mut lost_territories = Vec::new();

        for y in 0..self.world.height {
            for x in 0..self.world.width {
                if let Some(cell) = self.world.get_mut(x, y) {
                    if let Some(ref mut claim) = cell.territory {
                        // Check if owner is nearby (within 2 cells)
                        let owner_nearby = self.agents.iter().any(|a| {
                            a.id == claim.owner &&
                            a.is_alive() &&
                            manhattan_distance((a.physical.x, a.physical.y), (x, y)) <= 2
                        });

                        if owner_nearby {
                            claim.last_presence_epoch = epoch;
                            claim.strength = (claim.strength + 0.1).min(1.0);
                        } else {
                            // Decay
                            claim.strength -= 0.1;
                            if claim.strength <= 0.0 {
                                lost_territories.push((claim.owner, x, y));
                            }
                        }
                    }
                }
            }
        }

        // Remove fully decayed territories and log events
        for (owner, x, y) in lost_territories {
            if let Some(cell) = self.world.get_mut(x, y) {
                cell.territory = None;
            }
            self.log_and_track(Event::territory_lost(epoch, owner, x, y))?;
        }

        Ok(())
    }
}

/// Manhattan distance between two positions
fn manhattan_distance(a: (usize, usize), b: (usize, usize)) -> usize {
    ((a.0 as i32 - b.0 as i32).abs() + (a.1 as i32 - b.1 as i32).abs()) as usize
}

/// Check if two agents are adjacent (within 1 cell)
fn mating_failed(a: &Agent, b: &Agent, reason: &str) -> Result<()> {
    debug!("{} and {} could not mate: {}", a.name(), b.name(), reason);
    Ok(())
}

/// Relatedness as the agents know it: parent and child or full siblings 0.5, half siblings 0.25
fn relatedness(a: &Agent, b: &Agent) -> f64 {
    let (fa, fb) = (&a.reproduction.family, &b.reproduction.family);
    if fa.parents.contains(&b.id) || fb.parents.contains(&a.id) {
        return 0.5;
    }
    match fa.parents.iter().filter(|p| fb.parents.contains(p)).count() {
        0 => 0.0,
        1 => 0.25,
        _ => 0.5,
    }
}

fn is_adjacent(a: &Agent, b: &Agent) -> bool {
    let dx = (a.physical.x as i32 - b.physical.x as i32).abs();
    let dy = (a.physical.y as i32 - b.physical.y as i32).abs();
    dx <= 1 && dy <= 1
}

/// How effectively a hunter searches their range: encounter chance = 1 - exp(-a * animals)
const HUNT_SEARCH_EFFICIENCY: f64 = 0.15;

/// How readily fish bite: chance = 1 - exp(-a * fish in reach)
const FISH_SEARCH_EFFICIENCY: f64 = 0.1;

/// Build progress from one day's work (recipes need 10-30; a skilled builder
/// finishes a lean-to in a day, a novice in about three)
fn build_progress(crafting_skill: f64) -> u32 {
    4 + (crafting_skill * 6.0).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Engine with two adjacent, healthy, fed agents who have courted each other fully
    fn courting_pair(test_name: &str) -> (Engine, Uuid, Uuid) {
        let dir = std::env::temp_dir().join(format!("terrarium-test-{}-{}", test_name, std::process::id()));
        let mut config = Config::default();
        config.agents.count = 2;
        config.world.water.lakes = 0;
        let mut engine = Engine::new(config, dir.to_str().unwrap()).unwrap();

        let (a, b) = (engine.agents[0].id, engine.agents[1].id);
        for (i, partner) in [(0, b), (1, a)] {
            let agent = &mut engine.agents[i];
            agent.physical.x = 5;
            agent.physical.y = 5;
            agent.physical.food = 10;
            agent.physical.energy = 1.0;
            agent.reproduction.courtship_progress.insert(partner, 1.0);
        }
        (engine, a, b)
    }

    fn mate(agent: Uuid, target: Uuid) -> HashMap<Uuid, Action> {
        HashMap::from([(agent, Action::Mate { target })])
    }

    fn expecting(engine: &Engine) -> bool {
        engine.agents.iter().any(|a| a.reproduction.gestation.is_some())
    }

    #[test]
    fn test_mating_proposal_accepted_on_later_epoch() {
        let (mut engine, a, b) = courting_pair("accept");
        engine.resolve_mating(1, &mate(a, b)).unwrap();
        assert!(!expecting(&engine), "one-sided proposal must not conceive");

        engine.resolve_mating(1 + MATE_PROPOSAL_DAYS - 1, &mate(b, a)).unwrap();
        assert!(expecting(&engine));
        assert!(engine.mate_proposals.is_empty());
    }

    #[test]
    fn test_mating_proposal_expires() {
        let (mut engine, a, b) = courting_pair("expire");
        engine.resolve_mating(1, &mate(a, b)).unwrap();
        engine.resolve_mating(1 + MATE_PROPOSAL_DAYS, &HashMap::new()).unwrap();
        assert!(engine.mate_proposals.is_empty());

        // Accepting after expiry only opens a new proposal from b
        engine.resolve_mating(2 + MATE_PROPOSAL_DAYS, &mate(b, a)).unwrap();
        assert!(!expecting(&engine));
        assert!(engine.mate_proposals.contains_key(&b));
    }

    #[test]
    fn test_attack_seizes_food() {
        let (mut engine, a, b) = courting_pair("attack");
        engine.agents[0].physical.food = 0;
        engine.agents[1].physical.food = 10;

        engine.resolve_actions(1, HashMap::from([(a, Action::Attack { target: b })])).unwrap();

        // Damage 0.15-0.25 seizes 30-50% of the victim's food
        let seized = engine.agents[0].physical.food;
        assert!((3..=5).contains(&seized), "seized {}", seized);
        assert_eq!(engine.agents[1].physical.food, 10 - seized);
    }

    #[test]
    fn test_action_context_offers_craft_and_build() {
        use crate::crafting::ToolType;
        use crate::structures::StructureType;
        let (mut engine, _, _) = courting_pair("context");
        let context = engine.action_context(&engine.agents[0]);
        assert!(context.craftable_tools.is_empty());
        assert!(context.buildable_structures.is_empty());

        let agent = &mut engine.agents[0];
        agent.physical.add_material(MaterialType::Stone, 2);
        agent.physical.add_material(MaterialType::Wood, 3);
        agent.physical.add_material(MaterialType::Fiber, 2);
        let context = engine.action_context(&engine.agents[0]);
        assert!(context.craftable_tools.contains(&ToolType::StoneAxe));
        assert!(context.buildable_structures.contains(&StructureType::LeanTo));
        assert!(context.can_mark_territory);
    }

    #[test]
    fn test_unfinished_project_in_context_and_completion_logged() {
        use crate::structures::{Structure, StructureType};
        let (mut engine, a, b) = courting_pair("project");
        let structure = Structure::new(StructureType::LeanTo, a, 10, ToolQuality::Standard, 0);
        engine.world.get_mut(5, 5).unwrap().structure = Some(structure);
        engine.agents[0].physical.x = 2;

        let context = engine.action_context(&engine.agents[0]);
        assert_eq!(context.unfinished_project, Some((5, 5, StructureType::LeanTo, 0)));
        assert_eq!(context.home, None);
        let context = engine.action_context(&engine.agents[1]);
        assert_eq!(context.unfinished_here_owner.map(|(id, _)| id), Some(a));
        assert_eq!(context.buildable_structures, vec![StructureType::LeanTo]);

        // The friend finishes it for the owner
        let build = HashMap::from([(b, Action::Build { structure_type: StructureType::LeanTo })]);
        for _ in 0..10 {
            engine.agents[1].physical.energy = 1.0;
            engine.resolve_actions(1, build.clone()).unwrap();
        }
        assert!(engine.recent_events.iter().any(|e| matches!(e.event_type, crate::observation::EventType::StructureCompleted) && e.agent == Some(a)));
        let context = engine.action_context(&engine.agents[0]);
        assert_eq!(context.unfinished_project, None);
        assert_eq!(context.home, Some((5, 5)));
    }

    #[test]
    fn test_food_storage_deposit_and_withdraw() {
        use crate::structures::{Structure, StructureType};
        let (mut engine, a, b) = courting_pair("storage");
        let mut storage = Structure::new(StructureType::Storage, a, 20, ToolQuality::Standard, 0);
        storage.add_progress(20, 0);
        engine.world.get_mut(5, 5).unwrap().structure = Some(storage);
        let stored = |e: &Engine| e.world.get(5, 5).unwrap().structure.as_ref().unwrap().inventory.as_ref().unwrap().food;

        let context = engine.action_context(&engine.agents[0]);
        assert_eq!(context.storage, Some((5, 5, 0)));
        engine.resolve_actions(1, HashMap::from([(a, Action::DepositFood { amount: 6 })])).unwrap();
        assert_eq!((engine.agents[0].physical.food, stored(&engine)), (4, 6));

        // Only the owner (or permitted agents) may take from it
        engine.resolve_actions(2, HashMap::from([(b, Action::WithdrawFood { amount: 5 })])).unwrap();
        assert_eq!((engine.agents[1].physical.food, stored(&engine)), (10, 6));
        engine.resolve_actions(3, HashMap::from([(a, Action::WithdrawFood { amount: 5 })])).unwrap();
        assert_eq!((engine.agents[0].physical.food, stored(&engine)), (9, 1));
    }

    #[test]
    fn test_raid_unguarded_storehouse() {
        use crate::structures::{Structure, StructureType};
        let (mut engine, a, b) = courting_pair("raid");
        let mut storage = Structure::new(StructureType::Storage, a, 20, ToolQuality::Standard, 0);
        storage.add_progress(20, 0);
        storage.inventory.as_mut().unwrap().add_food(10);
        engine.world.get_mut(5, 5).unwrap().structure = Some(storage);
        engine.agents[0].physical.x = 0; // the owner is away

        let context = engine.action_context(&engine.agents[1]);
        assert_eq!(context.raid_target.map(|(x, y, owner, _, food)| (x, y, owner, food)), Some((5, 5, a, 10)));
        assert!(engine.action_context(&engine.agents[0]).raid_target.is_none());

        engine.resolve_actions(1, HashMap::from([(b, Action::Raid)])).unwrap();
        assert_eq!(engine.agents[1].physical.food, 16);
        let left = engine.world.get(5, 5).unwrap().structure.as_ref().unwrap().inventory.as_ref().unwrap().food;
        assert_eq!(left, 4);
        assert!(engine.recent_events.iter().any(|e| matches!(e.event_type, crate::observation::EventType::Raided)));
        // Unseen, the owner can't tell who did it
        assert!(engine.agents[0].beliefs.social.get(&b).is_none_or(|s| s.trust >= 0.0));
    }

    #[test]
    fn test_moving_away_does_not_dodge_a_same_day_attack() {
        let (mut engine, a, b) = courting_pair("dodge");
        for _ in 0..20 {
            engine.agents[1].physical.x = 5;
            engine.agents[1].physical.health = 1.0;
            let actions = HashMap::from([(a, Action::Attack { target: b }), (b, Action::Move(Direction::East))]);
            engine.resolve_actions(1, actions).unwrap();
            assert!(engine.agents[1].physical.health < 1.0, "the attack must land before the move");
            assert_eq!(engine.agents[1].physical.x, 6);
        }
    }

    #[test]
    fn test_gathering_depletes_the_land() {
        let (mut engine, a, _) = courting_pair("deplete");
        let cell = engine.world.get_mut(5, 5).unwrap();
        cell.terrain = Terrain::Barren;
        cell.stone = 3;
        // Stone and flint both come out of the cell's stone
        let mined = |e: &Engine| {
            let m = &e.agents[0].physical.materials;
            m.get(&MaterialType::Stone).copied().unwrap_or(0) + m.get(&MaterialType::Flint).copied().unwrap_or(0)
        };
        engine.resolve_actions(1, HashMap::from([(a, Action::GatherMaterials)])).unwrap();
        let got = mined(&engine);
        let left = engine.world.get(5, 5).unwrap().stone;
        assert_eq!(got + left, 3, "stone is moved, not created");
        assert!(got >= 2);
        engine.resolve_actions(2, HashMap::from([(a, Action::GatherMaterials)])).unwrap();
        engine.resolve_actions(3, HashMap::from([(a, Action::GatherMaterials)])).unwrap();
        assert_eq!(engine.world.get(5, 5).unwrap().stone, 0);
        assert_eq!(mined(&engine), 3);
        assert!(engine.world.describe_cell(5, 5).contains("quarried out"));
    }

    #[test]
    fn test_lakes_block_walkers_and_feed_anglers() {
        let (mut engine, a, _) = courting_pair("lake");
        let lake = engine.world.get_mut(6, 5).unwrap();
        lake.terrain = Terrain::Water;
        lake.fish = 4.0;
        lake.fish_capacity = 15.0;

        engine.resolve_actions(1, HashMap::from([(a, Action::Move(Direction::East))])).unwrap();
        assert_eq!(engine.agents[0].physical.x, 5, "nobody walks into a lake");

        engine.agents[0].physical.tools.push(Tool::new(ToolType::FishingPole, ToolQuality::Standard, None, 0));
        let food = engine.agents[0].physical.food;
        for day in 2..40 {
            engine.agents[0].physical.energy = 1.0;
            engine.resolve_actions(day, HashMap::from([(a, Action::Fish)])).unwrap();
        }
        let caught = engine.agents[0].physical.food - food;
        let left = engine.world.get(6, 5).unwrap().fish;
        assert!(caught > 0 && caught <= 4, "caught {}", caught);
        assert!((left - (4.0 - caught as f64)).abs() < 1e-9, "fish are caught, not conjured");

        // Away from water there is nothing to fish
        engine.agents[0].physical.x = 2;
        let food = engine.agents[0].physical.food;
        engine.resolve_actions(50, HashMap::from([(a, Action::Fish)])).unwrap();
        assert_eq!(engine.agents[0].physical.food, food);
    }

    #[test]
    fn test_seeing_an_emptied_patch_corrects_the_belief() {
        let (mut engine, _, _) = courting_pair("belief");
        engine.agents[0].beliefs.update_food_belief(6, 7, 9, 0);
        engine.world.get_mut(6, 7).unwrap().food = 0;
        engine.world.get_mut(3, 3).unwrap().food = 8;
        engine.update_beliefs(1);
        let belief = |x, y| {
            engine.agents[0].beliefs.world.food_locations.iter().find(|b| (b.x, b.y) == (x, y)).map(|b| b.belief.amount)
        };
        assert_eq!(belief(6, 7), Some(0), "within view: the agent sees it's gone");
        assert_eq!(belief(3, 3), Some(8), "2 cells away is still in view");
        assert_eq!(belief(9, 9), None, "out of view: nothing learned");
    }

    #[test]
    fn test_storms_batter_structures() {
        use crate::structures::{Structure, StructureType};
        let (mut engine, a, _) = courting_pair("storm");
        let mut lean_to = Structure::new(StructureType::LeanTo, a, 10, ToolQuality::Standard, 0);
        lean_to.add_progress(10, 0);
        lean_to.durability = 7;
        engine.world.get_mut(5, 5).unwrap().structure = Some(lean_to);
        engine.environment.weather = crate::environment::WeatherConfig::temperate();
        engine.storm_damage(1).unwrap();
        assert_eq!(engine.world.get(5, 5).unwrap().structure.as_ref().unwrap().durability, 2);
        engine.storm_damage(2).unwrap();
        assert!(engine.world.get(5, 5).unwrap().structure.is_none(), "worn-out structures are destroyed");
    }

    #[test]
    fn test_illness_spreads_to_neighbours_and_leaves_immunity() {
        let (mut engine, _, _) = courting_pair("illness");
        engine.config.disease = crate::config::DiseaseConfig {
            enabled: true,
            spontaneous_chance: 0.0,
            transmission: 1.0,
            sick_days: (3, 3),
            ..Default::default()
        };
        engine.agents[0].physical.sick_days = 3;
        engine.tick_disease(1).unwrap();
        assert_eq!(engine.agents[1].physical.sick_days, 3, "caught it from the neighbour");
        assert_eq!(engine.agents[0].physical.sick_days, 2);
        assert!(engine.agents[0].physical.health < 1.0);

        engine.tick_disease(2).unwrap();
        engine.tick_disease(3).unwrap();
        assert_eq!(engine.agents[0].physical.sick_days, 0, "recovered");
        assert!(engine.agents[0].physical.immune_until > 3);
        engine.tick_disease(4).unwrap();
        engine.tick_disease(5).unwrap();
        assert_eq!(engine.agents[0].physical.sick_days, 0, "immune to re-infection");

        // Distance protects
        let (mut engine, _, _) = courting_pair("illness-apart");
        engine.config.disease = crate::config::DiseaseConfig { enabled: true, spontaneous_chance: 0.0, transmission: 1.0, ..Default::default() };
        engine.agents[0].physical.sick_days = 5;
        engine.agents[1].physical.x = 8;
        engine.tick_disease(1).unwrap();
        assert_eq!(engine.agents[1].physical.sick_days, 0);
    }

    #[test]
    fn test_founders_are_all_created_and_spread_out() {
        let dir = std::env::temp_dir().join(format!("terrarium-test-founders-{}", std::process::id()));
        let mut config = Config::default();
        config.world.width = 30;
        config.world.height = 30;
        config.agents.count = 60;
        let engine = Engine::new(config, dir.to_str().unwrap()).unwrap();
        assert_eq!(engine.agents.len(), 60);
        let names: std::collections::HashSet<_> = engine.agents.iter().map(|a| a.name().to_string()).collect();
        assert_eq!(names.len(), 60, "names are unique");
        let ys: Vec<usize> = engine.agents.iter().map(|a| a.physical.y).collect();
        assert!(*ys.iter().min().unwrap() < 5 && *ys.iter().max().unwrap() > 25, "spread over the map: {:?}", ys);
        assert!(engine.agents.iter().all(|a| engine.world.walkable(a.physical.x, a.physical.y)));
    }

    #[test]
    fn test_moving_leaves_shelter() {
        let (mut engine, a, _) = courting_pair("shelter");
        engine.agents[0].physical.enter_shelter(5, 5);
        engine.resolve_actions(1, HashMap::from([(a, Action::Move(Direction::North))])).unwrap();
        assert!(engine.agents[0].physical.sheltered_at.is_none());
    }

    #[test]
    fn test_elders_cannot_conceive() {
        let (mut engine, a, b) = courting_pair("elder");
        engine.agents[0].physical.age = engine.config.aging.max_lifespan * 2 / 3;
        let actions = HashMap::from([(a, Action::Mate { target: b }), (b, Action::Mate { target: a })]);
        engine.resolve_mating(1, &actions).unwrap();
        assert!(!expecting(&engine));
        assert!(!engine.action_context(&engine.agents[0]).fertile);
        assert!(engine.action_context(&engine.agents[1]).fertile);
    }

    #[test]
    fn test_mutual_mate_same_epoch() {
        let (mut engine, a, b) = courting_pair("mutual");
        let actions = HashMap::from([(a, Action::Mate { target: b }), (b, Action::Mate { target: a })]);
        engine.resolve_mating(1, &actions).unwrap();
        assert!(expecting(&engine));
    }
}
