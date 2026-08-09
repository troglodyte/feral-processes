//! The cross-entropy method: sample a population of candidate vectors from
//! a Gaussian, keep the best few, refit the Gaussian to them, repeat.
//!
//! Derivative-free, so it optimises the metric that is actually wanted —
//! enemy win rate over a set of arena fights — rather than a differentiable
//! stand-in for it. There is no game in this file at all: the objective is
//! a closure, which is why `cem_converges_on_a_quadratic` can prove the
//! optimiser correct with nothing else in the picture.
//!
//! `train.rs` is the caller that supplies the arena as the objective.

pub struct CemConfig {
    pub dims: usize,
    pub population: usize,
    /// Fraction of each generation kept as the elite set the next Gaussian
    /// is fitted to. At least one candidate is always kept.
    pub elite_fraction: f32,
    pub iterations: usize,
    pub initial_std: f32,
    /// Added to the refitted standard deviation every generation so the
    /// search cannot collapse to a point and stop exploring. Without it a
    /// run silently stops learning after a handful of generations while
    /// still printing progress lines — see
    /// `the_std_floor_keeps_the_search_alive`.
    pub std_floor: f32,
}

pub struct CemProgress {
    pub iteration: usize,
    pub best_fitness: f32,
    pub mean_fitness: f32,
}

/// Runs `cfg.iterations` generations and returns the final distribution's
/// mean — the trained vector.
///
/// `fitness` is called once per candidate per generation and **higher is
/// better**. It may be expensive; nothing here calls it more than
/// `iterations * population` times. A non-finite fitness sorts to the back
/// of its generation rather than poisoning the elite set.
///
/// The whole run is reproducible from `rng`.
pub fn optimise<F, R>(
    cfg: &CemConfig,
    rng: &mut R,
    fitness: F,
    mut on_progress: impl FnMut(CemProgress),
) -> Vec<f32>
where
    F: Fn(&[f32]) -> f32,
    R: rand::Rng,
{
    let mut mean = vec![0.0f32; cfg.dims];
    let mut std = vec![cfg.initial_std; cfg.dims];
    let elite_count = ((cfg.population as f32 * cfg.elite_fraction).round() as usize)
        .clamp(1, cfg.population.max(1));

    for iteration in 0..cfg.iterations {
        let population: Vec<Vec<f32>> = (0..cfg.population)
            .map(|_| {
                (0..cfg.dims)
                    .map(|d| mean[d] + std[d] * standard_normal(rng))
                    .collect()
            })
            .collect();

        let mut scored: Vec<(f32, &Vec<f32>)> = population
            .iter()
            .map(|c| (finite_or_worst(fitness(c)), c))
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));

        let elite = &scored[..elite_count.min(scored.len())];
        for d in 0..cfg.dims {
            let values: Vec<f32> = elite.iter().map(|(_, c)| c[d]).collect();
            let m = values.iter().sum::<f32>() / values.len() as f32;
            // Population standard deviation of the elite set, not the
            // sample one: with `elite_count == 1` the sample form divides
            // by zero, and one elite genuinely has no spread.
            let var = values.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / values.len() as f32;
            mean[d] = m;
            std[d] = var.sqrt() + cfg.std_floor;
        }

        on_progress(CemProgress {
            iteration,
            best_fitness: scored.first().map(|(f, _)| *f).unwrap_or(0.0),
            mean_fitness: scored.iter().map(|(f, _)| f).sum::<f32>() / scored.len().max(1) as f32,
        });
    }
    mean
}

fn finite_or_worst(f: f32) -> f32 {
    if f.is_finite() { f } else { f32::NEG_INFINITY }
}

/// Box–Muller, rather than a dependency on `rand_distr` for one
/// distribution. `u1` is drawn away from zero because `ln(0)` is `-inf`.
fn standard_normal<R: rand::Rng>(rng: &mut R) -> f32 {
    use rand::RngExt;
    let u1: f32 = rng.random_range(f32::MIN_POSITIVE..1.0);
    let u2: f32 = rng.random_range(0.0..1.0);
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::cell::RefCell;

    fn config(dims: usize) -> CemConfig {
        CemConfig {
            dims,
            population: 40,
            elite_fraction: 0.25,
            iterations: 40,
            initial_std: 2.0,
            std_floor: 0.02,
        }
    }

    /// The optimiser proved with no game in sight: a quadratic whose peak is
    /// known exactly.
    #[test]
    fn cem_converges_on_a_quadratic() {
        let mut rng = StdRng::seed_from_u64(1);
        let best = optimise(
            &config(2),
            &mut rng,
            |x| -(x[0] - 3.0).powi(2) - (x[1] + 1.0).powi(2),
            |_| {},
        );
        assert!(
            (best[0] - 3.0).abs() < 0.1 && (best[1] + 1.0).abs() < 0.1,
            "converged on {best:?}, not (3, -1)"
        );
    }

    #[test]
    fn the_same_seed_reproduces_a_run() {
        let run = || {
            let mut rng = StdRng::seed_from_u64(99);
            optimise(
                &config(4),
                &mut rng,
                |x| -x.iter().map(|v| v * v).sum::<f32>(),
                |_| {},
            )
        };
        assert_eq!(run(), run());
    }

    /// The failure mode that makes a training run silently stop learning at
    /// generation four while still printing progress lines every time.
    ///
    /// Measured through the objective rather than by exposing the internal
    /// standard deviation: the closure sees every candidate, so the spread
    /// of the final generation *is* observable from outside.
    #[test]
    fn the_std_floor_keeps_the_search_alive() {
        let final_spread = |std_floor: f32| {
            let seen = RefCell::new(Vec::new());
            let cfg = CemConfig {
                std_floor,
                iterations: 12,
                ..config(1)
            };
            let mut rng = StdRng::seed_from_u64(7);
            // A peaked objective, which is where the collapse actually
            // happens: the elite set clusters tighter every generation, so
            // the refitted standard deviation shrinks geometrically and the
            // only thing holding it open is the floor. A *flat* objective
            // does not collapse at all — an arbitrary quarter of a Gaussian
            // sample has roughly that Gaussian's spread — which is worth
            // knowing before writing this test the obvious way.
            optimise(
                &cfg,
                &mut rng,
                |x| {
                    seen.borrow_mut().push(x[0]);
                    -x[0] * x[0]
                },
                |_| {},
            );
            let all = seen.into_inner();
            let last: Vec<f32> = all[all.len() - cfg.population..].to_vec();
            let m = last.iter().sum::<f32>() / last.len() as f32;
            (last.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / last.len() as f32).sqrt()
        };

        let collapsed = final_spread(0.0);
        let alive = final_spread(0.5);
        assert!(
            collapsed < 0.01,
            "with no floor the search should have collapsed, spread was {collapsed}"
        );
        assert!(
            alive > 0.4,
            "with a floor of 0.5 the search should still be exploring, spread was {alive}"
        );
    }

    #[test]
    fn progress_is_reported_once_per_iteration() {
        let mut rng = StdRng::seed_from_u64(5);
        let cfg = CemConfig {
            iterations: 6,
            ..config(2)
        };
        let mut seen = Vec::new();
        optimise(
            &cfg,
            &mut rng,
            |x| -(x[0] * x[0]) - x[1],
            |p| seen.push(p.iteration),
        );
        assert_eq!(seen, (0..6).collect::<Vec<_>>());
    }
}
