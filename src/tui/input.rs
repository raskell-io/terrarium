//! Input handling for the TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::App;
use crate::engine::Engine;

/// Handle a key event. Returns true if quit was requested.
pub fn handle_key(key: KeyEvent, app: &mut App, engine: &Engine) -> bool {
    // Get living agent IDs
    let living_agents: Vec<uuid::Uuid> = engine
        .agent_views()
        .iter()
        .filter(|a| a.alive)
        .map(|a| a.id)
        .collect();

    // Help overlay handling
    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') => {
                app.show_help = false;
            }
            _ => {}
        }
        return false;
    }

    match key.code {
        // Quit
        KeyCode::Char('q') | KeyCode::Char('Q') => return true,

        // Help
        KeyCode::Char('?') => {
            app.show_help = true;
        }

        // Simulation control
        KeyCode::Char(' ') => {
            app.toggle_running();
        }
        KeyCode::Char('n') | KeyCode::Char('N') => {
            // Step handled in main loop when paused
            if !app.running {
                // Signal step needed (handled in main loop via flag)
                app.running = false; // Ensure paused
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.speed_up();
        }
        KeyCode::Char('-') => {
            app.slow_down();
        }

        // Navigation
        KeyCode::Tab => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                app.select_prev(&living_agents);
            } else {
                app.select_next(&living_agents);
            }
        }
        KeyCode::BackTab => {
            app.select_prev(&living_agents);
        }

        // Number selection (1-9)
        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
            let n = c.to_digit(10).unwrap() as usize;
            app.select_by_number(n, &living_agents);
        }

        // Arrow keys - find adjacent agent
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
            select_adjacent(key.code, app, engine, &living_agents);
        }

        // View toggles
        KeyCode::Char('e') | KeyCode::Char('E') => {
            app.show_events = !app.show_events;
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            app.show_agent = !app.show_agent;
        }
        KeyCode::Char('f') | KeyCode::Char('F') => {
            app.show_full_agent = !app.show_full_agent;
        }

        // Scrolling
        KeyCode::PageUp => {
            app.scroll_events_up();
        }
        KeyCode::PageDown => {
            app.scroll_events_down();
        }

        // Escape
        KeyCode::Esc => {
            app.show_help = false;
        }

        _ => {}
    }

    // Ensure we have a valid selection
    app.ensure_selection(&living_agents);

    false
}

/// Select the agent in the given direction from current selection
fn select_adjacent(
    direction: KeyCode,
    app: &mut App,
    engine: &Engine,
    living_agents: &[uuid::Uuid],
) {
    let current_pos = if let Some(id) = app.selected_agent {
        engine
            .agent_views()
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.position)
    } else {
        None
    };

    let Some((cx, cy)) = current_pos else {
        return;
    };

    // Calculate direction delta
    let (dx, dy): (i32, i32) = match direction {
        KeyCode::Left => (-1, 0),
        KeyCode::Right => (1, 0),
        KeyCode::Up => (0, -1),
        KeyCode::Down => (0, 1),
        _ => return,
    };

    // Find the closest agent in that direction
    let agents = engine.agent_views();
    let mut best: Option<(uuid::Uuid, i32)> = None;

    for agent in agents.iter().filter(|a| a.alive && living_agents.contains(&a.id)) {
        let (ax, ay) = agent.position;
        let rel_x = ax as i32 - cx as i32;
        let rel_y = ay as i32 - cy as i32;

        // Check if agent is in the right direction
        let in_direction = match direction {
            KeyCode::Left => rel_x < 0,
            KeyCode::Right => rel_x > 0,
            KeyCode::Up => rel_y < 0,
            KeyCode::Down => rel_y > 0,
            _ => false,
        };

        if in_direction {
            let dist = rel_x.abs() + rel_y.abs();
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((agent.id, dist));
            }
        }
    }

    if let Some((id, _)) = best {
        app.selected_agent = Some(id);
        // Update index
        if let Some(idx) = living_agents.iter().position(|&a| a == id) {
            app.selected_index = idx;
        }
    }
}
