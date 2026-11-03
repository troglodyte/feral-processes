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
