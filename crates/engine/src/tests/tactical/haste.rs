//! Haste on a battle map: who it lands on, the second swing's guards, and
//! what the auto-driven party makes of it.

use super::*;
use crate::abilities::{AbilityDb, AbilityRange, AbilityShape};
use crate::components::{AbilityCooldowns, Derived, Durability, Hasted, Structure};
use crate::routine_craft::{Core, CraftedRoutine, Subroutine, Version, compile};
use crate::statuses::StatusId;

fn hasted_version(game: &Game, body: Entity) -> Option<u8> {
    game.world.get::<Hasted>(body).map(|h| h.version)
}

/// A Haste routine of `version` reshaped to `shape` and installed as `body`'s
/// only one. Returns its id.
fn haste_sweep(game: &mut Game, body: Entity, version: Version, shape: AbilityShape) -> String {
    let mut def = compile(&CraftedRoutine {
        id: "crafted/haste".to_string(),
        name: "Quicken".to_string(),
        parts: vec![Subroutine::Core(Core::Haste, version)],
    });
    def.shape = Some(shape);
    def.range = Some(AbilityRange { min: 0, max: 6 });
    let id = def.id.clone();
    game.world.resource_mut::<AbilityDb>().insert(def);
    only_routine(game, body, &id);
    id
}

#[test]
fn a_haste_sweep_never_quickens_a_hostile_inside_its_shape() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    haste_sweep(
        &mut game,
        player,
        Version::V2,
        AbilityShape::Radius { radius: 3 },
    );
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));

    assert!(game.tactical_use_routine(0, (0, 0)));

    assert_eq!(
        hasted_version(&game, player),
        Some(2),
        "the caster is covered"
    );
    assert_eq!(
        hasted_version(&game, pack[0]),
        None,
        "a hostile was hastened"
    );
}

/// A hasted v3 player's one attack on an adjacent hostile: the game, how many
/// swing lines it logged, and whether the first swing hard-faulted into a Crash.
fn hasted_swing(stream: u64, prepare: impl FnOnce(&mut Game, Entity)) -> (Game, usize, bool) {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 9_000);
    let player = game.player_entity();
    let wild = pack[0];
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, wild]);
    let (px, py) = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();
    place_one(&mut game, wild, (px + 1, py));
    game.world.get_mut::<Stats>(wild).unwrap().mitigation = 0;
    game.arm_haste(player, 3);
    crate::tests::support::reseed_rng(&mut game, stream);
    assert!(wait_for_turn(&mut game, player));
    prepare(&mut game, player);
    assert!(game.tactical_attack(wild));
    let log = game.message_log(50);
    let swings: Vec<_> = log
        .iter()
        .filter(|l| l.kind == crate::resources::MessageKind::PartyDamage && l.outcome.is_some())
        .collect();
    let crashed = swings
        .first()
        .is_some_and(|l| l.text.contains("hard-faults"));
    (game, swings.len(), crashed)
}

#[test]
fn an_older_stun_does_not_cost_a_hasted_body_its_second_swing() {
    let mut twice = 0;
    for stream in 0..48 {
        let (game, lines, crashed) = hasted_swing(stream, |game, player| {
            game.arm_status(player, &StatusId::from("stun"), 3, 0);
            assert!(game.is_stunned(player), "the fixture must start stunned");
        });
        if !crashed && game.creature_alive(game.player_entity()) {
            assert_eq!(lines, 2, "stream {stream}");
            twice += 1;
        }
    }
    assert!(twice > 0);
}

#[test]
fn a_crash_fumbled_by_the_first_swing_stops_the_second() {
    let mut crashes = 0;
    for stream in 0..600 {
        let (game, lines, crashed) = hasted_swing(stream, |game, player| {
            // Every miss a fumble, so the deepest rung turns up in a
            // searchable number of seeds.
            game.world.get_mut::<Derived>(player).unwrap().fumble = 1.0;
        });
        if crashed && game.creature_alive(game.player_entity()) {
            assert_eq!(lines, 1, "stream {stream}");
            crashes += 1;
        }
    }
    assert!(crashes > 0, "no stream crashed on the first swing");
}

/// A structure adjacent to the player, on `hp` Durability.
fn structure_beside_player(game: &mut Game, hp: u32) -> Entity {
    tactical_fight(game, 1, 9_000);
    let player = game.player_entity();
    assert!(wait_for_turn(game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();
    let cell = free_neighbour(game, at);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "test_structure".to_string(),
            },
            Durability { hp, max_hp: hp },
        ))
        .id();
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .place(structure, cell)
    );
    structure
}

fn structure_damage_taken(version: Option<u8>) -> u32 {
    let mut game = game();
    let structure = structure_beside_player(&mut game, 10_000);
    let player = game.player_entity();
    if let Some(v) = version {
        game.arm_haste(player, v);
    }
    assert!(game.tactical_attack(structure));
    assert_ne!(
        game.tactical_actor(),
        Some(player),
        "one action ends the turn"
    );
    10_000 - game.world.get::<Durability>(structure).unwrap().hp
}

#[test]
fn a_hasted_v3_swing_at_a_structure_lands_twice_in_one_action() {
    let plain = structure_damage_taken(None);
    assert!(plain > 0);
    assert_eq!(structure_damage_taken(Some(2)), plain, "v2 swings once");
    assert_eq!(structure_damage_taken(Some(3)), 2 * plain);
}

/// An auto-driven player beside a companion, hasted to v2 as asked, with a
/// Haste sweep covering the whole board. Returns whether the beat cast it.
fn auto_beat_casts(
    player_hasted: bool,
    companion_hasted: bool,
    radius: u32,
    hostile_at: (i32, i32),
) -> bool {
    let mut game = game();
    let companion = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, companion);
    let pack = tactical_fight(&mut game, 1, 10_000);
    let player = game.player_entity();
    open_ground(&mut game, &[pack[0], companion], &[hostile_at, (4, 3)]);
    let id = haste_sweep(
        &mut game,
        player,
        Version::V2,
        AbilityShape::Radius { radius },
    );
    if player_hasted {
        game.arm_haste(player, 2);
    }
    if companion_hasted {
        game.arm_haste(companion, 2);
    }
    assert!(wait_for_turn(&mut game, player));
    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        battle.set_actions_left(2);
        battle.commit_walk(Vec::new());
    }
    game.tactical_auto_beat();
    game.world
        .get::<AbilityCooldowns>(player)
        .is_some_and(|c| c.0.contains_key(id.as_str()))
}

#[test]
fn the_auto_driven_party_does_not_recast_haste_on_a_party_that_has_it() {
    assert!(
        auto_beat_casts(false, false, 2, (7, 7)),
        "control: an unhasted party is worth a cast"
    );
    assert!(
        auto_beat_casts(true, false, 2, (7, 7)),
        "control: a hasted caster beside an unhasted companion is worth a cast"
    );
    assert!(
        !auto_beat_casts(true, true, 2, (7, 7)),
        "everyone covered is already hasted"
    );
}

#[test]
fn a_haste_sweep_covering_a_hostile_still_casts_for_the_ally_it_reaches() {
    // The hostile beside the companion cannot be hastened, so it must not
    // count against the cast: only the unhasted companion is worth it.
    assert!(auto_beat_casts(true, false, 6, (4, 2)));
}

#[test]
fn a_version_outside_the_table_reads_the_nearest_row() {
    let mut game = game();
    let entity = body(&mut game, "scrapper");
    let base = game.movement_allowance(entity);
    for (version, bonus) in [(0, 4), (1, 4), (3, 8), (9, 8)] {
        game.arm_haste(entity, 1);
        game.world.get_mut::<Hasted>(entity).unwrap().version = version;
        assert_eq!(game.movement_allowance(entity), base + bonus, "v{version}");
        assert_eq!(crate::tuning::haste_move_bonus(version), bonus);
    }
}

/// Whether a v3 swing at `hp` killed, how many swing lines it logged, and
/// the XP it paid.
fn hasted_kill(hp: i32, max_hp: i32, stream: u64) -> (bool, usize, u32) {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, max_hp);
    let player = game.player_entity();
    let wild = pack[0];
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, wild]);
    let (px, py) = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();
    place_one(&mut game, wild, (px + 1, py));
    {
        let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
        stats.mitigation = 0;
        stats.hp = hp;
    }
    game.arm_haste(player, 3);
    crate::tests::support::reseed_rng(&mut game, stream);
    assert!(wait_for_turn(&mut game, player));
    let before = game.world.get::<Experience>(player).unwrap().xp;
    assert!(game.tactical_attack(wild));
    let swings: Vec<_> = game
        .message_log(50)
        .into_iter()
        .filter(|l| l.kind == crate::resources::MessageKind::PartyDamage)
        .filter_map(|l| l.outcome)
        .collect();
    // A fumble's condition on the player changes the threat a kill is priced
    // by, so a run that fumbled says nothing about double payment.
    let clean = !swings.contains(&crate::resources::SwingOutcome::Fumble);
    let dead = clean && game.world.get::<Stats>(wild).is_none_or(|s| s.hp <= 0);
    let paid = game.world.get::<Experience>(player).unwrap().xp - before;
    (dead, swings.len(), paid)
}

#[test]
fn a_kill_on_the_second_swing_pays_xp_exactly_once() {
    // XP follows the victim's max Integrity, so each size is measured
    // against a first-swing kill of the same size.
    let mut second = 0;
    for max_hp in (2..40).step_by(6) {
        let first = (0..48)
            .find_map(|stream| match hasted_kill(1, max_hp, stream) {
                (true, 1, paid) => Some(paid),
                _ => None,
            })
            .expect("no stream killed on the first swing");
        assert!(first > 0, "a kill pays something");
        for stream in 0..48 {
            if let (true, 2, paid) = hasted_kill(max_hp, max_hp, stream) {
                assert_eq!(paid, first, "max_hp {max_hp} stream {stream}");
                second += 1;
            }
        }
    }
    assert!(second > 0, "no stream killed on the second swing");
}

#[test]
fn a_fight_that_ends_clears_haste_on_every_body() {
    let mut game = game();
    let companion = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, companion);
    let pack = tactical_fight(&mut game, 1, 1);
    let player = game.player_entity();
    let wild = pack[0];
    for body in [player, companion, wild] {
        game.arm_haste(body, 3);
    }
    game.world.get_mut::<Stats>(wild).unwrap().mitigation = 0;
    for _ in 0..64 {
        if game.world.get_resource::<TacticalBattle>().is_none() {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let Some(at) = game.world.resource::<TacticalBattle>().cell_of(wild) else {
            break;
        };
        let beside = free_neighbour(&game, at);
        place_one(&mut game, player, beside);
        game.tactical_attack(wild);
    }
    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the fight never ended"
    );
    for body in [player, companion] {
        assert_eq!(hasted_version(&game, body), None);
    }
}
