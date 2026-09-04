//! Taking a ledger back: `Game::respec_perks` and `Game::respec_talents`.
//!
//! The compounding-printer tests are the ones this feature exists for — a
//! respec that refunds points without un-baking `Perk::Buffer` lets each
//! cycle compound off a larger maximum.

use super::support::*;
use crate::items::ids;
use crate::tuning::RESPEC_CREDIT_COST;
use crate::*;

/// Enough Credits for `n` respecs, and the points to buy with.
fn funded(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 50;
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ids::CREDITS.into(), RESPEC_CREDIT_COST * 10);
    game
}

fn max_hp(game: &Game, e: Entity) -> i32 {
    game.world.get::<Stats>(e).unwrap().max_hp
}

#[test]
fn respec_returns_a_buffer_level_to_exactly_its_pre_purchase_maximum() {
    let mut game = funded(11);
    let player = game.player_entity();

    let before = max_hp(&game, player);
    game.unlock_perk(Perk::Buffer).unwrap();
    assert!(
        max_hp(&game, player) > before,
        "Buffer should have raised the maximum"
    );

    game.respec_perks().unwrap();

    assert_eq!(
        max_hp(&game, player),
        before,
        "the respec must return the maximum exactly, not approximately"
    );
}

#[test]
fn buying_buffer_after_a_respec_cannot_compound() {
    let mut game = funded(12);
    let player = game.player_entity();

    game.unlock_perk(Perk::Buffer).unwrap();
    let once = max_hp(&game, player);

    game.respec_perks().unwrap();
    game.unlock_perk(Perk::Buffer).unwrap();

    assert_eq!(
        max_hp(&game, player),
        once,
        "a wipe-and-rebuy cycle must not print maximum HP"
    );
}

/// A companion with its rings open and enough levels to spend, the fixture
/// `tests::talents` uses.
fn developed(game: &mut Game, level: u32) -> Entity {
    let pet = spawn_tamed(game, 30, 6);
    game.world
        .entity_mut(pet)
        .insert(KernelRing(crate::tuning::KERNEL_RING_MAX));
    set_level(game, pet, level);
    pet
}

const GEN_HP: &str = "gen_frame";

#[test]
fn respec_returns_a_stat_talent_to_exactly_its_pre_purchase_value() {
    let mut game = funded(13);
    let pet = developed(&mut game, crate::tuning::TALENT_START_LEVEL + 2);

    let before = max_hp(&game, pet);
    game.take_talent(pet, &crate::talents::TalentId::from(GEN_HP))
        .unwrap();
    assert!(max_hp(&game, pet) > before, "the talent should have raised it");

    game.respec_talents(pet).unwrap();

    assert_eq!(max_hp(&game, pet), before);
    assert!(
        game.world.get::<Talents>(pet).is_none_or(|t| t.0.is_empty()),
        "the list is the talent-point refund"
    );
    assert_eq!(
        game.talent_points(pet).unspent(),
        2,
        "clearing the list is what hands the points back"
    );
}

#[test]
fn buying_a_stat_talent_after_a_respec_cannot_compound() {
    let mut game = funded(14);
    let pet = developed(&mut game, crate::tuning::TALENT_START_LEVEL + 2);
    let id = crate::talents::TalentId::from(GEN_HP);

    game.take_talent(pet, &id).unwrap();
    let once = max_hp(&game, pet);

    game.respec_talents(pet).unwrap();
    game.take_talent(pet, &id).unwrap();

    assert_eq!(max_hp(&game, pet), once);
}

#[test]
fn a_respec_does_not_heal() {
    let mut game = funded(15);
    let pet = developed(&mut game, crate::tuning::TALENT_START_LEVEL + 2);
    game.take_talent(pet, &crate::talents::TalentId::from(GEN_HP))
        .unwrap();

    // Hurt well below the pre-talent maximum, so a refill would be obvious.
    game.world.get_mut::<Stats>(pet).unwrap().hp = 3;
    game.respec_talents(pet).unwrap();

    let stats = game.world.get::<Stats>(pet).unwrap();
    assert_eq!(stats.hp, 3, "a respec must not be the strongest heal in the game");
    assert!(stats.hp <= stats.max_hp);
}
