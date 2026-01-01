//! Trades panel widget.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::observer::TradeStateView;

/// Draw the trades panel
pub fn draw(frame: &mut Frame, area: Rect, trade_view: &TradeStateView) {
    let block = Block::default()
        .title(" Trades & Obligations ")
        .borders(Borders::ALL);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Pending proposals section
    if !trade_view.pending_proposals.is_empty() {
        lines.push(Line::from(Span::styled(
            "Pending Trade Offers",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));

        for proposal in &trade_view.pending_proposals {
            let expiry_style = if proposal.expires_in <= 1 {
                Style::default().fg(Color::Red)
            } else if proposal.expires_in <= 3 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            lines.push(Line::from(vec![
                Span::styled("  ⇄ ", Style::default().fg(Color::Yellow)),
                Span::styled(&proposal.proposer_name, Style::default().fg(Color::Cyan)),
                Span::raw(" → "),
                Span::styled(&proposal.recipient_name, Style::default().fg(Color::Cyan)),
            ]));

            lines.push(Line::from(vec![
                Span::raw("    Offers: "),
                Span::styled(&proposal.offering, Style::default().fg(Color::Green)),
            ]));

            lines.push(Line::from(vec![
                Span::raw("    Wants:  "),
                Span::styled(&proposal.requesting, Style::default().fg(Color::Magenta)),
                Span::styled(
                    format!("  ({}d)", proposal.expires_in),
                    expiry_style,
                ),
            ]));

            lines.push(Line::from(""));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "No pending trade offers",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
    }

    // Alliances section
    let alliances: Vec<_> = trade_view
        .service_debts
        .iter()
        .filter(|d| d.is_alliance)
        .collect();

    if !alliances.is_empty() {
        lines.push(Line::from(Span::styled(
            "Active Alliances",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));

        for alliance in &alliances {
            let duration_str = alliance
                .deadline_in
                .map(|d| {
                    if d > 0 {
                        format!("{}d left", d)
                    } else {
                        "expiring".to_string()
                    }
                })
                .unwrap_or_else(|| "permanent".to_string());

            lines.push(Line::from(vec![
                Span::styled("  ⛨ ", Style::default().fg(Color::Cyan)),
                Span::styled(&alliance.debtor_name, Style::default().fg(Color::Cyan)),
                Span::raw(" ↔ "),
                Span::styled(&alliance.creditor_name, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("  ({})", duration_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Service debts section (non-alliance)
    let debts: Vec<_> = trade_view
        .service_debts
        .iter()
        .filter(|d| !d.is_alliance)
        .collect();

    if !debts.is_empty() {
        lines.push(Line::from(Span::styled(
            "Service Obligations",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        )));

        for debt in debts {
            let deadline_style = match debt.deadline_in {
                Some(d) if d < 0 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                Some(d) if d <= 3 => Style::default().fg(Color::Yellow),
                _ => Style::default().fg(Color::DarkGray),
            };

            let deadline_str = debt
                .deadline_in
                .map(|d| {
                    if d < 0 {
                        format!("OVERDUE by {}d", -d)
                    } else if d == 0 {
                        "due today".to_string()
                    } else {
                        format!("{}d left", d)
                    }
                })
                .unwrap_or_else(|| "no deadline".to_string());

            lines.push(Line::from(vec![
                Span::styled("  → ", Style::default().fg(Color::Magenta)),
                Span::styled(&debt.debtor_name, Style::default().fg(Color::Cyan)),
                Span::raw(" owes "),
                Span::styled(&debt.creditor_name, Style::default().fg(Color::Cyan)),
            ]));

            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(&debt.service, Style::default().fg(Color::White)),
                Span::styled(format!("  ({})", deadline_str), deadline_style),
            ]));
        }
    } else if alliances.is_empty() {
        lines.push(Line::from(Span::styled(
            "No active service obligations",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // If everything is empty
    if trade_view.pending_proposals.is_empty()
        && trade_view.service_debts.is_empty()
    {
        lines.clear();
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  No active trades or obligations",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Agents can propose trades with:",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            "  TRADE <name> OFFER <items> FOR <items>",
            Style::default().fg(Color::Yellow),
        )));
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}
