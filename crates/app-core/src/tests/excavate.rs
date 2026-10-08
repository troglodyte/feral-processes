//! `Mode::Excavate` — the Excavation plan: a cursor, a box, and the marks a
//! crew will one day work.
//!
//! The mode's whole claim is that it is a *mode* and not an action, so the
//! test that matters most here is the one asserting the clock never moves.

use feral_processes_engine::floors::FloorId;
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save;

use super::support::*;
use crate::*;

/// The party on the pocket's eastern edge, one step from solid rock — the
/// place a plan is actually drawn from.
fn app_at_the_frontier(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base_at(
        &mut app,
        feral_processes_engine::tuning::STARTING_POCKET_RADIUS,
        0,
    );
    app
}

/// A scratch install with everything `Game::new` needs but no
/// `assets/floors/` at all — the supported "no finish content" install the
/// brush must go inert against, `FloorDb::load_dir`'s own absent-directory
/// rule reached from the other end.
struct NoFloorsAssets(std::path::PathBuf);

impl Drop for NoFloorsAssets {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn app_without_floors(seed: u32) -> (App, NoFloorsAssets) {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("feral_processes_appcore_no_floors_{seed}_{unique}"));
    let shipped = test_assets_dir();
    for sub in [
        "species",
        "structures",
        "research",
        "items",
        "abilities",
        "perks",
        "talents",
        "achievements",
        "descriptions",
        "policies",
        "affixes",
    ] {
        let dst = dir.join(sub);
        std::fs::create_dir_all(&dst).unwrap();
        for entry in std::fs::read_dir(shipped.join(sub)).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), dst.join(entry.file_name())).unwrap();
        }
    }
    let guard = NoFloorsAssets(dir.clone());

    let mut app = App::new(
        dir.clone(),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_no_floors_{seed}_{unique}_saves"
        )),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_no_floors_{seed}_{unique}.log"
        )),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_no_floors_{seed}_{unique}_profile.ron"
        )),
        arenas_dir(),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_no_floors_{seed}_{unique}_telemetry.jsonl"
        )),
    );
    app.game = Game::new(seed, DifficultyMode::Forgiving, &dir).ok();
    if let Some(game) = &mut app.game {
        while game.take_notification().is_some() {}
    }
    app.mode = Mode::Playing;
    found_the_base(&mut app);

    // `stand_in_base_at` reloads through the real `test_assets_dir()`, which
    // would silently bring the shipped floors back — this fixture's whole
    // point is that they are absent, so the save/reload has to go through
    // the same scratch directory by hand.
    let path = dir.join("stand.sav");
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.locale = Locale::Base {
        x: feral_processes_engine::tuning::STARTING_POCKET_RADIUS,
        y: 0,
    };
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &dir).unwrap());

    (app, guard)
}

fn marks(app: &mut App) -> Vec<(i32, i32)> {
    app.game
        .as_mut()
        .expect("a fixture with a game")
        .marked_cells()
        .into_iter()
        .map(|m| m.pos)
        .collect()
}

fn tick(app: &App) -> u64 {
    app.game
        .as_ref()
        .expect("a fixture with a game")
        .current_tick()
}

#[test]
fn m_opens_excavation_plan_in_base_space_and_does_nothing_on_the_surface() {
    let mut app = app_at_the_frontier(4300);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(
        app.mode,
        Mode::Excavate,
        "m must open the plan in base space"
    );

    let mut surface = test_app(4301);
    found_the_base(&mut surface);
    surface.handle_key(GameKey::Char('m'));
    assert_eq!(
        surface.mode,
        Mode::Playing,
        "there is nothing to excavate on the open grid"
    );
    assert!(
        surface.status_line.is_some(),
        "a refused key must say why rather than looking broken"
    );
}

#[test]
fn the_cursor_starts_on_the_party_and_moves_with_the_direction_keys() {
    let mut app = app_at_the_frontier(4302);
    let party = app.game.as_ref().unwrap().base_pos().unwrap();

    app.handle_key(GameKey::Char('m'));
    assert_eq!(
        app.excavate_cursor,
        Some(party),
        "the cursor must open on the one cell the player already knows"
    );

    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Down);
    assert_eq!(
        app.excavate_cursor,
        Some((party.0 + 1, party.1 + 1)),
        "the cursor did not walk with the keys the player walks with"
    );
}

#[test]
fn committing_a_box_reaches_toggle_mark_box() {
    let mut app = app_at_the_frontier(4303);
    let (px, py) = app.game.as_ref().unwrap().base_pos().unwrap();

    app.handle_key(GameKey::Char('m'));
    // Out onto solid rock, so the box is drawn over cells that can take a
    // mark at all.
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Char('j'));
    app.handle_key(GameKey::Char(' '));

    assert_eq!(
        marks(&mut app),
        vec![(px + 1, py), (px + 1, py + 1)],
        "the committed box did not reach the engine's marks"
    );
    assert_eq!(
        app.excavate_anchor, None,
        "a committed box leaves its anchor down"
    );
    assert_eq!(
        app.mode,
        Mode::Excavate,
        "committing must not leave the mode"
    );
}

/// The load-bearing property of the whole mode: planning a wing of the base
/// costs no game time at all.
#[test]
fn excavation_plan_never_ticks_the_game() {
    let mut app = app_at_the_frontier(4304);
    let before = tick(&app);

    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Char('j'));
    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        tick(&app),
        before,
        "drawing a plan spent game time — a mode is not an action"
    );
}

/// One press is never two undos: with an anchor down, Esc takes back the
/// anchor and leaves the player where they were drawing.
#[test]
fn esc_with_an_anchor_down_drops_the_anchor_and_stays_in_the_mode() {
    let mut app = app_at_the_frontier(4305);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char(' '));
    assert!(app.excavate_anchor.is_some(), "space must drop an anchor");

    app.handle_key(GameKey::Esc);

    assert_eq!(
        app.mode,
        Mode::Excavate,
        "Esc left the mode as well as the anchor"
    );
    assert_eq!(
        app.excavate_anchor, None,
        "Esc did not take the anchor back"
    );
    assert!(
        marks(&mut app).is_empty(),
        "a dropped anchor marked something"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "a second Esc leaves the mode");
    assert_eq!(app.excavate_cursor, None, "leaving must clear the cursor");
}

// ---------------------------------------------------------------------------
// The brush
// ---------------------------------------------------------------------------

/// `[F]` opens the brush picker over the plan rather than cycling in place:
/// a popup of plain, every shipped finish in id order, then strip.
#[test]
fn f_opens_the_brush_picker_listing_plain_every_finish_then_strip() {
    let mut app = app_at_the_frontier(4306);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(app.excavate_brush, None, "the brush opens plain");

    app.handle_key(GameKey::Char('F'));
    assert_eq!(app.mode, Mode::ExcavateBrush);
    let rows = app.excavate_brush_rows();
    let brushes: Vec<_> = rows.iter().map(|r| r.brush.clone()).collect();
    assert_eq!(brushes.first(), Some(&None), "plain is the first row");
    assert_eq!(brushes.last(), Some(&Some(FinishOrder::Strip)));
    let finishes: Vec<_> = brushes[1..brushes.len() - 1]
        .iter()
        .map(|b| match b {
            Some(FinishOrder::Apply(id)) => id.clone(),
            other => panic!("a middle row must be a finish, got {other:?}"),
        })
        .collect();
    let mut sorted = finishes.clone();
    sorted.sort();
    assert_eq!(finishes, sorted, "finishes are listed in id order");
    assert_eq!(
        finishes.len(),
        app.game.as_ref().unwrap().floor_defs().len(),
        "every loaded finish is offered"
    );
    assert!(
        rows[1..rows.len() - 1].iter().all(|r| r.shade.is_some()),
        "every finish row carries its swatch"
    );
    assert!(rows[0].shade.is_none() && rows[rows.len() - 1].shade.is_none());
}

/// Picking a row sets the brush and drops back onto the plan with the
/// cursor and a dropped anchor untouched — the picker is a detour, not an
/// exit.
#[test]
fn picking_a_row_sets_the_brush_and_returns_to_the_plan() {
    let mut app = app_at_the_frontier(4312);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char(' '));
    let (cursor, anchor) = (app.excavate_cursor, app.excavate_anchor);
    assert!(anchor.is_some());

    app.handle_key(GameKey::Char('F'));
    let moss = app
        .excavate_brush_rows()
        .iter()
        .position(|r| r.brush == Some(FinishOrder::Apply(FloorId::from("moss_weave"))))
        .unwrap();
    for _ in 0..moss {
        app.handle_key(GameKey::Down);
    }
    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Excavate);
    assert_eq!(
        app.excavate_brush,
        Some(FinishOrder::Apply(FloorId::from("moss_weave")))
    );
    assert_eq!(app.excavate_cursor, cursor);
    assert_eq!(
        app.excavate_anchor, anchor,
        "the anchor survives the picker"
    );
}

/// The picker opens on the brush already in hand, so Enter alone keeps it.
#[test]
fn the_picker_opens_highlighting_the_current_brush() {
    let mut app = app_at_the_frontier(4313);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('F'));
    assert_eq!(app.menu_selected, 0, "plain is highlighted first");
    let last = app.excavate_brush_rows().len() - 1;
    app.handle_key(GameKey::Up);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.excavate_brush, Some(FinishOrder::Strip));

    app.handle_key(GameKey::Char('F'));
    assert_eq!(app.menu_selected, last, "strip is highlighted on reopening");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.excavate_brush, Some(FinishOrder::Strip));
}

/// Esc backs out to the plan with the brush unchanged — one press is one
/// step back, never the whole mode.
#[test]
fn esc_in_the_picker_returns_to_the_plan_unchanged() {
    let mut app = app_at_the_frontier(4314);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('F'));
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Excavate);
    assert_eq!(app.excavate_brush, None);
}

/// With no finish content loaded at all, `[F]` has nothing to offer and
/// touches nothing — the same "supported, does exactly today's game"
/// contract every asset-backed screen carries.
#[test]
fn f_is_inert_with_no_floor_catalogue_loaded() {
    let (mut app, _assets) = app_without_floors(4307);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(app.mode, Mode::Excavate);

    app.handle_key(GameKey::Char('F'));
    assert_eq!(app.mode, Mode::Excavate, "no picker over an empty FloorDb");
    assert_eq!(
        app.excavate_brush, None,
        "an empty FloorDb must leave the brush untouched"
    );
    assert_eq!(
        app.excavate_brush_label(),
        None,
        "an empty FloorDb must offer no header line at all"
    );
}

/// A row's printed shortcut picks exactly that row — past the ninth, a
/// letter.
#[test]
fn a_rows_shortcut_picks_that_row() {
    let mut app = app_at_the_frontier(4315);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('F'));
    let rows = app.excavate_brush_rows();
    assert!(rows.len() > 9, "the shipped catalogue reaches the letters");
    let last = rows.len() - 1;
    app.handle_key(GameKey::Char(crate::menu_shortcut(last)));
    assert_eq!(app.mode, Mode::Excavate);
    assert_eq!(app.excavate_brush, rows[last].brush);
}

/// Picks `brush` through the picker: `[F]`, then the highlight moved onto
/// its row and Enter.
fn pick_brush(app: &mut App, brush: Option<FinishOrder>) {
    app.handle_key(GameKey::Char('F'));
    let row = app
        .excavate_brush_rows()
        .iter()
        .position(|r| r.brush == brush)
        .expect("the brush is offered");
    app.menu_selected = row;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Excavate);
}

/// Opening the mode resets a brush left over from the last visit — the same
/// rule `excavate_anchor` already follows.
#[test]
fn opening_the_mode_resets_the_brush_to_plain() {
    let mut app = app_at_the_frontier(4308);
    app.handle_key(GameKey::Char('m'));
    pick_brush(&mut app, Some(FinishOrder::Strip));

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char('m'));
    assert_eq!(
        app.excavate_brush, None,
        "a brush left over from the last visit must not carry over"
    );
}

/// The brush the player picked is what actually reaches
/// `Game::toggle_mark_box`. A `Floor` cell is never touched by any other
/// kind of mark (`plain` skips it outright), so a marked floor cell here can
/// only be a finish or a strip mark reaching the engine.
#[test]
fn committing_with_a_finish_brush_marks_the_already_laid_floor_under_it() {
    let mut app = app_at_the_frontier(4309);
    let party = app.game.as_ref().unwrap().base_pos().unwrap();

    app.handle_key(GameKey::Char('m'));
    pick_brush(
        &mut app,
        Some(FinishOrder::Apply(FloorId::from("cobalt_carpet"))),
    );
    // A single-cell box on the party's own tile, which the pocket already
    // laid as floor — the one cell this fixture can be sure of without
    // digging anything first.
    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Char(' '));

    assert_eq!(
        marks(&mut app),
        vec![party],
        "the finish brush did not reach toggle_mark_box"
    );
}

/// `excavate_brush_label` names each brush state and is absent entirely
/// with no finish content loaded.
#[test]
fn excavate_brush_label_names_every_state() {
    let mut app = app_at_the_frontier(4310);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(
        app.excavate_brush_label().as_deref(),
        Some("Brush: plain [F]")
    );

    pick_brush(
        &mut app,
        Some(FinishOrder::Apply(FloorId::from("cobalt_carpet"))),
    );
    assert_eq!(
        app.excavate_brush_label().as_deref(),
        Some("Brush: Cobalt Carpet [F]")
    );
    pick_brush(&mut app, Some(FinishOrder::Strip));
    assert_eq!(
        app.excavate_brush_label().as_deref(),
        Some("Brush: strip [F]")
    );

    let (mut empty, _assets) = app_without_floors(4311);
    empty.handle_key(GameKey::Char('m'));
    assert_eq!(empty.excavate_brush_label(), None);
}
