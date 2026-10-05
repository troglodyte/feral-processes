//! What the bench aims at: target ranges for named measures, and the knobs
//! a search may turn to reach them.

use super::knob::Knob;
use feral_processes_engine::bench::MEASURES;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    pub measure: String,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Objective {
    pub template: String,
    pub ticks: u64,
    pub seeds: Vec<u64>,
    pub holdout_seeds: Vec<u64>,
    pub iterations: usize,
    pub population: usize,
    pub search_seed: u64,
    /// Batch orders `(item, qty)` queued before every run, so machines have
    /// standing demand (and posted workers to haul their output). Use a
    /// quantity the run cannot reach.
    #[serde(default)]
    pub orders: Vec<(String, u32)>,
    pub targets: Vec<Target>,
    pub knobs: Vec<Knob>,
}

/// Zero inside `[min, max]`, else the squared distance outside it as a
/// fraction of the range's width, so a target on a 0..1 share and one on a
/// 0..500 count weigh alike.
pub fn range_error(value: f64, min: f64, max: f64) -> f64 {
    let outside = (min - value).max(value - max).max(0.0);
    (outside / (max - min)).powi(2)
}

impl Objective {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read objective {}: {e}", path.display()))?;
        Self::from_ron(&text)
    }

    pub fn from_ron(text: &str) -> Result<Self, String> {
        let objective: Objective =
            ron::from_str(text).map_err(|e| format!("malformed objective: {e}"))?;
        objective.validate()?;
        Ok(objective)
    }

    /// Rejects what a hand-edited file gets wrong, before a search spends
    /// minutes finding out.
    fn validate(&self) -> Result<(), String> {
        if self.targets.is_empty() {
            return Err("objective names no targets".into());
        }
        if self.knobs.is_empty() {
            return Err("objective names no knobs".into());
        }
        if self.seeds.is_empty() || self.holdout_seeds.is_empty() {
            return Err("seeds and holdout_seeds must both be non-empty".into());
        }
        if let Some(seed) = self.seeds.iter().find(|s| self.holdout_seeds.contains(s)) {
            return Err(format!(
                "seed {seed} is in both seeds and holdout_seeds, so the hold-out is not unseen"
            ));
        }
        if self.iterations == 0 || self.population == 0 || self.ticks == 0 {
            return Err("iterations, population and ticks must be above zero".into());
        }
        for target in &self.targets {
            if !known_measure(&target.measure) {
                return Err(format!(
                    "unknown measure `{}`; known: {}",
                    target.measure,
                    MEASURES.join(", ")
                ));
            }
            if !is_range(target.min, target.max) {
                return Err(format!(
                    "target `{}`: min {} must be below max {}",
                    target.measure, target.min, target.max
                ));
            }
        }
        for (i, knob) in self.knobs.iter().enumerate() {
            if !is_range(knob.min, knob.max) {
                return Err(format!(
                    "knob {} `{}`: min {} must be below max {}",
                    knob.file, knob.field, knob.min, knob.max
                ));
            }
            if self.knobs[..i]
                .iter()
                .any(|k| k.file == knob.file && k.field == knob.field)
            {
                return Err(format!(
                    "knob {} `{}` is listed twice",
                    knob.file, knob.field
                ));
            }
        }
        Ok(())
    }
}

/// A usable range: finite, with room between the ends (NaN is neither).
fn is_range(min: f64, max: f64) -> bool {
    min.is_finite() && max.is_finite() && min < max
}

/// Whether `name` is one of `MEASURES`, whose entries end in `<placeholder>`
/// where a name is variable.
fn known_measure(name: &str) -> bool {
    MEASURES.iter().any(|m| match m.split_once('<') {
        Some((prefix, _)) => name.len() > prefix.len() && name.starts_with(prefix),
        None => name == *m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_the_range_is_zero() {
        assert_eq!(range_error(0.5, 0.2, 0.8), 0.0);
        assert_eq!(range_error(0.2, 0.2, 0.8), 0.0);
        assert_eq!(range_error(0.8, 0.2, 0.8), 0.0);
    }

    #[test]
    fn outside_grows_and_is_normalised_by_width() {
        let near = range_error(0.1, 0.2, 0.8);
        let far = range_error(0.0, 0.2, 0.8);
        assert!(0.0 < near && near < far);
        assert!((range_error(0.9, 0.2, 0.8) - range_error(0.1, 0.2, 0.8)).abs() < 1e-12);
        // The same relative miss costs the same on a wide range.
        let narrow = range_error(2.5, 1.0, 2.0);
        let wide = range_error(250.0, 100.0, 200.0);
        assert!((narrow - wide).abs() < 1e-12);
        assert!((narrow - 0.25).abs() < 1e-12);
    }

    fn ron_with(extra: &str) -> String {
        format!(
            r#"(template: "chains", ticks: 100, seeds: [1, 2], holdout_seeds: [3],
            iterations: 2, population: 2, search_seed: 7,
            targets: [(measure: "economy.labour_unworked", min: 0.0, max: 1.0)],
            knobs: [(file: "structures/assembly_bay.ron", field: "capacity", min: 1.0, max: 9.0)]
            {extra})"#
        )
    }

    fn err_of(text: &str) -> String {
        Objective::from_ron(text).unwrap_err()
    }

    #[test]
    fn a_well_formed_objective_loads() {
        let o = Objective::from_ron(&ron_with("")).unwrap();
        assert_eq!(o.targets.len(), 1);
        assert_eq!(o.knobs[0].field, "capacity");
    }

    #[test]
    fn unknown_measure_is_named() {
        let e = err_of(&ron_with("").replace("economy.labour_unworked", "economy.nope"));
        assert!(e.contains("economy.nope") && e.contains("known:"), "{e}");
        let e = err_of(&ron_with("").replace("economy.labour_unworked", "economy.items."));
        assert!(e.contains("unknown measure"), "{e}");
    }

    #[test]
    fn parameterised_measures_are_accepted() {
        let text = ron_with("").replace("economy.labour_unworked", "economy.items.patch_routine");
        assert!(Objective::from_ron(&text).is_ok());
    }

    #[test]
    fn inverted_ranges_are_rejected() {
        let e = err_of(&ron_with("").replace("min: 0.0, max: 1.0", "min: 1.0, max: 1.0"));
        assert!(e.contains("min"), "{e}");
        let e = err_of(&ron_with("").replace("min: 1.0, max: 9.0", "min: 9.0, max: 1.0"));
        assert!(e.contains("capacity"), "{e}");
    }

    #[test]
    fn seed_sets_must_be_non_empty_and_disjoint() {
        let e = err_of(&ron_with("").replace("holdout_seeds: [3]", "holdout_seeds: []"));
        assert!(e.contains("non-empty"), "{e}");
        let e = err_of(&ron_with("").replace("seeds: [1, 2]", "seeds: []"));
        assert!(e.contains("non-empty"), "{e}");
        let e = err_of(&ron_with("").replace("holdout_seeds: [3]", "holdout_seeds: [2, 3]"));
        assert!(e.contains("seed 2"), "{e}");
    }

    #[test]
    fn a_missing_bound_is_a_parse_error() {
        let e = err_of(&ron_with("").replace(", max: 1.0)", ")"));
        assert!(e.contains("malformed objective"), "{e}");
    }

    #[test]
    fn a_duplicate_knob_is_rejected() {
        let knob =
            r#"(file: "structures/assembly_bay.ron", field: "capacity", min: 1.0, max: 9.0)"#;
        let e = err_of(&ron_with("").replace(knob, &format!("{knob}, {knob}")));
        assert!(e.contains("twice"), "{e}");
    }
}
