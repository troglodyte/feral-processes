//! A scenario-driven harness that runs real battles offline.
//!
//! Pick the opponents — or a context to roll them from — and, on a fresh
//! player, the items; run N seeded reps; keep the round-by-round transcript.
//! Difficulty can then be tuned by measurement rather than by playing to the
//! fight.
//!
//! This is inside the engine crate deliberately. `start_battle`,
//! `spawn_wild_creature_scaled` and the `world` field are all reachable from
//! here and from nowhere outside, so the arena adds **no public `Game`
//! method at all** — the compiler barrier keeping the renderer out of the
//! ECS is untouched.
//!
//! Its known blind spot, stated rather than hidden: by default the party
//! plays the game's own All-Attack, which braces for nobody and fires no
//! companion Specials. An arena number is a floor on the party's output, the
//! same gap `balance_sim` has. `RunOptions::party` lifts half of that —
//! `PartyPlan::BraceWhenHurt` makes Defend reachable — and Specials remain
//! unexercised.

mod encounter;
mod report;
mod run;
mod scenario;
mod setup;
mod watch;

pub use report::{RepRecord, Report, Summary};
pub use scenario::{
    CharacterSpec, CompanionSpec, Encounter, EquipSpec, InventorySpec, OpponentSpec, PlayerSource,
    Scenario,
};
pub use watch::Watch;

use std::path::Path;

use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::progression;
use crate::resources::GameRng;
use crate::telemetry::Record;
use crate::tuning::{BASELINE_GROWTH_MULTIPLIER, arena_level_ceiling};
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
/// its species' curve and stops at the higher of the scenario's zone cap
/// and `tuning::arena_level_ceiling()`,
/// the player grows on the baseline and has no ceiling.
///
/// The *absolute* cap rather than `Game::companion_level_cap`, and that is
/// deliberate: an arena scenario authors its own composition and has no
/// `KernelRing` to read. `Ability`, `Affinity` and `RoutineSlot` talents are
/// invisible to `balance_sim`, so the arena is the only instrument that can
/// see them, and one clamped at `TALENT_START_LEVEL` could not stage the
/// fight the talent trees exist to change.
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
            (growth, Some(game.level_cap().max(arena_level_ceiling())))
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
        if progression::add_xp(&mut exp, &mut stats, owed, growth, cap, 0).levels == 0 {
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

/// A fight set up and open, with nobody having acted yet.
pub struct Staged {
    pub game: Game,
    pub watch: Watch,
    /// What the composition asks for past the zone's ceilings. Shown, never
    /// applied — explicit authoring is the point of a tester.
    pub warnings: Vec<String>,
}

/// Everything between "here is a scenario" and "the first round may be
/// planned".
///
/// The one way into an arena fight, whether a person or `run_rep` is going
/// to press the keys. Both halves therefore agree about the RNG stream, the
/// log's retention and who counts as an opponent without either knowing the
/// other exists.
///
/// `seed` is a parameter rather than `scenario.seed`, because rep *n* runs
/// at `scenario.seed + n` and the result screen's next-seed key is the same
/// increment — a `stage` that read the field would force both callers to
/// mutate a scenario they do not own.
pub fn stage(
    scenario: &Scenario,
    assets_dir: &Path,
    seed: u64,
    telemetry: bool,
) -> Result<Staged, String> {
    let mut game = setup::build_player(scenario, assets_dir)?;
    // Armed here rather than by whoever installs the `Game`, because
    // `begin_battle` below is what emits `fight_start` — a game armed after
    // staging joins its own fight already in progress. `run_rep` passes
    // `false`: the headless bin's output is its `Report`, and `train` runs
    // 1.9M fights a session that must not each open a file.
    if telemetry {
        game.enable_battle_telemetry();
    }

    // Per fight, not per run: twenty reps are then a sample rather than
    // twenty copies, and any one of them replays alone from its own seed.
    //
    // Before the opponents, not after: the composition is part of what a rep
    // samples. An authored one still rolls a `Potential` per member, and a
    // rolled one *is* the sample — a seed installed afterwards would draw
    // both from `Game::new(0)`'s stream and hand every rep the same pack.
    game.world
        .insert_resource(GameRng(StdRng::seed_from_u64(seed)));

    let (groups, warnings) = match &scenario.encounter {
        // A rolled encounter warns about nothing: nothing was asked for past
        // a ceiling, because nothing was asked for.
        Some(encounter) => (encounter::roll(&mut game, encounter)?, Vec::new()),
        None => setup::build_opponents(&mut game, &scenario.opponents)?,
    };

    // The arena's output is the blow-by-blow, so the prune that keeps a map
    // pane readable has nothing to do here — and it deletes the lines
    // outright from inside `battle_resolve_round`, so the round that ends
    // the fight cannot be read back afterwards.
    game.world
        .resource_mut::<MessageLog>()
        .keep_battle_narration = true;

    // Noted before the fight, because `BattleState` owns the groups from
    // here on and is gone again by the time the answer is wanted.
    let watch = Watch::new(&game, seed, &groups);
    game.begin_battle(groups);

    Ok(Staged {
        game,
        watch,
        warnings,
    })
}

/// Per-run knobs, with `Default` meaning "just play the fights".
///
/// A struct rather than a parameter per knob: the callers that want plain
/// behaviour spell it `RunOptions::default()` once and never change again,
/// so the next thing that has to reach `run_rep` — a party plan, a round
/// cap — costs no call-site churn.
#[derive(Clone, Debug, Default)]
pub struct RunOptions {
    /// Collect per-fight telemetry and hand it back beside the report.
    ///
    /// Off by default, and both existing callers want it that way: the
    /// headless bin's output is its `Report`, and `train`'s search runs
    /// 1.9M fights that must not each be recorded. It is the *evaluation*
    /// passes either side of that search that are worth logging.
    pub telemetry: bool,
    /// How the party plays its rounds.
    pub party: PartyPlan,
}

/// How the party decides its actions in an arena fight.
///
/// The default is the whole of the arena's original guarantee — see
/// `run::run_rep` — and anything else here is a decision invented for the
/// tester, so a variant has to earn its place by answering something the
/// game's own All-Attack cannot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PartyPlan {
    /// `battle_plan_remaining(Attack)`, exactly what pressing `[A]` does.
    #[default]
    AllAttack,
    /// All-Attack, except that a member under
    /// `run::BRACE_BELOW_HP_FRACTION` Defends instead.
    ///
    /// Exists because the party never bracing made Defend invisible to
    /// every arena measurement, and the three targeting features pinned in
    /// `assets/policies/enemy_battle.ron` are pinned on the strength of a
    /// single unit-test census as a result.
    ///
    /// **Not usable for measuring a response to Defend** — see
    /// `BraceInRotation`, which exists because of that.
    BraceWhenHurt,
    /// One slot Defends each round, rotating by round number, whatever
    /// anyone's health.
    ///
    /// An instrument rather than a model of play, and deliberately so.
    /// `BraceWhenHurt` fires on a threshold over `target_hp_frac`, which is
    /// the policy's largest weight — so bracing and being wounded are one
    /// variable (measured r = -0.8) and no reading can attribute anything to
    /// Defend. Rotating by round decorrelates it from health, and rotating
    /// rather than picking a fixed slot decorrelates it from slot position
    /// too, which carries its own aggro weight and would have been the next
    /// confound.
    BraceInRotation,
}

/// Runs `scenario` and reports what happened, plus whatever telemetry
/// `opts` asked for. The engine's whole public arena surface.
///
/// **A fresh `Game` per rep.** One carried over would bring the last
/// fight's dead companions, spent items and XP with it, so rep 2 would
/// measure a different party from rep 1 — and the drift would compound.
/// Warnings are taken from the first rep only: they are identical every
/// rep, and fifty copies of the same line is noise.
///
/// **Telemetry is drained inside the loop and renumbered**, both because of
/// that fresh `Game`: it mints fight ids from 1, so a drain after the loop
/// would see only the last rep's records and an un-renumbered one would
/// give every fight in the set the id 1. A rep is exactly one staged fight
/// — `stage` opens one battle — so the rep index *is* the fight id.
pub fn run(
    scenario: &Scenario,
    assets_dir: &Path,
    opts: RunOptions,
) -> Result<(Report, Vec<Record>), String> {
    let mut warnings = Vec::new();
    let mut reps = Vec::with_capacity(scenario.reps as usize);
    let mut records = Vec::new();
    for rep in 0..scenario.reps {
        let mut staged = stage(
            scenario,
            assets_dir,
            scenario.seed + rep as u64,
            opts.telemetry,
        )?;
        if rep == 0 {
            warnings = staged.warnings.clone();
        }
        reps.push(run::run_rep(
            &mut staged.game,
            &mut staged.watch,
            opts.party,
        ));
        if opts.telemetry {
            let fight = rep as u64 + 1;
            records.extend(
                staged
                    .game
                    .take_battle_telemetry()
                    .into_iter()
                    .map(|mut record| {
                        record.set_fight(fight);
                        record
                    }),
            );
        }
    }
    Ok((
        Report {
            scenario: scenario.clone(),
            warnings,
            reps,
        },
        records,
    ))
}

/// One staged fight, auto-played — the shared fixture for the tests of both
/// halves of the split. Two copies of it would be two answers to "what does
/// the headless path do", which is the thing the split has to preserve.
#[cfg(test)]
pub(crate) fn test_fight(scenario: &Scenario, seed: u64) -> RepRecord {
    let mut staged = stage(
        scenario,
        &crate::tests::support::test_assets_dir(),
        seed,
        false,
    )
    .unwrap();
    run::run_rep(&mut staged.game, &mut staged.watch, PartyPlan::default())
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
                    ..Default::default()
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
        let report = run(
            &a_scenario(3, 40, &[("glitch", 4)]),
            &test_assets_dir(),
            RunOptions::default(),
        )
        .unwrap()
        .0;
        let seeds: Vec<u64> = report.reps.iter().map(|r| r.seed).collect();
        assert_eq!(seeds, vec![40, 41, 42]);
    }

    #[test]
    fn the_same_scenario_run_twice_reports_the_same_thing() {
        // The property the whole tool rests on: without it a tuning
        // comparison measures noise.
        let s = a_scenario(3, 40, &[("glitch", 4)]);
        let a = run(&s, &test_assets_dir(), RunOptions::default())
            .unwrap()
            .0;
        let b = run(&s, &test_assets_dir(), RunOptions::default())
            .unwrap()
            .0;
        assert_eq!(a.reps, b.reps);
    }

    #[test]
    fn staging_leaves_the_fight_open_with_nobody_having_acted() {
        let s = a_scenario(1, 5, &[("glitch", 3)]);
        let staged = stage(&s, &test_assets_dir(), 5, false).unwrap();

        assert!(staged.game.has_active_battle());
        assert_eq!(staged.watch.rounds(), 0);
    }

    #[test]
    fn staging_then_running_matches_run_at_the_same_seed() {
        // The property the whole split rests on: the played fight and the
        // measured one are one code path, so a divergence here means `stage`
        // reordered something the RNG stream sees.
        let s = a_scenario(1, 40, &[("glitch", 4)]);
        let report = run(&s, &test_assets_dir(), RunOptions::default())
            .unwrap()
            .0;

        assert_eq!(report.reps[0], test_fight(&s, 40));
    }

    #[test]
    fn the_seed_varies_the_opponents_it_spawns() {
        // The composition is part of what a rep samples, not a constant it
        // repeats: `spawn_wild_creature_scaled` rolls a `Potential` per
        // member, and a seed installed after the spawn leaves every rep
        // fielding the same six programs.
        let s = Scenario {
            player: PlayerSource::Fresh { level: 6, zone: 4 },
            opponents: vec![OpponentSpec {
                species: "sub_process".into(),
                count: 6,
            }],
            ..Scenario::default()
        };
        let hp = |seed: u64| {
            let staged = stage(&s, &test_assets_dir(), seed, false).unwrap();
            let mut game = staged.game;
            let mut query = game.world.query_filtered::<&Stats, With<Hostile>>();
            query.iter(&game.world).map(|s| s.max_hp).sum::<i32>()
        };

        assert_ne!(hp(1), hp(999));
    }

    fn a_rolled_scenario(reps: u32, seed: u64) -> Scenario {
        Scenario {
            player: PlayerSource::Fresh { level: 10, zone: 3 },
            encounter: Some(Encounter::Stack {
                biome: crate::world::Biome::OpenGrid,
                depth: 3,
            }),
            reps,
            seed,
            ..Scenario::default()
        }
    }

    #[test]
    fn staging_a_rolled_encounter_opens_a_fight_with_no_warnings() {
        let staged = stage(&a_rolled_scenario(1, 4), &test_assets_dir(), 4, false).unwrap();

        assert!(staged.game.has_active_battle());
        assert_eq!(staged.watch.rounds(), 0);
        assert!(staged.warnings.is_empty(), "{:?}", staged.warnings);
    }

    #[test]
    fn staging_then_running_a_rolled_encounter_matches_at_the_same_seed() {
        // The same property `staging_then_running_matches_run_at_the_same_
        // seed` asserts for an authored fight, and it matters more here: the
        // played half and the measured half must roll the *same pack*, not
        // merely the same battle.
        let s = a_rolled_scenario(1, 40);
        let report = run(&s, &test_assets_dir(), RunOptions::default())
            .unwrap()
            .0;

        assert_eq!(report.reps[0], test_fight(&s, 40));
    }

    #[test]
    fn a_lost_stack_fight_is_not_reported_as_a_win() {
        // `Watch` reads "won" off the opponents, and `end_battle` despawns
        // whatever still carries `StackSpawn` — so a swept pack and a wiped
        // one look identical from outside, and a flattened level-1 player
        // read back as having cleared a depth-6 ambush.
        let s = Scenario {
            player: PlayerSource::Fresh { level: 1, zone: 6 },
            encounter: Some(Encounter::Stack {
                biome: crate::world::Biome::NullSector,
                depth: 6,
            }),
            reps: 5,
            seed: 1,
            ..Scenario::default()
        };
        let report = run(&s, &test_assets_dir(), RunOptions::default())
            .unwrap()
            .0;

        assert!(
            report.reps.iter().all(|r| !r.won),
            "a bare level-1 player cleared a depth-6 pack: {:?}",
            report
                .reps
                .iter()
                .map(|r| (r.seed, r.won))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn staging_reports_the_composition_warnings() {
        let s = Scenario {
            player: PlayerSource::Fresh { level: 1, zone: 1 },
            opponents: vec![OpponentSpec {
                species: "glitch".into(),
                count: 9,
            }],
            ..Scenario::default()
        };

        let staged = stage(&s, &test_assets_dir(), 0, false).unwrap();

        assert_eq!(staged.warnings.len(), 1, "{:?}", staged.warnings);
        let w = &staged.warnings[0];
        assert!(w.contains('9'), "the ask: {w}");
        assert!(w.contains('1'), "the ceiling and the zone: {w}");
        assert!(w.contains("glitch"), "which entry: {w}");
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
                ..Default::default()
            }],
            opponents: vec![OpponentSpec {
                species: "sub_process".into(),
                count: 6,
            }],
            reps: 2,
            seed: 11,
            ..Scenario::default()
        };
        let (report, _) = run(&s, &test_assets_dir(), RunOptions::default()).unwrap();

        let swung = |rep: &RepRecord| rep.transcript.iter().any(|l| l.contains("Glitch"));
        assert!(swung(&report.reps[0]), "{:?}", report.reps[0].transcript);
        assert!(
            swung(&report.reps[1]),
            "rep 2 fielded no companion — the `Game` was carried over: {:?}",
            report.reps[1].transcript
        );
    }

    /// A fight the party is losing, so somebody drops under the brace
    /// threshold rather than winning untouched.
    fn a_losing_scenario() -> Scenario {
        Scenario {
            player: PlayerSource::Fresh { level: 3, zone: 2 },
            party: vec![CompanionSpec {
                species: "glitch".into(),
                level: 3,
                ..Default::default()
            }],
            opponents: vec![OpponentSpec {
                species: "sub_process".into(),
                count: 4,
            }],
            reps: 6,
            seed: 31,
            ..Scenario::default()
        }
    }

    fn defends(records: &[Record]) -> usize {
        records
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    Record::PartyAction {
                        kind: crate::telemetry::ActionKind::Defend,
                        ..
                    }
                )
            })
            .count()
    }

    fn bracing_records(scenario: &Scenario, party: PartyPlan) -> Vec<Record> {
        run(
            scenario,
            &test_assets_dir(),
            RunOptions {
                telemetry: true,
                party,
            },
        )
        .unwrap()
        .1
    }

    /// The guarantee `run_rep`'s doc has always made, now that a mode exists
    /// which breaks it: by default the arena invents no decision the game
    /// would not make, and `[A]` never braces.
    #[test]
    fn the_default_party_plan_never_braces() {
        let records = bracing_records(&a_losing_scenario(), PartyPlan::default());
        assert_eq!(
            defends(&records),
            0,
            "All-Attack braced — the default is no longer the game's own plan"
        );
    }

    #[test]
    fn the_bracing_plan_defends_when_a_member_is_hurt() {
        let records = bracing_records(&a_losing_scenario(), PartyPlan::BraceWhenHurt);
        assert!(
            defends(&records) > 0,
            "nobody braced in a fight the party is losing"
        );
    }

    /// The payoff, and why the plan exists at all: with a party that braces,
    /// `target_bracing` stops being false on every swing ever recorded, so
    /// the Defend question becomes answerable from telemetry rather than
    /// from one unit-test census.
    #[test]
    fn a_bracing_party_is_visible_to_the_enemy() {
        let records = bracing_records(&a_losing_scenario(), PartyPlan::BraceWhenHurt);
        let at_bracing = records
            .iter()
            .filter(|r| {
                matches!(
                    r,
                    Record::EnemyChoice {
                        target_bracing: true,
                        ..
                    }
                )
            })
            .count();
        assert!(
            at_bracing > 0,
            "no swing landed on a bracing target — the harness still cannot see Defend"
        );
    }

    /// A fight the party wins without being hurt, so `BraceWhenHurt` never
    /// fires and any Defend seen came from the rotation instead.
    fn a_walkover_scenario() -> Scenario {
        Scenario {
            player: PlayerSource::Fresh { level: 20, zone: 1 },
            party: vec![CompanionSpec {
                species: "glitch".into(),
                level: 12,
                ..Default::default()
            }],
            opponents: vec![OpponentSpec {
                species: "sprite".into(),
                count: 3,
            }],
            reps: 4,
            seed: 77,
            ..Scenario::default()
        }
    }

    /// The whole point of the rotation, and the fix for the 2026-08-10 run
    /// that could not answer its own question: bracing has to vary
    /// independently of health, or it is just a restatement of
    /// `target_hp_frac` and the policy's response to the two cannot be told
    /// apart.
    #[test]
    fn the_rotating_plan_braces_at_full_health() {
        let healthy = a_walkover_scenario();
        assert_eq!(
            defends(&bracing_records(&healthy, PartyPlan::BraceWhenHurt)),
            0,
            "the fixture is meant to be a walkover — nobody should drop under half HP"
        );
        assert!(
            defends(&bracing_records(&healthy, PartyPlan::BraceInRotation)) > 0,
            "the rotation braced nobody in a fight where nobody was hurt"
        );
    }

    fn two_rep_scenario() -> Scenario {
        Scenario {
            player: PlayerSource::Fresh { level: 8, zone: 2 },
            opponents: vec![OpponentSpec {
                species: "sub_process".into(),
                count: 2,
            }],
            reps: 2,
            seed: 21,
            ..Scenario::default()
        }
    }

    #[test]
    fn a_run_collects_nothing_unless_asked() {
        let (_, records) = run(
            &two_rep_scenario(),
            &test_assets_dir(),
            RunOptions::default(),
        )
        .unwrap();
        assert!(
            records.is_empty(),
            "the default must stay free: {} records",
            records.len()
        );
    }

    /// The regression that matters, and it has two halves. A fresh `Game`
    /// per rep means each rep mints its own ids from 1, so an implementation
    /// that drains without renumbering hands every fight in a 200-rep
    /// evaluation the id `1` — and one that drains after the loop instead of
    /// inside it sees only the last rep's `Game` and loses the rest outright.
    #[test]
    fn every_rep_in_a_run_gets_its_own_fight_id() {
        let (_, records) = run(
            &two_rep_scenario(),
            &test_assets_dir(),
            RunOptions {
                telemetry: true,
                ..RunOptions::default()
            },
        )
        .unwrap();

        let starts: Vec<u64> = records
            .iter()
            .filter_map(|r| match r {
                Record::FightStart { fight, .. } => Some(*fight),
                _ => None,
            })
            .collect();
        assert_eq!(starts.len(), 2, "one per rep, got {starts:?}");
        assert_ne!(
            starts[0], starts[1],
            "both reps minted id {} — the drain did not renumber",
            starts[0]
        );

        // Every record, not just the openers: a renumber that touched only
        // `FightStart` would leave the swings pointing at the wrong fight.
        // No wildcard, matching `Record::set_fight`: a new variant must be
        // classified as one that carries a fight or one that does not,
        // rather than silently joining the set this asserts over. The base
        // records are keyed to a tick and cannot appear in an arena session
        // anyway, since nothing there runs a machine.
        fn fight_of(record: &Record) -> Option<u64> {
            match record {
                Record::FightStart { fight, .. }
                | Record::Round { fight, .. }
                | Record::EnemyChoice { fight, .. }
                | Record::PartyAction { fight, .. }
                | Record::FightEnd { fight, .. } => Some(*fight),
                Record::Extract { .. }
                | Record::Assemble { .. }
                | Record::MachineStall { .. }
                | Record::HandCraft { .. }
                | Record::Acquire { .. }
                | Record::Consume { .. } => None,
            }
        }
        let ids: std::collections::BTreeSet<u64> = records.iter().filter_map(fight_of).collect();
        assert_eq!(
            ids,
            starts.iter().copied().collect(),
            "some records carry an id no fight opened with"
        );
    }
}
