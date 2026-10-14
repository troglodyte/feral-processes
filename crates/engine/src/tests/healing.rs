//! Healing spends `patch_routine`: a rest in base buys Integrity at
//! `REST_INTEGRITY_PER_PATCH` a patch, and the base's trickle is the free
//! floor under it. The Repair Bay's patches are `tests::repair_bay_supply`.

use super::support::*;
use crate::tuning::{BASE_TRICKLE_PER_TICK, REST_INTEGRITY_PER_PATCH};
use crate::*;

fn base() -> Game {
    let mut game = Game::new(4401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    stand_in_base(&mut game);
    game
}

fn set_hp(game: &mut Game, who: Entity, hp: i32) {
    let mut stats = game.world.get_mut::<Stats>(who).unwrap();
    stats.max_hp = stats.max_hp.max(hp);
    stats.hp = hp;
}

/// Sets the player to `max` Integrity with `missing` of it gone.
fn hurt_player(game: &mut Game, max: i32, missing: i32) -> Entity {
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().max_hp = max;
    set_hp(game, player, max - missing);
    player
}

fn patches(game: &Game) -> u32 {
    crate::game::base::work_orders::base_holding(game, &ItemId::from(ids::PATCH_ROUTINE))
}

fn hp(game: &Game, who: Entity) -> i32 {
    game.world.get::<Stats>(who).unwrap().hp
}

#[test]
fn a_rest_in_base_spends_one_patch_per_forty_integrity_restored() {
    let mut game = base();
    let player = hurt_player(&mut game, 200, 100);
    stock_patches(&mut game, 10);

    game.rest().unwrap();

    assert_eq!(hp(&game, player), 200, "fully repaired");
    assert_eq!(
        patches(&game),
        10 - 100u32.div_ceil(REST_INTEGRITY_PER_PATCH as u32),
        "100 missing is three patches, the last one part-used"
    );
}

#[test]
fn a_rest_with_nothing_to_mend_spends_nothing() {
    let mut game = base();
    hurt_player(&mut game, 200, 0);
    stock_patches(&mut game, 4);

    game.rest().unwrap();

    assert_eq!(patches(&game), 4);
}

#[test]
fn a_rest_is_never_refused_for_want_of_patches() {
    let mut game = base();
    let player = hurt_player(&mut game, 200, 100);
    let power = game.max_power(player);
    game.world
        .get_mut::<PowerReserve>(player)
        .unwrap()
        .spend(power);

    assert!(game.rest().is_ok(), "rest is never gated");

    assert_eq!(hp(&game, player), 100, "no patches, no Integrity");
    assert_eq!(
        game.world.get::<PowerReserve>(player).unwrap().get(),
        power,
        "Power is free and refills regardless"
    );
}

#[test]
fn the_party_heals_worst_hurt_first_as_far_as_the_patches_cover() {
    let mut game = base();
    // Player at 60%, companion at 25%: the companion is worse by fraction
    // though the player is missing more points.
    let player = hurt_player(&mut game, 200, 80);
    let companion = spawn_tamed(&mut game, 40, 3);
    enlist(&mut game, companion);
    set_hp(&mut game, companion, 10);
    // Two patches buy 80 Integrity: the companion's 30 and 50 of the player's 80.
    stock_patches(&mut game, 2);

    game.rest().unwrap();

    assert_eq!(hp(&game, companion), 40, "the worst-hurt is mended first");
    assert_eq!(
        hp(&game, player),
        120 + 50,
        "and what the patches have left goes to the next"
    );
    assert_eq!(patches(&game), 0);
}

#[test]
fn a_partial_last_patch_rounds_in_the_partys_favour() {
    let mut game = base();
    let player = hurt_player(&mut game, 200, REST_INTEGRITY_PER_PATCH as i32 + 1);
    stock_patches(&mut game, 5);

    game.rest().unwrap();

    assert_eq!(hp(&game, player), 200, "41 missing is healed in full");
    assert_eq!(patches(&game), 3, "by two patches");
}

#[test]
fn a_partial_rest_says_so() {
    let mut game = base();
    hurt_player(&mut game, 200, 100);
    stock_patches(&mut game, 1);

    game.rest().unwrap();

    let lines: Vec<String> = game
        .message_log(usize::MAX)
        .into_iter()
        .map(|l| l.text)
        .collect();
    assert!(
        lines.iter().any(|l| l.contains("only partly repaired")),
        "a short rest names itself: {lines:?}"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("fully recharged and repaired")),
        "and does not claim a full repair"
    );
}

#[test]
fn a_field_rest_still_heals_in_full_and_spends_no_patch() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = hurt_player(&mut game, 200, 150);
    stock_patches(&mut game, 5);
    descend(&mut game);

    game.rest().unwrap();

    assert_eq!(hp(&game, player), 200, "the Outlet buys a full repair");
    assert_eq!(patches(&game), 5, "and no patch");
}

#[test]
fn the_trickle_heals_a_little_each_tick_in_base_for_free() {
    let mut game = base();
    let player = hurt_player(&mut game, 200, 100);
    stock_patches(&mut game, 3);

    game.run_base_trickle();

    assert_eq!(hp(&game, player), 100 + BASE_TRICKLE_PER_TICK);
    assert_eq!(patches(&game), 3, "the trickle costs nothing");
}

#[test]
fn the_trickle_mends_the_party_but_not_staff() {
    let mut game = base();
    hurt_player(&mut game, 200, 10);
    let companion = spawn_tamed(&mut game, 40, 3);
    enlist(&mut game, companion);
    set_hp(&mut game, companion, 10);
    let staff = spawn_tamed(&mut game, 40, 3);
    set_hp(&mut game, staff, 10);

    game.run_base_trickle();

    assert_eq!(hp(&game, companion), 10 + BASE_TRICKLE_PER_TICK);
    assert_eq!(hp(&game, staff), 10, "a rest's repaired set, no wider");
}

#[test]
fn the_trickle_does_nothing_outside_the_base() {
    let mut game = base();
    let player = hurt_player(&mut game, 200, 100);
    game.world.insert_resource(Locale::Surface);

    game.run_base_trickle();

    assert_eq!(hp(&game, player), 100);
}

#[test]
fn the_trickle_does_nothing_during_a_battle() {
    let mut game = base();
    let player = hurt_player(&mut game, 200, 100);
    battle_with_a_pack_of(&mut game, 1, 10);
    assert!(game.has_active_battle());

    game.run_base_trickle();

    assert_eq!(hp(&game, player), 100);
}

#[test]
fn the_trickle_does_not_raise_the_dead() {
    let mut game = base();
    let player = hurt_player(&mut game, 200, 200);

    game.run_base_trickle();

    assert_eq!(hp(&game, player), 0, "a body at zero is for the reaper");
}
