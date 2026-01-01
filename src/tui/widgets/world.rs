//! World map widget.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use uuid::Uuid;

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
        .title(format!(" World - Day {} ", world.epoch))
        .borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build map lines
    let mut lines = Vec::new();

    for y in 0..world.height {
        let mut spans = Vec::new();

        for x in 0..world.width {
            let cell = world.get(x, y);

            // Check if there's an agent here
            let agent_here: Option<&AgentView> = agents
                .iter()
                .find(|a| a.alive && a.position == (x, y));

            // Determine territory background color based on selected agent
            let territory_bg = if let Some(cell) = cell {
                if let Some(ref territory) = cell.territory {
                    if let Some(selected_id) = selected {
                        if territory.owner_id == selected_id {
                            // Own territory - green tint
                            Some(Color::Rgb(30, 50, 30))
                        } else {
                            // Check if selected agent is a guest (we'd need guest list, but TerritoryView has guest_count)
                            // For now, show foreign territory as red tint
                            Some(Color::Rgb(50, 30, 30))
                        }
                    } else {
                        // No agent selected, show neutral territory marker
                        Some(Color::Rgb(40, 40, 50))
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
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
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
                            "LeanTo" => ('△', Color::Yellow),
                            "Shelter" => ('▲', Color::Yellow),
                            "Storage" => ('□', Color::Cyan),
                            "Workbench" => ('⚒', Color::LightBlue),
                            "Farm" => ('♠', Color::Green),
                            _ => ('■', Color::White),
                        }
                    } else {
                        // Under construction
                        ('░', Color::DarkGray)
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
                                ('*', Style::default().fg(Color::Green))
                            } else if cell.food > 0 {
                                ('*', Style::default().fg(Color::DarkGray))
                            } else {
                                ('.', Style::default().fg(Color::DarkGray))
                            }
                        }
                        Terrain::Barren => ('.', Style::default().fg(Color::Rgb(50, 50, 50))),
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
        if y < lines.len() && x * 2 < inner.width as usize {
            // Mark death location with a cross
            let first_char = agent.name.chars().next().unwrap_or('?');
            lines[y] = {
                let mut spans: Vec<Span> = lines[y].spans.clone();
                if x < spans.len() {
                    spans[x] = Span::styled(
                        format!("{} ", first_char.to_ascii_lowercase()),
                        Style::default().fg(Color::DarkGray),
                    );
                }
                Line::from(spans)
            };
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
