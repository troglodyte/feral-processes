//! The scored choice a hostile makes on a battle map: which body it swings
//! at and whether it fires a routine, by the profile its species names.
//!
//! Each fixture overrides every hostile's profile with a hand-written one
//! (`BehaviourDb::set_override`), so a test names the weights it is about and
//! not the shipped numbers.

use super::support::{enlist, spawn_tamed, test_assets_dir};
use super::tactical::{only_routine, open_ground, tactical_fight, wait_for_turn};
use crate::Game;
use crate::abilities::TamperKind;
use crate::behaviours::{BehaviourDb, BehaviourDef};
use crate::components::{AbilityCooldowns, GlyphColor, Routines, Stats, Tampered};
use crate::resources::{Behaviours, DifficultyMode};
use crate::tactical::ai::ForecastAction;
use crate::tactical::{Decoy, TacticalBattle};
use bevy_ecs::prelude::Entity;

fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A profile named `test` weighing exactly `weights`.
fn profile(weights: &[(&str, f32)]) -> BehaviourDef {
    BehaviourDef {
        id: "test".into(),
        name: "Test".into(),
        description: String::new(),
        weights: weights.iter().map(|&(n, w)| (n.to_string(), w)).collect(),
        ..BehaviourDef::default()
    }
}

/// Every hostile plays `def` from here on.
fn play_as(game: &mut Game, def: BehaviourDef) {
    let mut db = game.world.resource_mut::<Behaviours>();
    db.0.insert(def);
    db.0.set_override(Some("test".into()));
}

fn strip_routines(game: &mut Game, body: Entity) {
    game.world.entity_mut(body).insert(Routines::default());
}

fn set_hp(game: &mut Game, body: Entity, hp: i32, max: i32) {
    let mut stats = game.world.get_mut::<Stats>(body).unwrap();
    stats.hp = hp;
    stats.max_hp = max;
}

/// Lets `forecast` read `body`'s intent, as a Profile tamper does.
fn profiled(game: &mut Game, body: Entity) {
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Profiled, 5, false);
    game.world.entity_mut(body).insert(tampered);
}

/// The cell `actor`'s next action lands on, with its walk pinned where it
/// stands, read off the bolt it cues.
fn swing_lands_on(game: &mut Game, actor: Entity) -> (i32, i32) {
    assert!(wait_for_turn(game, actor), "the fight ended early");
    game.world
        .resource_mut::<TacticalBattle>()
        .commit_walk(Vec::new());
    game.take_bolts();
    game.tactical_ai_beat();
    let bolts = game.take_bolts();
    assert_eq!(bolts.len(), 1, "expected one blow: {bolts:?}");
    bolts[0].to
}

/// A hostile at (3,4) beside the player at (4,4) and a companion at (3,3),
/// neither holding a routine to fire. The player sits at half Integrity, the
/// companion at `companion_hp` of its own `companion_max`.
struct Duel {
    game: Game,
    wild: Entity,
    companion: Entity,
}

fn duel(def: BehaviourDef, companion_hp: i32, companion_max: i32) -> Duel {
    let mut game = game();
    let companion = spawn_tamed(&mut game, companion_hp, 3);
    enlist(&mut game, companion);
    let wild = tactical_fight(&mut game, 1, 10_000)[0];
    strip_routines(&mut game, wild);
    open_ground(&mut game, &[wild, companion], &[(3, 4), (3, 3)]);
    let player = game.player_entity();
    set_hp(&mut game, player, 5_000, 10_000);
    set_hp(&mut game, companion, companion_hp, companion_max);
    play_as(&mut game, def);
    Duel {
        game,
        wild,
        companion,
    }
}

const COMPANION_CELL: (i32, i32) = (3, 3);

#[test]
fn a_profile_with_every_weight_zero_still_takes_its_turn_and_ties_go_to_reading_order() {
    let mut d = duel(profile(&[]), 40, 40);
    // Both in reach and scored alike: the companion stands on the earlier row.
    let hit = swing_lands_on(&mut d.game, d.wild);
    assert_eq!(hit, COMPANION_CELL);
}

/// The shipped `default` profile, which must reproduce the fixed rule it
/// replaced: swing at the body with the fewest absolute Integrity in reach.
#[test]
fn the_shipped_default_swings_at_the_weakest_body_in_reach() {
    let mut d = duel(profile(&[]), 40, 40);
    d.game
        .world
        .resource_mut::<Behaviours>()
        .0
        .set_override(Some("default".into()));
    // The companion is at full fraction but 40 Integrity; the player is at
    // half of 10 000. The old rule read the raw figure, not the fraction.
    assert_eq!(swing_lands_on(&mut d.game, d.wild), COMPANION_CELL);
}

#[test]
fn a_profile_that_rewards_would_kill_swings_at_the_body_it_can_finish() {
    // The companion is at full fraction but one hit from gone; the player is
    // at half and nowhere near. Lowest fraction picks the player, a taste for
    // kills the companion.
    let mut d = duel(
        profile(&[("target_hp_frac", -1.0), ("would_kill", 5.0)]),
        1,
        1,
    );
    assert_eq!(swing_lands_on(&mut d.game, d.wild), COMPANION_CELL);
    let _ = d.companion;
}

/// The old rule considered only the *first* ready routine, and swung when it
/// was not worth choosing — a Reanimate with nothing to raise. A later
/// routine is never a candidate.
#[test]
fn only_the_first_ready_routine_is_a_candidate() {
    let mut d = duel(profile(&[("is_routine", 5.0)]), 40, 40);
    d.game
        .world
        .entity_mut(d.wild)
        .insert(Routines(vec!["respawn".into(), "acid_wash".into()]));
    profiled(&mut d.game, d.wild);
    assert!(wait_for_turn(&mut d.game, d.wild));
    let forecast = d
        .game
        .tactical_forecast(d.wild)
        .expect("a profiled hostile");
    assert_eq!(forecast.action, ForecastAction::Swing);
    d.game.tactical_ai_turn();
    assert!(
        d.game
            .world
            .get::<AbilityCooldowns>(d.wild)
            .is_none_or(|c| c.0.is_empty()),
        "the second routine fired past a first that was not worth choosing"
    );
}

/// The routine-or-swing choice is made once and carried over the walk. Here
/// the swing wins from afar (it wants distance) and the routine from beside
/// the target, so a choice re-asked on arrival would fire the routine the
/// forecast never named.
#[test]
fn the_routine_or_swing_choice_survives_the_walk() {
    let mut game = game();
    let wild = tactical_fight(&mut game, 1, 10_000)[0];
    only_routine(&mut game, wild, "acid_wash");
    open_ground(&mut game, &[wild], &[(0, 4)]);
    play_as(
        &mut game,
        profile(&[("is_routine", 3.0), ("target_distance", 10.0)]),
    );
    profiled(&mut game, wild);
    assert!(wait_for_turn(&mut game, wild));
    let forecast = game.tactical_forecast(wild).expect("a profiled hostile");
    assert_eq!(forecast.action, ForecastAction::Swing);
    game.take_bolts();
    game.tactical_ai_turn();
    assert_ne!(
        game.world.resource::<TacticalBattle>().cell_of(wild),
        Some((0, 4)),
        "fixture: the hostile must have walked before it acted"
    );
    assert_eq!(
        game.take_bolts().len(),
        1,
        "the hostile did not swing the turn the forecast named"
    );
}

#[test]
fn a_profile_that_dislikes_routines_swings_instead_of_firing_a_ready_one() {
    let fired = |weights: &[(&str, f32)]| {
        let mut d = duel(profile(weights), 40, 40);
        only_routine(&mut d.game, d.wild, "acid_wash");
        assert!(wait_for_turn(&mut d.game, d.wild));
        d.game.tactical_ai_turn();
        d.game
            .world
            .get::<AbilityCooldowns>(d.wild)
            .is_some_and(|c| c.0.contains_key("acid_wash"))
    };
    assert!(
        fired(&[("is_routine", 5.0)]),
        "fixture: a routine lover fires"
    );
    assert!(
        !fired(&[("is_routine", -5.0)]),
        "a routine hater fired anyway"
    );
}

#[test]
fn a_hallucinating_hostile_strikes_its_decoy_whatever_the_profile_prefers() {
    let mut d = duel(profile(&[("target_is_player", 50.0)]), 40, 40);
    let decoy = (3, 5);
    {
        let mut battle = d.game.world.resource_mut::<TacticalBattle>();
        battle.place_decoy(Decoy {
            cell: decoy,
            owner_hostile: false,
            glyph: '@',
            color: GlyphColor::Cyan,
            of_player: true,
        });
    }
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Hallucinating { decoys: 1 }, 3, false);
    d.game.world.entity_mut(d.wild).insert(tampered);
    assert_eq!(swing_lands_on(&mut d.game, d.wild), decoy);
}

#[test]
fn an_injected_hostile_keeps_its_profile_and_swings_at_the_side_it_acts_for() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 3, 100);
    for &w in &pack {
        strip_routines(&mut game, w);
    }
    open_ground(&mut game, &pack, &[(3, 4), (3, 3), (2, 4)]);
    let (injected, hurt, healthy) = (pack[0], pack[1], pack[2]);
    set_hp(&mut game, hurt, 10, 100);
    // Each profile reads the hostile packmates as its targets once injected.
    let mut tampered = Tampered::default();
    tampered.apply(TamperKind::Injected, 5, false);
    game.world.entity_mut(injected).insert(tampered);
    play_as(&mut game, profile(&[("target_hp_frac", 1.0)]));

    // Prefers the healthier body, so not the one lowest-HP would take.
    assert_eq!(swing_lands_on(&mut game, injected), (2, 4));
    let _ = healthy;
}

#[test]
fn every_shipped_profile_forecasts_the_action_its_turn_takes() {
    let (db, _) = BehaviourDb::load_dir(&test_assets_dir().join("behaviours")).unwrap();
    let ids: Vec<String> = db.ids().map(str::to_string).collect();
    assert!(!ids.is_empty());
    for id in ids {
        for routine in [None, Some("acid_wash")] {
            let mut d = duel(profile(&[]), 40, 40);
            d.game
                .world
                .resource_mut::<Behaviours>()
                .0
                .set_override(Some(id.clone()));
            if let Some(r) = routine {
                only_routine(&mut d.game, d.wild, r);
            }
            let mut tampered = Tampered::default();
            tampered.apply(TamperKind::Profiled, 5, false);
            d.game.world.entity_mut(d.wild).insert(tampered);
            assert!(wait_for_turn(&mut d.game, d.wild));
            let forecast = d
                .game
                .tactical_forecast(d.wild)
                .expect("a profiled hostile");
            d.game.tactical_ai_turn();
            let fired = d
                .game
                .world
                .get::<AbilityCooldowns>(d.wild)
                .is_some_and(|c| c.0.contains_key("acid_wash"));
            let expected = if fired {
                ForecastAction::Routine("acid_wash".into())
            } else {
                ForecastAction::Swing
            };
            assert_eq!(
                forecast.action, expected,
                "profile {id:?}, routine {routine:?}"
            );
        }
    }
}

#[test]
fn the_same_seed_and_profile_replay_the_same_blow() {
    let blow = || {
        let mut d = duel(
            profile(&[("target_hp_frac", -1.0), ("est_damage_frac", 2.0)]),
            40,
            40,
        );
        swing_lands_on(&mut d.game, d.wild)
    };
    assert_eq!(blow(), blow());
}

// ---- personalities --------------------------------------------------------

mod personalities {
    use super::*;
    use crate::components::Creature;
    use crate::tactical::map::Board;
    use crate::tactical::reach;

    /// Plays `wild` as `species`, so it carries that species' profile and
    /// its swing range, with no routine to fire.
    fn become_species(game: &mut Game, wild: Entity, species: &str) {
        game.world.get_mut::<Creature>(wild).unwrap().species = species.to_string();
        strip_routines(game, wild);
    }

    fn override_profile(game: &mut Game, id: &str) {
        game.world
            .resource_mut::<Behaviours>()
            .0
            .set_override(Some(id.into()));
    }

    fn open_board(game: &mut Game, boulders: &[(usize, usize)]) {
        let mut rows = vec![".".repeat(12); 12];
        for &(x, y) in boulders {
            rows[y].replace_range(x..=x, "#");
        }
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        game.world.resource_mut::<TacticalBattle>().board = Board::from_rows(&rows);
    }

    fn put(game: &mut Game, body: Entity, cell: (i32, i32)) {
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(body, cell),
            "{cell:?} would not take a body"
        );
    }

    fn cell_of(game: &Game, body: Entity) -> (i32, i32) {
        game.world
            .resource::<TacticalBattle>()
            .cell_of(body)
            .expect("on the board")
    }

    /// A ranged `species` at (5,4) and the player at (8,4), a boulder on
    /// (7,3) beside the line between them, so the cell (6,4) has partial cover
    /// from the player and still sees them. Whether the cell its turn ends on
    /// has cover from the player.
    fn ranged_stand(species: &str) -> bool {
        let mut game = game();
        let wild = tactical_fight(&mut game, 1, 10_000)[0];
        become_species(&mut game, wild, species);
        assert!(game.swing_range(wild) > 1, "{species} is not ranged");
        open_board(&mut game, &[(7, 3)]);
        let player = game.player_entity();
        set_hp(&mut game, player, 10_000, 10_000);
        put(&mut game, player, (8, 4));
        put(&mut game, wild, (5, 4));
        assert!(wait_for_turn(&mut game, wild));
        assert!(game.tactical_ai_turn_at(0.0));
        let end = cell_of(&game, wild);
        let board = &game.world.resource::<TacticalBattle>().board;
        reach::cover_between(board, (8, 4), end)
    }

    #[test]
    fn a_brute_walks_past_cover_and_a_skirmisher_walks_to_it() {
        assert!(ranged_stand("glitch"), "the skirmisher took no cover");
        assert!(!ranged_stand("rootkit"), "the brute stopped for cover");
    }

    /// A ranged `species` at (2,4) with the player at (4,4) and a companion
    /// at (3,4); the player is the hurt one unless `companion_hp` says
    /// otherwise. Returns the cell its blow lands on.
    fn picks(species: &str, own_hp: i32, player_hp: i32, companion_hp: i32) -> (i32, i32) {
        let mut game = game();
        let companion = spawn_tamed(&mut game, 100, 3);
        enlist(&mut game, companion);
        let wild = tactical_fight(&mut game, 1, 100)[0];
        become_species(&mut game, wild, species);
        open_board(&mut game, &[]);
        let player = game.player_entity();
        set_hp(&mut game, player, player_hp, 100);
        set_hp(&mut game, companion, companion_hp, 100);
        set_hp(&mut game, wild, own_hp, 100);
        // Parked first so no body is refused a cell another has not left.
        put(&mut game, wild, (0, 11));
        put(&mut game, player, (4, 4));
        put(&mut game, companion, (3, 4));
        put(&mut game, wild, (2, 4));
        swing_lands_on(&mut game, wild)
    }

    #[test]
    fn a_hunter_swings_at_the_wounded_companion_over_a_player_it_would_otherwise_pick() {
        // The player is the weakest body, so `default` takes them; a hunter
        // would sooner have the soft target.
        let (player_cell, companion_cell) = ((4, 4), (3, 4));
        assert_eq!(picks("crawler", 100, 50, 75), player_cell, "the control");
        assert_eq!(picks("cipher", 100, 50, 75), companion_cell);
    }

    #[test]
    fn a_brute_swings_at_the_nearest_body_not_the_weakest() {
        let (player_cell, companion_cell) = ((4, 4), (3, 4));
        // The player is the weaker and the farther: `default` takes them, a
        // brute takes the companion beside it.
        assert_eq!(picks("crawler", 100, 50, 100), player_cell, "the control");
        assert_eq!(picks("rootkit", 100, 50, 100), companion_cell);
    }

    #[test]
    fn a_cautious_body_prefers_the_far_target_only_when_it_is_hurt() {
        let (player_cell, companion_cell) = ((4, 4), (3, 4));
        assert_eq!(
            picks("drone", 10, 100, 100),
            player_cell,
            "hurt: the far one"
        );
        assert_eq!(
            picks("drone", 100, 100, 100),
            companion_cell,
            "well: the near one"
        );
    }

    /// Nine of a kind fold into a 2x2 squad. Stood with its block just in
    /// reach of the player, it holds still: the block reaches, though its
    /// anchor is a cell short, and a profile that walks it anyway is
    /// scoring the anchor.
    #[test]
    fn a_squad_under_a_profile_scores_its_whole_footprint() {
        let mut game = game();
        tactical_fight(&mut game, 9, 10);
        let squad = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| game.world.get::<crate::components::Squad>(e).is_some())
                .expect("nine of a kind seat a squad")
        };
        override_profile(&mut game, "skirmisher");
        open_board(&mut game, &[]);
        let player = game.player_entity();
        let singles: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .filter(|&e| e != squad && e != player)
            .collect();
        for (i, &single) in singles.iter().enumerate() {
            put(&mut game, single, (i as i32 * 2, 11));
        }
        // On the very edge of the block's reach, which the anchor alone falls
        // one cell short of.
        let reach_of = game.swing_range(squad) as i32;
        put(&mut game, player, (2 + reach_of, 2));
        put(&mut game, squad, (1, 1));
        assert!(wait_for_turn(&mut game, squad));
        assert_eq!(
            game.world.resource::<TacticalBattle>().footprint_of(squad),
            2
        );

        assert!(game.tactical_ai_turn_at(0.0));

        assert_eq!(
            cell_of(&game, squad),
            (1, 1),
            "the squad walked off a reach it had"
        );
    }
}
