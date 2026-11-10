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

mod ambience;
mod capture;
mod effect_draw;
mod effects;
mod fx;
mod keys;
mod paint;
mod perf;
mod pointer;
mod render;
mod sounds;
mod sprite_layers;
mod sprites;
mod text;
mod wildlife;

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowMode};
use bevy_egui::egui;
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPreUpdateSet, EguiPrimaryContextPass, EguiUserTextures,
};

use ambience::Ambience;
use bevy::audio::Volume;
pub use capture::Capture;
use feral_processes_app_core::{App, GameKey, Mode, SoundEvent};
use fx::{FrameCues, Fx};
use keys::{KeyRepeat, SheetHold, TextGate};
use paint::{Color, Painter};
use sounds::{AMBIENT_BLIPS, AmbientDrone, SoundBank};
pub use sprites::install_library as install_sprite_library;

fn map_special_key(key: KeyCode) -> Option<GameKey> {
    match key {
        KeyCode::ArrowUp | KeyCode::Numpad8 => Some(GameKey::Up),
        KeyCode::ArrowDown | KeyCode::Numpad2 => Some(GameKey::Down),
        KeyCode::ArrowLeft | KeyCode::Numpad4 => Some(GameKey::Left),
        KeyCode::ArrowRight | KeyCode::Numpad6 => Some(GameKey::Right),
        // Physical key codes, so the numpad steers whether or not NumLock is
        // on — and the digit it types when it is never reaches the game
        // (`types_text`).
        KeyCode::Numpad7 => Some(GameKey::UpLeft),
        KeyCode::Numpad9 => Some(GameKey::UpRight),
        KeyCode::Numpad1 => Some(GameKey::DownLeft),
        KeyCode::Numpad3 => Some(GameKey::DownRight),
        KeyCode::Enter | KeyCode::NumpadEnter => Some(GameKey::Enter),
        KeyCode::Escape => Some(GameKey::Esc),
        KeyCode::Backspace => Some(GameKey::Backspace),
        KeyCode::Tab => Some(GameKey::Tab),
        KeyCode::PageUp => Some(GameKey::PageUp),
        KeyCode::PageDown => Some(GameKey::PageDown),
        _ => None,
    }
}

/// Whether a key's typed text reaches the game. A key `map_special_key`
/// reads is one gesture already: with NumLock on, the numpad's `1` also
/// types `'1'`, and on the map that both stepped the party and switched
/// the info tab in one press.
fn types_text(key: KeyCode) -> bool {
    map_special_key(key).is_none()
}

/// Delivered from the held state via `KeyRepeat` rather than from
/// `just_pressed`, so holding one keeps moving (and keeps scrolling, in the
/// menus that read the same keys).
/// The numpad is here for the same reason the arrows are: a held key must
/// keep walking. Two physical keys landing on one direction is deliberate —
/// holding both at once is the player's own gesture, not a double-fire the
/// list can prevent. PageUp/PageDown repeat too: paging is scrolling.
const REPEATING_KEYS: &[KeyCode] = &[
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::Numpad8,
    KeyCode::Numpad2,
    KeyCode::Numpad4,
    KeyCode::Numpad6,
    KeyCode::Numpad7,
    KeyCode::Numpad9,
    KeyCode::Numpad1,
    KeyCode::Numpad3,
    KeyCode::PageUp,
    KeyCode::PageDown,
];

/// The modifier keys, polled rather than mapped. They are not in
/// `REPEATING_KEYS` or `SPECIAL_KEYS` and produce no `GameKey` of their own —
/// holding one is a state the two horizontal arrows are read against, not a
/// press. Both physical keys of each pair count, so a left-handed Shift is
/// the same gesture as a right-handed one.
const SHIFT_KEYS: [KeyCode; 2] = [KeyCode::ShiftLeft, KeyCode::ShiftRight];
const CTRL_KEYS: [KeyCode; 2] = [KeyCode::ControlLeft, KeyCode::ControlRight];
/// Held to ask the map a question rather than to press anything — the Alt
/// marker's `reveal` flag (spec §4 "The Alt marker"). Unlike Shift and Ctrl
/// this produces no `GameKey` at all and binds no arrow: app-core never
/// hears about it, so there is nothing here for `App::handle_key` to strip
/// on a screen that doesn't want it.
const ALT_KEYS: [KeyCode; 2] = [KeyCode::AltLeft, KeyCode::AltRight];

/// Whether `FERAL_DEV_ALT` is set: holds `reveal` down for `--screenshot`,
/// whose `--keys` cannot carry a modifier. `FERAL_DEV_REVEAL`'s predicate.
fn dev_alt() -> bool {
    static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| env_on("FERAL_DEV_ALT"));
    *ON
}

/// Whether the environment variable `name` is set to something other than
/// empty or `0`.
fn env_on(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty() && v != "0")
}

/// Whether `FERAL_DEV_SHORTCUTS` is set: holds the shortcut sheet up for
/// `--screenshot`, `dev_alt`'s reason.
fn dev_shortcuts() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| env_on("FERAL_DEV_SHORTCUTS"));
    *ON
}

/// Folds held modifiers into the two horizontal arrows, and Shift into Tab
/// (the hub rule's "previous tab").
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
        (GameKey::Tab, true, _) => GameKey::ShiftTab,
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

/// How far apart a primary press and release may land, in pixels, and still
/// count as a click rather than a drag. The camera eases toward its target
/// while a travel is live (`Fx::camera_offset`), so a pointer held dead still
/// over several frames can see the tile under it drift a cell or two; a
/// tile-equality test dropped exactly the clicks that easing caused.
const CLICK_DRAG_TOLERANCE_PX: f32 = 6.0;

/// Scroll distance, in egui points, per map zoom step: egui's native
/// `line_scroll_speed`, so one notch of a line wheel is exactly one step,
/// while a touchpad's pixel deltas have to add up to the same distance.
const WHEEL_STEP_POINTS: f32 = 40.0;

/// How far down-right of the pointer the hover label sits, in pixels, so
/// the cursor does not cover its first letters.
const HOVER_OFFSET_PX: f32 = 16.0;

/// Everything the frame system carries between frames.
///
/// One resource rather than several because these are all the same thing —
/// what used to be the game loop's local variables — and splitting them up
/// would only create the chance of one being updated without the others.
#[derive(Resource)]
struct Frontend {
    app: App,
    fx: Fx,
    key_repeat: KeyRepeat,
    text_gate: TextGate,
    sheet_hold: SheetHold,
    last_mode: Mode,
    perf: perf::PerfMeter,
    pointer: pointer::Reader,
    /// `smooth_scroll_delta` not yet spent on a zoom step — see `wheel_steps`.
    wheel_acc: f32,
    ambience: Ambience,
}

/// Where a hover label of `size` goes: `offset` down-right of `cursor`,
/// pulled back so it stays inside `screen`.
fn hover_box_origin(
    cursor: (f32, f32),
    size: (f32, f32),
    screen: (f32, f32),
    offset: f32,
) -> (f32, f32) {
    let x = (cursor.0 + offset).min(screen.0 - size.0).max(0.0);
    let y = (cursor.1 + offset).min(screen.1 - size.1).max(0.0);
    (x, y)
}

/// A primary click on the world map picks the place under it — a grid
/// cell or a side-list row. A no-op off that screen, since
/// `render::world_map_hit` answers `None` there. Stands down for a gesture
/// the pointer layer's reader already read, or one click would act twice
/// (a keycap that closes a hub, then a pick on the screen it lands on).
fn handle_world_map_pointer(
    app: &mut App,
    ctx: &egui::Context,
    reader: &pointer::Reader,
    painter: &Painter,
) {
    if reader.owns_gesture() || ctx.egui_wants_pointer_input() {
        return;
    }
    let click = ctx.input(|i| {
        i.pointer
            .primary_clicked()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    let Some(pos) = click else {
        return;
    };
    match render::world_map_hit(app, painter, (pos.x, pos.y)) {
        Some(render::WorldMapHit::Chunk(chunk)) => app.pick_world_map_chunk(chunk),
        Some(render::WorldMapHit::Row(index)) => app.pick_world_map_row(index),
        None => {}
    }
}

/// Adds `delta_y` to `acc` and spends it in `WHEEL_STEP_POINTS`, returning
/// the signed number of steps (positive = wheel up = zoom in). The
/// remainder stays in `acc`, since a touchpad sends many small deltas.
///
/// Rounded, not truncated: egui smooths one notch over several frames and
/// its f32 pieces can sum to a hair under a whole notch, which `trunc`
/// dropped — leaving a near-notch debt that then ate the next notch back.
fn wheel_steps(acc: &mut f32, delta_y: f32) -> i32 {
    *acc += delta_y;
    let steps = (*acc / WHEEL_STEP_POINTS).round();
    *acc -= steps * WHEEL_STEP_POINTS;
    steps as i32
}

/// Turns the mouse wheel into `App::zoom_in`/`zoom_out`, whose own gate
/// decides whether any map is there to zoom. Over an egui widget or off the
/// map screen the half-spent remainder is dropped, so scrolling there
/// cannot help fire a step later.
///
/// Unlike `+`, which is a key and so cancels a queued walk
/// (`handle_playing_key`), the wheel calls past that: zooming out to see
/// where a travel is going should not stop it.
fn handle_wheel_zoom(app: &mut App, ctx: &egui::Context, acc: &mut f32) {
    if ctx.egui_wants_pointer_input() || app.mode != Mode::Playing {
        *acc = 0.0;
        return;
    }
    let delta_y = ctx.input(|i| i.smooth_scroll_delta().y);
    let steps = wheel_steps(acc, delta_y);
    for _ in 0..steps.unsigned_abs() {
        if steps > 0 {
            app.zoom_in();
        } else {
            app.zoom_out();
        }
    }
}

/// Draws the map's hover label: `lines` in a box beside the pointer, in
/// the perf readout's colours so the two read as the same kind of overlay.
fn draw_hover(lines: &[String], cursor: egui::Pos2, p: &Painter) {
    let m = text::ui_metrics(p.screen_h());
    let size = m.small();
    let width = lines
        .iter()
        .map(|l| p.measure_ui(l, size).width)
        .fold(0.0, f32::max);
    let box_w = width + m.inset * 2.0;
    let box_h = size as f32 + (lines.len() - 1) as f32 * m.line_height + m.inset * 2.0 + m.gap;
    let (x, y) = hover_box_origin(
        (cursor.x, cursor.y),
        (box_w, box_h),
        (p.screen_w(), p.screen_h()),
        HOVER_OFFSET_PX,
    );
    p.rect(x, y, box_w, box_h, Color::new(0.06, 0.07, 0.10, 0.85));
    for (i, line) in lines.iter().enumerate() {
        let baseline = y + m.inset + size as f32 + i as f32 * m.line_height;
        p.ui(
            line,
            x + m.inset,
            baseline,
            size,
            Color::new(0.92, 0.92, 0.92, 1.0),
        );
    }
}

/// Draws the frame-timing readout, on top of everything else.
///
/// Bottom-right rather than anywhere along the top: the status bar claims a
/// row off the top of the window, so the top edge is the one place this
/// cannot sit without hiding something the player asked to see.
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
///
/// With a `Capture`, this is the dev screenshot mode instead: a fixed-size
/// window that writes one PNG and exits — see `capture.rs`.
pub fn run(app: App, capture: Option<Capture>) -> AppExit {
    let last_mode = app.mode;
    let assets = asset_plugin(&app);
    let mut bevy_app = bevy::app::App::new();
    bevy_app
        .add_plugins(DefaultPlugins.set(assets).set(WindowPlugin {
            primary_window: Some(Window {
                title: "feral-processes".to_string(),
                resolution: match capture {
                    Some(_) => capture::resolution(),
                    None => default(),
                },
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
            key_repeat: KeyRepeat::new(),
            text_gate: TextGate::new(),
            sheet_hold: SheetHold::new(),
            last_mode,
            perf: perf::PerfMeter::new(),
            pointer: pointer::Reader::new(),
            wheel_acc: 0.0,
            ambience: Ambience::new(ambience_seed(), AMBIENT_BLIPS),
        })
        .init_resource::<sprites::Sprites>()
        .add_systems(Startup, (setup, sprites::load, effects::load))
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
        .add_systems(EguiPrimaryContextPass, (frame, sync_drone).chain());
    add_font_install(&mut bevy_app);
    match capture {
        Some(capture) => capture::install(&mut bevy_app, capture),
        None => {
            bevy_app.add_systems(Startup, maximize_window);
        }
    }
    bevy_app.run()
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

/// A different blip sequence each launch. Only the gui's ambience reads it,
/// so the wall clock here can't reach the sim or a test.
fn ambience_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// Starts, stops and re-levels the base's drone to match what `frame` just
/// decided. Its own system because `frame` is at clippy's argument limit and
/// the drone needs a query; the sink only exists from the frame after the
/// spawn, so a volume change in that one frame is picked up on the next.
fn sync_drone(
    mut commands: Commands,
    frontend: Res<Frontend>,
    sounds: Res<SoundBank>,
    mut drones: Query<(Entity, Option<&mut AudioSink>), With<AmbientDrone>>,
) {
    let volume = Volume::Linear(frontend.app.profile().volume() * ambience::DRONE_MIX);
    match (frontend.ambience.is_active(), drones.iter_mut().next()) {
        (true, None) => sounds.spawn_drone(
            &mut commands,
            frontend.app.profile().volume() * ambience::DRONE_MIX,
        ),
        (true, Some((_, Some(mut sink)))) if sink.volume() != volume => sink.set_volume(volume),
        (false, Some((entity, _))) => commands.entity(entity).despawn(),
        _ => {}
    }
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
    let fe = &mut *frontend;
    let mut painter = Painter::for_frame(
        contexts.ctx_mut()?,
        input.time.delta_secs(),
        sprites.table(),
    )
    .recording_into(fe.pointer.sink());

    fe.app.update_realtime(input.time.delta_secs());
    fe.app.advance_reveal(input.time.delta_secs());
    fe.app.advance_tactical(input.time.delta_secs());
    fe.app.advance_status(input.time.delta_secs());
    fe.app.advance_compile(input.time.delta_secs());

    let held: Vec<KeyCode> = REPEATING_KEYS
        .iter()
        .copied()
        .filter(|&key| input.keyboard.pressed(key))
        .collect();
    let shift = input.keyboard.any_pressed(SHIFT_KEYS);
    let ctrl = input.keyboard.any_pressed(CTRL_KEYS);
    let reveal = input.keyboard.any_pressed(ALT_KEYS) || dev_alt();
    for key in fe.key_repeat.tick(now, &held) {
        if let Some(game_key) = map_special_key(key) {
            fe.app.handle_key(with_modifiers(game_key, shift, ctrl));
        }
    }
    for &key in SPECIAL_KEYS {
        if input.keyboard.just_pressed(key)
            && let Some(game_key) = map_special_key(key)
        {
            fe.app.handle_key(with_modifiers(game_key, shift, false));
        }
    }
    // Letters and digits arrive as the text a keypress produced rather than
    // as physical key codes, so the bindings follow the player's keyboard
    // layout instead of assuming QWERTY positions. `TextGate` is what stops
    // the OS auto-repeat riding along that path into the screen the press
    // just opened — `block_held` below cannot reach these.
    // A release the window never saw (focus lost mid-hold) would leave the
    // sheet up and the key swallowed for good.
    fe.sheet_hold.release_if_up(|k| input.keyboard.pressed(k));
    for event in input.typed.read().filter(|e| types_text(e.key_code)) {
        let app = &mut fe.app;
        // `?` is the held shortcut sheet wherever the sheet exists; elsewhere
        // it is ordinary text (a name, a filter) and flows on as before.
        if event.state == bevy::input::ButtonState::Released {
            fe.sheet_hold.release(event.key_code);
        } else if fe.sheet_hold.swallows(event.key_code) {
            continue;
        } else if event.text.as_deref().is_some_and(|t| t.contains('?'))
            && app.shortcut_sheet().is_some()
        {
            fe.sheet_hold.press(event.key_code);
            continue;
        }
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
    {
        let ctx = contexts.ctx_mut()?;
        // Before the draw, which refills the target list this swaps out,
        // and before the older paths, which stand down on a gesture it owns.
        fe.pointer.frame(ctx, &mut fe.app);
    }
    // The settings live in the profile, which the game menu writes; the
    // frame only reads them.
    fe.fx.enabled = fe.app.profile().effects;
    // After the read, so the frame that turns effects off already holds
    // every sprite on its first cell.
    painter.set_anim_now(fe.fx.enabled.then_some(now));
    let volume = fe.app.profile().volume();

    // Held rather than played here: `frame_cues` below has to be able to
    // drop one of them, and it cannot do that once the clip is in flight.
    let queued = fe.app.take_sounds();

    if fe.app.quit {
        exit.write(AppExit::Success);
        return Ok(());
    }

    // Effects are drained every frame whether or not they'll be drawn,
    // so a disabled `Fx` can't leave the engine's queue at its cap.
    let in_battle = fx_in_fight(&fe.app);
    let at_home = fe.app.game.as_ref().is_some_and(|g| g.on_base_floor());
    let (cues, last_log) = match &mut fe.app.game {
        Some(game) => (FrameCues::take(game), game.message_log(1).pop()),
        None => (FrameCues::default(), None),
    };
    // **Before `begin_frame` consumes the vector**, and played whether or
    // not `Fx` is enabled — sound is not a visual effect.
    for event in frame_cues(
        queued,
        &cues.effects,
        &cues.tactical_fx,
        &cues.routines,
        at_home,
    ) {
        sounds.play(&mut commands, event, volume);
    }
    fe.fx.begin_frame(now, cues, in_battle);
    fe.fx.observe_log(last_log.as_ref());
    let round = fe.app.game.as_ref().and_then(|g| g.tactical_round());
    if fe.fx.observe_round(round) {
        sounds.play(&mut commands, SoundEvent::RoundEnd, volume);
    }
    // Not during a fight at the base: battle cues want the room to
    // themselves. Nor over a death there, which keeps the party's tile until
    // the game-over screen is dismissed. Nor while paused: the machines it
    // stands for are stopped too, and this also silences the drone.
    let ambient = at_home && !in_battle && fe.app.mode != Mode::GameOver && !fe.app.paused;
    if let Some(blip) = fe.ambience.tick(now, ambient) {
        sounds.play_blip(&mut commands, blip, volume * ambience::BLIP_MIX);
    }

    // Timed whether or not the readout is on: a meter fed only while it is
    // visible reports the first second after the overlay is switched on as a cold start every time.
    let started = std::time::Instant::now();
    render::draw(
        &mut fe.app,
        &mut fe.fx,
        &painter,
        reveal,
        fe.sheet_hold.held() || dev_shortcuts(),
    );
    // `draw_battle` takes them in a battle; this empties what a frame that
    // left battle queued, which no draw would ever take.
    let _ = fe.app.take_pending_impacts();
    fe.perf.sample(
        now,
        f64::from(input.time.delta_secs()),
        started.elapsed().as_secs_f64(),
    );
    fe.pointer.drawn(&fe.app);
    {
        let ctx = contexts.ctx_mut()?;
        handle_world_map_pointer(&mut fe.app, ctx, &fe.pointer, &painter);
        if let Some((cursor, tile)) = fe.pointer.hover_label() {
            let lines = fe.app.hover_lines(tile.0, tile.1);
            if !lines.is_empty() {
                draw_hover(&lines, cursor, &painter);
            }
        }
        handle_wheel_zoom(&mut fe.app, ctx, &mut fe.wheel_acc);
    }
    // Last, so nothing can be drawn over the figures.
    if fe.app.profile().perf_overlay
        && let Some(readout) = fe.perf.readout()
    {
        draw_perf(&readout.line(), &painter);
    }
    Ok(())
}

/// Whether `Fx` should keep a fight's own state — the bars' ghost trails,
/// a battle map's flashes and marks — across this frame.
///
/// **Not `Mode::is_battle` alone**: a battle map is deliberately not a
/// battle mode (that flag gates the reveal), so on it alone every hit
/// flash, heal mark and reaction mark was cleared in the frame that took it
/// in. The open fight is asked instead of a list of tactical modes, because
/// what the clear protects against is a mark outliving the board its cell
/// indexes — and that board is exactly the one `in_tactical_battle` says is
/// still open.
fn fx_in_fight(app: &App) -> bool {
    app.mode.is_battle() || app.game.as_ref().is_some_and(|g| g.in_tactical_battle())
}

/// Which cues this frame plays: what `App` queued for the key the player
/// pressed, plus whatever the engine's effect queue is carrying that has a
/// sound.
///
/// Two kinds of effect sound, and **at most one cue each however many
/// landed**. Several base beats resolve in one frame — a brawl's blows, a
/// crew's swings — and a clip per blow is a machine-gun.
///
/// The one interaction between the two sources is `Step`. A bump into
/// field rock is a swing (`Game::move_player`), but it reaches the
/// engine as `Game::move_player` like any other step, so
/// `App::after_world_action` queues `Step` for it and cannot tell the
/// difference. Dropping it here is what keeps the player's own swing from
/// sounding like a footstep with a chip on top.
///
/// What that costs is one swallowed footstep in a frame where the crew cuts
/// while the player walks somewhere else — inaudible, against app-core
/// otherwise re-deriving `move_player`'s wall branch to answer something
/// the engine already knows. A brawl deliberately does *not* suppress it:
/// two programs fighting across the base has nothing to do with the step the
/// player just took.
///
/// A third, a battle map's reaction, comes off the tactical fx queue and
/// plays at most once a frame the same way; a fourth, a routine run there,
/// comes off the `RoutineCue` queue the effects layer draws from.
///
/// **The two base kinds are heard only from the base's floor**, `at_home`
/// being `Game::on_base_floor()`: a crew keeps cutting while the player is
/// out in the field, and the din would follow them across it.
fn frame_cues(
    queued: Vec<SoundEvent>,
    effects: &[feral_processes_engine::resources::VisualEffect],
    tactical_fx: &[feral_processes_engine::TacticalFxCue],
    routines: &[feral_processes_engine::RoutineCue],
    at_home: bool,
) -> Vec<SoundEvent> {
    use feral_processes_engine::EffectKind;

    let sounded = |kind: EffectKind| at_home && effects.iter().any(|e| e.kind == kind);
    let mining = sounded(EffectKind::Mine);
    let mut cues: Vec<SoundEvent> = queued
        .into_iter()
        .filter(|cue| !(mining && *cue == SoundEvent::Step))
        .collect();
    if mining {
        cues.push(SoundEvent::Mine);
    }
    // A brawl has no clip of its own — see `EffectKind::Brawl`.
    if sounded(EffectKind::Brawl) {
        cues.push(SoundEvent::Hit);
    }
    // A battle map is not the base, so `at_home` does not gate this one.
    if tactical_fx
        .iter()
        .any(|c| c.kind == feral_processes_engine::TacticalFxKind::Reaction)
    {
        cues.push(SoundEvent::Reaction);
    }
    if !routines.is_empty() {
        cues.push(SoundEvent::Routine);
    }
    cues
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::components::GlyphColor;
    use feral_processes_engine::items::{ItemId, ids};
    use feral_processes_engine::resources::{
        EffectKind, Locale, SpeechCue, TransitCue, VisualEffect,
    };
    use feral_processes_engine::stack::{CellKind, Dir, FrameSpec, generate};
    use feral_processes_engine::{DifficultyMode, Game, save};
    use std::path::PathBuf;

    /// One notch of a line wheel is one zoom step; a touchpad's trickle of
    /// small deltas adds up to the same step rather than firing per event
    /// or never at all. Up (positive y) zooms in.
    #[test]
    fn wheel_steps_count_whole_notches_and_carry_the_remainder() {
        let mut acc = 0.0;
        assert_eq!(wheel_steps(&mut acc, WHEEL_STEP_POINTS), 1);
        assert_eq!(wheel_steps(&mut acc, -2.0 * WHEEL_STEP_POINTS), -2);
        assert_eq!(wheel_steps(&mut acc, WHEEL_STEP_POINTS * 0.3), 0);
        assert_eq!(wheel_steps(&mut acc, WHEEL_STEP_POINTS * 0.3), 1);
        assert!(
            (acc + WHEEL_STEP_POINTS * 0.4).abs() < 1e-3,
            "the remainder was not carried: {acc}"
        );
    }

    /// egui smooths one notch over several frames and its f32 pieces can
    /// sum to just under a whole notch — a step that must still fire, and
    /// must not leave a near-notch debt that eats the next notch back.
    #[test]
    fn a_notch_summing_just_short_still_steps_and_reverses_cleanly() {
        let mut acc = 0.0;
        let short = WHEEL_STEP_POINTS - 4e-6;
        let steps: i32 = [0.37, 0.25, 0.2, 0.18]
            .iter()
            .map(|f| wheel_steps(&mut acc, short * f))
            .sum();
        assert_eq!(steps, 1, "a notch 4e-6 short was dropped");
        assert_eq!(
            wheel_steps(&mut acc, -short),
            -1,
            "the reverse notch was eaten"
        );
    }

    /// Down-right of the cursor where it fits, pushed back on screen where
    /// it would not.
    #[test]
    fn hover_box_stays_on_screen() {
        let screen = (800.0, 600.0);
        let size = (100.0, 40.0);
        let (x, y) = hover_box_origin((10.0, 10.0), size, screen, 16.0);
        assert_eq!((x, y), (26.0, 26.0));
        let (x, y) = hover_box_origin((790.0, 590.0), size, screen, 16.0);
        assert!(
            x + size.0 <= screen.0 && y + size.1 <= screen.1,
            "({x}, {y}) runs off screen"
        );
        assert!(x >= 0.0 && y >= 0.0);
    }

    fn effect(kind: EffectKind) -> VisualEffect {
        VisualEffect { pos: (2, 3), kind }
    }

    /// The player's bump into rock queues `Step` from app-core and a mining
    /// cue from the engine, for the one keypress. Both played, the swing
    /// sounds like a footstep with a chip laid over it.
    #[test]
    fn a_mining_cue_replaces_the_step_the_same_keypress_queued() {
        assert_eq!(
            frame_cues(
                vec![SoundEvent::Step],
                &[effect(EffectKind::Mine)],
                &[],
                &[],
                true
            ),
            vec![SoundEvent::Mine],
        );
    }

    /// However many diggers finished a cycle on this frame. A clip per swing
    /// is `EffectKind::Brawl`'s machine-gun.
    #[test]
    fn a_whole_crews_swings_are_one_cue() {
        assert_eq!(
            frame_cues(
                Vec::new(),
                &[
                    effect(EffectKind::Mine),
                    effect(EffectKind::Mine),
                    effect(EffectKind::Mine),
                ],
                &[],
                &[],
                true,
            ),
            vec![SoundEvent::Mine],
        );
    }

    /// Only `Step` is dropped, and only by mining. A frame where the player
    /// takes a hit while the crew cuts still has to report the hit — that is
    /// the cue the game spends on losing Integrity.
    #[test]
    fn a_mining_cue_silences_nothing_but_the_step() {
        let cues = frame_cues(
            vec![SoundEvent::Hit, SoundEvent::Step, SoundEvent::Victory],
            &[effect(EffectKind::Mine)],
            &[],
            &[],
            true,
        );
        assert_eq!(
            cues,
            vec![SoundEvent::Hit, SoundEvent::Victory, SoundEvent::Mine],
        );
    }

    /// Two programs fighting across the base has nothing to do with the step
    /// the player just took, so the brawl cue is additive where mining's is a
    /// replacement.
    #[test]
    fn a_brawl_sounds_alongside_the_step_rather_than_instead_of_it() {
        assert_eq!(
            frame_cues(
                vec![SoundEvent::Step],
                &[effect(EffectKind::Brawl)],
                &[],
                &[],
                true
            ),
            vec![SoundEvent::Step, SoundEvent::Hit],
        );
    }

    /// The pass-through case, and what keeps the four above from being
    /// satisfied by a function that always answers the same thing.
    #[test]
    fn a_frame_with_no_sounding_effect_plays_exactly_what_was_queued() {
        assert_eq!(
            frame_cues(
                vec![SoundEvent::Step],
                &[effect(EffectKind::Hit), effect(EffectKind::Destroyed)],
                &[],
                &[],
                true,
            ),
            vec![SoundEvent::Step],
        );
    }

    /// A reaction on a battle map has a clip of its own, heard wherever the
    /// fight is — a battle map is not the base, so `at_home` has no say.
    #[test]
    fn a_reaction_is_heard_on_a_battle_map() {
        use feral_processes_engine::{TacticalFxCue, TacticalFxKind};
        let cue = |kind| TacticalFxCue { pos: (1, 1), kind };
        assert_eq!(
            frame_cues(
                vec![SoundEvent::Hit],
                &[],
                &[cue(TacticalFxKind::Reaction), cue(TacticalFxKind::Reaction)],
                &[],
                false,
            ),
            vec![SoundEvent::Hit, SoundEvent::Reaction],
        );
        assert_eq!(
            frame_cues(
                vec![SoundEvent::Hit],
                &[],
                &[cue(TacticalFxKind::Hit(4))],
                &[],
                false
            ),
            vec![SoundEvent::Hit],
            "only a reaction has the clip"
        );
    }

    /// A routine is heard when it is run, whatever it does — a debuff, a
    /// summon or a blast over empty cells lands no swing and so queues no
    /// hit or miss of its own. Once a frame, however many were run in it.
    #[test]
    fn a_routine_is_heard_on_a_battle_map() {
        use feral_processes_engine::RoutineCue;
        let cue = || RoutineCue {
            from: (0, 0),
            aim: (3, 3),
            cells: vec![(3, 3), (3, 4)],
            color: GlyphColor::Red,
            fx: None,
        };
        assert_eq!(
            frame_cues(Vec::new(), &[], &[], &[cue(), cue()], false),
            vec![SoundEvent::Routine],
        );
        assert_eq!(
            frame_cues(vec![SoundEvent::Hit], &[], &[], &[], false),
            vec![SoundEvent::Hit],
            "no routine, no clip"
        );
    }

    /// Every effect the engine queues is the base's, so a crew cutting rock
    /// or two programs brawling at home is not heard out in the field — and
    /// the step the player just took there is theirs, not a bump into rock.
    #[test]
    fn the_base_is_not_heard_from_outside_it() {
        assert_eq!(
            frame_cues(
                vec![SoundEvent::Step],
                &[effect(EffectKind::Mine), effect(EffectKind::Brawl)],
                &[],
                &[],
                false,
            ),
            vec![SoundEvent::Step],
        );
    }

    pub(crate) fn assets_dir() -> PathBuf {
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

    /// An `App` standing in an open battle-map fight, in the mode the router
    /// puts one in — `render::tactical`'s own fixture, lifted to an `App`.
    pub(crate) fn app_in_tactical_fight() -> App {
        let assets = assets_dir();
        let tmp = std::env::temp_dir().join("feral_processes_gui_tactical_fx");
        let mut app = App::new(
            assets.clone(),
            tmp.join("saves"),
            tmp.join("log"),
            tmp.join("profile.ron"),
            tmp.join("arenas"),
            tmp.join("telemetry.jsonl"),
        );
        for seed in 0..200u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &assets).unwrap();
            let mut profile = game.profile().clone();
            profile.tactical_battles = true;
            game.install_profile(profile);
            let at = game.player_status().position;
            let Some(target) = game
                .view_entities(12, 12)
                .into_iter()
                .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
                .find(|e| (e.pos.0 - at.0).abs() + (e.pos.1 - at.1).abs() == 1)
            else {
                continue;
            };
            game.move_player(target.pos.0 - at.0, target.pos.1 - at.1);
            if game.in_tactical_battle() {
                app.game = Some(game);
                app.mode = Mode::TacticalBattle;
                return app;
            }
        }
        panic!("no seed under 200 opened a battle-map fight");
    }

    /// A battle map's own effects must outlive the frame that took them in.
    /// The flag `begin_frame` clears them on was `Mode::is_battle`, which a
    /// battle map deliberately is not — so every hit flash, heal mark and
    /// reaction mark was wiped in the call that pushed it, and the renderer's
    /// own tests, all passing `true`, never saw it.
    #[test]
    fn a_battle_map_keeps_its_own_effects_past_the_frame() {
        use feral_processes_engine::{TacticalFxCue, TacticalFxKind};
        let app = app_in_tactical_fight();
        let mut fx = Fx::new();
        let cue = TacticalFxCue {
            pos: (2, 3),
            kind: TacticalFxKind::Hit(4),
        };
        fx.begin_frame(
            0.0,
            FrameCues {
                tactical_fx: vec![cue],
                ..Default::default()
            },
            fx_in_fight(&app),
        );
        assert!(
            fx.tactical_tile_flash((2, 3)).is_some(),
            "the hit's flash was cleared in the frame that queued it"
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

    /// An `App` with a Home deployed and the party standing on it, reached
    /// through `place_structure` itself.
    ///
    /// The materials are stocked by editing the save, the same trick and for
    /// the same reason as `app_underground`'s locale: a Home costs five Core
    /// Fragments, a fresh run carries none, and the engine deliberately
    /// hands nothing to a caller outside the crate. Everything after that is
    /// the real path, so the fixture cannot manufacture a state the game
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

        game.place_structure("home", 0, 0, None)
            .expect("the fixture stocked the Home's cost");
        app.game = Some(game);
        app
    }

    /// The readout is chrome painted over a live screen, so the two ways it
    /// can be wrong are being absent and being somewhere it hides something.
    ///
    /// The corner is the half worth pinning: the top of the window carries
    /// the status bar, so a later edit moving these
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
        let in_battle = fx_in_fight(app);
        let (cues, last_log) = match &mut app.game {
            Some(game) => (FrameCues::take(game), game.message_log(1).pop()),
            None => (FrameCues::default(), None),
        };
        fx.begin_frame(now, cues, in_battle);
        fx.observe_log(last_log.as_ref());
        paint::with_painter(|p| render::draw(app, fx, p, false, false));
    }

    /// A raid's tile flash washes the tile it names while the party is home.
    #[test]
    fn a_raid_flash_draws_on_the_base() {
        let mut app = app_in_base(4242);
        let home = app
            .game
            .as_ref()
            .unwrap()
            .field_pos()
            .expect("on the field");

        let mut fx = Fx::new();
        fx.begin_frame(
            0.0,
            FrameCues {
                effects: vec![VisualEffect {
                    pos: home,
                    kind: EffectKind::Hit,
                }],
                ..Default::default()
            },
            false,
        );
        let flash = fx
            .tile_flash(home)
            .expect("a flash queued this frame is live");
        let (_, shapes) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p, false, false));
        assert_eq!(
            paint::painted_rect_fill_count(&shapes, flash),
            1,
            "a raid on a structure must wash its tile while the party is home"
        );
    }

    /// A squad's departure walk is drawn by the interpolating pass.
    #[test]
    fn a_departure_walk_draws_on_the_base() {
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
            fx.begin_frame(
                0.0,
                FrameCues {
                    transits: vec![cue()],
                    ..Default::default()
                },
                false,
            );
            // A frame in, so the body is off its first cell and being drawn
            // by the interpolating pass rather than sitting on a tile.
            fx.begin_frame(0.06, FrameCues::default(), false);
            let (_, shapes) = paint::with_painter(|p| render::draw(app, &mut fx, p, false, false));
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
    }

    /// A speech mark draws over its cell. Counted against a control frame,
    /// because the quote glyph is not one nothing else draws on the map.
    #[test]
    fn a_speech_mark_draws_on_the_base() {
        let quotes = |app: &mut App, speech: Vec<SpeechCue>| {
            let mut fx = Fx::new();
            fx.begin_frame(
                0.0,
                FrameCues {
                    speech,
                    ..Default::default()
                },
                false,
            );
            let (_, shapes) = paint::with_painter(|p| render::draw(app, &mut fx, p, false, false));
            let mark = fx::SPEECH_GLYPH.to_string();
            paint::painted_text(&shapes)
                .iter()
                .filter(|t| **t == mark)
                .count()
        };
        let cue = || vec![SpeechCue { cell: (0, 0) }];

        let mut app = app_in_base(4247);
        let bare = quotes(&mut app, Vec::new());
        assert_eq!(
            quotes(&mut app, cue()),
            bare + 1,
            "a conversation in the base is marked"
        );
    }

    /// Cutting tools are a mode the player leaves armed and then forgets,
    /// and the only thing that ever showed it was the line in the log at
    /// the moment it was toggled. So the party's own tile wears a ring
    /// while the bump cuts.
    #[test]
    fn armed_cutting_tools_ring_the_party() {
        let mut app = app_in_base(4245);
        let mut fx = Fx::new();

        let (_, away) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p, false, false));
        assert_eq!(
            paint::painted_rect_stroke_count(&away, render::CUTTING_OUTLINE),
            0,
            "tools away must ring nothing at all"
        );

        assert!(
            app.game.as_mut().unwrap().toggle_mining(),
            "the fixture starts with the tools away"
        );
        let (_, out) = paint::with_painter(|p| render::draw(&mut app, &mut fx, p, false, false));
        assert_eq!(
            paint::painted_rect_stroke_count(&out, render::CUTTING_OUTLINE),
            1,
            "armed tools must ring the party's tile, and only theirs"
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
        assert_eq!(with_modifiers(GameKey::Tab, true, false), GameKey::ShiftTab);
        assert_eq!(with_modifiers(GameKey::Tab, false, true), GameKey::Tab);
        for untouched in [
            GameKey::Up,
            GameKey::Down,
            GameKey::Enter,
            GameKey::Esc,
            GameKey::Backspace,
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

    #[test]
    fn page_keys_map_to_the_page_game_keys() {
        assert_eq!(map_special_key(KeyCode::PageUp), Some(GameKey::PageUp));
        assert_eq!(map_special_key(KeyCode::PageDown), Some(GameKey::PageDown));
    }

    /// The repeating keys must cover the eight directions (plus the two
    /// page keys).
    ///
    /// **Deliberately not a uniqueness check any more.** The arrows and the
    /// numpad's cross land on the same four `GameKey`s on purpose, so what
    /// is worth pinning is coverage: every polled key names a direction, and
    /// between them they name all eight. A missing corner is a numpad that
    /// walks seven ways, which reads at the keyboard as the eighth key being
    /// broken.
    #[test]
    fn the_repeating_keys_cover_the_eight_directions() {
        let mapped: Vec<GameKey> = REPEATING_KEYS
            .iter()
            .filter_map(|&k| map_special_key(k))
            .collect();
        assert_eq!(
            mapped.len(),
            REPEATING_KEYS.len(),
            "a repeating key went unmapped"
        );
        for direction in [
            GameKey::Up,
            GameKey::Down,
            GameKey::Left,
            GameKey::Right,
            GameKey::UpLeft,
            GameKey::UpRight,
            GameKey::DownLeft,
            GameKey::DownRight,
        ] {
            assert!(
                mapped.contains(&direction),
                "{direction:?} is not on any polled key"
            );
        }
    }

    /// A steering key never also types: NumLock's digits off the numpad's
    /// eight directions are dropped, while the top row's digits still type.
    #[test]
    fn a_steering_numpad_key_types_nothing() {
        for key in [
            KeyCode::Numpad1,
            KeyCode::Numpad2,
            KeyCode::Numpad3,
            KeyCode::Numpad4,
            KeyCode::Numpad6,
            KeyCode::Numpad7,
            KeyCode::Numpad8,
            KeyCode::Numpad9,
        ] {
            assert!(!types_text(key), "{key:?} must steer only");
        }
        assert!(
            types_text(KeyCode::Digit1),
            "the top row still types digits"
        );
    }

    /// The numpad's corners are the diagonals, and each is its own. Pinned
    /// per key rather than as a set: a transposed pair passes a census that
    /// only counts, and walks the player the wrong way.
    #[test]
    fn the_numpad_corners_are_the_four_diagonals() {
        for (key, expected) in [
            (KeyCode::Numpad7, GameKey::UpLeft),
            (KeyCode::Numpad9, GameKey::UpRight),
            (KeyCode::Numpad1, GameKey::DownLeft),
            (KeyCode::Numpad3, GameKey::DownRight),
            (KeyCode::Numpad8, GameKey::Up),
            (KeyCode::Numpad2, GameKey::Down),
            (KeyCode::Numpad4, GameKey::Left),
            (KeyCode::Numpad6, GameKey::Right),
        ] {
            assert_eq!(map_special_key(key), Some(expected), "{key:?}");
        }
    }
}
