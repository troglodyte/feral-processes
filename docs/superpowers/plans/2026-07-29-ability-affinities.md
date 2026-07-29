# Ability Affinities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each ability category a per-caster multiplier — species carry
affinities as `.ron` data, the player buys them as perks — so a program can be
good at healing rather than good at everything equally.

**Architecture:** One new factor threaded through
`abilities::scaled_power`, resolved once per cast in `Game::use_ability` from
the actor. Species affinities come from a new `#[serde(default)]` field on
`SpeciesDef`; the player's come from five appended `Perk` variants. The player
has no species and a companion has no `Perks`, so the two sources can never
stack.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine, standalone), `serde`/`ron` for
assets, `bevy` + `bevy_egui` (gui). Spec:
`docs/superpowers/specs/2026-07-29-ability-affinities-design.md`.

## Global Constraints

- **Branch:** `feat/ability-affinities`, already created. The spec commit
  `b561074` is its first commit.
- **No save-format bump.** `save::SAVE_FORMAT_VERSION` must not change.
  Species affinities are data reloaded every start; appended `Perk` variants
  keep every existing index valid.
- **`Perk` variants are append-only.** Bincode encodes enums positionally and
  `PlayerSave::unlocked_perks` holds indices. Add the five new variants at the
  **end** of the enum and the **end** of `Perk::all()`. Never reorder.
- **Every new `SpeciesDef` field is `#[serde(default)]`** so existing species
  files, including third-party mods, keep parsing untouched.
- **A malformed `.ron` is skipped with a logged warning, never a panic.**
- **Difficulty magnitudes live in `crates/engine/src/tuning.rs`** as
  documented `pub const`s. Do not inline them in formulas or duplicate `.ron`
  values into them.
- **Affinity range:** `AFFINITY_MIN = 0.5`, `AFFINITY_MAX = 2.0`,
  `AFFINITY_NEUTRAL = 1.0`, `AFFINITY_PERK_BONUS_PER_LEVEL = 0.03`.
- **Update `assets/species/README.md` and `assets/perks/README.md`** in the
  same change as the schema they document. Also root `README.md` and
  `CHANGELOG.md`.
- **Comments explain *why*, never *what*.** Match the surrounding density.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix
  warnings rather than silencing them.
- **Out of scope, do not touch:** `fuse_companions` (parked for a separate
  design pass), finite-checking `taming_difficulty`/`growth_multiplier`,
  per-individual affinity rolls on `Potential`.

---

### Task 1: `AffinityKind` and the affinity-aware `scaled_power`

Adds the category enum, the effect→category mapping, and the third argument
on `scaled_power`. No behaviour change yet: every call site passes
`AFFINITY_NEUTRAL`.

**Files:**
- Modify: `crates/engine/src/tuning.rs` (add four consts near the existing
  `ABILITY_POWER_SCALE_PER_LEVEL` at line 846)
- Modify: `crates/engine/src/abilities.rs` (`AffinityKind`,
  `AbilityEffect::affinity_kind`, `scaled_power` at line 70)
- Modify: `crates/engine/src/game/combat_round.rs` (3 call sites: lines
  ~670, ~683, ~698)
- Test: `crates/engine/src/abilities.rs` (existing `mod tests` at line ~640)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum AffinityKind { Damage, Heal, Buff, Debuff, Drain }` —
    `Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize`
  - `impl AffinityKind { pub fn label(self) -> &'static str }`
  - `impl AbilityEffect { pub fn affinity_kind(&self) -> Option<AffinityKind> }`
  - `pub fn scaled_power(power: i32, level: u32, affinity: f32) -> i32`
  - `tuning::AFFINITY_NEUTRAL: f32`, `AFFINITY_MIN: f32`, `AFFINITY_MAX: f32`,
    `AFFINITY_PERK_BONUS_PER_LEVEL: f32`

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` in `crates/engine/src/abilities.rs`:

```rust
#[test]
fn neutral_affinity_leaves_scaled_power_unchanged() {
    // The regression guard on the signature change: at 1.0 this must be
    // exactly the level-only result the three call sites produced before.
    let level_only = (8.0 * ability_power_scale(20)).round() as i32;
    assert_eq!(scaled_power(8, 20, crate::tuning::AFFINITY_NEUTRAL), level_only);
}

#[test]
fn affinity_multiplies_on_top_of_the_level_scale() {
    // One combined multiply, not two rounds of rounding: 8 * 1.15 * 1.5
    // is 13.8 -> 14, where rounding twice gives 9 * 1.5 = 13.5 -> 14 by
    // luck at this level and diverges at others.
    assert_eq!(scaled_power(8, 1, 1.5), 14);
}

#[test]
fn affinity_scales_negative_magnitudes_too() {
    // A sap is a negative-power buff (see scaled_power's doc); an affinity
    // has to sharpen it, not flip or flatten it.
    assert_eq!(scaled_power(-4, 20, 1.5), -(scaled_power(4, 20, 1.5)));
}

#[test]
fn only_magnitude_carrying_effects_have_an_affinity_category() {
    use crate::components::{BuffKind, StatusKind};
    assert_eq!(
        AbilityEffect::Heal { power: 8 }.affinity_kind(),
        Some(AffinityKind::Heal)
    );
    assert_eq!(
        AbilityEffect::Damage { power: 6, status: None }.affinity_kind(),
        Some(AffinityKind::Damage)
    );
    assert_eq!(
        AbilityEffect::Buff { kind: BuffKind::Atk, power: 3, duration: 3 }
            .affinity_kind(),
        Some(AffinityKind::Buff)
    );
    assert_eq!(
        AbilityEffect::Debuff { kind: StatusKind::Stun, power: 0, duration: 1 }
            .affinity_kind(),
        Some(AffinityKind::Debuff)
    );
    assert_eq!(
        AbilityEffect::Drain { power: 10, heal_fraction: 0.5 }.affinity_kind(),
        Some(AffinityKind::Drain)
    );
    // Cleanse has no number to scale; Decompile's axis is already occupied
    // by the Decompiler stat and Perk::ExploitFocus.
    assert_eq!(AbilityEffect::Cleanse.affinity_kind(), None);
    assert_eq!(AbilityEffect::Decompile.affinity_kind(), None);
}
```

If `BuffKind`/`StatusKind` are not at `crate::components`, resolve the real
path with `grep -rn "pub enum BuffKind" crates/engine/src/` and fix the
`use`. Do not guess.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine abilities:: 2>&1 | tail -30`

Expected: compile failure — `AffinityKind` not found, `scaled_power` takes 2
arguments not 3, no method `affinity_kind`.

- [ ] **Step 3: Add the tuning constants**

In `crates/engine/src/tuning.rs`, immediately after
`ABILITY_POWER_SCALE_LEVEL_CAP` (line ~852):

```rust
/// An ability magnitude's neutral affinity — no bonus, no penalty. The
/// value every `AffinityKind` defaults to, and what a caster with neither
/// a species nor perks resolves to.
pub const AFFINITY_NEUTRAL: f32 = 1.0;

/// Bounds every affinity is clamped to when a species file is loaded.
/// Deliberately wider than `MIN_INDIVIDUAL_ROLL`..`MAX_INDIVIDUAL_ROLL`
/// (0.8-1.2): a damage affinity scales only an ability's *authored* power,
/// which is a minority of `power + ATK - DEF` at a high level, so a narrow
/// band would make damage affinities imperceptible.
///
/// These compound with `ability_power_scale`, which is itself up to 7x.
/// A companion caps at `CREATURE_MAX_LEVEL` (12), so its ceiling is 2.8x
/// from level times `AFFINITY_MAX` — 5.6x an authored power. That is the
/// modder's choice to make, which is the moddability contract.
pub const AFFINITY_MIN: f32 = 0.5;
pub const AFFINITY_MAX: f32 = 2.0;

/// Affinity a player affinity perk adds per level: the perk's multiplier is
/// `AFFINITY_NEUTRAL + this * level`. One shared constant rather than five
/// identical ones, because all five affinity perks are the same shape — see
/// `Perk::affinity_kind`. Matches
/// `EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL`, the closest existing
/// analogue among the perks that multiply rather than add.
pub const AFFINITY_PERK_BONUS_PER_LEVEL: f32 = 0.03;
```

- [ ] **Step 4: Add `AffinityKind` and the mapping**

In `crates/engine/src/abilities.rs`, immediately before `pub enum AbilityEffect`
(line ~143):

```rust
/// The category an ability's magnitude belongs to, for affinity purposes —
/// one per `AbilityEffect` variant that *has* a magnitude. A caster's
/// affinity for a category multiplies every magnitude in it (see
/// `Game::ability_affinity`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
}
```

Then in `impl AbilityEffect` (add the block if the effect enum has no `impl`
yet, next to `AbilityDef`'s):

```rust
impl AbilityEffect {
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
            AbilityEffect::Cleanse | AbilityEffect::Decompile => None,
        }
    }
}
```

- [ ] **Step 5: Give `scaled_power` its third argument**

Replace `scaled_power` (line ~70) with:

```rust
/// `power` scaled by `ability_power_scale(level)` and by the caster's
/// `affinity` for this effect's category, rounded once. Negative powers
/// scale too — a sap is a negative-power buff, and it has to sharpen with
/// level and with affinity the same way a buff does.
///
/// Both factors multiply before the single `round`: rounding after each
/// would drop points that one combined multiply keeps.
pub fn scaled_power(power: i32, level: u32, affinity: f32) -> i32 {
    (power as f32 * ability_power_scale(level) * affinity).round() as i32
}
```

- [ ] **Step 6: Pass neutral at every existing call site**

In `crates/engine/src/game/combat_round.rs`, the three
`abilities::scaled_power(*power, level)` calls become
`abilities::scaled_power(*power, level, crate::tuning::AFFINITY_NEUTRAL)`.
Task 4 replaces the constant with the resolved value; this step only keeps
the tree compiling with behaviour unchanged.

Then fix the four existing test call sites the signature change breaks:

```bash
grep -rn "scaled_power(" crates/engine/src/tests/
```

Add `, crate::tuning::AFFINITY_NEUTRAL` to each. Their asserted values must
not change — that is the point of the neutral constant.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine 2>&1 | tail -20`

Expected: PASS, whole engine suite. A failure here that is *not* a
signature-arity error means the neutral path changed behaviour — stop and
find out why rather than adjusting an assertion.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/tuning.rs crates/engine/src/abilities.rs \
        crates/engine/src/game/combat_round.rs crates/engine/src/tests/
git commit -m "feat: an ability magnitude has a category to be good at"
```

---

### Task 2: `Affinities` on `SpeciesDef`

The data half. A species declares affinities in its `.ron`; the loader
validates and clamps them.

**Files:**
- Modify: `crates/engine/src/species.rs` (`Affinities` struct, `SpeciesDef`
  field at line ~120 after `growth_multiplier`, `non_finite_field`,
  `SpeciesDb::load_dir` at line 158)
- Test: `crates/engine/src/tests/assets.rs`
- Modify: `assets/species/README.md`

**Interfaces:**
- Consumes: `AffinityKind` from Task 1.
- Produces:
  - `pub struct Affinities { pub damage: f32, pub heal: f32, pub buff: f32, pub debuff: f32, pub drain: f32 }` — `Clone, Copy, Debug, Serialize, Deserialize`
  - `impl Affinities { pub const NEUTRAL: Affinities; pub fn get(&self, kind: AffinityKind) -> f32; pub fn non_neutral(&self) -> Vec<(AffinityKind, f32)>; fn clamp_all(&mut self); fn non_finite_field(&self) -> Option<&'static str> }`
  - `SpeciesDef::affinities: Affinities`

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/tests/assets.rs`:

```rust
const AFFINITY_SPECIES: &str = r#"(
    id: "test_healer",
    name: "Test Healer",
    glyph: 'h',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    affinities: (heal: 1.5, damage: 0.8),
)"#;

#[test]
fn a_species_declares_affinities_and_omitted_ones_stay_neutral() {
    let dir = super::support::modded_assets_dir(
        "affinity_species",
        &[],
        &[],
        &[("test_healer.ron", AFFINITY_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), 1.5);
    assert_eq!(aff.get(AffinityKind::Damage), 0.8);
    // The three the file never named must default individually, not leave
    // the whole struct at its all-neutral fallback.
    assert_eq!(aff.get(AffinityKind::Buff), AFFINITY_NEUTRAL);
    assert_eq!(aff.get(AffinityKind::Debuff), AFFINITY_NEUTRAL);
    assert_eq!(aff.get(AffinityKind::Drain), AFFINITY_NEUTRAL);
}

#[test]
fn a_species_file_with_no_affinities_field_still_loads_neutral() {
    // The #[serde(default)] contract that keeps every shipped file and
    // every third-party mod parsing untouched.
    let dir = super::support::modded_assets_dir(
        "affinity_absent",
        &[],
        &[],
        &[("test_plain.ron", super::support::TWO_ABILITY_SPECIES)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_medic").unwrap();
    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert_eq!(aff.get(kind), AFFINITY_NEUTRAL);
    }
}

#[test]
fn an_out_of_range_affinity_is_clamped_at_load() {
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: 99.0");
    let dir = super::support::modded_assets_dir(
        "affinity_clamped",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let aff = game.species_affinities("test_healer").unwrap();
    assert_eq!(aff.get(AffinityKind::Heal), AFFINITY_MAX);
}

#[test]
fn a_nan_affinity_disqualifies_the_file_and_the_rest_still_load() {
    // NaN specifically, not just inf: f32::clamp returns NaN for a NaN
    // input, so the clamp alone would pass this straight through into
    // every magnitude the species ever casts.
    let body = AFFINITY_SPECIES.replace("heal: 1.5", "heal: NaN");
    let dir = super::support::modded_assets_dir(
        "affinity_nan",
        &[],
        &[],
        &[("test_healer.ron", &body)],
        &[],
        &[],
    );
    let game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    assert!(
        game.species_affinities("test_healer").is_none(),
        "a species with a non-finite affinity should not have loaded"
    );
    // A single bad mod file must not take the shipped roster down with it.
    assert!(game.species_affinities("drone").is_some());
}
```

`species_affinities` is a test-facing accessor you add in Step 5. If
`assets.rs` lacks `use` for `Game`/`DifficultyMode`/`AffinityKind`/the
tuning consts, copy the `use` block from a neighbouring test module rather
than inventing one.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine assets:: 2>&1 | tail -30`

Expected: compile failure — no `affinities` field, no `species_affinities`,
no `Affinities`.

- [ ] **Step 3: Add the `Affinities` struct**

In `crates/engine/src/species.rs`, before `pub struct SpeciesDef`:

```rust
fn default_affinity() -> f32 {
    crate::tuning::AFFINITY_NEUTRAL
}

/// A species' multiplier per ability category (see
/// `abilities::AffinityKind`). Every field defaults individually to
/// `AFFINITY_NEUTRAL`, so a file may name only the categories it cares
/// about; the struct as a whole is `#[serde(default)]` on `SpeciesDef`, so
/// a file may omit it entirely. Both defaults are needed — one covers the
/// partial form, the other the absent form.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Affinities {
    #[serde(default = "default_affinity")]
    pub damage: f32,
    #[serde(default = "default_affinity")]
    pub heal: f32,
    #[serde(default = "default_affinity")]
    pub buff: f32,
    #[serde(default = "default_affinity")]
    pub debuff: f32,
    #[serde(default = "default_affinity")]
    pub drain: f32,
}

impl Default for Affinities {
    fn default() -> Self {
        Affinities::NEUTRAL
    }
}

impl Affinities {
    /// No bonus and no penalty in any category — what the player resolves
    /// to before perks, and what a species that declares nothing gets.
    pub const NEUTRAL: Affinities = Affinities {
        damage: crate::tuning::AFFINITY_NEUTRAL,
        heal: crate::tuning::AFFINITY_NEUTRAL,
        buff: crate::tuning::AFFINITY_NEUTRAL,
        debuff: crate::tuning::AFFINITY_NEUTRAL,
        drain: crate::tuning::AFFINITY_NEUTRAL,
    };

    pub fn get(&self, kind: AffinityKind) -> f32 {
        match kind {
            AffinityKind::Damage => self.damage,
            AffinityKind::Heal => self.heal,
            AffinityKind::Buff => self.buff,
            AffinityKind::Debuff => self.debuff,
            AffinityKind::Drain => self.drain,
        }
    }

    /// Every category this species is not neutral in, in a fixed order —
    /// what the manifest screen lists. A species with nothing to say
    /// returns empty and the screen shows no section at all, rather than
    /// five rows of 1.00.
    pub fn non_neutral(&self) -> Vec<(AffinityKind, f32)> {
        [
            AffinityKind::Damage,
            AffinityKind::Heal,
            AffinityKind::Buff,
            AffinityKind::Debuff,
            AffinityKind::Drain,
        ]
        .into_iter()
        .map(|k| (k, self.get(k)))
        .filter(|&(_, v)| v != crate::tuning::AFFINITY_NEUTRAL)
        .collect()
    }

    /// Names the first non-finite field, if any. RON accepts bare
    /// `NaN`/`inf` literals, and `f32::clamp` returns NaN for a NaN input
    /// — so the clamp below cannot contain one and the file has to be
    /// refused instead. Same rationale as `AbilityDef::non_finite_field`.
    fn non_finite_field(&self) -> Option<&'static str> {
        for (name, v) in [
            ("damage", self.damage),
            ("heal", self.heal),
            ("buff", self.buff),
            ("debuff", self.debuff),
            ("drain", self.drain),
        ] {
            if !v.is_finite() {
                return Some(name);
            }
        }
        None
    }

    fn clamp_all(&mut self) {
        for v in [
            &mut self.damage,
            &mut self.heal,
            &mut self.buff,
            &mut self.debuff,
            &mut self.drain,
        ] {
            *v = v.clamp(crate::tuning::AFFINITY_MIN, crate::tuning::AFFINITY_MAX);
        }
    }
}
```

Add `use crate::abilities::AffinityKind;` to the module's imports if absent.

- [ ] **Step 4: Add the field to `SpeciesDef`**

Immediately after `growth_multiplier` (line ~120):

```rust
    /// This species' per-category ability multipliers — what a member of it
    /// is *good at*, as opposed to `growth_multiplier`, which scales all
    /// three stats uniformly. Applies to whatever is installed in its
    /// routine slots, not only to the abilities this file grants, so a
    /// strong heal affinity with no innate heal is a reason to install one
    /// here rather than a contradiction. `#[serde(default)]` so existing
    /// species files (including mods) without this field keep parsing at
    /// neutral.
    #[serde(default)]
    pub affinities: Affinities,
```

- [ ] **Step 5: Validate and clamp in `load_dir`, and add the accessor**

In `SpeciesDb::load_dir`, inside the `Ok(mut def)` arm, **before** the
existing `def.abilities.retain(...)`:

```rust
                    if let Some(field) = def.affinities.non_finite_field() {
                        warnings.push(format!(
                            "skipped invalid species file {path:?}: \
                             affinities.{field} is not a finite number"
                        ));
                        continue;
                    }
                    def.affinities.clamp_all();
```

Add to `impl Game` (put it in `crates/engine/src/game/inspection.rs`, next to
the other read-only lookups):

```rust
    /// A species' affinities, or `None` if no such species loaded.
    pub fn species_affinities(&self, id: &str) -> Option<Affinities> {
        self.world
            .resource::<SpeciesDb>()
            .get(id)
            .map(|s| s.affinities)
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine assets:: 2>&1 | tail -20`

Expected: PASS, all four new tests.

- [ ] **Step 7: Document the schema**

In `assets/species/README.md`, immediately after the `growth_multiplier`
block, add:

```ron
    // Optional; can be left out entirely, and so can any individual
    // category. Each is a multiplier on the *magnitude* of abilities in
    // that category when a member of this species casts them: 1.1 is a 10%
    // stronger heal, 0.8 a 20% weaker one. Clamped to 0.5-2.0 at load; a
    // non-finite value (RON accepts bare `NaN`/`inf`) skips the whole file
    // with a warning.
    //
    // The five categories match what an ability's `effect` does — see
    // ../abilities/README.md. `Cleanse` and `Decompile` have no magnitude
    // and so have no affinity.
    //
    // This applies to whatever is *installed* in the program's routine
    // slots, not only to the abilities listed above — and a routine can be
    // popped out and installed on a different species entirely. So a
    // species with a strong `heal` and no innate heal is not a mistake:
    // it's a reason to spend a researched heal routine on that program.
    affinities: (heal: 1.4, damage: 0.85),
```

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/species.rs crates/engine/src/game/inspection.rs \
        crates/engine/src/tests/assets.rs assets/species/README.md
git commit -m "feat: a species declares what its programs are good at"
```

---

### Task 3: Five affinity perks

The player half. Appended enum variants, a generic category mapping, and
five catalogue files.

**Files:**
- Modify: `crates/engine/src/perks.rs` (five variants, `all()`,
  `affinity_kind`)
- Create: `assets/perks/damage_affinity.ron`, `heal_affinity.ron`,
  `buff_affinity.ron`, `debuff_affinity.ron`, `drain_affinity.ron`
- Test: `crates/engine/src/tests/perks.rs`
- Modify: `assets/perks/README.md`

**Interfaces:**
- Consumes: `AffinityKind` (Task 1).
- Produces:
  - `Perk::{DamageAffinity, HealAffinity, BuffAffinity, DebuffAffinity, DrainAffinity}`
  - `Perk::all() -> [Perk; 12]`
  - `impl Perk { pub fn affinity_kind(self) -> Option<AffinityKind> }`
  - `impl AffinityKind { pub fn perk(self) -> Perk }`

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/tests/perks.rs`:

```rust
#[test]
fn the_original_seven_perks_keep_their_positions() {
    // Perk's variant order IS the save format: bincode encodes an enum
    // positionally and PlayerSave::unlocked_perks holds indices, so a
    // reordering would turn one player's Attacker levels into Defender
    // levels on load. The five affinity perks must be appended.
    let all = Perk::all();
    assert_eq!(all[0], Perk::KeenScavenger);
    assert_eq!(all[1], Perk::LowPowerMode);
    assert_eq!(all[2], Perk::ExploitFocus);
    assert_eq!(all[3], Perk::LeanCompiler);
    assert_eq!(all[4], Perk::Attacker);
    assert_eq!(all[5], Perk::Defender);
    assert_eq!(all[6], Perk::Buffer);
    assert_eq!(all.len(), 12);
}

#[test]
fn every_affinity_kind_maps_to_a_perk_and_back() {
    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert_eq!(kind.perk().affinity_kind(), Some(kind));
    }
}

#[test]
fn a_non_affinity_perk_has_no_category() {
    assert_eq!(Perk::Attacker.affinity_kind(), None);
    assert_eq!(Perk::KeenScavenger.affinity_kind(), None);
}

#[test]
fn all_five_affinity_perks_are_on_offer_in_the_picker() {
    // Driven by PerkDb::catalogue, so this is really "all five .ron files
    // parse" — a file naming a variant the build lacks is rejected by RON
    // as an unknown variant and the perk silently stops being offered.
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let offered: Vec<Perk> = game.perk_defs().iter().map(|d| d.id).collect();
    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert!(
            offered.contains(&kind.perk()),
            "{:?} affinity perk is not on offer",
            kind
        );
    }
}
```

`Game::perk_defs` (`game/catalog.rs:271`) returns `Vec<PerkDef>` off
`PerkDb::catalogue()`, so the `.iter().map(|d| d.id)` chain above is correct
as written.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine perks:: 2>&1 | tail -30`

Expected: compile failure — no `Perk::HealAffinity`, no `affinity_kind`, no
`AffinityKind::perk`.

- [ ] **Step 3: Append the five variants**

At the **end** of `pub enum Perk` in `crates/engine/src/perks.rs`, after
`Buffer`:

```rust
    /// Multiplies the magnitude of the player's own `Damage` abilities by
    /// `AFFINITY_PERK_BONUS_PER_LEVEL` per level. Scoped to the player's
    /// own casts: a companion's affinity is its species' business, and a
    /// party-wide perk would multiply against it.
    DamageAffinity,
    /// As `DamageAffinity`, for `Heal`.
    HealAffinity,
    /// As `DamageAffinity`, for `Buff` — including saps, which are
    /// negative-power buffs.
    BuffAffinity,
    /// As `DamageAffinity`, for `Debuff`.
    DebuffAffinity,
    /// As `DamageAffinity`, for `Drain`'s damage. Its `heal_fraction`
    /// rides the damage dealt and is not scaled again.
    DrainAffinity,
```

Append the same five to the end of `Perk::all()`'s array and change its
return type to `[Perk; 12]`.

- [ ] **Step 4: Add the two mappings**

In `impl Perk`:

```rust
    /// Which affinity category this perk multiplies, or `None` for the
    /// seven perks that do something else entirely. The one hook all five
    /// affinity perks share — they have a common shape, unlike the perks
    /// above them, so they get a common mapping rather than five bespoke
    /// arms in `unlock_perk`.
    pub fn affinity_kind(self) -> Option<AffinityKind> {
        match self {
            Perk::DamageAffinity => Some(AffinityKind::Damage),
            Perk::HealAffinity => Some(AffinityKind::Heal),
            Perk::BuffAffinity => Some(AffinityKind::Buff),
            Perk::DebuffAffinity => Some(AffinityKind::Debuff),
            Perk::DrainAffinity => Some(AffinityKind::Drain),
            _ => None,
        }
    }
```

In `crates/engine/src/abilities.rs`, in `impl AffinityKind`:

```rust
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
```

- [ ] **Step 5: Write the five catalogue files**

`assets/perks/heal_affinity.ron`:

```ron
(
    id: HealAffinity,
    name: "Field Medic",
    description: "Your own repair routines mend deeper. +3% healing per level.",
    cost: 2,
)
```

`assets/perks/damage_affinity.ron`:

```ron
(
    id: DamageAffinity,
    name: "Payload Tuning",
    description: "Your own offensive routines bite harder. +3% damage per level.",
    cost: 2,
)
```

`assets/perks/buff_affinity.ron`:

```ron
(
    id: BuffAffinity,
    name: "Overclocker",
    description: "Your own boosts and saps run stronger. +3% per level.",
    cost: 2,
)
```

`assets/perks/debuff_affinity.ron`:

```ron
(
    id: DebuffAffinity,
    name: "Corruption Vector",
    description: "Your own afflictions land heavier. +3% per level.",
    cost: 2,
)
```

`assets/perks/drain_affinity.ron`:

```ron
(
    id: DrainAffinity,
    name: "Siphon Protocol",
    description: "Your own siphons take more. +3% drain damage per level.",
    cost: 2,
)
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine perks:: 2>&1 | tail -20`

Expected: PASS, all four new tests.

- [ ] **Step 7: Document the perk catalogue**

In `assets/perks/README.md`, note that the five `*_affinity.ron` perks share
one magnitude constant (`AFFINITY_PERK_BONUS_PER_LEVEL`) rather than having
one each, that they scale **only the player's own** abilities, and that their
`description` is authored text you are free to rewrite — the engine never
derives it from the constant, so a re-tuned constant leaves the copy stale
until you edit it. Follow the file's existing voice.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/perks.rs crates/engine/src/abilities.rs \
        crates/engine/src/tests/perks.rs assets/perks/
git commit -m "feat: five perks for what the operator is good at"
```

---

### Task 4: Resolve and apply affinity in battle

The behaviour change. This is the task where a heal actually heals more.

**Files:**
- Modify: `crates/engine/src/game/combat.rs` (`ability_affinity`, next to
  `ability_user_level` at line 623)
- Modify: `crates/engine/src/game/combat_round.rs` (`use_ability` at line
  631: resolve once, pass to the three `scaled_power` calls and to the two
  damage arms)
- Test: `crates/engine/src/tests/combat_abilities.rs`

**Interfaces:**
- Consumes: `AffinityKind`, `scaled_power` (Task 1); `Affinities`,
  `species_affinities` (Task 2); `Perk::affinity_kind`, `AffinityKind::perk`
  (Task 3).
- Produces: `pub(crate) fn ability_affinity(&self, actor: Entity, effect: &AbilityEffect) -> f32`

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/tests/combat_abilities.rs`:

```rust
/// A `test_medic` (support::TWO_ABILITY_SPECIES) with a heal affinity —
/// same species, same `hot_patch`, one number different.
const HEALER_WITH_AFFINITY: &str = r#"(
    id: "test_medic",
    name: "Test Medic",
    glyph: 'm',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [(id: "hot_patch")],
    affinities: (heal: 1.5),
)"#;

#[test]
fn a_species_heal_affinity_scales_the_heal_it_casts() {
    let dir = support::modded_assets_dir(
        "heal_affinity_battle",
        &[],
        &[],
        &[("test_medic.ron", HEALER_WITH_AFFINITY)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    // Same spawn body as support::game_with_two_ability_companion — a
    // companion without install_innate_routines has no hot_patch to cast.
    let medic = game
        .world
        .spawn((
            Creature {
                species: "test_medic".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 5,
                def: 1,
            },
            Tamed { owner: player },
            Experience::default(),
        ))
        .id();
    game.install_innate_routines(medic);
    game.add_companion(medic).unwrap();

    // Wound the player so a heal has room to land, then have the medic
    // cast hot_patch (Heal(power: 8)) on them. Ally slot 0 is the player.
    let before = 20;
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 200;
        stats.hp = before;
    }
    let enemy = support::spawn_wild_on_player_tile(&mut game);
    support::insert_battle(&mut game, player, vec![enemy]);
    support::companion_uses_special(
        &mut game,
        medic,
        0,
        battle::SpecialTarget::Ally { slot: 0 },
    );

    let healed = game.world.get::<Stats>(player).unwrap().hp - before;
    // hot_patch is Heal(power: 8); the medic is level 1.
    let expected = crate::abilities::scaled_power(8, 1, 1.5);
    assert_eq!(healed, expected, "heal affinity should scale the heal");
    assert!(
        expected > crate::abilities::scaled_power(8, 1, AFFINITY_NEUTRAL),
        "the fixture must actually differ from neutral, or this proves nothing"
    );
}

#[test]
fn a_player_affinity_perk_scales_the_players_own_ability() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let effect = AbilityEffect::Heal { power: 8 };
    let before = game.ability_affinity(player, &effect);

    {
        let mut perks = game.world.get_mut::<Perks>(player).unwrap();
        perks.points = 99;
    }
    game.unlock_perk(Perk::HealAffinity).unwrap();
    game.unlock_perk(Perk::HealAffinity).unwrap();

    assert_eq!(before, AFFINITY_NEUTRAL);
    assert_eq!(
        game.ability_affinity(player, &effect),
        AFFINITY_NEUTRAL + 2.0 * AFFINITY_PERK_BONUS_PER_LEVEL
    );
}

#[test]
fn a_player_affinity_perk_does_not_scale_a_companions_ability() {
    // The scoping decision, asserted directly: the perk is the player's
    // own, and a companion answers to its species instead.
    let (mut game, medic) = support::game_with_two_ability_companion();
    let player = game.player_entity();
    {
        let mut perks = game.world.get_mut::<Perks>(player).unwrap();
        perks.points = 99;
    }
    game.unlock_perk(Perk::HealAffinity).unwrap();

    let effect = AbilityEffect::Heal { power: 8 };
    assert!(game.ability_affinity(player, &effect) > AFFINITY_NEUTRAL);
    assert_eq!(
        game.ability_affinity(medic, &effect),
        AFFINITY_NEUTRAL,
        "the player's perk must not reach a companion's cast"
    );
}

#[test]
fn an_effect_with_no_category_takes_no_multiplier() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut perks = game.world.get_mut::<Perks>(player).unwrap();
        perks.points = 99;
    }
    game.unlock_perk(Perk::HealAffinity).unwrap();
    // Cleanse has no magnitude; a perk must not invent one for it.
    assert_eq!(
        game.ability_affinity(player, &AbilityEffect::Cleanse),
        AFFINITY_NEUTRAL
    );
}
```

The spawn body duplicates `support::game_with_two_ability_companion`
(`support.rs:698`), which cannot be reused directly because it hardcodes
`TWO_ABILITY_SPECIES` rather than taking a body. If a second test in this
task needs the same setup, extract a `support.rs` helper taking the species
`.ron` body and refactor `game_with_two_ability_companion` to call it —
otherwise leave the duplication, per "three similar lines beat a speculative
abstraction".

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine combat_abilities:: 2>&1 | tail -30`

Expected: compile failure on `ability_affinity`; once that resolves, the
heal test fails with the neutral figure (9, not 14).

- [ ] **Step 3: Implement `ability_affinity`**

In `crates/engine/src/game/combat.rs`, immediately after
`ability_user_level` (line ~628):

```rust
    /// The caster's multiplier for `effect`'s category — the affinity half
    /// of an ability's magnitude, alongside `ability_user_level`'s scale.
    /// Resolved from `actor`, never from a recipient: an affinity is a
    /// property of who casts.
    ///
    /// The player's comes from perks and a companion's from its species,
    /// and the two can never stack — the player has no `Creature` and a
    /// companion has no `Perks` — so there is no combination rule here on
    /// purpose. A wild program has a `Creature` like any other, which is
    /// how a species affinity reaches a hostile carrier for free.
    pub(crate) fn ability_affinity(&self, actor: Entity, effect: &AbilityEffect) -> f32 {
        let Some(kind) = effect.affinity_kind() else {
            return AFFINITY_NEUTRAL;
        };
        if self.world.get::<Perks>(actor).is_some() {
            return AFFINITY_NEUTRAL
                + AFFINITY_PERK_BONUS_PER_LEVEL * self.player_perk_level(kind.perk()) as f32;
        }
        self.world
            .get::<Creature>(actor)
            .and_then(|c| self.species_affinities(&c.species))
            .map(|a| a.get(kind))
            .unwrap_or(AFFINITY_NEUTRAL)
    }
```

`player_perk_level` is `pub fn` on `Game` in `game/unlocks.rs:14`. Import
`AFFINITY_NEUTRAL` and `AFFINITY_PERK_BONUS_PER_LEVEL` from
`crate::tuning`.

- [ ] **Step 4: Apply it in `use_ability`**

In `crates/engine/src/game/combat_round.rs`, next to where `level` is
resolved (line ~641), add:

```rust
        // Resolved once for the whole cast, for the same reason `level` is:
        // this is the caster's property, and re-reading it inside the
        // recipient loop would invite keying it off the recipient.
        let affinity = self.ability_affinity(actor, &ability.effect);
```

Then replace the neutral constant Task 1 left in the three `scaled_power`
calls with `affinity`, and scale the authored power in the two arms that go
through `compute_damage`:

- `AbilityEffect::Damage` — replace `*power` in the `compute_damage` call
  with `abilities::scaled_affinity_power(*power, affinity)`.
- `AbilityEffect::Drain` — same substitution for its damage power. Leave
  `heal_fraction` alone: it multiplies the damage actually dealt, which has
  already been scaled, so touching it again double-dips.

Add to `crates/engine/src/abilities.rs`:

```rust
/// `power` scaled by `affinity` alone, for the two effects whose magnitude
/// goes through `battle::compute_damage` rather than standing on its own.
/// Level scaling is deliberately absent: `compute_damage` adds the caster's
/// ATK, which already grows with level, and applying
/// `ability_power_scale` here as well would scale the same progression
/// twice.
pub fn scaled_affinity_power(power: i32, affinity: f32) -> i32 {
    (power as f32 * affinity).round() as i32
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine combat_abilities:: combat_specials:: 2>&1 | tail -20`

Expected: PASS. If a pre-existing `combat_specials` test moves, a shipped
species has picked up a non-neutral affinity it should not have yet — Task 5
is where shipped species get theirs, not this task.

- [ ] **Step 6: Run the full suite and the balance gate**

Run:
```bash
cargo test -p feral-processes-engine balance_sim 2>&1 | tail -10
cargo test --workspace 2>&1 | tail -20
```

Expected: PASS, and **no `balance_sim` curve movement** — it models no
abilities (see its doc comments at lines 263 and 699), so it cannot see this
feature. Movement here means something outside abilities changed; stop and
find out what.

- [ ] **Step 7: Commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/game/combat.rs crates/engine/src/game/combat_round.rs \
        crates/engine/src/abilities.rs crates/engine/src/tests/
git commit -m "feat: a caster's affinity moves what its routines are worth"
```

---

### Task 5: Affinities on the shipped roster, and a wild carrier

Gives the feature content to be visible through, and pins the hostile path.

**Files:**
- Modify: 4-6 files in `assets/species/`
- Test: `crates/engine/src/tests/combat_abilities.rs`

**Interfaces:**
- Consumes: everything from Tasks 1-4. Produces no new API.

- [ ] **Step 1: Write the failing test**

The hostile path. `use_ability` serves both sides, so a wild carrier of a
species with a damage affinity should hit for more — this pins that rather
than leaving it to be discovered.

```rust
#[test]
fn a_wild_carrier_gets_its_species_damage_affinity() {
    const BITER: &str = r#"(
    id: "test_biter",
    name: "Test Biter",
    glyph: 'b',
    color: Red,
    base_hp: 40,
    base_atk: 6,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    affinities: (damage: 2.0),
)"#;
    let dir = support::modded_assets_dir(
        "wild_damage_affinity",
        &[],
        &[],
        &[("test_biter.ron", BITER)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    // Resolve through the same entry point battle uses, on a wild entity,
    // rather than asserting a damage total that pack composition and
    // initiative both move.
    let biter = support::spawn_wild_without_routine(&mut game, "test_biter", 3, 3);
    let effect = AbilityEffect::Damage { power: 6, status: None };
    assert_eq!(game.ability_affinity(biter, &effect), 2.0);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine wild_carrier 2>&1 | tail -20`

Expected: FAIL — asserts 2.0, gets 1.0 — only if you have *not* yet done
Task 4. If Task 4 is done this test should pass immediately; that is fine,
it is a characterisation test for the hostile path. Note in the commit which
it was.

- [ ] **Step 3: Give shipped species affinities**

Read every file in `assets/species/` first. Then, per the spec's rule rather
than a fixed table:

- At most **one strength and one weakness** per species.
- Leave most of the roster **neutral**. A roster where every species has five
  non-1.0 numbers is a roster where affinity means nothing.
- Pick from what each species already reads as — its `name`, `glyph`, stat
  spread, `moves`, and `abilities` list. A species whose innate ability is a
  heal is the obvious `heal` candidate; a `base_atk`-heavy one is the obvious
  `damage` candidate.
- Values inside `AFFINITY_MIN`..`AFFINITY_MAX`, and away from the bounds for
  a first pass — roughly 0.8 to 1.4.
- Do **not** give a boss a non-neutral affinity in this pass. A boss's stats
  are already authored huge and `is_boss` bars decompiling, so an affinity on
  top moves a number nobody has playtested.

Record which species you changed and why in the commit body.

- [ ] **Step 4: Run the full suite**

Run:
```bash
cargo test -p feral-processes-engine balance_sim 2>&1 | tail -10
cargo test --workspace 2>&1 | tail -20
```

Expected: PASS. `balance_sim` reads the real `.ron` assets, so if a curve
moves here it means a species file edit changed a stat, not an affinity —
`balance_sim` models no abilities. Check your diff for an accidental edit
outside `affinities:`.

- [ ] **Step 5: Commit**

```bash
cargo fmt
git add assets/species/ crates/engine/src/tests/combat_abilities.rs
git commit -m "feat: the roster gets things it is good and bad at"
```

---

### Task 6: Show affinities on the manifest screen

Without this the feature is invisible, which is why it is in scope.

**Files:**
- Modify: `crates/engine/src/views.rs` (`ProgramManifest` at line ~500)
- Modify: `crates/engine/src/game/inspection.rs` (construction at line ~307)
- Modify: `crates/gui/src/render/manifest.rs` (`program_sections` at line 354)
- Test: `crates/engine/src/tests/inspection.rs`

**Interfaces:**
- Consumes: `Affinities::non_neutral`, `AffinityKind::label` (Tasks 1-2).
- Produces: `ProgramManifest::affinities: Vec<(AffinityKind, f32)>`

- [ ] **Step 1: Write the failing test**

Add to `crates/engine/src/tests/inspection.rs`:

```rust
#[test]
fn the_manifest_lists_only_non_neutral_affinities() {
    const LOPSIDED: &str = r#"(
    id: "test_lopsided",
    name: "Test Lopsided",
    glyph: 'l',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [(name: "Poke", power: 3)],
    affinities: (heal: 1.4, damage: 0.8),
)"#;
    let dir = support::modded_assets_dir(
        "manifest_affinities",
        &[],
        &[],
        &[("test_lopsided.ron", LOPSIDED)],
        &[],
        &[],
    );
    let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
    let entity = support::spawn_wild_without_routine(&mut game, "test_lopsided", 3, 3);
    // `Game::manifest` is the public entry; `program_manifest` behind it is
    // private to game::inspection and not reachable from here.
    let ManifestSubject::Program(p) = game.manifest(entity).unwrap().subject else {
        panic!("expected a program manifest");
    };
    assert_eq!(
        p.affinities,
        vec![(AffinityKind::Damage, 0.8), (AffinityKind::Heal, 1.4)],
        "listed in AffinityKind order, and the three neutral ones omitted"
    );
}
```

Note the expected order is `Damage` then `Heal` — `non_neutral` walks the
categories in declaration order, not the order the `.ron` names them.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine manifest_lists 2>&1 | tail -20`

Expected: compile failure — `ProgramManifest` has no `affinities`.

- [ ] **Step 3: Add the view field**

In `crates/engine/src/views.rs`, in `ProgramManifest`, next to
`growth_multiplier`:

```rust
    /// Categories this species is not neutral in, in `AffinityKind` order.
    /// Empty for a species that declares nothing, so the screen omits the
    /// section entirely rather than drawing five rows of 1.00.
    pub affinities: Vec<(AffinityKind, f32)>,
```

- [ ] **Step 4: Populate it**

In `crates/engine/src/game/inspection.rs`, in the `ProgramManifest { ... }`
literal (line ~307), next to `growth_multiplier: species.growth_multiplier`:

```rust
                affinities: species.affinities.non_neutral(),
```

- [ ] **Step 5: Run the engine test to verify it passes**

Run: `cargo test -p feral-processes-engine manifest_lists 2>&1 | tail -20`

Expected: PASS.

- [ ] **Step 6: Draw it**

In `crates/gui/src/render/manifest.rs`, in `program_sections`, after the
`POTENTIAL` section's `if let Some(q) = &p.potential { ... }` block (line
~366) and before the `species` vec is built:

```rust
    if !p.affinities.is_empty() {
        sections.push(Section {
            title: "AFFINITIES",
            rows: section_rows(
                p.affinities
                    .iter()
                    .map(|&(kind, v)| stat(kind.label(), format!("{v:.2}x")))
                    .collect(),
            ),
            full_width: false,
        });
    }
```

`stat`'s first parameter is a `&str` label in the existing calls — check
whether it takes `&str` or `String` and match it; `AffinityKind::label`
returns `&'static str`. A conditional section is already supported and
tested: see `manifest_layout.rs:311`, which covers a subject missing one.

- [ ] **Step 7: Run the full suite**

Run: `cargo test --workspace 2>&1 | tail -20`

Expected: PASS. `manifest_layout.rs`'s tests count sections (line ~198), so
if one fails it is asserting a fixed section count that a new conditional
section moved — read the test and update the expectation deliberately, do
not delete it.

- [ ] **Step 8: Commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/views.rs crates/engine/src/game/inspection.rs \
        crates/gui/src/render/manifest.rs crates/engine/src/tests/inspection.rs \
        crates/gui/src/render/manifest_layout.rs
git commit -m "feat: the manifest says what a program is good at"
```

---

### Task 7: User-facing docs

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**Interfaces:** Consumes everything. Produces no API.

- [ ] **Step 1: Grep for claims this change falsifies**

```bash
grep -n "growth_multiplier\|perk\|Perk\|seven perks\|ability\|abilities" README.md
```

Any sentence saying the perk menu holds seven perks, or that a species
differs only in growth rate, is now false. Fix each one you find — this
project's standing doc obligation is that a change fixes the claims it
falsifies, not just adds a section.

- [ ] **Step 2: Add a README section**

Add an `## Affinities` section after `## Companions`. Cover: what an affinity
is; that species carry them and the player buys them as perks; that the two
never stack; that they apply to whatever is *installed* in a routine slot, so
a strong heal affinity is a reason to move a heal routine onto that program;
and that `Cleanse` and `Decompile` have none. Match the file's existing
voice — second person, concrete, no bullet-point feature lists.

- [ ] **Step 3: Add a CHANGELOG entry**

Under `## Unreleased`, add an `### Affinities` block in the voice of the
existing entries. State explicitly that **there is no save-format bump** and
why (species affinities are data; `Perk` variants were appended so existing
indices stay valid). Say plainly that the magnitudes are unplayed — the
0.03 per perk level, the 0.5-2.0 clamp, and the shipped species values are
arithmetic-plausible and have never been playtested, and `balance_sim`
cannot see them because it models no abilities.

- [ ] **Step 4: Verify the save-format claim before publishing it**

```bash
git diff main -- crates/engine/src/save.rs
grep -n "SAVE_FORMAT_VERSION" crates/engine/src/save.rs
```

Expected: no diff, version unchanged. If either shows a change, the
CHANGELOG claim is false — fix the code or the claim, whichever is wrong.

- [ ] **Step 5: Full gate**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -20
```

Expected: all green, no warnings.

- [ ] **Step 6: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: record affinities, and that nobody has played them"
```

---

## Self-review notes

Spec sections and where they land: categories → Task 1. Species data,
`serde` defaults, finite check, clamp → Task 2. Perks, append-only ordering,
shared const → Task 3. Resolution, no-stacking, `use_ability`, damage
decision, drain exclusion → Task 4. Shipped content and the hostile path →
Task 5. UI → Task 6. Docs and the save-format claim → Task 7.

Every signature in this plan was read from source, not recalled. The
spec's own audit had one claim wrong — it said `SpeciesDef` followed the
`non_finite_field` pattern when that mechanism exists only on `AbilityDef`
and `ItemDef`, so Task 2 adds it rather than following it. Three more were
caught while writing these tasks and are already corrected inline:
`SpecialTarget::Ally` is a struct variant (`{ slot }`, not a tuple),
`program_manifest` is private to `game::inspection` so tests go through the
public `Game::manifest`, and `perk_defs` returns `Vec<PerkDef>`.

The one thing left unverified is `stat`'s parameter type in Task 6 Step 6 —
`AffinityKind::label` returns `&'static str` and the existing calls pass
string literals, so it should fit, but check if it does not compile.

Task 4 Step 4 introduces `scaled_affinity_power`, which is **not** in the
spec. It exists because `Damage` and `Drain` route their magnitude through
`compute_damage`, which adds ATK, so they cannot reuse `scaled_power` without
scaling level progression twice. Flag this to the user if it looks like the
wrong call — it is the one interface decision made at plan time rather than
design time.
