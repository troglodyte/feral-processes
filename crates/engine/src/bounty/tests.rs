use rand::SeedableRng;
use rand::rngs::StdRng;

use super::*;
use crate::contracts::{ContractDb, ContractDef, ContractId, Objective, Reward, TemplateObjective};
use crate::resources::ActiveContract;
use crate::tests::support::test_assets_dir;
use crate::tuning::*;

fn input(zone: u32) -> BountyInput {
    BountyInput {
        zone,
        habitat: ["crawler", "drone", "glitch", "worm_swarm"]
            .map(String::from)
            .to_vec(),
        anchor_candidates: (0..50).map(|i| (30 + i, -40 + i)).collect(),
    }
}

fn rolled(zone: u32, seed: u64) -> ContractDef {
    roll(&input(zone), &mut StdRng::seed_from_u64(seed))
        .expect("a sector 2+ input with a habitat and anchors rolls")
        .def
}

fn quarry(def: &ContractDef) -> &BountyQuarry {
    match &def.objective {
        Objective::Bounty(q) => q,
        other => panic!("not a bounty: {other:?}"),
    }
}

fn credits_of(rewards: &[Reward]) -> u32 {
    rewards
        .iter()
        .find_map(|r| {
            if let Reward::Credits(n) = r {
                Some(*n)
            } else {
                None
            }
        })
        .expect("pays credits")
}

fn xp_of(rewards: &[Reward]) -> u32 {
    rewards
        .iter()
        .find_map(|r| {
            if let Reward::Xp(n) = r {
                Some(*n)
            } else {
                None
            }
        })
        .expect("pays XP")
}

#[test]
fn the_same_input_and_seed_roll_the_same_bounty() {
    assert_eq!(rolled(3, 7), rolled(3, 7));
}

#[test]
fn a_different_seed_rolls_a_different_bounty() {
    let first = rolled(3, 1);
    let differs = (2..40).any(|seed| {
        let other = rolled(3, seed);
        quarry(&other).name != quarry(&first).name
            || quarry(&other).species != quarry(&first).species
    });
    assert!(differs, "forty seeds all rolled the same name and species");
}

#[test]
fn escort_and_pay_never_fall_with_the_zone_and_are_higher_at_zone_6_than_2_at_one_grade() {
    for seed in 0..10 {
        let mut prev: Option<(u32, u32, u32)> = None;
        for zone in BOUNTY_MIN_ZONE..=10 {
            let def = rolled(zone, seed);
            let q = quarry(&def);
            let now = (q.escort_size(), credits_of(&def.reward), xp_of(&def.reward));
            if let Some(p) = prev {
                assert!(
                    now.0 >= p.0 && now.1 >= p.1 && now.2 >= p.2,
                    "zone {zone} fell from {p:?} to {now:?}"
                );
            }
            prev = Some(now);
        }
        let (low, high) = (rolled(2, seed), rolled(6, seed));
        let (lq, hq) = (quarry(&low), quarry(&high));
        assert_eq!(hq.grade_pct, lq.grade_pct);
        assert!(hq.escort_size() > lq.escort_size());
        assert!(hq.rarity >= lq.rarity);
        assert!(credits_of(&high.reward) > credits_of(&low.reward));
        assert!(xp_of(&high.reward) > xp_of(&low.reward));
    }
}

#[test]
fn no_bounty_below_the_minimum_zone_or_without_habitat_or_anchors() {
    let mut rng = StdRng::seed_from_u64(1);
    for zone in 0..BOUNTY_MIN_ZONE {
        assert!(roll(&input(zone), &mut rng).is_none(), "zone {zone}");
    }
    let no_habitat = BountyInput {
        habitat: vec![],
        ..input(4)
    };
    assert!(roll(&no_habitat, &mut rng).is_none());
    let no_anchor = BountyInput {
        anchor_candidates: vec![],
        ..input(4)
    };
    assert!(roll(&no_anchor, &mut rng).is_none());
}

#[test]
fn the_description_names_the_target_and_its_species() {
    for seed in 0..20 {
        let def = rolled(3, seed);
        let q = quarry(&def);
        assert!(def.description.contains(&q.name), "{}", def.description);
        assert!(
            def.description.contains(&q.species.replace('_', " ")),
            "{}",
            def.description
        );
        assert!(def.name.contains(&q.name));
    }
}

#[test]
fn the_quarry_is_placed_from_the_candidates_with_a_sorted_escort_of_the_habitat() {
    let inp = input(5);
    for seed in 0..20 {
        let def = roll(&inp, &mut StdRng::seed_from_u64(seed)).unwrap().def;
        let q = quarry(&def);
        assert!(inp.anchor_candidates.contains(&q.anchor));
        assert!(inp.habitat.contains(&q.species));
        assert_eq!(q.tether, BOUNTY_TETHER);
        assert!(
            q.escort.windows(2).all(|w| w[0].0 < w[1].0),
            "{:?}",
            q.escort
        );
        assert!(
            q.escort
                .iter()
                .all(|(s, n)| inp.habitat.contains(s) && *n > 0)
        );
        assert_eq!(
            q.escort_size(),
            BOUNTY_ESCORT_BASE + BOUNTY_ESCORT_PER_ZONE * (5 - BOUNTY_MIN_ZONE)
        );
    }
}

#[test]
fn rolled_ids_carry_the_rolled_separator_and_differ_between_seeds() {
    let a = rolled(3, 1);
    let b = rolled(3, 2);
    assert!(
        a.id.as_str()
            .contains(crate::contracts::ROLLED_ID_SEPARATOR)
    );
    assert_ne!(a.id, b.id);
}

#[test]
fn a_bounty_pays_clearly_more_than_a_hunt_at_any_zone() {
    let (db, warnings) = ContractDb::load_dir(&test_assets_dir().join("contracts")).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let hunt = db
        .templates()
        .find(|t| t.id.as_str() == "hunt")
        .expect("the shipped hunt template");
    let TemplateObjective::Terminate { count } = &hunt.objective else {
        panic!("hunt is not a Terminate template");
    };
    let hunt_credits = credits_of(&hunt.reward) * count.1;
    let hunt_xp = xp_of(&hunt.reward) * count.1;
    for zone in BOUNTY_MIN_ZONE..=12 {
        let def = rolled(zone, 3);
        assert!(
            credits_of(&def.reward) >= hunt_credits * 2,
            "zone {zone}: {} credits vs a hunt's {hunt_credits}",
            credits_of(&def.reward)
        );
        assert!(
            xp_of(&def.reward) >= hunt_xp * 2,
            "zone {zone}: {} XP vs a hunt's {hunt_xp}",
            xp_of(&def.reward)
        );
    }
}

#[test]
fn an_authored_bounty_file_is_skipped_with_a_warning_and_the_rest_still_load() {
    let dir = std::env::temp_dir().join(format!("fp_bounty_authored_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("bounty.ron"),
        r#"(id: "sneaky", name: "Sneaky", description: "d",
            objective: Bounty((name: "Vesk the Long Wire", species: "crawler",
                grade_pct: 200, rarity: Gold, escort: [("drone", 2)],
                anchor: (30, 30), tether: 6)),
            reward: [Credits(999)])"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("fine.ron"),
        r#"(id: "fine", name: "Fine", description: "d",
            objective: Terminate(species: None, count: 3),
            reward: [Credits(5)])"#,
    )
    .unwrap();
    let (db, warnings) = ContractDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("Bounty"), "{}", warnings[0]);
    assert_eq!(db.iter().count(), 1);
    assert!(db.get(&"fine".into()).is_some());
}

#[test]
fn a_bounty_survives_a_ron_round_trip_inside_an_active_contract() {
    let held = ActiveContract {
        def: rolled(4, 9),
        progress: 0,
        accepted_tick: 12,
        issuer: None,
    };
    let text = ron::to_string(&held).unwrap();
    let back: ActiveContract = ron::from_str(&text).unwrap();
    assert_eq!(back, held);
}

#[test]
fn the_name_banks_hold_no_occult_words() {
    let banned = [
        "demon", "devil", "satan", "hell", "occult", "ritual", "cult", "witch", "curse", "doom",
        "soul", "blood", "death",
    ];
    for seed in 0..200 {
        let name = quarry(&rolled(3, seed)).name.to_lowercase();
        assert!(
            !banned.iter().any(|b| name.contains(b)),
            "{name} reads as occult"
        );
    }
}

fn ids(names: &[&str]) -> Vec<ContractId> {
    names.iter().map(|n| ContractId::from(*n)).collect()
}

#[test]
fn reconcile_spawns_one_when_the_held_bounty_has_no_quarry() {
    let held = ContractId::from("a");
    let plan = reconcile(Some(&held), &[]);
    assert!(plan.spawn);
    assert!(plan.despawn.is_empty());
}

#[test]
fn reconcile_keeps_the_first_of_two_and_drops_the_rest_and_the_orphans() {
    let held = ContractId::from("a");
    let plan = reconcile(Some(&held), &ids(&["old", "a", "a"]));
    assert!(!plan.spawn);
    assert_eq!(plan.despawn, vec![0, 2]);
}

#[test]
fn reconcile_clears_every_mark_when_nothing_is_held() {
    let plan = reconcile(None, &ids(&["a", "b"]));
    assert!(!plan.spawn);
    assert_eq!(plan.despawn, vec![0, 1]);
}

#[test]
fn anchor_samples_are_sorted_inside_the_band_and_repeatable() {
    let home = (10, -4);
    let points = anchor_samples(home);
    assert_eq!(points, anchor_samples(home));
    assert!(points.windows(2).all(|w| w[0] < w[1]));
    let (near, far) = BOUNTY_ANCHOR_BAND;
    assert!(points.iter().all(|&(x, y)| {
        let d = (x - home.0).abs().max((y - home.1).abs());
        (near..=far).contains(&d)
    }));
}

#[test]
fn the_verdict_reads_only_whether_the_quarry_fell() {
    assert_eq!(verdict(true), BountyOutcome::Complete);
    assert_eq!(verdict(false), BountyOutcome::Fail);
}
