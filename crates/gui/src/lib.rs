//! macroquad renderer for `feral-processes-app-core::App`. Same role as
//! `feral-processes-tui`, just a different presentation: a real window with
//! colored tiles and drawn bars instead of terminal glyphs. Owns the
//! window/event loop and the mapping from macroquad's `KeyCode` to the
//! renderer-agnostic `GameKey` — the state machine itself lives in
//! `app-core` and knows nothing about macroquad.

mod fx;
mod render;
mod sounds;

use macroquad::prelude::*;

use feral_processes_app_core::{App, GameKey, Mode};
use fx::Fx;
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

const DEFAULT_VOLUME: f32 = 0.2;
const VOLUME_STEP: f32 = 0.1;
/// How long a readout ("Volume: NN%", "Effects: off") stays on screen
/// after the key that changed it, in seconds.
const TOAST_SECONDS: f64 = 1.5;

/// Draws a brief centered readout, on top of whatever `render::draw` just
/// drew — volume and effects are GUI-only concerns (`App` knows nothing
/// about either), so they stay local to the game loop rather than being
/// threaded through `render::draw`.
fn draw_toast(text: &str) {
    let font_size = 28.0;
    let dims = measure_text(text, None, font_size as u16, 1.0);
    let x = (screen_width() - dims.width) / 2.0;
    let y = 44.0;
    draw_rectangle(
        x - 14.0,
        y - dims.height - 10.0,
        dims.width + 28.0,
        dims.height + 22.0,
        Color::new(0.06, 0.07, 0.10, 0.85),
    );
    draw_text(text, x, y, font_size, Color::new(0.92, 0.92, 0.92, 1.0));
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
    let mut volume = DEFAULT_VOLUME;
    let mut fx = Fx::new();
    let mut toast: Option<String> = None;
    let mut toast_until = 0.0f64;
    loop {
        app.update_realtime();
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
        if is_key_pressed(KeyCode::LeftBracket) {
            volume = (volume - VOLUME_STEP).max(0.0);
            toast = Some(format!("Volume: {}%", (volume * 100.0).round() as i32));
            toast_until = get_time() + TOAST_SECONDS;
        }
        if is_key_pressed(KeyCode::RightBracket) {
            volume = (volume + VOLUME_STEP).min(1.0);
            toast = Some(format!("Volume: {}%", (volume * 100.0).round() as i32));
            toast_until = get_time() + TOAST_SECONDS;
        }
        // Backslash rather than a letter: letters reach the game through
        // `get_char_pressed` above and would collide with its bindings.
        if is_key_pressed(KeyCode::Backslash) {
            fx.enabled = !fx.enabled;
            toast = Some(format!(
                "Effects: {}",
                if fx.enabled { "on" } else { "off" }
            ));
            toast_until = get_time() + TOAST_SECONDS;
        }

        for event in app.take_sounds() {
            sound_bank.play(event, volume);
        }

        if app.quit {
            break;
        }

        // Effects are drained every frame whether or not they'll be drawn,
        // so a disabled `Fx` can't leave the engine's queue at its cap.
        let in_battle = matches!(app.mode, Mode::Battle | Mode::BattleCompanion);
        let (effects, last_log) = match &mut app.game {
            Some(game) => (game.take_effects(), game.message_log(1).pop()),
            None => (Vec::new(), None),
        };
        fx.begin_frame(get_time(), effects, in_battle);
        fx.observe_log(last_log.as_ref());

        render::draw(&mut app, &mut fx);
        if let Some(text) = &toast
            && get_time() < toast_until
        {
            draw_toast(text);
        }
        next_frame().await;
    }
}
