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
