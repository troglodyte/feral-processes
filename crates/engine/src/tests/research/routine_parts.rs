//! Routine parts are found before they are researched (ADR 0041): the
//! findability rule, and the extraction and study doors that roll on it.

use super::*;
use crate::components::{Routines, Tools};
use crate::routine_craft::{AreaKind, Core, Subroutine, Version};
use crate::routine_tree::{findable_parts, part_node_id, roll_part_finds};
use crate::tools::{ToolCategory, ToolDb};
use rand::SeedableRng;

fn core(c: Core, v: Version) -> Subroutine {
    Subroutine::Core(c, v)
}

/// The shipped routine every door test takes apart: Debuff v1.0, Everyone,
/// Trickle and Stun — four zone-1 parts, none of them a starting part.
const ROUTINE: &str = "hard_fault";

fn discovered(game: &Game) -> Vec<String> {
    let mut ids: Vec<String> = game
        .world
        .resource::<crate::resources::DiscoveredResearch>()
        .0
        .iter()
        .cloned()
        .collect();
    ids.sort();
    ids
}

fn carried_node_ids(game: &Game) -> Vec<String> {
    game.parts_of_routine(ROUTINE)
        .iter()
        .map(part_node_id)
        .collect()
}

// ---------------------------------------------------------------------------
// The pure rule
// ---------------------------------------------------------------------------

#[test]
fn a_v3_carrier_with_only_the_starting_rung_known_offers_v2() {
    let v1 = part_node_id(&core(Core::Heal, Version::V1));
    let found = findable_parts(
        &[core(Core::Heal, Version::V3)],
        |id| id == v1,
        |id| id != v1,
    );
    assert_eq!(found, vec![core(Core::Heal, Version::V2)]);
}

#[test]
fn a_find_never_skips_a_rung() {
    let found = findable_parts(&[core(Core::Damage, Version::V3)], |_| false, |_| true);
    assert_eq!(found, vec![core(Core::Damage, Version::V1)]);
}

#[test]
fn a_modifier_has_no_rung_and_duplicates_collapse() {
    let group = Subroutine::Area(AreaKind::Group);
    let found = findable_parts(
        &[group, core(Core::Damage, Version::V1), group],
        |_| false,
        |_| true,
    );
    assert_eq!(found, vec![core(Core::Damage, Version::V1), group]);
}

#[test]
fn an_ineligible_rung_is_not_offered() {
    let found = findable_parts(&[core(Core::Damage, Version::V1)], |_| false, |_| false);
    assert!(found.is_empty());
}

#[test]
fn the_roll_draws_once_per_part_and_finds_all_at_certainty() {
    let parts = [core(Core::Damage, Version::V1), Subroutine::Trickle];
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    assert_eq!(roll_part_finds(&parts, 1.0, &mut rng), parts.to_vec());
    assert!(roll_part_finds(&parts, 0.0, &mut rng).is_empty());
}

#[test]
fn an_empty_roll_draws_nothing() {
    use rand::RngExt;
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    let mut untouched = rand::rngs::StdRng::seed_from_u64(7);
    roll_part_finds(&[], 0.5, &mut rng);
    assert_eq!(rng.random::<u64>(), untouched.random::<u64>());
}

#[test]
fn a_weighted_pick_walks_the_cumulative_weights() {
    use crate::routine_tree::weighted_pick;
    let weights = [3, 1, 0, 3];
    let picks: Vec<usize> = (0..7).map(|r| weighted_pick(&weights, r)).collect();
    assert_eq!(picks, vec![0, 0, 0, 1, 3, 3, 3]);
}

#[test]
fn a_study_pick_with_nothing_exclusive_is_the_plain_uniform_draw() {
    use rand::RngExt;
    let mut picked = rand::rngs::StdRng::seed_from_u64(9);
    let mut plain = rand::rngs::StdRng::seed_from_u64(9);
    for _ in 0..50 {
        assert_eq!(
            crate::routine_tree::study_pick(7, |_| false, &mut picked),
            plain.random_range(0..7)
        );
    }
}

#[test]
fn a_study_pick_lands_on_an_exclusive_only_entry_a_third_as_often() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(9);
    let mut hits = [0u32; 2];
    for _ in 0..8000 {
        hits[crate::routine_tree::study_pick(2, |i| i == 1, &mut rng)] += 1;
    }
    // 3:1 over 8000 draws is 6000:2000.
    assert!((5700..6300).contains(&hits[0]), "{hits:?}");
    assert!((1700..2300).contains(&hits[1]), "{hits:?}");
}

#[test]
fn the_exclusive_only_parts_join_the_study_pool_off_any_subject() {
    let game = studied_game(4609, true);
    let pool = routine_pool(&game);
    for part in [
        core(Core::FieldMitigation, Version::V1),
        Subroutine::DamageType(crate::damage::DamageType::Magnetic),
    ] {
        assert!(pool.contains(&part_node_id(&part)), "{part:?}");
    }
}

// ---------------------------------------------------------------------------
// The extraction door
// ---------------------------------------------------------------------------

fn next_draw(game: &mut Game) -> u64 {
    use rand::RngExt;
    game.world
        .resource_mut::<crate::resources::GameRng>()
        .0
        .random()
}

#[test]
fn an_all_known_routine_draws_nothing() {
    let mut game = Game::new(4601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 1);
    discover_all_research(&mut game);
    let parts = game.parts_of_routine(ROUTINE);
    reseed_rng(&mut game, 55);
    let without = next_draw(&mut game);

    reseed_rng(&mut game, 55);
    game.take_routine(&parts);

    assert_eq!(next_draw(&mut game), without);
}

#[test]
fn a_routine_with_unfound_parts_draws_once_per_part() {
    let mut game = Game::new(4602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "routine_fabrication");
    set_zone(&mut game, 1);
    let parts = game.parts_of_routine(ROUTINE);
    assert_eq!(game.findable_routine_parts(&parts).len(), 4);
    reseed_rng(&mut game, 55);
    let expected = {
        use rand::RngExt;
        let mut rng = game.world.resource_mut::<crate::resources::GameRng>();
        for _ in 0..4 {
            rng.0
                .random_bool(crate::tuning::ROUTINE_PART_DISCOVERY_CHANCE);
        }
        rng.0.random::<u64>()
    };

    reseed_rng(&mut game, 55);
    game.take_routine(&parts);

    assert_eq!(next_draw(&mut game), expected);
}

/// Until the routine tree is open the part nodes are not in play, so an
/// extraction finds nothing and draws nothing.
#[test]
fn a_closed_routine_tree_finds_nothing_and_draws_nothing() {
    let mut game = Game::new(4608, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 1);
    assert!(!game.routine_tree_open());
    let parts = game.parts_of_routine(ROUTINE);
    reseed_rng(&mut game, 55);
    let without = next_draw(&mut game);

    reseed_rng(&mut game, 55);
    game.take_routine(&parts);

    assert!(discovered(&game).is_empty());
    assert_eq!(next_draw(&mut game), without);
}

/// The order claim of `roll_part_finds`, pinned: with this seed and a chance
/// of one half, these of five parts win.
#[test]
fn a_seeded_roll_picks_these_parts_in_order() {
    let parts = [
        core(Core::Damage, Version::V1),
        core(Core::Heal, Version::V1),
        Subroutine::Trickle,
        core(Core::Cloak, Version::V1),
        core(Core::Fork, Version::V1),
    ];
    let mut rng = rand::rngs::StdRng::seed_from_u64(11);
    let won = roll_part_finds(&parts, 0.5, &mut rng);
    assert_eq!(won, EXPECTED_WON.map(|i| parts[i]).to_vec());
}
const EXPECTED_WON: [usize; 3] = [0, 3, 4];

/// The tamed door: a program taken apart at a bench.
#[test]
fn breaking_down_a_tamed_program_finds_its_parts_and_lists_them() {
    let mut game = Game::new(4603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_an_extraction_bench(&mut game, 30, 30);
    unlock_research_chain(&mut game, "routine_fabrication");
    set_zone(&mut game, 1);
    while game.take_notification().is_some() {}
    for _ in 0..30 {
        let pet = spawn_tamed(&mut game, 10, 3);
        game.world
            .entity_mut(pet)
            .insert(Routines(vec![ROUTINE.to_string()]));
        game.extract_routine(pet, 0).unwrap();
        if !discovered(&game).is_empty() {
            break;
        }
    }
    let found = discovered(&game);
    assert!(!found.is_empty(), "thirty extractions found nothing");
    let carried = carried_node_ids(&game);
    let listed: Vec<String> = game
        .research_nodes(ResearchTree::Routines)
        .into_iter()
        .map(|n| n.id)
        .collect();
    for id in &found {
        assert!(carried.contains(id), "{id} is not a part of {ROUTINE}");
        assert!(listed.contains(id), "{id} was found but is not listed");
    }
    let said = game
        .message_history(500)
        .iter()
        .filter(|l| l.text.starts_with("Taking it apart shows you how"))
        .count();
    assert_eq!(said, found.len());
    assert!(
        !game
            .message_history(500)
            .iter()
            .any(|l| l.text.contains("The study uncovers")),
        "an extraction find is not a study"
    );
    assert!(game.notifications_pending() >= found.len());
}

/// The downed door: a `Routines` tool on a program in the store.
#[test]
fn a_routine_tool_on_a_downed_program_can_find_its_parts() {
    let mut game = Game::new(4604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "routine_fabrication");
    set_zone(&mut game, 1);
    let tool = game
        .world
        .resource::<ToolDb>()
        .all()
        .find(|def| def.category == ToolCategory::Routines)
        .map(|def| def.id.clone())
        .expect("some shipped tool reads routines");
    let player = game.player_entity();
    game.world
        .get_mut::<Tools>(player)
        .unwrap()
        .0
        .push(tool.clone());
    for _ in 0..30 {
        game.world
            .get_mut::<crate::components::DownedPrograms>(player)
            .unwrap()
            .0
            .push(DownedProgram {
                species: "scrapper".to_string(),
                level: 30,
                rarity: crate::Rarity::Ordinary,
                boss: false,
                condition: crate::tuning::CONDITION_BASE,
                carried: None,
            });
        game.extract_program(0, &tool).unwrap();
        if !discovered(&game).is_empty() {
            break;
        }
    }
    assert!(
        !discovered(&game).is_empty(),
        "thirty extractions found nothing"
    );
}

// ---------------------------------------------------------------------------
// The study door
// ---------------------------------------------------------------------------

fn studied_game(seed: u32, open: bool) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = base_with_a_research_node(&mut game);
    let program = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(program)
        .insert(Routines(vec![ROUTINE.to_string()]));
    pin_subject_at_pen(&mut game, program, node);
    if open {
        unlock_research_chain(&mut game, "routine_fabrication");
    }
    set_zone(&mut game, 1);
    game
}

fn routine_pool(game: &Game) -> Vec<String> {
    let db = game.world.resource::<crate::research::ResearchDb>();
    game.eligible_discoveries()
        .into_iter()
        .filter(|id| db.get(id).unwrap().tree == ResearchTree::Routines)
        .collect()
}

fn carried_anywhere(game: &Game) -> std::collections::BTreeSet<String> {
    crate::routine_tree::program_carried_parts(
        game.world.resource::<crate::abilities::AbilityDb>(),
        game.world.resource::<crate::species::SpeciesDb>(),
    )
    .iter()
    .map(part_node_id)
    .collect()
}

/// A carried part enters the pool only off a subject carrying it; a part no
/// shipped program carries enters it off any subject.
#[test]
fn a_study_pool_holds_the_subjects_parts_and_the_uncarried_ones() {
    let game = studied_game(4605, true);
    let carried_anywhere = carried_anywhere(&game);
    let pool = routine_pool(&game);
    let mut from_carriers: Vec<String> = pool
        .iter()
        .filter(|id| carried_anywhere.contains(*id))
        .cloned()
        .collect();
    from_carriers.sort();
    let mut subjects = carried_node_ids(&game);
    subjects.sort();
    assert_eq!(from_carriers, subjects);
    assert!(
        !pool.contains(&part_node_id(&core(Core::Damage, Version::V1))),
        "a carried part the subject does not carry never enters the pool"
    );
    let near = part_node_id(&Subroutine::Range(crate::routine_craft::RangeKind::Near));
    assert!(
        !carried_anywhere.contains(&near),
        "fixture: no program carries Near"
    );
    assert!(
        pool.contains(&near),
        "an uncarried part is findable by study"
    );
}

#[test]
fn a_closed_routine_tree_adds_nothing_to_the_study_pool() {
    let game = studied_game(4606, false);
    assert!(routine_pool(&game).is_empty());
}

#[test]
fn a_study_of_a_carrier_can_find_its_part() {
    let mut game = studied_game(4607, true);
    // Leave the pool nothing but the subject's parts.
    let carried_anywhere = carried_anywhere(&game);
    let base: Vec<String> = game
        .eligible_discoveries()
        .into_iter()
        .filter(|id| !carried_anywhere.contains(id))
        .collect();
    for id in base {
        game.discover_research(&id, crate::DiscoverySource::Study);
    }
    for _ in 0..200 {
        game.world
            .resource_mut::<crate::resources::ActiveResearch>()
            .study = crate::tuning::STUDY_ATTEMPT_DATA;
        game.settle_study();
        if game.pinned_subject().is_none() {
            break;
        }
    }
    let carried = carried_node_ids(&game);
    assert!(
        discovered(&game).iter().any(|id| carried.contains(id)),
        "two hundred attempts found none of the subject's parts"
    );
}

/// `settle_study` applies the lighter weight to an exclusive-only part, not
/// just `study_pick` in isolation: on a seed where the weighted and uniform
/// picks differ, the study lands on the weighted one.
#[test]
fn a_study_weighs_exclusive_only_parts_in_its_own_pick() {
    use rand::RngExt;
    let magnetic = part_node_id(&Subroutine::DamageType(crate::damage::DamageType::Magnetic));
    let probe = studied_game(4620, true);
    let pool = probe.eligible_discoveries();
    assert!(pool.contains(&magnetic), "fixture: Magnetic is in the pool");
    let heavy = |i: usize| {
        pool[i] == magnetic || part_node_id(&core(Core::FieldMitigation, Version::V1)) == pool[i]
    };
    let seed = (0..500u64)
        .find(|&seed| {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            if !rng.random_bool(crate::tuning::STUDY_DISCOVERY_CHANCE) {
                return false;
            }
            let mut uniform = rand::rngs::StdRng::seed_from_u64(seed);
            uniform.random_bool(crate::tuning::STUDY_DISCOVERY_CHANCE);
            let weighted = crate::routine_tree::study_pick(pool.len(), heavy, &mut rng);
            weighted != uniform.random_range(0..pool.len())
        })
        .expect("some seed tells the two picks apart");
    let expected = {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        rng.random_bool(crate::tuning::STUDY_DISCOVERY_CHANCE);
        pool[crate::routine_tree::study_pick(pool.len(), heavy, &mut rng)].clone()
    };

    let mut game = studied_game(4620, true);
    assert_eq!(game.eligible_discoveries(), pool);
    game.world
        .resource_mut::<crate::resources::ActiveResearch>()
        .study = crate::tuning::STUDY_ATTEMPT_DATA;
    reseed_rng(&mut game, seed);
    game.settle_study();

    assert_eq!(discovered(&game), vec![expected]);
}
