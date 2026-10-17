//! Input handling for the TUI.

use super::App;
use crate::engine::Engine;

/// A key press, independent of where it came from (the terminal or the browser)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Tab,
    /// Shift+Tab
    BackTab,
    Esc,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Other,
}

/// Handle a key press. Returns true if quit was requested.
pub fn handle_key(key: Key, app: &mut App, engine: &Engine) -> bool {
    // Get living agent IDs
    let living_agents: Vec<uuid::Uuid> = engine
        .agent_views()
        .iter()
        .filter(|a| a.alive)
        .map(|a| a.id)
        .collect();

    // Help overlay handling
    if app.show_help {
        match key {
            Key::Esc | Key::Char('?') => {
                app.show_help = false;
            }
            _ => {}
        }
        return false;
    }

    match key {
        // Quit
        Key::Char('q') | Key::Char('Q') => return app.can_quit,

        // Help
        Key::Char('?') => {
            app.show_help = true;
        }

        // Simulation control
        Key::Char(' ') => {
            app.toggle_running();
        }
        Key::Char('n') | Key::Char('N') => {
            // Step handled in main loop when paused
            if !app.running {
                // Signal step needed (handled in main loop via flag)
                app.running = false; // Ensure paused
            }
        }
        Key::Char('+') | Key::Char('=') => {
            app.speed_up();
        }
        Key::Char('-') => {
            app.slow_down();
        }

        // Navigation
        Key::Tab => {
            app.select_next(&living_agents);
        }
        Key::BackTab => {
            app.select_prev(&living_agents);
        }

        // Number selection (1-9)
        Key::Char(c) if c.is_ascii_digit() && c != '0' => {
            let n = c.to_digit(10).unwrap() as usize;
            app.select_by_number(n, &living_agents);
        }

        // Arrow keys - find adjacent agent
        Key::Left | Key::Right | Key::Up | Key::Down => {
            select_adjacent(key, app, engine, &living_agents);
        }

        // View toggles
        Key::Char('e') | Key::Char('E') => {
            app.show_events = !app.show_events;
        }
        Key::Char('t') | Key::Char('T') => {
            app.toggle_trades();
        }
        Key::Char('a') | Key::Char('A') => {
            app.show_agent = !app.show_agent;
        }
        Key::Char('f') | Key::Char('F') => {
            app.show_full_agent = !app.show_full_agent;
        }

        // Scrolling
        Key::PageUp => {
            app.scroll_events_up();
        }
        Key::PageDown => {
            app.scroll_events_down();
        }

        // Escape
        Key::Esc => {
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
    direction: Key,
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
        Key::Left => (-1, 0),
        Key::Right => (1, 0),
        Key::Up => (0, -1),
        Key::Down => (0, 1),
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
            Key::Left => rel_x < 0,
            Key::Right => rel_x > 0,
            Key::Up => rel_y < 0,
            Key::Down => rel_y > 0,
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
