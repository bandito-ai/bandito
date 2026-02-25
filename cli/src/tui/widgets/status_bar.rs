use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::tui::state::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let keys = "y:good  n:bad  s:skip  r:refresh  1/2/3:copy  ?:help  q:back";
    let status = if let Some(msg) = app.status_text() {
        format!(" {}  | {}", keys, msg)
    } else {
        format!(" {}", keys)
    };

    let bar = Paragraph::new(status).style(Style::default().fg(Color::DarkGray));
    f.render_widget(bar, area);
}
