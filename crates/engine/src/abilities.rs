use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::{BuffKind, FieldBuffKind, FieldScope, StatusKind};
use crate::species::MoveEffect;

pub type AbilityId = String;

/// The ability every companion falls back to when its species declares
/// none. Validated at startup (see `Game::new`) rather than defended at
/// every call site, the same way a missing economy role aborts the load.
pub const FALLBACK_ABILITY_ID: &str = "priority_boost";

/// The ability a new game pre-installs into the player's first routine slot
/// — capturing a program is reached through the Special menu like anything
/// else. Validated at startup the same way `FALLBACK_ABILITY_ID` is.
pub const DECOMPILE_ABILITY_ID: &str = "decompile";

/// Routine slots at `level`, from one constant set. Both public wrappers
/// call this so the companion and player curves cannot drift into two
/// different shapes — only their constants differ.
///
/// The floor of 1 is load-bearing: `COMPANION_ROUTINE_SLOT_BASE` is 0, so a
/// level-1 companion would otherwise have nowhere to put the kit its species
/// grants it at level 1.
fn routine_slots(level: u32, base: u32, per_level: u32, cap: u32) -> usize {
    (base + level / per_level).clamp(1, cap) as usize
}

/// How many routines a companion at `level` can hold — see
/// `tuning::COMPANION_ROUTINE_SLOT_BASE` and friends.
pub fn companion_routine_slots(level: u32) -> usize {
    routine_slots(
        level,
        crate::tuning::COMPANION_ROUTINE_SLOT_BASE,
        crate::tuning::COMPANION_ROUTINE_SLOT_PER_LEVEL,
        crate::tuning::COMPANION_ROUTINE_SLOT_CAP,
    )
}

/// How many routines the player at `level` can hold — see
/// `tuning::PLAYER_ROUTINE_SLOT_BASE` and friends.
pub fn player_routine_slots(level: u32) -> usize {
    routine_slots(
        level,
        crate::tuning::PLAYER_ROUTINE_SLOT_BASE,
        crate::tuning::PLAYER_ROUTINE_SLOT_PER_LEVEL,
        crate::tuning::PLAYER_ROUTINE_SLOT_CAP,
    )
}

/// `level`'s multiplier at `per_level`, clamped at
/// `tuning::ABILITY_SCALE_LEVEL_CAP` because the player has no level
/// ceiling; see `player_routine_slots`, which clamps for the same reason.
fn level_scale(level: u32, per_level: f32) -> f32 {
    let level = level.min(crate::tuning::ABILITY_SCALE_LEVEL_CAP);
    1.0 + level as f32 * per_level
}

/// The multiplier a **stat-point** magnitude is scaled by at `level` — a
/// `Buff` or `FieldBuff` power. See `tuning::ABILITY_STAT_SCALE_PER_LEVEL`
/// for why this is the gentler of the two.
pub fn ability_stat_scale(level: u32) -> f32 {
    level_scale(level, crate::tuning::ABILITY_STAT_SCALE_PER_LEVEL)
}

/// The multiplier an **HP** magnitude is scaled by at `level` — `Damage`,
/// `Drain`, `Heal`, `Debuff`. See `tuning::ABILITY_HP_SCALE_PER_LEVEL`.
pub fn ability_hp_scale(level: u32) -> f32 {
    level_scale(level, crate::tuning::ABILITY_HP_SCALE_PER_LEVEL)
}

/// A stat-point `power` scaled by `ability_stat_scale(level)` and by the
/// caster's `affinity` for this effect's category, rounded once. Negative
/// powers scale too — a sap is a negative-power buff, and it has to sharpen
/// with level and with affinity the same way a buff does.
///
/// Both factors multiply before the single `round`: rounding after each
/// would drop points that one combined multiply keeps.
pub fn scaled_stat_power(power: i32, level: u32, affinity: f32) -> i32 {
    (power as f32 * ability_stat_scale(level) * affinity).round() as i32
}

/// An HP `power` scaled by `ability_hp_scale(level)` and `affinity`, on the
/// same terms as `scaled_stat_power` — only the per-level rate differs.
pub fn scaled_hp_power(power: i32, level: u32, affinity: f32) -> i32 {
    (power as f32 * ability_hp_scale(level) * affinity).round() as i32
}

/// The cooldown armed on a combatant right after it casts an ability whose
/// authored value is `cooldown`, floored at `floor` rounds. Called from both
/// `resolve_one_action` (party side, `floor = 0`, so the authored value is
/// untouched — this is what keeps `decompile` spammable) and `wild_retaliate`
/// (hostile side, `floor = tuning::ENEMY_ROUTINE_MIN_COOLDOWN`, so a mod
/// ability declaring no cooldown still can't fire every single round).
///
/// One function rather than the same `+1` written twice: a comment saying
/// two formulas "match" can't keep them in sync if one drifts, so this is
/// the sync.
///
/// The `+1` is armed before the effect resolves and read again at the end of
/// this same round by `tick_ability_cooldowns` — without it, that tick would
/// eat a round the caster never actually got to wait out.
pub fn armed_cooldown(cooldown: u32, floor: u32) -> u32 {
    cooldown.max(floor) + 1
}

/// Index into `weights` that `roll` selects, treating each weight as the
/// width of a bucket. `roll` is expected in `0..weights.iter().sum()`.
///
/// `None` only when there is genuinely nothing to pick — an empty slice, or
/// every weight zero. An overshooting roll saturates to the last non-zero
/// bucket rather than returning `None`, so a caller that computes its range
/// wrong degrades to a valid pick instead of silently spawning nothing.
///
/// Pure, and takes the roll rather than the RNG, so the distribution can be
/// tested without a `Game`.
pub fn weighted_pick(weights: &[u32], roll: u32) -> Option<usize> {
    let mut remaining = roll;
    let mut last = None;
    for (index, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            continue;
        }
        last = Some(index);
        if remaining < weight {
            return Some(index);
        }
        remaining -= weight;
    }
    last
}

/// Who an ability lands on. Which picker the UI opens for it — if any — is
/// `AbilityTarget::targeting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityTarget {
    /// One party member the player picks.
    OneAlly,
    /// Every living party member, no picker.
    WholeParty,
    /// The front member of one enemy group the player picks.
    OneEnemyGroupFront,
    /// Every living member of one enemy group the player picks.
    WholeEnemyGroup,
    /// Every living enemy in every group, no picker.
    AllEnemies,
}

/// The category an ability's magnitude belongs to, for affinity purposes —
/// one per `AbilityEffect` variant that *has* a magnitude. A caster's
/// affinity for a category multiplies every magnitude in it (see
/// `Game::ability_affinity`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AffinityKind {
    Damage,
    Heal,
    Buff,
    Debuff,
    Drain,
}

impl AffinityKind {
    /// Display label, for the manifest screen. A taxonomy label rather than
    /// authored content, so it lives here and not in a `.ron` — same call
    /// as `Dir::label`.
    pub fn label(self) -> &'static str {
        match self {
            AffinityKind::Damage => "Damage",
            AffinityKind::Heal => "Healing",
            AffinityKind::Buff => "Buffs",
            AffinityKind::Debuff => "Debuffs",
            AffinityKind::Drain => "Drain",
        }
    }

    /// The perk that raises the player's affinity in this category.
    pub fn perk(self) -> crate::perks::Perk {
        match self {
            AffinityKind::Damage => crate::perks::Perk::DamageAffinity,
            AffinityKind::Heal => crate::perks::Perk::HealAffinity,
            AffinityKind::Buff => crate::perks::Perk::BuffAffinity,
            AffinityKind::Debuff => crate::perks::Perk::DebuffAffinity,
            AffinityKind::Drain => crate::perks::Perk::DrainAffinity,
        }
    }

    /// The affinity perk's per-level rate for this category — see
    /// `tuning::AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED`'s doc for why `Damage`
    /// and `Drain` need a different (higher) number than the other three.
    /// One lookup here rather than a match at the call site in
    /// `Game::ability_affinity`.
    pub fn perk_bonus_per_level(self) -> f32 {
        match self {
            AffinityKind::Damage | AffinityKind::Drain => {
                crate::tuning::AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED
            }
            AffinityKind::Heal | AffinityKind::Buff | AffinityKind::Debuff => {
                crate::tuning::AFFINITY_PERK_BONUS_PER_LEVEL
            }
        }
    }
}

/// What an ability does to each of its recipients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AbilityEffect {
    /// Direct damage through `battle::compute_damage`, so it scales with the
    /// user's ATK exactly as a `MoveDef` does, plus an optional status rider
    /// — the same shape a move already has.
    Damage {
        power: i32,
        #[serde(default)]
        status: Option<MoveEffect>,
    },
    Heal {
        power: i32,
    },
    Buff {
        kind: BuffKind,
        power: i32,
        duration: u32,
    },
    Debuff {
        kind: StatusKind,
        power: i32,
        duration: u32,
    },
    /// Damage through `battle::compute_damage`, then the user is healed for
    /// `heal_fraction` of the damage it actually dealt, capped at its own
    /// maximum Integrity.
    ///
    /// Deliberately excluded from `scaled_hp_power`: the heal rides the damage,
    /// which already rides the user's ATK, so this scales with level without
    /// being scaled.
    Drain {
        power: i32,
        /// Clamped to `0.0..=1.0` at load — see `AbilityDb::load_dir`. Bounded
        /// there rather than at use, so a `heal_fraction: 5.0` mod is a
        /// bounded ability instead of a bounded surprise inside a formula.
        heal_fraction: f32,
    },
    /// Clears each recipient's active status condition. Carries no fields.
    Cleanse,
    /// Spends a taming catalyst and rolls `taming::capture_chance` against
    /// the target group's front program — see `Game::attempt_decompile`.
    /// Carries no numbers of its own: the whole formula is `taming`'s, and
    /// duplicating any of it here would be a second copy to drift.
    Decompile,
    /// Arms a `components::ActiveFieldBuff` outside battle rather than
    /// resolving against a recipient in one. This is the field-only marker:
    /// there is no separate `field_cast: bool` on `AbilityDef`, an ability
    /// carrying this effect *is* field-only, and `AbilityDb::load_dir`
    /// rejects a `target` its `kind`'s `FieldScope` can't reach. `power_cost`
    /// is Power spent to cast, not `AbilityDef::fatigue_cost` — that field
    /// (and `cooldown`) are dead on this variant, since neither battle
    /// throttling concept applies outside one.
    FieldBuff {
        kind: FieldBuffKind,
        power: i32,
        duration: u32,
        power_cost: f32,
    },
    /// Steps the party through exactly one solid cell along their current
    /// facing, landing on the open cell beyond — see `Game::phase_landing`.
    ///
    /// Carries no fields, and specifically no depth: one wall is a rule of
    /// the mechanic rather than an authored magnitude, because a two-deep
    /// cast from the frame edge cuts a diagonal across the whole maze. A mod
    /// gets the routine, not a tunneller.
    Phase,
    /// Moves the party to any cell of the current frame the player points
    /// at, and kills them if that cell is solid — see `Game::wild_jump`.
    /// Carries no fields; the gamble is the whole mechanic.
    Jump,
}

impl AbilityEffect {
    /// Whether this effect runs outside battle only. `FieldBuff` was the
    /// original field-only marker and the two Stack movement effects joined
    /// it, which is why this is a predicate rather than a `matches!` at each
    /// of the four sites that need it: `Game::field_routines` (which builds
    /// the cast list), `Game::battle_special_options` and
    /// `Game::wild_routine_ready` (which exclude it from both sides of a
    /// fight), and `use_ability`'s `unreachable!` arm (which is only
    /// unreachable *because* the other three agree with this one).
    pub fn field_only(&self) -> bool {
        matches!(
            self,
            AbilityEffect::FieldBuff { .. } | AbilityEffect::Phase | AbilityEffect::Jump
        )
    }

    /// Which affinity category this effect's magnitude falls under, or
    /// `None` for the two variants that have no magnitude to scale.
    /// `Decompile` is deliberately `None` rather than a category of its own:
    /// the `Decompiler` stat and `Perk::ExploitFocus` already move those
    /// odds, and a third multiplier there is a fourth spelling of the same
    /// thing.
    pub fn affinity_kind(&self) -> Option<AffinityKind> {
        match self {
            AbilityEffect::Damage { .. } => Some(AffinityKind::Damage),
            AbilityEffect::Heal { .. } => Some(AffinityKind::Heal),
            AbilityEffect::Buff { .. } => Some(AffinityKind::Buff),
            AbilityEffect::Debuff { .. } => Some(AffinityKind::Debuff),
            AbilityEffect::Drain { .. } => Some(AffinityKind::Drain),
            // The three that move nothing measurable. `Phase` and `Jump`
            // carry no magnitude at all — how far they reach is fixed by the
            // mechanic — so there is nothing here for an affinity to scale.
            AbilityEffect::Cleanse
            | AbilityEffect::Decompile
            | AbilityEffect::Phase
            | AbilityEffect::Jump => None,
            AbilityEffect::FieldBuff { kind, .. } => kind.affinity_kind(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbilityDef {
    pub id: AbilityId,
    pub name: String,
    /// The one-line detail the ability picker shows. Authored rather than
    /// computed from `effect`, so a modder controls exactly how their
    /// ability reads.
    pub description: String,
    pub target: AbilityTarget,
    pub effect: AbilityEffect,
    /// Battle rounds before this ability can be used again by the same
    /// combatant. `#[serde(default)]` — 0 means usable every round.
    #[serde(default)]
    pub cooldown: u32,
    /// Player Fatigue spent commanding this ability. `#[serde(default)]` to
    /// the flat cost commanding a companion has always charged, so an
    /// ability omitting it behaves as before.
    #[serde(default = "default_fatigue_cost")]
    pub fatigue_cost: f32,
    /// How likely this ability is to be found already installed on a wild
    /// program — see `Game::spawn_wild_creature`. Relative within the pool,
    /// not a probability: weight 12 is twice as likely as weight 6, and the
    /// pool is normalised at pick time.
    ///
    /// `#[serde(default)]` to 0, which means "never spawns wild". Defaulting
    /// to exclusion is what keeps `priority_boost` and `decompile` — and
    /// every other ability reachable through a species or a research node —
    /// out of the pool without this module having to name them.
    #[serde(default)]
    pub wild_weight: u32,
}

fn default_fatigue_cost() -> f32 {
    crate::tuning::COMPANION_COMMAND_FATIGUE_COST
}

impl AbilityDef {
    /// Names the first field holding a NaN or infinity, if any. RON accepts
    /// bare `NaN`/`inf` literals and they survive every clamp downstream —
    /// cheaper to refuse the file at load than to defend every read. Same
    /// rationale as `ItemDef::non_finite_field`.
    fn non_finite_field(&self) -> Option<&'static str> {
        if !self.fatigue_cost.is_finite() {
            return Some("fatigue_cost");
        }
        if let AbilityEffect::Damage {
            status: Some(status),
            ..
        } = &self.effect
            && !status.chance.is_finite()
        {
            return Some("effect.status.chance");
        }
        if let AbilityEffect::Drain { heal_fraction, .. } = &self.effect
            && !heal_fraction.is_finite()
        {
            return Some("effect.heal_fraction");
        }
        if let AbilityEffect::FieldBuff { power_cost, .. } = &self.effect
            && !power_cost.is_finite()
        {
            return Some("effect.power_cost");
        }
        None
    }

    /// `Decompile` is resolved by group index in `Game::attempt_decompile`,
    /// which only ever runs when the planned target is a
    /// `battle::SpecialTarget::EnemyGroup` — the shape `AbilityTarget`'s
    /// `Enemy` targeting produces. Any other `target` would still arm the
    /// cooldown and spend Fatigue in `resolve_one_action`, then find no
    /// group index to act on and silently do nothing: the exact
    /// "wastes-the-round" failure mode this branch refuses loudly for
    /// everywhere else it can reach. Caught here instead, the same way
    /// `non_finite_field` catches a bad number before it reaches a formula.
    fn decompile_target_mismatch(&self) -> Option<&'static str> {
        if matches!(self.effect, AbilityEffect::Decompile)
            && self.target.targeting() != crate::battle::SpecialTargeting::Enemy
        {
            return Some(
                "effect: Decompile requires target: OneEnemyGroupFront or WholeEnemyGroup",
            );
        }
        None
    }

    /// A `FieldBuff` effect paired with a `target` its `kind`'s
    /// `FieldScope` can't reach. A `Run`-scoped kind always lands on the
    /// player (`FieldBuffKind::scope`, `Game::arm_field_buff`), so anything
    /// but `WholeParty` is a stated target the cast never actually honours.
    /// A `Creature`-scoped kind may aim at a party member — `OneAlly` or
    /// `WholeParty` — but never an enemy: there is no mechanic to aim a
    /// field buff at a hostile. Caught here for the same reason
    /// `decompile_target_mismatch` is: refused loudly at load rather than
    /// silently doing nothing (or nothing coherent) at cast time.
    fn field_buff_target_mismatch(&self) -> Option<&'static str> {
        let AbilityEffect::FieldBuff { kind, .. } = &self.effect else {
            return None;
        };
        match (kind.scope(), self.target) {
            (FieldScope::Run, AbilityTarget::WholeParty) => None,
            (FieldScope::Run, _) => {
                Some("effect: a Run-scoped FieldBuff requires target: WholeParty")
            }
            (FieldScope::Creature, AbilityTarget::OneAlly | AbilityTarget::WholeParty) => None,
            (FieldScope::Creature, _) => {
                Some("effect: a Creature-scoped FieldBuff requires target: OneAlly or WholeParty")
            }
        }
    }

    /// A `Phase` or `Jump` effect paired with a `target` other than
    /// `WholeParty`. Both move the party as a body — there is no mechanic to
    /// phase one companion through a wall and leave the rest behind — so any
    /// other target is a stated aim the cast never honours. Refused at load
    /// for the same reason `field_buff_target_mismatch` refuses its own
    /// mismatches rather than quietly doing something else at cast time.
    fn movement_target_mismatch(&self) -> Option<&'static str> {
        if !matches!(self.effect, AbilityEffect::Phase | AbilityEffect::Jump) {
            return None;
        }
        (self.target != AbilityTarget::WholeParty)
            .then_some("effect: Phase and Jump require target: WholeParty")
    }

    /// Names `cooldown` if it's set on a **field-only** ability, where it
    /// does nothing: cooldown throttles re-use within a battle, and a field
    /// ability runs outside one. Not a load failure — the def still loads —
    /// just something worth a modder knowing rather than silently swallowing.
    ///
    /// `fatigue_cost` is deliberately not checked here. On `Phase` and
    /// `Jump` it is the live cost of running the routine, so there would be
    /// nothing to warn about; and on `FieldBuff`, where it genuinely is dead
    /// (`power_cost` is that variant's own cost), its serde default
    /// (`default_fatigue_cost`) is the nonzero flat companion-command cost —
    /// so a file that never mentions the field looks byte-for-byte identical
    /// at this point to one that spells out the same number on purpose.
    /// There's nothing here to distinguish "careless" from "correct", so a
    /// warning on it would fire on every well-formed field buff just as often
    /// as a mistaken one. `cooldown` defaults to 0, so a nonzero value there
    /// is unambiguous authorial intent worth naming.
    /// `assets/abilities/README.md` tells a modder directly which fields
    /// apply to which field-only effect instead.
    fn field_only_dead_fields(&self) -> Option<&'static str> {
        if !self.effect.field_only() {
            return None;
        }
        (self.cooldown != 0).then_some("cooldown")
    }

    /// Bounds a `Drain`'s `heal_fraction` to `0.0..=1.0`. Applied at load so
    /// every reader downstream can treat it as a fraction, rather than each
    /// one re-clamping. Runs after `non_finite_field`, which has already
    /// refused a NaN — `clamp` would panic on one.
    fn clamp_ranges(&mut self) {
        if let AbilityEffect::Drain { heal_fraction, .. } = &mut self.effect {
            *heal_fraction = heal_fraction.clamp(0.0, 1.0);
        }
    }
}

impl AbilityTarget {
    /// Which picker the UI opens after this ability is chosen. `None` means
    /// it resolves immediately — there is nothing left for the player to
    /// choose.
    pub fn targeting(self) -> crate::battle::SpecialTargeting {
        use crate::battle::SpecialTargeting;
        match self {
            AbilityTarget::OneAlly => SpecialTargeting::Ally,
            AbilityTarget::OneEnemyGroupFront | AbilityTarget::WholeEnemyGroup => {
                SpecialTargeting::Enemy
            }
            AbilityTarget::WholeParty | AbilityTarget::AllEnemies => SpecialTargeting::None,
        }
    }
}

#[derive(Resource, Default)]
pub struct AbilityDb {
    abilities: HashMap<AbilityId, AbilityDef>,
}

impl AbilityDb {
    /// Loads every `*.ron` ability in `dir`. A malformed file is skipped
    /// with a returned warning rather than aborting the load, same as
    /// `ItemDb::load_dir`.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = AbilityDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<AbilityDef>(&text) {
                Ok(mut def) => {
                    if let Some(field) = def.non_finite_field() {
                        warnings.push(format!(
                            "skipped invalid ability file {path:?}: {field} is not a finite number"
                        ));
                        continue;
                    }
                    if let Some(reason) = def.decompile_target_mismatch() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    if let Some(reason) = def.field_buff_target_mismatch() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    if let Some(reason) = def.movement_target_mismatch() {
                        warnings.push(format!("skipped invalid ability file {path:?}: {reason}"));
                        continue;
                    }
                    def.clamp_ranges();
                    if let Some(dead) = def.field_only_dead_fields() {
                        warnings.push(format!(
                            "ability file {path:?} sets {dead}, which has no effect on a field-only ability"
                        ));
                    }
                    db.abilities.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid ability file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&AbilityDef> {
        self.abilities.get(id)
    }

    /// Every loaded ability, by id. `HashMap` iteration order is randomized
    /// per instance, so without this the picker's numbering would shuffle
    /// between sessions even though nothing about the files changed.
    pub fn all(&self) -> impl Iterator<Item = &AbilityDef> {
        let mut defs: Vec<&AbilityDef> = self.abilities.values().collect();
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        defs.into_iter()
    }

    /// Every ability that can be found on a wild program, paired with its
    /// weight, ordered by id.
    ///
    /// Ordered for the same reason `all()` is: `HashMap` iteration is
    /// randomised per instance, so a weighted walk over an unordered pool
    /// would not be reproducible from a seed — and every wild spawn in this
    /// game is.
    pub fn wild_pool(&self) -> Vec<(&AbilityDef, u32)> {
        self.all()
            .filter(|d| d.wild_weight > 0)
            .map(|d| (d, d.wild_weight))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `files` as `.ron` into a fresh temp dir and loads an
    /// `AbilityDb` from it.
    fn load(tag: &str, files: &[(&str, &str)]) -> (AbilityDb, Vec<String>) {
        let dir =
            std::env::temp_dir().join(format!("feral_abilities_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
        }
        let result = AbilityDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    const VALID: &str = r#"(
        id: "test_sweep",
        name: "Test Sweep",
        description: "Damage 6 to one group.",
        target: WholeEnemyGroup,
        effect: Damage(power: 6),
    )"#;

    #[test]
    fn a_valid_def_loads_with_defaulted_optional_fields() {
        let (db, warnings) = load("valid", &[("test_sweep", VALID)]);
        let def = db.get("test_sweep").expect("valid ability should load");
        assert_eq!(def.name, "Test Sweep");
        assert_eq!(def.target, AbilityTarget::WholeEnemyGroup);
        assert_eq!(def.cooldown, 0, "cooldown defaults to none");
        assert_eq!(
            def.fatigue_cost,
            crate::tuning::COMPANION_COMMAND_FATIGUE_COST,
            "an ability declaring no cost charges what commanding always did"
        );
        assert!(warnings.is_empty(), "a valid def warns about nothing");
    }

    /// Regression for M11: a `Decompile` effect is resolved by group index
    /// in `Game::attempt_decompile`, which only runs for a
    /// `SpecialTarget::EnemyGroup` — the shape only `OneEnemyGroupFront` and
    /// `WholeEnemyGroup` targeting produces. Pairing it with anything else
    /// would arm the cooldown and spend Fatigue and then silently waste the
    /// round, so it must be refused at load time instead.
    #[test]
    fn a_decompile_effect_paired_with_a_non_group_target_is_skipped() {
        let mismatched = r#"(
            id: "test_bad_decompile",
            name: "Bad Decompile",
            description: "d",
            target: AllEnemies,
            effect: Decompile,
        )"#;
        let (db, warnings) = load(
            "bad_decompile",
            &[("test_sweep", VALID), ("bad", mismatched)],
        );
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_decompile").is_none(),
            "the mismatched pairing must not load"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Decompile"), "{}", warnings[0]);
    }

    /// `Coolant` is `Run`-scoped (`FieldBuffKind::scope`) — it always lands
    /// on the player, so authoring `OneAlly` states a target the cast never
    /// actually reaches.
    #[test]
    fn a_run_scoped_field_buff_targeting_one_ally_is_skipped() {
        let mismatched = r#"(
            id: "test_bad_coolant",
            name: "Bad Coolant",
            description: "d",
            target: OneAlly,
            effect: FieldBuff(kind: Coolant, power: 4, duration: 20, power_cost: 5.0),
        )"#;
        let (db, warnings) = load(
            "bad_run_scope",
            &[("test_sweep", VALID), ("bad", mismatched)],
        );
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_coolant").is_none(),
            "a Run-scoped kind paired with anything but WholeParty must not load"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Run-scoped"), "{}", warnings[0]);
    }

    /// `Regen` is `Creature`-scoped and may aim at `OneAlly` or
    /// `WholeParty` — never an enemy target, since there is no mechanic to
    /// aim a field buff at a hostile.
    #[test]
    fn a_creature_scoped_field_buff_targeting_an_enemy_group_is_skipped() {
        let mismatched = r#"(
            id: "test_bad_regen",
            name: "Bad Regen",
            description: "d",
            target: WholeEnemyGroup,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20, power_cost: 5.0),
        )"#;
        let (db, warnings) = load(
            "bad_creature_scope",
            &[("test_sweep", VALID), ("bad", mismatched)],
        );
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_regen").is_none(),
            "a Creature-scoped kind can't be aimed at an enemy"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Creature-scoped"), "{}", warnings[0]);
    }

    #[test]
    fn a_creature_scoped_field_buff_targeting_one_ally_loads_clean() {
        // fatigue_cost is left at its serde default deliberately: that
        // default is the nonzero flat companion command cost, so a file
        // that never mentions the field must load exactly as warning-free
        // as one that does — see `field_buff_dead_fields`'s doc for why that
        // field is excluded from the dead-fields warning.
        let good = r#"(
            id: "test_good_regen",
            name: "Good Regen",
            description: "d",
            target: OneAlly,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20, power_cost: 5.0),
        )"#;
        let (db, warnings) = load("good_creature_scope", &[("good", good)]);
        assert!(
            db.get("test_good_regen").is_some(),
            "OneAlly is a legal target for a Creature-scoped kind"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn a_field_buff_with_a_non_finite_power_cost_is_skipped() {
        let bad = r#"(
            id: "test_bad_power_cost",
            name: "Bad Power Cost",
            description: "d",
            target: OneAlly,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20, power_cost: NaN),
        )"#;
        let (db, warnings) = load("bad_power_cost", &[("test_sweep", VALID), ("bad", bad)]);
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_power_cost").is_none(),
            "a NaN power_cost must not reach the cast formula"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("power_cost"), "{}", warnings[0]);
    }

    /// `cooldown` throttles re-use within a battle; a field ability runs
    /// outside one, so `cooldown` is dead weight the loader should point out
    /// rather than silently accept. `cooldown` defaults to 0, so any nonzero
    /// value here is unambiguous authorial intent — unlike `fatigue_cost`,
    /// whose own nonzero default makes "the author wrote this on purpose"
    /// indistinguishable from "the author never touched this field" (see the
    /// sibling test below and `field_buff_dead_fields`'s doc).
    #[test]
    fn a_field_buff_declaring_a_cooldown_loads_with_a_warning() {
        let noisy = r#"(
            id: "test_noisy_regen",
            name: "Noisy Regen",
            description: "d",
            target: OneAlly,
            cooldown: 3,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20, power_cost: 5.0),
        )"#;
        let (db, warnings) = load("noisy_field_buff", &[("noisy", noisy)]);
        assert!(
            db.get("test_noisy_regen").is_some(),
            "a dead field is a warning, not a load failure"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("cooldown"), "{}", warnings[0]);
    }

    /// The regression this guards: `fatigue_cost`'s serde default is the
    /// nonzero flat companion command cost, so a naive "warn when nonzero"
    /// check (the shape `cooldown`'s check uses) would fire on this file even
    /// though it never mentions the field at all — indistinguishable from a
    /// file that spells out the same number on purpose. It must stay silent.
    #[test]
    fn a_field_buff_leaving_fatigue_cost_at_its_default_is_silent() {
        let quiet = r#"(
            id: "test_quiet_regen",
            name: "Quiet Regen",
            description: "d",
            target: OneAlly,
            effect: FieldBuff(kind: Regen, power: 2, duration: 20, power_cost: 5.0),
        )"#;
        let (db, warnings) = load("quiet_field_buff", &[("quiet", quiet)]);
        assert!(db.get("test_quiet_regen").is_some());
        assert!(
            warnings.is_empty(),
            "fatigue_cost must never trigger the dead-fields warning: {warnings:?}"
        );
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_the_rest_still_load() {
        let (db, warnings) = load(
            "malformed",
            &[("test_sweep", VALID), ("broken", "(this is not ron")],
        );
        assert!(
            db.get("test_sweep").is_some(),
            "one bad mod file must not take the others down"
        );
        assert_eq!(warnings.len(), 1, "exactly the bad file should warn");
        assert!(warnings[0].contains("broken"));
    }

    #[test]
    fn all_is_ordered_by_id() {
        let b =
            r#"(id: "b", name: "B", description: "d", target: OneAlly, effect: Heal(power: 1))"#;
        let a =
            r#"(id: "a", name: "A", description: "d", target: OneAlly, effect: Heal(power: 1))"#;
        let (db, _) = load("order", &[("b", b), ("a", a)]);
        let ids: Vec<&str> = db.all().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["a", "b"],
            "HashMap order is randomized per instance; the menu must not be"
        );
    }

    #[test]
    fn the_shipped_set_loads_clean() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join("abilities");
        let ron_file_count = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|e| e.to_str()) == Some("ron"))
            .count();
        let (db, warnings) = AbilityDb::load_dir(&dir).unwrap();
        assert!(
            warnings.is_empty(),
            "the shipped set must not warn: {warnings:?}"
        );
        // Counted against the directory rather than a hardcoded number, so
        // shipping a new file never requires hand-updating this assertion.
        // What it still catches that no `tests::assets` coverage check
        // would: two files sharing an `id` silently overwrite one another in
        // `db.abilities.insert` (a `HashMap`, no warning on either side), so
        // the loaded count would fall below the file count with nothing
        // else to notice the collision.
        assert_eq!(
            db.all().count(),
            ron_file_count,
            "every .ron file in assets/abilities should have loaded as a distinct ability"
        );
        assert!(
            db.get(FALLBACK_ABILITY_ID).is_some(),
            "the fallback ability must ship, or every companion loses its Special"
        );
    }

    #[test]
    fn companion_slots_grow_one_per_two_levels_up_to_the_cap() {
        // Level 1 has no slot by the raw formula; the clamp gives it one, so
        // a freshly tamed program still has somewhere to keep its kit.
        let expected = [
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 2),
            (5, 2),
            (6, 3),
            (8, 4),
            (10, 5),
            (12, 6),
        ];
        for (level, slots) in expected {
            assert_eq!(
                companion_routine_slots(level),
                slots,
                "companion level {level}"
            );
        }
        assert_eq!(
            companion_routine_slots(50),
            crate::tuning::COMPANION_ROUTINE_SLOT_CAP as usize,
            "past the cap a companion stops gaining slots"
        );
    }

    #[test]
    fn player_slots_grow_one_per_ten_levels_so_the_first_free_one_lands_at_10() {
        assert_eq!(
            player_routine_slots(1),
            1,
            "the starting slot holds decompile"
        );
        assert_eq!(player_routine_slots(9), 1, "still nothing free at 9");
        assert_eq!(
            player_routine_slots(10),
            2,
            "the first free slot arrives at 10"
        );
        assert_eq!(player_routine_slots(49), 5);
        assert_eq!(player_routine_slots(50), 6);
        assert_eq!(
            player_routine_slots(9_999),
            crate::tuning::PLAYER_ROUTINE_SLOT_CAP as usize,
            "the player has no level cap, so only this clamp bounds their slots"
        );
    }

    #[test]
    fn wild_weight_defaults_to_zero_so_an_ability_opts_in_rather_than_out() {
        let (db, _) = load("wild_default", &[("test_sweep", VALID)]);
        let def = db.get("test_sweep").expect("valid ability should load");
        assert_eq!(
            def.wild_weight, 0,
            "an ability that says nothing must never spawn wild"
        );
    }

    #[test]
    fn wild_pool_holds_only_the_opted_in_abilities_ordered_by_id() {
        let common = r#"(id: "zebra", name: "Zebra", description: "d",
            target: OneAlly, effect: Heal(power: 1), cooldown: 1, wild_weight: 4)"#;
        let rare = r#"(id: "apple", name: "Apple", description: "d",
            target: OneAlly, effect: Heal(power: 1), cooldown: 1, wild_weight: 1)"#;
        let (db, _) = load(
            "wild_pool",
            &[("test_sweep", VALID), ("zebra", common), ("apple", rare)],
        );
        let pool: Vec<(&str, u32)> = db
            .wild_pool()
            .into_iter()
            .map(|(d, w)| (d.id.as_str(), w))
            .collect();
        assert_eq!(
            pool,
            vec![("apple", 1), ("zebra", 4)],
            "weight-0 abilities are excluded, and HashMap order must not leak into a seeded roll"
        );
    }

    #[test]
    fn weighted_pick_is_proportional_to_the_weights() {
        let weights = [1, 3, 1];
        // Roll 0 lands in the first bucket; 1..=3 in the second; 4 in the third.
        assert_eq!(weighted_pick(&weights, 0), Some(0));
        assert_eq!(weighted_pick(&weights, 1), Some(1));
        assert_eq!(weighted_pick(&weights, 3), Some(1));
        assert_eq!(weighted_pick(&weights, 4), Some(2));
    }

    #[test]
    fn weighted_pick_handles_an_empty_pool_and_an_overshooting_roll() {
        assert_eq!(weighted_pick(&[], 0), None, "nothing to pick from");
        assert_eq!(weighted_pick(&[0, 0], 0), None, "all weights excluded");
        assert_eq!(
            weighted_pick(&[2, 3], 99),
            Some(1),
            "an overshooting roll saturates to the last real bucket, never panics"
        );
    }

    #[test]
    fn a_drain_with_a_non_finite_heal_fraction_is_skipped() {
        let bad = r#"(id: "test_bad_drain", name: "Bad Drain", description: "d",
            target: OneEnemyGroupFront, cooldown: 1,
            effect: Drain(power: 8, heal_fraction: NaN))"#;
        let (db, warnings) = load("bad_drain", &[("test_sweep", VALID), ("bad", bad)]);
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_drain").is_none(),
            "a NaN heal fraction must not reach the formula"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("heal_fraction"), "{}", warnings[0]);
    }

    #[test]
    fn an_out_of_range_heal_fraction_is_clamped_rather_than_refused() {
        let greedy = r#"(id: "test_greedy", name: "Greedy", description: "d",
            target: OneEnemyGroupFront, cooldown: 1,
            effect: Drain(power: 8, heal_fraction: 5.0))"#;
        let (db, warnings) = load("greedy_drain", &[("greedy", greedy)]);
        let def = db.get("test_greedy").expect("clamped, not skipped");
        let AbilityEffect::Drain { heal_fraction, .. } = def.effect else {
            panic!("expected a Drain effect");
        };
        assert_eq!(
            heal_fraction, 1.0,
            "a mod asking for 500% lifesteal gets 100%, bounded at load not at use"
        );
        assert!(warnings.is_empty(), "clamping is not a load failure");
    }

    #[test]
    fn a_cleanse_needs_no_fields() {
        let cleanse = r#"(id: "test_cleanse", name: "Cleanse", description: "d",
            target: WholeParty, cooldown: 1, effect: Cleanse)"#;
        let (db, warnings) = load("cleanse", &[("cleanse", cleanse)]);
        assert!(db.get("test_cleanse").is_some(), "{warnings:?}");
    }

    #[test]
    fn both_ability_scales_grow_per_level_and_stop_at_the_shared_cap() {
        assert_eq!(ability_stat_scale(0), 1.0, "no level, no bonus");
        assert_eq!(ability_hp_scale(0), 1.0, "no level, no bonus");
        assert!(
            (ability_stat_scale(12) - 2.8).abs() < 1e-5,
            "a companion at its level cap runs stat routines at 2.8x"
        );
        assert!(
            ability_hp_scale(10) > ability_stat_scale(10),
            "an HP magnitude has Integrity's curve to keep pace with, not ATK's"
        );
        for scale in [ability_stat_scale as fn(u32) -> f32, ability_hp_scale] {
            assert_eq!(
                scale(9_999),
                scale(crate::tuning::ABILITY_SCALE_LEVEL_CAP),
                "the player has no level cap, so this clamp is the only bound"
            );
        }
    }

    #[test]
    fn scaled_power_scales_negative_magnitudes_too() {
        assert_eq!(
            scaled_stat_power(-4, 20, crate::tuning::AFFINITY_NEUTRAL),
            -16,
            "a sap must sharpen with level the same way a buff does"
        );
        assert_eq!(scaled_stat_power(0, 20, crate::tuning::AFFINITY_NEUTRAL), 0);
    }

    #[test]
    fn neutral_affinity_leaves_scaled_power_unchanged() {
        // The regression guard on the signature change: at 1.0 this must be
        // exactly the level-only result the three call sites produced before.
        let level_only = (8.0 * ability_stat_scale(20)).round() as i32;
        assert_eq!(
            scaled_stat_power(8, 20, crate::tuning::AFFINITY_NEUTRAL),
            level_only
        );
    }

    #[test]
    fn affinity_multiplies_on_top_of_the_level_scale() {
        // One combined multiply, not two rounds of rounding: 8 * 1.15 * 1.5
        // is 13.8 -> 14, where rounding twice gives 9 * 1.5 = 13.5 -> 14 by
        // luck at this level and diverges at others.
        assert_eq!(scaled_stat_power(8, 1, 1.5), 14);
    }

    #[test]
    fn affinity_scales_negative_magnitudes_too() {
        // A sap is a negative-power buff (see scaled_stat_power's doc); an affinity
        // has to sharpen it, not flip or flatten it.
        assert_eq!(
            scaled_stat_power(-4, 20, 1.5),
            -(scaled_stat_power(4, 20, 1.5))
        );
    }

    #[test]
    fn only_magnitude_carrying_effects_have_an_affinity_category() {
        use crate::components::{BuffKind, StatusKind};
        assert_eq!(
            AbilityEffect::Heal { power: 8 }.affinity_kind(),
            Some(AffinityKind::Heal)
        );
        assert_eq!(
            AbilityEffect::Damage {
                power: 6,
                status: None
            }
            .affinity_kind(),
            Some(AffinityKind::Damage)
        );
        assert_eq!(
            AbilityEffect::Buff {
                kind: BuffKind::Atk,
                power: 3,
                duration: 3
            }
            .affinity_kind(),
            Some(AffinityKind::Buff)
        );
        assert_eq!(
            AbilityEffect::Debuff {
                kind: StatusKind::Stun,
                power: 0,
                duration: 1
            }
            .affinity_kind(),
            Some(AffinityKind::Debuff)
        );
        assert_eq!(
            AbilityEffect::Drain {
                power: 10,
                heal_fraction: 0.5
            }
            .affinity_kind(),
            Some(AffinityKind::Drain)
        );
        // Cleanse has no number to scale; Decompile's axis is already occupied
        // by the Decompiler stat and Perk::ExploitFocus.
        assert_eq!(AbilityEffect::Cleanse.affinity_kind(), None);
        assert_eq!(AbilityEffect::Decompile.affinity_kind(), None);
    }

    #[test]
    fn armed_cooldown_floors_a_zero_authored_value_but_leaves_a_real_one_alone() {
        assert_eq!(
            armed_cooldown(0, crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN),
            2,
            "a mod ability declaring no cooldown is still floored, plus the +1"
        );
        assert_eq!(
            armed_cooldown(3, crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN),
            4,
            "a real cooldown above the floor is untouched but for the +1"
        );
        assert_eq!(
            armed_cooldown(0, 0),
            1,
            "the party side's floor is 0 — only the +1 applies, which is what \
             leaves `decompile` spammable"
        );
    }
}
