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
use bevy_ecs::prelude::Entity;

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

/// Board helpers for the movement tests: a 16 by 16 open field, the player
/// far to the east, and the pack parked where the test says.
mod field {
    use super::*;
    use crate::components::Stats;
    use crate::tactical::map::Board;
    use crate::tactical::reach;

    pub fn open_field(game: &mut Game, cover: &[(usize, usize)]) {
        let mut rows = vec![".".repeat(16); 16];
        for &(x, y) in cover {
            rows[y].replace_range(x..=x, "#");
        }
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        game.world.resource_mut::<TacticalBattle>().board = Board::from_rows(&rows);
        let player = game.player_entity();
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.hp = 10_000;
        stats.max_hp = 10_000;
    }

    pub fn put(game: &mut Game, body: Entity, cell: (i32, i32)) {
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(body, cell),
            "{cell:?} would not take a body"
        );
    }

    pub fn cell_of(game: &Game, body: Entity) -> (i32, i32) {
        game.world
            .resource::<TacticalBattle>()
            .cell_of(body)
            .expect("on the board")
    }

    /// `count` swing-only hostiles parked out of the way, then stood on
    /// `cells`, with the player at `player_at`.
    pub fn stand(
        game: &mut Game,
        count: usize,
        player_at: (i32, i32),
        cells: &[(i32, i32)],
    ) -> Vec<Entity> {
        let pack = tactical_fight(game, count, 10_000);
        for &body in &pack {
            game.world.entity_mut(body).insert(Routines::default());
        }
        open_field(game, &[]);
        for (i, &body) in pack.iter().enumerate() {
            put(game, body, (i as i32, 15));
        }
        let player = game.player_entity();
        put(game, player, player_at);
        for (&body, &cell) in pack.iter().zip(cells) {
            put(game, body, cell);
        }
        pack
    }

    pub fn step_distance(a: (i32, i32), b: (i32, i32)) -> u32 {
        reach::distance(a, b)
    }
}

mod together {
    use super::field::*;
    use super::*;
    use crate::tuning::TACTICAL_COHESION_SLACK;

    /// Where the first hostile ends its turn, with its packmate left behind
    /// at the west edge and the player far to the east.
    fn runner_ends_at(tactic: PackTactic) -> ((i32, i32), (i32, i32)) {
        let mut game = game();
        let pack = stand(&mut game, 2, (14, 8), &[(2, 8), (0, 8)]);
        game.set_pack_tactic(tactic);
        assert!(wait_for_turn(&mut game, pack[0]));
        assert!(game.tactical_ai_turn_at(0.0));
        (cell_of(&game, pack[0]), cell_of(&game, pack[1]))
    }

    #[test]
    fn a_body_that_would_outrun_its_packmate_stops_within_the_slack() {
        let (rush_runner, rush_mate) = runner_ends_at(PackTactic::Rush);
        assert!(
            step_distance(rush_runner, rush_mate) > TACTICAL_COHESION_SLACK,
            "the control did not outrun the pack: {rush_runner:?} vs {rush_mate:?}"
        );
        let (runner, mate) = runner_ends_at(PackTactic::Together);
        assert!(
            step_distance(runner, mate) <= TACTICAL_COHESION_SLACK,
            "outran the pack: {runner:?} vs {mate:?}"
        );
    }

    #[test]
    fn a_lone_body_under_together_closes_as_it_always_did() {
        let end = |tactic| {
            let mut game = game();
            let pack = stand(&mut game, 1, (14, 8), &[(2, 8)]);
            game.set_pack_tactic(tactic);
            assert!(wait_for_turn(&mut game, pack[0]));
            assert!(game.tactical_ai_turn_at(0.0));
            cell_of(&game, pack[0])
        };
        assert_eq!(end(PackTactic::Together), end(PackTactic::Rush));
    }
}

mod hold {
    use super::field::*;
    use super::*;
    use crate::components::Stats;
    use crate::tactical::reach;
    use crate::tuning::TACTICAL_HOLD_ROUNDS;

    const BREAKS: &str = "The pack breaks cover!";

    /// A holding pair on the west side, a boulder at (6,7) so the cell (5,8)
    /// is covered from a player standing due east on row 8.
    fn holding_pair(player_at: (i32, i32)) -> (Game, Vec<Entity>) {
        let mut game = game();
        let pack = stand(&mut game, 2, player_at, &[(2, 8), (1, 9)]);
        open_field_cover(&mut game);
        game.set_pack_tactic(PackTactic::Hold);
        (game, pack)
    }

    /// `open_field` again with the boulder in, bodies left where they stand.
    fn open_field_cover(game: &mut Game) {
        use crate::tactical::map::BattleCell;
        game.world
            .resource_mut::<TacticalBattle>()
            .board
            .put(6, 7, BattleCell::Cover);
    }

    fn take_turn(game: &mut Game, body: Entity) {
        assert!(wait_for_turn(game, body));
        assert!(game.tactical_ai_turn_at(0.0));
    }

    fn breaks_logged(game: &Game) -> usize {
        super::super::tactical::log_texts(game)
            .iter()
            .filter(|l| l.as_str() == BREAKS)
            .count()
    }

    fn covered(game: &Game, body: Entity, from: (i32, i32)) -> bool {
        let board = &game.world.resource::<TacticalBattle>().board;
        reach::cover_between(board, from, cell_of(game, body))
    }

    #[test]
    fn a_holding_body_walks_to_cover_and_then_stays_while_the_player_is_out_of_sight() {
        let player = (14, 8);
        let (mut game, pack) = holding_pair(player);
        take_turn(&mut game, pack[0]);
        assert!(covered(&game, pack[0], player), "took no cover");
        let parked = cell_of(&game, pack[0]);
        take_turn(&mut game, pack[0]);
        assert_eq!(cell_of(&game, pack[0]), parked, "left its cover");
        assert_eq!(tactic_of(&game), PackTactic::Hold);
        assert_eq!(breaks_logged(&game), 0);
    }

    #[test]
    fn hold_breaks_when_the_player_comes_into_sight() {
        // Six cells due east: inside the alert range and in the open.
        let (mut game, pack) = holding_pair((8, 8));
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Rush);
        assert_eq!(breaks_logged(&game), 1);
        // And the body whose turn it was planned under Rush: it closed.
        assert!(cell_of(&game, pack[0]).0 > 2, "did not advance that turn");
    }

    #[test]
    fn a_player_out_of_range_does_not_break_the_hold() {
        // Seven cells off: clear sight, but past the alert range.
        let (mut game, pack) = holding_pair((9, 8));
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Hold);
    }

    #[test]
    fn a_player_behind_cover_does_not_break_the_hold() {
        // A wall of boulders between them: in range, out of sight.
        let (mut game, pack) = holding_pair((8, 8));
        {
            use crate::tactical::map::BattleCell;
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            for y in 5..12 {
                battle.board.put(5, y, BattleCell::Cover);
            }
        }
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Hold);
    }

    #[test]
    fn hold_breaks_when_a_member_is_hit() {
        let (mut game, pack) = holding_pair((14, 8));
        game.world.get_mut::<Stats>(pack[1]).unwrap().hp -= 1;
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Rush);
        assert_eq!(breaks_logged(&game), 1);
    }

    #[test]
    fn hold_breaks_when_a_member_is_gone() {
        let (mut game, pack) = holding_pair((14, 8));
        game.world
            .resource_mut::<TacticalBattle>()
            .remove(pack[1]);
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Rush);
        assert_eq!(breaks_logged(&game), 1);
    }

    #[test]
    fn hold_breaks_on_the_timeout_round() {
        let (mut game, pack) = holding_pair((14, 8));
        let start = game.world.resource::<TacticalBattle>().round;
        game.world.resource_mut::<TacticalBattle>().round = start + TACTICAL_HOLD_ROUNDS - 2;
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Hold, "a round early");
        game.world.resource_mut::<TacticalBattle>().round = start + TACTICAL_HOLD_ROUNDS;
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Rush);
        assert_eq!(breaks_logged(&game), 1);
    }

    #[test]
    fn a_broken_hold_logs_once_and_stays_rush() {
        let (mut game, pack) = holding_pair((8, 8));
        take_turn(&mut game, pack[0]);
        take_turn(&mut game, pack[1]);
        take_turn(&mut game, pack[0]);
        assert_eq!(breaks_logged(&game), 1);
        assert_eq!(tactic_of(&game), PackTactic::Rush);
    }
}
