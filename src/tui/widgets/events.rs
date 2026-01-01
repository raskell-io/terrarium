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
            EventViewType::AllyIntervened => ("⛨", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            EventViewType::Death => ("†", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            EventViewType::Gossip => ("◊", Style::default().fg(Color::LightMagenta)),
            EventViewType::GroupFormed => ("★", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            EventViewType::GroupDissolved => ("☆", Style::default().fg(Color::DarkGray)),
            EventViewType::GroupChanged => ("○", Style::default().fg(Color::Cyan)),
            EventViewType::LeadershipChanged => ("♛", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            EventViewType::RivalryFormed => ("⚔", Style::default().fg(Color::Red)),
            EventViewType::RivalryChanged => ("↔", Style::default().fg(Color::LightRed)),
            EventViewType::RivalryEnded => ("☮", Style::default().fg(Color::Green)),
            EventViewType::Courtship => ("♥", Style::default().fg(Color::LightMagenta)),
            EventViewType::Conception => ("♥", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            EventViewType::Birth => ("★", Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
            EventViewType::SkillTaught => ("✦", Style::default().fg(Color::Cyan)),
            // Crafting events
            EventViewType::MaterialGathering => ("◇", Style::default().fg(Color::Yellow)),
            EventViewType::Crafting => ("⚒", Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
            EventViewType::Hunting => ("→", Style::default().fg(Color::Red)),
            EventViewType::Fishing => ("≈", Style::default().fg(Color::Blue)),
            EventViewType::Chopping => ("¶", Style::default().fg(Color::Green)),
            EventViewType::ToolBroke => ("✗", Style::default().fg(Color::Red)),
            // Territory events
            EventViewType::TerritoryMarked => ("▣", Style::default().fg(Color::Green)),
            EventViewType::TerritoryChallenged => ("⚔", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            EventViewType::TerritorySubmitted => ("↓", Style::default().fg(Color::Cyan)),
            EventViewType::TerritoryFight => ("⚔", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            EventViewType::TerritoryLost => ("▢", Style::default().fg(Color::DarkGray)),
            // Structure events
            EventViewType::FarmProduced => ("♠", Style::default().fg(Color::Green)),
            EventViewType::StructureDestroyed => ("✗", Style::default().fg(Color::Red)),
            // Trade events
            EventViewType::TradeProposed => ("⇄", Style::default().fg(Color::Yellow)),
            EventViewType::TradeAccepted => ("✓", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            EventViewType::TradeDeclined => ("✗", Style::default().fg(Color::Red)),
            EventViewType::TradeCountered => ("↔", Style::default().fg(Color::Yellow)),
            EventViewType::TradeExpired => ("⏱", Style::default().fg(Color::DarkGray)),
            EventViewType::TradeCancelled => ("⊘", Style::default().fg(Color::DarkGray)),
            EventViewType::TradeReneged => ("!", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            EventViewType::ServiceFulfilled => ("✓", Style::default().fg(Color::Cyan)),
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
