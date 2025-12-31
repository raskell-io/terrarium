use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod action;
mod agent;
mod config;
mod engine;
mod groups;
mod llm;
mod observation;
mod observer;
mod tui;
mod world;

use config::Config;
use engine::Engine;

#[derive(Parser, Debug)]
#[command(name = "terrarium")]
#[command(about = "A societal simulation engine where LLM-powered agents form emergent civilizations")]
#[command(version)]
struct Args {
    /// Path to scenario configuration file
    #[arg(short, long)]
    scenario: Option<String>,

    /// Output directory for logs and snapshots
    #[arg(short, long, default_value = "output")]
    output: String,

    /// Override number of epochs
    #[arg(short, long)]
    epochs: Option<usize>,

    /// Verbosity level (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Run with TUI viewer
    #[arg(long)]
    tui: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let filter = match args.verbose {
        0 => "terrarium=info",
        1 => "terrarium=debug",
        _ => "terrarium=trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .init();

    info!("Terrarium v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let mut config = match &args.scenario {
        Some(path) => {
            info!("Loading scenario from {}", path);
            Config::from_file(path)?
        }
        None => {
            info!("Using default configuration");
            Config::default()
        }
    };

    // Apply overrides
    if let Some(epochs) = args.epochs {
        config.simulation.epochs = epochs;
    }

    info!(
        "Scenario: {} ({} agents, {} epochs)",
        config.meta.name, config.agents.count, config.simulation.epochs
    );

    if args.tui {
        // Run with TUI viewer
        tui::run(config, &args.output).await?;
    } else {
        // Run headless (batch mode)
        let mut engine = Engine::new(config, &args.output)?;
        engine.run().await?;
    }

    info!("Output written to {}/", args.output);
    info!("  - events.jsonl: Full event log");
    info!("  - chronicle.md: Human-readable narrative");
    info!("  - states/: Periodic state snapshots");

    Ok(())
}
