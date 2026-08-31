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
/// ground that kills must not then roll an ambush onto the corpse.
#[test]
fn attrition_that_kills_does_not_then_start_an_ambush() {
    // Permadeath, because a Forgiving death reboots the player to full
    // Integrity inside the very tick this is asserting about — the corpse
    // the test needs to look at would be gone before it could.
    let mut game = Game::new(16, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    game.world.resource_mut::<ZoneLevel>().0 = 2;
    step_from_onto(&mut game, Biome::OpenGrid, Biome::Mainframe, true);
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
