//! A scenario-driven harness that runs real battles offline.
//!
//! Pick the opponents and, on a fresh player, the items; run N seeded reps;
//! keep the round-by-round transcript. Difficulty can then be tuned by
//! measurement rather than by playing to the fight.
//!
//! This is inside the engine crate deliberately. `start_battle`,
//! `spawn_wild_creature_scaled` and the `world` field are all reachable from
//! here and from nowhere outside, so the arena adds **no public `Game`
//! method at all** — the compiler barrier keeping the renderer out of the
//! ECS is untouched.
//!
//! Its known blind spot, stated rather than hidden: the party plays the
//! game's own All-Attack, which fires no companion Specials. An arena number
//! is a floor on the party's output, the same gap `balance_sim` has.

mod scenario;

pub use scenario::{CompanionSpec, EquipSpec, InventorySpec, OpponentSpec, PlayerSource, Scenario};

use crate::progression;
use crate::tuning::{BASELINE_GROWTH_MULTIPLIER, CREATURE_MAX_LEVEL};
use crate::*;

/// Raises `entity` to `level` the way play would.
///
/// Awarding XP rather than writing `Experience.level` is the whole point:
/// the growth curve lives in `progression::add_xp` and there is no second
/// copy of it here to drift from it. A creature set to level 20 with
/// level-1 stats is the failure this exists to make unreachable — an arena
/// scenario naming a level would otherwise measure a fight nobody can have.
///
/// Which multiplier and which ceiling apply is the same split
/// `award_player_xp` and `award_companion_xp` make: a `Creature` grows on
/// its species' curve and stops at `CREATURE_MAX_LEVEL`, the player grows
/// on the baseline and has no ceiling.
///
/// Shared with `tests/support.rs`, which re-exports it — two copies would
/// be two answers to "what is a level-N companion".
pub(crate) fn set_level(game: &mut Game, entity: Entity, level: u32) {
    let before = game
        .world
        .get::<Experience>(entity)
        .map(|e| e.level)
        .unwrap_or(1);
    let (growth, cap) = match game.world.get::<Creature>(entity) {
        Some(creature) => {
            let species = creature.species.clone();
            let growth = game
                .world
                .resource::<SpeciesDb>()
                .get(&species)
                .map(|s| s.growth_multiplier)
                .unwrap_or(BASELINE_GROWTH_MULTIPLIER);
            (growth, Some(CREATURE_MAX_LEVEL))
        }
        None => (BASELINE_GROWTH_MULTIPLIER, None),
    };

    let mut query = game.world.query::<(&mut Experience, &mut Stats)>();
    let Ok((mut exp, mut stats)) = query.get_mut(&mut game.world, entity) else {
        return;
    };
    // One level per pass, by paying exactly what the next one costs — so
    // the XP left over at the end is zero rather than an arbitrary
    // remainder a later kill would inherit.
    while exp.level < level {
        let owed = exp.xp_to_next.saturating_sub(exp.xp);
        if progression::add_xp(&mut exp, &mut stats, owed, growth, cap, 0) == 0 {
            break;
        }
    }

    if level > before {
        game.install_unlocked_routines(entity, before, level);
    }
}

/// A companion of `species` at `level`, standing on the player's own tile.
///
/// `Game::adopt_program` does the becoming-a-companion half, which is what
/// keeps this from being the third copy of that bundle `CLAUDE.md` warns
/// about — `install_innate_routines` is the step such a copy dropped once.
/// It deliberately does not push onto `Party`; which programs are fielded
/// is the caller's choice, and `build_player` makes it.
///
/// `None` for a species the roster does not hold: a scenario is authored,
/// so a typo should stop the run rather than quietly field a different
/// program.
pub(crate) fn spawn_companion(game: &mut Game, species: &str, level: u32) -> Option<Entity> {
    let pos = *game.world.get::<Position>(game.player_entity())?;
    let program = game.adopt_program(species, pos.x, pos.y, 1.0)?;
    set_level(game, program, level);
    Some(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::test_assets_dir;

    fn a_species(game: &Game) -> String {
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    }

    #[test]
    fn a_companion_spawns_at_the_requested_level_with_its_kit_installed() {
        let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species = a_species(&game);
        let baseline = spawn_companion(&mut game, &species, 1).unwrap();
        let baseline_atk = game.world.get::<Stats>(baseline).unwrap().atk;

        let program = spawn_companion(&mut game, &species, 5).unwrap();

        assert_eq!(
            game.world.get::<Creature>(program).unwrap().species,
            species
        );
        assert_eq!(
            game.world.get::<Tamed>(program).unwrap().owner,
            game.player_entity()
        );
        assert_eq!(game.world.get::<Experience>(program).unwrap().level, 5);
        assert!(
            game.world.get::<Stats>(program).unwrap().atk > baseline_atk,
            "four levels of growth should show in the stats"
        );
        assert!(
            !game.world.get::<Routines>(program).unwrap().0.is_empty(),
            "a companion arrives with its innate routines"
        );
    }

    #[test]
    fn an_unknown_species_spawns_no_companion() {
        let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(spawn_companion(&mut game, "not_a_program", 3).is_none());
    }
}
