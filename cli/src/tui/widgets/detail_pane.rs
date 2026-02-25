use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::state::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Detail ")
        .borders(Borders::ALL);

    if app.events.is_empty() || app.event_index >= app.events.len() {
        let msg = Paragraph::new("Select an event to view details.")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, area);
        return;
    }

    let event = &app.events[app.event_index];

    let mut lines: Vec<Line> = Vec::new();

    // Model info
    lines.push(Line::from(vec![
        Span::styled(
            format!("{} / {}", event.model_name, event.model_provider),
            Style::default().bold().fg(Color::Cyan),
        ),
    ]));

    // Metrics
    let cost_str = event
        .cost
        .map(|c| format!("${:.4}", c))
        .unwrap_or_else(|| "—".to_string());
    let latency_str = event
        .latency
        .map(|l| format!("{:.0}ms", l))
        .unwrap_or_else(|| "—".to_string());
    let reward_str = event
        .early_reward
        .map(|r| format!("{:.2}", r))
        .unwrap_or_else(|| "—".to_string());

    lines.push(Line::from(format!(
        "cost: {}  latency: {}  reward: {}",
        cost_str, latency_str, reward_str
    )).style(Style::default().fg(Color::Gray)));

    lines.push(Line::from(""));

    // Query text
    lines.push(Line::from(vec![
        Span::styled("USER INPUT", Style::default().bold().fg(Color::Yellow)),
        Span::styled("  [1] copy", Style::default().fg(Color::DarkGray)),
    ]));
    if let Some(query) = &event.query_text {
        for line in query.lines() {
            lines.push(Line::from(line.to_string()));
        }
    } else {
        lines.push(Line::from("(not available)").style(Style::default().fg(Color::DarkGray)));
    }

    lines.push(Line::from(""));

    // Response
    lines.push(Line::from(vec![
        Span::styled("RESPONSE", Style::default().bold().fg(Color::Green)),
        Span::styled("  [2] copy", Style::default().fg(Color::DarkGray)),
    ]));
    if let Some(response) = &event.response {
        for line in response.lines() {
            lines.push(Line::from(line.to_string()));
        }
    } else {
        lines.push(Line::from("(not available)").style(Style::default().fg(Color::DarkGray)));
    }

    lines.push(Line::from(""));

    // System prompt
    lines.push(Line::from(vec![
        Span::styled("SYSTEM PROMPT", Style::default().bold().fg(Color::Magenta)),
        Span::styled("  [3] copy", Style::default().fg(Color::DarkGray)),
    ]));
    if let Some(prompt) = &event.system_prompt {
        for line in prompt.lines() {
            lines.push(Line::from(line.to_string()));
        }
    } else {
        lines.push(Line::from("(not available)").style(Style::default().fg(Color::DarkGray)));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}
