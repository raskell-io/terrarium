//! World map widget.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::tui::theme;
use crate::observer::{AgentView, WorldView};
use crate::world::Terrain;

/// Draw the world map
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    world: &WorldView,
    agents: &[AgentView],
    selected: Option<Uuid>,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .title_style(Style::default().fg(theme::FG));
    let inner = block.inner(area);

    // A world bigger than the panel scrolls: show the part around the selected person
    // (each cell takes two columns)
    let cols = (inner.width as usize / 2).max(1);
    let rows = (inner.height as usize).max(1);
    let center = selected
        .and_then(|id| agents.iter().find(|a| a.id == id && a.alive))
        .map(|a| a.position)
        .unwrap_or((world.width / 2, world.height / 2));
    let window = |c: usize, span: usize, size: usize| c.saturating_sub(span / 2).min(size.saturating_sub(span));
    let (x0, y0) = (window(center.0, cols, world.width), window(center.1, rows, world.height));
    let (x1, y1) = ((x0 + cols).min(world.width), (y0 + rows).min(world.height));

    let title = if world.width > cols || world.height > rows {
        format!(" World - Day {} · {}x{}, showing ({}, {})-({}, {}) ", world.epoch, world.width, world.height, x0, y0, x1 - 1, y1 - 1)
    } else {
        format!(" World - Day {} ", world.epoch)
    };
    frame.render_widget(block.title(title), area);

    let living: HashMap<(usize, usize), &AgentView> =
        agents.iter().filter(|a| a.alive).map(|a| (a.position, a)).collect();

    // Build map lines
    let mut lines = Vec::new();

    for y in y0..y1 {
        let mut spans = Vec::new();

        for x in x0..x1 {
            let cell = world.get(x, y);

            // Check if there's an agent here
            let agent_here: Option<&AgentView> = living.get(&(x, y)).copied();

            // Determine territory background color based on selected agent
            let territory_bg = if let Some(cell) = cell {
                if let Some(ref territory) = cell.territory {
                    if let Some(selected_id) = selected {
                        if territory.owner_id == selected_id {
                            // Own territory - green tint
                            Some(theme::LAND_OWN)
                        } else {
                            // Check if selected agent is a guest (we'd need guest list, but TerritoryView has guest_count)
                            // For now, show foreign territory as red tint
                            Some(theme::LAND_FOREIGN)
                        }
                    } else {
                        // No agent selected, show neutral territory marker
                        Some(theme::LAND_NEUTRAL)
                    }
                } else {
                    None
                }
            } else {
                None
            };

            let (ch, style) = if let Some(agent) = agent_here {
                // Agent present
                let first_char = agent.name.chars().next().unwrap_or('?');
                let is_selected = selected == Some(agent.id);

                let mut style = if is_selected {
                    Style::default()
                        .fg(theme::BG)
                        .bg(theme::YELLOW)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme::WHITE).add_modifier(Modifier::BOLD)
                };

                // Apply territory background if not selected (selected has priority)
                if !is_selected {
                    if let Some(bg) = territory_bg {
                        style = style.bg(bg);
                    }
                }

                (first_char, style)
            } else if let Some(cell) = cell {
                // No agent - check for structure first
                if let Some(ref structure) = cell.structure {
                    // Show structure icon
                    let (ch, color) = if structure.is_complete {
                        // Complete structures
                        match structure.structure_type.as_str() {
                            "LeanTo" => ('△', theme::YELLOW),
                            "Shelter" => ('▲', theme::YELLOW),
                            "Storage" => ('□', theme::CYAN),
                            "Workbench" => ('⚒', theme::LIGHT_BLUE),
                            "Farm" => ('♠', theme::GREEN),
                            _ => ('■', theme::WHITE),
                        }
                    } else {
                        // Under construction
                        ('░', theme::MUTED)
                    };
                    let mut style = Style::default().fg(color);
                    if let Some(bg) = territory_bg {
                        style = style.bg(bg);
                    }
                    (ch, style)
                } else {
                    // No structure, show terrain
                    let (ch, mut style) = match cell.terrain {
                        Terrain::Fertile => {
                            if cell.food > 10 {
                                ('*', Style::default().fg(theme::GREEN))
                            } else if cell.food > 0 {
                                ('*', Style::default().fg(theme::MUTED))
                            } else {
                                ('.', Style::default().fg(theme::MUTED))
                            }
                        }
                        Terrain::Barren => ('.', Style::default().fg(theme::GROUND)),
                        Terrain::Water => ('~', Style::default().fg(theme::BLUE)),
                    };
                    // Apply territory background
                    if let Some(bg) = territory_bg {
                        style = style.bg(bg);
                    }
                    (ch, style)
                }
            } else {
                (' ', Style::default())
            };

            spans.push(Span::styled(format!("{} ", ch), style));
        }

        lines.push(Line::from(spans));
    }

    // Check for dead agents and show them
    for agent in agents.iter().filter(|a| !a.alive) {
        let (x, y) = agent.position;
        if x < x0 || y < y0 || x >= x1 || y >= y1 {
            continue;
        }
        let (x, y) = (x - x0, y - y0);
        if y < lines.len() && x * 2 < inner.width as usize {
            // Mark death location with a cross
            let first_char = agent.name.chars().next().unwrap_or('?');
            lines[y] = {
                let mut spans: Vec<Span> = lines[y].spans.clone();
                if x < spans.len() {
                    spans[x] = Span::styled(
                        format!("{} ", first_char.to_ascii_lowercase()),
                        Style::default().fg(theme::MUTED),
                    );
                }
                Line::from(spans)
            };
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
