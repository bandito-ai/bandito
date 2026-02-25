use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::tui::state::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    if app.events.is_empty() {
        let block = Block::default()
            .title(" Events ")
            .borders(Borders::ALL);
        let msg = ratatui::widgets::Paragraph::new("No ungraded events.")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem> = app
        .events
        .iter()
        .enumerate()
        .map(|(i, event)| {
            let is_skipped = app.skipped.contains(&event.uuid);
            let prefix = if i == app.event_index { ">" } else { " " };

            let label = format!(
                "{} {} / {}",
                prefix, event.model_name, event.model_provider
            );

            let detail = format!(
                "  {}  r:{}",
                format_time_ago(&event.created_at),
                event
                    .early_reward
                    .map(|r| format!("{:.2}", r))
                    .unwrap_or_else(|| "—".to_string()),
            );

            let style = if i == app.event_index {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else if is_skipped {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            let text = Text::from(vec![
                Line::from(label).style(style.bold()),
                Line::from(detail).style(if is_skipped {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Gray)
                }),
            ]);

            ListItem::new(text)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(format!(" Events ({}) ", app.events.len()))
            .borders(Borders::ALL),
    );

    f.render_widget(list, area);
}

fn format_time_ago(created_at: &str) -> String {
    // Parse ISO 8601 and compute relative time
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(created_at) {
        let now = chrono::Utc::now();
        let diff = now.signed_duration_since(dt);

        if diff.num_days() > 0 {
            format!("{}d", diff.num_days())
        } else if diff.num_hours() > 0 {
            format!("{}h", diff.num_hours())
        } else if diff.num_minutes() > 0 {
            format!("{}m", diff.num_minutes())
        } else {
            "now".to_string()
        }
    } else {
        // Fallback: try without timezone
        created_at
            .get(..16)
            .unwrap_or(created_at)
            .to_string()
    }
}
