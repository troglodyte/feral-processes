//! Permanent companion upgrades: the zone bump and the percentage buffs.

use super::support::*;
use crate::tuning::{MAX_COMPANION_REFACTORS, ZONE_STAT_GROWTH};
use crate::*;

const KERNEL: &str = "recompile_kernel";
const HP_BUFF: &str = "buffer_extension";
const ATK_BUFF: &str = "inline_cache";

/// Puts `qty` of `item` in the player's cargo.
fn stock(game: &mut Game, item: &str, qty: u32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(item), qty);
}

fn held(game: &Game, item: &str) -> u32 {
    let player = game.player_entity();
    game.world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(item))
}

/// Puts the player in `zone` without running a breach, which would move the
/// base and re-roll the map. Only the level matters to these refusals.
fn set_zone(game: &mut Game, zone: u32) {
    game.world.resource_mut::<crate::resources::ZoneLevel>().0 = zone;
}

/// `(hp, max_hp, atk, def)`. A tuple rather than `Stats` itself, which
/// carries no `PartialEq` — these tests want "nothing moved" in one line.
fn stats(game: &Game, pet: Entity) -> (i32, i32, i32, i32) {
    let s = game.world.get::<Stats>(pet).unwrap();
    (s.hp, s.max_hp, s.atk, s.def)
}

fn set_stats(game: &mut Game, pet: Entity, hp: i32, atk: i32, def: i32) {
    let mut s = game.world.get_mut::<Stats>(pet).unwrap();
    s.hp = hp;
    s.max_hp = hp;
    s.atk = atk;
    s.def = def;
}

#[test]
fn a_recompile_kernel_doubles_the_stat_block_and_raises_the_tier() {
    let mut game = Game::new(400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_stats(&mut game, pet, 80, 9, 5);
    game.world.entity_mut(pet).insert(ZonePortal(1));
    set_zone(&mut game, 3);
    stock(&mut game, KERNEL, 1);

    game.refactor_companion(pet, &ItemId::from(KERNEL)).unwrap();

    let (_, max_hp, atk, def) = stats(&game, pet);
    assert_eq!(
        (max_hp, atk, def),
        (
            80 * ZONE_STAT_GROWTH,
            9 * ZONE_STAT_GROWTH,
            5 * ZONE_STAT_GROWTH
        ),
        "a bump multiplies the whole block by one zone's growth"
    );
    assert_eq!(
        game.world.get::<ZonePortal>(pet).unwrap().0,
        2,
        "one tier at a time, not straight to the player's zone"
    );
    assert_eq!(held(&game, KERNEL), 0, "and the kernel is spent");
}

/// Current HP rises by exactly what the maximum rose by. A level-up
/// full-heals; a refactor must not, or a Recompile Kernel becomes the best
/// healing item in the game and gets carried into fights for that.
#[test]
fn a_refactor_raises_current_hp_by_the_delta_rather_than_healing() {
    let mut game = Game::new(401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_stats(&mut game, pet, 100, 10, 5);
    game.world.get_mut::<Stats>(pet).unwrap().hp = 20;
    game.world.entity_mut(pet).insert(ZonePortal(1));
    set_zone(&mut game, 2);
    stock(&mut game, KERNEL, 1);

    game.refactor_companion(pet, &ItemId::from(KERNEL)).unwrap();

    let (hp, max_hp, ..) = stats(&game, pet);
    assert_eq!(max_hp, 200);
    assert_eq!(
        hp, 120,
        "a program on its last legs stays on its last legs — +100 max, +100 current"
    );
}

#[test]
fn a_recompile_kernel_is_refused_once_the_program_has_caught_up() {
    let mut game = Game::new(402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(pet).insert(ZonePortal(3));
    set_zone(&mut game, 3);
    stock(&mut game, KERNEL, 1);
    let before = stats(&game, pet);

    let err = game
        .refactor_companion(pet, &ItemId::from(KERNEL))
        .unwrap_err();

    assert!(
        err.contains("current"),
        "the refusal has to say why, not just no: {err:?}"
    );
    assert_eq!(stats(&game, pet), before, "and nothing moved");
    assert_eq!(held(&game, KERNEL), 1, "a refused refactor spends nothing");
}

/// The property that makes percentages the right shape: a buff bought in
/// zone 1 is worth as much after a breach as one bought afterwards, so a
/// player cannot gain by hoarding buffs until they have bumped.
///
/// "As much" rather than "exactly as much" is the honest claim, and the
/// arithmetic is why. `×1.05` and `×2` commute over the reals, but both
/// steps round to a whole stat and the percentage step floors at `+1`, so
/// the two orders can land a single point apart. The clean block below
/// pins exact equality where rounding cannot bite; the spread pins the
/// bound everywhere else. A drift of one point is not an ordering anyone
/// can exploit — a drift that grew with the stat would be.
#[test]
fn a_percent_buff_commutes_with_a_zone_bump() {
    let one_of_each = |hp: i32, atk: i32, def: i32, buff_first: bool| {
        let mut game = Game::new(403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pet = spawn_tamed(&mut game, 10, 3);
        set_stats(&mut game, pet, hp, atk, def);
        game.world.entity_mut(pet).insert(ZonePortal(1));
        set_zone(&mut game, 2);
        stock(&mut game, KERNEL, 1);
        stock(&mut game, HP_BUFF, 1);

        let order = if buff_first {
            [HP_BUFF, KERNEL]
        } else {
            [KERNEL, HP_BUFF]
        };
        for item in order {
            game.refactor_companion(pet, &ItemId::from(item)).unwrap();
        }
        stats(&game, pet).1
    };

    assert_eq!(
        one_of_each(100, 10, 5, true),
        one_of_each(100, 10, 5, false),
        "on a block where neither step has to round, the orders are identical"
    );

    for hp in [3, 7, 13, 36, 98, 137, 512] {
        let (buff_first, bump_first) =
            (one_of_each(hp, 10, 5, true), one_of_each(hp, 10, 5, false));
        assert!(
            (buff_first - bump_first).abs() <= 1,
            "at {hp} HP the two orders landed on {buff_first} and {bump_first} — \
             rounding may cost a point, never more"
        );
    }
}

/// `+5%` of 3 ATK rounds back to 3, so without the floor a percentage buff
/// would do nothing at all to exactly the weak programs it exists to
/// rescue — and would still charge them a permanent slot for it.
#[test]
fn a_percent_buff_on_a_tiny_stat_still_gains_a_point() {
    let mut game = Game::new(404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_stats(&mut game, pet, 36, 3, 1);
    stock(&mut game, ATK_BUFF, 1);

    game.refactor_companion(pet, &ItemId::from(ATK_BUFF))
        .unwrap();

    assert_eq!(
        stats(&game, pet).2,
        4,
        "3 ATK plus five percent must not round back to 3"
    );
}

#[test]
fn the_upgrade_slots_run_out_but_the_zone_bump_never_does() {
    let mut game = Game::new(405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_stats(&mut game, pet, 100, 10, 5);
    game.world.entity_mut(pet).insert(ZonePortal(1));
    set_zone(&mut game, 8);
    stock(&mut game, HP_BUFF, MAX_COMPANION_REFACTORS + 1);
    stock(&mut game, KERNEL, 1);

    for _ in 0..MAX_COMPANION_REFACTORS {
        game.refactor_companion(pet, &ItemId::from(HP_BUFF))
            .unwrap();
    }
    let err = game
        .refactor_companion(pet, &ItemId::from(HP_BUFF))
        .unwrap_err();

    assert!(
        err.contains("slot"),
        "the refusal has to name what ran out: {err:?}"
    );
    assert_eq!(
        held(&game, HP_BUFF),
        1,
        "the sixth buff is still in cargo, not silently eaten"
    );
    assert!(
        game.refactor_companion(pet, &ItemId::from(KERNEL)).is_ok(),
        "a maxed-out program must still be able to stay level with the zone — \
         the two tracks do not share the pool"
    );
}

#[test]
fn a_refactor_is_refused_mid_battle() {
    let mut game = Game::new(406, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.add_companion(pet).unwrap();
    stock(&mut game, HP_BUFF, 1);
    let enemy = spawn_wild_on_player_tile(&mut game);
    let player = game.player_entity();
    insert_battle(&mut game, player, vec![enemy]);
    let before = stats(&game, pet);

    assert!(
        game.refactor_companion(pet, &ItemId::from(HP_BUFF))
            .is_err()
    );
    assert_eq!(stats(&game, pet), before);
    assert_eq!(held(&game, HP_BUFF), 1);
}

#[test]
fn a_refactor_is_refused_on_a_program_you_do_not_own() {
    let mut game = Game::new(407, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    stock(&mut game, HP_BUFF, 1);

    assert!(
        game.refactor_companion(wild, &ItemId::from(HP_BUFF))
            .is_err()
    );
    assert_eq!(held(&game, HP_BUFF), 1);
}

/// Ordinary cargo is not an upgrade, and the check for that comes before
/// the one that would take it out of the player's bag.
#[test]
fn an_item_that_upgrades_nothing_is_refused_and_kept() {
    let mut game = Game::new(408, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    stock(&mut game, ids::CORE_FRAGMENT, 4);
    let before = stats(&game, pet);
    let purse = held(&game, ids::CORE_FRAGMENT);

    let err = game
        .refactor_companion(pet, &ItemId::from(ids::CORE_FRAGMENT))
        .unwrap_err();

    assert!(err.to_lowercase().contains("refactor"), "{err:?}");
    assert_eq!(stats(&game, pet), before);
    assert_eq!(held(&game, ids::CORE_FRAGMENT), purse);
}

/// The spend-last ordering: every refusal above has to fire before the item
/// leaves the bag, and the one refusal that could plausibly be checked
/// *after* taking it is the one where the player simply hasn't got it.
#[test]
fn a_refactor_you_cannot_pay_for_changes_nothing() {
    let mut game = Game::new(409, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_stats(&mut game, pet, 100, 10, 5);
    let before = stats(&game, pet);

    assert!(
        game.refactor_companion(pet, &ItemId::from(HP_BUFF))
            .is_err()
    );

    assert_eq!(stats(&game, pet), before);
    assert_eq!(
        game.world
            .get::<Refactors>(pet)
            .copied()
            .unwrap_or_default(),
        Refactors(0),
        "a refactor that never happened must not have spent a slot"
    );
}

/// A refactor multiplies `Stats`, and a gear bonus sitting in there would be
/// multiplied with it — the later unequip subtracts only the unscaled amount
/// and welds the difference into the program's base numbers forever. This is
/// `EquippedItem::fusion_tier`'s trap reached by a new route, so the second
/// half of this test (unequip, compare) is the half that catches it; the
/// first half passes against the bug.
#[test]
fn refactoring_a_geared_program_scales_its_own_stats_and_not_the_gear() {
    let mut game = Game::new(402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_stats(&mut game, pet, 80, 9, 5);
    game.world.entity_mut(pet).insert(ZonePortal(1));
    set_zone(&mut game, 3);
    stock(&mut game, KERNEL, 1);

    // A bare twin refactored the same way is the yardstick: whatever the
    // formula does, gear must not change the answer.
    let bare = spawn_tamed(&mut game, 10, 3);
    set_stats(&mut game, bare, 80, 9, 5);
    game.world.entity_mut(bare).insert(ZonePortal(1));
    stock(&mut game, KERNEL, 1);
    game.refactor_companion(bare, &ItemId::from(KERNEL))
        .unwrap();
    let expected = stats(&game, bare);

    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    stock(&mut game, ids::OVERCLOCK_CORE, 1);
    let before_gear = stats(&game, pet);
    game.equip(pet, &weapon, 0).unwrap();

    game.refactor_companion(pet, &ItemId::from(KERNEL)).unwrap();
    game.unequip(pet, EquipmentSlot::Weapon).unwrap();

    assert_eq!(
        stats(&game, pet),
        expected,
        "a geared program must refactor to exactly what a bare one does"
    );
    let (_, max_hp, atk, def) = stats(&game, pet);
    let (_, was_max_hp, was_atk, was_def) = before_gear;
    assert_eq!(
        (max_hp, atk, def),
        (
            was_max_hp * ZONE_STAT_GROWTH,
            was_atk * ZONE_STAT_GROWTH,
            was_def * ZONE_STAT_GROWTH
        ),
        "and the multiplier lands on its own numbers, with no gear welded in"
    );
}
