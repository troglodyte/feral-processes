//! The one reader of egui's pointer input (ADR 0046).
//!
//! A screen registers each pointer target while it draws
//! (`Painter::target`); this reader turns the frame's raw pointer into one
//! `PointerEvent` against the targets the *previous* frame drew, and hands
//! it to `App::handle_pointer_event`. It owns the frame-to-frame state — the
//! last frame's targets and the screen they were drawn under, whether the
//! pointer is armed, the gesture in progress, the wheel's remainder — and
//! decides nothing about what a hit means: there is no `match app.mode` here
//! and no hub is named. That is app-core's, and it is always a key the
//! keyboard already presses.
//!
//! **A click is a press and a release on one target with no drag between**,
//! and that is when a hub acts, so a drag that starts on a row clicks
//! nothing. **Hover runs a frame behind the draw**, so a press is dropped
//! when the screen has changed since the targets were drawn (`drawn`).
//! While a gesture that began on a target is live the older per-screen paths
//! stand down (`owns_gesture`), or one click would be read twice.

use bevy_egui::egui;
use feral_processes_app_core::{App, HubPageKey, Mode, PointerButton, PointerEvent, PointerHit};

use crate::paint::{Rect, Targets};

/// Buttons in the order a frame pressing several is read.
const BUTTONS: [(egui::PointerButton, PointerButton); 3] = [
    (egui::PointerButton::Primary, PointerButton::Primary),
    (egui::PointerButton::Secondary, PointerButton::Secondary),
    (egui::PointerButton::Middle, PointerButton::Middle),
];

/// Which screen a list of targets was drawn for: a target list is only good
/// while the app is still on it.
type Screen = (Mode, Option<HubPageKey>);

fn screen_of(app: &App) -> Screen {
    (app.mode, app.hub_page().copied())
}

/// What one frame's egui input says about the pointer, before any target
/// is looked up.
struct Raw {
    pos: Option<egui::Pos2>,
    pressed: Option<PointerButton>,
    released: bool,
    down: bool,
    moved: bool,
    scroll_y: f32,
}

impl Raw {
    fn read(ctx: &egui::Context) -> Self {
        ctx.input(|i| Raw {
            pos: i.pointer.hover_pos(),
            pressed: BUTTONS
                .into_iter()
                .find(|(b, _)| i.pointer.button_pressed(*b))
                .map(|(_, mine)| match mine {
                    // Alt+click is the middle button for a mouse with none:
                    // the sprite editor's eyedropper.
                    PointerButton::Primary if i.modifiers.alt => PointerButton::Middle,
                    other => other,
                }),
            released: BUTTONS
                .into_iter()
                .any(|(b, _)| i.pointer.button_released(b)),
            down: i.pointer.any_down(),
            moved: i.pointer.delta() != egui::Vec2::ZERO,
            scroll_y: i.smooth_scroll_delta().y,
        })
    }
}

/// A press that began on a target, until its release.
struct Gesture {
    button: PointerButton,
    target: PointerHit,
    origin: egui::Pos2,
    /// The pointer went further than a click allows.
    dragged: bool,
}

pub struct Reader {
    /// The targets the last frame drew, in draw order.
    prev: Vec<(Rect, PointerHit)>,
    /// Where this frame's draw records; swapped into `prev` next frame.
    sink: Targets,
    /// The screen `prev` was drawn under (`drawn`).
    drawn_under: Option<Screen>,
    /// Whether a frame with no button held has been seen since a screen
    /// with targets opened. A button still held when a screen opens must
    /// not press whatever it happens to be over.
    armed: bool,
    gesture: Option<Gesture>,
    /// This frame's press, release or live gesture began on a target.
    owns: bool,
    /// Wheel travel not yet spent on a whole notch.
    wheel_acc: f32,
}

impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}

impl Reader {
    pub fn new() -> Self {
        Self {
            prev: Vec::new(),
            sink: Targets::default(),
            drawn_under: None,
            armed: false,
            gesture: None,
            owns: false,
            wheel_acc: 0.0,
        }
    }

    /// Where the frame's `Painter` records its targets
    /// (`Painter::recording_into`).
    pub fn sink(&self) -> &Targets {
        &self.sink
    }

    /// Records the screen the frame's draw was for. Called after the draw: a
    /// key before the next frame's `frame` may change the screen, and the
    /// targets then describe the one the player has left.
    pub fn drawn(&mut self, app: &App) {
        self.drawn_under = Some(screen_of(app));
    }

    /// Whether a gesture that began on one of this reader's targets is live
    /// this frame, or ended on it. The older per-screen pointer paths (the
    /// map and the world map) stand down while it is: they
    /// would read the same press or release a second time.
    pub fn owns_gesture(&self) -> bool {
        self.owns
    }

    /// Reads this frame's pointer, resolves it against the targets the last
    /// frame drew, and gives `app` the event.
    pub fn frame(&mut self, ctx: &egui::Context, app: &mut App) {
        self.prev = std::mem::take(
            &mut *self
                .sink
                .lock()
                .expect("the target list is never held across a panic"),
        );
        let raw = Raw::read(ctx);
        if self.drawn_under != Some(screen_of(app)) {
            // Drawn for a screen the app has since left.
            self.prev.clear();
            self.gesture = None;
        }
        let over_egui = ctx.egui_wants_pointer_input();
        if self.prev.is_empty() {
            self.armed = false;
        } else if !raw.down {
            self.armed = true;
        }
        let hover = raw
            .pos
            .filter(|_| !over_egui)
            .and_then(|pos| topmost(&self.prev, pos));
        let wheel = if hover.is_some() {
            crate::wheel_steps(&mut self.wheel_acc, raw.scroll_y)
        } else {
            self.wheel_acc = 0.0;
            0
        };
        let (press, click) = self.gesture_step(&raw, hover);
        app.handle_pointer_event(PointerEvent {
            hover,
            press,
            drag: raw.down && raw.moved,
            up: raw.released,
            click,
            button: raw.pressed.unwrap_or_default(),
            wheel,
        });
    }

    /// Advances the gesture by one frame: `(press, click)`.
    fn gesture_step(&mut self, raw: &Raw, hover: Option<PointerHit>) -> (bool, bool) {
        let press = self.begin_gesture(raw, hover);
        if let (Some(g), Some(pos)) = (&mut self.gesture, raw.pos)
            && pos.distance(g.origin) > crate::CLICK_DRAG_TOLERANCE_PX
        {
            g.dragged = true;
        }
        let ended = self.gesture.take_if(|_| raw.released || !raw.down);
        let click = ended.as_ref().is_some_and(|g| {
            raw.released
                && !g.dragged
                && g.button == PointerButton::Primary
                && hover == Some(g.target)
        });
        self.owns = self.gesture.is_some() || press || ended.is_some();
        (press, click)
    }

    /// A press on an armed reader, over a target, starts a gesture.
    fn begin_gesture(&mut self, raw: &Raw, hover: Option<PointerHit>) -> bool {
        let (true, Some(button), Some(target), Some(origin)) =
            (self.armed, raw.pressed, hover, raw.pos)
        else {
            return false;
        };
        self.gesture = Some(Gesture {
            button,
            target,
            origin,
            dragged: false,
        });
        true
    }
}

/// The target on top at `pos`: the last drawn that contains it, so a row
/// drawn over a region's background wins.
fn topmost(targets: &[(Rect, PointerHit)], pos: egui::Pos2) -> Option<PointerHit> {
    targets
        .iter()
        .rev()
        .find(|(r, _)| pos.x >= r.x && pos.x < r.x + r.w && pos.y >= r.y && pos.y < r.y + r.h)
        .map(|(_, hit)| *hit)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::paint::Painter;
    use feral_processes_app_core::{GameKey, Mode, Region};
    use std::path::PathBuf;
    use std::sync::Arc;

    pub(crate) fn new_app(seed: u32) -> App {
        let tmp = std::env::temp_dir().join(format!("feral_processes_gui_pointer_{seed}"));
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let mut app = App::new(
            assets.clone(),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            tmp.join("arenas"),
            tmp.join("telemetry.jsonl"),
        );
        app.game = feral_processes_engine::Game::new(
            seed,
            feral_processes_engine::DifficultyMode::Forgiving,
            &assets,
        )
        .ok();
        app.mode = Mode::Playing;
        // Leaving a hub shows whatever notice the run had waiting.
        app.handle_key(GameKey::Esc);
        app.handle_key(GameKey::Esc);
        while app.mode == Mode::Notification {
            app.handle_key(GameKey::Enter);
        }
        app.mode = Mode::Playing;
        app
    }

    /// An app on the base report's first page with the cursor in it: a hub
    /// whose Enter does something visible.
    fn app_in_a_hub(seed: u32) -> App {
        let mut app = new_app(seed);
        app.handle_key(GameKey::Char('b'));
        app.handle_key(GameKey::Right);
        app
    }

    const ROW0: Rect = Rect::new(100.0, 100.0, 200.0, 20.0);
    const ROW1: Rect = Rect::new(100.0, 120.0, 200.0, 20.0);
    const PAGE: Rect = Rect::new(0.0, 0.0, 800.0, 600.0);
    const KEYCAP: Rect = Rect::new(10.0, 570.0, 60.0, 20.0);

    /// What a draw would register: a page background with two rows and a
    /// keycap over it.
    fn register(reader: &Reader) {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {
            let painter =
                Painter::for_frame(&ctx, 0.0, Arc::default()).recording_into(reader.sink());
            painter.target(PAGE, PointerHit::Region(Region::Page));
            painter.target(ROW0, PointerHit::Row(0));
            painter.target(ROW1, PointerHit::Row(1));
            painter.target(KEYCAP, PointerHit::Key(GameKey::Down));
        });
    }

    #[derive(Clone, Copy)]
    pub(crate) struct Frame {
        pub(crate) pos: egui::Pos2,
        press: bool,
        release: bool,
        scroll: f32,
        pub(crate) time: f64,
    }

    impl Frame {
        pub(crate) fn at(x: f32, y: f32, time: f64) -> Self {
            Frame {
                pos: egui::pos2(x, y),
                press: false,
                release: false,
                scroll: 0.0,
                time,
            }
        }
        pub(crate) fn press(mut self) -> Self {
            self.press = true;
            self
        }
        pub(crate) fn release(mut self) -> Self {
            self.release = true;
            self
        }
        fn scroll(mut self, y: f32) -> Self {
            self.scroll = y;
            self
        }
        pub(crate) fn input(self) -> egui::RawInput {
            let button = |pressed| egui::Event::PointerButton {
                pos: self.pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            };
            let mut events = vec![egui::Event::PointerMoved(self.pos)];
            if self.press {
                events.push(button(true));
            }
            if self.release {
                events.push(button(false));
            }
            if self.scroll != 0.0 {
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, self.scroll),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::default(),
                });
            }
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            }
        }
    }

    /// One real `run_ui` pass of `f` into `reader`, then the targets a draw
    /// would register and the screen it drew. `run_ui` and not a bare pass
    /// for the reason the map's tests give: `egui_wants_pointer_input`
    /// needs a root `Ui`.
    fn run(reader: &mut Reader, ctx: &egui::Context, app: &mut App, f: Frame) {
        let _ = ctx.run_ui(f.input(), |_| reader.frame(ctx, app));
        register(reader);
        reader.drawn(app);
    }

    /// Frames with the pointer off every target, so the reader is armed and
    /// holds a previous frame's targets.
    fn settled(reader: &mut Reader, ctx: &egui::Context, app: &mut App) {
        run(reader, ctx, app, Frame::at(900.0, 900.0, 0.0));
        run(reader, ctx, app, Frame::at(900.0, 900.0, 0.1));
    }

    fn place(app: &App) -> (Mode, usize) {
        (app.mode, app.menu_selected)
    }

    /// A press then a release at `(x, y)`, from time `t`.
    fn click_at(reader: &mut Reader, ctx: &egui::Context, app: &mut App, x: f32, y: f32, t: f64) {
        run(reader, ctx, app, Frame::at(x, y, t).press());
        run(reader, ctx, app, Frame::at(x, y, t + 0.05).release());
    }

    #[test]
    fn hover_follows_the_previous_frames_targets() {
        let mut app = app_in_a_hub(41300);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        // The first frame has drawn nothing before it, so nothing is hit.
        run(&mut reader, &ctx, &mut app, Frame::at(150.0, 110.0, 0.0));
        assert_eq!(app.pointer_hover, None);
        // The next frame resolves against what that one registered.
        run(&mut reader, &ctx, &mut app, Frame::at(150.0, 110.0, 0.1));
        assert!(app.hovered(&PointerHit::Row(0)));
        run(&mut reader, &ctx, &mut app, Frame::at(150.0, 130.0, 0.2));
        assert!(app.hovered(&PointerHit::Row(1)));
        run(&mut reader, &ctx, &mut app, Frame::at(500.0, 500.0, 0.3));
        assert!(app.hovered(&PointerHit::Region(Region::Page)));
        run(&mut reader, &ctx, &mut app, Frame::at(20.0, 580.0, 0.4));
        assert!(
            app.hovered(&PointerHit::Key(GameKey::Down)),
            "the last drawn is on top"
        );
        run(&mut reader, &ctx, &mut app, Frame::at(900.0, 900.0, 0.5));
        assert_eq!(app.pointer_hover, None);
    }

    #[test]
    fn an_unarmed_press_does_nothing() {
        let mut app = app_in_a_hub(41301);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        // The button goes down over a row on the first frames, before any
        // frame without one: it was already held when the screen opened.
        run(&mut reader, &ctx, &mut app, Frame::at(150.0, 130.0, 0.0));
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 130.0, 0.1).press(),
        );
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 130.0, 0.2).release(),
        );
        assert_eq!(app.menu_selected, 0);
        assert!(!reader.owns_gesture());
    }

    #[test]
    fn a_click_acts_on_the_release_not_the_press() {
        let mut app = app_in_a_hub(41302);
        let mut twin = app_in_a_hub(41302);
        twin.handle_key(GameKey::Down);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        settled(&mut reader, &ctx, &mut app);
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 130.0, 0.2).press(),
        );
        assert_eq!(app.menu_selected, 0, "a press alone is not a click");
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 130.0, 0.25).release(),
        );
        assert_eq!(place(&app), place(&twin));
        assert_eq!(app.menu_selected, 1);
    }

    /// A drag that starts on a row and ends anywhere is no click.
    #[test]
    fn a_drag_off_the_row_clicks_nothing() {
        let mut app = app_in_a_hub(41310);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        settled(&mut reader, &ctx, &mut app);
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 130.0, 0.2).press(),
        );
        run(&mut reader, &ctx, &mut app, Frame::at(150.0, 400.0, 0.25));
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 130.0, 0.3).release(),
        );
        assert_eq!(app.menu_selected, 0, "dragged away and back");
        // And one released on another target is not a click on either.
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 110.0, 0.4).press(),
        );
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 130.0, 0.45).release(),
        );
        assert_eq!(app.menu_selected, 0);
    }

    /// Two clicks on a row are select, then one `Enter`: egui's own
    /// double-click detection is not needed, and a slow second click is the
    /// same thing.
    #[test]
    fn two_clicks_are_enter_once() {
        let mut app = app_in_a_hub(41303);
        let mut twin = app_in_a_hub(41303);
        twin.handle_key(GameKey::Down);
        twin.handle_key(GameKey::Enter);
        let before = place(&app);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        settled(&mut reader, &ctx, &mut app);
        click_at(&mut reader, &ctx, &mut app, 150.0, 130.0, 0.2);
        assert_eq!(app.mode, before.0, "the first click only selects");
        click_at(&mut reader, &ctx, &mut app, 150.0, 130.0, 0.3);
        assert_ne!(place(&app), before);
        assert_eq!(place(&app), place(&twin), "select, then one Enter");
    }

    #[test]
    fn a_keycap_click_equals_its_key() {
        let mut app = app_in_a_hub(41305);
        let mut twin = app_in_a_hub(41305);
        twin.handle_key(GameKey::Down);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        settled(&mut reader, &ctx, &mut app);
        click_at(&mut reader, &ctx, &mut app, 20.0, 580.0, 0.2);
        assert_eq!(place(&app), place(&twin));
        assert_eq!(app.menu_selected, 1);
    }

    /// Targets drawn for one screen are dropped once the app is on another:
    /// a key between the draw and this frame can change the screen, and a
    /// `Key` target is not gated by the screen it lands on.
    #[test]
    fn a_click_on_targets_drawn_for_a_screen_the_app_has_left_does_nothing() {
        let mut app = app_in_a_hub(41311);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        settled(&mut reader, &ctx, &mut app);
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(20.0, 580.0, 0.2).press(),
        );
        // A key leaves the hub before the release lands.
        app.handle_key(GameKey::Esc);
        app.handle_key(GameKey::Esc);
        let left = place(&app);
        assert_ne!(left.0, Mode::BaseOverview);
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(20.0, 580.0, 0.25).release(),
        );
        assert_eq!(place(&app), left, "the stale Key(Down) was dropped");
    }

    #[test]
    fn a_gesture_that_began_on_a_target_is_owned_until_it_ends() {
        let mut app = app_in_a_hub(41312);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        settled(&mut reader, &ctx, &mut app);
        assert!(!reader.owns_gesture());
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(150.0, 130.0, 0.2).press(),
        );
        assert!(reader.owns_gesture(), "the press");
        run(&mut reader, &ctx, &mut app, Frame::at(160.0, 130.0, 0.22));
        assert!(reader.owns_gesture(), "held");
        run(
            &mut reader,
            &ctx,
            &mut app,
            Frame::at(160.0, 130.0, 0.25).release(),
        );
        assert!(reader.owns_gesture(), "the release frame");
        run(&mut reader, &ctx, &mut app, Frame::at(160.0, 130.0, 0.3));
        assert!(!reader.owns_gesture());
    }

    #[test]
    fn the_wheel_equals_up_and_down() {
        let mut app = app_in_a_hub(41307);
        let mut twin = app_in_a_hub(41307);
        twin.handle_key(GameKey::Down);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        settled(&mut reader, &ctx, &mut app);
        let down = Frame::at(150.0, 110.0, 0.2).scroll(-crate::WHEEL_STEP_POINTS);
        run(&mut reader, &ctx, &mut app, down);
        assert_eq!(place(&app), place(&twin));
        assert_eq!(app.menu_selected, 1);
        let up = Frame::at(150.0, 110.0, 0.3).scroll(crate::WHEEL_STEP_POINTS);
        run(&mut reader, &ctx, &mut app, up);
        twin.handle_key(GameKey::Up);
        assert_eq!(place(&app), place(&twin));
    }

    #[test]
    fn a_wheel_off_every_target_is_dropped_not_banked() {
        let mut app = app_in_a_hub(41309);
        let mut reader = Reader::new();
        let ctx = egui::Context::default();
        settled(&mut reader, &ctx, &mut app);
        let off = Frame::at(900.0, 900.0, 0.2).scroll(-crate::WHEEL_STEP_POINTS);
        run(&mut reader, &ctx, &mut app, off);
        run(&mut reader, &ctx, &mut app, Frame::at(150.0, 110.0, 0.3));
        assert_eq!(app.menu_selected, 0);
    }

    // ----- one click, read once -----
    //
    // The pointer layer acts on a release; the older per-screen paths (map,
    // world map, sprite editor) read the same gesture a frame or two later.
    // These drive both, through the same gating `frame` uses.

    struct Both {
        reader: Reader,
        ctx: egui::Context,
        fx: crate::Fx,
        map: crate::MapPointer,
        /// The target the hub registers, in screen pixels.
        target: Option<(Rect, PointerHit)>,
    }

    impl Both {
        fn new(layout: Option<crate::render::MapClickLayout>) -> Self {
            let mut fx = crate::Fx::new();
            fx.set_map_click(layout);
            let ctx = egui::Context::default();
            crate::paint::install_fonts(&ctx);
            Both {
                reader: Reader::new(),
                ctx,
                fx,
                map: crate::MapPointer::default(),
                target: None,
            }
        }

        /// One frame: the reader, the older paths as `frame` gates them,
        /// then the draw's registration (only while a hub is up).
        fn frame(&mut self, app: &mut App, f: Frame) {
            let ctx = self.ctx.clone();
            let _ = ctx.run_ui(f.input(), |_| {
                self.reader.frame(&ctx, app);
                let painter = Painter::for_frame(&ctx, 0.0, Arc::default());
                crate::map_pointer_paths(
                    app,
                    &ctx,
                    &self.reader,
                    &self.fx,
                    &painter,
                    &mut self.map,
                );
            });
            if app.mode == Mode::SpritePicker {
                let painter = Painter::for_frame(&ctx, 0.0, Arc::default())
                    .recording_into(self.reader.sink());
                crate::render::draw(app, &mut self.fx, &painter, false, false);
            }
            if let (Some((rect, hit)), true) = (self.target, app.hub_at().is_some()) {
                let sink = self.reader.sink();
                sink.lock()
                    .expect("no panic held the list")
                    .push((rect, hit));
            }
            self.reader.drawn(app);
        }

        fn click(&mut self, app: &mut App, at: egui::Pos2, t: f64) {
            self.frame(app, Frame::at(at.x, at.y, t).press());
            self.frame(app, Frame::at(at.x, at.y, t + 0.05).release());
        }

        fn settle(&mut self, app: &mut App) {
            self.frame(app, Frame::at(790.0, 590.0, 0.0));
            self.frame(app, Frame::at(790.0, 590.0, 0.1));
        }
    }

    /// An Esc keycap closes the hub on the release; the map under it, whose
    /// pane the click then lands in, must not also read it as a travel.
    #[test]
    fn a_keycap_that_closes_a_hub_does_not_also_travel() {
        let mut app = new_app(41320);
        let start = app.game.as_ref().unwrap().player_status().position;
        let layout = crate::render::MapClickLayout {
            pane: Rect::new(0.0, 0.0, 800.0, 600.0),
            center: start,
            half: (40, 30),
            off: (0.0, 0.0),
            tile_px: 10.0,
        };
        let mut both = Both::new(Some(layout));
        app.handle_key(GameKey::Char('b'));
        assert!(app.hub_at().is_some());
        both.target = Some((
            Rect::new(300.0, 300.0, 60.0, 20.0),
            PointerHit::Key(GameKey::Esc),
        ));
        both.settle(&mut app);
        both.click(&mut app, egui::pos2(330.0, 310.0), 0.2);
        assert_eq!(app.mode, Mode::Playing, "the keycap closed the hub");
        for _ in 0..6 {
            app.update_realtime(1.0 / app.world_speed.ticks_per_second());
        }
        assert_eq!(
            app.game.as_ref().unwrap().player_status().position,
            start,
            "the same click travelled on the map under it"
        );
    }

    /// Esc, down to Sprites, and a click on it opens the picker. A click on
    /// the picker's first row then only selects it, whatever came just
    /// before; a second opens it, the way two clicks on a hub row are select
    /// then Enter.
    #[test]
    fn a_click_that_opens_the_picker_does_not_open_a_sprite_with_the_next() {
        let mut app = new_app(41321);
        app.handle_key(GameKey::Esc);
        for _ in 0..3 {
            app.handle_key(GameKey::Down);
        }
        assert_eq!(app.mode, Mode::SpriteOptions);
        let mut both = Both::new(None);
        let first = egui::pos2(330.0, 310.0);
        both.target = Some((Rect::new(300.0, 300.0, 60.0, 20.0), PointerHit::Row(0)));
        both.settle(&mut app);
        both.click(&mut app, first, 0.2);
        assert_eq!(app.mode, Mode::SpritePicker, "the first click opened it");
        let row = both
            .reader
            .sink()
            .lock()
            .expect("no panic held the list")
            .iter()
            .find(|(_, hit)| *hit == PointerHit::Subject(0))
            .map(|(r, _)| egui::pos2(r.x + r.w / 2.0, r.y + r.h / 2.0))
            .expect("the picker drew a first row");
        both.click(&mut app, row, 0.3);
        assert_eq!(
            app.mode,
            Mode::SpritePicker,
            "the next click opened the subject under it"
        );
        both.click(&mut app, row, 0.4);
        assert_eq!(app.mode, Mode::SpriteEditor, "a second click opens it");
    }
}
