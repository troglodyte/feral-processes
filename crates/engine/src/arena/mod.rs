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

mod report;
mod run;
mod scenario;
mod setup;

pub use report::{RepRecord, Report, Summary};
pub use scenario::{CompanionSpec, EquipSpec, InventorySpec, OpponentSpec, PlayerSource, Scenario};

use std::path::Path;

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

/// Runs `scenario` and reports what happened. The engine's whole public
/// arena surface.
///
/// **A fresh `Game` per rep.** One carried over would bring the last
/// fight's dead companions, spent items and XP with it, so rep 2 would
/// measure a different party from rep 1 — and the drift would compound.
/// Warnings are taken from the first rep only: they are identical every
/// rep, and fifty copies of the same line is noise.
pub fn run(scenario: &Scenario, assets_dir: &Path) -> Result<Report, String> {
    let mut warnings = Vec::new();
    let mut reps = Vec::with_capacity(scenario.reps as usize);
    for rep in 0..scenario.reps {
        let mut game = setup::build_player(scenario, assets_dir)?;
        let (groups, rep_warnings) = setup::build_opponents(&mut game, &scenario.opponents)?;
        if rep == 0 {
            warnings = rep_warnings;
        }
        reps.push(run::run_rep(&mut game, groups, scenario.seed + rep as u64));
    }
    Ok(Report {
        scenario: scenario.clone(),
        warnings,
        reps,
    })
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

    fn a_scenario(reps: u32, seed: u64, party: &[(&str, u32)]) -> Scenario {
        Scenario {
            player: PlayerSource::Fresh { level: 6, zone: 2 },
            party: party
                .iter()
                .map(|(species, level)| CompanionSpec {
                    species: (*species).into(),
                    level: *level,
                })
                .collect(),
            opponents: vec![OpponentSpec {
                species: "sub_process".into(),
                count: 3,
            }],
            reps,
            seed,
            ..Scenario::default()
        }
    }

    #[test]
    fn each_rep_runs_at_its_own_seed_counted_up_from_the_scenarios() {
        let report = run(&a_scenario(3, 40, &[("glitch", 4)]), &test_assets_dir()).unwrap();
        let seeds: Vec<u64> = report.reps.iter().map(|r| r.seed).collect();
        assert_eq!(seeds, vec![40, 41, 42]);
    }

    #[test]
    fn the_same_scenario_run_twice_reports_the_same_thing() {
        // The property the whole tool rests on: without it a tuning
        // comparison measures noise.
        let s = a_scenario(3, 40, &[("glitch", 4)]);
        let a = run(&s, &test_assets_dir()).unwrap();
        let b = run(&s, &test_assets_dir()).unwrap();
        assert_eq!(a.reps, b.reps);
    }

    #[test]
    fn a_party_wiped_in_one_rep_is_whole_again_in_the_next() {
        // Asserting on rep 2's transcript rather than on its `won` flag: a
        // shared `Game` would field a corpse, and a lopsided enough fight
        // would still be won without the companion ever swinging.
        let s = Scenario {
            player: PlayerSource::Fresh { level: 1, zone: 4 },
            party: vec![CompanionSpec {
                species: "glitch".into(),
                level: 1,
            }],
            opponents: vec![OpponentSpec {
                species: "sub_process".into(),
                count: 6,
            }],
            reps: 2,
            seed: 11,
            ..Scenario::default()
        };
        let report = run(&s, &test_assets_dir()).unwrap();

        let swung = |rep: &RepRecord| rep.transcript.iter().any(|l| l.contains("Glitch"));
        assert!(swung(&report.reps[0]), "{:?}", report.reps[0].transcript);
        assert!(
            swung(&report.reps[1]),
            "rep 2 fielded no companion — the `Game` was carried over: {:?}",
            report.reps[1].transcript
        );
    }
}
