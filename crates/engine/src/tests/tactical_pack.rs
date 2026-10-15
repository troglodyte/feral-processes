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
use crate::tactical::TacticalBattle;
use crate::tactical::ai::ForecastAction;
use crate::tactical::pack;
use crate::tactical::pack::PackTactic;
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
    fn the_roll_varies_across_world_seed_and_across_site_at_a_fixed_tick() {
        let (game, pack) = pack_of(3);
        let across = |vary: &dyn Fn(u32) -> BattleSpec| -> usize {
            (0..300)
                .map(|i| pack::roll(vary(i), &pack, &game))
                .collect::<std::collections::HashSet<_>>()
                .len()
        };
        let seeds = across(&|i| BattleSpec {
            world_seed: i,
            ..spec(9)
        });
        let sites = across(&|i| BattleSpec {
            site: (i as i32, 0),
            ..spec(9)
        });
        let rows = across(&|i| BattleSpec {
            site: (0, i as i32),
            ..spec(9)
        });
        assert!(seeds > 1, "world_seed is not in the fold");
        assert!(sites > 1, "site.0 is not in the fold");
        assert!(rows > 1, "site.1 is not in the fold");
    }

    #[test]
    fn the_roll_seed_carries_its_own_salt() {
        use crate::derive::{FNV_BASIS, fold};
        let s = spec(9);
        let saltless = fold(
            FNV_BASIS,
            &[
                s.world_seed as u64,
                s.site.0 as u32 as u64,
                s.site.1 as u32 as u64,
                s.tick,
            ],
        );
        assert_ne!(pack::roll_seed(s), saltless);
    }

    /// Frequencies of each tactic over many ticks for a pack of `species`.
    fn frequencies(species: &str, profile: Option<&str>) -> [f32; 3] {
        let (mut game, pack) = pack_of(3);
        for &body in &pack {
            game.world
                .get_mut::<crate::components::Creature>(body)
                .unwrap()
                .species = species.to_string();
        }
        if let Some(id) = profile {
            game.world
                .resource_mut::<crate::resources::Behaviours>()
                .0
                .set_override(Some(id.to_string()));
        }
        let n = 3000;
        let mut counts = [0u32; 3];
        for tick in 0..n {
            let at = PackTactic::ALL
                .iter()
                .position(|t| *t == pack::roll(spec(tick), &pack, &game))
                .unwrap();
            counts[at] += 1;
        }
        counts.map(|c| c as f32 / n as f32)
    }

    #[test]
    fn a_brute_led_pack_follows_the_brute_row_and_not_the_default_one() {
        let brute = frequencies("rootkit", None);
        let default = frequencies("construct", Some("default"));
        let total = |row: [u32; 3]| row.iter().sum::<u32>() as f32;
        for (i, (&got, &weight)) in brute.iter().zip(&pack::odds_row("brute")).enumerate() {
            let want = weight as f32 / total(pack::odds_row("brute"));
            assert!((got - want).abs() < 0.05, "column {i}: {got} vs {want}");
        }
        for (i, (&got, &weight)) in default.iter().zip(&pack::odds_row("default")).enumerate() {
            let want = weight as f32 / total(pack::odds_row("default"));
            assert!((got - want).abs() < 0.05, "column {i}: {got} vs {want}");
        }
        assert!(
            brute[2] + 0.1 < default[2],
            "a brute holds as often as default"
        );
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
    fn a_scenario_tactic_is_seated_and_announced() {
        for forced in [PackTactic::Together, PackTactic::Hold] {
            let (tactic, log) = staged_tactic(&format!("tactic: Some({forced:?})"), 5);
            assert_eq!(tactic, forced);
            let line = pack::opening_line(forced).unwrap();
            assert!(log.iter().any(|l| l == line), "{log:?}");
        }
        let (tactic, log) = staged_tactic("tactic: Some(Rush)", 5);
        assert_eq!(tactic, PackTactic::Rush);
        assert!(!log.iter().any(|l| l.starts_with("The pack")), "{log:?}");
    }

    /// Every rep of a scenario shares one `BattleSpec`, so a rolled tactic
    /// would be one fixed tactic for the whole run and would shift every
    /// existing baseline.
    #[test]
    fn an_absent_tactic_is_rush_whatever_the_seed() {
        for seed in 0..12 {
            let (tactic, log) = staged_tactic("", seed);
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

    /// Plays the pack as a melee species, so the only reason it has to look
    /// for cover is the tactic's own: a ranged body already walks for cover on
    /// its profile's weight and would hide what the Hold bonus is worth.
    pub fn melee(game: &mut Game, pack: &[Entity]) {
        let ids: Vec<String> = game
            .species_defs()
            .into_iter()
            .filter(|d| !d.is_hybrid())
            .map(|d| d.id.clone())
            .collect();
        for &body in pack {
            let found = ids.iter().find(|id| {
                game.world
                    .get_mut::<crate::components::Creature>(body)
                    .unwrap()
                    .species = (*id).clone();
                game.swing_range(body) == 1
            });
            assert!(found.is_some(), "no melee species ships");
        }
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
        let pack = stand(&mut game, 2, player_at, &[(4, 8), (3, 9)]);
        melee(&mut game, &pack);
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
    fn a_holding_body_does_not_close_on_a_distant_player() {
        let moved = |tactic| {
            let mut game = game();
            let pack = stand(&mut game, 2, (14, 8), &[(2, 8), (1, 9)]);
            melee(&mut game, &pack);
            game.set_pack_tactic(tactic);
            take_turn(&mut game, pack[0]);
            cell_of(&game, pack[0]) != (2, 8)
        };
        assert!(moved(PackTactic::Rush), "the control did not close");
        assert!(!moved(PackTactic::Hold));
    }

    #[test]
    fn the_planner_reads_the_tactic_a_pending_trigger_would_leave() {
        // The forecast cannot write, so it must plan under what the turn will:
        // a hold that is about to break already closes at full scale.
        let quiet = holding_pair((14, 8));
        let sighted = holding_pair((8, 8));
        assert_eq!(pack::plan(&quiet.0, quiet.1[0], 1.0).closing_scale(), 0.0);
        assert_eq!(
            pack::plan(&sighted.0, sighted.1[0], 1.0).closing_scale(),
            1.0
        );
        assert_eq!(tactic_of(&sighted.0), PackTactic::Hold, "the read wrote");
    }

    #[test]
    fn hold_breaks_when_the_player_comes_into_sight() {
        // Four cells due east: inside the alert range and in the open.
        let (mut game, pack) = holding_pair((8, 8));
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Rush);
        assert_eq!(breaks_logged(&game), 1);
        // And the body whose turn it was planned under Rush: it closed.
        assert!(cell_of(&game, pack[0]).0 > 4, "did not advance that turn");
    }

    #[test]
    fn a_player_out_of_range_does_not_break_the_hold() {
        // Seven cells off: clear sight, but past the alert range.
        let (mut game, pack) = holding_pair((11, 8));
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Hold);
        // The stored field alone proves little: the planner must still be
        // holding too.
        assert_eq!(pack::plan(&game, pack[1], 1.0).closing_scale(), 0.0);
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
        game.world.resource_mut::<TacticalBattle>().remove(pack[1]);
        take_turn(&mut game, pack[0]);
        assert_eq!(tactic_of(&game), PackTactic::Rush);
        assert_eq!(breaks_logged(&game), 1);
    }

    #[test]
    fn hold_breaks_on_exactly_the_timeout_round() {
        let (mut game, _pack) = holding_pair((14, 8));
        let start = game.world.resource::<TacticalBattle>().round;
        // Set and checked directly, so no turn wrap moves the round between.
        game.world.resource_mut::<TacticalBattle>().round = start + TACTICAL_HOLD_ROUNDS - 1;
        assert_eq!(pack::check_transition(&mut game), None, "a round early");
        assert_eq!(tactic_of(&game), PackTactic::Hold);
        game.world.resource_mut::<TacticalBattle>().round = start + TACTICAL_HOLD_ROUNDS;
        assert_eq!(pack::check_transition(&mut game), Some(BREAKS));
        assert_eq!(tactic_of(&game), PackTactic::Rush);
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

/// Fights opened the way the game opens them, not hand-placed boards.
mod real_fights {
    use super::*;

    /// A Hold in a real fight must outlast two full rounds in most seeds, with
    /// every hostile turn played through. The sides open
    /// `TACTICAL_DEPLOY_GAP` apart, so an alert range at that gap sees the
    /// party at once, and a hold that walks to the nearest cover walks into
    /// the range its own trigger watches.
    #[test]
    fn a_hold_survives_two_full_rounds_in_most_real_fights() {
        use crate::components::Hostile;
        let seeds = 1..41u32;
        let total = seeds.len();
        let mut held = 0;
        for seed in seeds {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            tactical_fight(&mut game, 3, 100);
            game.set_pack_tactic(PackTactic::Hold);
            for _ in 0..200 {
                let Some(actor) = game.tactical_actor() else {
                    break;
                };
                if game.world.resource::<TacticalBattle>().round >= 3 {
                    break;
                }
                if game.world.get::<Hostile>(actor).is_some() {
                    assert!(game.tactical_ai_turn_at(0.0));
                } else {
                    game.tactical_end_turn();
                }
            }
            if game.has_active_battle() && tactic_of(&game) == PackTactic::Hold {
                held += 1;
            }
        }
        assert!(held * 4 >= total * 3, "only {held} of {total} holds held");
    }

    #[test]
    fn an_unforced_real_fight_can_seat_a_tactic_other_than_rush() {
        let seated: std::collections::HashSet<PackTactic> = (1..41u32)
            .map(|seed| {
                let mut game =
                    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
                let pack = super::super::tactical::tactical_pack(&mut game, 3, 100);
                game.open_tactical_battle(pack);
                tactic_of(&game)
            })
            .collect();
        assert!(seated.len() > 1, "every fight rolled {seated:?}");
    }
}

mod rush_and_forecast {
    use super::field::*;
    use super::*;
    use crate::components::{Creature, Stats, Tampered};
    use crate::tactical::map::Board;
    use crate::tactical::reach;

    /// `tactical_policy`'s `a_brute_walks_past_cover_and_a_skirmisher_walks_to_it`
    /// under an explicit `Rush`: a fight that rolled nothing decides as it
    /// always has.
    fn ranged_stand(species: &str) -> bool {
        let mut game = game();
        let wild = tactical_fight(&mut game, 1, 10_000)[0];
        game.world.get_mut::<Creature>(wild).unwrap().species = species.to_string();
        game.world.entity_mut(wild).insert(Routines::default());
        assert!(game.swing_range(wild) > 1, "{species} is not ranged");
        game.set_pack_tactic(PackTactic::Rush);
        let mut rows = vec![".".repeat(12); 12];
        rows[3].replace_range(7..=7, "#");
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        game.world.resource_mut::<TacticalBattle>().board = Board::from_rows(&rows);
        let player = game.player_entity();
        game.world.get_mut::<Stats>(player).unwrap().hp = 10_000;
        game.world.get_mut::<Stats>(player).unwrap().max_hp = 10_000;
        put(&mut game, player, (8, 4));
        put(&mut game, wild, (5, 4));
        assert!(wait_for_turn(&mut game, wild));
        assert!(game.tactical_ai_turn_at(0.0));
        let board = &game.world.resource::<TacticalBattle>().board;
        reach::cover_between(board, (8, 4), cell_of(&game, wild))
    }

    #[test]
    fn a_brute_walks_past_cover_and_a_skirmisher_walks_to_it_under_explicit_rush() {
        assert!(ranged_stand("glitch"), "the skirmisher took no cover");
        assert!(!ranged_stand("rootkit"), "the brute stopped for cover");
    }

    /// The forecast's destination against the cell the turn then takes, at
    /// temperature zero and with the player out of alert range, so Hold's and
    /// Together's terms are live rather than a hold about to break.
    fn forecast_and_turn(
        tactic: PackTactic,
        profile: &str,
        cells: &[(i32, i32)],
        cover: bool,
    ) -> ((i32, i32), (i32, i32), (i32, i32)) {
        let mut game = game();
        let pack = stand(&mut game, cells.len(), (14, 8), cells);
        melee(&mut game, &pack);
        if cover {
            game.world.resource_mut::<TacticalBattle>().board.put(
                6,
                7,
                crate::tactical::map::BattleCell::Cover,
            );
        }
        game.set_pack_tactic(tactic);
        game.world
            .resource_mut::<Behaviours>()
            .0
            .set_override(Some(profile.to_string()));
        let wild = pack[0];
        let mut tampered = Tampered::default();
        tampered.apply(TamperKind::Profiled, 9, false);
        tampered.apply(TamperKind::Temperature(0.0), 9, false);
        game.world.entity_mut(wild).insert(tampered);
        assert!(wait_for_turn(&mut game, wild));
        let from = cell_of(&game, wild);
        let forecast = game.tactical_forecast(wild).expect("a profiled hostile");
        let predicted = forecast.walk.last().copied().unwrap_or(from);
        game.tactical_ai_turn();
        (from, predicted, cell_of(&game, wild))
    }

    #[test]
    fn the_forecast_walk_ends_where_the_turn_does_under_every_tactic_and_profile() {
        let (db, _) = BehaviourDb::load_dir(&test_assets_dir().join("behaviours")).unwrap();
        let ids: Vec<String> = db.ids().map(str::to_string).collect();
        let mut moved = std::collections::HashSet::new();
        for id in &ids {
            for (tactic, cells, cover) in [
                (PackTactic::Rush, vec![(2, 8), (1, 9)], false),
                (PackTactic::Together, vec![(3, 8), (0, 8)], false),
                (PackTactic::Hold, vec![(4, 8), (3, 9)], true),
            ] {
                let (from, predicted, took) = forecast_and_turn(tactic, id, &cells, cover);
                assert_eq!(predicted, took, "{tactic:?}, profile {id:?}");
                if took != from {
                    moved.insert(tactic);
                }
            }
        }
        // Without this the loop could pass with every body standing still.
        for tactic in [PackTactic::Rush, PackTactic::Together, PackTactic::Hold] {
            assert!(moved.contains(&tactic), "{tactic:?} never moved a body");
        }
    }

    /// The runner at the pack's front and its slow packmate behind; where the
    /// runner ends its turn, with `weights` as its profile.
    fn runner_under(tactic: PackTactic, closing: f32) -> u32 {
        let mut game = game();
        let pack = stand(&mut game, 2, (14, 8), &[(2, 8), (0, 8)]);
        let mut def = crate::behaviours::BehaviourDef {
            id: "test".into(),
            name: "Test".into(),
            ..Default::default()
        };
        def.cells.closing = closing;
        {
            let mut db = game.world.resource_mut::<Behaviours>();
            db.0.insert(def);
            db.0.set_override(Some("test".into()));
        }
        game.set_pack_tactic(tactic);
        assert!(wait_for_turn(&mut game, pack[0]));
        assert!(game.tactical_ai_turn_at(0.0));
        step_distance(cell_of(&game, pack[0]), cell_of(&game, pack[1]))
    }

    #[test]
    fn together_holds_back_a_body_whose_profile_closes_hard() {
        use crate::tuning::TACTICAL_COHESION_SLACK;
        // The shipped brute's weight, and one well past it: the pull scales
        // with the profile, so neither is held back by a tie.
        for closing in [1.5, 4.0] {
            assert!(
                runner_under(PackTactic::Rush, closing) > TACTICAL_COHESION_SLACK,
                "control, closing {closing}"
            );
            assert!(
                runner_under(PackTactic::Together, closing) <= TACTICAL_COHESION_SLACK,
                "outran the pack at closing {closing}"
            );
        }
    }
}
