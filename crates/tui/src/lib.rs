//! Ratatui/crossterm renderer for `feral-processes-app-core::App`. Owns the
//! terminal event loop and the mapping from crossterm's `KeyCode` to the
//! renderer-agnostic `GameKey` — the state machine itself lives in
//! `app-core` and knows nothing about crossterm.

mod ui;

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use feral_processes_app_core::{App, GameKey};

fn map_key(code: KeyCode) -> Option<GameKey> {
    match code {
        KeyCode::Up => Some(GameKey::Up),
        KeyCode::Down => Some(GameKey::Down),
        KeyCode::Left => Some(GameKey::Left),
        KeyCode::Right => Some(GameKey::Right),
        KeyCode::Enter => Some(GameKey::Enter),
        KeyCode::Esc => Some(GameKey::Esc),
        KeyCode::Backspace => Some(GameKey::Backspace),
        KeyCode::Char(c) => Some(GameKey::Char(c)),
        _ => None,
    }
}

/// Runs the ASCII frontend to completion (until `app.quit` or an I/O
/// error), initializing and restoring the terminal itself.
pub fn run(app: &mut App) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, app);
    ratatui::restore();
    result
}

fn run_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    while !app.quit {
        terminal.draw(|f| ui::render(f, app))?;
        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && let Some(game_key) = map_key(key.code)
        {
            app.handle_key(game_key);
        }
    }
    Ok(())
}
