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
mod render;
mod sounds;
mod text;

use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPreUpdateSet, EguiPrimaryContextPass};

use feral_processes_app_core::{App, GameKey, Mode};
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

/// Edge-triggered: these commit or cancel, and a held Escape unwinding
/// several modes at once is nobody's intent. `ButtonInput` gives that for
/// free — see the note in `keys.rs` on why the OS auto-repeat doesn't reach
/// `just_pressed`.
const SPECIAL_KEYS: &[KeyCode] = &[
    KeyCode::Enter,
    KeyCode::NumpadEnter,
    KeyCode::Escape,
    KeyCode::Backspace,
];

const WINDOW_WIDTH: u32 = 2560;
const WINDOW_HEIGHT: u32 = 1440;

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

/// Runs the graphics frontend to completion (until `app.quit`). Takes `App`
/// by value: the Bevy app owns it as a resource for the rest of the process,
/// and nothing is lost by that, since the process exits once this returns.
pub fn run(app: App) {
    let last_mode = app.mode;
    let mut bevy_app = bevy::app::App::new();
    bevy_app
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "feral-processes".to_string(),
                resolution: WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
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
        })
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, frame);
    add_font_install(&mut bevy_app);
    bevy_app.run();
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
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let now = input.time.elapsed_secs_f64();
    let painter = Painter::for_frame(contexts.ctx_mut()?, input.time.delta_secs());

    let fe = &mut *frontend;
    fe.app.update_realtime();
    fe.app.advance_reveal(input.time.delta_secs());
    fe.app.advance_status(input.time.delta_secs());

    let held: Vec<KeyCode> = REPEATING_KEYS
        .iter()
        .copied()
        .filter(|&key| input.keyboard.pressed(key))
        .collect();
    for key in fe.key_repeat.tick(now, &held) {
        if let Some(game_key) = map_special_key(key) {
            fe.app.handle_key(game_key);
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
    let (effects, last_log) = match &mut fe.app.game {
        Some(game) => (game.take_effects(), game.message_log(1).pop()),
        None => (Vec::new(), None),
    };
    fe.fx.begin_frame(now, effects, in_battle);
    fe.fx.observe_log(last_log.as_ref());

    render::draw(&mut fe.app, &mut fe.fx, &painter);
    if let Some(text) = &fe.toast
        && now < fe.toast_until
    {
        draw_toast(text, &painter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use feral_processes_engine::resources::Locale;
    use feral_processes_engine::stack::{CellKind, Dir, FrameSpec, generate};
    use feral_processes_engine::{DifficultyMode, Game, save};
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
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

    /// One turn of the real frontend loop: drain effects the way `frame`
    /// does, then draw whatever mode the app is now in.
    fn draw_a_frame(app: &mut App, fx: &mut Fx, now: f64) {
        let in_battle = app.mode.is_battle();
        let (effects, last_log) = match &mut app.game {
            Some(game) => (game.take_effects(), game.message_log(1).pop()),
            None => (Vec::new(), None),
        };
        fx.begin_frame(now, effects, in_battle);
        fx.observe_log(last_log.as_ref());
        paint::with_painter(|p| render::draw(app, fx, p));
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
            let p = Painter::for_frame(contexts.ctx_mut()?, 1.0 / 60.0);
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
