//! The Refit Bench mending on its own: idle within reach of a stocked bench,
//! a repair point a tick, a core's worth of points at a time.

use super::support::*;
use crate::components::RefitCharge;
use crate::game::refit::{RefitStep, refit_step};
use crate::items::{EquipmentSlot, ids};
use crate::resources::RefitStand;
use crate::tuning::{GEAR_DURABILITY, REFIT_IDLE_TICKS, REFIT_POINTS_PER_CORE};
use crate::*;

fn worn_copy(item: &str, wear: u8) -> GearCopy {
    GearCopy {
        wear,
        ..GearCopy::plain(ItemId::from(item))
    }
}

fn stock_bench(game: &mut Game, bench: Entity, cores: u32) {
    game.world.entity_mut(bench).insert(Stock {
        input: [(ItemId::from(ids::ANNEALED_CORE), cores)]
            .into_iter()
            .collect(),
        ..Default::default()
    });
}

/// In base space at the origin with a bench two tiles away holding `cores`.
fn stand_by_bench(seed: u32, cores: u32) -> (Game, Entity) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bench = spawn_structure_at(&mut game, "refit_bench", 2, 0);
    stock_bench(&mut game, bench, cores);
    stand_in_base_at(&mut game, 0, 0);
    (game, bench)
}

fn wear_on(game: &mut Game, wearer: Entity, copy: &GearCopy) {
    game.add_copies(copy, 1);
    game.equip(wearer, copy).unwrap();
    // Equipping spends a tick of its own, which would count towards the wait.
    *game.world.resource_mut::<RefitStand>() = RefitStand::default();
}

fn wear_of(game: &Game, wearer: Entity, slot: EquipmentSlot) -> u8 {
    game.worn(wearer, slot).unwrap().copy.wear
}

/// The cores left at the bench that would serve.
fn cores(game: &Game) -> u32 {
    game.refit_bench_reserve().map_or(0, |(_, cores)| cores)
}

fn charge(game: &Game, bench: Entity) -> u32 {
    game.world.get::<RefitCharge>(bench).map_or(0, |c| c.0)
}

fn player_refitting(game: &mut Game) -> bool {
    game.view_entities(10, 10)
        .iter()
        .find(|v| v.is_player)
        .unwrap()
        .refitting
}

#[test]
fn the_bench_waits_out_the_idle_ticks_then_mends_a_point_a_tick() {
    let (mut game, bench) = stand_by_bench(6001, 5);
    let player = game.player_entity();
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, 1));
    for _ in 0..REFIT_IDLE_TICKS {
        game.tick();
        assert_eq!(cores(&game), 5, "still waiting");
    }
    game.tick();
    assert_eq!(wear_of(&game, player, EquipmentSlot::Weapon), 0);
    assert_eq!(cores(&game), 4);
    assert_eq!(charge(&game, bench), REFIT_POINTS_PER_CORE - 1);
}

#[test]
fn a_step_restarts_the_wait() {
    let (mut game, _) = stand_by_bench(6002, 5);
    let player = game.player_entity();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
    );
    game.tick();
    game.tick();
    stand_in_base_at(&mut game, 1, 0);
    game.tick();
    game.tick();
    assert_eq!(cores(&game), 5, "the step threw the two ticks away");
    game.tick();
    assert_eq!(cores(&game), 4);
}

#[test]
fn a_bench_three_tiles_away_never_mends() {
    let (mut game, bench) = stand_by_bench(6003, 5);
    game.world.entity_mut(bench).insert(Position { x: 3, y: 0 });
    let player = game.player_entity();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
    );
    for _ in 0..8 {
        game.tick();
    }
    assert_eq!(cores(&game), 5);
}

#[test]
fn an_empty_bench_never_mends_and_says_so_once() {
    let (mut game, _) = stand_by_bench(6004, 0);
    let player = game.player_entity();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
    );
    for _ in 0..8 {
        game.tick();
    }
    assert_eq!(
        wear_of(&game, player, EquipmentSlot::Weapon),
        GEAR_DURABILITY
    );
    let said = game
        .message_log(200)
        .iter()
        .filter(|l| l.text.contains("out of Annealed Cores"))
        .count();
    assert_eq!(said, 1);
}

#[test]
fn service_goes_player_then_party_then_pack_and_most_worn_first() {
    let (mut game, _) = stand_by_bench(6006, 9);
    let player = game.player_entity();
    let member = spawn_tamed_on_map(&mut game, 0, 0);
    game.world.resource_mut::<Party>().0.push(member);
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, 1));
    wear_on(&mut game, player, &worn_copy(ids::FIREWALL_PLATING, 2));
    wear_on(&mut game, member, &worn_copy(ids::FIREWALL_PLATING, 1));
    let pack = worn_copy("kinetic_edge", 1);
    game.add_copies(&pack, 1);

    for _ in 0..REFIT_IDLE_TICKS {
        game.tick();
    }
    let mut order = Vec::new();
    for _ in 0..5 {
        game.tick();
        order.push((
            wear_of(&game, player, EquipmentSlot::Armor) == 0,
            wear_of(&game, player, EquipmentSlot::Weapon) == 0,
            wear_of(&game, member, EquipmentSlot::Armor) == 0,
            game.count_copies(&pack) == 0,
        ));
    }
    assert_eq!(
        order,
        vec![
            (false, false, false, false),
            (false, true, false, false),
            (true, true, false, false),
            (true, true, true, false),
            (true, true, true, true),
        ],
        "the player's most worn down to a tie, ties in slot order, then the party, then the pack"
    );
    assert_eq!(cores(&game), 8, "five points, one core");
}

#[test]
fn a_mended_pack_copy_is_plain_and_lands_in_the_inventory() {
    let (mut game, _) = stand_by_bench(6007, 5);
    let worn = worn_copy("kinetic_edge", 1);
    game.add_copies(&worn, 1);
    for _ in 0..=REFIT_IDLE_TICKS {
        game.tick();
    }
    assert_eq!(game.count_copies(&worn), 0);
    let plain = GearCopy::plain(ItemId::from("kinetic_edge"));
    assert_eq!(game.count_copies(&plain), 1);
    assert_eq!(cores(&game), 4);
}

#[test]
fn the_player_view_reads_refitting_while_the_bench_has_work_and_stock() {
    let (mut game, _) = stand_by_bench(6008, 5);
    let player = game.player_entity();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
    );
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::FIREWALL_PLATING, GEAR_DURABILITY),
    );
    for _ in 0..REFIT_IDLE_TICKS {
        game.tick();
    }
    assert!(!player_refitting(&mut game), "still waiting");
    game.tick();
    assert!(player_refitting(&mut game), "one copy mended, one waiting");
    stand_in_base_at(&mut game, 1, 0);
    game.tick();
    assert!(!player_refitting(&mut game), "a step ended the stand");
}

#[test]
fn an_empty_bench_never_shows_the_mark() {
    let (mut game, _) = stand_by_bench(6009, 0);
    let player = game.player_entity();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
    );
    for _ in 0..=REFIT_IDLE_TICKS {
        game.tick();
    }
    assert!(!player_refitting(&mut game));
}

fn said_out_of_cores(game: &Game) -> usize {
    game.message_log(200)
        .iter()
        .filter(|l| l.text.contains("out of Annealed Cores"))
        .count()
}

#[test]
fn an_empty_bench_out_of_reach_says_nothing() {
    let (mut game, bench) = stand_by_bench(6010, 0);
    game.world.entity_mut(bench).insert(Position { x: 5, y: 0 });
    let player = game.player_entity();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
    );
    for _ in 0..8 {
        game.tick();
    }
    assert_eq!(said_out_of_cores(&game), 0);
}

#[test]
fn beside_an_empty_bench_a_stocked_one_out_of_reach_does_not_serve() {
    let (mut game, _) = stand_by_bench(6011, 0);
    let far = spawn_structure_at(&mut game, "refit_bench", 6, 0);
    stock_bench(&mut game, far, 5);
    let player = game.player_entity();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
    );
    for _ in 0..8 {
        game.tick();
    }
    assert_eq!(
        wear_of(&game, player, EquipmentSlot::Weapon),
        GEAR_DURABILITY
    );
    assert_eq!(said_out_of_cores(&game), 1);
}

#[test]
fn the_mark_shows_on_the_tick_that_mends_the_last_worn_copy() {
    let (mut game, _) = stand_by_bench(6012, 5);
    let player = game.player_entity();
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, 1));
    for _ in 0..REFIT_IDLE_TICKS {
        game.tick();
    }
    game.tick();
    assert_eq!(wear_of(&game, player, EquipmentSlot::Weapon), 0);
    assert!(player_refitting(&mut game), "the tick that mended it");
    game.tick();
    assert!(!player_refitting(&mut game), "nothing left to mend");
}

#[test]
fn pack_copies_are_mended_most_worn_first() {
    let (mut game, _) = stand_by_bench(6013, 5);
    let worse = worn_copy("kinetic_edge", GEAR_DURABILITY);
    let better = worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY - 1);
    game.add_copies(&better, 1);
    game.add_copies(&worse, 1);
    for _ in 0..=REFIT_IDLE_TICKS {
        game.tick();
    }
    assert_eq!(game.count_copies(&worse), 0, "the most worn went first");
    assert_eq!(game.count_copies(&better), 1);
}

#[test]
fn reach_is_chebyshev_so_a_diagonal_two_away_mends_and_three_does_not() {
    for (at, mends) in [(2, true), (3, false)] {
        let (mut game, bench) = stand_by_bench(6014, 5);
        game.world
            .entity_mut(bench)
            .insert(Position { x: at, y: at });
        let player = game.player_entity();
        wear_on(
            &mut game,
            player,
            &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
        );
        for _ in 0..=REFIT_IDLE_TICKS {
            game.tick();
        }
        assert_eq!(cores(&game) == 4, mends, "bench at ({at}, {at})");
    }
}

#[test]
fn refit_step_spends_a_core_only_on_an_empty_charge_and_restores_a_point() {
    let step = |charge, cores, wear| refit_step(charge, cores, wear);
    assert_eq!(
        step(0, 3, 5),
        RefitStep {
            spend_core: true,
            restored: 1,
            charge_after: REFIT_POINTS_PER_CORE - 1
        }
    );
    assert_eq!(
        step(7, 3, 1),
        RefitStep {
            spend_core: false,
            restored: 1,
            charge_after: 6
        }
    );
    assert_eq!(
        step(0, 0, 5),
        RefitStep {
            spend_core: false,
            restored: 0,
            charge_after: 0
        }
    );
}

fn said_mended(game: &Game) -> usize {
    game.message_log(200)
        .iter()
        .filter(|l| l.text.contains("Refit Bench mended"))
        .count()
}

/// The reported state: 4 of 15 left sat above the old alert line, and the
/// bench held cores and never touched it.
#[test]
fn a_copy_above_the_old_alert_line_is_mended() {
    let (mut game, _) = stand_by_bench(6015, 5);
    let player = game.player_entity();
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, 11));
    for _ in 0..REFIT_IDLE_TICKS + 11 {
        game.tick();
    }
    assert_eq!(wear_of(&game, player, EquipmentSlot::Weapon), 0);
}

#[test]
fn mending_takes_a_tick_a_point_and_says_so_once_on_the_last() {
    let (mut game, _) = stand_by_bench(6016, 5);
    let player = game.player_entity();
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, 4));
    for _ in 0..REFIT_IDLE_TICKS + 3 {
        game.tick();
    }
    assert_eq!(wear_of(&game, player, EquipmentSlot::Weapon), 1);
    assert_eq!(said_mended(&game), 0, "not finished yet");
    game.tick();
    assert_eq!(wear_of(&game, player, EquipmentSlot::Weapon), 0);
    assert_eq!(said_mended(&game), 1);
}

#[test]
fn one_core_covers_fifty_points_across_copies() {
    let (mut game, bench) = stand_by_bench(6017, 9);
    let player = game.player_entity();
    let worn = GEAR_DURABILITY;
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, worn));
    wear_on(&mut game, player, &worn_copy(ids::FIREWALL_PLATING, worn));
    game.add_copies(&worn_copy("kinetic_edge", worn), 1);
    game.add_copies(&worn_copy("redundant_bank", worn), 1);
    let total = 4 * u32::from(worn);
    for _ in 0..REFIT_IDLE_TICKS + REFIT_POINTS_PER_CORE {
        game.tick();
    }
    assert_eq!(cores(&game), 8, "fifty points, one core");
    assert_eq!(charge(&game, bench), 0);
    for _ in REFIT_POINTS_PER_CORE..total {
        game.tick();
    }
    assert_eq!(cores(&game), 9 - total.div_ceil(REFIT_POINTS_PER_CORE));
    assert_eq!(charge(&game, bench), 2 * REFIT_POINTS_PER_CORE - total);
    assert_eq!(wear_of(&game, player, EquipmentSlot::Weapon), 0);
    assert_eq!(wear_of(&game, player, EquipmentSlot::Armor), 0);
}

#[test]
fn a_charged_bench_with_no_cores_still_serves() {
    let (mut game, bench) = stand_by_bench(6018, 0);
    game.world.entity_mut(bench).insert(RefitCharge(3));
    let player = game.player_entity();
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, 2));
    for _ in 0..REFIT_IDLE_TICKS + 2 {
        game.tick();
    }
    assert_eq!(wear_of(&game, player, EquipmentSlot::Weapon), 0);
    assert_eq!(charge(&game, bench), 1);
    assert_eq!(said_out_of_cores(&game), 0);
}

#[test]
fn the_first_serviceable_bench_in_position_order_serves() {
    let (mut game, near) = stand_by_bench(6019, 0);
    let first = spawn_structure_at(&mut game, "refit_bench", 1, 0);
    stock_bench(&mut game, first, 5);
    let later = spawn_structure_at(&mut game, "refit_bench", 2, 1);
    stock_bench(&mut game, later, 5);
    let player = game.player_entity();
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, 1));
    for _ in 0..=REFIT_IDLE_TICKS {
        game.tick();
    }
    assert_eq!(charge(&game, first), REFIT_POINTS_PER_CORE - 1);
    assert_eq!(charge(&game, later), 0);
    assert_eq!(charge(&game, near), 0);
}

#[test]
fn a_mended_broken_copy_gives_its_bonus_back_exactly() {
    let (mut game, _) = stand_by_bench(6020, 5);
    let player = game.player_entity();
    let bare = *game.world.get::<Stats>(player).unwrap();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::FIREWALL_PLATING, GEAR_DURABILITY),
    );
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().mitigation,
        bare.mitigation,
        "broken gear is inert"
    );
    for _ in 0..REFIT_IDLE_TICKS + u32::from(GEAR_DURABILITY) {
        game.tick();
    }
    assert_eq!(wear_of(&game, player, EquipmentSlot::Armor), 0);
    assert!(game.world.get::<Stats>(player).unwrap().mitigation > bare.mitigation);
    game.unequip(player, EquipmentSlot::Armor).unwrap();
    assert_eq!(*game.world.get::<Stats>(player).unwrap(), bare);
}

/// The bench's core is a sink like any other: `spend_supply` writes it to the
/// ledger under the refit's own source.
#[test]
fn a_core_poured_into_the_charge_is_logged_to_the_ledger() {
    let (mut game, _) = stand_by_bench(6021, 5);
    let player = game.player_entity();
    wear_on(&mut game, player, &worn_copy(ids::MONOFILAMENT_WHIP, 1));
    let ledger = |game: &Game| {
        game.world
            .resource::<crate::base_ledger::BaseLedger>()
            .lifetime
            .get(&ItemId::from(ids::ANNEALED_CORE))
            .map_or(0, |t| t.consumed)
    };
    let before = ledger(&game);
    for _ in 0..=REFIT_IDLE_TICKS {
        game.tick();
    }
    assert_eq!(ledger(&game) - before, 1);
}
