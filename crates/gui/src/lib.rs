//! macroquad renderer for `feral-processes-app-core::App`. Same role as
//! `feral-processes-tui`, just a different presentation: a real window with
//! colored tiles and drawn bars instead of terminal glyphs. Owns the
//! window/event loop and the mapping from macroquad's `KeyCode` to the
//! renderer-agnostic `GameKey` — the state machine itself lives in
//! `app-core` and knows nothing about macroquad.

mod render;
mod sounds;

use macroquad::prelude::*;

use feral_processes_app_core::{App, GameKey};
use sounds::SoundBank;

fn map_special_key(key: KeyCode) -> Option<GameKey> {
    match key {
        KeyCode::Up => Some(GameKey::Up),
        KeyCode::Down => Some(GameKey::Down),
        KeyCode::Left => Some(GameKey::Left),
        KeyCode::Right => Some(GameKey::Right),
        KeyCode::Enter | KeyCode::KpEnter => Some(GameKey::Enter),
        KeyCode::Escape => Some(GameKey::Esc),
        KeyCode::Backspace => Some(GameKey::Backspace),
        _ => None,
    }
}

const SPECIAL_KEYS: &[KeyCode] = &[
    KeyCode::Up,
    KeyCode::Down,
    KeyCode::Left,
    KeyCode::Right,
    KeyCode::Enter,
    KeyCode::KpEnter,
    KeyCode::Escape,
    KeyCode::Backspace,
];

fn window_conf() -> Conf {
    Conf {
        window_title: "feral-processes".to_string(),
        window_width: 1440,
        window_height: 900,
        high_dpi: true,
        ..Default::default()
    }
}

/// Runs the graphics frontend to completion (until `app.quit`). Takes `App`
/// by value — macroquad's `Window::from_config` requires a `'static`
/// future, so the loop owns the state machine outright rather than
/// borrowing it; there's nothing for a caller to do with `App` afterward
/// (the process exits either way once a frontend's loop ends), so this
/// isn't a real loss of capability, just a different shape than
/// `feral_processes_tui::run(&mut App)`.
pub fn run(app: App) {
    macroquad::Window::from_config(window_conf(), game_loop(app));
}

async fn game_loop(mut app: App) {
    let sound_bank = SoundBank::load().await;
    loop {
        for &key in SPECIAL_KEYS {
            if is_key_pressed(key)
                && let Some(game_key) = map_special_key(key)
            {
                app.handle_key(game_key);
            }
        }
        while let Some(c) = get_char_pressed() {
            if !c.is_control() {
                app.handle_key(GameKey::Char(c));
            }
        }

        for event in app.take_sounds() {
            sound_bank.play(event);
        }

        if app.quit {
            break;
        }

        render::draw(&mut app);
        next_frame().await;
    }
}
