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

const PLAYER_CELL: (i32, i32) = (4, 4);
const COMPANION_CELL: (i32, i32) = (3, 3);

#[test]
fn a_profile_with_every_weight_zero_still_takes_its_turn_and_ties_go_to_reading_order() {
    let mut d = duel(profile(&[]), 40, 40);
    // Both in reach and scored alike: the companion stands on the earlier row.
    let hit = swing_lands_on(&mut d.game, d.wild);
    assert_eq!(hit, COMPANION_CELL);
}

/// The shipped `default` profile, which must reproduce the fixed rule it
/// replaced: swing at the weakest body in reach.
#[test]
fn the_shipped_default_swings_at_the_weakest_body_in_reach() {
    let mut d = duel(profile(&[]), 40, 40);
    d.game
        .world
        .resource_mut::<Behaviours>()
        .0
        .set_override(Some("default".into()));
    // The player is at half, the companion at full.
    assert_eq!(swing_lands_on(&mut d.game, d.wild), PLAYER_CELL);
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
