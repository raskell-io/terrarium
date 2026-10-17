//! UI rendering for the TUI.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::tui::theme;
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

    // Paint the whole screen in the theme's background and text colour first
    frame.render_widget(Block::default().style(Style::default().bg(theme::BG).fg(theme::FG)), frame.area());

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

    // Draw events or trades panel (if enabled)
    if app.show_events && top_chunks.len() > 1 {
        if app.show_trades {
            draw_trades(frame, top_chunks[1], engine);
        } else {
            draw_events(frame, top_chunks[1], engine, app);
        }
    }

    // Draw agent panel (if enabled)
    if app.show_agent {
        draw_agent(frame, main_chunks[1], engine, app);
    }

    // Draw status bar
    draw_status_bar(frame, main_chunks[2], engine, app);

    // Draw help overlay if active
    if app.show_help {
        draw_help(frame, app);
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

/// Draw the trades panel
fn draw_trades(frame: &mut Frame, area: Rect, engine: &Engine) {
    let trade_view = engine.trade_views();
    widgets::trades::draw(frame, area, &trade_view);
}

/// Draw the agent panel
fn draw_agent(frame: &mut Frame, area: Rect, engine: &Engine, app: &App) {
    if let Some(id) = app.selected_agent {
        if let Some(agent_view) = engine.agent_view(id) {
            // Find if agent is in a group and if they're the leader
            let group_info = engine
                .current_groups()
                .iter()
                .find(|g| g.members.contains(&id))
                .map(|g| (g.name.as_str(), g.leader == Some(id)));
            widgets::agent::draw(frame, area, &agent_view, app.show_full_agent, group_info);
        }
    } else {
        // No agent selected
        let block = Block::default()
            .title(" No Agent Selected ")
            .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .title_style(Style::default().fg(theme::FG));
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
        Style::default().fg(theme::GREEN)
    } else {
        Style::default().fg(theme::YELLOW)
    };

    let speed_text = format!("{}ms/epoch", app.speed_ms);

    // Get environment state
    let env_state = engine.environment_state();

    // Choose phase color based on hazard level
    let phase_style = if env_state.hazard_level > 0.5 {
        Style::default().fg(theme::RED)
    } else if env_state.food_regen_modifier < 0.5 {
        Style::default().fg(theme::YELLOW)
    } else if env_state.food_regen_modifier > 1.0 {
        Style::default().fg(theme::GREEN)
    } else {
        Style::default().fg(theme::CYAN)
    };

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
        Span::styled("?", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Help"),
        Span::raw("  "),
        Span::styled(status, status_style),
        // Open-ended runs (the browser) have no last day worth showing
        Span::raw(if engine.total_epochs() > 100_000 {
            format!("  Day {}  Alive: {}  ", engine.epoch(), engine.alive_count())
        } else {
            format!("  Day {} / {}  Alive: {}  ", engine.epoch(), engine.total_epochs(), engine.alive_count())
        }),
        Span::styled(
            format!("{}", env_state.current_phase),
            phase_style,
        ),
        Span::raw(format!(
            " (Yr {})  Groups: {}  [{}]",
            env_state.cycle_number + 1,
            engine.current_groups().len(),
            speed_text,
        )),
    ]);

    let paragraph = Paragraph::new(line).style(Style::default().bg(theme::BAR).fg(theme::FG));
    frame.render_widget(paragraph, area);
}

/// Draw the help overlay
fn draw_help(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let mut help_text = vec![
        Line::from(Span::styled(
            "TERRARIUM - Keybindings",
            Style::default()
                .fg(theme::CYAN)
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
        Line::from("  T           Toggle trades panel"),
        Line::from("  A           Toggle agent panel"),
        Line::from("  F           Toggle full agent details"),
        Line::from("  PageUp/Down Scroll events"),
        Line::from(""),
    ];
    if !app.client_keys.is_empty() {
        help_text.push(Line::from(Span::styled("World", Style::default().add_modifier(Modifier::UNDERLINED))));
        for (key, what) in &app.client_keys {
            help_text.push(Line::from(format!("  {:<12}{}", key, what)));
        }
        help_text.push(Line::from(""));
    }
    if app.can_quit {
        help_text.push(Line::from("  Q           Quit"));
    }
    help_text.push(Line::from("  ?           Toggle this help"));

    // Center the help popup, sized to its contents
    let popup_width = 60.min(area.width);
    let popup_height = (help_text.len() as u16 + 2).min(area.height);
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear background
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .title_style(Style::default().fg(theme::FG))
        .style(Style::default().bg(theme::BG).fg(theme::FG));

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, popup_area);
}
