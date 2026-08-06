//! `T` in the battle item picker — throwing the highlighted consumable at
//! the wild group instead of using it.
//!
//! The key is deliberately named by nothing on screen; see
//! `crates/engine/EASTER_EGGS.md`.

use super::support::*;
use crate::tuning::THROWN_ITEM_DAMAGE;
use crate::*;

fn game() -> Game {
    Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn cell() -> ItemId {
    ItemId::from(ids::POWER_CELL)
}

fn held(game: &Game, item: &ItemId) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .items
        .iter()
        .find(|(id, _)| id == item)
        .map(|&(_, qty)| qty)
        .unwrap_or(0)
}

fn hp(game: &Game, entity: Entity) -> i32 {
    game.world.get::<Stats>(entity).unwrap().hp
}

/// A fight against one wild program on `hp`, with three power cells in
/// cargo to throw at it.
fn battle_with_a_target_on(game: &mut Game, hp: i32) -> Entity {
    set_inventory(game, &[(ids::POWER_CELL, 3)]);
    let enemy = battle_with_a_pack_of(game, 1, hp)[0];
    assert!(
        game.battle_usable_items().contains(&cell()),
        "the fixture's power cells should be throwable"
    );
    enemy
}

#[test]
fn throwing_spends_exactly_one_unit_of_the_stack() {
    let mut game = game();
    battle_with_a_target_on(&mut game, 30);

    game.throw_item(&cell()).unwrap();

    assert_eq!(held(&game, &cell()), 2, "a throw should cost one unit");
}

#[test]
fn a_thrown_item_takes_hp_off_the_front_of_the_wild_group() {
    let mut game = game();
    let enemy = battle_with_a_target_on(&mut game, 30);

    game.throw_item(&cell()).unwrap();

    assert_eq!(hp(&game, enemy), 30 - THROWN_ITEM_DAMAGE);
}

/// The clamp, and the reason for it. A kill resolving from outside the
/// round loop would end a battle next to `BattleState::planned`'s
/// positional indexing into `Party` — clamping at 1 makes that state
/// unreachable rather than merely unlikely.
#[test]
fn a_throw_cannot_take_the_target_below_one_hp() {
    let mut game = game();
    let enemy = battle_with_a_target_on(&mut game, 1);

    game.throw_item(&cell()).unwrap();

    assert_eq!(hp(&game, enemy), 1, "the throw was lethal");
    assert!(
        game.has_active_battle(),
        "a throw ended the battle from outside the round loop"
    );
    assert_eq!(
        held(&game, &cell()),
        2,
        "a throw that bounced off still costs the item"
    );
}

#[test]
fn throwing_something_not_in_cargo_is_refused_and_costs_nothing() {
    let mut game = game();
    let enemy = battle_with_a_target_on(&mut game, 30);
    let absent = ItemId::from(ids::PATCH_ROUTINE);

    assert!(game.throw_item(&absent).is_err());

    assert_eq!(held(&game, &absent), 0);
    assert_eq!(held(&game, &cell()), 3, "the refusal spent the wrong stack");
    assert_eq!(hp(&game, enemy), 30, "the refusal did damage anyway");
}

#[test]
fn throwing_outside_a_battle_is_refused() {
    let mut game = game();
    set_inventory(&mut game, &[(ids::POWER_CELL, 3)]);

    assert!(game.throw_item(&cell()).is_err());

    assert_eq!(held(&game, &cell()), 3, "the refusal spent the item anyway");
}
