//! Events panel widget.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::observer::{EventView, EventViewType};

/// Draw the events panel
pub fn draw(frame: &mut Frame, area: Rect, events: &[EventView], current_epoch: usize, scroll: usize) {
    let block = Block::default()
        .title(" Events ")
        .borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Group events by epoch
    let mut lines = Vec::new();
    let mut last_epoch: Option<usize> = None;

    // Show events in reverse order (most recent first)
    for event in events.iter().rev().skip(scroll) {
        // Add epoch header if changed
        if last_epoch != Some(event.epoch) {
            if last_epoch.is_some() {
                lines.push(Line::from(""));
            }
            let epoch_style = if event.epoch == current_epoch {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            };
            lines.push(Line::from(Span::styled(
                format!("Day {}", event.epoch),
                epoch_style,
            )));
            last_epoch = Some(event.epoch);
        }

        // Format event with icon
        let (icon, style) = match event.event_type {
            EventViewType::Movement => ("►", Style::default().fg(Color::Blue)),
            EventViewType::Gathering => ("◆", Style::default().fg(Color::Green)),
            EventViewType::Eating => ("♦", Style::default().fg(Color::Green)),
            EventViewType::Resting => ("♦", Style::default().fg(Color::Cyan)),
            EventViewType::Speech => ("", Style::default().fg(Color::Yellow)),
            EventViewType::Gift => ("→", Style::default().fg(Color::Magenta)),
            EventViewType::Attack => ("!", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            EventViewType::Death => ("†", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            EventViewType::Gossip => ("◊", Style::default().fg(Color::LightMagenta)),
            EventViewType::GroupFormed => ("★", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            EventViewType::GroupDissolved => ("☆", Style::default().fg(Color::DarkGray)),
            EventViewType::GroupChanged => ("○", Style::default().fg(Color::Cyan)),
            EventViewType::LeadershipChanged => ("♛", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            EventViewType::Meta => ("", Style::default().fg(Color::DarkGray)),
        };

        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(icon, style),
            Span::raw(" "),
            Span::styled(&event.description, style),
        ]);
        lines.push(line);

        // Limit lines to fit
        if lines.len() >= inner.height as usize {
            break;
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No events yet",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}
