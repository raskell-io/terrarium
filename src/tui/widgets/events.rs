//! Events panel widget.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::theme;
use crate::observer::{EventView, EventViewType};

/// Draw the events panel
pub fn draw(frame: &mut Frame, area: Rect, events: &[EventView], current_epoch: usize, scroll: usize) {
    let block = Block::default()
        .title(" Events ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER))
        .title_style(Style::default().fg(theme::FG));

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
                    .fg(theme::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(theme::MUTED)
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
            EventViewType::Movement => ("►", Style::default().fg(theme::BLUE)),
            EventViewType::Gathering => ("◆", Style::default().fg(theme::GREEN)),
            EventViewType::Eating => ("♦", Style::default().fg(theme::GREEN)),
            EventViewType::Resting => ("♦", Style::default().fg(theme::CYAN)),
            EventViewType::Speech => ("", Style::default().fg(theme::YELLOW)),
            EventViewType::Gift => ("→", Style::default().fg(theme::MAGENTA)),
            EventViewType::Attack => ("!", Style::default().fg(theme::RED).add_modifier(Modifier::BOLD)),
            EventViewType::AllyIntervened => ("⛨", Style::default().fg(theme::CYAN).add_modifier(Modifier::BOLD)),
            EventViewType::Death => ("†", Style::default().fg(theme::RED).add_modifier(Modifier::BOLD)),
            EventViewType::Gossip => ("◊", Style::default().fg(theme::LIGHT_MAGENTA)),
            EventViewType::GroupFormed => ("★", Style::default().fg(theme::CYAN).add_modifier(Modifier::BOLD)),
            EventViewType::GroupDissolved => ("☆", Style::default().fg(theme::MUTED)),
            EventViewType::GroupChanged => ("○", Style::default().fg(theme::CYAN)),
            EventViewType::LeadershipChanged => ("♛", Style::default().fg(theme::YELLOW).add_modifier(Modifier::BOLD)),
            EventViewType::RivalryFormed => ("⚔", Style::default().fg(theme::RED)),
            EventViewType::RivalryChanged => ("↔", Style::default().fg(theme::LIGHT_RED)),
            EventViewType::RivalryEnded => ("☮", Style::default().fg(theme::GREEN)),
            EventViewType::Courtship => ("♥", Style::default().fg(theme::LIGHT_MAGENTA)),
            EventViewType::Conception => ("♥", Style::default().fg(theme::MAGENTA).add_modifier(Modifier::BOLD)),
            EventViewType::Birth => ("★", Style::default().fg(theme::LIGHT_GREEN).add_modifier(Modifier::BOLD)),
            EventViewType::SkillTaught => ("✦", Style::default().fg(theme::CYAN)),
            // Crafting events
            EventViewType::MaterialGathering => ("◇", Style::default().fg(theme::YELLOW)),
            EventViewType::Crafting => ("⚒", Style::default().fg(theme::LIGHT_BLUE).add_modifier(Modifier::BOLD)),
            EventViewType::Hunting => ("→", Style::default().fg(theme::RED)),
            EventViewType::Fishing => ("≈", Style::default().fg(theme::BLUE)),
            EventViewType::Chopping => ("¶", Style::default().fg(theme::GREEN)),
            EventViewType::ToolBroke => ("✗", Style::default().fg(theme::RED)),
            // Territory events
            EventViewType::TerritoryMarked => ("▣", Style::default().fg(theme::GREEN)),
            EventViewType::TerritoryChallenged => ("⚔", Style::default().fg(theme::YELLOW).add_modifier(Modifier::BOLD)),
            EventViewType::TerritorySubmitted => ("↓", Style::default().fg(theme::CYAN)),
            EventViewType::TerritoryFight => ("⚔", Style::default().fg(theme::RED).add_modifier(Modifier::BOLD)),
            EventViewType::TerritoryLost => ("▢", Style::default().fg(theme::MUTED)),
            // Structure events
            EventViewType::Weather => ("☁", Style::default().fg(theme::CYAN)),
            EventViewType::Illness => ("✚", Style::default().fg(theme::MAGENTA)),
            EventViewType::FarmProduced => ("♠", Style::default().fg(theme::GREEN)),
            EventViewType::StructureCompleted => ("⌂", Style::default().fg(theme::GREEN).add_modifier(Modifier::BOLD)),
            EventViewType::StructureDestroyed => ("✗", Style::default().fg(theme::RED)),
            // Trade events
            EventViewType::TradeProposed => ("⇄", Style::default().fg(theme::YELLOW)),
            EventViewType::TradeAccepted => ("✓", Style::default().fg(theme::GREEN).add_modifier(Modifier::BOLD)),
            EventViewType::TradeDeclined => ("✗", Style::default().fg(theme::RED)),
            EventViewType::TradeCountered => ("↔", Style::default().fg(theme::YELLOW)),
            EventViewType::TradeExpired => ("⏱", Style::default().fg(theme::MUTED)),
            EventViewType::TradeCancelled => ("⊘", Style::default().fg(theme::MUTED)),
            EventViewType::TradeReneged => ("!", Style::default().fg(theme::RED).add_modifier(Modifier::BOLD)),
            EventViewType::ServiceFulfilled => ("✓", Style::default().fg(theme::CYAN)),
            EventViewType::Meta => ("", Style::default().fg(theme::MUTED)),
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
            Style::default().fg(theme::MUTED),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}
