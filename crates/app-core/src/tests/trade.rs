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
        .find(|r| r.item.as_str() == item)
        .map(|r| r.qty)
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
    app.pending_inventory_item = Some((ItemId::from(ids::CORE_FRAGMENT), 0));
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
        matches!(&app.pending_trade_choice, Some(TradeChoice::Sell(item, _))
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
        fusions: 0,
        rarity: Default::default(),
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
        fusions: 0,
        rarity: Default::default(),
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

/// `S` sells one unit off the highlighted row without a trip through the
/// quantity page, and leaves you on the list — a trade visit is normally a
/// run of trades, so the screen you want next is the one you're on.
#[test]
fn capital_s_sells_one_unit_from_the_highlighted_row() {
    let mut app = app_at_a_trading_post(940, &[(ids::CORE_FRAGMENT, 5)]);
    open_the_trading_post(&mut app);
    app.menu_selected = 0;

    app.handle_key(GameKey::Char('S'));

    assert_eq!(held(&app, ids::CORE_FRAGMENT), 4, "exactly one unit moves");
    assert_eq!(app.mode, Mode::TradeAction, "the visit continues");

    app.handle_key(GameKey::Char('S'));
    assert_eq!(held(&app, ids::CORE_FRAGMENT), 3, "presses accumulate");
}

/// `S`'s mirror on the trader's own stock. The pack is stocked with the
/// item being bought so the sell section keeps its length across the
/// purchase — otherwise the new stack would open a sell row and shift every
/// buy row down under the player's highlight.
#[test]
fn capital_b_buys_one_unit_from_the_highlighted_row() {
    let mut app = app_at_a_trading_post(946, &[(ids::CREDITS, 500), (ids::POWER_CELL, 1)]);
    open_the_trading_post(&mut app);
    let structure = app.pending_trade_structure.unwrap();
    let buy = app
        .game
        .as_mut()
        .unwrap()
        .trade_options(structure)
        .unwrap()
        .buy;
    let offered = buy
        .iter()
        .position(|(i, _)| i.as_str() == ids::POWER_CELL)
        .expect("the Black Market stocks Power Cells");
    // One sell row: Credits are filtered out of the sell list, leaving the
    // Power Cells.
    app.menu_selected = 1 + offered;

    app.handle_key(GameKey::Char('B'));

    assert_eq!(held(&app, ids::POWER_CELL), 2, "exactly one unit moves");
    assert!(held(&app, ids::CREDITS) < 500, "and was paid for");
    assert_eq!(app.mode, Mode::TradeAction, "the visit continues");

    app.handle_key(GameKey::Char('B'));
    assert_eq!(held(&app, ids::POWER_CELL), 3, "presses accumulate");
}

/// Each row is either a sell or a buy, so the wrong key for a row is a
/// mis-hit. It says so rather than guessing which one you meant.
#[test]
fn the_wrong_direction_key_transacts_nothing_and_says_why() {
    let mut app = app_at_a_trading_post(941, &[(ids::CORE_FRAGMENT, 5)]);
    open_the_trading_post(&mut app);
    app.menu_selected = 0;

    app.handle_key(GameKey::Char('B'));

    assert_eq!(held(&app, ids::CORE_FRAGMENT), 5, "nothing moved");
    assert_eq!(app.mode, Mode::TradeAction);
    assert!(
        app.status_line.is_some(),
        "a key that did nothing must say why"
    );
}

/// The one row `S` refuses to make faster. Selling a levelled program is
/// permanent, and a quick key is exactly a mis-hit risk — so it lands on
/// the same confirmation Enter does.
#[test]
fn capital_s_on_a_program_row_still_asks_for_confirmation() {
    let mut app = app_at_a_trading_post(942, &[]);
    open_the_trading_post(&mut app);
    let structure = app.pending_trade_structure.unwrap();
    let game = app.game.as_mut().unwrap();
    // Programs are the last rows; with an empty pack and an untouched
    // shelf, everything before them is the trader's own buy list.
    let first_program_row = game.trade_options(structure).unwrap().buy.len();
    assert_eq!(game.program_sale_options(structure).len(), 1);
    app.menu_selected = first_program_row;

    app.handle_key(GameKey::Char('S'));

    assert_eq!(
        app.mode,
        Mode::TradeProgramConfirm,
        "a program sale must be confirmed, however it was reached"
    );
    let structure = app.pending_trade_structure.unwrap();
    assert_eq!(
        app.game
            .as_mut()
            .unwrap()
            .program_sale_options(structure)
            .len(),
        1,
        "the program is alive until the confirmation is answered"
    );
}

/// `S` in the pack sells one to the trader in range, skipping the item
/// action page, the trader picker and the quantity page.
#[test]
fn capital_s_in_the_inventory_sells_one_to_the_only_trader_in_range() {
    let mut app = app_at_a_trading_post(943, &[(ids::CORE_FRAGMENT, 5)]);
    app.mode = Mode::Inventory;
    // Three equipment slot rows come before the pack.
    app.menu_selected = 3;

    app.handle_key(GameKey::Char('S'));

    assert_eq!(held(&app, ids::CORE_FRAGMENT), 4);
    assert_eq!(app.mode, Mode::Inventory, "you stay in your pack");
}

/// With nowhere to sell, `[S]ell` is not offered on the action page either
/// — so the quick key says so rather than silently doing nothing.
#[test]
fn capital_s_in_the_inventory_needs_a_trader_in_range() {
    let mut app = test_app(944);
    app.mode = Mode::Inventory;
    app.menu_selected = 3;

    app.handle_key(GameKey::Char('S'));

    assert_eq!(app.mode, Mode::Inventory);
    assert!(
        app.status_line.is_some(),
        "a key that did nothing must say why"
    );
}

/// The quick key carries no rules of its own about what may be sold — it
/// calls `Game::sell_item` and surfaces whatever comes back. The trade
/// screen filters the currency out of its sell list, but the pack lists
/// everything, so this is the row that proves it. A second copy of the rule
/// here is the copy that would drift.
#[test]
fn capital_s_on_the_currency_is_refused_by_the_engine() {
    let mut app = app_at_a_trading_post(947, &[(ids::CREDITS, 4), (ids::CORE_FRAGMENT, 5)]);
    app.mode = Mode::Inventory;
    let inventory = app.game.as_ref().unwrap().player_status().inventory;
    let row = inventory
        .iter()
        .position(|r| r.item.as_str() == ids::CREDITS)
        .expect("the pack was stocked with Credits");
    // Three equipment slot rows come before the pack.
    app.menu_selected = 3 + row;

    app.handle_key(GameKey::Char('S'));

    assert_eq!(held(&app, ids::CREDITS), 4, "the money is not merchandise");
    assert_eq!(app.mode, Mode::Inventory);
    assert!(
        app.status_line
            .as_ref()
            .is_some_and(|line| line.contains("Credits")),
        "the engine's own refusal is what reaches the player"
    );
}

/// Two traders is a question the key cannot answer, so it asks instead of
/// picking a shop for you — the existing picker, with the item already
/// decided.
#[test]
fn capital_s_with_two_traders_in_range_asks_which_one() {
    let mut app = app_at_trading_posts(945, &[(ids::CORE_FRAGMENT, 5)], 2);
    app.mode = Mode::Inventory;
    app.menu_selected = 3;

    app.handle_key(GameKey::Char('S'));

    assert_eq!(app.mode, Mode::Trade, "the trader picker opens");
    assert!(
        matches!(&app.pending_trade_choice, Some(TradeChoice::Sell(item, _))
            if item.as_str() == ids::CORE_FRAGMENT),
        "the item is already decided; only the shop is in question"
    );
    assert_eq!(held(&app, ids::CORE_FRAGMENT), 5, "nothing sold yet");
}

/// The sell list needs no filter of its own for this: it is built from
/// `PlayerStatus::inventory`, which omits banked items, and that is the
/// whole reason the filter lives in the engine rather than on each screen.
/// Asserted through the row *indexing* rather than by inspecting the list,
/// because a banked row that survived would not merely be visible — it
/// would shift every row after it, selling the item below the one the
/// player is looking at.
#[test]
fn the_sell_list_hides_a_banked_item() {
    let mut app = app_at_a_trading_post(
        924,
        &[
            (ids::CREDITS, 4),
            (ids::RESEARCH_DATA, 40),
            (ids::CORE_FRAGMENT, 5),
        ],
    );
    open_the_trading_post(&mut app);

    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::TradeQuantity);
    assert!(
        matches!(&app.pending_trade_choice, Some(TradeChoice::Sell(item, _))
            if item.as_str() == ids::CORE_FRAGMENT),
        "the first sell row must be the salvage — Research Data is banked and \
         Credits are the currency, so neither is a sell row"
    );
}
