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
