//! `Game::seat_derived`: a tamed program's stats become the derivation of its
//! attributes without moving a single figure.

use super::support::*;
use crate::attributes::AttributeId;
use crate::components::{Attributes, BoughtStats, Derived, HoldPoints, ProgramBase, StatPoints};
use crate::*;

fn game() -> Game {
    Game::new(9101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn species(game: &Game) -> String {
    game.species_defs()[0].id.to_string()
}

fn set_attribute(game: &mut Game, entity: Entity, id: &str, value: i32) {
    game.world
        .get_mut::<Attributes>(entity)
        .unwrap()
        .set(&AttributeId::from(id), value);
}

fn stats_of(game: &Game, entity: Entity) -> Stats {
    *game.world.get::<Stats>(entity).unwrap()
}

/// A tamed program that no door has seated: what `roster_parts` leaves behind.
fn unseated_program(game: &mut Game) -> Entity {
    let s = species(game);
    let program = game
        .spawn_wild_creature_scaled(&s, 60, 60, 1.0, false)
        .unwrap();
    game.world
        .entity_mut(program)
        .remove::<(Hostile, WanderAi)>();
    let parts = game.roster_parts();
    game.world.entity_mut(program).insert(parts);
    program
}

/// Seated, and already at a fixpoint: recomputing from the stored base gives
/// the figures it was seated from.
fn assert_seated_and_stable(game: &mut Game, program: Entity) {
    assert!(game.world.get::<ProgramBase>(program).is_some());
    assert!(game.world.get::<Derived>(program).is_some());
    assert_eq!(game.world.get::<StatPoints>(program), Some(&StatPoints(0)));
    assert_eq!(
        game.world.get::<HoldPoints>(program),
        Some(&HoldPoints(false))
    );
    let before = stats_of(game, program);
    game.recompute_derived(program);
    assert_eq!(stats_of(game, program), before);
}

#[test]
fn seating_leaves_stats_alone_with_gear_worn_and_a_receipt() {
    let mut game = game();
    let program = unseated_program(&mut game);
    set_attribute(&mut game, program, "parity", 63);
    set_attribute(&mut game, program, "analysis", 13);
    set_attribute(&mut game, program, "footprint", 52);
    game.world.entity_mut(program).insert(BoughtStats {
        atk: 3,
        mitigation: 2,
        max_hp: 7,
        ..Default::default()
    });
    {
        let mut stats = game.world.get_mut::<Stats>(program).unwrap();
        stats.atk += 3;
        stats.mitigation += 2;
        stats.max_hp += 7;
    }
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(program, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
        .unwrap();
    assert!(
        game.gear_bonus(program).atk > 0,
        "the fixture must wear something"
    );
    let before = stats_of(&game, program);

    game.seat_derived(program);

    assert_eq!(stats_of(&game, program), before);
    assert_seated_and_stable(&mut game, program);
}

#[test]
fn a_program_whose_attributes_would_clamp_a_stat_still_round_trips() {
    let mut game = game();
    let program = unseated_program(&mut game);
    // Footprint 20 is -25 Mitigation against its base, so a program with
    // 3 Mitigation seats a base above the player's and not a negative one.
    set_attribute(&mut game, program, "footprint", 20);
    game.world.get_mut::<Stats>(program).unwrap().mitigation = 0;
    game.world.get_mut::<Stats>(program).unwrap().max_hp = 1;
    game.world.get_mut::<Stats>(program).unwrap().hp = 1;
    let before = stats_of(&game, program);

    game.seat_derived(program);

    assert_eq!(stats_of(&game, program), before);
    assert_seated_and_stable(&mut game, program);
    assert!(game.world.get::<ProgramBase>(program).unwrap().0.mitigation > 0);
}

#[test]
fn seating_twice_changes_nothing() {
    let mut game = game();
    let program = unseated_program(&mut game);
    game.seat_derived(program);
    let base = *game.world.get::<ProgramBase>(program).unwrap();
    set_attribute(&mut game, program, "parity", 90);
    game.seat_derived(program);
    assert_eq!(*game.world.get::<ProgramBase>(program).unwrap(), base);
}

#[test]
fn a_wild_creature_is_never_seated() {
    let mut game = game();
    let s = species(&game);
    let wild = game
        .spawn_wild_creature_scaled(&s, 61, 61, 1.0, false)
        .unwrap();
    game.seat_derived(wild);
    assert!(game.world.get::<ProgramBase>(wild).is_none());
    assert!(game.world.get::<Derived>(wild).is_none());
}

#[test]
fn adopt_program_seats() {
    let mut game = game();
    let s = species(&game);
    let program = game.adopt_program(&s, 62, 62, 1.0).unwrap();
    assert_seated_and_stable(&mut game, program);
}

#[test]
fn a_starting_program_is_seated() {
    let mut game = game();
    let s = species(&game);
    game.grant_starting_program(&s).unwrap();
    let mut q = game
        .world
        .query_filtered::<Entity, (With<crate::components::Tamed>, With<ProgramBase>)>();
    assert_eq!(q.iter(&game.world).count(), 1);
}

#[test]
fn a_decompiled_body_is_seated_with_its_stats_intact() {
    let mut game = game();
    let player = game.player_entity();
    let s = species(&game);
    let wild = game
        .spawn_wild_creature_scaled(&s, 3, 3, 1.0, false)
        .unwrap();
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    let before = stats_of(&game, wild);
    insert_battle(&mut game, player, vec![wild]);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;
    for _ in 0..50 {
        if game.world.get::<crate::components::Tamed>(wild).is_some() {
            break;
        }
        player_decompiles(&mut game);
    }
    assert!(game.world.get::<crate::components::Tamed>(wild).is_some());
    assert_eq!(stats_of(&game, wild), before);
    assert_seated_and_stable(&mut game, wild);
}

#[test]
fn a_seated_high_entropy_program_crits_more() {
    let mut game = game();
    let program = unseated_program(&mut game);
    set_attribute(&mut game, program, "entropy", 90);
    game.seat_derived(program);
    let crit = game.world.get::<Derived>(program).unwrap().crit;
    assert!(crit > crate::tuning::CRIT_CHANCE, "{crit}");
}

#[test]
fn a_seated_high_persistence_program_shrugs_a_status_off_sooner() {
    let mut game = game();
    let program = unseated_program(&mut game);
    set_attribute(&mut game, program, "persistence", 80);
    game.seat_derived(program);
    game.world
        .entity_mut(program)
        .insert(crate::components::StatusEffects::default());
    game.arm_status(program, crate::components::StatusKind::Stun, 10, 0);
    let remaining = game
        .world
        .get::<crate::components::StatusEffects>(program)
        .unwrap()
        .active
        .unwrap()
        .remaining;
    assert_eq!(remaining, 7);
}

#[test]
fn an_arena_companion_levelled_after_adoption_is_stable_under_recompute() {
    let mut game = game();
    let s = species(&game);
    let program = crate::arena::spawn_companion(&mut game, &s, 6).unwrap();
    assert!(stats_of(&game, program).max_hp > 0);
    assert_seated_and_stable(&mut game, program);
}
