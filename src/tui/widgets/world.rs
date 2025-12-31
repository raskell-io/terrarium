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

            let (ch, style) = if let Some(agent) = agent_here {
                // Agent present
                let first_char = agent.name.chars().next().unwrap_or('?');
                let is_selected = selected == Some(agent.id);

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                };

                (first_char, style)
            } else if let Some(cell) = cell {
                // No agent, show terrain
                match cell.terrain {
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
