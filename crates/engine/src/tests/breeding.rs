//! Breeding: the species rule, inheritance, and `Game::breed`'s lifecycle.

use super::support::*;
use crate::attributes::{AttributeDb, AttributeId};
use crate::breeding::{ParentRolls, child_species, inherit};
use crate::tuning::*;
use crate::*;
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
