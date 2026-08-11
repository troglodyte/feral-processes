//! Hill-climbing with random restarts over candidate rosters.
//!
//! Deliberately the simple thing. CMA-ES would be a dependency and a guess;
//! this is a few dozen lines with no dependency at all, and if it visibly
//! plateaus then upgrading is a decision backed by evidence rather than by
//! anticipation. The search is seeded, so a tuner run reproduces exactly —
//! same objective and same `search_seed` give the same proposal.
//!
//! The evaluator is a parameter rather than a call into `eval`, which is
//! what lets the accept/reject logic be tested against a synthetic error
//! surface in microseconds instead of against thousands of real fights.

use super::objective::Objective;
use super::roster::{Candidate, Field};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

/// How many consecutive non-improving candidates before the search gives up
/// on this hill and drops back to the baseline.
///
/// Restarting from the *baseline* rather than from a random roster is the
/// point: the shipped numbers are a hand-tuned local optimum, and a random
/// roster is overwhelmingly likely to be rejected outright or to be far
/// worse than one, wasting the restart.
const RESTART_AFTER: u32 = 25;

/// How far one perturbation may move a field, as a fraction of that field's
/// whole allowed range. Small enough that a step is a nudge rather than a
/// reroll, which is what makes a hill climb a climb.
const STEP_FRACTION: f64 = 0.08;

/// How many fields one perturbation touches at once.
const MAX_FIELDS_PER_STEP: usize = 3;

#[derive(Debug)]
pub struct Outcome {
    pub best: Candidate,
    pub best_error: f32,
    pub baseline_error: f32,
    /// Candidates actually fought.
    pub evaluated: u32,
    /// Candidates thrown out by a constraint before any fight ran.
    pub rejected: u32,
}

/// Moves one to three fields on one species by a bounded random step.
///
/// One species at a time on purpose: a step that moves the whole roster is
/// a step whose score change cannot be attributed, and a hill climb that
/// cannot attribute its improvements wanders.
pub fn perturb(current: &Candidate, objective: &Objective, rng: &mut StdRng) -> Candidate {
    let mut next = current.clone();
    let ids: Vec<&String> = current.species.keys().collect();
    if ids.is_empty() {
        return next;
    }
    let id = ids[rng.random_range(0..ids.len())].clone();

    let Some(fields) = next.species.get_mut(&id) else {
        return next;
    };
    let count = rng.random_range(1..=MAX_FIELDS_PER_STEP.min(Field::ALL.len()));
    for _ in 0..count {
        let field = Field::ALL[rng.random_range(0..Field::ALL.len())];
        let (min, max) = objective.bound(field);
        let step = (max - min) * STEP_FRACTION;
        let delta = rng.random_range(-step..=step);

        match fields.iter_mut().find(|(f, _)| *f == field) {
            Some((_, value)) => *value = objective.clamp(field, *value + delta),
            // A field the shipped file omits still has a bound, so the
            // search may reach it — otherwise a `#[serde(default)]` field
            // would be frozen for some species and free for others.
            None => {
                let midpoint = (min + max) / 2.0;
                fields.push((field, objective.clamp(field, midpoint + delta)));
            }
        }
    }
    next
}

/// Climbs from `baseline`, returning the best roster found.
///
/// `evaluate` returns `None` for a candidate a constraint rejected — those
/// cost nothing and are counted separately, because a run that rejected
/// most of what it tried has bounds set wrong and should say so rather than
/// silently reporting few improvements.
pub fn search<F>(
    baseline: Candidate,
    objective: &Objective,
    mut evaluate: F,
) -> Result<Outcome, String>
where
    F: FnMut(&Candidate) -> Option<f32>,
{
    let baseline_error = evaluate(&baseline).ok_or_else(|| {
        "the shipped roster is rejected by the objective's own constraints".to_string()
    })?;

    let mut rng = StdRng::seed_from_u64(objective.search_seed);
    let mut current = baseline.clone();
    let mut current_error = baseline_error;
    let mut best = baseline.clone();
    let mut best_error = baseline_error;
    let mut evaluated = 1;
    let mut rejected = 0;
    let mut stale = 0;

    for _ in 0..objective.iterations {
        let candidate = perturb(&current, objective, &mut rng);
        let Some(error) = evaluate(&candidate) else {
            rejected += 1;
            continue;
        };
        evaluated += 1;

        if error < current_error {
            current = candidate;
            current_error = error;
            stale = 0;
            if error < best_error {
                best = current.clone();
                best_error = error;
            }
        } else {
            stale += 1;
            if stale >= RESTART_AFTER {
                current = baseline.clone();
                current_error = baseline_error;
                stale = 0;
            }
        }
    }

    Ok(Outcome {
        best,
        best_error,
        baseline_error,
        evaluated,
        rejected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuner::objective::Bound;
    use crate::tuner::score::Target;

    fn objective(iterations: u32) -> Objective {
        Objective {
            seeds: 4,
            holdout_seeds: 2,
            iterations,
            search_seed: 11,
            targets: vec![Target {
                scenario: "unused-in-these-tests.ron".into(),
                reps: 4,
                want_win_rate: 0.9,
                want_hp_left: 0.6,
                weight: 1.0,
            }],
            bounds: Field::ALL
                .iter()
                .map(|f| Bound {
                    field: f.key().into(),
                    min: 0.0,
                    max: 100.0,
                })
                .collect(),
        }
    }

    fn baseline() -> Candidate {
        let mut species = std::collections::BTreeMap::new();
        species.insert("drone".into(), vec![(Field::BaseHp, 10.0)]);
        Candidate { species }
    }

    /// A synthetic surface whose optimum is `base_hp == 80`, so the search
    /// can be checked in microseconds rather than against real fights.
    fn distance_from_80(candidate: &Candidate) -> Option<f32> {
        let hp = candidate
            .species
            .get("drone")?
            .iter()
            .find(|(f, _)| *f == Field::BaseHp)?
            .1;
        Some((hp - 80.0).abs() as f32)
    }

    #[test]
    fn the_search_climbs_towards_the_optimum() {
        let out = search(baseline(), &objective(400), distance_from_80).unwrap();
        assert!(
            out.best_error < out.baseline_error,
            "no improvement: {} -> {}",
            out.baseline_error,
            out.best_error
        );
        assert!(
            out.best_error < 5.0,
            "expected to get near base_hp 80, error was {}",
            out.best_error
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_proposal() {
        // A tuner run that cannot be reproduced cannot be reviewed: the
        // whole output is a proposal a person diffs, and "run it again"
        // has to give them the same thing to diff.
        let a = search(baseline(), &objective(200), distance_from_80).unwrap();
        let b = search(baseline(), &objective(200), distance_from_80).unwrap();
        assert_eq!(a.best, b.best);
        assert_eq!(a.best_error, b.best_error);
    }

    #[test]
    fn a_different_seed_explores_differently() {
        let mut other = objective(200);
        other.search_seed = 99;
        let a = search(baseline(), &objective(200), distance_from_80).unwrap();
        let b = search(baseline(), &other, distance_from_80).unwrap();
        assert_ne!(a.best, b.best);
    }

    #[test]
    fn a_rejected_candidate_is_counted_and_never_accepted() {
        let out = search(baseline(), &objective(50), |c| {
            // Everything but the baseline is rejected.
            (c == &baseline()).then_some(1.0)
        })
        .unwrap();
        assert_eq!(out.best, baseline());
        assert_eq!(out.rejected, 50);
        assert_eq!(out.evaluated, 1);
    }

    #[test]
    fn a_baseline_the_constraints_reject_is_an_error_not_a_silent_zero() {
        let err = search(baseline(), &objective(10), |_| None).unwrap_err();
        assert!(err.contains("shipped roster"), "got: {err}");
    }

    #[test]
    fn perturbation_never_leaves_a_field_bound() {
        let mut rng = StdRng::seed_from_u64(3);
        let objective = objective(1);
        let mut candidate = baseline();
        for _ in 0..500 {
            candidate = perturb(&candidate, &objective, &mut rng);
            for changes in candidate.species.values() {
                for (field, value) in changes {
                    let (min, max) = objective.bound(*field);
                    assert!(
                        *value >= min && *value <= max,
                        "{} left its bound at {value}",
                        field.key()
                    );
                }
            }
        }
    }

    #[test]
    fn perturbation_never_introduces_a_species_the_candidate_does_not_carry() {
        // This is what makes leaving a frozen species out of the candidate
        // *sufficient* rather than merely tidy. `eval::Workspace::baseline`
        // drops the species the player fields; if `perturb` could add a key
        // back, that omission would buy nothing and the freeze would be a
        // comment rather than a mechanism.
        let mut rng = StdRng::seed_from_u64(17);
        let objective = objective(1);
        let mut candidate = baseline();
        let carried: Vec<String> = candidate.species.keys().cloned().collect();
        for _ in 0..1000 {
            candidate = perturb(&candidate, &objective, &mut rng);
            let keys: Vec<String> = candidate.species.keys().cloned().collect();
            assert_eq!(keys, carried, "perturb changed which species are movable");
        }
    }

    #[test]
    fn perturbation_can_reach_a_field_the_shipped_file_omits() {
        // `growth_multiplier` is `#[serde(default)]` and most species omit
        // it. If the search could only move fields already written, those
        // species would have a smaller search space than the ones that
        // happen to spell it out.
        let mut rng = StdRng::seed_from_u64(5);
        let objective = objective(1);
        let mut candidate = baseline();
        for _ in 0..300 {
            candidate = perturb(&candidate, &objective, &mut rng);
        }
        assert!(
            candidate.species["drone"]
                .iter()
                .any(|(f, _)| *f == Field::GrowthMultiplier),
            "never reached growth_multiplier"
        );
    }
}
