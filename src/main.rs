use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod action;
mod agent;
mod config;
mod crafting;
mod engine;
mod environment;
mod groups;
mod llm;
mod observation;
mod observer;
mod structures;
mod trade;
mod tui;
mod world;

use config::Config;
use engine::Engine;
use environment::EnvironmentConfig;

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

    /// Override environment preset (earth, mars, moon, antarctica, exoplanet, desert, station)
    #[arg(long)]
    environment: Option<String>,

    /// List available environment presets
    #[arg(long)]
    list_environments: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle --list-environments
    if args.list_environments {
        println!("Available environment presets:");
        println!();
        for preset in EnvironmentConfig::available_presets() {
            let env = EnvironmentConfig::from_name(preset).unwrap();
            println!("  {:20} - {}", preset, env.description);
        }
        println!();
        println!("Use with: --environment <preset>");
        return Ok(());
    }

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

    // Override environment if specified
    if let Some(env_name) = &args.environment {
        if let Some(env_config) = EnvironmentConfig::from_name(env_name) {
            info!("Overriding environment: {}", env_config.name);
            config.environment = Some(env_config);
        } else {
            eprintln!("Unknown environment: {}", env_name);
            eprintln!("Use --list-environments to see available presets");
            std::process::exit(1);
        }
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
