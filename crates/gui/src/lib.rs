//! Bevy + egui renderer for `feral-processes-app-core::App` — the game's only
//! frontend: a real window with colored tiles and drawn bars. Owns the window
//! and the mapping from Bevy's `KeyCode` to the renderer-agnostic `GameKey`;
//! the state machine itself lives in `app-core` and knows nothing about
//! either library.
//!
//! The whole frontend is one Bevy system, `frame`, holding the state machine
//! in a single resource. That is deliberately not idiomatic Bevy — a Bevy app
//! would normally spread this across systems over ECS components — but the
//! sim already *is* an ECS world behind `Game`, and the architectural rule is
//! that the renderer reaches it only through that handle. Modelling the
//! frontend as a second world of components would mean either duplicating
//! game state into it or reaching past `Game`, and the second is the thing
//! that rule exists to prevent. Bevy is here for its window, input, audio and
//! render pipeline; the game loop stays a loop.

mod fx;
mod keys;
mod paint;
mod perf;
mod render;
mod sounds;
mod sprites;
mod text;

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode};
use bevy_egui::egui;
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPreUpdateSet, EguiPrimaryContextPass, EguiUserTextures,
};

use feral_processes_app_core::{App, GameKey, Mode, PointerButton, PointerHit, PointerPhase};
use fx::Fx;
use keys::{KeyRepeat, TextGate};
use paint::{Color, Painter};
use sounds::SoundBank;

fn map_special_key(key: KeyCode) -> Option<GameKey> {
    match key {
        KeyCode::ArrowUp => Some(GameKey::Up),
        KeyCode::ArrowDown => Some(GameKey::Down),
        KeyCode::ArrowLeft => Some(GameKey::Left),
        KeyCode::ArrowRight => Some(GameKey::Right),
        KeyCode::Enter | KeyCode::NumpadEnter => Some(GameKey::Enter),
        KeyCode::Escape => Some(GameKey::Esc),
        KeyCode::Backspace => Some(GameKey::Backspace),
        KeyCode::Tab => Some(GameKey::Tab),
        _ => None,
    }
}

/// Delivered from the held state via `KeyRepeat` rather than from
/// `just_pressed`, so holding one keeps moving (and keeps scrolling, in the
/// menus that read the same keys).
const REPEATING_KEYS: &[KeyCode] = &[
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
];

/// The modifier keys, polled rather than mapped. They are not in
/// `REPEATING_KEYS` or `SPECIAL_KEYS` and produce no `GameKey` of their own —
/// holding one is a state the two horizontal arrows are read against, not a
/// press. Both physical keys of each pair count, so a left-handed Shift is
/// the same gesture as a right-handed one.
const SHIFT_KEYS: [KeyCode; 2] = [KeyCode::ShiftLeft, KeyCode::ShiftRight];
const CTRL_KEYS: [KeyCode; 2] = [KeyCode::ControlLeft, KeyCode::ControlRight];

/// Folds held modifiers into the two horizontal arrows.
///
/// Up and Down are deliberately untouched: they move a cursor on every
/// screen that reads them, and there is no second meaning to give them. The
/// frontend sends the modified form unconditionally — `App::handle_key`
/// strips it again for every mode but the collect picker, so this cannot
/// make a modified arrow dead anywhere.
///
/// Shift wins a tie. Holding both is nobody's deliberate gesture, and "all"
/// is the less surprising of the two to land on by accident, being the one
/// the screen's own `[A]` already does.
fn with_modifiers(key: GameKey, shift: bool, ctrl: bool) -> GameKey {
    match (key, shift, ctrl) {
        (GameKey::Left, true, _) => GameKey::ShiftLeft,
        (GameKey::Right, true, _) => GameKey::ShiftRight,
        (GameKey::Left, false, true) => GameKey::CtrlLeft,
        (GameKey::Right, false, true) => GameKey::CtrlRight,
        (key, _, _) => key,
    }
}

/// Edge-triggered: these commit or cancel, or — Tab — toggle, and a held
/// Escape unwinding several modes at once is nobody's intent any more than
/// a held Tab flickering focus back and forth is. `ButtonInput` gives that
/// for free — see the note in `keys.rs` on why the OS auto-repeat doesn't
/// reach `just_pressed`.
const SPECIAL_KEYS: &[KeyCode] = &[
    KeyCode::Enter,
    KeyCode::NumpadEnter,
    KeyCode::Escape,
    KeyCode::Backspace,
    KeyCode::Tab,
];

const DEFAULT_VOLUME: f32 = 0.2;
const VOLUME_STEP: f32 = 0.1;
/// How long a readout ("Volume: NN%", "Effects: off") stays on screen
/// after the key that changed it, in seconds.
const TOAST_SECONDS: f64 = 1.5;

/// Everything the frame system carries between frames.
///
/// One resource rather than several because these are all the same thing —
/// what used to be the game loop's local variables — and splitting them up
/// would only create the chance of one being updated without the others.
#[derive(Resource)]
struct Frontend {
    app: App,
    fx: Fx,
    volume: f32,
    toast: Option<String>,
    toast_until: f64,
    key_repeat: KeyRepeat,
    text_gate: TextGate,
    last_mode: Mode,
    perf_on: bool,
    perf: perf::PerfMeter,
    sprite_pointer: SpritePointer,
}

/// Frame-to-frame pointer state for `Mode::SpriteEditor` — the first mouse
/// tracking anywhere in this crate, confined to that one screen by the
/// guard at `handle_sprite_pointer`'s one call site.
///
/// `active` names which button is mid-stroke, so a fresh press (`None ->
/// Some`) is `Down`, a held frame that still resolves is `Drag`, and a
/// release or a switch of buttons is `Up`. `last_hit` is where the stroke
/// last actually resolved: `App::handle_pointer` takes a `PointerHit` on
/// every phase including `Up`, so a stroke dragged off both the canvas and
/// the swatch row while still held still needs one to close with — the
/// design's own rule that leaving the rect must end the stroke rather than
/// let the next click join it, since a stroke is one undo entry.
#[derive(Default)]
struct SpritePointer {
    active: Option<PointerButton>,
    last_hit: Option<PointerHit>,
}

/// Reads the egui pointer and drives `App::handle_pointer` for the sprite
/// editor's canvas and swatch panels — the mouse's one entry point into
/// `crates/gui`, and it never sees a pixel further than this function:
/// everything past `rects.resolve` is a `PointerHit`
/// (`render::sprite_forge::cell_at`/`swatch_at`, tested headlessly).
///
/// A no-op whenever `render::sprite_editor_hit_rects` returns `None` — no
/// mode check needed here beyond that, since a session is only ever open in
/// `Mode::SpriteEditor` — and the tracker is reset in that case so a stroke
/// cannot survive into a session that opens later.
///
/// **No interpolation between frames** (M6, final review): a fast drag
/// reports only the cell under the pointer each frame, so a quick stroke at
/// 60fps can leave a dotted rather than solid line. Acceptable for a dev
/// tool — worth knowing, not worth fixing here.
fn handle_sprite_pointer(
    app: &mut App,
    ctx: &egui::Context,
    painter: &Painter,
    tracker: &mut SpritePointer,
) {
    let Some(rects) = render::sprite_editor_hit_rects(app, painter) else {
        *tracker = SpritePointer::default();
        return;
    };

    let (primary_down, secondary_down, hover) = ctx.input(|i| {
        (
            i.pointer.primary_down(),
            i.pointer.secondary_down(),
            i.pointer.hover_pos(),
        )
    });
    let now_button = if primary_down {
        Some(PointerButton::Primary)
    } else if secondary_down {
        Some(PointerButton::Secondary)
    } else {
        None
    };
    let hit = hover.and_then(|pos| rects.resolve((pos.x, pos.y)));
    if let Some(h) = hit {
        tracker.last_hit = Some(h);
    }

    match (tracker.active, now_button) {
        (None, Some(button)) => {
            // A press that starts outside both panels opens no stroke —
            // there is nothing to paint or select yet, and one may still
            // begin later if the pointer moves onto a panel while held.
            if let Some(h) = hit {
                app.handle_pointer(h, button, PointerPhase::Down);
                tracker.active = Some(button);
            }
        }
        (Some(active), Some(button)) if active == button => {
            if let Some(h) = hit {
                app.handle_pointer(h, button, PointerPhase::Drag);
            } else if let Some(last) = tracker.last_hit {
                // The pointer left both rects while held — end the stroke
                // now rather than waiting for release, so the next click
                // cannot join it into one undo entry.
                app.handle_pointer(last, active, PointerPhase::Up);
                tracker.active = None;
            }
        }
        (Some(active), _) => {
            // Release, or a different button now down — either way the
            // held stroke is over. `hit.or(tracker.last_hit)` is never
            // `None` here: `active` could only be set by a `Down` that had
            // a hit, and every hit since has been kept in `last_hit`.
            if let Some(h) = hit.or(tracker.last_hit) {
                app.handle_pointer(h, active, PointerPhase::Up);
            }
            tracker.active = None;
            tracker.last_hit = None;
        }
        (None, None) => {}
    }
}

/// Draws a brief centered readout, on top of whatever `render::draw` just
/// drew — volume and effects are GUI-only concerns (`App` knows nothing
/// about either), so they stay local to the frame system rather than being
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

/// Draws the frame-timing readout, on top of everything else.
///
/// Bottom-right rather than anywhere along the top: the stock strip claims a
/// row off the top of the window and `draw_toast` is centred on that same
/// row, so the top edge is the one place this cannot sit without hiding
/// something the player asked to see.
///
/// The box is placed from the text's *measured* width, so it cannot run off
/// the right edge the way an unmeasured row can — the trap the map's status
/// column carries. `small()` rather than the body size because this is
/// chrome, and chrome the player has to look past for most of a session.
fn draw_perf(text: &str, p: &Painter) {
    let m = text::ui_metrics(p.screen_h());
    let font_size = m.small();
    let dims = p.measure_ui(text, font_size);
    let x = p.screen_w() - dims.width - m.pad - m.inset;
    let y = p.screen_h() - m.pad;
    p.rect(
        x - m.inset,
        y - dims.height - m.inset,
        dims.width + m.inset * 2.0,
        dims.height + m.inset * 2.0,
        Color::new(0.06, 0.07, 0.10, 0.85),
    );
    p.ui(text, x, y, font_size, Color::new(0.92, 0.92, 0.92, 1.0));
}

/// Points Bevy's asset server at the content tree the launcher already
/// resolved.
///
/// Left to its default, `AssetPlugin` resolves `assets/` itself — against
/// `CARGO_MANIFEST_DIR` in a dev build and the executable's directory once
/// installed. That is a second site deciding a runtime path, which
/// `CLAUDE.md` names as the trap under `crates/launcher/src/paths.rs`: it
/// works on the build machine, works nowhere else, and nothing fails to
/// compile. `App` is already carrying the resolved value, so taking it from
/// there is what keeps the two from disagreeing.
///
/// The path is absolute, and bevy joins `file_path` onto its own base — an
/// absolute join replaces the base, which is what makes this override the
/// guess rather than sit under it.
fn asset_plugin(app: &App) -> AssetPlugin {
    AssetPlugin {
        file_path: app.assets_dir().to_string_lossy().into_owned(),
        ..default()
    }
}

/// Runs the graphics frontend to completion (until `app.quit`). Takes `App`
/// by value: the Bevy app owns it as a resource for the rest of the process,
/// and nothing is lost by that, since the process exits once this returns.
pub fn run(app: App) {
    let last_mode = app.mode;
    let assets = asset_plugin(&app);
    let mut bevy_app = bevy::app::App::new();
    bevy_app
        .add_plugins(DefaultPlugins.set(assets).set(WindowPlugin {
            primary_window: Some(Window {
                title: "feral-processes".to_string(),
                // Windowed and maximized rather than borderless fullscreen:
                // the window keeps its decorations so it can be dragged and
                // resized, and starts filling the screen anyway. Every size
                // on screen is either a fraction of the window or clamped, so
                // no resolution literal is needed and none is carried — the
                // monitor decides the starting size via `maximize_window`.
                mode: WindowMode::Windowed,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(Frontend {
            app,
            fx: Fx::new(),
            volume: DEFAULT_VOLUME,
            toast: None,
            toast_until: 0.0,
            key_repeat: KeyRepeat::new(),
            text_gate: TextGate::new(),
            last_mode,
            perf_on: false,
            perf: perf::PerfMeter::new(),
            sprite_pointer: SpritePointer::default(),
        })
        .init_resource::<sprites::Sprites>()
        .add_systems(
            Startup,
            (
                setup,
                maximize_window,
                sprites::load,
                sprites::install_library,
            ),
        )
        // In `PreUpdate` rather than the egui pass: registration needs
        // `EguiUserTextures` mutably, and the pass already holds the context.
        // It runs every frame but returns immediately once nothing is pending.
        // `drain_writes` is ordered ahead of `register` explicitly — an
        // unordered tuple gives bevy no reason to run a save's own load
        // request before `register` sees it, and the brief's promise ("a
        // save reaches the table on the same frame") needs that order, not
        // luck.
        .add_systems(
            PreUpdate,
            (
                sprites::drain_writes.before(sprites::register),
                sprites::register,
                sync_drawn_icon,
            ),
        )
        .add_systems(EguiPrimaryContextPass, frame);
    add_font_install(&mut bevy_app);
    bevy_app.run();
}

/// Keeps the player's own drawing on the GPU, one upload per change.
///
/// `PreUpdate` beside `sprites::register`, for its reason and one more:
/// registration needs `EguiUserTextures` mutably and the egui pass already
/// holds the context, and the table has to be complete before `frame`
/// clones it for the `Painter`.
///
/// Which drawing is live is a two-case question and `App::game` answers it.
/// A run in progress draws the player's own, off the component the save
/// carries. With no run the wizard's preview cell is the only thing asking,
/// so the choice it is editing *is* the answer — reading the choice
/// unconditionally instead would let a stale wizard drawing outlive the
/// wizard and follow a player who never kept one onto the map.
fn sync_drawn_icon(
    frontend: Res<Frontend>,
    mut sprites: ResMut<sprites::Sprites>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<EguiUserTextures>,
) {
    let icon = match &frontend.app.game {
        Some(game) => game.player_icon(),
        None => frontend.app.creation_choice().icon.as_ref(),
    };
    sprites.sync_drawn_icon(icon, &mut images, &mut textures);
}

/// Schedules the one-shot font install.
///
/// Shared with the test that boots the plugin headlessly, so the slot the
/// test proves is the slot production actually uses.
fn add_font_install(app: &mut bevy::app::App) {
    app.add_systems(
        PreUpdate,
        install_fonts_once
            .after(EguiPreUpdateSet::InitContexts)
            .before(EguiPreUpdateSet::BeginPass),
    );
}

/// Binds the three font families, once, before the first pass begins.
///
/// `egui::Context::set_fonts` does not bind anything itself — it stashes the
/// definitions and egui installs them at the *start of the next pass*. Doing
/// this from inside `EguiPrimaryContextPass` is therefore too late by one
/// frame: the pass is already underway, so frame 1 lays its text out against
/// families that aren't bound yet and epaint panics rather than falling back
/// (`FontDefinitions::empty()` left it nothing to fall back to).
///
/// The context does not exist until bevy_egui has seen the camera, which is
/// why this retries rather than running once in `Startup`.
fn install_fonts_once(mut contexts: EguiContexts, mut installed: Local<bool>) {
    if *installed {
        return;
    }
    if let Ok(ctx) = contexts.ctx_mut() {
        paint::install_fonts(ctx);
        *installed = true;
    }
}

/// Asks the OS to maximize the window on the first frame.
///
/// There is no `maximized` field on `Window` to set at creation — bevy_winit
/// only applies a request queued through `set_maximized`, in `Last`. The
/// winit window is created before the first `App::update`, so a `Startup`
/// system is early enough for the request to be taken the same frame; a
/// resolution literal here instead would be a guess at the monitor's size.
fn maximize_window(mut window: Single<&mut Window, With<PrimaryWindow>>) {
    window.set_maximized(true);
}

fn setup(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    // bevy_egui attaches the primary Egui context to the first camera it
    // sees, so spawning one is the whole of the context's setup.
    commands.spawn(Camera2d);
    commands.insert_resource(SoundBank::load(&mut sources));
}

/// What the frame reads about the player and the clock.
///
/// Bundled rather than passed loose because the three together push `frame`
/// past clippy's argument threshold — same reasoning as `BarGeometry` and
/// `BarStyle` in `render/bars.rs`.
#[derive(SystemParam)]
struct FrameInput<'w, 's> {
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    typed: MessageReader<'w, 's, KeyboardInput>,
    time: Res<'w, Time>,
}

/// The whole frontend for one frame: input, the engine's realtime tick, then
/// drawing.
///
/// Runs in `EguiPrimaryContextPass` rather than `Update` because everything
/// it draws goes through the egui context, which is only valid inside that
/// schedule.
fn frame(
    mut contexts: EguiContexts,
    mut frontend: ResMut<Frontend>,
    mut input: FrameInput,
    mut commands: Commands,
    sounds: Res<SoundBank>,
    sprites: Res<sprites::Sprites>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let now = input.time.elapsed_secs_f64();
    let painter = Painter::for_frame(
        contexts.ctx_mut()?,
        input.time.delta_secs(),
        sprites.table(),
    );

    let fe = &mut *frontend;
    fe.app.update_realtime();
    fe.app.advance_reveal(input.time.delta_secs());
    fe.app.advance_status(input.time.delta_secs());
    fe.app.advance_compile(input.time.delta_secs());

    let held: Vec<KeyCode> = REPEATING_KEYS
        .iter()
        .copied()
        .filter(|&key| input.keyboard.pressed(key))
        .collect();
    let shift = input.keyboard.any_pressed(SHIFT_KEYS);
    let ctrl = input.keyboard.any_pressed(CTRL_KEYS);
    for key in fe.key_repeat.tick(now, &held) {
        if let Some(game_key) = map_special_key(key) {
            fe.app.handle_key(with_modifiers(game_key, shift, ctrl));
        }
    }
    for &key in SPECIAL_KEYS {
        if input.keyboard.just_pressed(key)
            && let Some(game_key) = map_special_key(key)
        {
            fe.app.handle_key(game_key);
        }
    }
    // Letters and digits arrive as the text a keypress produced rather than
    // as physical key codes, so the bindings follow the player's keyboard
    // layout instead of assuming QWERTY positions. `TextGate` is what stops
    // the OS auto-repeat riding along that path into the screen the press
    // just opened — `block_held` below cannot reach these.
    for event in input.typed.read() {
        let app = &mut fe.app;
        fe.text_gate
            .feed(event.key_code, event.state, event.text.as_deref(), |c| {
                let before = app.mode;
                app.handle_key(GameKey::Char(c));
                app.mode != before
            });
    }
    // A held key that changed the screen out from under itself — walking
    // into a wild creature, say — must not go on driving the screen it
    // landed on.
    if fe.app.mode != fe.last_mode {
        fe.key_repeat.block_held();
        fe.last_mode = fe.app.mode;
    }
    // Confined to `Mode::SpriteEditor` by `render::sprite_editor_hit_rects`
    // returning `None` everywhere else — see `handle_sprite_pointer`'s own
    // doc comment for why no separate mode check is needed here.
    {
        let ctx = contexts.ctx_mut()?;
        handle_sprite_pointer(&mut fe.app, ctx, &painter, &mut fe.sprite_pointer);
    }
    if input.keyboard.just_pressed(KeyCode::BracketLeft) {
        fe.volume = (fe.volume - VOLUME_STEP).max(0.0);
        fe.toast = Some(format!("Volume: {}%", (fe.volume * 100.0).round() as i32));
        fe.toast_until = now + TOAST_SECONDS;
    }
    if input.keyboard.just_pressed(KeyCode::BracketRight) {
        fe.volume = (fe.volume + VOLUME_STEP).min(1.0);
        fe.toast = Some(format!("Volume: {}%", (fe.volume * 100.0).round() as i32));
        fe.toast_until = now + TOAST_SECONDS;
    }
    // Backslash rather than a letter: letters reach the game as typed text
    // above and would collide with its bindings.
    if input.keyboard.just_pressed(KeyCode::Backslash) {
        fe.fx.enabled = !fe.fx.enabled;
        fe.toast = Some(format!(
            "Effects: {}",
            if fe.fx.enabled { "on" } else { "off" }
        ));
        fe.toast_until = now + TOAST_SECONDS;
    }
    // A function key rather than a letter for the same reason backslash is
    // one, and one better: letters reach the game as typed text, and F3
    // produces no text at all, so it cannot collide with a binding on any
    // screen or any keyboard layout.
    if input.keyboard.just_pressed(KeyCode::F3) {
        fe.perf_on = !fe.perf_on;
    }

    for event in fe.app.take_sounds() {
        sounds.play(&mut commands, event, fe.volume);
    }

    if fe.app.quit {
        exit.write(AppExit::Success);
        return Ok(());
    }

    // Effects are drained every frame whether or not they'll be drawn,
    // so a disabled `Fx` can't leave the engine's queue at its cap.
    let in_battle = fe.app.mode.is_battle();
    let (effects, transits, last_log) = match &mut fe.app.game {
        Some(game) => (
            game.take_effects(),
            game.take_transits(),
            game.message_log(1).pop(),
        ),
        None => (Vec::new(), Vec::new(), None),
    };
    fe.fx.begin_frame(now, effects, transits, in_battle);
    fe.fx.observe_log(last_log.as_ref());

    // Timed whether or not the readout is on: a meter fed only while it is
    // visible reports the first second after F3 as a cold start every time.
    let started = std::time::Instant::now();
    render::draw(&mut fe.app, &mut fe.fx, &painter);
    fe.perf.sample(
        now,
        f64::from(input.time.delta_secs()),
        started.elapsed().as_secs_f64(),
    );
    if let Some(text) = &fe.toast
        && now < fe.toast_until
    {
        draw_toast(text, &painter);
    }
    // Last, so nothing can be drawn over the figures — including the toast,
    // which the volume keys can put on screen at any moment.
    if fe.perf_on
        && let Some(readout) = fe.perf.readout()
    {
        draw_perf(&readout.line(), &painter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::components::GlyphColor;
    use feral_processes_engine::items::{ItemId, ids};
    use feral_processes_engine::resources::{EffectKind, Locale, TransitCue, VisualEffect};
    use feral_processes_engine::stack::{CellKind, Dir, FrameSpec, generate};
    use feral_processes_engine::{DifficultyMode, Game, save};
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    /// An `App` carrying nothing but a known content root.
    fn app_with_assets(assets: &std::path::Path) -> App {
        let tmp = std::env::temp_dir().join("feral_processes_gui_asset_root");
        App::new(
            assets.to_path_buf(),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            tmp.join("arenas"),
            tmp.join("telemetry.jsonl"),
        )
    }

    /// Bevy resolves `AssetPlugin::file_path` against `CARGO_MANIFEST_DIR`
    /// when it is left to its own devices, which works on the build machine
    /// and nowhere else — the second-path-resolver trap `CLAUDE.md` names
    /// under `crates/launcher/src/paths.rs`. Nothing fails to compile if
    /// this regresses, so it is asserted rather than reviewed.
    #[test]
    fn the_asset_root_comes_from_the_resolved_content_tree() {
        let dir = PathBuf::from("/not/the/manifest/dir/assets");
        let app = app_with_assets(&dir);
        assert_eq!(
            asset_plugin(&app).file_path,
            dir.to_string_lossy(),
            "the asset root must come from the App's resolved path"
        );
    }

    /// An absolute `file_path` has to survive bevy's `get_base_path().join()`
    /// — it does, because joining an absolute path replaces the base — so
    /// the resolved root must actually be handed over absolute.
    #[test]
    fn the_asset_root_is_absolute() {
        let app = app_with_assets(&assets_dir().canonicalize().unwrap());
        assert!(
            std::path::Path::new(&asset_plugin(&app).file_path).is_absolute(),
            "a relative root would be re-anchored by bevy's base path"
        );
    }

    /// An `App` standing on `(x, y)` of Stack frame 1 with the given facing.
    ///
    /// Built by editing a save and reloading it, which is what
    /// `app-core`'s own Stack tests do and for the same reason: the engine
    /// deliberately exposes no way in from outside the crate, since on a
    /// real run it only ever happens by walking onto an entrance.
    fn app_underground(seed: u32, at: (i32, i32), facing: Dir) -> (App, FrameSpec) {
        // Counted, not keyed on the seed: these tests run in parallel and
        // share one, so a seed-named scratch file has two of them writing
        // and deleting the same path. `app-core`'s helper learned this
        // first.
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let seed_tag = format!(
            "{seed}_{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let assets = assets_dir();
        let tmp = std::env::temp_dir();
        let mut app = App::new(
            assets.clone(),
            tmp.join(format!("feral_processes_gui_{seed_tag}_saves")),
            tmp.join(format!("feral_processes_gui_{seed_tag}.log")),
            tmp.join(format!("feral_processes_gui_{seed_tag}_profile.ron")),
            tmp.join(format!("feral_processes_gui_{seed_tag}_arenas")),
            tmp.join(format!("feral_processes_gui_{seed_tag}_telemetry.jsonl")),
        );
        app.game = Game::new(seed, DifficultyMode::Forgiving, &assets).ok();
        app.mode = Mode::Playing;

        let path = tmp.join(format!("feral_processes_gui_{seed_tag}.sav"));
        app.game.as_mut().unwrap().save(&path).unwrap();
        let mut data = save::load_from_file(&path).unwrap();
        let spec = FrameSpec {
            world_seed: data.seed,
            entrance: data.player.position,
            depth: 1,
            frames: 2,
            tier: 1,
        };
        data.locale = Locale::Stack {
            depth: spec.depth,
            frames: spec.frames,
            x: at.0,
            y: at.1,
            facing,
            entrance: spec.entrance,
        };
        save::save_to_file(&path, &data).unwrap();
        app.game = Some(Game::load(&path, &assets).unwrap());
        let _ = std::fs::remove_file(&path);
        (app, spec)
    }

    /// The cell of `kind` a party could step onto, and the neighbour to
    /// stand on facing it.
    fn approach(spec: FrameSpec, kind: CellKind) -> Option<((i32, i32), Dir)> {
        let frame = generate(spec);
        let target = (0..frame.height)
            .flat_map(|y| (0..frame.width).map(move |x| (x, y)))
            .find(|&(x, y)| frame.cell(x, y) == kind)?;
        [Dir::North, Dir::East, Dir::South, Dir::West]
            .into_iter()
            .find_map(|dir| {
                let (dx, dy) = dir.delta();
                let from = (target.0 - dx, target.1 - dy);
                frame.walkable(from.0, from.1).then_some((from, dir))
            })
    }

    /// An `App` with a Home deployed and the party standing in base space,
    /// reached through `place_structure` and `enter_base` themselves.
    ///
    /// The materials are stocked by editing the save, the same trick and for
    /// the same reason as `app_underground`'s locale: a Home costs five Core
    /// Fragments, a fresh run carries none, and the engine deliberately
    /// hands nothing to a caller outside the crate. Everything after that is
    /// the real path in, so the fixture cannot manufacture a locale the game
    /// would never produce.
    fn app_in_base(seed: u32) -> App {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let seed_tag = format!(
            "base_{seed}_{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let assets = assets_dir();
        let tmp = std::env::temp_dir();
        let mut app = App::new(
            assets.clone(),
            tmp.join(format!("feral_processes_gui_{seed_tag}_saves")),
            tmp.join(format!("feral_processes_gui_{seed_tag}.log")),
            tmp.join(format!("feral_processes_gui_{seed_tag}_profile.ron")),
            tmp.join(format!("feral_processes_gui_{seed_tag}_arenas")),
            tmp.join(format!("feral_processes_gui_{seed_tag}_telemetry.jsonl")),
        );
        app.game = Game::new(seed, DifficultyMode::Forgiving, &assets).ok();
        app.mode = Mode::Playing;

        let path = tmp.join(format!("feral_processes_gui_{seed_tag}.sav"));
        app.game.as_mut().unwrap().save(&path).unwrap();
        let mut data = save::load_from_file(&path).unwrap();
        data.player
            .inventory
            .push((ItemId::from(ids::CORE_FRAGMENT), 20));
        save::save_to_file(&path, &data).unwrap();
        let mut game = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        game.place_structure("home", 1, 0)
            .expect("the fixture stocked the Home's cost");
        game.enter_base()
            .expect("a fresh run starts the player on the anchor");
        app.game = Some(game);
        app
    }

    /// The readout is chrome painted over a live screen, so the two ways it
    /// can be wrong are being absent and being somewhere it hides something.
    ///
    /// The corner is the half worth pinning: the top of the window carries
    /// the stock strip and the volume toast, so a later edit moving these
    /// figures up there would cover a row the player asked to see, and
    /// nothing else in the suite would notice.
    #[test]
    fn the_frame_readout_paints_in_the_bottom_right_corner() {
        let line = "58 fps   17.2 ms   peak 41.3   draw 2.1";
        let ((w, h), shapes) = paint::with_painter(|p| {
            draw_perf(line, p);
            (p.screen_w(), p.screen_h())
        });
        assert!(
            paint::painted_text(&shapes).iter().any(|t| t == line),
            "the readout painted no figures at all: {:?}",
            paint::painted_text(&shapes)
        );
        let box_rect = paint::painted_rects(&shapes)
            .into_iter()
            .find(|r| r.width() > 0.0 && r.height() > 0.0)
            .expect("the figures sit on a backing box");
        assert!(
            box_rect.min.x > w / 2.0 && box_rect.min.y > h / 2.0,
            "the readout left the bottom-right corner: {box_rect:?} in {w}x{h}"
        );
        assert!(
            box_rect.min.x >= 0.0 && box_rect.max.x <= w && box_rect.max.y <= h,
            "the readout hangs off the window: {box_rect:?} in {w}x{h}"
        );
    }

    /// One turn of the real frontend loop: drain effects the way `frame`
    /// does, then draw whatever mode the app is now in.
    fn draw_a_frame(app: &mut App, fx: &mut Fx, now: f64) {
        let in_battle = app.mode.is_battle();
        let (effects, transits, last_log) = match &mut app.game {
            Some(game) => (
                game.take_effects(),
                game.take_transits(),
                game.message_log(1).pop(),
            ),
            None => (Vec::new(), Vec::new(), None),
        };
        fx.begin_frame(now, effects, transits, in_battle);
        fx.observe_log(last_log.as_ref());
        paint::with_painter(|p| render::draw(app, fx, p));
    }

    /// The stock strip is only worth its row if it is actually on the
    /// screen. Wired in `draw_playing_base` rather than per mode, so it
    /// reaches every screen that draws the world behind it — asserted here
    /// through the real `render::draw` rather than by calling the strip
    /// itself, which would pass with the call site deleted.
    #[test]
    fn the_playing_screen_carries_the_stock_strip() {
        let mut app = app_in_base(4243);
        let mut fx = Fx::new();
        let (_, shapes) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p));
        assert!(
            paint::painted_text(&shapes)
                .iter()
                .any(|t| t.starts_with("base stock")),
            "the playing screen draws no stock strip at all"
        );
    }

    /// The whole reason the strip sits at the top of the window rather than
    /// in the log pane: `draw_popup` caps a panel at 85% of the window and
    /// centres it, so a menu buries the log pane and leaves the strip
    /// standing. A player deciding what to build is exactly who wants to
    /// know what the base is holding.
    #[test]
    fn the_stock_strip_outlives_a_menu_popup() {
        let mut app = app_in_base(4244);
        app.mode = Mode::BaseMenu;
        let mut fx = Fx::new();
        let (_, shapes) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p));
        let painted = paint::painted_text(&shapes);
        assert!(
            painted.iter().any(|t| t.starts_with("base stock")),
            "a menu buried the strip: {painted:?}"
        );
    }

    /// A raid's tile flash is a *base-space* cue and must be drawn in base
    /// space alone.
    ///
    /// Every `VisualEffect` the engine queues names a structure's tile, and
    /// a structure stands in base space — so drawing one while the pane is
    /// showing the surface paints it onto whatever unrelated ground happens
    /// to share those numbers, with no structure there to explain it. The
    /// same cross-space read `view_entities` and the spawn-point outline
    /// already refuse.
    ///
    /// Both halves put the effect on the pane's own centre tile — base
    /// space's origin in one, the player's surface `Position` in the other
    /// — so neither half can pass merely by being scrolled off-screen, and
    /// dropping the guard makes the surface half paint the flash on the
    /// party's own tile.
    #[test]
    fn a_raid_flash_draws_in_base_space_and_never_on_the_surface() {
        let mut app = app_in_base(4242);
        let in_base = app
            .game
            .as_ref()
            .unwrap()
            .base_pos()
            .expect("in base space");

        let mut fx = Fx::new();
        fx.begin_frame(
            0.0,
            vec![VisualEffect {
                pos: in_base,
                kind: EffectKind::Hit,
            }],
            Vec::new(),
            false,
        );
        let flash = fx
            .tile_flash(in_base)
            .expect("a flash queued this frame is live");
        let (_, shapes) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p));
        assert_eq!(
            paint::painted_rect_fill_count(&shapes, flash),
            1,
            "a raid on a structure must wash its tile while the party is home"
        );

        app.game.as_mut().unwrap().leave_base().unwrap();
        let on_surface = app.game.as_ref().unwrap().player_status().position;

        let mut fx = Fx::new();
        fx.begin_frame(
            0.0,
            vec![VisualEffect {
                pos: on_surface,
                kind: EffectKind::Hit,
            }],
            Vec::new(),
            false,
        );
        let flash = fx
            .tile_flash(on_surface)
            .expect("a flash queued this frame is live");
        let (_, shapes) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p));
        assert_eq!(
            paint::painted_rect_fill_count(&shapes, flash),
            0,
            "the zone map draws no structures, so it must draw no structure's raid flash \
             either — this is the tile the party is standing on"
        );

        // The wash and the debris are two draw sites reading one queue, so
        // the guard has to be on both — a rect count alone would leave the
        // sparks free to keep landing on open ground.
        let mut quiet = Fx::new();
        quiet.begin_frame(0.0, Vec::new(), Vec::new(), false);
        let (_, bare) = paint::with_painter(|p| render::draw(&mut app, &mut quiet, p));
        assert_eq!(
            paint::painted_line_count(&shapes),
            paint::painted_line_count(&bare),
            "an effect queued while the party is on the surface must change nothing \
             the pane paints, sparks included"
        );
    }

    /// A squad's departure walk is base-space too, and the gate it sits
    /// behind is the flash's.
    ///
    /// A `TransitCue` names base-space cells, so a walk drawn while the pane
    /// is showing the zone surface files a squad of glyphs across whatever
    /// unrelated ground shares those numbers — the party's own tile among
    /// them. Both halves in one test for the flash's reason: the base half
    /// alone passes against a draw pass that is never gated at all.
    #[test]
    fn a_departure_walk_draws_in_base_space_and_never_on_the_surface() {
        /// A glyph nothing on either map draws, so counting it needs no
        /// control draw to subtract.
        const MARK: char = 'Ж';
        let cue = || TransitCue {
            glyph: MARK,
            color: GlyphColor::Cyan,
            path: vec![(0, 0), (1, 0), (2, 0), (3, 0)],
        };
        let painted = |app: &mut App| {
            let mut fx = Fx::new();
            fx.begin_frame(0.0, Vec::new(), vec![cue()], false);
            // A frame in, so the body is off its first cell and being drawn
            // by the interpolating pass rather than sitting on a tile.
            fx.begin_frame(0.06, Vec::new(), Vec::new(), false);
            let (_, shapes) = paint::with_painter(|p| render::draw(app, &mut fx, p));
            paint::painted_text(&shapes)
                .iter()
                .filter(|t| t.contains(MARK))
                .count()
        };

        let mut app = app_in_base(4246);
        assert_eq!(
            painted(&mut app),
            1,
            "a body walking out of the base is drawn"
        );

        app.game.as_mut().unwrap().leave_base().unwrap();
        assert_eq!(
            painted(&mut app),
            0,
            "the zone map is not base space, so a walk across it must draw nothing"
        );
    }

    /// Cutting tools are a mode the player leaves armed and then forgets,
    /// and the only thing that ever showed it was the line in the log at
    /// the moment it was toggled. So the party's own tile wears a ring
    /// while the bump cuts.
    ///
    /// Both halves in one test for the raid flash's reason: the mode
    /// survives a walk back out through the anchor — nothing disarms it —
    /// so an ungated ring follows the party onto the zone map, where there
    /// is no rock to cut and the ring is a claim about nothing.
    #[test]
    fn armed_cutting_tools_ring_the_party_and_only_in_base_space() {
        let mut app = app_in_base(4245);
        let mut fx = Fx::new();

        let (_, away) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p));
        assert_eq!(
            paint::painted_rect_stroke_count(&away, render::CUTTING_OUTLINE),
            0,
            "tools away must ring nothing at all"
        );

        assert!(
            app.game.as_mut().unwrap().toggle_mining(),
            "the fixture starts with the tools away"
        );
        let (_, out) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p));
        assert_eq!(
            paint::painted_rect_stroke_count(&out, render::CUTTING_OUTLINE),
            1,
            "armed tools must ring the party's tile, and only theirs"
        );

        app.game.as_mut().unwrap().leave_base().unwrap();
        let (_, surface) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p));
        assert_eq!(
            paint::painted_rect_stroke_count(&surface, render::CUTTING_OUTLINE),
            0,
            "the zone map has no rock to cut, so the ring claims nothing out here"
        );
    }

    /// Changing frame is the one thing that swaps the map out from under the
    /// corner inset mid-run: the party is drawn into frame 1, falls, and the
    /// very next draw is of a frame they have seen three cells of. This
    /// walks the real loop across that boundary — engine step, effect drain
    /// and draw — because the inset is rebuilt from `Game::frame_map` every
    /// pass and a frame change is where "rebuilt" has to actually mean it.
    #[test]
    fn the_map_redraws_when_a_fall_changes_frame() {
        let (_, spec) = app_underground(16, (0, 0), Dir::North);
        let (from, facing) = approach(spec, CellKind::Fault).expect("frame 1 lays a fault");
        let (mut app, _) = app_underground(16, from, facing);

        let mut fx = Fx::new();
        draw_a_frame(&mut app, &mut fx, 0.0);

        let before = app
            .game
            .as_ref()
            .unwrap()
            .frame_map()
            .expect("a map on frame 1");
        assert_eq!(before.depth, 1);

        app.handle_key(GameKey::Up);

        let after = app
            .game
            .as_ref()
            .unwrap()
            .frame_map()
            .expect("a map on the frame fallen into");
        assert_eq!(after.depth, 2, "the fall did not change frame");
        assert_ne!(
            after.cells, before.cells,
            "the map handed to the renderer is still the frame the party left"
        );

        draw_a_frame(&mut app, &mut fx, 1.0 / 60.0);
    }

    /// The same boundary by the other route. A fall is `fall_to` and a link
    /// is `descend_to`, and they reach `enter_frame` from different sides —
    /// so the map the inset is handed after each one is worth pinning
    /// separately rather than assuming the shared spine covers both.
    #[test]
    fn the_map_redraws_when_a_link_changes_frame() {
        let (_, spec) = app_underground(16, (0, 0), Dir::North);
        let frame = generate(spec);
        let link = frame.link_down.expect("a non-bottom frame has a way down");
        let (mut app, _) = app_underground(16, link, Dir::North);

        let mut fx = Fx::new();
        draw_a_frame(&mut app, &mut fx, 0.0);
        let before = app
            .game
            .as_ref()
            .unwrap()
            .frame_map()
            .expect("a map on frame 1");

        app.handle_key(GameKey::Char('>'));

        let after = app
            .game
            .as_ref()
            .unwrap()
            .frame_map()
            .expect("a map on the frame descended into");
        assert_eq!(
            after.depth,
            before.depth + 1,
            "the link did not change frame"
        );
        draw_a_frame(&mut app, &mut fx, 1.0 / 60.0);
    }

    /// The whole frontend draws in three font families that only exist
    /// because `install_fonts` registered them, and `FontDefinitions::empty()`
    /// leaves egui nothing to fall back on — so a family that isn't bound by
    /// the time the pass runs is a panic, not blank text. It has to be bound
    /// for the *first* pass, which is what this pins: the plugin's real
    /// scheduling, the real install slot, and a measurement in every face
    /// from inside the pass, exactly as frame 1 does it.
    ///
    /// Installing from within `EguiPrimaryContextPass` — where this used to
    /// happen — fails here, because `set_fonts` only takes effect at the
    /// start of the pass after the one it is called from.
    #[test]
    fn the_first_pass_can_draw_in_every_face() {
        #[derive(Resource, Default)]
        struct Measured(bool);

        fn probe(mut contexts: EguiContexts, mut measured: ResMut<Measured>) -> Result {
            let p = Painter::for_frame(contexts.ctx_mut()?, 1.0 / 60.0, std::sync::Arc::default());
            for face in [paint::Face::Ui, paint::Face::UiBold, paint::Face::Map] {
                let dims = p.measure(face, "Integrity", 24);
                assert!(dims.width > 0.0, "{face:?} measured nothing");
            }
            // A damage line in the log mixes both UI faces on one baseline,
            // which reaches egui through `layout_job` rather than the
            // single-face path above. Drawn rather than measured, since that
            // is the call frame 1 actually makes.
            p.ui_runs(
                &[
                    paint::TextRun {
                        text: "for ",
                        bold: false,
                        color: paint::WHITE,
                    },
                    paint::TextRun {
                        text: "7",
                        bold: true,
                        color: paint::WHITE,
                    },
                ],
                0.0,
                24.0,
                24,
            );
            measured.0 = true;
            Ok(())
        }

        let mut app = bevy::app::App::new();
        // `EguiPlugin` registers its shader as an internal asset at build
        // time, so the asset plumbing has to exist even though nothing here
        // renders. Everything past that is CPU-side text layout.
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            // bevy_egui reads keyboard, gesture and IME messages every
            // PreUpdate and treats an unregistered message type as fatal, so
            // the plugins that register them have to be here. Neither opens a
            // window: `WindowPlugin` without a primary window just declares
            // the message types.
            bevy::input::InputPlugin,
            WindowPlugin {
                primary_window: None,
                exit_condition: bevy::window::ExitCondition::DontExit,
                ..default()
            },
        ))
        .init_asset::<bevy::shader::Shader>()
        .init_asset::<Image>()
        .add_plugins(EguiPlugin::default())
        .init_resource::<Measured>()
        .add_systems(EguiPrimaryContextPass, probe);
        add_font_install(&mut app);
        app.world_mut().spawn(bevy_egui::PrimaryEguiContext);

        app.update();

        assert!(
            app.world().resource::<Measured>().0,
            "the egui pass never ran, so this proved nothing"
        );
    }

    /// A key listed in either table but missing from `map_special_key` is
    /// polled every frame and silently does nothing — the sort of dead
    /// binding that only shows up when someone tries the key. Renaming the
    /// backend's key enum (as the move off macroquad did: `Up` became
    /// `ArrowUp`) is exactly when that can happen.
    #[test]
    fn every_polled_key_maps_to_a_game_key() {
        for &key in REPEATING_KEYS.iter().chain(SPECIAL_KEYS) {
            assert!(
                map_special_key(key).is_some(),
                "{key:?} is polled every frame but maps to nothing"
            );
        }
    }

    /// Shift and Ctrl reach the two horizontal arrows and nothing else.
    ///
    /// The vertical pair is the case worth pinning: Up and Down move a cursor
    /// on every screen that reads them, so promoting a modified Up to a
    /// variant nothing handles would make Shift+Up dead on all of them. The
    /// non-arrow arm covers the same hazard for Enter and Esc, which a player
    /// may well be holding Shift over by accident on the way out of a screen.
    #[test]
    fn only_the_horizontal_arrows_take_a_modifier() {
        for (held, shift, ctrl) in [
            (GameKey::Left, GameKey::ShiftLeft, GameKey::CtrlLeft),
            (GameKey::Right, GameKey::ShiftRight, GameKey::CtrlRight),
        ] {
            assert_eq!(with_modifiers(held, false, false), held);
            assert_eq!(with_modifiers(held, true, false), shift);
            assert_eq!(with_modifiers(held, false, true), ctrl);
            assert_eq!(
                with_modifiers(held, true, true),
                shift,
                "Shift wins a tie — landing on \"all\" by accident is the \
                 milder surprise, and the screen already does it with [A]"
            );
        }
        for untouched in [
            GameKey::Up,
            GameKey::Down,
            GameKey::Enter,
            GameKey::Esc,
            GameKey::Backspace,
            GameKey::Tab,
            GameKey::Char('c'),
        ] {
            assert_eq!(
                with_modifiers(untouched, true, true),
                untouched,
                "{untouched:?} must survive a held modifier unchanged"
            );
        }
    }

    /// The icon editor (Task 5) reads `GameKey::Tab` to move focus between
    /// its canvas and its palette. Pinned on its own rather than folded into
    /// `every_polled_key_maps_to_a_game_key` alone, because that census only
    /// proves *some* `GameKey` comes back — this proves it's the right one.
    #[test]
    fn tab_maps_to_game_key_tab() {
        assert_eq!(map_special_key(KeyCode::Tab), Some(GameKey::Tab));
    }

    /// `Tab` is a toggle between two panels, not a movement key — holding it
    /// must not repeat, so it belongs in `SPECIAL_KEYS` (edge-triggered) and
    /// never in `REPEATING_KEYS`. A `REPEATING_KEYS` entry would flicker
    /// focus back and forth for as long as the key stayed down.
    #[test]
    fn tab_is_not_a_repeating_key() {
        assert!(!REPEATING_KEYS.contains(&KeyCode::Tab));
    }

    /// The four repeating keys must be the four directions and nothing else:
    /// a duplicate would make one direction unreachable while another fired
    /// twice.
    #[test]
    fn the_repeating_keys_are_exactly_the_four_directions() {
        let mut mapped: Vec<GameKey> = REPEATING_KEYS
            .iter()
            .filter_map(|&k| map_special_key(k))
            .collect();
        let before = mapped.len();
        mapped.dedup();
        assert_eq!(before, REPEATING_KEYS.len(), "a direction went unmapped");
        assert_eq!(mapped.len(), 4, "two arrow keys map to the same direction");
    }
}
