use clap::Parser;
use tracing::info;

mod agent;
mod chronicle;
mod config;
mod engine;
mod llm;
mod world;

use config::SimulationConfig;
use engine::Engine;

#[derive(Parser, Debug)]
#[command(name = "terrarium")]
#[command(about = "A societal simulation engine where LLM-powered agents form emergent civilizations")]
struct Args {
    /// Number of agents to simulate
    #[arg(short, long, default_value = "10")]
    agents: usize,

    /// Number of epochs to run
    #[arg(short, long, default_value = "100")]
    epochs: usize,

    /// Path to configuration file
    #[arg(short, long)]
    config: Option<String>,

    /// Output directory for chronicles and snapshots
    #[arg(short, long, default_value = "output")]
    output: String,

    /// Random seed for reproducibility
    #[arg(short, long)]
    seed: Option<u64>,

    /// Verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize logging
    let filter = match args.verbose {
        0 => "terrarium=info",
        1 => "terrarium=debug",
        _ => "terrarium=trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    info!("Terrarium v{}", env!("CARGO_PKG_VERSION"));
    info!("Initializing simulation with {} agents for {} epochs", args.agents, args.epochs);

    // Load or create configuration
    let config = match args.config {
        Some(path) => SimulationConfig::from_file(&path)?,
        None => SimulationConfig::default_with(args.agents, args.epochs, args.seed),
    };

    // Create and run the engine
    let mut engine = Engine::new(config, &args.output).await?;
    engine.run().await?;

    info!("Simulation complete. Chronicle written to {}/chronicle.md", args.output);
    Ok(())
}
