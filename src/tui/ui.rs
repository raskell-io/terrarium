//! UI rendering for the TUI.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::widgets;
use super::App;
use crate::engine::Engine;

/// Draw the entire UI
pub fn draw(frame: &mut Frame, engine: &Engine, app: &mut App) {
    // Ensure we have a valid selection
    let living_agents: Vec<uuid::Uuid> = engine
        .agent_views()
        .iter()
        .filter(|a| a.alive)
        .map(|a| a.id)
        .collect();
    app.ensure_selection(&living_agents);

    // Main layout
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(12),    // Top (world + events)
            Constraint::Length(12), // Agent panel
            Constraint::Length(1),  // Status bar
        ])
        .split(frame.area());

    // Top section: world and events side by side
    let top_constraints = if app.show_events {
        vec![Constraint::Percentage(50), Constraint::Percentage(50)]
    } else {
        vec![Constraint::Percentage(100)]
    };

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(top_constraints)
        .split(main_chunks[0]);

    // Draw world
    draw_world(frame, top_chunks[0], engine, app);

    // Draw events (if enabled)
    if app.show_events && top_chunks.len() > 1 {
        draw_events(frame, top_chunks[1], engine, app);
    }

    // Draw agent panel (if enabled)
    if app.show_agent {
        draw_agent(frame, main_chunks[1], engine, app);
    }

    // Draw status bar
    draw_status_bar(frame, main_chunks[2], engine, app);

    // Draw help overlay if active
    if app.show_help {
        draw_help(frame);
    }
}

/// Draw the world map
fn draw_world(frame: &mut Frame, area: Rect, engine: &Engine, app: &App) {
    let world_view = engine.world_view();
    let agent_views = engine.agent_views();

    widgets::world::draw(frame, area, &world_view, &agent_views, app.selected_agent);
}

/// Draw the events panel
fn draw_events(frame: &mut Frame, area: Rect, engine: &Engine, app: &App) {
    let events = engine.recent_event_views();
    widgets::events::draw(frame, area, &events, engine.epoch(), app.events_scroll);
}

/// Draw the agent panel
fn draw_agent(frame: &mut Frame, area: Rect, engine: &Engine, app: &App) {
    if let Some(id) = app.selected_agent {
        if let Some(agent_view) = engine.agent_view(id) {
            widgets::agent::draw(frame, area, &agent_view, app.show_full_agent);
        }
    } else {
        // No agent selected
        let block = Block::default()
            .title(" No Agent Selected ")
            .borders(Borders::ALL);
        frame.render_widget(block, area);
    }
}

/// Draw the status bar
fn draw_status_bar(frame: &mut Frame, area: Rect, engine: &Engine, app: &App) {
    let status = if engine.is_complete() {
        "COMPLETE"
    } else if app.running {
        "RUNNING"
    } else {
        "PAUSED"
    };

    let status_style = if app.running {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Yellow)
    };

    let speed_text = format!("{}ms/epoch", app.speed_ms);

    let line = Line::from(vec![
        Span::styled(
            " Space",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(": Play/Pause"),
        Span::raw(" | "),
        Span::styled("N", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Step"),
        Span::raw(" | "),
        Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Next Agent"),
        Span::raw(" | "),
        Span::styled("Arrows", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Select"),
        Span::raw(" | "),
        Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Quit"),
        Span::raw(" | "),
        Span::styled("?", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Help"),
        Span::raw("  "),
        Span::styled(status, status_style),
        Span::raw(format!(
            "  Day {} / {}  Alive: {}  [{}]",
            engine.epoch(),
            engine.total_epochs(),
            engine.alive_count(),
            speed_text,
        )),
    ]);

    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}

/// Draw the help overlay
fn draw_help(frame: &mut Frame) {
    let area = frame.area();

    // Center the help popup
    let popup_width = 60;
    let popup_height = 22;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear background
    frame.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(
            "TERRARIUM - Keybindings",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Simulation Control",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from("  Space       Pause / Resume"),
        Line::from("  N           Step one epoch (when paused)"),
        Line::from("  + / =       Increase speed"),
        Line::from("  -           Decrease speed"),
        Line::from(""),
        Line::from(Span::styled(
            "Navigation",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from("  Tab         Next agent"),
        Line::from("  Shift+Tab   Previous agent"),
        Line::from("  1-9         Select agent by number"),
        Line::from("  Arrows      Select adjacent agent"),
        Line::from(""),
        Line::from(Span::styled(
            "View",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )),
        Line::from("  E           Toggle events panel"),
        Line::from("  A           Toggle agent panel"),
        Line::from("  F           Toggle full agent details"),
        Line::from("  PageUp/Down Scroll events"),
        Line::from(""),
        Line::from("  Q           Quit"),
        Line::from("  ?           Toggle this help"),
    ];

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}
