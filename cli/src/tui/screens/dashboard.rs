use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::tui::state::{App, Screen, Section};
use crate::tui::widgets::{detail_pane, event_list, status_bar};
use crate::tui::Action;

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Header: bandit name
    let bandit_name = app
        .current_bandit
        .as_ref()
        .map(|b| b.name.as_str())
        .unwrap_or("?");
    let header = Paragraph::new(format!(" {} — grading workbench", bandit_name))
        .style(Style::default().bold());
    f.render_widget(header, chunks[0]);

    // Main area: event list + detail pane
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(chunks[1]);

    event_list::render(f, app, main_chunks[0]);
    detail_pane::render(f, app, main_chunks[1]);

    // Status bar
    status_bar::render(f, app, chunks[2]);
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.current_bandit = None;
            app.events.clear();
            app.skipped.clear();
            Action::SwitchScreen(Screen::BanditSelect)
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if !app.events.is_empty() {
                app.event_index = (app.event_index + 1).min(app.events.len() - 1);
                app.tick_status();
            }
            Action::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if !app.events.is_empty() {
                app.event_index = app.event_index.saturating_sub(1);
                app.tick_status();
            }
            Action::None
        }
        KeyCode::Char('y') => {
            app.grade_current(1.0);
            Action::None
        }
        KeyCode::Char('n') => {
            app.grade_current(0.0);
            Action::None
        }
        KeyCode::Char('s') => {
            app.skip_current();
            Action::None
        }
        KeyCode::Char('r') => {
            app.load_events();
            Action::None
        }
        KeyCode::Char('1') => {
            app.copy_section(Section::Query);
            Action::None
        }
        KeyCode::Char('2') => {
            app.copy_section(Section::Response);
            Action::None
        }
        KeyCode::Char('3') => {
            app.copy_section(Section::Prompt);
            Action::None
        }
        KeyCode::Char('?') => Action::SwitchScreen(Screen::Help),
        _ => Action::None,
    }
}
