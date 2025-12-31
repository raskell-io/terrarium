//! TUI client for Terrarium simulations.
//!
//! A Dwarf Fortress-inspired terminal viewer with modern keybindings.

mod app;
mod input;
mod ui;
mod widgets;

pub use app::App;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::engine::Engine;

type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Run the TUI application
pub async fn run(config: Config, output_dir: &str) -> Result<()> {
    // Initialize terminal
    let mut terminal = setup_terminal()?;

    // Create engine and app
    let mut engine = Engine::new(config, output_dir)?;
    engine.initialize()?;

    let mut app = App::new();

    // Main loop
    let result = run_app(&mut terminal, &mut engine, &mut app).await;

    // Finalize
    engine.finalize()?;

    // Restore terminal
    restore_terminal(&mut terminal)?;

    result
}

/// Set up the terminal for TUI rendering
fn setup_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to normal mode
fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Main application loop
async fn run_app(terminal: &mut Tui, engine: &mut Engine, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();
    let mut last_step = Instant::now();

    loop {
        // Draw UI
        terminal.draw(|frame| ui::draw(frame, engine, app))?;

        // Handle input with timeout
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events (not release)
                if key.kind == KeyEventKind::Press {
                    // Check for step request (n key when paused)
                    let step_requested = !app.running
                        && !engine.is_complete()
                        && matches!(
                            key.code,
                            crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Char('N')
                        );

                    if input::handle_key(key, app, engine) {
                        break; // Quit requested
                    }

                    // Execute step if requested
                    if step_requested {
                        engine.step().await?;
                    }
                }
            }
        }

        // Update tick
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        // Auto-advance simulation if running
        if app.running && !engine.is_complete() {
            let step_interval = Duration::from_millis(app.speed_ms as u64);
            if last_step.elapsed() >= step_interval {
                engine.step().await?;
                last_step = Instant::now();
            }
        }
    }

    Ok(())
}
