use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::tui::state::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    // Split: hotkeys on the left, notification on the right
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(40)])
        .split(area);

    // Hotkey bar: styled key labels
    let key_style = Style::default().fg(Color::Yellow).bold();
    let sep_style = Style::default().fg(Color::DarkGray);
    let desc_style = Style::default().fg(Color::Gray);

    let hotkeys = Line::from(vec![
        Span::raw(" "),
        Span::styled("y", key_style),
        Span::styled(" good  ", desc_style),
        Span::styled("n", key_style),
        Span::styled(" bad  ", desc_style),
        Span::styled("s", key_style),
        Span::styled(" skip  ", desc_style),
        Span::styled("|", sep_style),
        Span::styled("  r", key_style),
        Span::styled(" refresh  ", desc_style),
        Span::styled("1/2/3", key_style),
        Span::styled(" copy  ", desc_style),
        Span::styled("|", sep_style),
        Span::styled("  ?", key_style),
        Span::styled(" help  ", desc_style),
        Span::styled("q", key_style),
        Span::styled(" back", desc_style),
    ]);
    f.render_widget(Paragraph::new(hotkeys), chunks[0]);

    // Notification / toast area (right-aligned)
    if let Some((msg, style)) = app.toast() {
        let toast = Paragraph::new(Line::from(vec![
            Span::styled(format!(" {} ", msg), style),
        ]))
        .alignment(Alignment::Right);
        f.render_widget(toast, chunks[1]);
    }
}
