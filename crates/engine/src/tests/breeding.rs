//! Breeding: the species rule, inheritance, and `Game::breed`'s lifecycle.

use super::support::*;
use crate::attributes::{AttributeDb, AttributeId};
use crate::breeding::{ParentRolls, child_species, inherit};
use crate::tuning::*;
use crate::views::BreedSpeciesPreview;
use crate::*;
use bevy_ecs::prelude::{With, Without};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn species_db() -> SpeciesDb {
    let (abilities, _) =
        crate::abilities::AbilityDb::load_dir(&test_assets_dir().join("abilities")).unwrap();
    SpeciesDb::load_dir(&test_assets_dir().join("species"), &abilities)
        .unwrap()
        .0
}

fn attribute_db() -> AttributeDb {
    AttributeDb::load_dir(&test_assets_dir().join("attributes"))
        .unwrap()
        .0
}

fn rolls(roll: f32, generation: u32, attribute: i32) -> ParentRolls {
    let db = attribute_db();
    ParentRolls {
        potential: Potential {
            hp_roll: roll,
            atk_roll: roll,
            def_roll: roll,
            growth_roll: roll,
            assembly_roll: roll,
            extraction_roll: roll,
        },
        attributes: db.iter().map(|d| (d.id.clone(), attribute)).collect(),
        generation,
    }
}

fn all_rolls(p: &Potential) -> [f32; 6] {
    [
        p.hp_roll,
        p.atk_roll,
        p.def_roll,
        p.growth_roll,
        p.assembly_roll,
        p.extraction_roll,
    ]
}

/// Every child of a draw loop, so a clamp that only bites on some seeds is
/// still seen biting.
fn children(a: &ParentRolls, b: &ParentRolls, species: &str) -> Vec<crate::breeding::ChildRolls> {
    let db = species_db();
    let def = db.get(species).unwrap();
    let attrs = attribute_db();
    let mut rng = StdRng::seed_from_u64(7);
    (0..300)
        .map(|_| inherit(a, b, def, &attrs, &mut rng))
        .collect()
}

#[test]
fn a_pair_of_one_species_breeds_true() {
    let db = species_db();
    let mut rng = StdRng::seed_from_u64(1);
    assert_eq!(child_species("worm", "worm", &db, &mut rng), "worm");
}

#[test]
fn an_authored_pair_breeds_its_hybrid_in_either_order() {
    let db = species_db();
    let mut rng = StdRng::seed_from_u64(1);
    for _ in 0..20 {
        assert_eq!(child_species("worm", "virus", &db, &mut rng), "botnet");
        assert_eq!(child_species("virus", "worm", &db, &mut rng), "botnet");
    }
}

#[test]
fn an_unauthored_pair_yields_either_parent_and_both_turn_up() {
    let db = species_db();
    let mut rng = StdRng::seed_from_u64(1);
    let kinds: std::collections::BTreeSet<String> = (0..60)
        .map(|_| child_species("drone", "scrapper", &db, &mut rng))
        .collect();
    assert_eq!(
        kinds,
        ["drone", "scrapper"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    );
}

#[test]
fn a_child_is_one_generation_past_its_deeper_parent() {
    let kids = children(&rolls(1.0, 2, 50), &rolls(1.0, 5, 50), "worm");
    assert!(kids.iter().all(|k| k.generation == 6));
}

#[test]
fn a_child_takes_the_better_roll_within_the_mutation() {
    let kids = children(&rolls(0.9, 0, 50), &rolls(1.1, 0, 50), "worm");
    for k in &kids {
        for r in all_rolls(&k.potential) {
            assert!(
                (1.1 - BREEDING_MUTATION - 1e-5..=1.1 + BREEDING_MUTATION + 1e-5).contains(&r),
                "{r}"
            );
        }
    }
    let spread: Vec<f32> = kids.iter().map(|k| k.potential.hp_roll).collect();
    let (lo, hi) = spread
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), r| (l.min(*r), h.max(*r)));
    assert!(hi - lo > BREEDING_MUTATION, "the mutation should show");
}

#[test]
fn a_roll_cannot_pass_its_generations_ceiling() {
    let kids = children(&rolls(1.4, 0, 50), &rolls(1.4, 0, 50), "worm");
    let ceiling = MAX_INDIVIDUAL_ROLL + BREEDING_GEN_STEP;
    for k in &kids {
        for r in all_rolls(&k.potential) {
            assert!(r <= ceiling + 1e-5, "{r} > {ceiling}");
        }
    }
    assert!(
        kids.iter()
            .any(|k| (k.potential.hp_roll - ceiling).abs() < 1e-5),
        "the clamp should be reached"
    );
}

#[test]
fn a_deep_lineage_stops_at_the_hard_cap() {
    let kids = children(&rolls(1.4, 30, 50), &rolls(1.4, 30, 50), "worm");
    for k in &kids {
        for r in all_rolls(&k.potential) {
            assert!(r <= BREEDING_ROLL_HARD_CAP + 1e-5, "{r}");
        }
    }
    assert!(
        kids.iter()
            .any(|k| (k.potential.hp_roll - BREEDING_ROLL_HARD_CAP).abs() < 1e-5)
    );
}

#[test]
fn a_roll_cannot_fall_below_the_floor() {
    let kids = children(&rolls(0.8, 0, 50), &rolls(0.8, 0, 50), "worm");
    for k in &kids {
        for r in all_rolls(&k.potential) {
            assert!(r >= MIN_INDIVIDUAL_ROLL - 1e-5, "{r}");
        }
    }
    assert!(
        kids.iter()
            .any(|k| (k.potential.hp_roll - MIN_INDIVIDUAL_ROLL).abs() < 1e-5)
    );
}

/// The wild range of `id` for `species`, read as `mint` reads it.
fn wild_range(species: &str, id: &str) -> (i32, i32) {
    let db = attribute_db();
    let def = db.get(&AttributeId::from(id)).unwrap();
    let sp = species_db();
    let base = sp
        .get(species)
        .unwrap()
        .attributes
        .get(id)
        .copied()
        .unwrap_or(def.base);
    (base - def.spread, base + def.spread)
}

#[test]
fn trained_attribute_points_do_not_pass_down_past_the_clamp() {
    // A parent that spent a great many level-up points.
    let kids = children(&rolls(1.0, 0, 500), &rolls(1.0, 0, 500), "worm");
    for k in &kids {
        assert_eq!(k.generation, 1);
        for (id, value) in &k.attributes {
            let (_, top) = wild_range("worm", id.as_str());
            assert!(*value <= top + 1, "{id}: {value} > {}", top + 1);
        }
    }
    assert!(
        kids.iter().any(|k| k
            .attributes
            .iter()
            .any(|(id, v)| *v == wild_range("worm", id.as_str()).1 + 1)),
        "the lift should be reached"
    );
}

#[test]
fn an_attribute_lift_stops_at_its_hard_cap() {
    let kids = children(&rolls(1.0, 40, 500), &rolls(1.0, 40, 500), "worm");
    for k in &kids {
        for (id, value) in &k.attributes {
            let (_, top) = wild_range("worm", id.as_str());
            assert!(*value <= top + BREEDING_ATTRIBUTE_HARD_CAP as i32);
        }
    }
}

#[test]
fn an_attribute_cannot_fall_below_the_wild_floor() {
    let kids = children(&rolls(1.0, 0, -500), &rolls(1.0, 0, -500), "worm");
    for k in &kids {
        for (id, value) in &k.attributes {
            let (low, _) = wild_range("worm", id.as_str());
            assert!(*value >= low, "{id}: {value} < {low}");
        }
    }
}

#[test]
fn the_better_parents_attribute_is_the_one_inherited() {
    // Mid-range values, so no clamp interferes.
    let db = attribute_db();
    let id = db.iter().next().unwrap().id.clone();
    let (low, high) = wild_range("worm", id.as_str());
    let (mid_low, mid_high) = (low + 1, (low + high) / 2);
    let mut a = rolls(1.0, 0, mid_low);
    let b = rolls(1.0, 0, mid_high);
    a.attributes.insert(id.clone(), mid_low);
    for k in children(&a, &b, "worm") {
        let v = k.attributes[&id];
        assert!((mid_high - 1..=mid_high + 1).contains(&v), "{v}");
    }
}

// ---- Game::breed ----

use crate::breeding::BreedRefusal;
use crate::components::{BreedReadyAt, Generation, Incubator};

fn seed() -> ItemId {
    ItemId::from("breeding_seed")
}

/// A base with a built bay, two owned programs of the given species and
/// `seeds` breeding seeds in the pack.
fn bay_game(a: &str, b: &str, seeds: u32) -> (Game, Entity, Entity, Entity) {
    let mut game = Game::new(5150, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let bay = spawn_machine_at(&mut game, "breeding_bay", 3, 3);
    let pa = game.adopt_program(a, 0, 0, 1.0).unwrap();
    let pb = game.adopt_program(b, 0, 0, 1.0).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(seed(), seeds);
    (game, pa, pb, bay)
}

fn seeds_in_pack(game: &Game) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&seed())
}

fn now(game: &Game) -> u64 {
    game.world.resource::<GameClock>().tick
}

/// Everything a refused breeding must leave alone.
fn snapshot(
    game: &Game,
    a: Entity,
    b: Entity,
    bay: Entity,
) -> (u32, Option<u64>, Option<u64>, Incubator) {
    (
        seeds_in_pack(game),
        game.world.get::<BreedReadyAt>(a).map(|r| r.0),
        game.world.get::<BreedReadyAt>(b).map(|r| r.0),
        game.world.get::<Incubator>(bay).cloned().unwrap(),
    )
}

#[test]
fn a_built_bay_carries_one_empty_slot() {
    let (game, _, _, bay) = bay_game("worm", "worm", 1);
    assert_eq!(game.world.get::<Incubator>(bay).unwrap().slots, vec![None]);
}

#[test]
fn breeding_spends_a_seed_rests_both_parents_and_fills_the_slot() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 2);
    let started = now(&game);
    game.breed(a, b, bay).unwrap();

    assert_eq!(seeds_in_pack(&game), 1);
    for parent in [a, b] {
        assert_eq!(
            game.world.get::<BreedReadyAt>(parent).unwrap().0,
            started + BREEDING_COOLDOWN_TICKS
        );
    }
    let child = game.world.get::<Incubator>(bay).unwrap().slots[0]
        .clone()
        .expect("the slot should hold the child");
    assert_eq!(child.species, "botnet", "worm x virus is an authored pair");
    assert_eq!(child.generation, 1);
    assert_eq!(child.due, started + INCUBATION_TICKS);
    assert!(!child.attributes.is_empty());
}

#[test]
fn a_boss_cannot_be_a_parent() {
    let (mut game, a, _, bay) = bay_game("worm", "worm", 1);
    let boss = game.adopt_program("overseer", 0, 0, 1.0).unwrap();
    assert_eq!(game.breed_refusal(boss), Some(BreedRefusal::Boss));
    assert_eq!(game.breed(a, boss, bay), Err(BreedRefusal::Boss));
    assert_eq!(game.breed(boss, a, bay), Err(BreedRefusal::Boss));
    assert_eq!(seeds_in_pack(&game), 1);
}

#[test]
fn a_childs_generation_follows_its_deeper_parent() {
    let (mut game, a, b, bay) = bay_game("worm", "worm", 1);
    game.world.entity_mut(b).insert(Generation(4));
    game.breed(a, b, bay).unwrap();
    let child = game.world.get::<Incubator>(bay).unwrap().slots[0]
        .clone()
        .unwrap();
    assert_eq!(child.generation, 5);
}

#[test]
fn every_refusal_leaves_the_game_as_it_was() {
    // Same program twice.
    let (mut game, a, b, bay) = bay_game("worm", "worm", 1);
    let before = snapshot(&game, a, b, bay);
    assert_eq!(game.breed(a, a, bay), Err(BreedRefusal::SameProgram));
    assert_eq!(snapshot(&game, a, b, bay), before);

    // A program the player does not own.
    let wild = game.spawn_wild_creature("worm", 1, 1).unwrap();
    assert_eq!(game.breed(a, wild, bay), Err(BreedRefusal::NotYours));
    assert_eq!(snapshot(&game, a, b, bay), before);

    // Something that is not a bay.
    let other = spawn_machine_at(&mut game, "quarantine_rack", 6, 6);
    assert_eq!(game.breed(a, b, other), Err(BreedRefusal::NoFreeSlot));
    assert_eq!(snapshot(&game, a, b, bay), before);

    // No seed.
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(seed(), 1);
    let before = snapshot(&game, a, b, bay);
    assert_eq!(game.breed(a, b, bay), Err(BreedRefusal::NoSeed));
    assert_eq!(snapshot(&game, a, b, bay), before);
}

#[test]
fn a_parent_on_cooldown_is_refused_until_it_has_rested() {
    let (mut game, a, b, bay) = bay_game("worm", "worm", 3);
    let c = game.adopt_program("worm", 0, 0, 1.0).unwrap();
    game.breed(a, b, bay).unwrap();
    // Free the slot by hand so only the cooldown is in the way.
    game.world.get_mut::<Incubator>(bay).unwrap().slots[0] = None;

    let before = snapshot(&game, a, c, bay);
    assert_eq!(game.breed(a, c, bay), Err(BreedRefusal::OnCooldown));
    assert_eq!(snapshot(&game, a, c, bay), before);
    assert_eq!(game.breed_refusal(a), Some(BreedRefusal::OnCooldown));
    assert_eq!(game.breed_refusal(c), None);

    game.world.resource_mut::<GameClock>().tick += BREEDING_COOLDOWN_TICKS;
    assert_eq!(game.breed_refusal(a), None);
    game.breed(a, c, bay).unwrap();
}

#[test]
fn a_full_bay_refuses_a_second_breeding() {
    let (mut game, a, b, bay) = bay_game("worm", "worm", 2);
    let c = game.adopt_program("worm", 0, 0, 1.0).unwrap();
    let d = game.adopt_program("worm", 0, 0, 1.0).unwrap();
    game.breed(a, b, bay).unwrap();
    let before = snapshot(&game, c, d, bay);
    assert_eq!(game.breed(c, d, bay), Err(BreedRefusal::NoFreeSlot));
    assert_eq!(snapshot(&game, c, d, bay), before);
}

#[test]
fn breed_refusal_reports_a_program_that_is_not_the_players() {
    let (mut game, a, _, _) = bay_game("worm", "worm", 1);
    let wild = game.spawn_wild_creature("worm", 1, 1).unwrap();
    assert_eq!(game.breed_refusal(a), None);
    assert_eq!(game.breed_refusal(wild), Some(BreedRefusal::NotYours));
}

#[test]
fn the_preview_names_a_certain_hybrid_and_calls_the_clamp_for_its_rolls() {
    let (game, a, b, _) = bay_game("worm", "virus", 1);
    let preview = game.breed_preview(a, b).unwrap();
    assert_eq!(
        preview.species,
        BreedSpeciesPreview::Certain("Botnet".to_string())
    );
    assert_eq!(preview.generation, 1);
    assert_eq!(preview.rolls.len(), 6);
    let pa = *game.world.get::<Potential>(a).unwrap();
    let pb = *game.world.get::<Potential>(b).unwrap();
    let (lo, hi) = crate::breeding::roll_span(pa.hp_roll.max(pb.hp_roll), 1);
    let hp = &preview.rolls[0];
    assert_eq!((hp.min, hp.max), (lo, hi));
}

#[test]
fn the_preview_of_an_unauthored_pair_names_both_possibilities() {
    let (game, a, b, _) = bay_game("drone", "scrapper", 1);
    let preview = game.breed_preview(a, b).unwrap();
    assert!(matches!(
        preview.species,
        BreedSpeciesPreview::OneOf(ref x, ref y) if x != y
    ));
}

#[test]
fn incubations_lists_one_row_per_slot_with_the_child_while_it_grows() {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    let empty = game.incubations(bay);
    assert_eq!(empty.len(), 1);
    assert!(empty[0].child.is_none());

    game.breed(a, b, bay).unwrap();
    let rows = game.incubations(bay);
    let child = rows[0].child.as_ref().unwrap();
    assert_eq!(child.species, "Botnet");
    assert_eq!(child.generation, 1);
    assert_eq!(child.ticks_left, INCUBATION_TICKS);
    assert!(!child.held);
}

// ---- hatching ----

use crate::components::{ProgramBase, ProgramId, Routines};

fn incubating_game() -> (Game, Entity, Entity, Entity) {
    let (mut game, a, b, bay) = bay_game("worm", "virus", 1);
    game.breed(a, b, bay).unwrap();
    (game, a, b, bay)
}

fn make_due(game: &mut Game) {
    game.world.resource_mut::<GameClock>().tick += INCUBATION_TICKS;
}

/// Fills the roster to its hard cap with bare tamed bodies.
fn fill_the_roster(game: &mut Game) {
    let player = game.player_entity();
    while game.roster_room() > 0 {
        game.world.spawn(Tamed { owner: player });
    }
}

#[test]
fn a_child_stays_in_the_bay_until_it_is_due() {
    let (mut game, _, _, bay) = incubating_game();
    let before = game.pet_count();
    game.world.resource_mut::<GameClock>().tick += INCUBATION_TICKS - 1;
    game.hatch_incubations();
    assert_eq!(game.pet_count(), before);
    assert!(game.world.get::<Incubator>(bay).unwrap().slots[0].is_some());
}

#[test]
fn a_due_child_hatches_onto_the_roster_with_the_rolls_recorded_at_the_start() {
    let (mut game, _, _, bay) = incubating_game();
    let child = game.world.get::<Incubator>(bay).unwrap().slots[0]
        .clone()
        .unwrap();
    let before = game.pet_count();
    make_due(&mut game);
    game.hatch_incubations();
    // Everything a hatchling is was drawn when the breeding started, so a
    // twin game that never hatches is at the same point in the RNG stream.
    let (mut twin, ..) = incubating_game();
    make_due(&mut twin);
    assert_eq!(
        game.world.resource_mut::<GameRng>().0.random::<u64>(),
        twin.world.resource_mut::<GameRng>().0.random::<u64>(),
        "hatching must not draw from the game RNG"
    );

    assert_eq!(game.pet_count(), before + 1);
    assert!(game.world.get::<Incubator>(bay).unwrap().slots[0].is_none());

    let hatched = {
        let mut query = game
            .world
            .query::<(Entity, &Creature, &Generation, &Tamed)>();
        let found: Vec<Entity> = query
            .iter(&game.world)
            .filter(|(_, c, g, _)| c.species == child.species && g.0 == child.generation)
            .map(|(e, ..)| e)
            .collect();
        assert_eq!(found.len(), 1);
        found[0]
    };
    assert_eq!(
        *game.world.get::<Potential>(hatched).unwrap(),
        child.potential
    );
    let attrs: std::collections::BTreeMap<_, _> = game
        .world
        .get::<crate::components::Attributes>(hatched)
        .unwrap()
        .iter()
        .map(|(id, v)| (id.clone(), v))
        .collect();
    assert_eq!(attrs, child.attributes);
    assert_eq!(game.world.get::<Experience>(hatched).unwrap().level, 1);
    assert_eq!(
        *game.world.get::<Rarity>(hatched).unwrap(),
        Rarity::Ordinary
    );
    // Through `roster_parts`, `install_innate_routines` and `seat_derived`.
    assert!(game.world.get::<ProgramId>(hatched).is_some());
    assert!(game.world.get::<ProgramBase>(hatched).is_some());
    assert!(!game.world.get::<Routines>(hatched).unwrap().0.is_empty());
    // The same `round(base * roll)` a wild spawn applies at zone 1, with no
    // other factor in it.
    let def = game
        .world
        .resource::<SpeciesDb>()
        .get(&child.species)
        .cloned()
        .unwrap();
    let stats = game.world.get::<Stats>(hatched).unwrap();
    let expect = |base: i32, roll: f32| ((base as f32) * roll).round() as i32;
    assert_eq!(stats.max_hp, expect(def.base_hp, child.potential.hp_roll));
    assert_eq!(stats.hp, stats.max_hp);
    assert_eq!(stats.atk, expect(def.base_atk, child.potential.atk_roll));
    assert!(game.world.get::<BreedReadyAt>(hatched).is_none());
}

#[test]
fn hatching_logs_one_line_naming_the_species_and_generation() {
    let (mut game, _, _, _) = incubating_game();
    make_due(&mut game);
    let before = game.message_log(1000).len();
    game.hatch_incubations();
    let lines: Vec<String> = game
        .message_log(1000)
        .into_iter()
        .skip(before)
        .map(|l| l.text)
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("Botnet") && lines[0].contains("gen 1"),
        "{lines:?}"
    );
}

#[test]
fn a_hatch_runs_from_the_game_tick() {
    let (mut game, _, _, bay) = incubating_game();
    make_due(&mut game);
    game.tick();
    assert!(game.world.get::<Incubator>(bay).unwrap().slots[0].is_none());
}

#[test]
fn a_full_roster_holds_the_child_in_the_bay_and_it_hatches_when_there_is_room() {
    let (mut game, _, _, bay) = incubating_game();
    fill_the_roster(&mut game);
    make_due(&mut game);
    game.hatch_incubations();
    assert!(game.world.get::<Incubator>(bay).unwrap().slots[0].is_some());
    assert!(game.incubations(bay)[0].child.as_ref().unwrap().held);

    // Make room by releasing one filler body.
    let filler = {
        let mut query = game
            .world
            .query_filtered::<Entity, (With<Tamed>, Without<Creature>)>();
        query.iter(&game.world).next().unwrap()
    };
    game.world.despawn(filler);
    game.hatch_incubations();
    assert!(game.world.get::<Incubator>(bay).unwrap().slots[0].is_none());
    assert_eq!(game.roster_room(), 0);
}

// ---- demolition and parents ----

#[test]
fn a_bay_with_a_child_in_it_cannot_be_demolished() {
    let (mut game, _, _, bay) = incubating_game();
    let err = game.remove_structure(bay).unwrap_err();
    assert!(err.contains("incubating"), "{err}");
    assert!(game.world.get::<Structure>(bay).is_some());
    assert!(game.world.get::<Incubator>(bay).unwrap().slots[0].is_some());
}

#[test]
fn a_raided_bay_loses_its_child_and_says_so() {
    let (mut game, _, _, bay) = incubating_game();
    // `spawn_machine_at` attaches nothing raidable.
    game.world
        .entity_mut(bay)
        .insert(Durability { hp: 1, max_hp: 1 });
    let before = game.world.resource::<MessageLog>().lines.len();
    game.damage_structure(bay, u32::MAX, "The Breeding Bay", "a siege");
    assert!(game.world.get::<Structure>(bay).is_none());
    let said: Vec<String> = game.world.resource::<MessageLog>().lines[before..]
        .iter()
        .map(|e| e.text.clone())
        .collect();
    assert!(
        said.iter()
            .any(|l| l.contains("Botnet") && l.contains("gen 1") && l.contains("lost")),
        "one line should name the lost child, got {said:?}"
    );
}

#[test]
fn a_raided_empty_bay_is_silent_about_children() {
    let (mut game, _, _, bay) = bay_game("worm", "virus", 1);
    game.world
        .entity_mut(bay)
        .insert(Durability { hp: 1, max_hp: 1 });
    let before = game.world.resource::<MessageLog>().lines.len();
    game.damage_structure(bay, u32::MAX, "The Breeding Bay", "a siege");
    assert!(
        !game.world.resource::<MessageLog>().lines[before..]
            .iter()
            .any(|e| e.text.contains("gen ")),
    );
}

#[test]
fn an_empty_bay_demolishes_and_so_does_one_after_the_child_hatches() {
    let (mut game, _, _, bay) = incubating_game();
    make_due(&mut game);
    game.hatch_incubations();
    game.remove_structure(bay).unwrap();
    assert!(game.world.get::<Structure>(bay).is_none());
}

#[test]
fn demolishing_the_home_is_refused_while_any_bay_holds_a_child() {
    let (mut game, _, _, bay) = incubating_game();
    let home = spawn_machine_at(&mut game, crate::HOME_STRUCTURE_ID, 9, 9);
    assert!(game.remove_structure(home).is_err());
    assert!(game.world.get::<Structure>(home).is_some());
    assert!(game.world.get::<Structure>(bay).is_some());
}

#[test]
fn parents_stay_usable_while_the_child_incubates() {
    let (mut game, a, b, bay) = incubating_game();
    // Join the party, and be fused, mid-incubation.
    enlist(&mut game, a);
    assert!(game.world.resource::<Party>().0.contains(&a));
    unlock_research_chain(&mut game, "program_refactoring");
    game.fuse_companions(a, b, None).unwrap();
    // The bay still holds its child and the parents' absence is harmless.
    make_due(&mut game);
    game.hatch_incubations();
    assert!(game.world.get::<Incubator>(bay).unwrap().slots[0].is_none());
}

// ---- save and load ----

/// A RON round trip cannot see a skipped field, so breeding state is
/// asserted through a real save and load.
fn save_and_load(game: &mut Game, tag: &str) -> Game {
    let path =
        std::env::temp_dir().join(format!("feral_breeding_{tag}_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    loaded
}

#[test]
fn a_retune_to_no_slots_does_not_eat_a_stored_child() {
    let (mut game, _, _, _) = incubating_game();
    let path =
        std::env::temp_dir().join(format!("feral_breeding_noslots_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let shrunk = assets_dir_with_extra_structure(
        "bay-no-slots",
        "breeding_bay.ron",
        r#"(id: "breeding_bay", name: "Breeding Bay", description: "x", glyph: 'N',
            color: Green, build_cost: [("core_fragment", 14)], incubation_slots: 0)"#,
    );
    let mut loaded = Game::load(&path, &shrunk).unwrap();
    let _ = std::fs::remove_file(&path);
    let bay = loaded_bay(&mut loaded);
    assert!(loaded.world.get::<Incubator>(bay).unwrap().slots[0].is_some());
}

fn loaded_bay(game: &mut Game) -> Entity {
    let mut query = game.world.query::<(Entity, &Incubator)>();
    query.iter(&game.world).next().expect("the bay stands").0
}

#[test]
fn a_child_saved_mid_incubation_hatches_with_the_recorded_rolls() {
    let (mut game, _, _, bay) = incubating_game();
    let child = game.world.get::<Incubator>(bay).unwrap().slots[0]
        .clone()
        .unwrap();
    let mut loaded = save_and_load(&mut game, "incubating");
    let bay = loaded_bay(&mut loaded);
    assert_eq!(
        loaded.world.get::<Incubator>(bay).unwrap().slots,
        vec![Some(child.clone())],
        "the bay keeps its stored slot, not a fresh empty one"
    );

    let before = loaded.pet_count();
    make_due(&mut loaded);
    loaded.hatch_incubations();
    assert_eq!(loaded.pet_count(), before + 1);

    let mut query = loaded.world.query::<(
        &Creature,
        &Generation,
        &Potential,
        &crate::components::Attributes,
    )>();
    let (_, generation, potential, attributes) = query
        .iter(&loaded.world)
        .find(|(c, g, ..)| c.species == child.species && g.0 == child.generation)
        .expect("the hatched child");
    assert_eq!(generation.0, child.generation);
    assert_eq!(*potential, child.potential);
    let attrs: std::collections::BTreeMap<_, _> =
        attributes.iter().map(|(id, v)| (id.clone(), v)).collect();
    assert_eq!(attrs, child.attributes);
}

#[test]
fn generation_and_breeding_cooldown_survive_a_save() {
    let (mut game, a, _, _) = bay_game("worm", "virus", 1);
    game.world
        .entity_mut(a)
        .insert((Generation(3), BreedReadyAt(777)));
    let mut loaded = save_and_load(&mut game, "cooldown");
    let mut query = loaded
        .world
        .query::<(&Generation, &BreedReadyAt, &Creature)>();
    let rows: Vec<_> = query
        .iter(&loaded.world)
        .map(|(g, r, c)| (g.0, r.0, c.species.clone()))
        .collect();
    assert_eq!(rows, vec![(3, 777, "worm".to_string())]);
}
