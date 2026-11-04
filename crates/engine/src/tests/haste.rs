//! `Hasted`: armed by the Haste Core, kept at the higher version, and gone
//! when the fight is.

use super::support::*;
use crate::abilities::AbilityDef;
use crate::components::Hasted;
use crate::routine_craft::{Core, CraftedRoutine, Subroutine, Version, compile};
use crate::*;

fn haste(version: Version) -> AbilityDef {
    compile(&CraftedRoutine {
        id: "crafted/1".to_string(),
        name: "Quicken".to_string(),
        parts: vec![Subroutine::Core(Core::Haste, version)],
    })
}

fn hasted_version(game: &Game, entity: Entity) -> Option<u8> {
    game.world.get::<Hasted>(entity).map(|h| h.version)
}

fn fight_with_a_companion() -> (Game, Entity, Entity, Entity) {
    let mut game = Game::new(7_301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 100_000, 1);
    enlist(&mut game, pet);
    let pos = *game.world.get::<Position>(player).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", pos.x, pos.y);
    insert_battle(&mut game, player, vec![wild]);
    (game, player, pet, wild)
}

#[test]
fn casting_haste_arms_it_on_the_recipient_and_nobody_else() {
    let (mut game, player, pet, _) = fight_with_a_companion();
    game.use_ability(&haste(Version::V2), player, "You", &[pet]);
    assert_eq!(hasted_version(&game, pet), Some(2));
    assert_eq!(hasted_version(&game, player), None);
}

#[test]
fn recasting_keeps_the_higher_version_in_either_order() {
    let (mut game, player, pet, _) = fight_with_a_companion();
    game.use_ability(&haste(Version::V1), player, "You", &[pet]);
    game.use_ability(&haste(Version::V3), player, "You", &[pet]);
    assert_eq!(hasted_version(&game, pet), Some(3));
    game.use_ability(&haste(Version::V1), player, "You", &[pet]);
    assert_eq!(hasted_version(&game, pet), Some(3));
}

#[test]
fn the_end_of_the_fight_clears_haste_on_every_side() {
    let (mut game, player, pet, wild) = fight_with_a_companion();
    for body in [player, pet, wild] {
        game.arm_haste(body, 3);
    }
    game.clear_battle_status_effects(player, Some(wild));
    for body in [player, pet, wild] {
        assert_eq!(hasted_version(&game, body), None);
    }
}

#[test]
fn a_hasted_body_shows_as_a_buff_row_carrying_its_version() {
    let (mut game, player, pet, _) = fight_with_a_companion();
    game.use_ability(&haste(Version::V3), player, "You", &[pet]);
    let row = game
        .active_buffs()
        .into_iter()
        .find(|b| b.name == "Hasted v3")
        .expect("a hasted companion is something running");
    assert_eq!(row.magnitude, "\u{2014}");
    assert_eq!(
        row.remaining, "",
        "haste has no round count to print, only the fight"
    );
    assert!(row.holder_label.is_some(), "a companion's row is tagged");
}

#[test]
fn a_hasted_body_reaches_the_tactical_view_marked() {
    let (mut game, player, _, wild) = fight_with_a_companion();
    game.world.entity_mut(player).insert(Hasted { version: 1 });
    game.open_tactical_battle(vec![wild]);
    let view = game.tactical_view().expect("a fight is open");
    let flag = |e| view.bodies.iter().find(|b| b.entity == e).map(|b| b.hasted);
    assert_eq!(flag(player), Some(true));
    assert_eq!(flag(wild), Some(false));
}
