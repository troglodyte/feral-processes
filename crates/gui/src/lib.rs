//! macroquad renderer for `feral-processes-app-core::App` — the game's only
//! frontend: a real window with colored tiles and drawn bars. Owns the
//! window/event loop and the mapping from macroquad's `KeyCode` to the
//! renderer-agnostic `GameKey` — the state machine itself lives in
//! `app-core` and knows nothing about macroquad.

mod fx;
mod keys;
mod paint;
mod render;
mod sounds;
mod text;

use macroquad::miniquad::conf::{LinuxBackend, Platform};
use macroquad::prelude::*;

use feral_processes_app_core::{App, GameKey};
use fx::Fx;
use keys::KeyRepeat;
use paint::{Color, Painter};
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

/// Delivered from the held state via `KeyRepeat` rather than from
/// `is_key_pressed`, so holding one keeps moving (and keeps scrolling, in
/// the menus that read the same keys).
const REPEATING_KEYS: &[KeyCode] = &[KeyCode::Up, KeyCode::Down, KeyCode::Left, KeyCode::Right];

/// Edge-triggered: these commit or cancel, and a held Escape unwinding
/// several modes at once is nobody's intent.
const SPECIAL_KEYS: &[KeyCode] = &[
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
        // Both of the following exist to keep `dpi_scale()` at 1.0.
        // macroquad rasterizes a font at `font_size * dpi_scale()`, and the
        // map glyph ladder is only pixel-crisp at whole multiples of
        // unscii's native 16px cell (`text::MAP_GLYPH_NATIVE`); a
        // fractional scale pushes every step off the pixel grid, and
        // `FilterMode::Nearest` then sharpens the artifacts rather than
        // hiding them.
        //
        // Leaving `high_dpi` false is necessary but not sufficient: only
        // miniquad's Wayland backend gates `dpi_scale` on it. The X11
        // backend sets it from `Xft.dpi / 96.0` unconditionally, so under
        // the default `LinuxBackend::X11Only` a desktop at 125% scaling
        // (`Xft.dpi: 120`) turns the 16/32/48/64 ladder into 20/40/60/80.
        // Preferring Wayland avoids that; X11 machines keep the blur.
        platform: Platform {
            linux_backend: LinuxBackend::WaylandWithX11Fallback,
            ..Default::default()
        },
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
fn draw_toast(text: &str, p: &Painter) {
    let m = text::ui_metrics(p.screen_h());
    let font_size = m.title();
    let dims = p.measure_ui(text, font_size);
    let x = (p.screen_w() - dims.width) / 2.0;
    let y = m.pad + m.title() as f32;
    // The box's margins were hand-tuned (14 horizontal, 10 above the text,
    // 12 below the baseline) rather than symmetric, so they're reproduced
    // as arithmetic on pad/inset/gap instead of one uniform padding value.
    let margin_x = m.pad + m.inset - 2.0 * m.gap;
    p.rect(
        x - margin_x,
        y - dims.height - m.inset,
        dims.width + margin_x * 2.0,
        dims.height + m.inset + m.gap * 2.0,
        Color::new(0.06, 0.07, 0.10, 0.85),
    );
    p.ui(text, x, y, font_size, Color::new(0.92, 0.92, 0.92, 1.0));
}

/// Runs the graphics frontend to completion (until `app.quit`). Takes `App`
/// by value — macroquad's `Window::from_config` requires a `'static`
/// future, so the loop owns the state machine outright rather than
/// borrowing it. Nothing is lost by that: the process exits once this
/// returns, so there is nothing for a caller to do with `App` afterward.
pub fn run(app: App) {
    macroquad::Window::from_config(window_conf(), game_loop(app));
}

async fn game_loop(mut app: App) {
    let sound_bank = SoundBank::load().await;
    let mut painter = Painter::new();
    let mut volume = DEFAULT_VOLUME;
    let mut fx = Fx::new();
    let mut toast: Option<String> = None;
    let mut toast_until = 0.0f64;
    let mut key_repeat = KeyRepeat::new();
    let mut last_mode = app.mode;
    loop {
        painter.begin_frame();
        app.update_realtime();
        let held: Vec<KeyCode> = REPEATING_KEYS
            .iter()
            .copied()
            .filter(|&key| is_key_down(key))
            .collect();
        for key in key_repeat.tick(painter.time(), &held) {
            if let Some(game_key) = map_special_key(key) {
                app.handle_key(game_key);
            }
        }
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
        // A held key that changed the screen out from under itself — walking
        // into a wild creature, say — must not go on driving the screen it
        // landed on.
        if app.mode != last_mode {
            key_repeat.block_held();
            last_mode = app.mode;
        }
        if is_key_pressed(KeyCode::LeftBracket) {
            volume = (volume - VOLUME_STEP).max(0.0);
            toast = Some(format!("Volume: {}%", (volume * 100.0).round() as i32));
            toast_until = painter.time() + TOAST_SECONDS;
        }
        if is_key_pressed(KeyCode::RightBracket) {
            volume = (volume + VOLUME_STEP).min(1.0);
            toast = Some(format!("Volume: {}%", (volume * 100.0).round() as i32));
            toast_until = painter.time() + TOAST_SECONDS;
        }
        // Backslash rather than a letter: letters reach the game through
        // `get_char_pressed` above and would collide with its bindings.
        if is_key_pressed(KeyCode::Backslash) {
            fx.enabled = !fx.enabled;
            toast = Some(format!(
                "Effects: {}",
                if fx.enabled { "on" } else { "off" }
            ));
            toast_until = painter.time() + TOAST_SECONDS;
        }

        for event in app.take_sounds() {
            sound_bank.play(event, volume);
        }

        if app.quit {
            break;
        }

        // Effects are drained every frame whether or not they'll be drawn,
        // so a disabled `Fx` can't leave the engine's queue at its cap.
        let in_battle = app.mode.is_battle();
        let (effects, last_log) = match &mut app.game {
            Some(game) => (game.take_effects(), game.message_log(1).pop()),
            None => (Vec::new(), None),
        };
        fx.begin_frame(painter.time(), effects, in_battle);
        fx.observe_log(last_log.as_ref());

        render::draw(&mut app, &mut fx, &painter);
        if let Some(text) = &toast
            && painter.time() < toast_until
        {
            draw_toast(text, &painter);
        }
        next_frame().await;
    }
}
