//! The stall a party meets in a corridor: its shelf, and the one extra
//! pick the narrowest routine rung takes.

use feral_processes_engine::MarketOffer;
use feral_processes_engine::items::ids;
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save;
use feral_processes_engine::stack::{CellKind, Dir, FrameSpec, generate};

use super::support::*;
use crate::app::stack_market::{MarketRow, market_row};
use crate::*;
use feral_processes_engine::{MarketOfferKind, RoutineScope};

/// An app standing on a Stack market, carrying `credits`.
///
/// Sweeps world seeds because only about a third of frames stand a stall
/// (`STACK_MARKET_CHANCE`) — a fixed seed here would be a fixture that
/// silently stopped finding one the first time that constant is retuned.
fn app_at_a_market(credits: u32) -> App {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let assets_dir = test_assets_dir();

    for seed in 900..960 {
        let mut app = test_app(seed);
        let path = std::env::temp_dir().join(format!(
            "feral_processes_appcore_market_{seed}_{unique}.sav"
        ));
        app.game.as_mut().unwrap().save(&path).unwrap();
        let mut data = save::load_from_file(&path).unwrap();

        let spec = FrameSpec {
            world_seed: data.seed,
            entrance: data.player.position,
            depth: 1,
            frames: 2,
        };
        let frame = generate(spec);
        let stall = (0..frame.height)
            .flat_map(|y| (0..frame.width).map(move |x| (x, y)))
            .find(|&(x, y)| frame.cell(x, y) == CellKind::Market);
        let Some((x, y)) = stall else {
            let _ = std::fs::remove_file(&path);
            continue;
        };

        data.player
            .inventory
            .push((ItemId::from(ids::CREDITS), credits));
        // A fresh player has one routine slot with `decompile` already in
        // it, so every rung would be *correctly* refused for want of
        // anywhere to write — which reads as a broken screen rather than as
        // a full player. Emptying the slot is what makes these tests about
        // the market.
        data.player.routines.clear();
        data.locale = Locale::Stack {
            depth: spec.depth,
            frames: spec.frames,
            x,
            y,
            facing: Dir::North,
            entrance: spec.entrance,
        };
        save::save_to_file(&path, &data).unwrap();
        app.game = Game::load(&path, &assets_dir).ok();
        let _ = std::fs::remove_file(&path);
        app.mode = Mode::Playing;
        return app;
    }
    panic!("no seed in the sweep put a market on its depth-1 frame");
}

fn shelf(app: &mut App) -> Vec<MarketOffer> {
    app.game.as_mut().unwrap().stack_market().unwrap().offers
}

/// The row number the drawn list gives an offer of `scope`.
fn row_of_scope(app: &mut App, want: RoutineScope) -> usize {
    shelf(app)
        .iter()
        .position(
            |offer| matches!(offer.kind, MarketOfferKind::Routine { scope, .. } if scope == want),
        )
        .expect("every shelf lists its routines at all three scopes")
}

/// The offset arithmetic, which is where a screen with two sections goes
/// wrong — and the one part of this flow testable without a stall to stand
/// at. Same reasoning as `trade_row`'s own test.
#[test]
fn market_rows_resolve_against_both_sections() {
    assert_eq!(market_row(0, 2, 3), Some(MarketRow::Offer(0)));
    assert_eq!(market_row(1, 2, 3), Some(MarketRow::Offer(1)));
    assert_eq!(market_row(2, 2, 3), Some(MarketRow::Sell(0)));
    assert_eq!(market_row(4, 2, 3), Some(MarketRow::Sell(2)));
    assert_eq!(market_row(5, 2, 3), None);
    // A stall bought out of everything still buys, so an offer-less screen
    // has to number its cargo rows from zero.
    assert_eq!(market_row(0, 0, 2), Some(MarketRow::Sell(0)));
}

/// `t` is the trade key wherever the party is standing. Underground it
/// opens whoever is selling *here* — the surface trader list scans from a
/// `Position` pinned to the entrance tile, so it would otherwise offer to
/// trade with a base four frames overhead.
#[test]
fn t_underground_opens_the_stall_underfoot() {
    let mut app = app_at_a_market(0);
    app.handle_key(GameKey::Char('t'));
    assert_eq!(app.mode, Mode::StackMarket);
}

#[test]
fn t_underground_with_no_stall_refuses_rather_than_listing_the_base() {
    let mut app = app_underground(4242);
    app.handle_key(GameKey::Char('t'));
    assert_eq!(
        app.mode,
        Mode::Playing,
        "the trader picker opened on a base the party cannot reach from here"
    );
    assert_eq!(
        app.status_line.as_deref(),
        Some("There's nobody selling anything here.")
    );
}

/// No rung opens a picker any more: every routine row sells disks into
/// cargo, and who ends up running them is a question the routine panel asks
/// later. This is the app-side half of what deleted `routine_recipients`.
#[test]
fn no_routine_rung_asks_who_it_is_for() {
    let mut app = app_at_a_market(5_000);
    app.handle_key(GameKey::Char('t'));

    for scope in [RoutineScope::One, RoutineScope::Everyone] {
        let mut app = app_at_a_market(5_000);
        app.handle_key(GameKey::Char('t'));
        let row = row_of_scope(&mut app, scope);
        app.handle_key(GameKey::Char(menu_shortcut(row)));
        assert_eq!(
            app.mode,
            Mode::StackMarket,
            "{scope:?} sent the player through a picker for a choice they do not have"
        );
    }
}

/// Buying leaves the screen open on the shelf — a visit is normally a run
/// of trades, exactly as it is at a surface post — and the disks land in
/// cargo rather than in anybody's slot.
#[test]
fn buying_a_rung_banks_the_disks_and_returns_to_the_shelf() {
    let mut app = app_at_a_market(5_000);
    app.handle_key(GameKey::Char('t'));
    let before = shelf(&mut app).len();
    let row = row_of_scope(&mut app, RoutineScope::One);
    let index = shelf(&mut app)[row].index;
    let ability = match &shelf(&mut app)[row].kind {
        MarketOfferKind::Routine { ability, .. } => ability.clone(),
        other => panic!("row {row} is {other:?}, not a routine"),
    };

    app.handle_key(GameKey::Char(menu_shortcut(row)));

    assert_eq!(app.mode, Mode::StackMarket);
    let game = app.game.as_ref().unwrap();
    assert_eq!(
        game.etched_disks_of(&ability),
        RoutineScope::One.disks(),
        "the rung's disks are not in cargo"
    );
    let after = shelf(&mut app);
    assert_eq!(after.len(), before - 1, "the row is still on the shelf");
    assert!(!after.iter().any(|offer| offer.index == index));
}

/// A screen with no stall under it is a screen with nothing on it. The
/// shelf can empty while it is open — the last row bought is the usual way
/// — so the handler answers to the stall on every key rather than trusting
/// the mode it was opened in.
#[test]
fn a_market_screen_with_no_stall_under_it_drops_back_to_the_map() {
    let mut app = app_underground(3131);
    app.mode = Mode::StackMarket;
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Playing);
}

/// Selling is one unit a press, with the whole stack on the shifted key —
/// uppercase for the reason the trade screen reserves it, and bulk on the
/// shifted key specifically because this trader keeps no buyback shelf to
/// undo a mis-hit from.
#[test]
fn a_cargo_row_sells_one_on_a_pick_and_the_stack_on_shift() {
    let held = |app: &mut App| -> u32 {
        app.game
            .as_mut()
            .unwrap()
            .stack_market()
            .map(|view| {
                view.sells
                    .iter()
                    .find(|row| row.copy.item.as_str() == ids::CORE_FRAGMENT)
                    .map(|row| row.qty)
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    };

    let mut app = app_at_a_market(0);
    app.handle_key(GameKey::Char('t'));
    let offers = shelf(&mut app).len();
    let stock = held(&mut app);
    assert!(stock > 1, "the player starts with salvage to sell");

    let row = app
        .game
        .as_mut()
        .unwrap()
        .stack_market()
        .unwrap()
        .sells
        .iter()
        .position(|row| row.copy.item.as_str() == ids::CORE_FRAGMENT)
        .expect("salvage is sellable");
    app.handle_key(GameKey::Char(menu_shortcut(offers + row)));
    assert_eq!(held(&mut app), stock - 1, "a pick sold more than one unit");

    // `S` acts on the highlighted row, so put the highlight on it first.
    app.menu_selected = offers + row;
    app.handle_key(GameKey::Char('S'));
    assert_eq!(
        held(&mut app),
        0,
        "shift-sell left part of the stack behind"
    );
}

/// A Stack stall's cargo rows name the player's own gear, so `[I]` answers
/// there too. The offer rows do not: a routine disk or a program already
/// carries its own `detail` line, and neither is a `GearCopy`.
#[test]
fn a_cargo_row_opens_the_inspect_page_and_an_offer_row_does_not() {
    let mut app = app_at_a_market(0);
    app.handle_key(GameKey::Char('t'));
    assert_eq!(app.mode, Mode::StackMarket);
    let offers = shelf(&mut app).len();

    app.menu_selected = 0;
    app.handle_key(GameKey::Char('I'));
    assert_eq!(
        app.mode,
        Mode::StackMarket,
        "an offer row has no carried copy to describe"
    );
    assert!(app.pending_inspect.is_none());

    app.menu_selected = offers;
    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    assert!(app.pending_inspect.is_some());

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::StackMarket,
        "back to the stall it opened from"
    );
}
