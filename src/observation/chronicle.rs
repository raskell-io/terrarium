use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use uuid::Uuid;

use super::events::{Event, EventType};
use crate::agent::Agent;
use crate::world::World;

/// Generates human-readable chronicle from events
pub struct Chronicle {
    output_dir: PathBuf,
    events_file: BufWriter<File>,
    chronicle_file: BufWriter<File>,
    agent_names: HashMap<Uuid, String>,
}

impl Chronicle {
    pub fn new(output_dir: &str) -> anyhow::Result<Self> {
        let output_path = PathBuf::from(output_dir);
        fs::create_dir_all(&output_path)?;

        let events_path = output_path.join("events.jsonl");
        let chronicle_path = output_path.join("chronicle.md");

        let events_file = BufWriter::new(File::create(events_path)?);
        let chronicle_file = BufWriter::new(File::create(chronicle_path)?);

        Ok(Self {
            output_dir: output_path,
            events_file,
            chronicle_file,
            agent_names: HashMap::new(),
        })
    }

    /// Register agent names for narrative generation
    pub fn register_agents(&mut self, agents: &[Agent]) {
        for agent in agents {
            self.agent_names.insert(agent.id, agent.name().to_string());
        }
    }

    /// Write the chronicle header
    pub fn write_header(&mut self, scenario_name: &str, world: &World, agents: &[Agent]) -> anyhow::Result<()> {
        writeln!(self.chronicle_file, "# {}", scenario_name)?;
        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "> A Terrarium Chronicle")?;
        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "## The World")?;
        writeln!(self.chronicle_file)?;
        writeln!(
            self.chronicle_file,
            "A {}x{} world. {} souls begin their journey.",
            world.width, world.height, agents.len()
        )?;
        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "## The Inhabitants")?;
        writeln!(self.chronicle_file)?;

        for agent in agents {
            writeln!(
                self.chronicle_file,
                "- **{}**: {} Their aspiration: {}.",
                agent.name(),
                agent.identity.personality.describe(),
                agent.identity.aspiration.describe()
            )?;
        }

        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "---")?;
        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "## Chronicle")?;
        writeln!(self.chronicle_file)?;

        self.chronicle_file.flush()?;
        Ok(())
    }

    /// Log an event (to both events.jsonl and potentially chronicle)
    pub fn log_event(&mut self, event: &Event) -> anyhow::Result<()> {
        // Write to events.jsonl
        let json = serde_json::to_string(event)?;
        writeln!(self.events_file, "{}", json)?;

        // Write significant events to chronicle
        if let Some(narrative) = self.event_to_narrative(event) {
            writeln!(self.chronicle_file, "{}", narrative)?;
            self.chronicle_file.flush()?;
        }

        Ok(())
    }

    /// Flush both files
    pub fn flush(&mut self) -> anyhow::Result<()> {
        self.events_file.flush()?;
        self.chronicle_file.flush()?;
        Ok(())
    }

    /// Convert an event to narrative (returns None for insignificant events)
    fn event_to_narrative(&self, event: &Event) -> Option<String> {
        let agent_name = event.agent.and_then(|id| self.agent_names.get(&id));
        let target_name = event.target.and_then(|id| self.agent_names.get(&id));

        match &event.event_type {
            EventType::EpochStart => {
                Some(format!("### Day {}\n", event.epoch))
            }
            EventType::Spoke => {
                let agent = agent_name?;
                let target = target_name?;
                let message = event.data.message.as_ref()?;
                Some(format!("**{}** said to **{}**: \"{}\"", agent, target, message))
            }
            EventType::Gave => {
                let agent = agent_name?;
                let target = target_name?;
                let amount = event.data.amount?;
                Some(format!("**{}** gave {} food to **{}**.", agent, amount, target))
            }
            EventType::Attacked => {
                let agent = agent_name?;
                let target = target_name?;
                Some(format!("**{}** attacked **{}**!", agent, target))
            }
            EventType::AllyIntervened => {
                let target = target_name?;
                let ally_name = event.data.ally.and_then(|id| self.agent_names.get(&id))?;
                let reduction = event.data.damage_reduction.unwrap_or(0.0) * 100.0;
                Some(format!("**{}** defended **{}**, reducing damage by {:.0}%.", ally_name, target, reduction))
            }
            EventType::Died => {
                let agent = agent_name?;
                let cause = event.data.description.as_deref().unwrap_or("unknown causes");
                Some(format!("**{}** has died from {}.", agent, cause))
            }
            EventType::TradeProposed => {
                let agent = agent_name?;
                let target = target_name?;
                let offering = event.data.trade_offering.as_deref().unwrap_or("items");
                let requesting = event.data.trade_requesting.as_deref().unwrap_or("items");
                Some(format!("**{}** proposed a trade to **{}**: offering {} for {}.", agent, target, offering, requesting))
            }
            EventType::TradeAccepted => {
                let agent = agent_name?;
                let target = target_name?;
                Some(format!("**{}** accepted a trade from **{}**.", agent, target))
            }
            EventType::TradeDeclined => {
                let agent = agent_name?;
                let target = target_name?;
                Some(format!("**{}** declined a trade from **{}**.", agent, target))
            }
            EventType::TradeCountered => {
                let agent = agent_name?;
                let target = target_name?;
                let offering = event.data.trade_offering.as_deref().unwrap_or("items");
                let requesting = event.data.trade_requesting.as_deref().unwrap_or("items");
                Some(format!("**{}** counter-offered to **{}**: offering {} for {}.", agent, target, offering, requesting))
            }
            EventType::TradeExpired => {
                let agent = agent_name?;
                let target = target_name?;
                Some(format!("A trade proposal from **{}** to **{}** expired.", agent, target))
            }
            EventType::TradeReneged => {
                let agent = agent_name?;
                let target = target_name?;
                let service = event.data.service_type.as_deref().unwrap_or("their promise");
                Some(format!("**{}** reneged on {} to **{}**!", agent, service, target))
            }
            _ => None, // Don't narrate routine events
        }
    }

    /// Write the chronicle footer
    pub fn write_footer(&mut self, world: &World, agents: &[Agent]) -> anyhow::Result<()> {
        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "---")?;
        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "## Aftermath")?;
        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "After {} days:", world.epoch)?;
        writeln!(self.chronicle_file)?;

        let alive: Vec<_> = agents.iter().filter(|a| a.is_alive()).collect();
        let dead: Vec<_> = agents.iter().filter(|a| !a.is_alive()).collect();

        writeln!(self.chronicle_file, "**Survivors ({}):**", alive.len())?;
        for agent in &alive {
            writeln!(
                self.chronicle_file,
                "- **{}**: {:.0}% health, {} food",
                agent.name(),
                agent.physical.health * 100.0,
                agent.physical.food
            )?;
        }

        if !dead.is_empty() {
            writeln!(self.chronicle_file)?;
            writeln!(self.chronicle_file, "**Perished ({}):**", dead.len())?;
            for agent in &dead {
                writeln!(self.chronicle_file, "- **{}**", agent.name())?;
            }
        }

        writeln!(self.chronicle_file)?;
        writeln!(self.chronicle_file, "---")?;
        writeln!(self.chronicle_file)?;
        writeln!(
            self.chronicle_file,
            "*Chronicle generated by Terrarium v{}*",
            env!("CARGO_PKG_VERSION")
        )?;

        self.flush()?;
        Ok(())
    }

    /// Save a state snapshot
    pub fn save_snapshot(&self, epoch: usize, world: &World, agents: &[Agent]) -> anyhow::Result<()> {
        let states_dir = self.output_dir.join("states");
        fs::create_dir_all(&states_dir)?;

        let snapshot = Snapshot { epoch, world: world.clone(), agents: agents.to_vec() };
        let path = states_dir.join(format!("epoch_{:04}.json", epoch));
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &snapshot)?;

        Ok(())
    }
}

#[derive(serde::Serialize)]
struct Snapshot {
    epoch: usize,
    world: World,
    agents: Vec<Agent>,
}
