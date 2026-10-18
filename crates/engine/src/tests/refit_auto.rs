//! The Refit Bench mending on its own: idle within reach of a stocked bench.

use super::support::*;
use crate::items::{EquipmentSlot, ids};
use crate::resources::RefitStand;
use crate::tuning::{GEAR_DURABILITY, REFIT_IDLE_TICKS};
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

fn player_refitting(game: &mut Game) -> bool {
    game.view_entities(10, 10)
        .iter()
        .find(|v| v.is_player)
        .unwrap()
        .refitting
}

#[test]
fn the_bench_waits_out_the_idle_ticks_then_mends_one_copy_a_tick() {
    let (mut game, _) = stand_by_bench(6001, 5);
    let player = game.player_entity();
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY),
    );
    for _ in 0..REFIT_IDLE_TICKS {
        game.tick();
        assert_eq!(game.refit_bench_cores(), 5, "still waiting");
    }
    game.tick();
    assert_eq!(wear_of(&game, player, EquipmentSlot::Weapon), 0);
    assert_eq!(game.refit_bench_cores(), 4);
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
    assert_eq!(
        game.refit_bench_cores(),
        5,
        "the step threw the two ticks away"
    );
    game.tick();
    assert_eq!(game.refit_bench_cores(), 4);
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
    assert_eq!(game.refit_bench_cores(), 5);
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
fn a_copy_above_the_alert_line_is_left_alone_but_manual_refit_takes_it() {
    let (mut game, _) = stand_by_bench(6005, 5);
    let player = game.player_entity();
    let scratched = worn_copy(ids::MONOFILAMENT_WHIP, 1);
    wear_on(&mut game, player, &scratched);
    for _ in 0..6 {
        game.tick();
    }
    assert_eq!(game.refit_bench_cores(), 5);
    assert!(game.refit_copy(&scratched).is_ok());
}

#[test]
fn service_goes_player_then_party_then_pack_and_most_worn_first() {
    let (mut game, _) = stand_by_bench(6006, 9);
    let player = game.player_entity();
    let member = spawn_tamed_on_map(&mut game, 0, 0);
    game.world.resource_mut::<Party>().0.push(member);
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::MONOFILAMENT_WHIP, GEAR_DURABILITY - 1),
    );
    wear_on(
        &mut game,
        player,
        &worn_copy(ids::FIREWALL_PLATING, GEAR_DURABILITY),
    );
    wear_on(
        &mut game,
        member,
        &worn_copy(ids::FIREWALL_PLATING, GEAR_DURABILITY),
    );
    let pack = worn_copy("kinetic_edge", GEAR_DURABILITY);
    game.add_copies(&pack, 1);

    for _ in 0..REFIT_IDLE_TICKS {
        game.tick();
    }
    let mut order = Vec::new();
    for _ in 0..4 {
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
            (true, false, false, false),
            (true, true, false, false),
            (true, true, true, false),
            (true, true, true, true),
        ],
        "the player's most worn, the rest of theirs, then the party, then the pack"
    );
    assert_eq!(game.refit_bench_cores(), 5);
}

#[test]
fn a_mended_pack_copy_is_plain_and_lands_in_the_inventory() {
    let (mut game, _) = stand_by_bench(6007, 5);
    let worn = worn_copy("kinetic_edge", GEAR_DURABILITY);
    game.add_copies(&worn, 1);
    for _ in 0..=REFIT_IDLE_TICKS {
        game.tick();
    }
    assert_eq!(game.count_copies(&worn), 0);
    let plain = GearCopy::plain(ItemId::from("kinetic_edge"));
    assert_eq!(game.count_copies(&plain), 1);
    assert_eq!(game.refit_bench_cores(), 4);
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
