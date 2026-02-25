use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::state::{App, Screen};
use crate::tui::Action;

pub fn render(f: &mut Frame, _app: &App) {
    let block = Block::default()
        .title(" Help — press any key to close ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Grading",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("    y          Grade good (1.0)"),
        Line::from("    n          Grade bad (0.0)"),
        Line::from("    s          Skip (move to end of list)"),
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("    j / Down   Next event"),
        Line::from("    k / Up     Previous event"),
        Line::from("    r          Refresh events from cloud"),
        Line::from("    q / Esc    Back to bandit select"),
        Line::from(""),
        Line::from(Span::styled(
            "  Copy to Clipboard",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("    1          Copy user input"),
        Line::from("    2          Copy response"),
        Line::from("    3          Copy system prompt"),
        Line::from(""),
        Line::from(Span::styled(
            "  Other",
            Style::default().bold().fg(Color::Yellow),
        )),
        Line::from("    ?          This help screen"),
        Line::from("    Ctrl+C     Quit"),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });

    // Center the help overlay
    let area = centered_rect(60, 70, f.area());
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(paragraph, area);
}

pub fn handle_key(_app: &mut App, _key: KeyEvent) -> Action {
    // Any key closes help and returns to dashboard
    Action::SwitchScreen(Screen::Dashboard)
}

/// Create a centered rectangle using percentage of the parent area.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
