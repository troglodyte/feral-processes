//! Selling programs, and the confirmation that gates it.

use feral_processes_engine::items::ids;

use super::support::*;
use crate::*;

/// Opens the trade screen on the fixture's Black Market: `t`'s picker, then
/// its only nearby trader.
fn open_the_trading_post(app: &mut App) {
    app.mode = Mode::Trade;
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::TradeAction);
}

fn held(app: &App, item: &str) -> u32 {
    app.game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .find(|(i, _)| i.as_str() == item)
        .map(|(_, qty)| *qty)
        .unwrap_or(0)
}

/// Selling is reachable straight from the inventory, but only when there is
/// somewhere to sell: the action is hidden rather than offered-and-refused.
#[test]
fn sell_is_offered_from_the_inventory_only_with_a_trading_post_in_range() {
    let keys = |app: &mut App| -> Vec<char> {
        let item = ItemId::from(ids::CORE_FRAGMENT);
        let game = app.game.as_mut().unwrap();
        inventory_item_actions(game, &item)
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    };

    let mut at_post = app_at_a_trading_post(921, &[(ids::CORE_FRAGMENT, 5)]);
    assert!(
        keys(&mut at_post).contains(&'s'),
        "a trading post is in range, so selling should be on offer"
    );

    let mut in_the_field = test_app(922);
    assert!(
        !keys(&mut in_the_field).contains(&'s'),
        "with nowhere to sell, the action should not be listed at all"
    );
}

/// The mirror of `selling_an_item_lands_back_on_the_traders_list`: where a
/// sale returns you depends on where you started it. Coming from the
/// inventory, the trader's list is a screen the player never opened.
#[test]
fn selling_from_the_inventory_lands_back_in_the_inventory() {
    let mut app = app_at_a_trading_post(923, &[(ids::CORE_FRAGMENT, 5)]);
    app.pending_inventory_item = Some(ItemId::from(ids::CORE_FRAGMENT));
    app.mode = Mode::InventoryItemAction;

    app.handle_key(GameKey::Char('s'));
    assert_eq!(
        app.mode,
        Mode::TradeQuantity,
        "one trader in range, so the picker should be skipped"
    );
    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.mode,
        Mode::Inventory,
        "a sale begun in the inventory returns to the inventory"
    );
    assert_eq!(held(&app, ids::CORE_FRAGMENT), 4, "one unit was sold");
}

/// A sale is one of a run of them — you clear out a full pack a stack at a
/// time — so finishing one leaves the player on the trader's list, not back
/// on the map having to walk the whole menu path again.
#[test]
fn selling_an_item_lands_back_on_the_traders_list() {
    let mut app = app_at_a_trading_post(920, &[(ids::CREDITS, 4), (ids::CORE_FRAGMENT, 5)]);
    open_the_trading_post(&mut app);

    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::TradeQuantity);
    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::TradeAction, "the visit continues");
    assert!(
        app.pending_trade_structure.is_some(),
        "staying on the list means staying with the trader that drew it"
    );
    assert_eq!(held(&app, ids::CORE_FRAGMENT), 4, "one fragment sold");
    assert!(held(&app, ids::CREDITS) > 4, "and paid for");
    assert!(app.pending_trade_choice.is_none());
}

/// Rows shift as stock sells out, so the highlight goes back to the top
/// rather than staying on a line number that now means something else.
#[test]
fn a_completed_sale_puts_the_highlight_back_on_the_first_row() {
    let mut app = app_at_a_trading_post(921, &[(ids::CREDITS, 4), (ids::CORE_FRAGMENT, 1)]);
    open_the_trading_post(&mut app);
    app.menu_selected = 3;

    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);

    assert_eq!(app.menu_selected, 0);
}

/// The row the player picks has to be the row the renderer drew. Both
/// sides list the inventory minus the trade currency — Credits, which a
/// trader won't buy for more Credits — and *not* minus the build salvage,
/// which is ordinary goods to a trader and the main thing there is to sell.
#[test]
fn the_sell_list_hides_credits_and_offers_the_salvage() {
    let mut app = app_at_a_trading_post(922, &[(ids::CREDITS, 4), (ids::CORE_FRAGMENT, 5)]);
    open_the_trading_post(&mut app);

    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::TradeQuantity);
    assert!(
        matches!(&app.pending_trade_choice, Some(TradeChoice::Sell(item))
            if item.as_str() == ids::CORE_FRAGMENT),
        "the first sell row is the salvage, not the money"
    );
}

/// A program sale ends the same way an item sale does — back on the list,
/// which is where the payout can be spent.
#[test]
fn selling_a_program_lands_back_on_the_traders_list() {
    let mut app = app_at_a_trading_post(923, &[(ids::CREDITS, 4)]);
    open_the_trading_post(&mut app);
    let structure = app.pending_trade_structure.unwrap();
    let programs = app.game.as_mut().unwrap().program_sale_options(structure);
    assert_eq!(programs.len(), 1, "the fixture owns exactly one program");

    app.pending_trade_program = Some(programs[0].clone());
    app.mode = Mode::TradeProgramConfirm;
    app.handle_key(GameKey::Char('y'));

    assert_eq!(app.mode, Mode::TradeAction);
    assert!(app.pending_trade_structure.is_some());
    assert!(held(&app, ids::CREDITS) > 4, "the program sold");
}

/// Esc out of the sale confirmation abandons it and steps back to the
/// trade list, leaving the program alive and nothing pending.
///
/// The sale itself is not driven from here: staging a trading post needs
/// a Home, build clearance and 16 Core Fragments, and there is no public
/// way to grant those — the same reach limit that keeps multi-group
/// battle logic in engine tests. `sell_companion` is covered there.
#[test]
fn escaping_the_program_sale_confirmation_sells_nothing() {
    let mut app = test_app(910);
    app.mode = Mode::TradeProgramConfirm;
    app.pending_trade_program = Some(ProgramSaleOption {
        entity: Entity::from_raw_u32(1).unwrap(),
        name: "Sparkgrub".to_string(),
        level: 4,
        power: 62,
        payout: 6,
        activity: "in party".to_string(),
        detaches: vec!["leaves your battle party".to_string()],
    });

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::TradeAction);
    assert!(
        app.pending_trade_program.is_none(),
        "backing out has to drop the pending sale, not leave it armed"
    );
}

/// Declining is not the same key as backing out, but has to do the same
/// thing — a mis-hit `n` must never sell.
#[test]
fn declining_the_program_sale_confirmation_sells_nothing() {
    let mut app = test_app(911);
    app.mode = Mode::TradeProgramConfirm;
    app.pending_trade_program = Some(ProgramSaleOption {
        entity: Entity::from_raw_u32(1).unwrap(),
        name: "Sparkgrub".to_string(),
        level: 4,
        power: 62,
        payout: 6,
        activity: "idle".to_string(),
        detaches: Vec::new(),
    });

    app.handle_key(GameKey::Char('n'));

    assert_eq!(app.mode, Mode::TradeAction);
    assert!(app.pending_trade_program.is_none());
}

/// The buyback shelf slots between the buy list and the programs, so both
/// the sections before it keep their row numbers and the programs — which
/// branch to a confirmation instead of a quantity — stay last.
///
/// Tested against the arithmetic directly rather than by driving the
/// screen: staging a trading post needs a Home, build clearance and 16 Core
/// Fragments, and the player starts with 5.
#[test]
fn buyback_rows_are_numbered_between_the_buy_list_and_the_programs() {
    use crate::app::trade::{TradeRow, trade_row};

    let (sells, buys, buybacks, programs) = (2, 3, 2, 1);
    let row = |i| trade_row(i, sells, buys, buybacks, programs);

    assert_eq!(row(0), Some(TradeRow::Sell(0)));
    assert_eq!(row(1), Some(TradeRow::Sell(1)));
    assert_eq!(row(2), Some(TradeRow::Buy(0)));
    assert_eq!(row(4), Some(TradeRow::Buy(2)));
    assert_eq!(row(5), Some(TradeRow::BuyBack(0)));
    assert_eq!(row(6), Some(TradeRow::BuyBack(1)));
    assert_eq!(row(7), Some(TradeRow::Program(0)));
    assert_eq!(row(8), None, "a row past the last one picks nothing");
}

/// An empty shelf must not shift the rows under the player — which is what
/// makes this change invisible at a trader they've never sold to.
#[test]
fn an_empty_shelf_leaves_the_other_sections_where_they_were() {
    use crate::app::trade::{TradeRow, trade_row};

    assert_eq!(trade_row(2, 2, 3, 0, 1), Some(TradeRow::Buy(0)));
    assert_eq!(trade_row(5, 2, 3, 0, 1), Some(TradeRow::Program(0)));
}

/// The confirmation screen is not part of an intrusion, and the
/// exhaustive match in `Mode::is_battle` is what forces that to be
/// decided rather than defaulted.
#[test]
fn the_program_sale_confirmation_is_not_a_battle_screen() {
    assert!(!Mode::TradeProgramConfirm.is_battle());
}
