//! `Game::seat_derived`: a tamed program's stats become the derivation of its
//! attributes without moving a single figure.

use super::support::*;
use crate::attributes::AttributeId;
use crate::components::{Attributes, BoughtStats, Derived, HoldPoints, ProgramBase, StatPoints};
use crate::*;

fn game() -> Game {
    Game::new(9101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn species(game: &Game) -> String {
    game.species_defs()[0].id.to_string()
}

fn set_attribute(game: &mut Game, entity: Entity, id: &str, value: i32) {
    game.world
        .get_mut::<Attributes>(entity)
        .unwrap()
        .set(&AttributeId::from(id), value);
}

fn stats_of(game: &Game, entity: Entity) -> Stats {
    *game.world.get::<Stats>(entity).unwrap()
}

/// A tamed program that no door has seated: what `roster_parts` leaves behind.
fn unseated_program(game: &mut Game) -> Entity {
    let s = species(game);
    let program = game
        .spawn_wild_creature_scaled(&s, 60, 60, 1.0, false)
        .unwrap();
    game.world
        .entity_mut(program)
        .remove::<(Hostile, WanderAi)>();
    let parts = game.roster_parts();
    game.world.entity_mut(program).insert(parts);
    program
}

/// Seated, and already at a fixpoint: recomputing from the stored base gives
/// the figures it was seated from.
fn assert_seated_and_stable(game: &mut Game, program: Entity) {
    assert!(game.world.get::<ProgramBase>(program).is_some());
    assert!(game.world.get::<Derived>(program).is_some());
    assert_eq!(game.world.get::<StatPoints>(program), Some(&StatPoints(0)));
    assert_eq!(
        game.world.get::<HoldPoints>(program),
        Some(&HoldPoints(false))
    );
    let before = stats_of(game, program);
    game.recompute_derived(program);
    assert_eq!(stats_of(game, program), before);
}

#[test]
fn seating_leaves_stats_alone_with_gear_worn_and_a_receipt() {
    let mut game = game();
    let program = unseated_program(&mut game);
    set_attribute(&mut game, program, "parity", 63);
    set_attribute(&mut game, program, "analysis", 13);
    set_attribute(&mut game, program, "footprint", 52);
    game.world.entity_mut(program).insert(BoughtStats {
        atk: 3,
        mitigation: 2,
        max_hp: 7,
        ..Default::default()
    });
    {
        let mut stats = game.world.get_mut::<Stats>(program).unwrap();
        stats.atk += 3;
        stats.mitigation += 2;
        stats.max_hp += 7;
    }
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(program, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
        .unwrap();
    assert!(
        game.gear_bonus(program).atk > 0,
        "the fixture must wear something"
    );
    let before = stats_of(&game, program);

    game.seat_derived(program);

    assert_eq!(stats_of(&game, program), before);
    assert_seated_and_stable(&mut game, program);
}

#[test]
fn a_program_whose_attributes_would_clamp_a_stat_still_round_trips() {
    let mut game = game();
    let program = unseated_program(&mut game);
    // Footprint 20 is -25 Mitigation against its base, so a program with
    // 3 Mitigation seats a base above the player's and not a negative one.
    set_attribute(&mut game, program, "footprint", 20);
    game.world.get_mut::<Stats>(program).unwrap().mitigation = 0;
    game.world.get_mut::<Stats>(program).unwrap().max_hp = 1;
    game.world.get_mut::<Stats>(program).unwrap().hp = 1;
    let before = stats_of(&game, program);

    game.seat_derived(program);

    assert_eq!(stats_of(&game, program), before);
    assert_seated_and_stable(&mut game, program);
    assert!(game.world.get::<ProgramBase>(program).unwrap().0.mitigation > 0);
}

#[test]
fn seating_twice_changes_nothing() {
    let mut game = game();
    let program = unseated_program(&mut game);
    game.seat_derived(program);
    let base = *game.world.get::<ProgramBase>(program).unwrap();
    set_attribute(&mut game, program, "parity", 90);
    game.seat_derived(program);
    assert_eq!(*game.world.get::<ProgramBase>(program).unwrap(), base);
}

#[test]
fn a_wild_creature_is_never_seated() {
    let mut game = game();
    let s = species(&game);
    let wild = game
        .spawn_wild_creature_scaled(&s, 61, 61, 1.0, false)
        .unwrap();
    game.seat_derived(wild);
    assert!(game.world.get::<ProgramBase>(wild).is_none());
    assert!(game.world.get::<Derived>(wild).is_none());
}

#[test]
fn adopt_program_seats() {
    let mut game = game();
    let s = species(&game);
    let program = game.adopt_program(&s, 62, 62, 1.0).unwrap();
    assert_seated_and_stable(&mut game, program);
}

#[test]
fn a_starting_program_is_seated() {
    let mut game = game();
    let s = species(&game);
    game.grant_starting_program(&s).unwrap();
    let mut q = game
        .world
        .query_filtered::<Entity, (With<crate::components::Tamed>, With<ProgramBase>)>();
    assert_eq!(q.iter(&game.world).count(), 1);
}

#[test]
fn a_decompiled_body_is_seated_with_its_stats_intact() {
    let mut game = game();
    let player = game.player_entity();
    let s = species(&game);
    let wild = game
        .spawn_wild_creature_scaled(&s, 3, 3, 1.0, false)
        .unwrap();
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    let before = stats_of(&game, wild);
    insert_battle(&mut game, player, vec![wild]);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;
    for _ in 0..50 {
        if game.world.get::<crate::components::Tamed>(wild).is_some() {
            break;
        }
        player_decompiles(&mut game);
    }
    assert!(game.world.get::<crate::components::Tamed>(wild).is_some());
    assert_eq!(stats_of(&game, wild), before);
    assert_seated_and_stable(&mut game, wild);
}

#[test]
fn a_seated_high_entropy_program_crits_more() {
    let mut game = game();
    let program = unseated_program(&mut game);
    set_attribute(&mut game, program, "entropy", 90);
    game.seat_derived(program);
    let crit = game.world.get::<Derived>(program).unwrap().crit;
    assert!(crit > crate::tuning::CRIT_CHANCE, "{crit}");
}

#[test]
fn a_seated_high_persistence_program_shrugs_a_status_off_sooner() {
    let mut game = game();
    let program = unseated_program(&mut game);
    set_attribute(&mut game, program, "persistence", 80);
    game.seat_derived(program);
    game.world
        .entity_mut(program)
        .insert(crate::components::StatusEffects::default());
    game.arm_status(program, crate::components::StatusKind::Stun, 10, 0);
    let remaining = game
        .world
        .get::<crate::components::StatusEffects>(program)
        .unwrap()
        .active
        .unwrap()
        .remaining;
    assert_eq!(remaining, 7);
}

#[test]
fn an_arena_companion_levelled_after_adoption_is_stable_under_recompute() {
    let mut game = game();
    let s = species(&game);
    let program = crate::arena::spawn_companion(&mut game, &s, 6).unwrap();
    assert!(stats_of(&game, program).max_hp > 0);
    assert_seated_and_stable(&mut game, program);
}

fn seated_program(game: &mut Game) -> Entity {
    let s = species(game);
    let program = game.adopt_program(&s, 70, 70, 1.0).unwrap();
    assert!(game.world.get::<ProgramBase>(program).is_some());
    program
}

fn attribute(game: &Game, entity: Entity, id: &str) -> i32 {
    game.world
        .get::<Attributes>(entity)
        .unwrap()
        .get(&AttributeId::from(id))
        .unwrap()
}

/// What one level earns this program, from the same function the game calls.
fn one_level(game: &Game, program: Entity) -> (u32, u32) {
    let (g, roll) = game.program_growth(program);
    crate::progression::program_level_points(g, roll)
}

fn feed_one_level(game: &mut Game, program: Entity) {
    let owed = game.world.get::<Experience>(program).unwrap().xp_to_next;
    game.award_companion_xp(program, owed);
}

#[test]
fn an_unheld_level_raises_parity_and_analysis_and_heals() {
    let mut game = game();
    let program = seated_program(&mut game);
    let (parity, analysis) = one_level(&game, program);
    let (p0, a0) = (
        attribute(&game, program, "parity"),
        attribute(&game, program, "analysis"),
    );
    let max_before = stats_of(&game, program).max_hp;
    game.world.get_mut::<Stats>(program).unwrap().hp = 1;

    feed_one_level(&mut game, program);

    assert_eq!(attribute(&game, program, "parity"), p0 + parity as i32);
    assert_eq!(attribute(&game, program, "analysis"), a0 + analysis as i32);
    let after = stats_of(&game, program);
    assert!(after.max_hp > max_before);
    assert_eq!(after.hp, after.max_hp, "a level full-heals");
    let derived = game.derived_stats(program);
    assert_eq!(after.max_hp, derived.max_hp);
}

#[test]
fn a_held_level_banks_points_and_moves_no_stat() {
    let mut game = game();
    let program = seated_program(&mut game);
    game.set_hold_points(program, true).unwrap();
    let (parity, analysis) = one_level(&game, program);
    let before = stats_of(&game, program);
    let parity_before = attribute(&game, program, "parity");

    feed_one_level(&mut game, program);

    assert_eq!(
        game.world.get::<StatPoints>(program),
        Some(&StatPoints(parity + analysis))
    );
    assert_eq!(attribute(&game, program, "parity"), parity_before);
    let after = stats_of(&game, program);
    assert_eq!((after.max_hp, after.atk), (before.max_hp, before.atk));
    assert_eq!(after.hp, after.max_hp);
}

#[test]
fn turning_hold_off_spends_the_whole_bank() {
    let mut game = game();
    let program = seated_program(&mut game);
    game.set_hold_points(program, true).unwrap();
    // Two levels' worth and a remainder that is not a level.
    let (parity, analysis) = one_level(&game, program);
    let bank = 2 * (parity + analysis) + 1;
    game.world.get_mut::<StatPoints>(program).unwrap().0 = bank;
    let (p0, a0) = (
        attribute(&game, program, "parity"),
        attribute(&game, program, "analysis"),
    );

    game.set_hold_points(program, false).unwrap();

    assert_eq!(game.world.get::<StatPoints>(program), Some(&StatPoints(0)));
    assert_eq!(
        game.world.get::<HoldPoints>(program),
        Some(&HoldPoints(false))
    );
    assert_eq!(
        attribute(&game, program, "parity"),
        p0 + 2 * parity as i32 + 1,
        "the remainder lands in Parity"
    );
    assert_eq!(
        attribute(&game, program, "analysis"),
        a0 + 2 * analysis as i32
    );
    assert_eq!(
        stats_of(&game, program).max_hp,
        game.derived_stats(program).max_hp
    );
}

#[test]
fn spending_a_held_programs_points_raises_its_stats() {
    let mut game = game();
    let program = seated_program(&mut game);
    game.set_hold_points(program, true).unwrap();
    game.world.get_mut::<StatPoints>(program).unwrap().0 = 3;
    let before = stats_of(&game, program);

    game.spend_stat_points(
        StatOwner::Program(program),
        &[(AttributeId::from("parity"), 3)],
    )
    .unwrap();

    assert_eq!(game.world.get::<StatPoints>(program), Some(&StatPoints(0)));
    assert_eq!(stats_of(&game, program).max_hp, before.max_hp + 18);
}

#[test]
fn a_refused_program_spend_writes_nothing() {
    let mut game = game();
    let unseated = unseated_program(&mut game);
    let seated = seated_program(&mut game);
    game.world.get_mut::<StatPoints>(seated).unwrap().0 = 2;
    let attrs = game.attributes_of(seated);
    let stats = stats_of(&game, seated);

    assert_eq!(
        game.spend_stat_points(
            StatOwner::Program(unseated),
            &[(AttributeId::from("parity"), 1)]
        ),
        Err(SpendError::NoSuchTarget)
    );
    assert_eq!(
        game.spend_stat_points(
            StatOwner::Program(seated),
            &[(AttributeId::from("parity"), 3)]
        ),
        Err(SpendError::InsufficientPoints)
    );
    assert_eq!(game.attributes_of(seated), attrs);
    assert_eq!(stats_of(&game, seated), stats);
    assert_eq!(game.world.get::<StatPoints>(seated), Some(&StatPoints(2)));
}

#[test]
fn attention_names_programs_holding_points_and_clears_when_spent() {
    let mut game = game();
    let program = seated_program(&mut game);
    let has_row = |game: &mut Game| {
        game.attention()
            .iter()
            .any(|r| r.kind == AttentionKind::ProgramPoints)
    };
    assert!(!has_row(&mut game));
    game.set_hold_points(program, true).unwrap();
    assert!(!has_row(&mut game), "holding with nothing banked is quiet");
    game.world.get_mut::<StatPoints>(program).unwrap().0 = 2;
    assert!(has_row(&mut game));
    let row = game
        .attention()
        .into_iter()
        .find(|r| r.kind == AttentionKind::ProgramPoints)
        .unwrap();
    assert_eq!(row.text, "1 program has points to spend (Manifest)");
    game.set_hold_points(program, false).unwrap();
    assert!(!has_row(&mut game));
}

#[test]
fn a_posted_workers_level_lands_through_the_drain() {
    let mut game = game();
    stand_ample_grid_supply(&mut game);
    let worker = seated_program(&mut game);
    let (parity, analysis) = one_level(&game, worker);
    let (p0, a0) = (
        attribute(&game, worker, "parity"),
        attribute(&game, worker, "analysis"),
    );
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
            work_node_parts(),
        ))
        .id();
    *game.world.get_mut::<Position>(worker).unwrap() = Position { x: 3, y: 3 };
    {
        let mut exp = game.world.get_mut::<Experience>(worker).unwrap();
        exp.xp = exp.xp_to_next - 1;
    }
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 0,
        required: 1,
    });

    for _ in 0..3 {
        game.tick();
    }

    assert!(game.world.get::<Experience>(worker).unwrap().level >= 2);
    assert_eq!(attribute(&game, worker, "parity"), p0 + parity as i32);
    assert_eq!(attribute(&game, worker, "analysis"), a0 + analysis as i32);
    assert_eq!(
        stats_of(&game, worker).max_hp,
        game.derived_stats(worker).max_hp
    );
}

#[test]
fn an_arena_companion_levelled_after_adoption_grew_through_points() {
    let mut game = game();
    let s = species(&game);
    let level_one = crate::arena::spawn_companion(&mut game, &s, 1).unwrap();
    let program = crate::arena::spawn_companion(&mut game, &s, 6).unwrap();
    let (parity, _) = one_level(&game, program);
    assert_eq!(
        attribute(&game, program, "parity"),
        attribute(&game, level_one, "parity") + 5 * parity as i32
    );
    assert!(stats_of(&game, program).max_hp > stats_of(&game, level_one).max_hp);
    assert_eq!(
        stats_of(&game, program).max_hp,
        game.derived_stats(program).max_hp
    );
}

#[test]
fn a_fused_child_is_seated_and_stable() {
    let mut game = game();
    unlock_research_chain(&mut game, "program_refactoring");
    let a = seated_program(&mut game);
    let b = seated_program(&mut game);
    game.fuse_companions(a, b, None).unwrap();
    let child = game
        .owned_pets()
        .into_iter()
        .max_by_key(|p| p.fusions)
        .unwrap()
        .entity;
    assert_seated_and_stable(&mut game, child);
}

#[test]
fn spending_on_an_unseated_program_is_refused() {
    let mut game = game();
    let program = unseated_program(&mut game);
    let before = game.world.get::<Attributes>(program).cloned();

    let result = game.spend_stat_points(
        crate::progression::StatOwner::Program(program),
        &[(AttributeId::from("parity"), 1)],
    );

    assert_eq!(result, Err(crate::progression::SpendError::NoSuchTarget));
    assert_eq!(game.world.get::<Attributes>(program).cloned(), before);
}

#[test]
fn a_refactor_of_a_seated_program_survives_a_recompute() {
    let mut game = game();
    let program = seated_program(&mut game);
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(ItemId::from("buffer_extension"), 1);
        inv.add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    }
    game.equip(program, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
        .unwrap();
    let before = stats_of(&game, program);

    game.refactor_companion(program, &ItemId::from("buffer_extension"))
        .unwrap();

    let after = stats_of(&game, program);
    assert!(after.max_hp > before.max_hp);
    assert_eq!(after.hp, before.hp + (after.max_hp - before.max_hp));
    assert_eq!(after.atk, before.atk, "gear must not be scaled or lost");
    assert_seated_and_stable(&mut game, program);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "seated program")]
fn retiering_a_seated_program_is_refused() {
    let mut game = game();
    let program = seated_program(&mut game);
    game.retier_rarity(program, Rarity::Gold);
}

/// A real save and load, never RON alone: a skipped field is invisible to a
/// round trip through the struct.
#[test]
fn a_held_programs_derivation_survives_a_save_and_load() {
    let dir = scratch_assets_dir("derived_program_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    let mut game = game();
    let program = seated_program(&mut game);
    game.world
        .entity_mut(program)
        .insert(crate::components::KernelRing(
            crate::tuning::KERNEL_RING_MAX,
        ));
    game.set_hold_points(program, true).unwrap();
    feed_one_level(&mut game, program);
    set_level(&mut game, program, crate::tuning::TALENT_START_LEVEL + 2);
    let node = game.talent_tree(program).expect("a tree").tiers[0]
        .0
        .iter()
        .find(|c| matches!(c.node, crate::talents::TalentNode::Stat { .. }))
        .expect("the first tier offers a stat node")
        .id
        .clone();
    game.take_talent(program, &node).unwrap();
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.add(ItemId::from("buffer_extension"), 1);
        inv.add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    }
    game.equip(program, &gear(&ItemId::from(ids::OVERCLOCK_CORE), 0))
        .unwrap();
    game.refactor_companion(program, &ItemId::from("buffer_extension"))
        .unwrap();
    assert!(game.world.get::<StatPoints>(program).unwrap().0 > 0);
    assert!(game.world.get::<BoughtStats>(program).is_some());
    let id = game.world.get::<ProgramId>(program).unwrap().0;
    let snapshot = |game: &Game, e: Entity| {
        (
            stats_of(game, e),
            *game.world.get::<Derived>(e).unwrap(),
            *game.world.get::<StatPoints>(e).unwrap(),
            *game.world.get::<HoldPoints>(e).unwrap(),
            game.world.get::<ProgramBase>(e).unwrap().0,
            game.world.get::<Attributes>(e).cloned(),
            (game.gear_bonus(e).atk, game.gear_bonus(e).mitigation),
            game.world.get::<BoughtStats>(e).copied(),
            game.world.get::<Talents>(e).map(|t| t.0.clone()),
        )
    };
    let before = snapshot(&game, program);
    let record = game.creature_save_for(program).unwrap();
    assert_eq!(
        (record.max_hp, record.atk, record.mitigation),
        (0, 0, 0),
        "a seated program's derived figures are not written"
    );
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded.world.query::<(Entity, &ProgramId)>();
    let restored = query
        .iter(&loaded.world)
        .find(|(_, p)| p.0 == id)
        .map(|(e, _)| e)
        .expect("the program survives");

    assert_eq!(snapshot(&loaded, restored), before);
}

#[test]
fn the_manifest_reports_a_seated_programs_points_and_hold() {
    let mut game = game();
    let program = seated_program(&mut game);
    game.set_hold_points(program, true).unwrap();
    game.world.get_mut::<StatPoints>(program).unwrap().0 = 3;

    let points = program_manifest(&game, program)
        .points
        .expect("a seated program reports its points");
    assert_eq!((points.banked, points.holding), (3, true));
}

#[test]
fn an_unseated_program_reports_no_points() {
    let mut game = game();
    let program = unseated_program(&mut game);
    assert!(program_manifest(&game, program).points.is_none());
}
