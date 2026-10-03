//! The Power Siphon's ledger half: a siphon supplies its authored
//! `power_supply` only while some program holds a `Siphoned` pointing at it.
//! Every figure is read from the shipped def, never a literal.

use super::support::*;
use crate::components::Siphoned;
use crate::game::base::power::ledger;
use crate::structures::StructureDb;
use crate::*;

const SIPHON: &str = "power_siphon";

fn game() -> Game {
    Game::new(6101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn supply(game: &Game) -> u32 {
    let db = game.world.resource::<StructureDb>();
    let items = game.world.resource::<crate::items_db::ItemDb>();
    ledger(&game.world, db, items).supply
}

fn authored_supply(game: &Game) -> u32 {
    game.world
        .resource::<StructureDb>()
        .get(SIPHON)
        .expect("the shipped siphon loads")
        .power_supply
}

#[test]
fn an_empty_siphon_supplies_nothing_and_draws_nothing() {
    let mut game = game();
    let baseline = supply(&game);
    let (draw_before, _) = game.base_power();
    spawn_structure_at(&mut game, SIPHON, 3, 3);

    assert_eq!(supply(&game), baseline);
    let (draw, supply_now) = game.base_power();
    assert_eq!(draw, draw_before);
    assert_eq!(supply_now, baseline);
}

#[test]
fn an_occupied_siphon_adds_its_authored_supply() {
    let mut game = game();
    let baseline = supply(&game);
    let siphon = spawn_structure_at(&mut game, SIPHON, 3, 3);
    let program = spawn_tamed(&mut game, 20, 5);
    game.world.entity_mut(program).insert(Siphoned { siphon });
    let authored = authored_supply(&game);
    assert!(authored > 0);

    assert_eq!(supply(&game), baseline + authored);
    assert_eq!(game.base_power().1, baseline + authored);
}

#[test]
fn releasing_the_marker_drops_the_supply_again() {
    let mut game = game();
    let baseline = supply(&game);
    let siphon = spawn_structure_at(&mut game, SIPHON, 3, 3);
    let program = spawn_tamed(&mut game, 20, 5);
    game.world.entity_mut(program).insert(Siphoned { siphon });
    game.world.entity_mut(program).remove::<Siphoned>();

    assert_eq!(supply(&game), baseline);
}

#[test]
fn a_siphon_with_a_dangling_marker_counts_nothing_extra() {
    let mut game = game();
    let baseline = supply(&game);
    let _empty = spawn_structure_at(&mut game, SIPHON, 3, 3);
    let elsewhere = spawn_structure_at(&mut game, "data_cache", 5, 5);
    let program = spawn_tamed(&mut game, 20, 5);
    game.world
        .entity_mut(program)
        .insert(Siphoned { siphon: elsewhere });

    assert_eq!(supply(&game), baseline);
}

#[test]
fn the_shipped_siphon_is_a_pure_supplier() {
    let game = game();
    let def = game
        .world
        .resource::<StructureDb>()
        .get(SIPHON)
        .expect("the shipped siphon loads");
    assert!(def.siphons);
    assert!(def.power_supply > 0);
    assert!(def.power_upkeep.is_none());
    assert!(def.work.is_none());
}
