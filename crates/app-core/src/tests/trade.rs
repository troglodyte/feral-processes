//! Selling programs, and the confirmation that gates it.

use super::support::*;
use crate::*;

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
