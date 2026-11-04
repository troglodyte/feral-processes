//! Auto-recast: the per-routine flag, and the rerun when a timed field buff
//! runs out.

use super::support::*;
use crate::abilities::AbilityDb;
use crate::components::{AutoRecast, Routines};
use crate::routine_craft::Core;
use crate::*;

fn game() -> Game {
    Game::new(41_001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Gives `entity` a freshly minted routine built from `core` and returns its
/// id. Built, not hand-written, so the def is the one the game compiles.
fn give_core(game: &mut Game, entity: Entity, core: Core) -> String {
    make_room_for_routine(game, entity);
    let id = built_core(game, core);
    let mut installed = game.world.get::<Routines>(entity).unwrap().0.clone();
    installed.push(id.clone());
    game.world.entity_mut(entity).insert(Routines(installed));
    id
}

fn eligible(game: &Game, id: &str) -> bool {
    game.world
        .resource::<AbilityDb>()
        .get(id)
        .unwrap()
        .auto_recastable()
}

#[test]
fn only_a_timed_field_buff_is_eligible() {
    let mut game = game();
    let regen = built_core(&mut game, Core::FieldRegen);
    let recharge = built_core(&mut game, Core::Recharge);
    let shell = built_core(&mut game, Core::FieldMitigation);
    let heal = built_core(&mut game, Core::Heal);
    let jump = built_core(&mut game, Core::Jump);

    assert!(eligible(&game, &regen));
    assert!(eligible(&game, &recharge));
    assert!(!eligible(&game, &shell), "it runs until rest");
    assert!(!eligible(&game, &heal));
    assert!(!eligible(&game, &jump));
}

#[test]
fn flags_survive_a_save_and_load_for_the_player_and_a_companion() {
    let mut game = game();
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 10);
    let mine = give_core(&mut game, player, Core::FieldRegen);
    let theirs = give_core(&mut game, pet, Core::Recharge);
    game.set_auto_recast(player, &mine, true);
    game.set_auto_recast(pet, &theirs, true);

    let dir = scratch_assets_dir("auto_recast_roundtrip");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("s.ron");
    game.save(&path).expect("save");
    let mut loaded = Game::load(&path, &test_assets_dir()).expect("load");

    assert_eq!(loaded.auto_recast_of(loaded.player_entity()), vec![mine]);
    let loaded_pet = loaded
        .owned_pets()
        .first()
        .map(|p| p.entity)
        .expect("the companion came back");
    assert_eq!(loaded.auto_recast_of(loaded_pet), vec![theirs]);
}

#[test]
fn an_empty_flag_set_writes_nothing() {
    let mut game = game();
    let dir = scratch_assets_dir("auto_recast_empty");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("s.ron");
    game.save(&path).expect("save");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(!text.contains("auto_recast"));
}

#[test]
fn uninstalling_a_routine_clears_its_flag() {
    let mut game = game();
    let player = game.player_entity();
    let id = give_core(&mut game, player, Core::FieldRegen);
    game.set_auto_recast(player, &id, true);
    let slot = game
        .world
        .get::<Routines>(player)
        .unwrap()
        .0
        .iter()
        .position(|r| *r == id)
        .unwrap();

    game.uninstall_routine(player, slot).unwrap();

    assert!(game.auto_recast_of(player).is_empty());
}

#[test]
fn fusion_keeps_a_flag_only_for_a_routine_the_child_holds() {
    let mut game = game();
    unlock_research_chain(&mut game, "program_refactoring");
    let dominant = spawn_tamed(&mut game, 30, 6);
    set_level(&mut game, dominant, 10);
    let other = spawn_tamed(&mut game, 10, 3);
    let built = give_core(&mut game, dominant, Core::FieldRegen);
    let innate = crate::abilities::FALLBACK_ABILITY_ID.to_string();
    game.world
        .entity_mut(dominant)
        .insert(AutoRecast(vec![built.clone(), innate.clone()]));

    game.fuse_companions(dominant, other, None).unwrap();

    let child = game.owned_pets().first().map(|p| p.entity).unwrap();
    let kit = game.world.get::<Routines>(child).unwrap().0.clone();
    assert!(!kit.contains(&built), "a built routine does not survive");
    let flags = game.auto_recast_of(child);
    assert!(!flags.contains(&built));
    assert_eq!(flags.contains(&innate), kit.contains(&innate));
}
