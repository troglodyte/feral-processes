//! The enemy battle policy's arithmetic: a fixed-order feature vector, one
//! shared weight vector over it, and a temperature softmax to sample from.
//!
//! Deliberately free of `World` and `Game` — feature *extraction* lives in
//! `game/combat_policy.rs`, which is where the battle state is. Everything
//! here is pure, so it is unit-testable on its own and the code that reads
//! the world can trust it.

pub const FEATURE_COUNT: usize = 19;

/// One term of the score a candidate `(move, target)` pair gets.
///
/// The order is fixed and `as usize` indexes both `Features` and
/// `PolicyWeights`, but nothing outside this module may rely on that: a
/// weight file is keyed by *name*, so inserting a feature here degrades a
/// mod's file to "that name is absent, so zero" rather than silently
/// re-reading its numbers against different meanings.
///
/// There is deliberately no bias term. A constant added to every candidate
/// cancels under a softmax over candidates, so one would be an unlearnable
/// parameter for the trainer to waste a dimension on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    MovePowerRel,
    MoveRanged,
    MoveHasEffect,
    MoveEffectStun,
    MoveEffectBleed,
    MoveEffectChance,
    TargetHpFrac,
    TargetIsPlayer,
    TargetDefRel,
    TargetStunned,
    TargetBleeding,
    TargetBracing,
    EstDamageFrac,
    WouldKill,
    SelfHpFrac,
    SelfFrontGroup,
    EffectXTargetHealthy,
    StunXNotStunned,
    BleedXNotBleeding,
}

impl Feature {
    pub const ALL: [Feature; FEATURE_COUNT] = [
        Feature::MovePowerRel,
        Feature::MoveRanged,
        Feature::MoveHasEffect,
        Feature::MoveEffectStun,
        Feature::MoveEffectBleed,
        Feature::MoveEffectChance,
        Feature::TargetHpFrac,
        Feature::TargetIsPlayer,
        Feature::TargetDefRel,
        Feature::TargetStunned,
        Feature::TargetBleeding,
        Feature::TargetBracing,
        Feature::EstDamageFrac,
        Feature::WouldKill,
        Feature::SelfHpFrac,
        Feature::SelfFrontGroup,
        Feature::EffectXTargetHealthy,
        Feature::StunXNotStunned,
        Feature::BleedXNotBleeding,
    ];

    /// The weight file's key for this feature, and what the training report
    /// prints. `every_feature_name_round_trips` is what stops a typo here
    /// silently zeroing a feature that extraction is faithfully filling in.
    pub fn name(self) -> &'static str {
        match self {
            Feature::MovePowerRel => "move_power_rel",
            Feature::MoveRanged => "move_ranged",
            Feature::MoveHasEffect => "move_has_effect",
            Feature::MoveEffectStun => "move_effect_stun",
            Feature::MoveEffectBleed => "move_effect_bleed",
            Feature::MoveEffectChance => "move_effect_chance",
            Feature::TargetHpFrac => "target_hp_frac",
            Feature::TargetIsPlayer => "target_is_player",
            Feature::TargetDefRel => "target_def_rel",
            Feature::TargetStunned => "target_stunned",
            Feature::TargetBleeding => "target_bleeding",
            Feature::TargetBracing => "target_bracing",
            Feature::EstDamageFrac => "est_damage_frac",
            Feature::WouldKill => "would_kill",
            Feature::SelfHpFrac => "self_hp_frac",
            Feature::SelfFrontGroup => "self_front_group",
            Feature::EffectXTargetHealthy => "effect_x_target_healthy",
            Feature::StunXNotStunned => "stun_x_not_stunned",
            Feature::BleedXNotBleeding => "bleed_x_not_bleeding",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Feature::ALL.into_iter().find(|f| f.name() == s)
    }
}

/// One candidate pair's feature values, every one normalised to roughly
/// `[0, 1]` so the trainer's Gaussian is well-scaled across dimensions.
#[derive(Clone, Copy, Debug)]
pub struct Features([f32; FEATURE_COUNT]);

impl Features {
    pub fn zeroed() -> Self {
        Features([0.0; FEATURE_COUNT])
    }

    pub fn set(&mut self, f: Feature, v: f32) {
        self.0[f as usize] = v;
    }

    pub fn get(&self, f: Feature) -> f32 {
        self.0[f as usize]
    }
}

/// The trained brain: one weight per feature, shared by every candidate pair
/// of every species. That is what makes a modded species with a moveset
/// nobody trained against still get scored sensibly.
#[derive(Clone, Debug, Default)]
pub struct PolicyWeights([f32; FEATURE_COUNT]);

impl PolicyWeights {
    /// Builds a weight vector from name/value pairs, as read out of a `.ron`
    /// file. An unknown name is warned about and ignored; an absent one
    /// stays zero, which is the same as not being there at all.
    ///
    /// A non-finite weight is an `Err` for the whole file rather than a
    /// skipped row: it would reach `exp` every round of every fight, and the
    /// place to refuse it is once, at load — the same shape as
    /// `ItemDb`'s non-finite drop chance.
    pub fn from_pairs(pairs: &[(String, f32)]) -> Result<(Self, Vec<String>), String> {
        let mut w = PolicyWeights::default();
        let mut warnings = Vec::new();
        for (name, value) in pairs {
            if !value.is_finite() {
                return Err(format!("feature {name:?} has a non-finite weight {value}"));
            }
            match Feature::from_name(name) {
                Some(f) => w.0[f as usize] = *value,
                None => warnings.push(format!(
                    "unknown policy feature {name:?} ignored — this build knows \
                     {FEATURE_COUNT} features, see assets/policies/README.md"
                )),
            }
        }
        Ok((w, warnings))
    }

    /// Every weight, in `Feature::ALL` order, including the zeroes. A
    /// trained file naming all nineteen is legible in a way one naming the
    /// six that happened to end up large is not.
    pub fn to_pairs(&self) -> Vec<(String, f32)> {
        Feature::ALL
            .into_iter()
            .map(|f| (f.name().to_string(), self.0[f as usize]))
            .collect()
    }

    pub fn score(&self, f: &Features) -> f32 {
        Feature::ALL
            .into_iter()
            .map(|k| self.0[k as usize] * f.0[k as usize])
            .sum()
    }
}

/// Picks an index into `scores`, softmax-weighted at `temperature`.
///
/// The maximum score is subtracted before `exp`, so a weight the trainer
/// pushed to an absurd magnitude saturates to a certain choice rather than
/// to `inf`/NaN. A temperature at or below zero is argmax — the same
/// meaning `ENEMY_POLICY_TEMPERATURE` documents, and the fallback if the
/// exponentials somehow leave nothing to sample from.
///
/// Panics on an empty slice: "nothing reaches" is the caller's decision to
/// make and `Game::choose_wild_action` makes it before getting here.
pub fn sample_scored<R: rand::Rng>(scores: &[f32], temperature: f32, rng: &mut R) -> usize {
    use rand::RngExt;
    assert!(!scores.is_empty(), "sample_scored needs a candidate");
    let argmax = || {
        scores
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    if temperature <= 0.0 {
        return argmax();
    }
    let max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f64> = scores
        .iter()
        .map(|s| (((s - max) / temperature) as f64).exp())
        .collect();
    let total: f64 = weights.iter().sum();
    if !total.is_finite() || total <= 0.0 {
        return argmax();
    }
    let mut roll = rng.random_range(0.0..total);
    for (i, w) in weights.iter().enumerate() {
        if roll < *w {
            return i;
        }
        roll -= w;
    }
    weights.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn every_feature_name_round_trips() {
        let mut seen = std::collections::HashSet::new();
        for f in Feature::ALL {
            assert_eq!(Feature::from_name(f.name()), Some(f), "{}", f.name());
            assert!(seen.insert(f.name()), "duplicate feature name {}", f.name());
        }
        assert_eq!(seen.len(), FEATURE_COUNT);
    }

    #[test]
    fn an_unknown_feature_name_is_warned_and_ignored() {
        let (w, warnings) = PolicyWeights::from_pairs(&[
            ("would_kill".into(), 2.0),
            ("target_charisma".into(), 9.0),
        ])
        .unwrap();

        let mut f = Features::zeroed();
        f.set(Feature::WouldKill, 1.0);
        assert_eq!(w.score(&f), 2.0);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0].contains("target_charisma"),
            "warning should name the unknown key: {}",
            warnings[0]
        );
    }

    #[test]
    fn an_absent_feature_name_defaults_to_zero() {
        let (w, warnings) = PolicyWeights::from_pairs(&[
            ("target_hp_frac".into(), -1.0),
            ("would_kill".into(), 2.0),
            ("self_hp_frac".into(), 0.5),
        ])
        .unwrap();
        assert!(warnings.is_empty());

        let named = [
            Feature::TargetHpFrac,
            Feature::WouldKill,
            Feature::SelfHpFrac,
        ];
        for f in Feature::ALL.into_iter().filter(|f| !named.contains(f)) {
            let mut v = Features::zeroed();
            v.set(f, 1.0);
            assert_eq!(w.score(&v), 0.0, "{} should default to zero", f.name());
        }
    }

    #[test]
    fn a_non_finite_weight_is_rejected() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(
                PolicyWeights::from_pairs(&[("would_kill".into(), bad)]).is_err(),
                "{bad} should be rejected at load"
            );
        }
    }

    #[test]
    fn equal_scores_sample_uniformly() {
        let mut rng = StdRng::seed_from_u64(7);
        let mut counts = [0usize; 3];
        const DRAWS: usize = 9_000;
        for _ in 0..DRAWS {
            counts[sample_scored(&[1.5, 1.5, 1.5], 1.0, &mut rng)] += 1;
        }
        for (i, c) in counts.iter().enumerate() {
            let share = *c as f32 / DRAWS as f32;
            assert!(
                (share - 1.0 / 3.0).abs() < 0.03,
                "index {i} took {share} of {DRAWS} draws"
            );
        }
    }

    #[test]
    fn a_large_score_does_not_overflow() {
        let mut rng = StdRng::seed_from_u64(1);
        for _ in 0..100 {
            assert_eq!(sample_scored(&[1e30, 0.0], 1.0, &mut rng), 0);
        }
    }

    #[test]
    fn a_high_temperature_approaches_uniform() {
        let mut rng = StdRng::seed_from_u64(11);
        let mut counts = [0usize; 3];
        const DRAWS: usize = 9_000;
        for _ in 0..DRAWS {
            counts[sample_scored(&[-4.0, 0.0, 6.0], 1_000.0, &mut rng)] += 1;
        }
        for (i, c) in counts.iter().enumerate() {
            let share = *c as f32 / DRAWS as f32;
            assert!(
                (share - 1.0 / 3.0).abs() < 0.03,
                "index {i} took {share} of {DRAWS} draws at a high temperature"
            );
        }
    }

    #[test]
    fn a_zero_temperature_is_argmax() {
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..100 {
            assert_eq!(sample_scored(&[0.0, 2.5, -1.0, 2.4], 0.0, &mut rng), 1);
        }
    }
}
