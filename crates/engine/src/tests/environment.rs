//! The ground catalogue, and the one reader that resolves a tile to an
//! effect.
//!
//! Nothing here draws from `resources::GameRng`. Ambient ground is a
//! property of the *place*, resolved from the biome every time it is asked
//! rather than rolled or stored. The catalogue itself is Rust now, so these
//! tests build `EnvironmentEffect`s and `GroundCondition`s by hand rather
//! than writing scratch `.ron` files — there is no loader left to exercise.

use crate::environment::{EnvironmentEffect, GroundCondition};
use crate::tuning::{
    MAX_ENVIRONMENT_ATTRITION, MAX_ENVIRONMENT_DRAG_TICKS, MAX_STATIC_AMBUSH_MULT,
};
use crate::world::Biome;

// -------------------------------------------------------------- the catalogue

#[test]
fn for_biome_claims_the_three_ground_conditions() {
    assert_eq!(
        GroundCondition::for_biome(Biome::NullSector),
        Some(GroundCondition::DanglingReads)
    );
    assert_eq!(
        GroundCondition::for_biome(Biome::Mainframe),
        Some(GroundCondition::ThermalLoad)
    );
    assert_eq!(
        GroundCondition::for_biome(Biome::Deadlock),
        Some(GroundCondition::LockContention)
    );
}

/// Unclaimed is the common case — most of the map has to read as scenery,
/// not as a tax on walking. `Platform` is here too: the base's own floor
/// must never be claimed, whatever a future condition's biome list says.
#[test]
fn for_biome_leaves_the_rest_of_the_map_neutral() {
    for biome in [
        Biome::OpenGrid,
        Biome::DataVoid,
        Biome::BlackIce,
        Biome::Platform,
    ] {
        assert!(
            GroundCondition::for_biome(biome).is_none(),
            "{biome:?} should be neutral ground"
        );
    }
}

/// Walks the array rather than naming three conditions by hand, so the
/// array length is what fails to compile when a fourth is added without
/// words to go with it.
#[test]
fn every_condition_has_a_name_and_a_description() {
    for condition in GroundCondition::all() {
        let def = condition.def();
        assert!(!def.name.is_empty(), "{condition:?} has no name");
        assert!(
            !def.description.is_empty(),
            "{condition:?} has no description"
        );
    }
}

/// Replaces `EnvironmentDef::fault`'s three load-time refusals: nothing here
/// is authored by a stranger any more, so the ceilings are a compile-time
/// census over the shipped catalogue instead of a startup check.
#[test]
fn every_condition_stays_inside_its_ceiling() {
    for condition in GroundCondition::all() {
        let effect = condition.def().effect;
        assert!(
            (0.0..=MAX_ENVIRONMENT_ATTRITION).contains(&effect.attrition_percent),
            "{condition:?} authors attrition_percent {}",
            effect.attrition_percent
        );
        assert!(
            effect.min_damage >= 0,
            "{condition:?} authors min_damage {}",
            effect.min_damage
        );
        assert!(
            effect.extra_ticks <= MAX_ENVIRONMENT_DRAG_TICKS,
            "{condition:?} authors extra_ticks {}",
            effect.extra_ticks
        );
        assert!(
            effect.ambush_mult <= MAX_STATIC_AMBUSH_MULT,
            "{condition:?} authors ambush_mult {}",
            effect.ambush_mult
        );
    }
}

/// The guard on the one-of → all-of shape change: a reader that only looks
/// at the attrition terms and ignores the rest would compile clean and
/// silently drop both drag and the ambush multiplier, so all four terms are
/// asserted together against two hand-built effects.
#[test]
fn fold_adds_attrition_and_drag_and_multiplies_ambush() {
    let a = EnvironmentEffect {
        attrition_percent: 0.25,
        min_damage: 1,
        extra_ticks: 1,
        ambush_mult: 1.5,
    };
    let b = EnvironmentEffect {
        attrition_percent: 0.5,
        min_damage: 2,
        extra_ticks: 2,
        ambush_mult: 2.0,
    };

    let folded = a.fold(b);

    assert_eq!(folded.attrition_percent, 0.75, "attrition adds");
    assert_eq!(folded.min_damage, 3, "the floor adds");
    assert_eq!(folded.extra_ticks, 3, "drag adds");
    assert_eq!(folded.ambush_mult, 3.0, "the ambush term multiplies");
}

/// Built by hand rather than off the shipped conditions, so this does not
/// depend on their magnitudes staying where they are.
#[test]
fn clamped_cuts_each_term_to_its_ceiling() {
    let excessive = EnvironmentEffect {
        attrition_percent: MAX_ENVIRONMENT_ATTRITION + 1.0,
        min_damage: 5,
        extra_ticks: MAX_ENVIRONMENT_DRAG_TICKS + 5,
        ambush_mult: MAX_STATIC_AMBUSH_MULT + 1.0,
    };

    let clamped = excessive.clamped();

    assert_eq!(clamped.attrition_percent, MAX_ENVIRONMENT_ATTRITION);
    assert_eq!(clamped.min_damage, 5, "min_damage has no ceiling to cut");
    assert_eq!(clamped.extra_ticks, MAX_ENVIRONMENT_DRAG_TICKS);
    assert_eq!(clamped.ambush_mult, MAX_STATIC_AMBUSH_MULT);
}

#[test]
fn bite_is_the_floored_percentage_and_zero_for_none() {
    let effect = EnvironmentEffect {
        attrition_percent: 0.1,
        min_damage: 3,
        extra_ticks: 0,
        ambush_mult: 1.0,
    };
    assert_eq!(
        effect.bite(100),
        10,
        "the percentage wins once it clears the floor"
    );
    assert_eq!(
        effect.bite(20),
        3,
        "the floor wins when the percentage would round under it"
    );

    assert_eq!(
        EnvironmentEffect::NONE.bite(1000),
        0,
        "a no-attrition effect must not deal the floor"
    );
}

// ---------------------------------------------------------------- the reader

use crate::components::{ActiveFieldBuff, BuffSource, FieldBuffKind, Position, Stats};
use crate::resources::{MessageLog, Party, ZoneLevel};
use crate::tests::support::{enlist, spawn_tamed, test_assets_dir};
use crate::world::{Tile, WorldMap};
use crate::{DifficultyMode, Game};

/// Stands the player on `from` with `to` one step east, both written
/// through the override overlay, and clears anything squatting on the
/// destination — walking into a program, a nest or a structure is a fight
/// or a door, not travel.
fn step_from_onto(game: &mut Game, from: Biome, to: Biome, to_walkable: bool) {
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let (nx, ny) = (pos.x + 1, pos.y);
    let squatters: Vec<crate::Entity> = {
        let mut q = game.world.query::<(crate::Entity, &Position)>();
        q.iter(&game.world)
            .filter(|(e, p)| *e != player && p.x == nx && p.y == ny)
            .map(|(e, _)| e)
            .collect()
    };
    for e in squatters {
        game.world.despawn(e);
    }
    let mut map = game.world.resource_mut::<WorldMap>();
    map.set_override(
        pos.x,
        pos.y,
        Tile {
            biome: from,
            walkable: true,
            rock_shade: None,
        },
    );
    map.set_override(
        nx,
        ny,
        Tile {
            biome: to,
            walkable: to_walkable,
            rock_shade: None,
        },
    );
}

fn player_hp(game: &Game) -> i32 {
    game.world.get::<Stats>(game.player_entity()).unwrap().hp
}

fn clock(game: &Game) -> u64 {
    game.world.resource::<crate::resources::GameClock>().tick
}

/// A game past zone 1, standing on Open Grid with one step east onto
/// `onto`. The shipped catalogue is fixed Rust now, so there is no per-test
/// asset directory to build — every test runs against the real
/// `assets/` tree.
fn game_about_to_step(onto: Biome) -> Game {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    step_from_onto(&mut game, Biome::OpenGrid, onto, true);
    game
}

#[test]
fn a_step_onto_attrition_ground_costs_integrity() {
    let mut game = game_about_to_step(Biome::NullSector);
    let max_hp = game
        .world
        .get::<Stats>(game.player_entity())
        .unwrap()
        .max_hp;
    let before = player_hp(&game);

    game.move_player(1, 0);

    let expected = ((max_hp as f32 * 0.02).round() as i32).max(1);
    assert_eq!(before - player_hp(&game), expected);
}

#[test]
fn a_step_that_bounces_off_a_wall_costs_no_integrity() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    // Unwalkable Null Sector is unreachable on a generated map, which is the
    // point: what is under test is that the bite rides the *step*, and the
    // only way to hold everything else steady is to make the tile refuse.
    step_from_onto(&mut game, Biome::OpenGrid, Biome::NullSector, false);
    let before = player_hp(&game);

    game.move_player(1, 0);

    assert_eq!(player_hp(&game), before, "shoving at a wall is not travel");
}

/// The player alone takes environment damage. Corrupting the party would
/// route program deaths — and, on Permadeath, the run-ending path — through
/// something that is not a fight.
#[test]
fn attrition_never_touches_the_party() {
    let mut game = game_about_to_step(Biome::NullSector);
    let member = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, member);
    let before = game.world.get::<Stats>(member).unwrap().hp;
    assert!(game.world.resource::<Party>().0.contains(&member));

    game.move_player(1, 0);

    assert_eq!(game.world.get::<Stats>(member).unwrap().hp, before);
}

/// Free only because the bite goes through `Game::apply_damage`, the one
/// code path that lowers a creature's HP. This test is what stops someone
/// "simplifying" the hook into a direct write to `Stats::hp`.
#[test]
fn a_mitigation_buff_reduces_the_bite() {
    let mut game = game_about_to_step(Biome::Mainframe);
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 400;
        stats.hp = 400;
    }
    let unmitigated = {
        let mut probe = game_about_to_step(Biome::Mainframe);
        let p = probe.player_entity();
        {
            let mut stats = probe.world.get_mut::<Stats>(p).unwrap();
            stats.max_hp = 400;
            stats.hp = 400;
        }
        probe.move_player(1, 0);
        400 - player_hp(&probe)
    };
    assert!(unmitigated > 1, "the fixture has to have room to mitigate");

    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Mitigation,
            name: "Ablative Layer".into(),
            power: 50,
            remaining: 100,
            interval: 1,
            source: BuffSource::Routine,
        },
    );
    game.move_player(1, 0);

    assert!(
        400 - player_hp(&game) < unmitigated,
        "mitigation must apply to the ground the same way it applies to a hit"
    );
}

/// The one place in this phase where two systems meet at a lethal edge:
/// ground that kills must not then roll an ambush onto the corpse. Null
/// Sector with a forced live `LeakingMemory` epoch, so weather is in the sum
/// that kills — `maybe_ambush`'s `is_game_over` gate does not know or care
/// how many sources contributed to the damage.
#[test]
fn attrition_that_kills_does_not_then_start_an_ambush() {
    // Permadeath, because a Forgiving death reboots the player to full
    // Integrity inside the very tick this is asserting about — the corpse
    // the test needs to look at would be gone before it could.
    let mut game = Game::new(16, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    step_from_onto(&mut game, Biome::OpenGrid, Biome::NullSector, true);
    let epoch = (0..2000u64)
        .find(|&e| game.static_in_epoch(Biome::NullSector, e) == Some(StaticEvent::LeakingMemory))
        .expect("LeakingMemory must be reachable in Null Sector's pool");
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS + 1);
    {
        let mut stats = game.world.get_mut::<Stats>(game.player_entity()).unwrap();
        stats.hp = 1;
    }

    game.move_player(1, 0);

    assert_eq!(player_hp(&game), 0);
    assert!(
        !game.has_active_battle(),
        "a fight started against a dead player is unwinnable and unloseable"
    );
}

/// Both halves in one test on purpose: the effect half alone passes against
/// a bare early return that also swallowed the name.
#[test]
fn zone_one_takes_no_bite_but_still_names_the_ground() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.world.resource::<ZoneLevel>().0, 1);
    step_from_onto(&mut game, Biome::OpenGrid, Biome::NullSector, true);
    let before = player_hp(&game);

    game.move_player(1, 0);

    assert_eq!(player_hp(&game), before, "zone 1 is neutral ground");
    assert!(
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains(Biome::NullSector.name())),
        "the ground is named from the first step of a run"
    );
}

/// The base's own floor never bites, whatever a stray `Platform` tile turns
/// up under it — the refusal lives inside `Game::terrain_at`, not at the
/// call site.
#[test]
fn platform_takes_no_effect() {
    let mut game = game_about_to_step(Biome::Platform);
    let before = player_hp(&game);

    game.move_player(1, 0);

    assert_eq!(player_hp(&game), before, "the base's own floor never bites");
}

#[test]
fn a_step_onto_drag_ground_costs_the_extra_ticks() {
    let mut plain = game_about_to_step(Biome::OpenGrid);
    let before = clock(&plain);
    plain.move_player(1, 0);
    let ordinary = clock(&plain) - before;
    assert_eq!(ordinary, 1, "an ordinary step is one tick");

    let mut game = game_about_to_step(Biome::Deadlock);
    let before = clock(&game);

    game.move_player(1, 0);

    assert_eq!(clock(&game) - before, 2);
}

/// The second effect kind exists precisely so the vocabulary is not all
/// damage. Read off `Stats` rather than a downstream consequence.
#[test]
fn drag_ground_takes_no_integrity() {
    let mut game = game_about_to_step(Biome::Deadlock);
    let before = player_hp(&game);

    game.move_player(1, 0);

    assert_eq!(player_hp(&game), before);
}

/// A tick can start a fight — `nest_aggro_tick` is the precedent, and it is
/// why `rest`'s tick loop needed a battle check. Anything that ticks in a
/// loop inherits that obligation: the remaining ticks would resolve a world
/// the player is no longer standing in, while a fight waits on the screen.
#[test]
fn a_drag_step_stops_ticking_the_moment_a_battle_opens() {
    let mut game = game_about_to_step(Biome::Deadlock);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    // A provoked guardian already standing beside the destination reaches
    // the player on the step's very first tick, which is the only
    // deterministic way to open a fight from inside the loop.
    let nest = game.spawn_nest("scrapper", pos.x + 2, pos.y);
    game.provoke_nest(nest);
    let before = clock(&game);

    game.move_player(1, 0);

    assert!(
        game.has_active_battle(),
        "the fixture never started a fight"
    );
    assert_eq!(
        clock(&game) - before,
        1,
        "the remaining drag ticks must not run behind a fight the player has not seen"
    );
}

#[test]
fn the_crossing_line_names_a_biome_change_and_nothing_within_one_biome() {
    let mut same = game_about_to_step(Biome::OpenGrid);

    same.move_player(1, 0);

    assert!(
        !same
            .world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains("cross into")),
        "a step that stays within one biome logs nothing about crossing"
    );

    let mut crossing = game_about_to_step(Biome::NullSector);

    crossing.move_player(1, 0);

    assert!(
        crossing
            .world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains("cross into")),
        "a step across a biome boundary names the crossing"
    );
}

// ---------------------------------------------------------------- the weather

use crate::environment::StaticEvent;
use crate::resources::{GameClock, GameRng};
use crate::tests::support::reseed_rng;
use crate::tuning::STATIC_EPOCH_TICKS;

#[test]
fn every_static_event_has_a_name_a_description_and_a_pool() {
    for event in StaticEvent::all() {
        let def = event.def();
        assert!(!def.name.is_empty(), "{event:?} has no name");
        assert!(!def.description.is_empty(), "{event:?} has no description");
        assert!(!def.biomes.is_empty(), "{event:?} claims no biome");
    }
}

/// The census over ceilings from phase 1, extended to the fourth event kind.
#[test]
fn every_static_event_stays_inside_its_ceiling() {
    for event in StaticEvent::all() {
        let effect = event.def().effect;
        assert!(
            (0.0..=MAX_ENVIRONMENT_ATTRITION).contains(&effect.attrition_percent),
            "{event:?} authors attrition_percent {}",
            effect.attrition_percent
        );
        assert!(
            effect.min_damage >= 0,
            "{event:?} authors min_damage {}",
            effect.min_damage
        );
        assert!(
            effect.extra_ticks <= MAX_ENVIRONMENT_DRAG_TICKS,
            "{event:?} authors extra_ticks {}",
            effect.extra_ticks
        );
        assert!(
            effect.ambush_mult <= MAX_STATIC_AMBUSH_MULT,
            "{event:?} authors ambush_mult {}",
            effect.ambush_mult
        );
    }
}

/// The base's own floor, and the two biomes that are holes in the map, must
/// never grow weather — the same rule `for_biome_leaves_the_rest_of_the_map_neutral`
/// holds for `GroundCondition`.
#[test]
fn no_static_event_claims_platform_or_a_hole_in_the_map() {
    for event in StaticEvent::all() {
        for biome in [Biome::Platform, Biome::DataVoid, Biome::BlackIce] {
            assert!(!event.claims(biome), "{event:?} must not claim {biome:?}");
        }
    }
}

/// Every claimed biome must be one `WorldMap::classify` can actually stamp
/// on the surface, or the event ships unreachable.
#[test]
fn every_claimed_biome_is_one_classify_can_produce() {
    let producible = [
        Biome::Deadlock,
        Biome::NullSector,
        Biome::Mainframe,
        Biome::OpenGrid,
    ];
    for event in StaticEvent::all() {
        for &biome in event.def().biomes {
            assert!(
                producible.contains(&biome),
                "{event:?} claims {biome:?}, which classify never produces"
            );
        }
    }
}

/// The spec's reach argument: Deadlock alone is only the dominant biome in a
/// `cold_storage` sector, so an event claiming only Deadlock would ship as
/// unreachable as `LockContention` already is. This is the guard against
/// shipping a second one nobody meets.
#[test]
fn signal_noise_claims_a_biome_besides_deadlock() {
    assert!(
        StaticEvent::SignalNoise
            .def()
            .biomes
            .iter()
            .any(|&biome| biome != Biome::Deadlock),
        "SignalNoise must reach past Deadlock alone"
    );
}

fn fresh_game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn set_tick(game: &mut Game, tick: u64) {
    game.world.resource_mut::<GameClock>().tick = tick;
}

#[test]
fn static_at_is_stable_within_one_epoch() {
    let mut game = fresh_game(500);
    // Well clear of either edge of the epoch, so `+ 1` cannot cross a
    // boundary by construction.
    let start = 7 * STATIC_EPOCH_TICKS + 3;
    set_tick(&mut game, start);
    let a = game.static_at(Biome::NullSector);

    set_tick(&mut game, start + 1);
    let b = game.static_at(Biome::NullSector);

    assert_eq!(a, b, "two ticks inside the same epoch must agree");
}

/// Over many epochs a single biome must turn over, and both a live event and
/// clear ground must be reachable — otherwise `STATIC_CLEAR_WEIGHT` or a
/// pool weight is wrong, or the fold itself never varies.
#[test]
fn static_at_changes_across_epochs() {
    let game = fresh_game(500);
    let answers: Vec<Option<StaticEvent>> = (0..300u64)
        .map(|epoch| game.static_in_epoch(Biome::NullSector, epoch))
        .collect();

    assert!(
        answers.iter().any(|a| *a != answers[0]),
        "weather never turned over across 300 epochs"
    );
    assert!(
        answers.contains(&None),
        "clear ground must be reachable in Null Sector's pool"
    );
    assert!(
        answers.iter().any(|a| a.is_some()),
        "a live event must be reachable in Null Sector's pool"
    );
}

/// The worldgen rule: a derivation must not draw from the shared `GameRng`,
/// or it would not survive a save/load and would shift every later roll in
/// the run.
#[test]
fn static_at_draws_no_game_rng() {
    use rand::RngExt;
    let mut game = fresh_game(501);
    reseed_rng(&mut game, 4242);

    for epoch in 0..100u64 {
        let _ = game.static_in_epoch(Biome::Mainframe, epoch);
    }
    let _ = game.static_at(Biome::OpenGrid);

    let mut untouched: rand::rngs::StdRng = rand::SeedableRng::seed_from_u64(4242);
    let expected: u64 = untouched.random();
    let actual: u64 = game.world.resource_mut::<GameRng>().0.random();
    assert_eq!(
        actual, expected,
        "static_at moved the shared RNG stream, so it would shift every later \
         roll in the run"
    );
}

/// The derivation reads the world seed, the zone and the clock — all three
/// survive a real save and load, so the answer must too.
#[test]
fn static_at_survives_a_save_and_load_round_trip() {
    let mut game = fresh_game(502);
    set_tick(&mut game, 40 * STATIC_EPOCH_TICKS + 10);
    let biomes = [
        Biome::NullSector,
        Biome::Mainframe,
        Biome::OpenGrid,
        Biome::Deadlock,
    ];
    let before: Vec<Option<StaticEvent>> = biomes.iter().map(|&b| game.static_at(b)).collect();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_static_weather_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let after: Vec<Option<StaticEvent>> = biomes.iter().map(|&b| loaded.static_at(b)).collect();

    assert_eq!(
        before, after,
        "weather is derived, never stored — a reload must not move it"
    );
}

/// Two biomes folding the same seed and zone, differing only in the biome
/// word `static_seed` mixes in, must not answer identically epoch for
/// epoch — a hash that let the biome wash out would make every claimed
/// biome's weather turn over in lockstep, which would read as one global
/// weather flag rather than a property of the place.
///
/// This does **not** guard `derive::index` against `%`: that reduction's
/// documented failure is a *two-entry* pool reading nothing but its seed's
/// lowest bit, and `static_in_epoch`'s pools here are 4-5 entries (the clear
/// weight plus each claimed event) with a varying epoch folded in last, so
/// `%` decorrelates these two sequences too. `derive::index` stays the
/// implementation regardless — it is the global rule and the rest of the
/// codebase's convention — but this test's reach is the decorrelation
/// itself, not a choice between the two reductions.
#[test]
fn adjacent_biomes_decorrelate_across_epochs() {
    let game = fresh_game(503);
    let null_sector: Vec<Option<StaticEvent>> = (0..200u64)
        .map(|epoch| game.static_in_epoch(Biome::NullSector, epoch))
        .collect();
    let mainframe: Vec<Option<StaticEvent>> = (0..200u64)
        .map(|epoch| game.static_in_epoch(Biome::Mainframe, epoch))
        .collect();

    assert_ne!(
        null_sector, mainframe,
        "two biomes folding the same seed, zone and epoch must not answer identically"
    );
}

// ------------------------------------------------------- weather reaches the player

/// The guard on the fold reaching `terrain_at`: a reader that took only the
/// bite would compile clean and silently drop the drag and ambush terms, so
/// all three are asserted together, the shape
/// `fold_adds_attrition_and_drag_and_multiplies_ambush` already checked for
/// `EnvironmentEffect` alone. Null Sector is the one biome where a ground
/// condition and a weather event both claim the same tile, which is what
/// makes stacking observable here rather than merely folding onto `NONE`.
#[test]
fn ground_and_weather_effects_stack() {
    let mut game = game_about_to_step(Biome::NullSector);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let (nx, ny) = (pos.x + 1, pos.y);
    let epoch = (0..2000u64)
        .find(|&e| game.static_in_epoch(Biome::NullSector, e) == Some(StaticEvent::LeakingMemory))
        .expect("LeakingMemory must be reachable in Null Sector's pool");
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS + 1);

    let terrain = game.terrain_at(nx, ny);

    assert_eq!(terrain.event, Some(StaticEvent::LeakingMemory));
    let ground = GroundCondition::DanglingReads.def().effect;
    let weather = StaticEvent::LeakingMemory.def().effect;
    assert_eq!(
        terrain.effect.attrition_percent,
        ground.attrition_percent + weather.attrition_percent,
        "attrition sums"
    );
    assert_eq!(
        terrain.effect.min_damage,
        ground.min_damage + weather.min_damage,
        "the floor sums"
    );
    assert_eq!(
        terrain.effect.extra_ticks,
        ground.extra_ticks + weather.extra_ticks,
        "drag sums"
    );
    assert_eq!(
        terrain.effect.ambush_mult,
        ground.ambush_mult * weather.ambush_mult,
        "the ambush term multiplies"
    );
}

/// The one-call guard: `apply_damage`'s mitigation floors any positive
/// damage at 1, so two sources billed through two calls floor twice.
/// Chosen so the numbers only pull apart under that per-call floor — small
/// enough that each source's own attrition rounds away to nothing and only
/// its `min_damage` survives, and mitigated hard enough that even the
/// summed bite rounds under 1 before the floor catches it once.
#[test]
fn ground_and_weather_attrition_lands_as_one_bite() {
    let mut game = game_about_to_step(Biome::NullSector);
    let epoch = (0..2000u64)
        .find(|&e| game.static_in_epoch(Biome::NullSector, e) == Some(StaticEvent::LeakingMemory))
        .expect("LeakingMemory must be reachable in Null Sector's pool");
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS + 1);
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 5;
        stats.hp = 5;
    }
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Mitigation,
            name: "Ablative Layer".into(),
            power: 60,
            remaining: 100,
            interval: 1,
            source: BuffSource::Routine,
        },
    );

    game.move_player(1, 0);

    assert_eq!(
        5 - player_hp(&game),
        1,
        "the summed attrition must land through one apply_damage call — \
         one call per source would each floor at 1 and cost 2"
    );
}

#[test]
fn zone_one_takes_no_weather() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.world.resource::<ZoneLevel>().0, 1);
    step_from_onto(&mut game, Biome::OpenGrid, Biome::NullSector, true);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let (nx, ny) = (pos.x + 1, pos.y);
    // Forced live so a zone-1 read that skipped the gate would be caught —
    // the gate itself does not consult the clock, so any epoch would
    // otherwise pass this test by coincidence.
    let epoch = (0..2000u64)
        .find(|&e| game.static_in_epoch(Biome::NullSector, e).is_some())
        .expect("a live epoch must be reachable in Null Sector's pool");
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS + 1);

    let terrain = game.terrain_at(nx, ny);

    assert_eq!(terrain.event, None, "zone 1 takes no weather");
    assert_eq!(terrain.effect, EnvironmentEffect::NONE);
    assert_eq!(
        terrain.biome,
        Biome::NullSector,
        "the ground is still named"
    );
}

#[test]
fn platform_takes_no_weather() {
    let mut game = game_about_to_step(Biome::Platform);
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let (nx, ny) = (pos.x + 1, pos.y);

    let terrain = game.terrain_at(nx, ny);

    assert_eq!(terrain.event, None, "the base's own floor takes no weather");
    assert_eq!(terrain.effect, EnvironmentEffect::NONE);
}

/// `Game::environment_biome_at` is the one gate `terrain_at` and
/// `note_static_turnover` both read — Task 4 shipped `note_static_turnover`
/// with its own copy of these two checks, which is exactly the shape
/// CLAUDE.md's ground section warns about: nothing fails to compile when
/// one copy is edited and the other is not. Testing the shared predicate
/// directly, rather than only through `terrain_at`, is what would catch a
/// future edit to this one definition going wrong for *either* caller —
/// `zone_one_takes_no_weather` and `platform_takes_no_weather` above only
/// ever exercised it through `terrain_at`.
#[test]
fn environment_biome_at_refuses_zone_one_and_platform() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.world.resource::<ZoneLevel>().0, 1);
    assert_eq!(
        game.environment_biome_at(0, 0),
        None,
        "zone 1 refuses everywhere, whatever the biome"
    );

    game.world.resource_mut::<ZoneLevel>().0 = 2;
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.world.resource_mut::<WorldMap>().set_override(
        pos.x,
        pos.y,
        Tile {
            biome: Biome::Platform,
            walkable: true,
            rock_shade: None,
        },
    );
    assert_eq!(
        game.environment_biome_at(pos.x, pos.y),
        None,
        "the base's own floor refuses past zone 1 too"
    );

    game.world.resource_mut::<WorldMap>().set_override(
        pos.x,
        pos.y,
        Tile {
            biome: Biome::NullSector,
            walkable: true,
            rock_shade: None,
        },
    );
    assert_eq!(
        game.environment_biome_at(pos.x, pos.y),
        Some(Biome::NullSector),
        "an ordinary biome past zone 1 takes environment effects"
    );
}

/// A single ambush attempt on a fresh, independently seeded fixture: builds
/// the game, stamps the player's own tile as `Mainframe`, sets the clock to
/// `tick`, reseeds `GameRng` to `rng_seed`, then reports whether that one
/// `maybe_ambush` call started a battle. Independent games rather than one
/// game looped, so a battle starting on one trial never blocks the next.
fn ambush_fires(seed: u32, rng_seed: u64, tick: u64) -> bool {
    let mut game = fresh_game(seed);
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    game.world.resource_mut::<WorldMap>().set_override(
        pos.x,
        pos.y,
        Tile {
            biome: Biome::Mainframe,
            walkable: true,
            rock_shade: None,
        },
    );
    set_tick(&mut game, tick);
    reseed_rng(&mut game, rng_seed);
    game.maybe_ambush();
    game.has_active_battle()
}

/// The multiplier reaches the roll, not just the number on `Terrain`: the
/// same 400 seeded trials, once with Mainframe's epoch forced to
/// `ThreadStorm` and once forced clear, must ambush more often live than
/// clear. Asserted as a **difference**, never an absolute rate — an
/// absolute is a seed-luck test that fails the day an unrelated change
/// shifts the RNG stream, and `-p feral-processes-engine` vs `--workspace`
/// already shift it differently for identical source.
#[test]
fn ambush_multiplier_reaches_the_roll() {
    let seed = 900;
    let mut probe = fresh_game(seed);
    probe.world.resource_mut::<ZoneLevel>().0 = 2;
    let live_epoch = (0..2000u64)
        .find(|&e| probe.static_in_epoch(Biome::Mainframe, e) == Some(StaticEvent::ThreadStorm))
        .expect("ThreadStorm must be reachable in Mainframe's pool");
    let clear_epoch = (0..2000u64)
        .find(|&e| probe.static_in_epoch(Biome::Mainframe, e).is_none())
        .expect("clear must be reachable in Mainframe's pool");

    let trials = 400u64;
    let live_hits = (0..trials)
        .filter(|&i| ambush_fires(seed, i, live_epoch * STATIC_EPOCH_TICKS + 1))
        .count();
    let clear_hits = (0..trials)
        .filter(|&i| ambush_fires(seed, i, clear_epoch * STATIC_EPOCH_TICKS + 1))
        .count();

    assert!(
        live_hits > clear_hits,
        "a live ambush multiplier must ambush more often: live {live_hits} \
         vs clear {clear_hits} across {trials} trials"
    );
}

/// `a_mitigation_buff_reduces_the_bite`'s shape, over the stacked
/// ground-and-weather bite rather than ground alone — mitigation goes
/// through `apply_damage`, which does not know how many sources summed into
/// the number it received.
#[test]
fn mitigation_blunts_the_stacked_bite() {
    let mut game = game_about_to_step(Biome::NullSector);
    let epoch = (0..2000u64)
        .find(|&e| game.static_in_epoch(Biome::NullSector, e) == Some(StaticEvent::LeakingMemory))
        .expect("LeakingMemory must be reachable in Null Sector's pool");
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS + 1);
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 400;
        stats.hp = 400;
    }
    let unmitigated = {
        // Same seed as `game_about_to_step` uses, so the same epoch answers
        // the same way for this probe.
        let mut probe = game_about_to_step(Biome::NullSector);
        set_tick(&mut probe, epoch * STATIC_EPOCH_TICKS + 1);
        let p = probe.player_entity();
        {
            let mut stats = probe.world.get_mut::<Stats>(p).unwrap();
            stats.max_hp = 400;
            stats.hp = 400;
        }
        probe.move_player(1, 0);
        400 - player_hp(&probe)
    };
    assert!(unmitigated > 1, "the fixture has to have room to mitigate");

    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Mitigation,
            name: "Ablative Layer".into(),
            power: 50,
            remaining: 100,
            interval: 1,
            source: BuffSource::Routine,
        },
    );
    game.move_player(1, 0);

    assert!(
        400 - player_hp(&game) < unmitigated,
        "mitigation must blunt the stacked ground-and-weather bite the same \
         way it blunts the ground alone"
    );
}

/// `attrition_never_touches_the_party`'s shape, with a live weather event
/// folded into the ground's bite — corrupting the party would route program
/// deaths, and on Permadeath the run-ending path, through something that is
/// not a fight.
#[test]
fn party_untouched_by_weather() {
    let mut game = game_about_to_step(Biome::NullSector);
    let epoch = (0..2000u64)
        .find(|&e| game.static_in_epoch(Biome::NullSector, e) == Some(StaticEvent::LeakingMemory))
        .expect("LeakingMemory must be reachable in Null Sector's pool");
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS + 1);
    let member = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, member);
    let before = game.world.get::<Stats>(member).unwrap().hp;
    assert!(game.world.resource::<Party>().0.contains(&member));

    game.move_player(1, 0);

    assert_eq!(game.world.get::<Stats>(member).unwrap().hp, before);
}

// ------------------------------------------------------------- the readout

/// Trigger 1: the crossing line gains the condition's name, joined onto the
/// existing biome line rather than getting one of its own.
#[test]
fn crossing_line_names_the_condition() {
    let mut game = game_about_to_step(Biome::NullSector);

    game.move_player(1, 0);

    let condition_name = GroundCondition::DanglingReads.def().name;
    assert!(
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains(Biome::NullSector.name()) && l.text.contains(condition_name)),
        "the crossing line must name both the biome and its condition"
    );
}

/// Trigger 1's other half: unclaimed ground must read exactly as it did
/// before this feature — no condition name appended to a biome that has
/// none.
#[test]
fn crossing_into_unclaimed_ground_names_no_condition() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    step_from_onto(&mut game, Biome::NullSector, Biome::OpenGrid, true);

    game.move_player(1, 0);

    assert!(
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text == format!("You cross into {}.", Biome::OpenGrid.name())),
        "Open Grid must read exactly as it does today"
    );
}

/// Trigger 2: the condition's description is read for the first time this
/// session, and never again — leaving and crossing back must not repeat it.
/// Counted on the **raw** log rather than a folded entry count:
/// `message_history` collapses repeats into one entry with a `repeats`
/// count, so counting entries would pass even if the line fired five times.
#[test]
fn condition_description_fires_once_per_session() {
    let mut game = game_about_to_step(Biome::NullSector);
    let description = GroundCondition::DanglingReads.def().description;

    game.move_player(1, 0); // cross into Null Sector: first sight
    game.move_player(-1, 0); // leave, back onto Open Grid
    game.move_player(1, 0); // cross back into Null Sector: already seen

    let count = game
        .world
        .resource::<MessageLog>()
        .lines
        .iter()
        .filter(|l| l.text.contains(description))
        .count();
    assert_eq!(
        count, 1,
        "the description must fire exactly once per session, not once per crossing"
    );
}

/// The epoch a Null-Sector run just crossed into `LeakingMemory`, with the
/// epoch *before* it forced clear — the `None -> Some` half of the
/// boundary, the only direction that is an arrival.
fn null_sector_arrival_epoch(game: &Game) -> u64 {
    (1..2000u64)
        .find(|&e| {
            game.static_in_epoch(Biome::NullSector, e) == Some(StaticEvent::LeakingMemory)
                && game.static_in_epoch(Biome::NullSector, e - 1).is_none()
        })
        .expect("a clear-to-LeakingMemory transition must be reachable in Null Sector")
}

/// A game standing on `biome` already — both the current tile and the one
/// step east are `biome` — so a further step inside it logs no crossing
/// line at all and only `note_static_turnover`'s own message can appear.
fn game_standing_on(biome: Biome) -> Game {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    step_from_onto(&mut game, biome, biome, true);
    game
}

/// Trigger 3: weather arriving fires on the tick that crosses the epoch
/// boundary, names the biome the player stands in, and carries the event's
/// description.
#[test]
fn weather_arrival_fires_on_the_boundary_in_the_players_biome() {
    let mut game = game_standing_on(Biome::NullSector);
    let epoch = null_sector_arrival_epoch(&game);
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS - 1);

    game.move_player(1, 0);

    let description = StaticEvent::LeakingMemory.def().description;
    let lines = &game.world.resource::<MessageLog>().lines;
    assert!(
        lines
            .iter()
            .any(|l| l.text.contains(description) && l.text.contains(Biome::NullSector.name())),
        "the arrival line must name the biome and carry the event's description"
    );
}

/// Trigger 4: the same boundary, the other way — a live event clearing.
#[test]
fn weather_clearing_fires_on_the_boundary_the_other_way() {
    let mut game = game_standing_on(Biome::NullSector);
    let epoch = (1..2000u64)
        .find(|&e| {
            game.static_in_epoch(Biome::NullSector, e - 1) == Some(StaticEvent::LeakingMemory)
                && game.static_in_epoch(Biome::NullSector, e).is_none()
        })
        .expect("a LeakingMemory-to-clear transition must be reachable in Null Sector");
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS - 1);

    game.move_player(1, 0);

    let name = StaticEvent::LeakingMemory.def().name;
    let lines = &game.world.resource::<MessageLog>().lines;
    assert!(
        lines
            .iter()
            .any(|l| l.text.contains(name) && l.text.contains(Biome::NullSector.name())),
        "the clearing line must name the event and the biome"
    );
}

/// Trigger 3/4's boundary: the other four biomes turning over must stay
/// silent. The player stands in Open Grid while Null Sector's own boundary
/// crosses into `LeakingMemory`.
#[test]
fn turnover_in_a_biome_the_player_is_not_standing_in_is_silent() {
    let mut game = game_standing_on(Biome::OpenGrid);
    let epoch = null_sector_arrival_epoch(&game);
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS - 1);

    game.move_player(1, 0);

    let description = StaticEvent::LeakingMemory.def().description;
    assert!(
        !game
            .world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains(description)),
        "weather turning over in a biome the player isn't standing in must stay silent"
    );
}

/// `terrain_at` and `note_static_turnover` reading the same
/// `environment_biome_at` means a zone-1 player crossing straight through a
/// live-weather arrival cannot see one caller refuse it while the other
/// announces it — the seam Task 4's two independent copies of the same two
/// checks put at risk. Asserted on the actual arrival boundary rather than
/// an arbitrary tick, so this exercises exactly the moment
/// `weather_arrival_fires_on_the_boundary_in_the_players_biome` fires the
/// notice at zone 2.
#[test]
fn zone_one_terrain_and_turnover_agree_on_no_weather() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.world.resource::<ZoneLevel>().0, 1);
    step_from_onto(&mut game, Biome::OpenGrid, Biome::NullSector, true);
    let epoch = null_sector_arrival_epoch(&game);
    set_tick(&mut game, epoch * STATIC_EPOCH_TICKS - 1);

    game.move_player(1, 0);

    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    assert_eq!(
        game.terrain_at(pos.x, pos.y).event,
        None,
        "terrain_at must refuse weather at zone 1"
    );
    let description = StaticEvent::LeakingMemory.def().description;
    assert!(
        !game
            .world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains(description)),
        "note_static_turnover must refuse the same arrival terrain_at refuses"
    );
}

/// Nothing about the previous epoch is stored anywhere — `static_epoch` and
/// `static_in_epoch` are both pure calls, never a saved field — so landing a
/// save/load comfortably inside a live epoch (well clear of the boundary
/// itself) and then taking a further step still inside that same epoch must
/// not manufacture a crossing that never happened. `MessageLog` itself is
/// not saved (both `Game` constructors reset it to `default`), so this
/// cannot be "the old line survived the round trip" — it is asserting that
/// the round trip does not fabricate a *new* one, which is what a stray
/// stored "last known epoch" defaulting to 0 after load would do.
#[test]
fn save_load_mid_epoch_does_not_reannounce_arrival() {
    let mut game = game_standing_on(Biome::NullSector);
    let epoch = null_sector_arrival_epoch(&game);
    set_tick(
        &mut game,
        epoch * STATIC_EPOCH_TICKS + STATIC_EPOCH_TICKS / 2,
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_static_weather_turnover_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    // Two more steps well inside the same epoch, crossing no boundary.
    loaded.move_player(1, 0);
    loaded.move_player(-1, 0);

    let description = StaticEvent::LeakingMemory.def().description;
    assert!(
        !loaded
            .world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains(description)),
        "a save/load landing mid-epoch must not fabricate an arrival that never happened"
    );
}
