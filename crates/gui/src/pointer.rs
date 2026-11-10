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
//! While a gesture that began on a target is live the older per-screen path
//! (the world map's) stands down (`owns_gesture`), or one click would be read
//! twice.
//!
//! **A click on the map is the one exception to "the same target":** every
//! tile is a target, and the camera eases while a travel is live, so the tile
//! under a pointer held dead still can change between press and release. A
//! click on any tile that ends a press on any tile, within
//! `CLICK_DRAG_TOLERANCE_PX`, is a click on the tile it ended on — what the
//! player saw when they let go.

use bevy_egui::egui;
use feral_processes_app_core::{App, HubPageKey, Mode, PointerButton, PointerEvent, PointerHit};

use crate::paint::{Rect, Targets};

/// Buttons in the order a frame pressing several is read.
const BUTTONS: [(egui::PointerButton, PointerButton); 3] = [
    (egui::PointerButton::Primary, PointerButton::Primary),
    (egui::PointerButton::Secondary, PointerButton::Secondary),
    (egui::PointerButton::Middle, PointerButton::Middle),
];

/// How long the pointer must rest on one map tile before its hover label
/// shows — long enough that sweeping across the map to click somewhere
/// does not flicker a label over every tile on the way.
const HOVER_DELAY_SECS: f64 = 0.5;

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
    /// Alt was held: the middle button for a mouse with none, anywhere but
    /// the map, where Alt is the room overlay and a click still travels.
    alt: bool,
    time: f64,
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
                .map(|(_, mine)| mine),
            alt: i.modifiers.alt,
            time: i.time,
            released: BUTTONS
                .into_iter()
                .any(|(b, _)| i.pointer.button_released(b)),
            down: i.pointer.any_down(),
            moved: i.pointer.delta() != egui::Vec2::ZERO,
            scroll_y: i.smooth_scroll_delta().y,
        })
    }
}

/// The button a press means over `hover`: Alt+click is the middle button for
/// a mouse with none (the sprite editor's eyedropper), except on the map.
fn button_for(pressed: PointerButton, alt: bool, hover: Option<PointerHit>) -> PointerButton {
    match pressed {
        PointerButton::Primary if alt && !matches!(hover, Some(PointerHit::MapTile(..))) => {
            PointerButton::Middle
        }
        other => other,
    }
}

/// Whether a release over `now` ends a press that began on `began`: the same
/// target, or any tile for any tile.
fn same_target(began: PointerHit, now: Option<PointerHit>) -> bool {
    match (began, now) {
        (PointerHit::MapTile(..), Some(PointerHit::MapTile(..))) => true,
        (began, Some(now)) => began == now,
        _ => false,
    }
}

/// Advances the hover timer: `tile` is what the pointer is over this frame
/// (`None` off the map), `now` is in seconds. Answers the tile once the
/// pointer has rested on it for `HOVER_DELAY_SECS`; a different tile, or
/// none, starts the wait over.
fn hover_tile(
    hover: &mut Option<((i32, i32), f64)>,
    tile: Option<(i32, i32)>,
    now: f64,
) -> Option<(i32, i32)> {
    let Some(tile) = tile else {
        *hover = None;
        return None;
    };
    let since = match *hover {
        Some((held, since)) if held == tile => since,
        _ => {
            *hover = Some((tile, now));
            now
        }
    };
    (now - since >= HOVER_DELAY_SECS).then_some(tile)
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
    /// The tile the pointer is resting on and when it arrived
    /// (`hover_tile`).
    dwell: Option<((i32, i32), f64)>,
    /// The pointer's pixel and the tile it has rested on long enough for a
    /// label.
    label: Option<(egui::Pos2, (i32, i32))>,
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
            dwell: None,
            label: None,
        }
    }

    /// Where a hover label is due and for which tile: the pointer has rested
    /// on one with no button held. A held button hides it — a press is the
    /// start of a click, not a pause to read.
    pub fn hover_label(&self) -> Option<(egui::Pos2, (i32, i32))> {
        self.label
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
        let pressed = raw.pressed.map(|b| button_for(b, raw.alt, hover));
        let (press, click) = self.gesture_step(&raw, pressed, hover);
        app.handle_pointer_event(PointerEvent {
            hover,
            press,
            drag: raw.down && raw.moved,
            up: raw.released,
            click: click.is_some(),
            button: click.or(pressed).unwrap_or_default(),
            wheel,
        });
        let resting = raw.pos.filter(|_| !raw.down).zip(app.hovered_tile());
        self.label = hover_tile(&mut self.dwell, resting.map(|(_, t)| t), raw.time).and(resting);
    }

    /// Advances the gesture by one frame: whether a press began, and the
    /// button of the click that ended, if one did.
    fn gesture_step(
        &mut self,
        raw: &Raw,
        pressed: Option<PointerButton>,
        hover: Option<PointerHit>,
    ) -> (bool, Option<PointerButton>) {
        let press = self.begin_gesture(raw, pressed, hover);
        if let (Some(g), Some(pos)) = (&mut self.gesture, raw.pos)
            && pos.distance(g.origin) > crate::CLICK_DRAG_TOLERANCE_PX
        {
            g.dragged = true;
        }
        let ended = self.gesture.take_if(|_| raw.released || !raw.down);
        let click = ended
            .as_ref()
            .filter(|g| raw.released && !g.dragged && same_target(g.target, hover))
            .map(|g| g.button);
        self.owns = self.gesture.is_some() || press || ended.is_some();
        (press, click)
    }

    /// A press on an armed reader, over a target, starts a gesture.
    fn begin_gesture(
        &mut self,
        raw: &Raw,
        pressed: Option<PointerButton>,
        hover: Option<PointerHit>,
    ) -> bool {
        let (true, Some(button), Some(target), Some(origin)) =
            (self.armed, pressed, hover, raw.pos)
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
        alt: bool,
        button: egui::PointerButton,
    }

    impl Frame {
        pub(crate) fn at(x: f32, y: f32, time: f64) -> Self {
            Frame {
                pos: egui::pos2(x, y),
                press: false,
                release: false,
                scroll: 0.0,
                time,
                alt: false,
                button: egui::PointerButton::Primary,
            }
        }
        fn alt(mut self) -> Self {
            self.alt = true;
            self
        }
        fn secondary(mut self) -> Self {
            self.button = egui::PointerButton::Secondary;
            self
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
                button: self.button,
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
                modifiers: egui::Modifiers {
                    alt: self.alt,
                    ..Default::default()
                },
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
        /// The map pane the draw registers tiles for, drawn under everything.
        layout: Option<crate::render::MapLayout>,
        /// The target the hub registers, in screen pixels.
        target: Option<(Rect, PointerHit)>,
    }

    impl Both {
        fn new(layout: Option<crate::render::MapLayout>) -> Self {
            let ctx = egui::Context::default();
            crate::paint::install_fonts(&ctx);
            Both {
                reader: Reader::new(),
                ctx,
                fx: crate::Fx::new(),
                layout,
                target: None,
            }
        }

        /// One frame: the reader, the world map's older path as `frame`
        /// gates it, then the draw's registration — the map's tiles first,
        /// so what is drawn over them wins, and a hub's target only while a
        /// hub is up.
        fn frame(&mut self, app: &mut App, f: Frame) {
            let ctx = self.ctx.clone();
            let _ = ctx.run_ui(f.input(), |_| {
                self.reader.frame(&ctx, app);
                let painter = Painter::for_frame(&ctx, 0.0, Arc::default());
                crate::handle_world_map_pointer(app, &ctx, &self.reader, &painter);
            });
            if app.mode == Mode::SpritePicker {
                let painter = Painter::for_frame(&ctx, 0.0, Arc::default())
                    .recording_into(self.reader.sink());
                crate::render::draw(app, &mut self.fx, &painter, false, false);
            }
            if let Some(layout) = self.layout {
                let painter = Painter::for_frame(&ctx, 0.0, Arc::default())
                    .recording_into(self.reader.sink());
                layout.register_tiles(&painter);
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
        let mut both = Both::new(Some(map_layout(start)));
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

    // ----- the map: click to walk or aim, and the hover label -----
    //
    // The map registers a `MapTile` per tile it draws; these drive the
    // reader through headless egui frames against a fixed layout, as the
    // map's own pointer path was driven before it moved onto the layer.

    /// An `App` standing on the map in `Mode::Playing` with a real `Game`,
    /// since click-to-travel needs a live run to hand `App::travel_to` to.
    fn app_on_the_map(seed: u32) -> App {
        let mut app = new_app(seed);
        app.mode = Mode::Playing;
        app
    }

    /// A layout whose pixel math is simple enough to invert by hand: ten
    /// pixels to a tile over the whole 800x600 screen, `center` at the
    /// middle tile.
    fn map_layout(center: (i32, i32)) -> crate::render::MapLayout {
        crate::render::MapLayout {
            pane: Rect::new(0.0, 0.0, 800.0, 600.0),
            center,
            half: (40, 30),
            off: (0.0, 0.0),
            tile_px: 10.0,
        }
    }

    /// The pixel in the middle of `center`'s own tile in `map_layout`.
    const MIDDLE: (f32, f32) = (405.0, 305.0);

    /// A target offset from the player's own position that `Game::
    /// travel_step` answers `Toward` for, paired with the exact first
    /// step `Toward` names — found by probing a handful of candidates
    /// rather than assumed from the seed, since a real route's first step
    /// need not be the target's own direction. `travel_step` is a pure
    /// read (see its own doc) with no `GameRng` draw, so probing it ahead
    /// of the click, on an otherwise untouched world, is guaranteed to
    /// answer the click's own later call identically.
    fn find_a_routable_offset(app: &mut App) -> ((i32, i32), (i32, i32)) {
        let player = app.game.as_ref().unwrap().player_status().position;
        let game = app.game.as_mut().unwrap();
        for radius in 2..8 {
            for &(dx, dy) in &[(radius, 0), (-radius, 0), (0, radius), (0, -radius)] {
                let target = (player.0 + dx, player.1 + dy);
                if let feral_processes_engine::TravelStep::Toward(sx, sy) =
                    game.travel_step(feral_processes_engine::TravelGoal::Tile(target.0, target.1))
                {
                    return ((dx, dy), (sx, sy));
                }
            }
        }
        panic!("no routable offset found within radius 8 for this seed");
    }

    /// The pixel `dx`, `dy` tiles from the middle tile's corner, `ix`, `iy`
    /// pixels into it.
    fn tile_px_at(dx: i32, dy: i32, ix: f32, iy: f32) -> (f32, f32) {
        (((dx + 40) * 10) as f32 + ix, ((dy + 30) * 10) as f32 + iy)
    }

    /// The label waits `HOVER_DELAY_SECS` on one tile, and moving to
    /// another tile — or off the map — starts the wait over.
    #[test]
    fn hover_waits_on_one_tile_and_restarts_when_the_tile_changes() {
        let mut hover = None;
        let (a, b) = ((1, 1), (2, 1));
        assert_eq!(hover_tile(&mut hover, Some(a), 10.0), None);
        assert_eq!(
            hover_tile(&mut hover, Some(a), 10.0 + HOVER_DELAY_SECS * 0.9),
            None
        );
        assert_eq!(
            hover_tile(&mut hover, Some(a), 10.0 + HOVER_DELAY_SECS),
            Some(a)
        );
        assert_eq!(hover_tile(&mut hover, Some(b), 11.0), None);
        assert_eq!(
            hover_tile(&mut hover, Some(b), 11.0 + HOVER_DELAY_SECS),
            Some(b)
        );
        assert_eq!(hover_tile(&mut hover, None, 12.0), None);
        assert_eq!(
            hover_tile(&mut hover, Some(b), 12.0),
            None,
            "leaving the map did not reset the wait"
        );
    }

    /// The whole hover path through the reader: nothing until the pointer
    /// has rested `HOVER_DELAY_SECS` on one tile, then that tile at the
    /// pointer's pixel, and nothing again while a button is held.
    #[test]
    fn the_reader_answers_a_hover_after_a_rest_and_not_while_held() {
        let mut app = app_on_the_map(6601);
        let start = app.game.as_ref().unwrap().player_status().position;
        let layout = map_layout(start);
        let mut both = Both::new(Some(layout));
        both.settle(&mut app);
        let pos = egui::pos2(MIDDLE.0, MIDDLE.1);
        let tile = crate::render::tile_at_px(pos.x, pos.y, &layout);
        assert_eq!(tile, start);

        both.frame(&mut app, Frame::at(pos.x, pos.y, 1.0));
        assert_eq!(both.reader.hover_label(), None);
        both.frame(&mut app, Frame::at(pos.x, pos.y, 1.0 + HOVER_DELAY_SECS));
        assert_eq!(both.reader.hover_label(), Some((pos, tile)));
        both.frame(&mut app, Frame::at(pos.x, pos.y, 2.0).press());
        assert_eq!(
            both.reader.hover_label(),
            None,
            "a held button kept the label"
        );
    }

    /// Inside the pane the pointer is on a tile and nothing outside it is.
    #[test]
    fn a_tile_resolves_inside_the_pane_and_nothing_outside_it() {
        let mut app = app_on_the_map(6602);
        let layout = crate::render::MapLayout {
            pane: Rect::new(100.0, 100.0, 200.0, 200.0),
            center: (0, 0),
            half: (10, 10),
            off: (0.0, 0.0),
            tile_px: 10.0,
        };
        let mut both = Both::new(Some(layout));
        both.settle(&mut app);
        let at = |both: &mut Both, app: &mut App, x: f32, y: f32, t: f64| {
            both.frame(app, Frame::at(x, y, t));
            both.frame(app, Frame::at(x, y, t + 0.01));
            app.hovered_tile()
        };
        assert_eq!(
            at(&mut both, &mut app, 150.0, 150.0, 1.0),
            Some(crate::render::tile_at_px(150.0, 150.0, &layout))
        );
        assert_eq!(at(&mut both, &mut app, 99.0, 150.0, 2.0), None);
        assert_eq!(at(&mut both, &mut app, 150.0, 300.0, 3.0), None);
    }

    /// **The regression finding 7 exists for.** The camera eases toward its
    /// target while a travel is live, so a pointer held dead still can see
    /// the tile under it drift between press and release — a tile-equality
    /// test dropped exactly the clicks that caused. A press and release
    /// within `CLICK_DRAG_TOLERANCE_PX` of each other, landing on two
    /// *different* tiles, must still travel — resolved at the release
    /// position, proven by walking the party toward it.
    #[test]
    fn a_press_and_release_within_tolerance_travels_to_the_release_tile() {
        let mut app = app_on_the_map(6501);
        let ((dx, dy), (sx, sy)) = find_a_routable_offset(&mut app);
        let start = app.game.as_ref().unwrap().player_status().position;
        let mut both = Both::new(Some(map_layout(start)));
        both.settle(&mut app);

        // The release pixel sits 1px inside the target tile; the press
        // pixel sits 2px away from it, in the *previous* tile — well
        // inside `CLICK_DRAG_TOLERANCE_PX`, and exactly the boundary a
        // tile-equality test would have refused.
        let (rx, ry) = tile_px_at(dx, dy, 1.0, 5.0);
        both.frame(&mut app, Frame::at(rx - 2.0, ry, 0.2).press());
        both.frame(&mut app, Frame::at(rx, ry, 0.25).release());
        app.update_realtime(1.0 / app.world_speed.ticks_per_second());

        assert_eq!(
            app.game.as_ref().unwrap().player_status().position,
            (start.0 + sx, start.1 + sy),
            "a press+release within tolerance must travel toward the release tile"
        );
    }

    /// A press and release beyond `CLICK_DRAG_TOLERANCE_PX` is a drag, not
    /// a click — no travel is queued, so the party never leaves its tile.
    #[test]
    fn a_press_and_release_beyond_tolerance_is_not_a_click() {
        let mut app = app_on_the_map(6502);
        let start = app.game.as_ref().unwrap().player_status().position;
        let mut both = Both::new(Some(map_layout(start)));
        both.settle(&mut app);

        let (x, y) = MIDDLE;
        both.frame(&mut app, Frame::at(x, y, 0.2).press());
        both.frame(
            &mut app,
            Frame::at(x + crate::CLICK_DRAG_TOLERANCE_PX + 10.0, y, 0.25).release(),
        );
        for _ in 0..5 {
            app.update_realtime(1.0 / app.world_speed.ticks_per_second());
        }

        assert_eq!(
            app.game.as_ref().unwrap().player_status().position,
            start,
            "a press+release beyond tolerance must not queue a travel"
        );
    }

    /// Alt is the room overlay on the map, so an Alt+click there is still
    /// a click and travels — the middle button Alt stands for elsewhere
    /// (the sprite editor's eyedropper) is not what the map reads.
    #[test]
    fn an_alt_click_on_the_map_still_travels() {
        let mut app = app_on_the_map(6503);
        let ((dx, dy), (sx, sy)) = find_a_routable_offset(&mut app);
        let start = app.game.as_ref().unwrap().player_status().position;
        let mut both = Both::new(Some(map_layout(start)));
        both.settle(&mut app);

        let (x, y) = tile_px_at(dx, dy, 5.0, 5.0);
        both.frame(&mut app, Frame::at(x, y, 0.2).press().alt());
        both.frame(&mut app, Frame::at(x, y, 0.25).release().alt());
        app.update_realtime(1.0 / app.world_speed.ticks_per_second());

        assert_eq!(
            app.game.as_ref().unwrap().player_status().position,
            (start.0 + sx, start.1 + sy),
            "Alt+click on the map did not travel"
        );
    }

    /// Every frame the reader tells app-core which tile the pointer is
    /// over — the aim cursor's follow and the board's hover path both read
    /// it — and clears it the frame the pointer leaves the pane.
    #[test]
    fn a_frame_over_the_pane_sets_the_apps_hover_tile_and_off_it_clears_it() {
        let mut app = app_on_the_map(6701);
        let start = app.game.as_ref().unwrap().player_status().position;
        let layout = map_layout(start);
        let mut both = Both::new(Some(layout));
        both.settle(&mut app);
        let tile = crate::render::tile_at_px(MIDDLE.0, MIDDLE.1, &layout);

        both.frame(&mut app, Frame::at(MIDDLE.0, MIDDLE.1, 1.0));
        assert_eq!(app.pointer_hover, Some(PointerHit::MapTile(tile.0, tile.1)));

        both.frame(&mut app, Frame::at(-5.0, -5.0, 1.1));
        assert_eq!(app.pointer_hover, None);
    }

    /// A secondary click reaches app-core with the secondary button, under
    /// the same press-and-release rule as a primary one. Observed on a
    /// battle map, where a secondary click clears the target mark — on the
    /// surface map the button means nothing, so nothing there could tell a
    /// delivered click from a dropped one.
    #[test]
    fn a_secondary_click_reaches_the_tile_click_with_the_secondary_button() {
        let mut app = crate::tests::app_in_tactical_fight();
        for _ in 0..64 {
            if app.tactical_player_turn() {
                break;
            }
            assert!(app.game.as_mut().unwrap().tactical_ai_turn());
        }
        assert!(app.tactical_player_turn(), "the player's turn never came");
        let hostile = app
            .game
            .as_mut()
            .unwrap()
            .tactical_view()
            .unwrap()
            .bodies
            .into_iter()
            .find(|b| b.is_hostile)
            .expect("a hostile is seated");
        app.handle_pointer_event(PointerEvent {
            hover: Some(PointerHit::MapTile(hostile.cell.0, hostile.cell.1)),
            click: true,
            ..Default::default()
        });
        assert!(
            app.tactical_target.is_some(),
            "the primary click marked nothing"
        );

        let mut both = Both::new(Some(map_layout(hostile.cell)));
        both.settle(&mut app);
        let (x, y) = MIDDLE;
        both.frame(&mut app, Frame::at(x, y, 0.2).press().secondary());
        both.frame(&mut app, Frame::at(x, y, 0.25).release().secondary());

        assert_eq!(
            app.tactical_target, None,
            "the secondary click never reached app-core"
        );
    }
}
