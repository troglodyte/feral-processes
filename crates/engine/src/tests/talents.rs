//! Spending levels earned past the cap: what a point is, what it buys, and
//! every refusal on the way.

use super::support::*;
use crate::talents::{TalentId, TalentNode};
use crate::tuning::{CREATURE_MAX_LEVEL, KERNEL_RING_MAX};
use crate::*;

/// A companion at `level`, with the rings its level implies already open so
/// nothing in the fixture is refused for the wrong reason.
fn developed(game: &mut Game, level: u32) -> Entity {
    let pet = spawn_tamed(game, 30, 6);
    game.world
        .entity_mut(pet)
        .insert(KernelRing(KERNEL_RING_MAX));
    set_level(game, pet, level);
    pet
}

fn taken(game: &Game, pet: Entity) -> Vec<String> {
    game.world
        .get::<Talents>(pet)
        .map(|t| t.0.iter().map(|id| id.to_string()).collect())
        .unwrap_or_default()
}

/// The first choice of the generic tree's first tier — `spawn_tamed`'s species
/// raises no affinity axis, so that is the tree it spends in.
const GEN_HP: &str = "gen_frame";
const GEN_ATK: &str = "gen_edge";

#[test]
fn a_point_is_earned_per_level_above_the_base_cap() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let capped = developed(&mut game, CREATURE_MAX_LEVEL);
    assert_eq!(game.talent_points(capped).earned, 0);
    assert_eq!(game.talent_points(capped).unspent(), 0);

    let past = developed(&mut game, CREATURE_MAX_LEVEL + 2);
    let points = game.talent_points(past);
    assert_eq!(points.earned, 2);
    assert_eq!(points.spent, 0);
    assert_eq!(points.unspent(), 2);
}

#[test]
fn taking_a_talent_with_no_unspent_points_is_refused_and_records_nothing() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = developed(&mut game, CREATURE_MAX_LEVEL);

    assert!(game.take_talent(pet, &TalentId::from(GEN_HP)).is_err());
    assert!(taken(&game, pet).is_empty());
}

#[test]
fn a_tier_cannot_be_skipped() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = developed(&mut game, CREATURE_MAX_LEVEL + 2);
    let tier_two = game.talent_tree(pet).expect("a tree").tiers[1].0[0]
        .id
        .clone();

    assert!(
        game.take_talent(pet, &tier_two).is_err(),
        "tier 2 is not reachable while tier 1 is untaken"
    );
    assert!(taken(&game, pet).is_empty());
}

/// Staged with a node that exists in a *different* tree, which is the case a
/// naive "is this id known anywhere" check gets wrong.
#[test]
fn a_node_from_another_classs_tree_is_refused() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = developed(&mut game, CREATURE_MAX_LEVEL + 2);

    assert!(
        game.take_talent(pet, &TalentId::from("striker_edge"))
            .is_err(),
        "a generic program cannot buy a Striker's node"
    );
    assert!(taken(&game, pet).is_empty());
}

#[test]
fn a_stat_node_raises_its_stat_once_and_cannot_be_taken_twice() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = developed(&mut game, CREATURE_MAX_LEVEL + 2);
    let before = *game.world.get::<Stats>(pet).unwrap();

    game.take_talent(pet, &TalentId::from(GEN_HP)).unwrap();

    let after = *game.world.get::<Stats>(pet).unwrap();
    assert!(
        after.max_hp > before.max_hp,
        "a stat node has to actually raise the stat ({} → {})",
        before.max_hp,
        after.max_hp
    );
    assert_eq!(after.atk, before.atk, "and only the one it names");
    assert_eq!(taken(&game, pet), vec![GEN_HP.to_string()]);

    assert!(
        game.take_talent(pet, &TalentId::from(GEN_HP)).is_err(),
        "a node already taken is not on offer again"
    );
    assert_eq!(
        game.world.get::<Stats>(pet).unwrap().max_hp,
        after.max_hp,
        "and the refusal changed nothing"
    );
}

/// `refactor::raised`'s floor, reached through the talent path: the test exists
/// to prove that path *calls* it rather than restating the arithmetic. 8% of 3
/// ATK rounds straight back to 3.
#[test]
fn a_stat_node_on_a_small_program_still_gains_a_whole_point() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(pet).insert(KernelRing(1));
    set_level(&mut game, pet, CREATURE_MAX_LEVEL + 1);
    {
        let mut stats = game.world.get_mut::<Stats>(pet).unwrap();
        stats.atk = 3;
    }

    game.take_talent(pet, &TalentId::from(GEN_ATK)).unwrap();

    assert_eq!(
        game.world.get::<Stats>(pet).unwrap().atk,
        4,
        "the floor is what makes a percentage buff worth anything to a weak program"
    );
}

#[test]
fn a_program_with_no_class_spends_in_the_generic_tree() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = developed(&mut game, CREATURE_MAX_LEVEL + 1);

    let tree = game.talent_tree(pet).expect("no class is not no tree");
    assert!(tree.class.is_none());
    assert!(
        tree.tiers[0].0.iter().any(|c| c.id.as_str() == GEN_HP),
        "and it is the generic tree, not some class's"
    );
}

#[test]
fn the_options_list_offers_the_next_tier_and_marks_what_is_spent() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = developed(&mut game, CREATURE_MAX_LEVEL + 1);

    let rows = game.talent_options(pet);
    assert!(
        rows.iter().filter(|r| r.takeable).count() == 2,
        "one point buys either choice in tier 1 and nothing deeper"
    );
    assert!(rows.iter().all(|r| !r.taken));
    assert!(
        rows.iter().any(|r| r.tag == "stat"),
        "a row carries the node's shape, so the screen never works it out"
    );

    game.take_talent(pet, &TalentId::from(GEN_HP)).unwrap();
    let rows = game.talent_options(pet);
    assert_eq!(
        rows.iter().filter(|r| r.taken).count(),
        1,
        "what was bought reads as bought"
    );
    assert!(
        !rows.iter().any(|r| r.takeable),
        "and the point is gone, so nothing deeper is on offer yet"
    );
}

/// A **save → load → assert**, and the load must not re-apply the node:
/// `CreatureSave` already writes the raised numbers, so re-applying would
/// compound the bonus on every reload. This is the same rule refactors follow.
#[test]
fn talents_survive_a_save_without_their_stats_being_applied_twice() {
    let dir = scratch_assets_dir("talent_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = developed(&mut game, CREATURE_MAX_LEVEL + 2);
    game.world
        .resource_mut::<crate::resources::Party>()
        .0
        .push(pet);
    game.take_talent(pet, &TalentId::from(GEN_HP)).unwrap();
    let before = *game.world.get::<Stats>(pet).unwrap();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let restored = loaded.world.resource::<crate::resources::Party>().0[0];
    assert_eq!(
        loaded
            .world
            .get::<Talents>(restored)
            .map(|t| t.0.clone())
            .unwrap_or_default(),
        vec![TalentId::from(GEN_HP)],
        "the receipt has to travel"
    );
    let after = *loaded.world.get::<Stats>(restored).unwrap();
    assert_eq!(
        (after.max_hp, after.atk, after.def),
        (before.max_hp, before.atk, before.def),
        "and nothing may re-apply it — a saved program's stats already carry its talents"
    );
    assert_eq!(loaded.talent_points(restored).spent, 1);
}

/// Every node kind is authorable, and the tag is what a menu row reads.
#[test]
fn every_node_kind_carries_a_tag() {
    for (node, tag) in [
        (
            TalentNode::Stat {
                stat: crate::talents::TalentStat::Atk,
                percent: 5.0,
            },
            "stat",
        ),
        (
            TalentNode::Affinity {
                kind: crate::abilities::AffinityKind::Damage,
                mult: 1.1,
            },
            "affinity",
        ),
        (
            TalentNode::Ability {
                id: "core_dump".to_string(),
            },
            "routine",
        ),
        (TalentNode::RoutineSlot, "slot"),
    ] {
        assert_eq!(node.tag(), tag);
    }
}

/// A `RoutineSlot` node. Its tier is the generic tree's third, so the fixture
/// spends two points to get there.
fn a_pet_with_a_slot_talent(game: &mut Game) -> Entity {
    let pet = developed(game, CREATURE_MAX_LEVEL + 3);
    game.take_talent(pet, &TalentId::from(GEN_HP)).unwrap();
    game.take_talent(pet, &TalentId::from("gen_interrupt"))
        .unwrap();
    game.take_talent(pet, &TalentId::from("gen_slot")).unwrap();
    pet
}

#[test]
fn a_routine_slot_talent_gives_a_companion_one_more_slot() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let plain = developed(&mut game, CREATURE_MAX_LEVEL + 3);
    let baseline = game.routine_slots(plain);

    let widened = a_pet_with_a_slot_talent(&mut game);

    assert_eq!(
        game.routine_slots(widened),
        baseline + 1,
        "the node buys exactly one slot over an identical companion without it"
    );
}

/// The player is not a companion and must not read a companion tree — its
/// slots come from `PLAYER_ROUTINE_SLOT_PER_LEVEL` and nothing else.
#[test]
fn a_slot_talent_on_the_player_changes_nothing() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let before = game.routine_slots(player);
    game.world
        .entity_mut(player)
        .insert(Talents(vec![TalentId::from("gen_slot")]));

    assert_eq!(game.routine_slots(player), before);
}
