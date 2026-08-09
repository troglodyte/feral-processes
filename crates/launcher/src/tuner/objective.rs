//! What the tuner is aiming at, read from `dev-tuning/objective.ron`.
//!
//! The objective is data rather than code for the same reason a scenario is:
//! saying "depth 5 should be winnable a bit over half the time" is a design
//! decision, and a design decision that lives in a Rust constant is one
//! nobody revisits. It names existing `dev-arenas/*.ron` files rather than
//! carrying its own fight definitions, so the file the arena's builder
//! screen writes with `[S]` is the same file the tuner optimises — and any
//! proposal it produces can be played by hand with `[L]`.

use super::roster::Field;
use super::score::Target;

/// A per-field range a candidate may not leave.
///
/// Every movable field needs one. An unbounded search proposes a 4000-HP
/// drone that satisfies the objective by making the opening fight
/// unwinnable in a way no target happened to measure.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Bound {
    pub field: String,
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct Objective {
    /// Fights per target when scoring a candidate. Overrides each
    /// scenario file's own `reps`, which is usually 1 so the builder screen
    /// prints a transcript — a rate cannot be read off one fight.
    pub seeds: u32,
    /// Fights per target used only to score the final proposal, on seeds
    /// the search never saw. The search compares candidates on one pinned
    /// seed set so differences are the roster rather than noise, which is
    /// exactly the setup that rewards overfitting; this is what catches it.
    pub holdout_seeds: u32,
    /// How many candidates to try before reporting the best.
    pub iterations: u32,
    /// Seeds the search's own perturbations, so a tuner run reproduces.
    pub search_seed: u64,
    pub targets: Vec<Target>,
    pub bounds: Vec<Bound>,
}

impl Objective {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
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

    /// Rejects an objective that cannot mean anything, rather than letting
    /// the search spend an hour discovering it. Everything here is a typo a
    /// person makes editing the file by hand.
    fn validate(&self) -> Result<(), String> {
        if self.targets.is_empty() {
            return Err("objective names no targets".into());
        }
        if self.seeds == 0 || self.holdout_seeds == 0 {
            return Err("seeds and holdout_seeds must both be above zero".into());
        }
        for target in &self.targets {
            if target.weight <= 0.0 {
                return Err(format!(
                    "target {} has a non-positive weight",
                    target.scenario
                ));
            }
            if !(0.0..=1.0).contains(&target.want_win_rate) {
                return Err(format!(
                    "target {} wants a win rate outside 0..=1",
                    target.scenario
                ));
            }
            if !(0.0..=1.0).contains(&target.want_hp_left) {
                return Err(format!(
                    "target {} wants an hp fraction outside 0..=1",
                    target.scenario
                ));
            }
        }
        for bound in &self.bounds {
            if Field::from_key(&bound.field).is_none() {
                return Err(format!(
                    "bound names {:?}, which is not a field the tuner may move",
                    bound.field
                ));
            }
            if bound.min >= bound.max {
                return Err(format!("bound on {} has min >= max", bound.field));
            }
        }
        // A field with no bound would be the one the search runs away with,
        // so absence is an error rather than an implicit "anything goes".
        for field in Field::ALL {
            if !self.bounds.iter().any(|b| b.field == field.key()) {
                return Err(format!("no bound given for {}", field.key()));
            }
        }
        Ok(())
    }

    /// The `(min, max)` for one field. Infallible because `validate`
    /// already refused an objective missing any of them.
    pub fn bound(&self, field: Field) -> (f64, f64) {
        let b = self
            .bounds
            .iter()
            .find(|b| b.field == field.key())
            .expect("validate rejects an objective missing a bound");
        (b.min, b.max)
    }

    /// Clamps a proposed value into its field's range.
    pub fn clamp(&self, field: Field, value: f64) -> f64 {
        let (min, max) = self.bound(field);
        value.clamp(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOUNDS: &str = r#"
        Bound(field: "base_hp", min: 1.0, max: 400.0),
        Bound(field: "base_atk", min: 1.0, max: 80.0),
        Bound(field: "base_def", min: 0.0, max: 60.0),
        Bound(field: "base_speed", min: 1.0, max: 40.0),
        Bound(field: "taming_difficulty", min: 0.05, max: 0.95),
        Bound(field: "growth_multiplier", min: 0.5, max: 2.5),
    "#;

    fn objective_ron(targets: &str) -> String {
        format!(
            "Objective(
                seeds: 40,
                holdout_seeds: 20,
                iterations: 200,
                search_seed: 7,
                targets: [{targets}],
                bounds: [{BOUNDS}],
            )"
        )
    }

    const ONE_TARGET: &str = r#"Target(
        scenario: "dev-arenas/opening-fight.ron",
        reps: 40,
        want_win_rate: 0.9,
        want_hp_left: 0.6,
        weight: 1.0,
    ),"#;

    #[test]
    fn a_well_formed_objective_parses() {
        let objective = Objective::from_ron(&objective_ron(ONE_TARGET)).unwrap();
        assert_eq!(objective.targets.len(), 1);
        assert_eq!(objective.bound(Field::BaseHp), (1.0, 400.0));
    }

    #[test]
    fn malformed_ron_is_an_error_rather_than_a_panic() {
        assert!(Objective::from_ron("this is not ron").is_err());
    }

    #[test]
    fn an_objective_with_no_targets_is_refused() {
        assert!(Objective::from_ron(&objective_ron("")).is_err());
    }

    #[test]
    fn a_bound_naming_an_unmovable_field_is_refused() {
        // `glyph` is what a species *is*, not what it is worth. Naming it
        // is a typo, and silently ignoring it would let a person think they
        // had constrained something.
        let ron = objective_ron(ONE_TARGET).replace(r#""base_hp""#, r#""glyph""#);
        let err = Objective::from_ron(&ron).unwrap_err();
        assert!(err.contains("glyph"), "got: {err}");
    }

    #[test]
    fn a_field_with_no_bound_is_refused() {
        let ron = objective_ron(ONE_TARGET)
            .replace(r#"Bound(field: "base_hp", min: 1.0, max: 400.0),"#, "");
        let err = Objective::from_ron(&ron).unwrap_err();
        assert!(err.contains("base_hp"), "got: {err}");
    }

    #[test]
    fn an_inverted_bound_is_refused() {
        let ron = objective_ron(ONE_TARGET).replace("min: 1.0, max: 400.0", "min: 400.0, max: 1.0");
        assert!(Objective::from_ron(&ron).is_err());
    }

    #[test]
    fn a_win_rate_target_outside_zero_to_one_is_refused() {
        let ron = objective_ron(ONE_TARGET).replace("want_win_rate: 0.9", "want_win_rate: 1.4");
        assert!(Objective::from_ron(&ron).is_err());
    }

    #[test]
    fn clamping_holds_a_value_inside_its_field_bound() {
        let objective = Objective::from_ron(&objective_ron(ONE_TARGET)).unwrap();
        assert_eq!(objective.clamp(Field::BaseHp, 9000.0), 400.0);
        assert_eq!(objective.clamp(Field::BaseHp, -5.0), 1.0);
        assert_eq!(objective.clamp(Field::BaseHp, 42.0), 42.0);
    }
}
