use super::*;
use crate::tuning::SKILL_LEVEL_CAP;

#[test]
fn practice_below_the_cost_only_adds_practice() {
    let mut t = SkillTrack::default();
    let out = practise(&mut t, 5);
    assert_eq!(out.levels_gained, 0);
    assert_eq!(out.new_level, 0);
    assert_eq!((t.level, t.practice), (0, 5));
}

#[test]
fn reaching_the_cost_rolls_over_with_the_remainder() {
    let cost = practice_to_next(0).unwrap();
    let mut t = SkillTrack::default();
    let out = practise(&mut t, cost + 3);
    assert_eq!(out.levels_gained, 1);
    assert_eq!((t.level, t.practice), (1, 3));
}

#[test]
fn one_large_amount_gains_several_levels() {
    let two = practice_to_next(0).unwrap() + practice_to_next(1).unwrap();
    let mut t = SkillTrack::default();
    let out = practise(&mut t, two + 1);
    assert_eq!(out.levels_gained, 2);
    assert_eq!(out.new_level, 2);
    assert_eq!(t.practice, 1);
}

#[test]
fn the_cap_stops_levels_and_discards_practice() {
    let mut t = SkillTrack::default();
    let out = practise(&mut t, u32::MAX / 2);
    assert_eq!(t.level, SKILL_LEVEL_CAP);
    assert_eq!(out.levels_gained, SKILL_LEVEL_CAP);
    assert_eq!(t.practice, 0);
    let again = practise(&mut t, 1000);
    assert_eq!(again.levels_gained, 0);
    assert_eq!((t.level, t.practice), (SKILL_LEVEL_CAP, 0));
}

#[test]
fn the_cost_curve_strictly_increases_and_ends_at_the_cap() {
    let costs: Vec<u32> = (0..SKILL_LEVEL_CAP)
        .map(|l| practice_to_next(l).unwrap())
        .collect();
    assert!(costs.windows(2).all(|w| w[0] < w[1]), "{costs:?}");
    assert!(costs[0] > 0);
    assert_eq!(practice_to_next(SKILL_LEVEL_CAP), None);
}

#[test]
fn set_clamps_a_level_past_the_cap() {
    let mut s = Skills::default();
    s.set(Skill::Dig, SKILL_LEVEL_CAP + 9, 77);
    assert_eq!(
        s.track(Skill::Dig),
        SkillTrack {
            level: SKILL_LEVEL_CAP,
            practice: 0
        }
    );
}

#[test]
fn skills_hold_one_track_each() {
    let mut s = Skills::default();
    s.track_mut(Skill::Build).level = 4;
    assert_eq!(s.level(Skill::Build), 4);
    assert_eq!(s.level(Skill::Farm), 0);
}

#[test]
fn names_round_trip_and_unknown_is_none() {
    for s in Skill::ALL {
        assert_eq!(Skill::from_name(s.name()), Some(s));
    }
    assert_eq!(Skill::from_name("juggling"), None);
}

#[test]
fn equality_ignores_the_unsaved_carry() {
    let mut a = Skills::default();
    a.pace_steps(500);
    assert_eq!(a, Skills::default());
    a.track_mut(Skill::Build).level = 1;
    assert_ne!(a, Skills::default());
}

#[test]
fn scale_is_neutral_at_ten_and_symmetric_about_it() {
    assert_eq!(scale(SKILL_NEUTRAL_LEVEL, 0.5), 1.0);
    assert_eq!(scale(0, 0.1), 0.9);
    assert_eq!(scale(20, 0.1), 1.1);
    for d in 0..=10 {
        let (lo, hi) = (
            scale(SKILL_NEUTRAL_LEVEL - d, 0.3),
            scale(SKILL_NEUTRAL_LEVEL + d, 0.3),
        );
        assert!((lo + hi - 2.0).abs() < 1e-6, "{d}: {lo} {hi}");
    }
}

#[test]
fn the_families_partition_all() {
    for s in Skill::ALL {
        let in_work = Skill::WORK.contains(&s);
        let in_combat = Skill::COMBAT.contains(&s);
        assert!(in_work != in_combat, "{s:?} must be in exactly one family");
    }
    let union: Vec<Skill> = Skill::WORK.into_iter().chain(Skill::COMBAT).collect();
    assert_eq!(union, Skill::ALL);
}

#[test]
fn overlay_replaces_listed_rows_and_keeps_the_seed_elsewhere() {
    let mut seed = Skills::default();
    seed.set(Skill::Force, 4, 0);
    seed.set(Skill::Dig, 2, 0);
    let got = Skills::overlay(seed, [(Skill::Dig, 7, 3)]);
    assert_eq!(
        got.track(Skill::Dig),
        SkillTrack {
            level: 7,
            practice: 3
        }
    );
    assert_eq!(got.level(Skill::Force), 4);
}

#[test]
fn seeded_starts_combat_at_neutral_plus_the_bonus_and_work_absolute() {
    let none = Skills::seeded(&[]);
    for s in Skill::COMBAT {
        assert_eq!(none.level(s), SKILL_NEUTRAL_LEVEL, "{s:?}");
    }
    for s in Skill::WORK {
        assert_eq!(none.level(s), 0, "{s:?}");
    }
    let some = Skills::seeded(&[(Skill::Force, 4), (Skill::Dig, 4)]);
    assert_eq!(some.level(Skill::Force), SKILL_NEUTRAL_LEVEL + 4);
    assert_eq!(some.level(Skill::Aim), SKILL_NEUTRAL_LEVEL);
    assert_eq!(some.level(Skill::Dig), 4);
    let over = Skills::seeded(&[(Skill::Aim, 15)]);
    assert_eq!(over.level(Skill::Aim), SKILL_LEVEL_CAP);
}

/// The manifest's skill bar: levels toward the cap, with the current level's
/// practice as the fraction between two whole levels.
#[test]
fn progress_to_cap_counts_levels_and_the_part_of_the_next() {
    assert_eq!(progress_to_cap(0, 0), 0.0);
    let half = practice_to_next(5).unwrap() / 2;
    let expected = (5.0 + half as f32 / practice_to_next(5).unwrap() as f32) / 20.0;
    assert!((progress_to_cap(5, half) - expected).abs() < 1e-6);
    assert_eq!(progress_to_cap(SKILL_LEVEL_CAP, 0), 1.0);
    assert!(progress_to_cap(SKILL_LEVEL_CAP - 1, 0) < 1.0);
}
