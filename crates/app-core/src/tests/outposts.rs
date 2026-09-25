//! An outpost's own page (`Mode::OutpostVisit`), opened by walking onto its
//! tile, and its staff picker (`Mode::OutpostPost`) — `settlement.rs`'s
//! shape one landmark over.

use super::support::*;
use crate::*;
use feral_processes_engine::save;

#[test]
fn bumping_an_outpost_opens_its_page_with_the_tile_set() {
    let mut app = test_app(970);
    let tile = place_outpost_east_of_player(&mut app);

    walk(&mut app, GameKey::Right);

    assert_eq!(app.mode, Mode::OutpostVisit);
    assert_eq!(app.pending_outpost, Some(tile));
}

/// The tile does not admit you, `Game::move_player`'s outpost arm returning
/// before the walkable step below it ever runs — `settlement.rs`'s own
/// pinned behaviour, one landmark over.
#[test]
fn bumping_an_outpost_does_not_move_the_player() {
    let mut app = test_app(971);
    place_outpost_east_of_player(&mut app);
    let before = app.game.as_ref().unwrap().player_status().position;

    walk(&mut app, GameKey::Right);

    let after = app.game.as_ref().unwrap().player_status().position;
    assert_eq!(before, after, "an outpost admits nobody");
}

#[test]
fn esc_returns_to_playing_and_clears_the_pending_outpost() {
    let mut app = test_app(972);
    place_outpost_east_of_player(&mut app);
    walk(&mut app, GameKey::Right);
    assert_eq!(app.mode, Mode::OutpostVisit);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.pending_outpost, None);
}

/// The drain, `an_unrelated_action_after_esc_does_not_reopen_the_page`'s own
/// reproducer: `Game::take_visit` only ever answers `Some` once, and a wait
/// spent after Esc has already backed out of the page must not reopen it.
#[test]
fn an_unrelated_action_after_esc_does_not_reopen_the_outpost_page() {
    let mut app = test_app(973);
    place_outpost_east_of_player(&mut app);
    walk(&mut app, GameKey::Right);
    assert_eq!(app.mode, Mode::OutpostVisit);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char('.'));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "an outpost bump queues no second visit, so nothing should have reopened the page"
    );
}

/// I4: `x` toward an outpost finds it through `find_target_in_direction` and
/// opens the same page a bump would — `examining_a_settlement_opens_the_
/// same_page`'s shape, `InspectTarget::Outpost` one fixture over.
#[test]
fn examining_an_outpost_opens_the_same_page() {
    let mut app = test_app(985);
    let tile = place_outpost_east_of_player(&mut app);

    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::InspectDirection);
    app.handle_key(GameKey::Right);

    assert_eq!(app.mode, Mode::OutpostVisit);
    assert_eq!(app.pending_outpost, Some(tile));
}

/// `[P]` opens the staff picker, and a letter posts the highlighted
/// candidate — driven entirely by keypress, `Mode::SortieSquad`'s own
/// candidate list one screen over.
#[test]
fn p_then_a_letter_posts_a_staff_program_and_returns_to_the_visit_page() {
    let mut app = test_app(974);
    place_outpost_with_a_staff_program_east_of_player(&mut app);
    walk(&mut app, GameKey::Right);
    assert_eq!(app.mode, Mode::OutpostVisit);

    app.handle_key(GameKey::Char('P'));
    assert_eq!(app.mode, Mode::OutpostPost);
    assert_eq!(app.outpost_post_candidates().len(), 1);

    app.handle_key(GameKey::Char('a'));

    assert_eq!(app.mode, Mode::OutpostVisit);
    assert_eq!(
        app.outpost_report().unwrap().crew.len(),
        1,
        "posting the only candidate must land it on the crew list"
    );
}

/// Esc from the picker backs out to the visit page without posting anyone.
#[test]
fn esc_from_the_post_picker_posts_nobody() {
    let mut app = test_app(975);
    place_outpost_with_a_staff_program_east_of_player(&mut app);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('P'));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::OutpostVisit);
    assert_eq!(app.outpost_report().unwrap().crew.len(), 0);
}

/// `[U]` recalls the crew row a lowercase letter selected — the screen's
/// own two-step convention (design spec §9): select, then act.
#[test]
fn a_letter_then_u_recalls_the_selected_crew_member() {
    let mut app = test_app(976);
    place_outpost_with_a_staff_program_east_of_player(&mut app);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('P'));
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.outpost_report().unwrap().crew.len(), 1);

    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('U'));

    assert_eq!(
        app.outpost_report().unwrap().crew.len(),
        0,
        "U must recall the row the letter selected"
    );
}

/// `[R]` refuses at full integrity — there is nothing to repair, and
/// `Game::repair_outpost` says so rather than spending materials on a no-op.
#[test]
fn r_refuses_at_full_integrity() {
    let mut app = test_app(977);
    place_outpost_east_of_player(&mut app);
    walk(&mut app, GameKey::Right);

    app.handle_key(GameKey::Char('R'));

    assert!(app.status_line.is_some(), "R must refuse, not act silently");
    assert_eq!(app.mode, Mode::OutpostVisit);
}

/// `[R]` repairs a damaged outpost — design spec §8, wired in Phase 5.
/// Stocked generously past the shipped `repair_cost` (Logic Wafers and
/// Bytecode Blocks) so the test stays about the key reaching
/// `Game::repair_outpost`, not about matching a content decision exactly.
#[test]
fn r_repairs_a_damaged_outpost() {
    let mut app = test_app(978);
    place_damaged_outpost_east_of_player(&mut app, 10);
    give_player_items(&mut app, &[("logic_wafer", 200), ("bytecode_block", 200)]);
    walk(&mut app, GameKey::Right);

    app.handle_key(GameKey::Char('R'));

    assert_eq!(
        app.outpost_report().unwrap().integrity,
        feral_processes_engine::tuning::OUTPOST_MAX_INTEGRITY,
        "R must restore full integrity"
    );
}

/// `C` opens the transfer picker against the outpost's own stock — design
/// correction 11's `TransferSource::Outpost`. Uppercase (I2): lowercase `c`
/// is the third crew row (`OUTPOST_CREW_CAP` 6 → rows a-f) and would collide.
#[test]
fn shift_c_opens_transfer_with_the_outposts_stock() {
    let mut app = test_app(978);
    let tile = place_outpost_with_stock(&mut app, &[("raw_trace", 4)]);
    walk(&mut app, GameKey::Right);

    app.handle_key(GameKey::Char('C'));

    assert_eq!(app.mode, Mode::Transfer);
    assert_eq!(app.transfer_source, TransferSource::Outpost(tile));
    assert_eq!(app.basket_rows.len(), 1);
    assert_eq!(
        app.basket_room, None,
        "an outpost row can never be put into"
    );
}

/// The commit reaches `Game::take_from_outpost` rather than
/// `Game::transfer_items` — the whole point of `TransferSource`. A taken
/// unit must leave the outpost's stock and land in the pack, and the screen
/// must return to `Mode::OutpostVisit`, not the raw map.
#[test]
fn committing_a_take_spends_through_take_from_outpost_and_returns_to_the_visit_page() {
    let mut app = test_app(979);
    place_outpost_with_stock(&mut app, &[("raw_trace", 4)]);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('C'));
    assert_eq!(app.mode, Mode::Transfer);
    // Take everything on the one row.
    app.handle_key(GameKey::ShiftLeft);

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::OutpostVisit);
    assert_eq!(
        app.outpost_report().unwrap().stock,
        0,
        "the take must have emptied the outpost's stock"
    );
}

/// Esc from the outpost's own transfer picker returns to the visit page
/// too, `commit_transfer`'s own mirror on the cancel path.
#[test]
fn esc_from_the_outposts_transfer_picker_returns_to_the_visit_page() {
    let mut app = test_app(980);
    place_outpost_with_stock(&mut app, &[("raw_trace", 4)]);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('C'));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::OutpostVisit);
}

/// `c` selects the third crew row and `C` opens transfer — the two must not
/// collide (I2). `OUTPOST_CREW_CAP` is 6, rows a-f, so three crew puts a
/// selectable row at `c`.
#[test]
fn lowercase_c_selects_a_crew_row_and_uppercase_c_opens_transfer() {
    let mut app = test_app(981);
    place_outpost_with_crew_and_stock(&mut app, 3, &[("raw_trace", 4)]);
    walk(&mut app, GameKey::Right);
    assert_eq!(app.mode, Mode::OutpostVisit);
    assert_eq!(app.outpost_report().unwrap().crew.len(), 3);

    app.handle_key(GameKey::Char('c'));
    assert_eq!(
        app.menu_selected, 2,
        "lowercase c must select the third crew row, not open transfer"
    );
    assert_eq!(app.mode, Mode::OutpostVisit);

    app.handle_key(GameKey::Char('C'));
    assert_eq!(app.mode, Mode::Transfer, "uppercase C must open transfer");
}

/// `place_outpost_east_of_player` plus a fixed stock, for the transfer
/// tests above — its own tiny save edit rather than a shared fixture
/// parameter, since nothing else needs an outpost with stock on it yet.
fn place_outpost_with_stock(app: &mut App, stock: &[(&str, u32)]) -> (i32, i32) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("outpost_stock", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let target = (px + 1, py);
    data.creatures.retain(|c| c.position != target);
    data.nests.retain(|n| n.position != target);
    data.link_sites.retain(|&site| site != target);
    data.settlements.0.retain(|_, s| s.tile != target);
    data.outposts.push(save::OutpostSave {
        tile: target,
        biome: feral_processes_engine::world::Biome::Deadlock,
        growth: 0,
        integrity: feral_processes_engine::tuning::OUTPOST_MAX_INTEGRITY,
        stock: stock
            .iter()
            .map(|(id, qty)| (feral_processes_engine::items::ItemId(id.to_string()), *qty))
            .collect(),
        stale_ticks: 0,
        cycle_progress: 0,
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    target
}
