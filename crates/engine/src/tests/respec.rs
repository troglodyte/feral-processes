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
    assert!(
        max_hp(&game, pet) > before,
        "the talent should have raised it"
    );

    game.respec_talents(pet).unwrap();

    assert_eq!(max_hp(&game, pet), before);
    assert!(
        game.world
            .get::<Talents>(pet)
            .is_none_or(|t| t.0.is_empty()),
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
    assert_eq!(
        stats.hp, 3,
        "a respec must not be the strongest heal in the game"
    );
    assert!(stats.hp <= stats.max_hp);
}

fn credits(game: &Game) -> u32 {
    game.banked(&ids::CREDITS.into())
}

#[test]
fn a_talent_respec_takes_back_the_routine_the_tree_granted() {
    let mut game = funded(16);
    let pet = developed(&mut game, crate::tuning::TALENT_START_LEVEL + 3);
    for id in ["gen_frame", "gen_interrupt", "gen_slot"] {
        game.take_talent(pet, &crate::talents::TalentId::from(id))
            .unwrap();
    }
    let granted = crate::abilities::AbilityId::from("interrupt_request");
    assert!(
        game.world
            .get::<Routines>(pet)
            .unwrap()
            .0
            .contains(&granted),
        "the tier-2 node should have installed its routine"
    );

    game.respec_talents(pet).unwrap();

    let after = game.world.get::<Routines>(pet).unwrap().0.clone();
    assert!(
        !after.contains(&granted),
        "the tree's routine goes back with the tree"
    );
    assert!(
        after.len() <= game.routine_slots(pet),
        "refunding the slot node must not leave the kit over capacity"
    );
    assert!(!after.is_empty(), "a program is never left holding nothing");
}

#[test]
fn a_talent_respec_leaves_a_hand_installed_routine_alone() {
    let mut game = funded(17);
    let pet = developed(&mut game, crate::tuning::TALENT_START_LEVEL + 2);
    game.take_talent(pet, &crate::talents::TalentId::from(GEN_HP))
        .unwrap();

    // Stands in for a routine off a disk: something in the slots that the
    // tree did not put there and has no business taking back.
    let mine = crate::abilities::AbilityId::from("hot_patch");
    game.world
        .get_mut::<Routines>(pet)
        .unwrap()
        .0
        .push(mine.clone());

    game.respec_talents(pet).unwrap();

    assert!(
        game.world.get::<Routines>(pet).unwrap().0.contains(&mine),
        "a respec clears the tree's routines, not the player's"
    );
}

#[test]
fn gear_worn_across_a_respec_is_neither_scaled_nor_welded_in() {
    let mut game = funded(18);
    let player = game.player_entity();
    let base = game.world.get::<Stats>(player).unwrap().atk;

    game.unlock_perk(Perk::Attacker).unwrap();
    wear(&mut game, player, "monofilament_whip");
    game.respec_perks().unwrap();
    let worn = game.world.get::<Stats>(player).unwrap().atk;
    game.unequip(player, EquipmentSlot::Weapon).unwrap();

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().atk,
        base,
        "the bonus must come off clean — a respec run with gear in `Stats` \
         welds the difference into base stats forever"
    );
    assert!(
        worn > base,
        "the weapon was actually worn across the respec"
    );
}

#[test]
fn the_overflow_price_does_not_reset_on_a_respec() {
    let mut game = funded(19);
    let player = game.player_entity();
    for _ in 0..3 {
        game.unlock_perk(Perk::Teardown).unwrap();
    }
    let priced_with_three = crate::tuning::OVERFLOW_XP_BASE + crate::tuning::OVERFLOW_XP_STEP * 3;

    game.respec_perks().unwrap();

    // Exactly one point's worth at the *escalated* price, which is less than
    // one point's worth at the opening rate would have bought.
    game.world.get_mut::<Experience>(player).unwrap().xp = priced_with_three;
    let before = game.world.get::<Perks>(player).unwrap().points;
    let minted = game.convert_overflow_xp();

    assert_eq!(
        minted, 1,
        "the escalator is priced off perks ever bought, so a wipe must not \
         make the next point cheap"
    );
    assert_eq!(game.world.get::<Perks>(player).unwrap().points, before + 1);
    assert_eq!(
        game.world.get::<Experience>(player).unwrap().xp,
        0,
        "the whole escalated price was charged"
    );
}

/// Every refusal lands before a Credit moves, `commit_caravan_basket`'s
/// rule — asserted **per refusal**, because a single test over one of them
/// passes against every path that never spends anyway.
mod refusals {
    use super::*;

    fn assert_free(game: &mut Game, act: impl FnOnce(&mut Game) -> Result<(), String>) -> String {
        let before = credits(game);
        let err = act(game).expect_err("this should have been refused");
        assert_eq!(credits(game), before, "a refusal must not spend: {err}");
        err
    }

    #[test]
    fn with_no_perks_bought() {
        let mut game = funded(20);
        let err = assert_free(&mut game, |g| g.respec_perks());
        assert!(err.contains("no perks"), "{err}");
    }

    #[test]
    fn without_the_credits() {
        let mut game = funded(21);
        game.unlock_perk(Perk::Teardown).unwrap();
        let player = game.player_entity();
        let held = credits(&game);
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(ids::CREDITS.into(), held);

        let err = assert_free(&mut game, |g| g.respec_perks());
        assert!(err.contains("Not enough Credits"), "{err}");
    }

    #[test]
    fn during_a_battle() {
        let mut game = funded(22);
        game.unlock_perk(Perk::Teardown).unwrap();
        let foe = spawn_wild_on_player_tile(&mut game);
        let player = game.player_entity();
        insert_battle(&mut game, player, vec![foe]);

        let err = assert_free(&mut game, |g| g.respec_perks());
        assert!(err.contains("right now"), "{err}");
    }

    #[test]
    fn when_process_pool_would_overfill_the_roster() {
        let mut game = funded(23);
        game.unlock_perk(Perk::ProcessPool).unwrap();
        // Fill every slot the perk opened, so refunding it leaves the roster
        // above `pet_capacity`.
        while game.pet_count() < game.pet_capacity() {
            spawn_tamed(&mut game, 20, 4);
        }

        let err = assert_free(&mut game, |g| g.respec_perks());
        assert!(err.contains("release"), "{err}");
    }

    #[test]
    fn on_a_program_with_no_talents() {
        let mut game = funded(24);
        let pet = developed(&mut game, crate::tuning::TALENT_START_LEVEL + 2);
        let err = assert_free(&mut game, |g| g.respec_talents(pet));
        assert!(err.contains("no talents"), "{err}");
    }

    #[test]
    fn on_a_program_the_player_does_not_control() {
        let mut game = funded(25);
        let wild = spawn_wild_on_player_tile(&mut game);
        let err = assert_free(&mut game, |g| g.respec_talents(wild));
        assert!(err.contains("don't control"), "{err}");
    }
}

#[test]
fn both_receipts_survive_a_save_and_load() {
    let mut game = funded(26);
    let player = game.player_entity();
    let pet = developed(&mut game, crate::tuning::TALENT_START_LEVEL + 2);
    game.unlock_perk(Perk::Buffer).unwrap();
    game.take_talent(pet, &crate::talents::TalentId::from(GEN_HP))
        .unwrap();
    let player_receipt = *game.world.get::<BoughtStats>(player).unwrap();
    let pet_receipt = *game.world.get::<BoughtStats>(pet).unwrap();
    assert!(player_receipt.max_hp > 0 && pet_receipt.max_hp > 0);

    let path = std::env::temp_dir().join(format!("feral_respec_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        *loaded
            .world
            .get::<BoughtStats>(loaded.player_entity())
            .unwrap(),
        player_receipt
    );
    let reloaded_pet = loaded
        .world
        .iter_entities()
        .find(|e| e.contains::<Talents>())
        .expect("the companion should have come back");
    assert_eq!(*reloaded_pet.get::<BoughtStats>().unwrap(), pet_receipt);
}

#[test]
fn a_pre_respec_save_seeds_its_overflow_price_from_the_perks_it_holds() {
    let mut game = funded(27);
    let player = game.player_entity();
    for _ in 0..3 {
        game.unlock_perk(Perk::Teardown).unwrap();
    }
    // A save written before the receipt existed: the perks are recorded, the
    // count is not.
    game.world.entity_mut(player).insert(BoughtStats::default());

    let path = std::env::temp_dir().join(format!("feral_respec_old_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded
            .world
            .get::<BoughtStats>(loaded.player_entity())
            .unwrap()
            .ever_bought,
        3,
        "an old save's escalator is seeded from the perks it is holding"
    );
}
