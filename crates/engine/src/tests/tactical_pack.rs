//! Pack tactics: the hostile side of a battle-map fight rolls one tactic
//! that adds pack-aware terms on top of each body's personality.
//!
//! Every fixture sets its tactic directly through `Game::set_pack_tactic`
//! rather than leaning on the roll, so a test names the tactic it is about.

use super::support::test_assets_dir;
use super::tactical::{only_routine, open_ground, tactical_fight, wait_for_turn};
use crate::Game;
use crate::abilities::TamperKind;
use crate::behaviours::BehaviourDb;
use crate::components::{Routines, Tampered};
use crate::resources::{Behaviours, DifficultyMode};
use crate::tactical::ai::ForecastAction;
use crate::tactical::pack::PackTactic;
use crate::tactical::TacticalBattle;

fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn tactic_of(game: &Game) -> PackTactic {
    game.world.resource::<TacticalBattle>().pack.tactic
}

#[test]
fn a_hand_built_battle_is_rush() {
    use crate::tactical::map::{BattleSpec, Board};
    use crate::world::Biome;
    let spec = BattleSpec {
        world_seed: 1,
        site: (0, 0),
        tick: 0,
        zone: 1,
        biome: Biome::DataVoid,
        bodies: 2,
    };
    let battle = TacticalBattle::open(spec, Board::from_rows(&["..", ".."]));
    assert_eq!(battle.pack.tactic, PackTactic::Rush);
}

/// `every_shipped_profile_forecasts_the_action_its_turn_takes`, looped over
/// the tactics too: a forecast is a call into the planner the turn runs, so
/// a tactic's terms must reach both.
#[test]
fn every_shipped_profile_forecasts_the_action_its_turn_takes_under_every_tactic() {
    let (db, _) = BehaviourDb::load_dir(&test_assets_dir().join("behaviours")).unwrap();
    let ids: Vec<String> = db.ids().map(str::to_string).collect();
    assert!(!ids.is_empty());
    for tactic in [PackTactic::Rush, PackTactic::Together, PackTactic::Hold] {
        for id in &ids {
            for routine in [None, Some("acid_wash")] {
                let mut game = game();
                let pack = tactical_fight(&mut game, 2, 10_000);
                let wild = pack[0];
                game.world.entity_mut(pack[1]).insert(Routines::default());
                game.world.entity_mut(wild).insert(Routines::default());
                open_ground(&mut game, &pack, &[(3, 4), (3, 3)]);
                game.set_pack_tactic(tactic);
                game.world
                    .resource_mut::<Behaviours>()
                    .0
                    .set_override(Some(id.clone()));
                if let Some(r) = routine {
                    only_routine(&mut game, wild, r);
                }
                let mut tampered = Tampered::default();
                tampered.apply(TamperKind::Profiled, 5, false);
                game.world.entity_mut(wild).insert(tampered);
                assert!(wait_for_turn(&mut game, wild));
                let forecast = game.tactical_forecast(wild).expect("a profiled hostile");
                game.tactical_ai_turn();
                let fired = game
                    .world
                    .get::<crate::components::AbilityCooldowns>(wild)
                    .is_some_and(|c| c.0.contains_key("acid_wash"));
                let expected = if fired {
                    ForecastAction::Routine("acid_wash".into())
                } else {
                    ForecastAction::Swing
                };
                assert_eq!(
                    forecast.action, expected,
                    "{tactic:?}, profile {id:?}, routine {routine:?}"
                );
            }
        }
    }
}

mod roll {
    use super::*;
    use bevy_ecs::prelude::Entity;
    use crate::components::Stats;
    use crate::resources::GameRng;
    use crate::tactical::map::BattleSpec;
    use crate::tactical::pack;
    use crate::world::Biome;
    use rand::RngExt;

    fn spec(tick: u64) -> BattleSpec {
        BattleSpec {
            world_seed: 7,
            site: (3, -2),
            tick,
            zone: 1,
            biome: Biome::DataVoid,
            bodies: 4,
        }
    }

    /// A game with `count` hostiles seated, to roll for.
    fn pack_of(count: usize) -> (Game, Vec<Entity>) {
        let mut game = game();
        let pack = tactical_fight(&mut game, count, 100);
        (game, pack)
    }

    #[test]
    fn the_same_spec_and_pack_give_the_same_tactic() {
        let (game, pack) = pack_of(3);
        for tick in 0..40 {
            assert_eq!(
                pack::roll(spec(tick), &pack, &game),
                pack::roll(spec(tick), &pack, &game),
                "tick {tick}"
            );
        }
    }

    #[test]
    fn the_roll_spans_every_tactic_across_specs() {
        let (game, pack) = pack_of(3);
        let seen: std::collections::HashSet<PackTactic> = (0..400)
            .map(|tick| pack::roll(spec(tick), &pack, &game))
            .collect();
        for tactic in [PackTactic::Rush, PackTactic::Together, PackTactic::Hold] {
            assert!(seen.contains(&tactic), "{tactic:?} was never rolled");
        }
    }

    #[test]
    fn rolling_draws_nothing_from_the_game_rng() {
        // `StdRng` cannot be cloned, so the stream position is read off a twin
        // game that never rolled.
        let (mut rolled, pack) = pack_of(3);
        let (mut untouched, _) = pack_of(3);
        for tick in 0..50 {
            pack::roll(spec(tick), &pack, &rolled);
        }
        let next = |game: &mut Game| game.world.resource_mut::<GameRng>().0.random::<u64>();
        assert_eq!(next(&mut rolled), next(&mut untouched));
    }

    #[test]
    fn a_lone_body_is_never_rolled_together() {
        let (game, pack) = pack_of(1);
        for tick in 0..400 {
            assert_ne!(
                pack::roll(spec(tick), &pack, &game),
                PackTactic::Together,
                "tick {tick}"
            );
        }
    }

    #[test]
    fn the_pack_is_profiled_by_its_toughest_body_and_ties_go_to_the_first() {
        let (mut game, pack) = pack_of(3);
        game.world.get_mut::<Stats>(pack[2]).unwrap().max_hp = 500;
        assert_eq!(pack::lead(&game, &pack), Some(pack[2]));
        game.world.get_mut::<Stats>(pack[2]).unwrap().max_hp = 100;
        assert_eq!(pack::lead(&game, &pack), Some(pack[0]));
        let reversed: Vec<Entity> = pack.iter().rev().copied().collect();
        assert_eq!(pack::lead(&game, &reversed), Some(pack[2]));
    }

    #[test]
    fn an_unlisted_profile_rolls_on_the_default_row() {
        assert_eq!(pack::odds_row("no_such_profile"), pack::odds_row("default"));
        assert_ne!(pack::odds_row("brute"), pack::odds_row("default"));
    }

    #[test]
    fn rush_has_no_opening_line_and_the_rest_do() {
        assert_eq!(pack::opening_line(PackTactic::Rush), None);
        assert!(pack::opening_line(PackTactic::Together).is_some());
        assert!(pack::opening_line(PackTactic::Hold).is_some());
    }

    #[test]
    fn a_siege_pack_is_rush() {
        use super::super::support::{set_zone, stand_in_base};
        let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.lay_starting_pocket();
        stand_in_base(&mut game);
        set_zone(&mut game, 2);
        assert!(game.open_siege());
        assert_eq!(tactic_of(&game), PackTactic::Rush);
    }
}

mod arena_override {
    use super::*;
    use crate::arena::{CombatModel, Scenario, stage};
    use crate::tactical::pack;

    fn scenario(extra: &str) -> Result<Scenario, String> {
        Scenario::from_ron(&format!(
            r#"(opponents: [(species: "glitch", count: 3)], model: Tactical, {extra})"#
        ))
    }

    fn staged_tactic(extra: &str, seed: u64) -> (PackTactic, Vec<String>) {
        let s = scenario(extra).unwrap();
        let staged = stage(&s, &test_assets_dir(), seed, false, CombatModel::Tactical).unwrap();
        let log = super::super::tactical::log_texts(&staged.game);
        (tactic_of(&staged.game), log)
    }

    #[test]
    fn a_scenario_tactic_wins_over_the_roll_and_is_announced() {
        // Seeds vary the staged fight's own spec, so a roll that was merely
        // lucky would show up as a mismatch on one of them.
        for seed in 0..12 {
            for forced in [PackTactic::Together, PackTactic::Hold] {
                let (tactic, log) = staged_tactic(&format!("tactic: Some({forced:?})"), seed);
                assert_eq!(tactic, forced, "seed {seed}");
                let line = pack::opening_line(forced).unwrap();
                assert!(log.iter().any(|l| l == line), "seed {seed}: {log:?}");
            }
            let (tactic, log) = staged_tactic("tactic: Some(Rush)", seed);
            assert_eq!(tactic, PackTactic::Rush, "seed {seed}");
            assert!(!log.iter().any(|l| l.starts_with("The pack")), "{log:?}");
        }
    }

    #[test]
    fn an_unknown_tactic_name_fails_to_parse() {
        let err = scenario("tactic: Some(Sneak)").unwrap_err();
        assert!(err.contains("Sneak"), "{err}");
    }

    #[test]
    fn a_tactic_on_a_group_scenario_is_refused() {
        let err = Scenario::from_ron(
            r#"(opponents: [(species: "glitch", count: 1)], tactic: Some(Hold))"#,
        )
        .unwrap_err();
        assert!(err.contains("Tactical"), "{err}");
    }
}
