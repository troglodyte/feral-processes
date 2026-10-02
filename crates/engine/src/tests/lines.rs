//! Production lines: which machines feed which, and how a connected run
//! groups into one job.

use super::support::*;
use crate::systems::{feeds, feeds_fuel, feeds_ingredient};
use crate::*;

fn def(game: &Game, id: &str) -> crate::structures::StructureDef {
    game.structure_defs()
        .into_iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("{id} should be a shipped structure"))
}

fn game() -> Game {
    Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

#[test]
fn a_mining_node_feeds_a_lathe_its_ingredient() {
    let g = game();
    let items = g.world.resource::<ItemDb>();
    let (mining, lathe) = (def(&g, "mining_node"), def(&g, "lathe"));
    assert!(feeds_ingredient(&mining, &lathe, items));
    assert!(feeds(&mining, &lathe, items));
    assert!(!feeds_ingredient(&lathe, &mining, items));
}

#[test]
fn a_power_conduit_feeds_a_recharger_its_fuel() {
    let g = game();
    let items = g.world.resource::<ItemDb>();
    let (conduit, recharger) = (def(&g, "power_conduit"), def(&g, "recharger_node"));
    assert!(feeds_fuel(&conduit, &recharger));
    assert!(!feeds_ingredient(&conduit, &recharger, items));
    assert!(feeds(&conduit, &recharger, items));
}

#[test]
fn a_machine_does_not_feed_its_own_kind() {
    let g = game();
    let items = g.world.resource::<ItemDb>();
    let mining = def(&g, "mining_node");
    assert!(!feeds(&mining, &mining, items));
}

#[test]
fn a_teardown_rig_feeds_nothing() {
    let g = game();
    let items = g.world.resource::<ItemDb>();
    let rig = def(&g, "teardown_rig");
    for other in g.structure_defs() {
        assert!(
            !feeds(&rig, &other, items),
            "rig must not feed {}",
            other.id
        );
    }
}
