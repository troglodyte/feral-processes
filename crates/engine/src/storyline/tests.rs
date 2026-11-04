use super::*;
use crate::components::ProgramId;
use crate::tuning::{BASE_AI_FUGUE_TICKS, BASE_AI_START_SPEED};

fn cand(id: u32, analysis: i32, free: bool) -> HostCandidate {
    HostCandidate {
        id: ProgramId(id),
        analysis,
        free,
    }
}

fn spot(anchor: (i32, i32), distance: u32) -> StationSpot {
    StationSpot { anchor, distance }
}

fn input<'a>(
    researched: u32,
    candidates: &'a [HostCandidate],
    stations: &'a [StationSpot],
) -> StepInput<'a> {
    StepInput {
        researched,
        trigger: 7,
        candidates,
        stations,
        host_free: true,
        host_arrived: false,
        host_analysis: 20,
        host_pace: 0.4,
    }
}

fn fused() -> FusedAi {
    FusedAi {
        host: ProgramId(1),
        station: (0, 0),
        level: 0,
        analysis_at_fusion: 20,
        pace_at_fusion: 0.4,
        practice: 0,
        carry: 0.0,
        training: TrainingTarget::default(),
    }
}

#[test]
fn trigger_count_is_between_five_and_ten_for_every_seed() {
    for seed in 0..10_000 {
        assert!((5..=10).contains(&trigger_count(seed)), "seed {seed}");
    }
}

#[test]
fn trigger_count_spreads_across_the_range() {
    let seen: std::collections::BTreeSet<u32> = (0..10_000).map(trigger_count).collect();
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        vec![5, 6, 7, 8, 9, 10]
    );
}

#[test]
fn pick_host_takes_the_highest_free_analysis_and_ties_go_to_the_lower_id() {
    let c = [cand(4, 30, true), cand(2, 30, true), cand(1, 20, true)];
    assert_eq!(pick_host(&c), Some(ProgramId(2)));
    let c = [cand(3, 10, true), cand(5, 25, true)];
    assert_eq!(pick_host(&c), Some(ProgramId(5)));
}

#[test]
fn pick_host_skips_unfree_programs() {
    let c = [cand(1, 99, false), cand(2, 10, true)];
    assert_eq!(pick_host(&c), Some(ProgramId(2)));
}

#[test]
fn pick_host_with_no_free_program_is_none() {
    assert_eq!(pick_host(&[]), None);
    assert_eq!(pick_host(&[cand(1, 50, false)]), None);
}

#[test]
fn pick_station_takes_the_nearest_and_ties_go_to_the_lower_anchor() {
    let s = [spot((9, 9), 3), spot((5, 1), 3), spot((0, 0), 8)];
    assert_eq!(pick_station(&s), Some((5, 1)));
    assert_eq!(pick_station(&[]), None);
}

#[test]
fn step_stays_dormant_below_the_trigger() {
    let c = [cand(1, 20, true)];
    let s = [spot((0, 0), 1)];
    let out = step(&BaseAi::default(), &input(6, &c, &s));
    assert_eq!(out, StepOutcome::None);
}

#[test]
fn step_begins_the_claim_at_the_trigger_even_with_no_research_selected() {
    let c = [cand(1, 20, true)];
    let s = [spot((2, 3), 1)];
    let out = step(&BaseAi::default(), &input(7, &c, &s));
    assert_eq!(
        out,
        StepOutcome::BeginClaim {
            host: ProgramId(1),
            station: (2, 3),
            announce: true
        }
    );
}

#[test]
fn step_does_not_announce_a_second_claim() {
    let c = [cand(1, 20, true)];
    let s = [spot((2, 3), 1)];
    let out = step(&BaseAi::Dormant { announced: true }, &input(12, &c, &s));
    assert!(matches!(
        out,
        StepOutcome::BeginClaim {
            announce: false,
            ..
        }
    ));
}

#[test]
fn step_waits_when_no_candidate_or_no_station() {
    let c = [cand(1, 20, true)];
    let s = [spot((2, 3), 1)];
    let none = step(&BaseAi::default(), &input(9, &[], &s));
    assert_eq!(none, StepOutcome::None);
    let none = step(&BaseAi::default(), &input(9, &c, &[]));
    assert_eq!(none, StepOutcome::None);
}

#[test]
fn step_abandons_the_claim_when_the_host_stops_being_free() {
    let arc = BaseAi::Claiming {
        host: ProgramId(1),
        station: (0, 0),
    };
    let mut i = input(9, &[], &[]);
    i.host_free = false;
    assert_eq!(step(&arc, &i), StepOutcome::AbandonClaim);
}

#[test]
fn step_starts_the_fugue_on_arrival_and_fuses_after_120_ticks() {
    let mut arc = BaseAi::Claiming {
        host: ProgramId(1),
        station: (4, 4),
    };
    let mut i = input(9, &[], &[]);
    assert_eq!(step(&arc, &i), StepOutcome::None);
    i.host_arrived = true;
    assert_eq!(step(&arc, &i), StepOutcome::StartFugue);

    arc = BaseAi::Fugue {
        host: ProgramId(1),
        station: (4, 4),
        ticks_left: BASE_AI_FUGUE_TICKS,
    };
    assert_eq!(BASE_AI_FUGUE_TICKS, 120);
    let mut ticks = 0;
    let fused = loop {
        match step(&arc, &i) {
            StepOutcome::TickFugue => {
                ticks += 1;
                let BaseAi::Fugue { ticks_left, .. } = &mut arc else {
                    unreachable!()
                };
                *ticks_left -= 1;
            }
            StepOutcome::Fuse(f) => break f,
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_eq!(ticks, 120);
    assert_eq!(fused.level, 0);
    assert_eq!(fused.analysis_at_fusion, i.host_analysis);
    assert_eq!(fused.pace_at_fusion, i.host_pace);
    assert_eq!((fused.host, fused.station), (ProgramId(1), (4, 4)));
}

#[test]
fn pace_starts_at_a_quarter_and_rises_with_analysis_without_a_ceiling() {
    let f = fused();
    let (a0, p0) = (f.analysis_at_fusion, f.pace_at_fusion);
    let near = |x: f64, y: f64| (x - y).abs() < 1e-9;
    assert_eq!(BASE_AI_START_SPEED, 0.25);
    assert!(near(pace(&f, a0), 0.25 * p0));
    assert!(near(pace(&f, 4 * a0), p0));
    assert!(near(pace(&f, 40 * a0), 10.0 * p0));
}

#[test]
fn practise_raises_analysis_and_each_point_costs_more() {
    let f = fused();
    let first = practise(&f, 20, practice_cost(0));
    assert_eq!(first.analysis, 21);
    // One tick short of the second point's price buys nothing more.
    let second = practise(&f, 21, practice_cost(1) - 1);
    assert_eq!(second.analysis, 21);
    assert!(practice_cost(1) > practice_cost(0));
    assert!(practice_cost(5) > practice_cost(1));
}

#[test]
fn practise_stalls_exactly_at_the_cap_and_drops_overflow() {
    let f = fused();
    let cap = analysis_cap(&f);
    let out = practise(&f, 20, 1_000_000);
    assert_eq!(out.analysis, cap);
    assert!(out.stalled);
    assert_eq!(out.practice, 0);
    // Already stalled: nothing moves.
    let again = practise(&f, cap, 500);
    assert_eq!(again, out);
}

#[test]
fn practise_after_the_last_offset_never_stalls() {
    let mut f = fused();
    f.level = 5;
    assert_eq!(analysis_cap(&f), i32::MAX);
    let out = practise(&f, 125, 100_000);
    assert!(!out.stalled);
    assert!(out.analysis > 125);
}
