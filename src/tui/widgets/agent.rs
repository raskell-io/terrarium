//! Agent panel widget.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Wrap},
    Frame,
};

use crate::observer::AgentView;

/// Draw the agent panel
pub fn draw(frame: &mut Frame, area: Rect, agent: &AgentView, show_full: bool, group_name: Option<&str>) {
    let title = match group_name {
        Some(name) => format!(" {} [{}] ", agent.name, name),
        None => format!(" {} ", agent.name),
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(if agent.alive {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if !agent.alive {
        let dead_text = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "DECEASED",
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )),
        ])
        .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(dead_text, inner);
        return;
    }

    // Layout: stats on left, info on right
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(40), Constraint::Min(20)])
        .split(inner);

    // Left side: stats bars
    draw_stats(frame, chunks[0], agent);

    // Right side: personality, goal, relationships
    draw_info(frame, chunks[1], agent, show_full);
}

/// Draw the stats section (health, hunger, energy bars)
fn draw_stats(frame: &mut Frame, area: Rect, agent: &AgentView) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    // Health bar
    let health_pct = (agent.health * 100.0) as u16;
    let health_color = if health_pct > 60 {
        Color::Green
    } else if health_pct > 30 {
        Color::Yellow
    } else {
        Color::Red
    };
    let health_gauge = Gauge::default()
        .block(Block::default().title("Health"))
        .gauge_style(Style::default().fg(health_color))
        .percent(health_pct)
        .label(format!("{}%", health_pct));
    frame.render_widget(health_gauge, chunks[0]);

    // Hunger bar (inverted - low hunger is good)
    let hunger_pct = (agent.hunger * 100.0) as u16;
    let hunger_color = if hunger_pct < 30 {
        Color::Green
    } else if hunger_pct < 60 {
        Color::Yellow
    } else {
        Color::Red
    };
    let hunger_gauge = Gauge::default()
        .block(Block::default().title("Hunger"))
        .gauge_style(Style::default().fg(hunger_color))
        .percent(hunger_pct)
        .label(format!("{}%", hunger_pct));
    frame.render_widget(hunger_gauge, chunks[1]);

    // Energy bar
    let energy_pct = (agent.energy * 100.0) as u16;
    let energy_color = if energy_pct > 60 {
        Color::Cyan
    } else if energy_pct > 30 {
        Color::Yellow
    } else {
        Color::Red
    };
    let energy_gauge = Gauge::default()
        .block(Block::default().title("Energy"))
        .gauge_style(Style::default().fg(energy_color))
        .percent(energy_pct)
        .label(format!("{}%", energy_pct));
    frame.render_widget(energy_gauge, chunks[2]);

    // Position and inventory
    let info = Line::from(vec![
        Span::raw("Position: "),
        Span::styled(
            format!("({}, {})", agent.position.0, agent.position.1),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  Food: "),
        Span::styled(
            format!("{}", agent.food),
            Style::default().fg(Color::Green),
        ),
    ]);
    let info_paragraph = Paragraph::new(info);
    frame.render_widget(info_paragraph, chunks[3]);
}

/// Draw the info section (personality, goal, relationships)
fn draw_info(frame: &mut Frame, area: Rect, agent: &AgentView, show_full: bool) {
    let mut lines = Vec::new();

    // Personality
    lines.push(Line::from(Span::styled(
        &agent.personality_summary,
        Style::default().fg(Color::White),
    )));

    // Aspiration
    lines.push(Line::from(vec![
        Span::raw("Aspiration: "),
        Span::styled(
            &agent.aspiration,
            Style::default().fg(Color::Magenta),
        ),
    ]));

    // Current goal
    if let Some(goal) = &agent.current_goal {
        lines.push(Line::from(vec![
            Span::raw("Goal: "),
            Span::styled(goal, Style::default().fg(Color::Yellow)),
        ]));
    }

    lines.push(Line::from(""));

    // Relationships
    if !agent.social_beliefs.is_empty() {
        lines.push(Line::from(Span::styled(
            "Relationships:",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )));

        for belief in &agent.social_beliefs {
            let trust_hearts = trust_display(belief.trust);
            let sentiment_color = if belief.sentiment > 0.2 {
                Color::Green
            } else if belief.sentiment < -0.2 {
                Color::Red
            } else {
                Color::White
            };
            let sentiment_text = if belief.sentiment > 0.5 {
                "likes"
            } else if belief.sentiment > 0.2 {
                "friendly"
            } else if belief.sentiment < -0.5 {
                "hostile"
            } else if belief.sentiment < -0.2 {
                "wary"
            } else {
                "neutral"
            };

            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(&belief.about, Style::default().fg(Color::Cyan)),
                Span::raw(": "),
                Span::styled(trust_hearts, Style::default().fg(Color::Red)),
                Span::raw(" "),
                Span::styled(sentiment_text, Style::default().fg(sentiment_color)),
            ]));
        }
    }

    // Recent memories (if full view)
    if show_full && !agent.recent_memories.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recent:",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )));

        for memory in agent.recent_memories.iter().take(3) {
            lines.push(Line::from(Span::styled(
                format!("  {}", memory),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

/// Convert trust value to heart display
fn trust_display(trust: f64) -> String {
    let filled = ((trust + 1.0) / 2.0 * 5.0).round() as usize;
    let empty = 5 - filled;
    format!("{}{}", "♥".repeat(filled), "♡".repeat(empty))
}
