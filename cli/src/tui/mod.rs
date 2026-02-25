mod screens;
mod state;
mod widgets;

use anyhow::{bail, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;

use crate::config::Config;
use crate::http::HttpClient;
use crate::store::EventStore;
use state::{App, Screen};

pub fn run() -> Result<()> {
    let config = Config::load()?;
    if !config.is_configured() {
        bail!("Not configured. Run `bandito signup` or `bandito config` first.");
    }

    let http = HttpClient::from_config(&config)?;
    let store = EventStore::open()?;
    let mut app = App::new(http, store);

    // Load bandits for the initial screen
    app.load_bandits()?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = run_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| {
            match app.screen {
                Screen::BanditSelect => screens::bandit_select::render(f, app),
                Screen::Dashboard => screens::dashboard::render(f, app),
                Screen::Help => {
                    // Render dashboard underneath, then help overlay on top
                    screens::dashboard::render(f, app);
                    screens::help::render(f, app);
                }
            }
        })?;

        if let Event::Key(key) = event::read()? {
            // Global quit: Ctrl+C
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Ok(());
            }

            let action = match app.screen {
                Screen::BanditSelect => screens::bandit_select::handle_key(app, key),
                Screen::Dashboard => screens::dashboard::handle_key(app, key),
                Screen::Help => screens::help::handle_key(app, key),
            };

            match action {
                Action::None => {}
                Action::Quit => return Ok(()),
                Action::SwitchScreen(s) => app.screen = s,
            }
        }
    }
}

pub enum Action {
    None,
    Quit,
    SwitchScreen(Screen),
}
