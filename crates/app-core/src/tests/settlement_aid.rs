//! The two aid verbs and the keys that reach them — `[G]` and `[T]` on the
//! town page, `[T]` on the Relay hub.
//!
//! The fixtures are `tests/dispatch.rs`'s, reused rather than rebuilt: both
//! verbs need a Relay standing, and the engine exposes no way to hand-place
//! a structure from outside the crate, so a save round trip is the only way
//! in either file.

use super::dispatch::{app_at_a_relay, register_a_known_settlement};
use super::support::*;
use crate::*;
use feral_processes_engine::items::ItemId;
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save;
use feral_processes_engine::settlements::SettlementKey;
use feral_processes_engine::tuning::SETTLEMENT_ALLIED_STANDING;

/// Puts `key` at `standing` through a save round trip — app-core cannot
/// reach `Game::adjust_standing`, which is `pub(crate)` on purpose.
fn set_standing(app: &mut App, key: SettlementKey, standing: i32) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("aid_standing", standing.unsigned_abs());
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.standings.0.insert(
        key,
        feral_processes_engine::settlements::relations::Relation {
            standing,
            ..Default::default()
        },
    );
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// A Relay standing, the party out on the surface at the anchor, and an
/// Allied town on the tile beside them — the one arrangement in which both
/// aid verbs are legal.
fn app_at_an_allied_town(seed: u32) -> (App, SettlementKey) {
    let mut app = app_at_a_relay(seed, &ItemId::from("cache_grain"), 10);
    let key = SettlementKey { rx: 6, ry: 6 };
    register_a_known_settlement(&mut app, key, (0, 0));

    // Out of base space, standing on the anchor, with the town beside it.
    let assets_dir = test_assets_dir();
    let path = scratch_path("aid_surface", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    let anchor = data.anchor.expect("the fixture founded a base");
    data.locale = Locale::Surface;
    // North of the anchor: east would be the Relay's own base-space
    // coordinates, and a settlement recorded there collides by number.
    data.settlements.0.get_mut(&key).unwrap().tile = (anchor.0, anchor.1 + 1);
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);

    set_standing(&mut app, key, SETTLEMENT_ALLIED_STANDING);
    app.pending_settlement = Some(key);
    app.mode = Mode::Settlement;
    (app, key)
}

fn roster_size(app: &App) -> usize {
    app.game.as_ref().unwrap().player_status().pet_count
}

#[test]
fn g_asks_the_town_for_a_program() {
    let (mut app, _key) = app_at_an_allied_town(7301);
    let before = roster_size(&app);

    app.handle_key(GameKey::Char('G'));

    assert_eq!(roster_size(&app), before + 1, "no program arrived");
    assert_eq!(app.status_line, None, "a granted gift refused");
}

/// The reach check `[M]` and `[J]` already make, for their reason: `x`
/// opens this page from anywhere inside `EXAMINE_RANGE_TILES`, and a town
/// read from across the map must not be asked for a favour.
#[test]
fn g_from_across_the_map_refuses_and_grants_nothing() {
    let (mut app, key) = app_at_an_allied_town(7302);
    let assets_dir = test_assets_dir();
    let path = scratch_path("aid_far", 7302);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.settlements.0.get_mut(&key).unwrap().tile = (400, 400);
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    let before = roster_size(&app);

    app.handle_key(GameKey::Char('G'));

    assert_eq!(roster_size(&app), before, "a distant town gifted anyway");
    assert!(
        app.status_line.is_some(),
        "the refusal never reached the screen"
    );
}

/// A refusal is one sentence on two surfaces — `App::refuse` is the door.
#[test]
fn a_refused_gift_reaches_both_the_screen_and_the_log() {
    let (mut app, key) = app_at_an_allied_town(7303);
    set_standing(&mut app, key, 0);

    app.handle_key(GameKey::Char('G'));

    let line = app.status_line.clone().expect("a refusal on the screen");
    let logged = app
        .game
        .as_mut()
        .unwrap()
        .message_history(200)
        .into_iter()
        .any(|row| row.text.contains(line.trim_end_matches('.')));
    assert!(logged, "the refusal never reached the log: {line}");
}

#[test]
fn t_on_the_town_page_sends_you_home_and_closes_the_page() {
    let (mut app, _key) = app_at_an_allied_town(7304);
    let anchor = app.game.as_mut().unwrap().anchor_position().unwrap();

    app.handle_key(GameKey::Char('T'));

    assert_eq!(app.game.as_ref().unwrap().player_status().position, anchor);
    assert_eq!(
        app.mode,
        Mode::Playing,
        "the page stayed open describing a town that is now far away"
    );
    assert_eq!(app.pending_settlement, None);
}

#[test]
fn t_on_the_town_page_refuses_below_allied_and_stays_put() {
    let (mut app, key) = app_at_an_allied_town(7305);
    set_standing(&mut app, key, 0);
    let before = app.game.as_ref().unwrap().player_status().position;

    app.handle_key(GameKey::Char('T'));

    assert_eq!(app.game.as_ref().unwrap().player_status().position, before);
    assert_eq!(app.mode, Mode::Settlement, "a refusal closed the page");
    assert!(app.status_line.is_some());
}

// ---------------------------------------------------------------------------
// `[T]` on the hub — the outbound half
// ---------------------------------------------------------------------------

/// A Relay, and one Allied destination known but never walked to.
fn app_at_a_hub_with_an_ally(seed: u32) -> (App, SettlementKey) {
    let mut app = app_at_a_relay(seed, &ItemId::from("cache_grain"), 10);
    let key = SettlementKey { rx: 6, ry: 6 };
    register_a_known_settlement(&mut app, key, (60, 60));
    set_standing(&mut app, key, SETTLEMENT_ALLIED_STANDING);
    app.mode = Mode::Dispatch;
    app.menu_selected = 0;
    (app, key)
}

/// The hub numbers sortie sites and route destinations continuously, so a
/// destination action on a site row has to refuse — `[C]`'s own rule.
#[test]
fn t_on_a_site_row_refuses() {
    let (mut app, _key) = app_at_a_hub_with_an_ally(7310);
    let (sites, _destinations) = app.dispatch_hub_sections().unwrap_or_default();
    if sites.is_empty() {
        return; // no site to stand on; the destination case below still runs
    }
    app.menu_selected = 0;
    let before = app.game.as_ref().unwrap().player_status().position;

    app.handle_key(GameKey::Char('T'));

    assert_eq!(app.game.as_ref().unwrap().player_status().position, before);
    assert!(app.status_line.is_some(), "a site row travelled");
}

#[test]
fn t_on_a_destination_row_travels_and_closes_the_hub() {
    let (mut app, key) = app_at_a_hub_with_an_ally(7311);
    let (sites, destinations) = app.dispatch_hub_sections().unwrap_or_default();
    assert!(
        !destinations.is_empty(),
        "the fixture registered no destination"
    );
    app.menu_selected = sites.len();
    let town = (60, 60);

    app.handle_key(GameKey::Char('T'));

    assert_eq!(
        app.status_line, None,
        "the trip was refused: {:?}",
        app.status_line
    );
    let landed = app.game.as_ref().unwrap().player_status().position;
    assert_ne!(landed, town, "the party landed on the settlement tile");
    // Near, not necessarily adjacent — the ring finds the nearest standable
    // ground, and the page opens only when that turned out to be in reach.
    let in_reach = (landed.0 - town.0).abs().max((landed.1 - town.1).abs()) <= 1;
    if in_reach {
        assert_eq!(
            app.mode,
            Mode::Settlement,
            "arriving in reach must open the page"
        );
        assert_eq!(app.pending_settlement, Some(key));
    } else {
        assert_eq!(
            app.mode,
            Mode::Playing,
            "a distant set-down must not open the page"
        );
    }
}

#[test]
fn t_on_a_destination_below_allied_refuses_and_stays_at_the_hub() {
    let (mut app, key) = app_at_a_hub_with_an_ally(7312);
    set_standing(&mut app, key, 0);
    app.mode = Mode::Dispatch;
    let (sites, _destinations) = app.dispatch_hub_sections().unwrap_or_default();
    app.menu_selected = sites.len();
    let before = app.game.as_ref().unwrap().player_status().position;

    app.handle_key(GameKey::Char('T'));

    assert_eq!(app.game.as_ref().unwrap().player_status().position, before);
    assert_eq!(app.mode, Mode::Dispatch);
    assert!(app.status_line.is_some());
}
