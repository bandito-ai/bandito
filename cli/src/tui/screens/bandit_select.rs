use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};

use crate::tui::state::{App, Screen};
use crate::tui::Action;

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Title
    let title = Paragraph::new("Select a bandit")
        .style(Style::default().bold())
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(title, chunks[0]);

    // Bandit table
    if app.bandits.is_empty() {
        let msg = Paragraph::new("No bandits. Create one with `bandito create`.");
        f.render_widget(msg, chunks[1]);
    } else {
        let header = Row::new(vec!["Name", "Type", "Arms", "Pulls", "Mode"])
            .style(Style::default().bold().fg(Color::Cyan));

        let rows: Vec<Row> = app
            .bandits
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let style = if i == app.bandit_index {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };
                Row::new(vec![
                    b.name.clone(),
                    b.bandit_type.clone(),
                    b.arm_count.to_string(),
                    b.total_pulls.to_string(),
                    b.mode.clone(),
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Min(20),
                Constraint::Length(10),
                Constraint::Length(6),
                Constraint::Length(8),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::NONE));

        f.render_widget(table, chunks[1]);
    }

    // Status bar
    let status = Paragraph::new("j/k:navigate  Enter:select  r:refresh  q:quit")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.bandits.is_empty() {
                app.bandit_index = (app.bandit_index + 1).min(app.bandits.len() - 1);
            }
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if !app.bandits.is_empty() {
                app.bandit_index = app.bandit_index.saturating_sub(1);
            }
            Action::None
        }
        KeyCode::Char('r') => {
            let _ = app.load_bandits();
            Action::None
        }
        KeyCode::Enter => {
            if app.bandits.is_empty() {
                return Action::None;
            }
            let bandit = app.bandits[app.bandit_index].clone();
            app.current_bandit = Some(bandit);
            app.events.clear();
            app.event_index = 0;
            app.skipped.clear();
            app.load_events();
            Action::SwitchScreen(Screen::Dashboard)
        }
        _ => Action::None,
    }
}
