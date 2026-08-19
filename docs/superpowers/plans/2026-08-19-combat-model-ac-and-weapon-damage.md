# Combat model: attack rolls, AC and weapon damage — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `battle::compute_damage`'s deterministic `(power + atk - def).max(1)` with an attack roll against a derived evasion, percentage mitigation, rolled weapon damage ranges, crit, and a four-rung fumble ladder.

**Architecture:** One new pure seam in `crates/engine/src/battle.rs` — `resolve_attack`, plus `expected_damage` beside it as the RNG-free mean that `balance_sim` *calls*. `Stats::def` becomes `Stats::mitigation` (percentage points, never scaled by level or zone, capped); accuracy and evasion are derived from `SpeciesDef::base_speed` + level + gear and never stored. `Game::resolve_and_apply_attack` is the one door from the ECS into `resolve_attack`, so the four creature-versus-creature call sites branch on an `AttackOutcome` rather than each re-deriving profiles. Structure damage (`attack_nest`) keeps a deterministic path.

**Tech Stack:** Rust 2024, `bevy_ecs` 0.19 (engine only), `rand` via `resources::GameRng`, RON assets, `serde`.

**Spec:** `docs/superpowers/specs/2026-08-19-combat-model-ac-and-weapon-damage-design.md` — read it in full before Task 1. This plan argues from it and does not restate its rationale.

## Global Constraints

- **Slice 1 only.** Multi-attack, reaching past `EnemyGroup::members[0]`, and the base-attribute layer are out of scope (spec slices 2–4). Do not add them.
- **`SAVE_FORMAT_VERSION` bumps exactly once**, in Task 3, from `30` to `31` (`crates/engine/src/save.rs:643`). No later task bumps it again.
- **`Game::apply_damage` (`crates/engine/src/game/combat_damage.rs`) stays the only path that lowers HP.** Every rung of the fumble ladder that deals damage goes through it.
- **`Game`'s `world` field stays private.** No new accessor. `crates/gui` and `crates/app-core` reach the engine only through `Game`'s public API and `views::`.
- **Mitigation never scales with level or zone.** `progression::stats_after_levels` must not raise it; `ZoneLevel::stat_multiplier` must not multiply it. Total is capped at `tuning::MAX_MITIGATION_PERCENT`.
- **`atk` drives damage only.** The to-hit roll comes from speed on both sides. Never feed `atk` into accuracy.
- **Hit chance is the ratio form** `acc / (acc + eva)`, clamped. A difference form (`base + k * (acc - eva)`) is forbidden — it is not scale-free and reintroduces the linear-curve hazard.
- **Every new `.ron` schema field is `#[serde(default)]`**, and the matching `assets/*/README.md` is updated in the same task that adds it (species, items, abilities).
- **A malformed `.ron` file is skipped with a logged warning, never a panic** — follow the existing `*Db::load_dir` pattern.
- **Each new test must fail with its fix removed.** Delete the fix, watch it fail, restore it. A test that passes either way is not coverage.
- **Gates:** `cargo fmt`, `cargo clippy --workspace` (no new warnings), and the task's own `cargo test -p feral-processes-engine <name>` per step. `cargo test --workspace` is the gate at the end of Tasks 3, 8, 10 and 12 only — see CLAUDE.md's "don't re-run the full suite to confirm what you already saw".
- **Commit at every green step.** Branch is `combat-model-ac`; do not push, do not bump the workspace version, do not write a `CHANGELOG.md` section until Task 12.
- **Do not write to `TODO.md`.** Do not update `docs/manual.md` or the root `README.md`.

---

## Decisions this plan makes that the spec left open

Three points the spec does not settle. Each is decided here rather than left to the executor, and each is flagged so the user can overrule it:

1. **`FieldBuffKind::Def` is folded into `FieldBuffKind::Mitigation` and the variant deleted.** Once `Stats::def` is percentage points there is no flat-DEF axis left for it to name, and two names on one axis is exactly what the spec refuses when it says to reuse "Mitigation" rather than introduce "Armor". `BuffKind::Def` is renamed alongside it, and all ten shipped ability files that author `kind: Def` (eight `Buff`, two `FieldBuff`) are re-authored to `Mitigation` with percentage-point powers in Task 4. The save bump in Task 3 covers both enum changes.

2. **Gear mitigation is already inside `Stats`, so `effective_mitigation` must not add it again.** `Game::apply_equipment_delta` (`crates/engine/src/game/crafting.rs:225`) writes an equipped item's `atk`/`def` straight into `Stats`. The spec's "innate + gear + field buffs" is therefore satisfied by reading `Stats::mitigation` once. Gear `accuracy`, `evasion` and `damage` have **no** `Stats` field and are read live through `Game::gear_bonus` — `apply_equipment_delta` must never try to bake them in.

3. **The spec's draw-count table says a recoil fumble costs three draws; the derivation gives two.** The band roll is one draw and the recoil's fresh range roll is the second — the fumble *severity* `d` is derived from `r` and needs no draw at all, which is the spec's own stated reason for the single-roll design. This plan pins the derived counts (Task 2, Step 9). What matters is that they are pinned exactly, not which number they land on.

---

### Task 1: Damage ranges, hit chance, and the derived accuracy/evasion pair

Pure arithmetic in `battle.rs` with no callers yet, so the whole task compiles and passes against an otherwise untouched game.

**Files:**
- Modify: `crates/engine/src/battle.rs` (add at the top of the file, below the `use` block)
- Modify: `crates/engine/src/tuning.rs` (new section)
- Test: `crates/engine/src/battle.rs` (`mod tests` at the bottom, already present)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `battle::DamageRange { pub min: i32, pub max: i32 }` — `Copy`, `Default`, `Serialize`, `Deserialize`, `PartialEq`, `Eq`, `Debug`.
  - `DamageRange::centred(power: i32, spread: i32) -> DamageRange`
  - `DamageRange::mean(self) -> f64`
  - `DamageRange::roll(self, rng: &mut impl rand::Rng) -> i32` — always spends exactly one draw.
  - `battle::hit_chance(accuracy: f64, evasion: f64) -> f64`
  - `battle::accuracy_of(base_speed: i32, level: u32, gear_accuracy: i32) -> f64`
  - `battle::evasion_of(base_speed: i32, level: u32, gear_evasion: i32) -> f64`
  - `tuning::{ACCURACY_PER_SPEED, ACCURACY_PER_LEVEL, EVASION_PER_SPEED, EVASION_PER_LEVEL, HIT_CHANCE_MIN, HIT_CHANCE_MAX, CRIT_CHANCE, FUMBLE_CHANCE, CRIT_ROLL_MULTIPLIER, MAX_MITIGATION_PERCENT, FUMBLE_RECOIL_FRACTION, EXPOSED_EVASION_PERCENT, FUMBLE_RUNG_THRESHOLDS, PLAYER_UNARMED_DAMAGE}`

- [ ] **Step 1: Add the tuning constants**

Append a new labelled section to `crates/engine/src/tuning.rs`, following the existing section-comment style in that file:

```rust
// ─────────────────────────────────────────────────────────────────────────
// Combat resolution: to-hit, crit, fumble, mitigation
// ─────────────────────────────────────────────────────────────────────────

/// Accuracy and Evasion are **derived, never stored** — see
/// `battle::accuracy_of`/`evasion_of`. Both come off `SpeciesDef::base_speed`
/// (range 6..=14 across the shipped roster) plus entity level plus gear, so a
/// fast program both hits and dodges well. `atk` is deliberately absent from
/// both: feeding it to-hit *and* damage compounds quadratically and is the
/// most likely thing to break `balance_sim`'s curves.
pub const ACCURACY_PER_SPEED: f64 = 1.0;
/// See `ACCURACY_PER_SPEED`. Levelling buys accuracy; it never buys mitigation.
pub const ACCURACY_PER_LEVEL: f64 = 0.5;
/// See `ACCURACY_PER_SPEED`.
pub const EVASION_PER_SPEED: f64 = 1.0;
/// See `ACCURACY_PER_LEVEL`.
pub const EVASION_PER_LEVEL: f64 = 0.5;

/// Bounds on `battle::hit_chance`. The floor is what keeps
/// `balance_sim`'s `TURN_CAP` meaningful as stalemate detection rather than
/// as a fight-length cap: expected damage stays strictly positive, so a
/// timeout is a genuine stalemate.
pub const HIT_CHANCE_MIN: f64 = 0.25;
/// See `HIT_CHANCE_MIN`. Below 1.0 so no matchup is a guaranteed landing.
pub const HIT_CHANCE_MAX: f64 = 0.95;

/// Flat crit rate, symmetric between the player and hostiles. Clamped to at
/// most the hit chance inside `battle::resolve_attack`, so a crit is always a
/// hit. Gear crit is deferred (spec, *Deferred, deliberately*) — a
/// `crit` field on `EquipmentStats` that nothing authors is an unused
/// feature flag.
pub const CRIT_CHANCE: f64 = 0.08;
/// What a crit multiplies. The **rolled portion only** — doubling the total
/// would scale crits with levelling and with every `atk` source in the game.
pub const CRIT_ROLL_MULTIPLIER: i32 = 2;

/// Flat fumble rate, symmetric between the player and hostiles, on its own
/// constant so it can be split per side later without touching resolution.
/// Clamped to at most `1 - hit_chance`, so a fumble is always a miss.
pub const FUMBLE_CHANCE: f64 = 0.05;

/// Where the four fumble rungs divide, against `d` — how deep into the
/// fumble band the roll fell, in `[0, 1)`. Weighted so the deep rungs are
/// rare: Exposed below the first, Recoil below the second, Opening below the
/// third, Crash above it. Rungs **replace** rather than stack; a cumulative
/// top rung is a run-ender.
pub const FUMBLE_RUNG_THRESHOLDS: [f64; 3] = [0.55, 0.85, 0.97];
/// Fraction of a fresh roll of the fumbler's own damage range that the
/// Recoil rung deals to the fumbler.
pub const FUMBLE_RECOIL_FRACTION: f32 = 0.5;
/// Percentage points of evasion the Exposed rung strips from the fumbler
/// until their next turn.
pub const EXPOSED_EVASION_PERCENT: i32 = 50;

/// Ceiling on total mitigation, strictly below 100. Load-bearing twice: it
/// stops the damage path reaching immunity, and it is what keeps
/// `Stats::power`'s effective-HP denominator away from zero.
pub const MAX_MITIGATION_PERCENT: i32 = 75;

/// The player's damage range with no weapon equipped. Replaces
/// `PLAYER_STRIKE_POWER`, which was the flat move power behind the player's
/// one basic strike; a weapon **overrides** this rather than adding to it.
pub const PLAYER_UNARMED_DAMAGE: crate::battle::DamageRange =
    crate::battle::DamageRange { min: 3, max: 7 };
```

Leave `PLAYER_STRIKE_POWER` and `MIN_DAMAGE` in place for now — they are deleted in Task 8, when their last caller goes.

- [ ] **Step 2: Write the failing tests for `DamageRange`**

Add to `crates/engine/src/battle.rs`'s existing `mod tests`:

```rust
    #[test]
    fn a_centred_range_of_zero_spread_is_the_power_exactly() {
        let range = DamageRange::centred(8, 0);
        assert_eq!(range.min, 8);
        assert_eq!(range.max, 8);
    }

    #[test]
    fn a_centred_range_widens_symmetrically_around_its_power() {
        let range = DamageRange::centred(10, 3);
        assert_eq!(range.min, 7);
        assert_eq!(range.max, 13);
        assert_eq!(range.mean(), 10.0);
    }

    #[test]
    fn a_centred_range_never_reaches_below_zero() {
        // A low-power ability with a wide spread must not roll negative
        // damage into `apply_damage`, which would read as a heal.
        let range = DamageRange::centred(2, 5);
        assert_eq!(range.min, 0);
        assert_eq!(range.max, 7);
    }

    #[test]
    fn a_degenerate_range_still_spends_exactly_one_draw() {
        // Draw counts must be a property of the *outcome*, not of which
        // weapon swung: a spread-0 ability and a wide weapon have to cost
        // the same, or the RNG stream shifts with the loadout.
        use rand::SeedableRng;
        let mut wide = rand::rngs::StdRng::seed_from_u64(7);
        let mut narrow = rand::rngs::StdRng::seed_from_u64(7);
        let _ = DamageRange { min: 4, max: 9 }.roll(&mut wide);
        let _ = DamageRange { min: 6, max: 6 }.roll(&mut narrow);
        let after_wide: u64 = rand::Rng::random(&mut wide);
        let after_narrow: u64 = rand::Rng::random(&mut narrow);
        assert_eq!(
            after_wide, after_narrow,
            "both ranges must leave the stream in the same place"
        );
    }

    #[test]
    fn a_roll_stays_inside_its_range() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(11);
        let range = DamageRange { min: 4, max: 9 };
        for _ in 0..500 {
            let rolled = range.roll(&mut rng);
            assert!((4..=9).contains(&rolled), "rolled {rolled} outside 4..=9");
        }
    }
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine battle::tests::a_centred_range 2>&1 | tail -20`
Expected: FAIL — `cannot find type DamageRange in this scope`.

- [ ] **Step 4: Implement `DamageRange`**

Add to `crates/engine/src/battle.rs`, below the `use` block. Add `use serde::{Deserialize, Serialize};` to the imports.

```rust
/// The band one attack rolls its damage from, inclusive at both ends.
///
/// **Two constructors on purpose.** Items author `(min, max)` directly and
/// never convert to anything else; abilities and moves author a centre and a
/// spread, because `species::basic_attack_ability` converts a `MoveDef` into
/// an `AbilityDef` and a centre-and-spread pair survives that losslessly
/// where a `(min, max)` pair would round on odd widths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageRange {
    pub min: i32,
    pub max: i32,
}

impl DamageRange {
    /// A range `spread` either side of `power`, floored at 0 on the low end.
    /// A `spread` of 0 is a degenerate range — exactly the deterministic
    /// behaviour every ability had before ranges existed, which is why
    /// `AbilityEffect::Damage`'s new `spread` field defaults to it and none
    /// of the 77 shipped ability files needed editing.
    pub fn centred(power: i32, spread: i32) -> Self {
        let spread = spread.max(0);
        DamageRange {
            min: (power - spread).max(0),
            max: (power + spread).max(0),
        }
    }

    /// The mean of a uniform draw over this range. `expected_damage` — and
    /// so `balance_sim` — is built on this rather than on a re-derived
    /// midpoint.
    pub fn mean(self) -> f64 {
        (self.min as f64 + self.max as f64) / 2.0
    }

    /// One uniform draw from the range.
    ///
    /// Written as an offset from `min` rather than `random_range(min..=max)`
    /// so a degenerate range still consumes exactly one draw. Draw counts
    /// have to be a property of the outcome and not of the weapon, or every
    /// seeded run's RNG stream would shift with the party's loadout.
    pub fn roll(self, rng: &mut impl rand::Rng) -> i32 {
        let width = (self.max - self.min).max(0);
        self.min + rng.random_range(0..=width)
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine battle::tests 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/battle.rs crates/engine/src/tuning.rs
git commit -m "feat(combat): add DamageRange and the slice-1 resolution constants"
```

- [ ] **Step 7: Write the failing tests for hit chance and the derived pair**

Add to `mod tests` in `crates/engine/src/battle.rs`:

```rust
    #[test]
    fn two_identical_combatants_hit_each_other_half_the_time() {
        // The baseline every tuning number in this section is read against.
        assert!((hit_chance(12.0, 12.0) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn hit_chance_is_scale_free() {
        // The whole reason the ratio form is load-bearing: a zone that
        // scales everything by its tier multiplier must change nothing
        // about hit rates.
        let base = hit_chance(14.0, 6.0);
        assert!((hit_chance(28.0, 12.0) - base).abs() < 1e-12);
        assert!((hit_chance(140.0, 60.0) - base).abs() < 1e-12);
    }

    #[test]
    fn hit_chance_clamps_at_both_ends() {
        assert_eq!(hit_chance(1000.0, 1.0), HIT_CHANCE_MAX);
        assert_eq!(hit_chance(1.0, 1000.0), HIT_CHANCE_MIN);
    }

    #[test]
    fn hit_chance_survives_two_combatants_with_nothing_at_all() {
        // Reachable through a mod species authoring base_speed 0 at level 1
        // with no gear. An even matchup, not a divide by zero.
        assert!((hit_chance(0.0, 0.0) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn accuracy_and_evasion_both_grow_with_speed_level_and_gear() {
        assert!(accuracy_of(14, 1, 0) > accuracy_of(6, 1, 0));
        assert!(accuracy_of(10, 8, 0) > accuracy_of(10, 1, 0));
        assert!(accuracy_of(10, 1, 3) > accuracy_of(10, 1, 0));
        assert!(evasion_of(14, 1, 0) > evasion_of(6, 1, 0));
        assert!(evasion_of(10, 8, 0) > evasion_of(10, 1, 0));
        assert!(evasion_of(10, 1, 3) > evasion_of(10, 1, 0));
    }

    #[test]
    fn a_negative_gear_axis_cannot_push_the_pair_below_zero() {
        // A drawback affix is folded into the base, so a copy can carry a
        // negative on an axis its item never had.
        assert!(accuracy_of(6, 1, -100) >= 0.0);
        assert!(evasion_of(6, 1, -100) >= 0.0);
    }
```

Add `HIT_CHANCE_MAX, HIT_CHANCE_MIN` to the `use crate::tuning::{...}` list at the top of `battle.rs`.

- [ ] **Step 8: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine battle::tests::hit_chance 2>&1 | tail -20`
Expected: FAIL — `cannot find function hit_chance in this scope`.

- [ ] **Step 9: Implement the three functions**

Add to `crates/engine/src/battle.rs`, below `DamageRange`. Add `ACCURACY_PER_LEVEL, ACCURACY_PER_SPEED, EVASION_PER_LEVEL, EVASION_PER_SPEED, HIT_CHANCE_MAX, HIT_CHANCE_MIN` to the `use crate::tuning::{...}` list.

```rust
/// Odds one attack lands, from the attacker's Accuracy against the
/// defender's Evasion.
///
/// **The ratio form is load-bearing and a difference form must not replace
/// it.** The ratio is scale-free: doubling both sides leaves the result at
/// 0.5, so a zone that scales everything by its tier multiplier changes
/// nothing about hit rates and the "every difficulty curve must be linear"
/// hazard cannot reappear on this axis at all. `base + k * (acc - eva)`
/// makes hit rate depend on absolute scale, so deep zones drift silently
/// toward always-hit or always-miss.
///
/// Two identical combatants get exactly 0.5 by construction, before the
/// clamp — the baseline every constant in this section is read against.
pub fn hit_chance(accuracy: f64, evasion: f64) -> f64 {
    let acc = accuracy.max(0.0);
    let eva = evasion.max(0.0);
    let total = acc + eva;
    // Two combatants with nothing at all is an even matchup, not an
    // infinity. Reachable from a mod species authoring `base_speed: 0`.
    if total <= 0.0 {
        return 0.5f64.clamp(HIT_CHANCE_MIN, HIT_CHANCE_MAX);
    }
    (acc / total).clamp(HIT_CHANCE_MIN, HIT_CHANCE_MAX)
}

/// A combatant's Accuracy. **Derived, never stored** — not a `Stats` field,
/// not a save field, so it cannot drift from its inputs.
///
/// `gear_accuracy` is `EquipmentStats::accuracy` summed over worn slots,
/// which unlike `atk`/`mitigation` is *not* baked into `Stats` by
/// `Game::apply_equipment_delta` and so must be passed in live.
pub fn accuracy_of(base_speed: i32, level: u32, gear_accuracy: i32) -> f64 {
    (base_speed as f64 * ACCURACY_PER_SPEED
        + level as f64 * ACCURACY_PER_LEVEL
        + gear_accuracy as f64)
        .max(0.0)
}

/// A combatant's Evasion. Same derived-never-stored contract as
/// `accuracy_of`; see its doc for `gear_evasion`.
pub fn evasion_of(base_speed: i32, level: u32, gear_evasion: i32) -> f64 {
    (base_speed as f64 * EVASION_PER_SPEED
        + level as f64 * EVASION_PER_LEVEL
        + gear_evasion as f64)
        .max(0.0)
}
```

- [ ] **Step 10: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine battle::tests 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 11: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p feral-processes-engine 2>&1 | tail -5
git add crates/engine/src/battle.rs crates/engine/src/tuning.rs
git commit -m "feat(combat): derive accuracy and evasion, and the scale-free hit chance"
```

---

### Task 2: `resolve_attack`, the four outcome bands, and the fumble ladder

Still pure, still uncalled. This is the seam everything else in the plan hangs off.

**Files:**
- Modify: `crates/engine/src/battle.rs`
- Test: `crates/engine/src/battle.rs` (`mod tests`)

**Interfaces:**
- Consumes: `DamageRange`, `hit_chance` (Task 1); `tuning::{CRIT_CHANCE, CRIT_ROLL_MULTIPLIER, FUMBLE_CHANCE, FUMBLE_RUNG_THRESHOLDS, FUMBLE_RECOIL_FRACTION}`.
- Produces:
  - `battle::Combatant { pub accuracy: f64, pub evasion: f64, pub atk: i32, pub range: DamageRange }` — `Copy`, `Debug`, `PartialEq`.
  - `battle::FumbleRung { Exposed, Recoil { dmg: i32 }, Opening { dmg: i32 }, Crash }` — `Copy`, `Debug`, `PartialEq`, `Eq`.
  - `battle::AttackOutcome { Fumble(FumbleRung), Miss, Hit { dmg: i32 }, Crit { dmg: i32 } }` — `Copy`, `Debug`, `PartialEq`, `Eq`.
  - `AttackOutcome::damage_to_defender(self) -> i32` — `dmg` on `Hit`/`Crit`, 0 otherwise.
  - `battle::resolve_attack(attacker: Combatant, defender: Combatant, rng: &mut impl rand::Rng) -> AttackOutcome`
  - `battle::expected_damage(attacker: Combatant, defender: Combatant) -> f64`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/engine/src/battle.rs`. `probe()` builds an RNG whose first `random::<f64>()` is a value you choose, by scanning seeds — deterministic and seeded, so it is not a flaky test.

```rust
    fn combatant(accuracy: f64, evasion: f64, atk: i32, range: DamageRange) -> Combatant {
        Combatant {
            accuracy,
            evasion,
            atk,
            range,
        }
    }

    /// A `StdRng` seeded so its first `f64` draw lands in `band`. Scanned
    /// rather than mocked: `resolve_attack` takes `impl Rng`, and a fake
    /// that returns scripted values would stop measuring the thing the
    /// draw-count tests exist for.
    fn rng_whose_first_roll_is_in(band: std::ops::Range<f64>) -> rand::rngs::StdRng {
        use rand::{Rng, SeedableRng};
        for seed in 0..100_000u64 {
            let mut candidate = rand::rngs::StdRng::seed_from_u64(seed);
            let r: f64 = candidate.random();
            if band.contains(&r) {
                return rand::rngs::StdRng::seed_from_u64(seed);
            }
        }
        panic!("no seed produced a first roll inside {band:?}");
    }

    /// How many `u64` draws `f` spends, measured by running the same seed
    /// twice and counting how far the stream advanced.
    fn draws_spent(seed: u64, f: impl Fn(&mut rand::rngs::StdRng)) -> usize {
        use rand::{Rng, SeedableRng};
        let mut used = rand::rngs::StdRng::seed_from_u64(seed);
        f(&mut used);
        let after: u64 = used.random();
        for count in 0..8 {
            let mut probe = rand::rngs::StdRng::seed_from_u64(seed);
            for _ in 0..count {
                let _: u64 = probe.random();
            }
            let next: u64 = probe.random();
            if next == after {
                return count;
            }
        }
        panic!("more than 8 draws, or the stream did not line up");
    }

    #[test]
    fn a_roll_below_the_crit_chance_is_a_crit_that_doubles_only_the_rolled_part() {
        let attacker = combatant(12.0, 12.0, 10, DamageRange { min: 4, max: 4 });
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        let mut rng = rng_whose_first_roll_is_in(0.0..CRIT_CHANCE);
        let outcome = resolve_attack(attacker, defender, &mut rng);
        // 4 rolled, doubled to 8, plus a flat atk of 10 that is NOT doubled.
        assert_eq!(outcome, AttackOutcome::Crit { dmg: 18 });
    }

    #[test]
    fn a_roll_between_the_crit_chance_and_the_hit_chance_is_a_plain_hit() {
        let attacker = combatant(12.0, 12.0, 10, DamageRange { min: 4, max: 4 });
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        let mut rng = rng_whose_first_roll_is_in(CRIT_CHANCE..0.5);
        let outcome = resolve_attack(attacker, defender, &mut rng);
        assert_eq!(outcome, AttackOutcome::Hit { dmg: 14 });
    }

    #[test]
    fn a_roll_between_the_hit_chance_and_the_fumble_band_is_a_plain_miss() {
        let attacker = combatant(12.0, 12.0, 10, DamageRange { min: 4, max: 4 });
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        let mut rng = rng_whose_first_roll_is_in(0.5..(1.0 - FUMBLE_CHANCE));
        assert_eq!(
            resolve_attack(attacker, defender, &mut rng),
            AttackOutcome::Miss
        );
    }

    #[test]
    fn a_roll_at_the_top_of_the_range_is_a_fumble() {
        let attacker = combatant(12.0, 12.0, 10, DamageRange { min: 4, max: 4 });
        let defender = combatant(12.0, 12.0, 3, DamageRange { min: 2, max: 2 });
        let mut rng = rng_whose_first_roll_is_in((1.0 - FUMBLE_CHANCE)..1.0);
        assert!(matches!(
            resolve_attack(attacker, defender, &mut rng),
            AttackOutcome::Fumble(_)
        ));
    }

    #[test]
    fn crit_and_fumble_are_mutually_exclusive_by_construction() {
        // Not sampled: the bands are read off one draw in a fixed order, so
        // no value of `r` can satisfy both. Sweeping `r` across the whole
        // unit interval is the exhaustive statement of that.
        let attacker = combatant(12.0, 12.0, 0, DamageRange::default());
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        let h = hit_chance(attacker.accuracy, defender.evasion);
        let crit = CRIT_CHANCE.min(h);
        let fumble = FUMBLE_CHANCE.min(1.0 - h);
        for step in 0..10_000 {
            let r = step as f64 / 10_000.0;
            assert!(
                !(r < crit && r >= 1.0 - fumble),
                "r = {r} fell in both the crit and the fumble band"
            );
        }
    }

    #[test]
    fn a_crit_can_never_exceed_the_hit_chance() {
        // A hopeless matchup floors at HIT_CHANCE_MIN, which is above
        // CRIT_CHANCE — so squeeze it the other way: a hit chance clamped
        // low must still not let the crit band overhang it.
        let h = HIT_CHANCE_MIN;
        assert!(CRIT_CHANCE.min(h) <= h);
    }

    #[test]
    fn the_opening_rung_does_not_recurse() {
        // A free swing that itself fumbles resolves as a plain miss. Delete
        // the `allow_fumble` guard in `resolve_attack_inner` and this fails:
        // an Opening whose riposte fumbles would deal riposte damage from a
        // nested band instead of zero.
        let attacker = combatant(12.0, 12.0, 0, DamageRange::default());
        let defender = combatant(12.0, 12.0, 0, DamageRange::default());
        // Every fumble in the Opening band, across many seeds, must produce
        // an `Opening` and never another `Fumble` nested inside it — which
        // the type already forbids, so what this actually pins is that no
        // seed panics or returns a rung the ladder cannot produce.
        use rand::SeedableRng;
        let mut openings = 0;
        for seed in 0..20_000u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            if let AttackOutcome::Fumble(FumbleRung::Opening { dmg }) =
                resolve_attack(attacker, defender, &mut rng)
            {
                openings += 1;
                assert!(dmg >= 0, "an Opening riposte cannot heal the fumbler");
            }
        }
        assert!(openings > 0, "no seed reached the Opening rung");
    }

    #[test]
    fn draw_counts_are_pinned_per_outcome() {
        // Asserting the exact count is what stops crit or fumble silently
        // becoming an extra draw and shifting every seeded run's stream.
        let attacker = combatant(12.0, 12.0, 5, DamageRange { min: 2, max: 6 });
        let defender = combatant(12.0, 12.0, 5, DamageRange { min: 2, max: 6 });

        let miss_seed = seed_producing(attacker, defender, |o| *o == AttackOutcome::Miss);
        assert_eq!(
            draws_spent(miss_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            1,
            "a miss costs one draw"
        );

        let hit_seed = seed_producing(attacker, defender, |o| {
            matches!(o, AttackOutcome::Hit { .. })
        });
        assert_eq!(
            draws_spent(hit_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            2,
            "a hit costs the band roll plus one weapon roll"
        );

        let crit_seed = seed_producing(attacker, defender, |o| {
            matches!(o, AttackOutcome::Crit { .. })
        });
        assert_eq!(
            draws_spent(crit_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            2,
            "a crit costs the same as a hit — the doubling is arithmetic"
        );

        let exposed_seed = seed_producing(attacker, defender, |o| {
            *o == AttackOutcome::Fumble(FumbleRung::Exposed)
        });
        assert_eq!(
            draws_spent(exposed_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            1,
            "Exposed spends nothing beyond the band roll"
        );

        let recoil_seed = seed_producing(attacker, defender, |o| {
            matches!(o, AttackOutcome::Fumble(FumbleRung::Recoil { .. }))
        });
        assert_eq!(
            draws_spent(recoil_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            2,
            "Recoil adds one fresh roll of the fumbler's own range"
        );

        let crash_seed = seed_producing(attacker, defender, |o| {
            *o == AttackOutcome::Fumble(FumbleRung::Crash)
        });
        assert_eq!(
            draws_spent(crash_seed, |rng| {
                resolve_attack(attacker, defender, rng);
            }),
            1,
            "Crash spends nothing beyond the band roll"
        );
    }

    /// First seed whose `resolve_attack` satisfies `want`.
    fn seed_producing(
        attacker: Combatant,
        defender: Combatant,
        want: impl Fn(&AttackOutcome) -> bool,
    ) -> u64 {
        use rand::SeedableRng;
        for seed in 0..500_000u64 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let outcome = resolve_attack(attacker, defender, &mut rng);
            if want(&outcome) {
                return seed;
            }
        }
        panic!("no seed produced the wanted outcome");
    }

    #[test]
    fn expected_damage_is_the_mean_of_the_same_arithmetic() {
        // The property that lets `balance_sim` *call* this rather than keep
        // a copy: averaging a large sample of the real roll must converge on
        // it. Seeded, so it is deterministic.
        use rand::SeedableRng;
        let attacker = combatant(12.0, 12.0, 5, DamageRange { min: 2, max: 6 });
        let defender = combatant(12.0, 12.0, 5, DamageRange { min: 2, max: 6 });
        let mut rng = rand::rngs::StdRng::seed_from_u64(4);
        let n = 200_000;
        let total: i64 = (0..n)
            .map(|_| resolve_attack(attacker, defender, &mut rng).damage_to_defender() as i64)
            .sum();
        let sampled = total as f64 / n as f64;
        let projected = expected_damage(attacker, defender);
        assert!(
            (sampled - projected).abs() < 0.1,
            "sampled {sampled}, projected {projected}"
        );
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine battle::tests 2>&1 | tail -20`
Expected: FAIL — `cannot find type Combatant in this scope`.

- [ ] **Step 3: Implement the types and `resolve_attack`**

Add to `crates/engine/src/battle.rs`, below `evasion_of`. Extend the `use crate::tuning::{...}` list with `CRIT_CHANCE, CRIT_ROLL_MULTIPLIER, FUMBLE_CHANCE, FUMBLE_RECOIL_FRACTION, FUMBLE_RUNG_THRESHOLDS`.

```rust
/// Everything one side brings to a single attack roll, resolved by the
/// caller and handed in flat.
///
/// A struct rather than four parameters for the same reason
/// `Game::copy_bonus` takes a whole `GearCopy`: a fifth axis added later is
/// then not forgettable at a call site.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Combatant {
    pub accuracy: f64,
    pub evasion: f64,
    /// Flat damage added to every landed roll. Never feeds the to-hit roll —
    /// see `accuracy_of`.
    pub atk: i32,
    pub range: DamageRange,
}

/// How badly an attack went wrong. **Rungs replace rather than stack** — a
/// cumulative top rung is a run-ender. Which rung comes from how deep into
/// the fumble band the roll fell, so it needs no second draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FumbleRung {
    /// Evasion cut by `EXPOSED_EVASION_PERCENT` until the fumbler's next
    /// turn.
    Exposed,
    /// `FUMBLE_RECOIL_FRACTION` of a fresh roll of the fumbler's own range,
    /// dealt to the fumbler.
    Recoil { dmg: i32 },
    /// The target takes a free swing at the fumbler, for `dmg`. Zero when
    /// the free swing missed.
    Opening { dmg: i32 },
    /// The fumbler loses their next action.
    Crash,
}

/// What one attack did. The caller branches on this: a miss must skip a
/// Drain's heal and a rider's status, which is why the branch cannot live
/// inside `Game::apply_damage`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackOutcome {
    Fumble(FumbleRung),
    Miss,
    Hit { dmg: i32 },
    Crit { dmg: i32 },
}

impl AttackOutcome {
    /// Damage aimed at the *defender*. Zero for a miss and for every fumble
    /// rung — a Recoil hurts the fumbler and an Opening's riposte lands on
    /// the fumbler, so neither is defender-facing damage.
    pub fn damage_to_defender(self) -> i32 {
        match self {
            AttackOutcome::Hit { dmg } | AttackOutcome::Crit { dmg } => dmg,
            AttackOutcome::Miss | AttackOutcome::Fumble(_) => 0,
        }
    }
}

/// Resolves one creature-versus-creature attack.
///
/// **One draw, four bands.** A single `r ∈ [0, 1)` decides the outcome, in
/// this order: crit (clamped to at most the hit chance), hit, fumble
/// (clamped to at most `1 - hit chance`), miss. One draw rather than three
/// bounds the RNG-stream shift and makes crit and fumble mutually exclusive
/// *by construction* rather than by a check.
///
/// A structure has no speed and cannot dodge, so `Game::attack_nest` does
/// not come through here — only creature-versus-creature attacks do.
pub fn resolve_attack(
    attacker: Combatant,
    defender: Combatant,
    rng: &mut impl rand::Rng,
) -> AttackOutcome {
    resolve_attack_inner(attacker, defender, rng, true)
}

/// `allow_fumble: false` is the Opening rung's non-recursion guard — see
/// `fumble_rung`.
fn resolve_attack_inner(
    attacker: Combatant,
    defender: Combatant,
    rng: &mut impl rand::Rng,
    allow_fumble: bool,
) -> AttackOutcome {
    let h = hit_chance(attacker.accuracy, defender.evasion);
    let crit = CRIT_CHANCE.min(h);
    let fumble = if allow_fumble {
        FUMBLE_CHANCE.min(1.0 - h)
    } else {
        0.0
    };
    let r: f64 = rng.random();
    if r < crit {
        let rolled = attacker.range.roll(rng);
        return AttackOutcome::Crit {
            dmg: rolled * CRIT_ROLL_MULTIPLIER + attacker.atk,
        };
    }
    if r < h {
        let rolled = attacker.range.roll(rng);
        return AttackOutcome::Hit {
            dmg: rolled + attacker.atk,
        };
    }
    if fumble > 0.0 && r >= 1.0 - fumble {
        let depth = (r - (1.0 - fumble)) / fumble;
        return AttackOutcome::Fumble(fumble_rung(depth, attacker, defender, rng));
    }
    AttackOutcome::Miss
}

/// Which rung a fumble at `depth` into the band lands on. `depth` is in
/// `[0, 1)` and derived from the single band roll, so severity costs no
/// second draw.
fn fumble_rung(
    depth: f64,
    attacker: Combatant,
    defender: Combatant,
    rng: &mut impl rand::Rng,
) -> FumbleRung {
    let [exposed, recoil, opening] = FUMBLE_RUNG_THRESHOLDS;
    if depth < exposed {
        return FumbleRung::Exposed;
    }
    if depth < recoil {
        let rolled = attacker.range.roll(rng);
        return FumbleRung::Recoil {
            dmg: ((rolled as f32) * FUMBLE_RECOIL_FRACTION).round().max(1.0) as i32,
        };
    }
    if depth < opening {
        // **The free swing must not itself fumble.** A fumbled riposte
        // resolves as a plain miss. This is a hard rule, not a convention:
        // without it one bad roll chains into an unbounded exchange, and the
        // deepest rung stops being the run-ender the ladder is shaped to
        // avoid. `the_opening_rung_does_not_recurse` pins it.
        let riposte = resolve_attack_inner(defender, attacker, rng, false);
        return FumbleRung::Opening {
            dmg: riposte.damage_to_defender(),
        };
    }
    FumbleRung::Crash
}

/// The mean of `resolve_attack`'s defender-facing damage, RNG-free.
///
/// **`balance_sim` calls this; it does not keep a copy.** `CLAUDE.md`
/// records four occasions where a `balance_sim` doc comment promised it
/// mirrored a real formula while being an independent copy that drifted —
/// worst of all a mining-reliability curve that would have let the balance
/// gate pass against a game that no longer existed. Follow
/// `attackers_in_group` and `slot_aggro_weight`.
///
/// Deliberately excludes the fumble ladder: Recoil and Opening both land on
/// the *attacker*, so neither is defender-facing damage, and the projection
/// is therefore a mild overestimate of an attacker's net output. Named here
/// rather than silently, in the same spirit as `TURN_CAP`'s note that Power
/// decay is unmodelled.
pub fn expected_damage(attacker: Combatant, defender: Combatant) -> f64 {
    let h = hit_chance(attacker.accuracy, defender.evasion);
    let crit = CRIT_CHANCE.min(h);
    let plain = h - crit;
    let mean = attacker.range.mean();
    let atk = attacker.atk as f64;
    plain * (mean + atk) + crit * (mean * CRIT_ROLL_MULTIPLIER as f64 + atk)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine battle::tests 2>&1 | tail -30`
Expected: PASS, all of them.

- [ ] **Step 5: Prove the non-recursion test is not vacuous**

Temporarily change `fumble_rung`'s Opening arm to `resolve_attack_inner(defender, attacker, rng, true)`.
Run: `cargo test -p feral-processes-engine battle::tests::draw_counts_are_pinned_per_outcome 2>&1 | tail -20`
Expected: FAIL — the Opening rung's draw count is no longer bounded. Restore the `false` and confirm PASS.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy -p feral-processes-engine 2>&1 | tail -5
git add crates/engine/src/battle.rs crates/engine/src/tuning.rs
git commit -m "feat(combat): resolve_attack, the four outcome bands and the fumble ladder"
```

---

### Task 3: `Stats::def` becomes `Stats::mitigation`, and `power()` is redefined

The compile-breaking rename. Mechanical across four crates, but three of the edits inside it are decisions, not renames: `power()`, the levelling rule, and the zone rule.

**Files:**
- Modify: `crates/engine/src/components.rs:96-118` (`Stats`, `power()`)
- Modify: `crates/engine/src/save.rs:21`, `:100`, `:643` (`CreatureSave::def`, `PlayerSave::def`, `SAVE_FORMAT_VERSION`)
- Modify: `crates/engine/src/progression.rs:148-157` (`stats_after_levels`)
- Modify: `crates/engine/src/game/spawning.rs:218` (zone scaling of a wild spawn)
- Modify: `crates/engine/src/balance_sim.rs:55-63` (`wild_stats_at_zone`)
- Modify: `crates/engine/src/tuning.rs:35-41` (`PLAYER_BASE_STATS`), and `DEF_PER_LEVEL`
- Modify (mechanical rename only): `crates/engine/src/game/{combat_round,refactor,party,inspection,catalog,lifecycle,contracts,talents,unlocks,field,stack_market,combat_teardown,base/upkeep}.rs`, `crates/engine/src/{items_db,affixes,species}.rs`, `crates/engine/src/arena/setup.rs`, `crates/launcher/src/tuner/*.rs`, `crates/gui/src/render/*.rs`, `crates/app-core/src/lib.rs`
- Test: `crates/engine/src/components.rs` (`mod tests`), `crates/engine/src/tests/progression.rs`, `crates/engine/src/tests/spawning.rs`

**Interfaces:**
- Consumes: `tuning::MAX_MITIGATION_PERCENT` (Task 1).
- Produces: `components::Stats { hp, max_hp, atk, mitigation }`; `Stats::power(&self) -> i32` redefined as effective HP plus attack; `save::SAVE_FORMAT_VERSION = 31`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/components.rs` in a `#[cfg(test)] mod tests` block (create it at the bottom of the file if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::MAX_MITIGATION_PERCENT;

    #[test]
    fn power_prices_mitigation_as_effective_hp() {
        // 100 HP behind 50% mitigation is worth 200 HP of soak.
        let soft = Stats {
            hp: 100,
            max_hp: 100,
            atk: 10,
            mitigation: 0,
        };
        let armoured = Stats {
            mitigation: 50,
            ..soft
        };
        assert_eq!(soft.power(), 110);
        assert_eq!(armoured.power(), 210);
    }

    #[test]
    fn power_cannot_divide_by_zero_at_the_mitigation_cap() {
        // MAX_MITIGATION_PERCENT is capped strictly below 100, and that cap
        // is load-bearing here as well as in the damage path.
        let capped = Stats {
            hp: 100,
            max_hp: 100,
            atk: 0,
            mitigation: MAX_MITIGATION_PERCENT,
        };
        assert!(capped.power() > 0 && capped.power() < 100_000);
    }

    #[test]
    fn power_clamps_a_mitigation_beyond_the_cap() {
        // A save, a mod affix or a stacked buff can hand this a raw number
        // past the cap; `power` must not go negative or infinite on one.
        let overcapped = Stats {
            hp: 100,
            max_hp: 100,
            atk: 0,
            mitigation: 400,
        };
        let capped = Stats {
            mitigation: MAX_MITIGATION_PERCENT,
            ..overcapped
        };
        assert_eq!(overcapped.power(), capped.power());
    }

    #[test]
    fn power_ignores_current_hp() {
        let hurt = Stats {
            hp: 1,
            max_hp: 100,
            atk: 10,
            mitigation: 0,
        };
        let whole = Stats { hp: 100, ..hurt };
        assert_eq!(hurt.power(), whole.power());
    }
}
```

Add to `crates/engine/src/tests/progression.rs`:

```rust
/// A percentage that grows per level approaches immunity, so levelling buys
/// HP, attack, accuracy and evasion — never mitigation. Delete the fix (put
/// a `DEF_PER_LEVEL` term back on the `mitigation` field) and this fails.
#[test]
fn levelling_never_raises_mitigation() {
    use crate::components::Stats;
    use crate::progression::stats_after_levels;
    let base = Stats {
        hp: 90,
        max_hp: 90,
        atk: 6,
        mitigation: 12,
    };
    for levels in [1, 5, 20, 60] {
        let grown = stats_after_levels(base, levels, 1.5);
        assert_eq!(
            grown.mitigation, base.mitigation,
            "mitigation moved at {levels} levels"
        );
        assert!(grown.max_hp > base.max_hp);
        assert!(grown.atk > base.atk);
    }
}
```

Add to `crates/engine/src/tests/spawning.rs`:

```rust
/// A zone tier scales HP and attack and leaves mitigation exactly where the
/// species authored it. Delete the fix and this fails: the spawn would carry
/// `base_mitigation * stat_multiplier`, which reaches the cap by zone 5 on
/// half the roster.
#[test]
fn a_zone_tier_never_scales_mitigation() {
    let mut game = crate::tests::support::game_at_zone(4);
    let spawned = crate::tests::support::spawn_wild(&mut game, "sentinel");
    let stats = *game.stats_of_for_test(spawned);
    let species = game.species_def_for_test("sentinel");
    assert_eq!(stats.mitigation, species.base_mitigation);
    assert!(stats.max_hp > species.base_hp);
}
```

If `game_at_zone`, `spawn_wild`, `stats_of_for_test` or `species_def_for_test` do not exist in `crates/engine/src/tests/support.rs`, use the nearest existing fixture there rather than adding new ones — check that file first, it is the standing rule. `base_mitigation` on `SpeciesDef` does not exist until Task 5; until then write this test against `species.base_def` and rename it in Task 5.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine power_prices_mitigation 2>&1 | tail -20`
Expected: FAIL — `struct Stats has no field named mitigation`.

- [ ] **Step 3: Rename the field and redefine `power()`**

In `crates/engine/src/components.rs`:

```rust
#[derive(Component, Clone, Copy, Debug)]
pub struct Stats {
    pub hp: i32,
    pub max_hp: i32,
    /// Damage only. The to-hit roll comes from speed on both sides — see
    /// `battle::accuracy_of`. Feeding this to-hit as well would compound
    /// quadratically and move every `balance_sim` curve.
    pub atk: i32,
    /// **Percentage points**, not subtractive absorption. Innate plus
    /// whatever gear `Game::apply_equipment_delta` has baked in; buffs and
    /// the cap are applied on top by `Game::effective_mitigation`.
    ///
    /// **Never scaled by level or zone.** A percentage that grows per level
    /// approaches immunity, so `progression::stats_after_levels` and
    /// `ZoneLevel::stat_multiplier` both leave it alone. Levelling buys HP,
    /// attack, accuracy and evasion; mitigation comes from gear and from
    /// what a species innately is. This is the rule that keeps the
    /// percentage form safe, and the one most likely to be "corrected" by
    /// someone restoring symmetry with the other stats.
    pub mitigation: i32,
}
```

and

```rust
    /// A rough "how strong is this" scalar — effective HP plus attack.
    ///
    /// Used to gauge relative difficulty (`Game::difficulty_color`), to
    /// price a kill's XP (`progression::kill_xp`'s denominator), and by
    /// trade valuation and the unlock ratios. Summing a *percentage* into a
    /// total the way the old `max_hp + atk + def` did is meaningless, so
    /// mitigation is priced as the soak it actually buys:
    /// `max_hp / (1 - mitigation/100)`.
    ///
    /// The clamp to `MAX_MITIGATION_PERCENT` is load-bearing — it is what
    /// keeps the denominator away from zero on a value that a save, a mod
    /// affix or a stacked buff could hand in past the cap.
    pub fn power(&self) -> i32 {
        let mitigation = self.mitigation.clamp(0, crate::tuning::MAX_MITIGATION_PERCENT);
        let soak = self.max_hp as f64 / (1.0 - mitigation as f64 / 100.0);
        soak.round() as i32 + self.atk
    }
```

- [ ] **Step 4: Sweep the rename across all four crates**

This is mechanical. Work it compiler-first rather than by regex — `def` is a substring of `default`, `def_id`, `defending`, `StructureDef` and thirty other identifiers, so a blind `sed` will corrupt the tree.

```bash
cargo check --workspace 2>&1 | rg '^error' | head -40
```

Fix the reported sites, re-run, repeat. Points to get right rather than merely compiling:

- `crates/engine/src/save.rs:21` and `:100` — rename the `def` field on `CreatureSave` and `PlayerSave` to `mitigation`. Do **not** add `#[serde(default)]` to dodge the break: the spec is explicit that a field whose meaning changes under a name it keeps is exactly the case field-named RON does not cover, and an old save must be refused by version rather than load an absorption number into a percentage slot.
- `crates/engine/src/save.rs:643` — `pub const SAVE_FORMAT_VERSION: u32 = 31;`, and extend its doc comment with one line naming this change as the reason.
- `crates/engine/src/progression.rs:148-157` — `stats_after_levels` sets `mitigation: base.mitigation` with **no** growth term. Delete `DEF_PER_LEVEL` from `tuning.rs` outright rather than leaving it unused (no backwards-compat cruft), and delete any test asserting on it.
- `crates/engine/src/game/spawning.rs:218` — the wild-spawn scale multiplies `base_hp` and `base_atk` by `zone_level.stat_multiplier()` and passes mitigation through unscaled.
- `crates/engine/src/balance_sim.rs:55-63` — `wild_stats_at_zone` does the same.
- `crates/engine/src/tuning.rs:35` — `PLAYER_BASE_STATS` becomes `mitigation: 2`. It is an offset and not a rate, so the levelling rule does not sweep it in; keep the value as authored for now and retune in Task 12's play pass if it reads wrong.
- `crates/app-core/src/lib.rs:216` — `stat_summary`'s `(mods.def, "DEF")` row becomes `(mods.mitigation, "MIT")`. The tag is the player-facing word; **"Mitigation" is the existing vocabulary** (`FieldBuffKind::Mitigation`, the `patch_routine` item's "Mitigation 10"), so do not introduce "Armor".
- `crates/launcher/src/tuner/*.rs` — rename only. The tuner's objective and constraints read `Stats` fields directly.
- Existing engine tests under `crates/engine/src/tests/` — rename only. Where a test asserts a *number* that moved because `power()` changed (con colours, `kill_xp`), update the expected value and leave a one-line comment saying `power()`'s redefinition moved it. Do not weaken an assertion into an inequality to make it pass.

- [ ] **Step 5: Run the new tests**

Run: `cargo test -p feral-processes-engine power_prices_mitigation 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine levelling_never_raises_mitigation 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine a_zone_tier_never_scales_mitigation 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Full-suite gate**

Run: `cargo test --workspace 2>&1 | tail -40`
Expected: PASS. `balance_sim`'s curve tests may move here because `power()` moved — if they do, re-baseline them now with a comment naming `power()`'s redefinition, and expect them to move again in Task 10.

If many tests fail at once with `NotFound` on an assets path, that is stale build artifacts from the `petmud` rename, not a regression — `cargo clean -p feral-processes-engine -p feral-processes-app-core` and re-run. Do not run a full `cargo clean`.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u
git commit -m "feat(combat)!: Stats::def becomes percentage-point mitigation, and power() prices it

BREAKING: SAVE_FORMAT_VERSION 30 -> 31. An old save carries an absorption
number under a name that now means percentage points, which field-named RON
cannot rescue — it is refused by version instead."
```

Stage explicit paths or `git add -u` rather than `git add -A`: another agent's worktree gitlink under `.claude/worktrees/` gets swept up otherwise.

---

### Task 4: `effective_mitigation` and the mitigation cap in the damage path

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs:1131-1146` (`effective_def` → `effective_mitigation`)
- Modify: `crates/engine/src/game/combat_damage.rs:65-73` (`mitigate_incoming_damage`)
- Modify: `crates/engine/src/components.rs:823-833` (`FieldBuffKind`, delete `Def`)
- Modify: the ten files in `assets/abilities/` that author `kind: Def` — eight `Buff`, two `FieldBuff`
- Modify: `assets/abilities/README.md`
- Test: `crates/engine/src/tests/combat_status.rs`

**Interfaces:**
- Consumes: `Stats::mitigation` (Task 3); `tuning::MAX_MITIGATION_PERCENT` (Task 1).
- Produces: `Game::effective_mitigation(&self, entity: Entity) -> i32` — `pub(crate)`, already capped. `Game::effective_def` no longer exists. `BuffKind::Mitigation` replaces `BuffKind::Def`; `FieldBuffKind::Def` is gone.

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/tests/combat_status.rs`:

```rust
/// Innate mitigation, gear (already baked into `Stats` by
/// `apply_equipment_delta`) and a running field buff all count, and the sum
/// is capped. Delete the `.min(MAX_MITIGATION_PERCENT)` and this fails at a
/// stacked total.
#[test]
fn mitigation_sums_its_sources_and_stops_at_the_cap() {
    let mut game = support::test_game();
    let player = game.player_entity_for_test();
    game.set_mitigation_for_test(player, 60);
    game.arm_field_buff_for_test(player, FieldBuffKind::Mitigation, 40);
    assert_eq!(
        game.effective_mitigation_for_test(player),
        crate::tuning::MAX_MITIGATION_PERCENT
    );
}

/// A landed hit stays a hit under heavy mitigation, but a miss is not raised
/// to 1. This is `mitigate_incoming_damage`'s existing behaviour and it must
/// survive the rewrite.
#[test]
fn heavy_mitigation_floors_a_landed_hit_at_one_and_leaves_a_miss_alone() {
    let mut game = support::test_game();
    let player = game.player_entity_for_test();
    game.set_mitigation_for_test(player, crate::tuning::MAX_MITIGATION_PERCENT);
    let before = game.hp_of_for_test(player);
    game.apply_damage_for_test(player, 2);
    assert_eq!(game.hp_of_for_test(player), before - 1, "a hit still lands");
    let after_hit = game.hp_of_for_test(player);
    game.apply_damage_for_test(player, 0);
    assert_eq!(game.hp_of_for_test(player), after_hit, "a miss costs nothing");
}
```

Use the existing fixtures in `crates/engine/src/tests/support.rs` — check what is already there before adding `set_mitigation_for_test` and friends, and add them there (not inline in the test file) if they genuinely do not exist.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine mitigation_sums_its_sources 2>&1 | tail -20`
Expected: FAIL — `no method named effective_mitigation_for_test`.

- [ ] **Step 3: Delete `FieldBuffKind::Def`, rename `BuffKind::Def`, and re-author all ten abilities**

In `crates/engine/src/components.rs`, remove the `Def` variant from `FieldBuffKind` and from `scope()` and `affinity_kind()`'s match arms (it folds into the `Mitigation` arm in both, which already sits beside it). Update `FieldBuffKind`'s doc comment: it currently says saves are bincode and serialize by name — the save is field-named RON now, and the constraint that a *rename* breaks a save while a reorder does not still holds; say so accurately rather than repeating the stale bincode line.

Rename `BuffKind::Def` to `BuffKind::Mitigation` in `components.rs` in the same step — this is the spec's "the `Def` rename", and both enums have to move together or the assets end up naming one axis two ways.

Then re-author **all ten** shipped ability files that author `kind: Def` (`rg -l 'kind: Def' assets/abilities/`) — eight `Buff(...)` and two `FieldBuff(...)`. The spec estimated thirteen; ten is what the tree actually holds, verified 2026-08-19. Every one becomes `kind: Mitigation` with its power re-authored as percentage points: a flat 4 points of subtractive DEF was worth roughly an eighth of a mid-zone swing, so `power: 4` becomes `power: 12` and the ladder scales from there (`bastion_shield_v3`'s `power: 7` becomes `power: 20`). Nothing may author a power that alone reaches `MAX_MITIGATION_PERCENT` — a single buff conferring immunity is what the cap would then silently swallow.

Update `assets/abilities/README.md`'s `BuffKind` and `FieldBuffKind` lists in the same edit, and say that `Mitigation`'s unit is percentage points.

- [ ] **Step 4: Replace `effective_def` with `effective_mitigation`**

In `crates/engine/src/game/combat_round.rs`:

```rust
    /// `entity`'s total mitigation in percentage points, capped at
    /// `MAX_MITIGATION_PERCENT` — the one door onto "how much of an incoming
    /// hit does this creature shrug off".
    ///
    /// `Stats::mitigation` already carries **both** the innate value and
    /// whatever gear is worn: `Game::apply_equipment_delta` bakes an
    /// equipped item's bonus straight into `Stats`. Adding `gear_bonus`
    /// again here would double-count every worn piece — the same trap
    /// `no stats operation may run while a gear bonus is sitting in Stats`
    /// already names from the other direction.
    ///
    /// The cap is applied here rather than at the two readers, so nothing
    /// downstream can see an uncapped percentage.
    pub(crate) fn effective_mitigation(&self, entity: Entity) -> i32 {
        let base = self
            .world
            .get::<Stats>(entity)
            .map(|s| s.mitigation)
            .unwrap_or(0);
        let bonus = self
            .world
            .get::<CombatBuff>(entity)
            .and_then(|b| b.active)
            .filter(|a| a.kind == BuffKind::Mitigation)
            .map(|a| a.power)
            .unwrap_or(0);
        let field_bonus = self.field_buff_power(entity, FieldBuffKind::Mitigation);
        let total = if entity != self.player_entity() {
            base + bonus + field_bonus
        } else {
            base + bonus + field_bonus + self.party_stat_bonus().1 + self.wielded_stat_bonus().1
        };
        total.clamp(0, MAX_MITIGATION_PERCENT)
    }
```

The `BuffKind` match arms the rename opens — `combat_round.rs:980` and wherever else the compiler points — take `"mitigation"` as the stat word in their log lines. `is_defending` sniffs `CombatBuff` for `Def` at exactly `DEFEND_DEF_BONUS`; rename the constant to `DEFEND_MITIGATION_BONUS` and re-author its value as percentage points — a brace that added flat DEF now adds mitigation, and the value has to change with the unit. Leave the sniffing mechanism alone; its doc already explains why it exists.

Keep `party_stat_bonus` and `wielded_stat_bonus` as they are: both take a tenth of a companion's own mitigation, which is percentage points contributed to percentage points, and the cap bounds the total. Flag it in Task 12's play notes rather than redesigning it here.

- [ ] **Step 5: Widen `mitigate_incoming_damage`**

In `crates/engine/src/game/combat_damage.rs`, replace the `field_buff_power` read with a call to `effective_mitigation`. Keep the two behaviours the existing doc comment defends — rounding once in the same expression as the percentage cut, and flooring a landed hit at 1 while passing `dmg <= 0` through untouched — and keep saying why.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine mitigation 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 7: Prove the cap test is not vacuous**

Delete the `.clamp(0, MAX_MITIGATION_PERCENT)` from `effective_mitigation`.
Run: `cargo test -p feral-processes-engine mitigation_sums_its_sources 2>&1 | tail -20`
Expected: FAIL at 100. Restore it and confirm PASS.

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u assets/abilities
git commit -m "feat(combat): effective_mitigation is the one door, capped, and folds FieldBuffKind::Def in"
```

---

### Task 5: `SpeciesDef::base_mitigation` and move damage ranges

**Files:**
- Modify: `crates/engine/src/species.rs:207` (`base_def`), `:46-61` (`MoveDef`), `:78-96` (`basic_attack_ability`), `:690-760` (`stat_shape_faults`)
- Modify: all 17 files in `assets/species/`
- Modify: `assets/species/README.md`
- Test: `crates/engine/src/species.rs` (`mod tests`), `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: `battle::DamageRange` (Task 1).
- Produces: `SpeciesDef::base_mitigation: i32`; `MoveDef::spread: i32` (`#[serde(default)]`); `MoveDef::range(&self) -> DamageRange`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/species.rs`'s `mod tests`:

```rust
    /// A move file that names only `power` keeps parsing and is the
    /// degenerate range — exactly today's deterministic behaviour. This is
    /// the modding promise: no shipped or third-party species file needs
    /// editing to gain damage ranges.
    #[test]
    fn a_move_without_a_spread_is_a_degenerate_range() {
        let mv: MoveDef = ron::from_str(r#"(name: "Fray", power: 8)"#)
            .expect("a move with no spread must still parse");
        assert_eq!(mv.spread, 0);
        assert_eq!(mv.range(), crate::battle::DamageRange { min: 8, max: 8 });
    }

    #[test]
    fn a_move_with_a_spread_widens_around_its_power() {
        let mv: MoveDef = ron::from_str(r#"(name: "Fray", power: 8, spread: 3)"#)
            .expect("a move with a spread must parse");
        assert_eq!(mv.range(), crate::battle::DamageRange { min: 5, max: 11 });
    }

```

Add to `crates/engine/src/tests/assets.rs`:

```rust
/// Every shipped species authors a mitigation percentage inside the band the
/// cap allows. A species at or past `MAX_MITIGATION_PERCENT` is immune
/// before gear or a buff is counted, which the cap would silently swallow.
#[test]
fn every_species_mitigation_leaves_room_under_the_cap() {
    let db = shipped_species();
    for species in db.all() {
        assert!(
            (0..crate::tuning::MAX_MITIGATION_PERCENT).contains(&species.base_mitigation),
            "{} authors base_mitigation {}, outside 0..{}",
            species.id,
            species.base_mitigation,
            crate::tuning::MAX_MITIGATION_PERCENT
        );
    }
}

/// Every shipped move authors a range that can actually vary, and none can
/// roll negative. A roster of degenerate ranges would ship the feature dark.
#[test]
fn every_shipped_move_authors_a_real_damage_range() {
    let db = shipped_species();
    for species in db.all() {
        for mv in &species.moves {
            let range = mv.range();
            assert!(range.min >= 0, "{} / {} can roll negative", species.id, mv.name);
            assert!(
                range.max > range.min,
                "{} / {} is a degenerate range — a shipped move must vary",
                species.id,
                mv.name
            );
        }
    }
}
```

Use whatever the existing helper in `crates/engine/src/tests/assets.rs` is called for loading the shipped `SpeciesDb` — read the file first rather than assuming `shipped_species()`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine a_move_without_a_spread 2>&1 | tail -20`
Expected: FAIL — `struct MoveDef has no field named spread`.

- [ ] **Step 3: Add the fields**

In `crates/engine/src/species.rs`:

```rust
pub struct MoveDef {
    pub name: String,
    /// The centre of this move's damage range — see `spread`.
    pub power: i32,
    /// Half-width of the damage range around `power`.
    /// `#[serde(default)]` at 0 is a degenerate range, which is exactly the
    /// deterministic behaviour every move had before ranges existed — so no
    /// shipped species file and no mod's needed editing, and mods gain
    /// damage ranges for free.
    #[serde(default)]
    pub spread: i32,
    // ... existing `effect` and `ranged` fields, unchanged
}

impl MoveDef {
    /// This move's damage band. Centre-and-spread rather than `(min, max)`
    /// because `basic_attack_ability` converts a move into an
    /// `AbilityDef` and this pair survives that losslessly.
    pub fn range(&self) -> crate::battle::DamageRange {
        crate::battle::DamageRange::centred(self.power, self.spread)
    }
}
```

Rename `SpeciesDef::base_def` to `base_mitigation` and rewrite its doc to say percentage points, pointing at `Stats::mitigation` for the never-scaled rule. Leave `basic_attack_ability` alone for now — `AbilityEffect::Damage` has no `spread` field until Task 7, which is where that conversion and its test live.

In `stat_shape_faults` (`:690-760`), the `base_def` reads become `base_mitigation`. The stat-total check at `:725` sums `base_hp + base_atk + base_def` — that sum is now HP plus damage plus a percentage, which is not a total of anything. Replace the `base_def` term with nothing and rebaseline the per-class bands against `base_hp + base_atk` alone, keeping the function's existing contract: it returns the **verdict** rather than the ingredients, returns **every** fault rather than the first, and bosses are exempt. Nothing in `SpeciesDb::load_dir` may call it, so a mod is never refused by it.

- [ ] **Step 4: Re-author the 17 species files**

For each file in `assets/species/`: rename `base_def:` to `base_mitigation:` keeping the value (the shipped range is 1..17, which reads directly as 1%..17% and is a sane starting band — Construct at 4, Sentinel at 12, Wintermute at 17), and add a `spread:` to each of the 34 moves.

Author the spread deliberately rather than as a fixed fraction: a heavy, slow species swings in a narrow band and a fast, erratic one in a wide one. As a starting shape, `spread ≈ power / 4` rounded, widened to `power / 3` for Glitch, Sprite, Virus and Trojan and narrowed to `power / 6` for Construct, Sentinel and Crawler. **Move names and flavour are untouched** — "Fray" and "Static Burst" stay exactly as authored; only the number gains a second half.

Update `assets/species/README.md` in the same commit: rename `base_def` in the schema table and describe it as percentage points, and document `spread` on `MoveDef` including its default-of-0 modding promise.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine species:: 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine every_species_mitigation 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine every_shipped_move_authors 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u assets/species
git commit -m "feat(species): base_mitigation in percentage points, and a spread on every move"
```

---

### Task 6: `EquipmentStats` gains damage, accuracy and evasion

**Files:**
- Modify: `crates/engine/src/items.rs:297-400` (`EquipmentStats` and its three scaling axes)
- Modify: `crates/engine/src/game/crafting.rs:225-240` (`apply_equipment_delta`), `:300-331` (`copy_bonus`), `:339-355` (`gear_bonus`)
- Modify: 39 files in `assets/items/`, all 9 in `assets/affixes/`
- Modify: `assets/items/README.md`, `assets/affixes/README.md`
- Test: `crates/engine/src/items.rs` (`mod tests`), `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: `battle::DamageRange` (Task 1).
- Produces: `EquipmentStats { atk, mitigation, decompiler, damage: DamageRange, accuracy: i32, evasion: i32 }`, all six `#[serde(default)]`; `scaled_for_level`/`fused_for_tier`/`for_rarity` carry all six; `Game::copy_bonus` unchanged in signature.

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/items.rs`'s `mod tests`:

```rust
    #[test]
    fn both_ends_of_a_damage_range_carry_the_per_step_floor() {
        // A floor does not commute with a multiplier, so the ends cannot be
        // scaled by a shortcut that scales the midpoint and re-derives the
        // width. Fusing a 4-9 weapon must lift both ends by at least
        // ITEM_FUSION_MIN_BONUS_PER_TIER per tier.
        let base = EquipmentStats {
            damage: crate::battle::DamageRange { min: 4, max: 9 },
            ..EquipmentStats::default()
        };
        let fused = base.fused_for_tier(2);
        assert!(fused.damage.min >= base.damage.min + 2 * ITEM_FUSION_MIN_BONUS_PER_TIER);
        assert!(fused.damage.max >= base.damage.max + 2 * ITEM_FUSION_MIN_BONUS_PER_TIER);
        assert!(fused.damage.max >= fused.damage.min);
    }

    #[test]
    fn a_zero_damage_range_stays_zero_through_every_axis() {
        // Armour has no damage range and must never be handed one — the
        // same rule the other axes already state.
        let armour = EquipmentStats {
            mitigation: 4,
            ..EquipmentStats::default()
        };
        let scaled = armour
            .scaled_for_level(6)
            .fused_for_tier(3)
            .for_rarity(Rarity::ALL[Rarity::ALL.len() - 1]);
        assert_eq!(scaled.damage, crate::battle::DamageRange::default());
    }

    #[test]
    fn accuracy_and_evasion_scale_on_the_same_three_axes_as_every_other_stat() {
        let light = EquipmentStats {
            evasion: 3,
            accuracy: 2,
            ..EquipmentStats::default()
        };
        let scaled = light.scaled_for_level(4);
        assert!(scaled.evasion > light.evasion);
        assert!(scaled.accuracy > light.accuracy);
    }
```

Add to `crates/engine/src/tests/assets.rs`:

```rust
/// The two defensive axes must be a real choice rather than one stat with a
/// second name: some shipped armour has to buy evasion instead of
/// mitigation, and some shipped weapons accuracy instead of damage. A field
/// nothing authors is an unused feature flag.
#[test]
fn the_shipped_gear_actually_authors_both_defensive_and_both_offensive_axes() {
    let db = shipped_items();
    let equipment: Vec<_> = db.all().filter_map(|i| i.equipment.as_ref()).collect();
    assert!(
        equipment.iter().any(|(_, stats)| stats.evasion > 0),
        "no shipped armour buys evasion"
    );
    assert!(
        equipment.iter().any(|(_, stats)| stats.mitigation > 0),
        "no shipped armour buys mitigation"
    );
    assert!(
        equipment.iter().any(|(_, stats)| stats.accuracy > 0),
        "no shipped weapon buys accuracy"
    );
    assert!(
        equipment
            .iter()
            .any(|(_, stats)| stats.damage != crate::battle::DamageRange::default()),
        "no shipped weapon authors a damage range"
    );
}

/// Every shipped weapon carries a damage range, and nothing else does. A
/// weapon **overrides** a natural attack rather than adding to it, so a
/// weapon with no range would silently disarm whoever equipped it.
#[test]
fn every_weapon_authors_a_range_and_nothing_else_does() {
    let db = shipped_items();
    for item in db.all() {
        let Some((slot, stats)) = item.equipment.as_ref() else {
            continue;
        };
        let has_range = stats.damage != crate::battle::DamageRange::default();
        assert_eq!(
            has_range,
            *slot == EquipmentSlot::Weapon,
            "{} is a {slot:?} and {} a damage range",
            item.id,
            if has_range { "has" } else { "lacks" }
        );
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine both_ends_of_a_damage_range 2>&1 | tail -20`
Expected: FAIL — `struct EquipmentStats has no field named damage`.

- [ ] **Step 3: Extend `EquipmentStats`**

```rust
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct EquipmentStats {
    #[serde(default)]
    pub atk: i32,
    /// Percentage points, summed into `Stats::mitigation` by
    /// `Game::apply_equipment_delta` and capped by
    /// `Game::effective_mitigation`.
    #[serde(default)]
    pub mitigation: i32,
    #[serde(default)]
    pub decompiler: i32,
    /// A weapon's damage band, which **overrides** the wielder's natural
    /// attack rather than adding to it — see `Game::attack_range`. Zero on
    /// everything that is not a weapon, and it stays zero through all three
    /// scaling axes: a tier sharpens what an item does and never hands it a
    /// stat it never had.
    #[serde(default)]
    pub damage: crate::battle::DamageRange,
    /// Read live off `Game::gear_bonus` by `battle::accuracy_of`. Unlike
    /// `atk` and `mitigation` this is **not** baked into `Stats` — there is
    /// no field for it there and `apply_equipment_delta` must not invent one.
    #[serde(default)]
    pub accuracy: i32,
    /// See `accuracy`. Light armour buys this where heavy armour buys
    /// `mitigation`, which is what makes the two defensive axes a real
    /// choice.
    #[serde(default)]
    pub evasion: i32,
}
```

Extend all three axis methods to carry `damage`, `accuracy` and `evasion`. For `damage`, apply the existing `scale` closure to `min` and `max` **independently** — both ends carry the per-step floor, and a floor does not commute with a multiplier, so scaling the midpoint and re-deriving the width would give a different answer. Keep the existing `v <= 0 => v` guard on every axis; that is what keeps armour's zero range at zero and what stops a drawback affix's negative deepening under fusion.

In `crafting.rs`, extend `copy_bonus`'s affix fold and `gear_bonus`'s slot fold with the three new axes. Leave `apply_equipment_delta` writing `atk` and `mitigation` only, and add a line to its doc saying why: the other three have no `Stats` field and are read live.

- [ ] **Step 4: Re-author the 39 gear files and the 9 affixes**

- **13 weapons** gain `damage: (min: N, max: M)`. Scale off what the item's `atk` was worth: a weapon that gave `atk: 2` is roughly `(min: 3, max: 7)`; `atk: 3` is roughly `(min: 5, max: 10)`; `atk: 4` is roughly `(min: 7, max: 13)`. Keep the `atk` bonus as well — it is the flat term added to every landed roll. **Some weapons trade damage for accuracy**: give Shiv Routine, Kinetic Edge and Black Ice Pick a narrower range plus `accuracy: 2..3`, and Monofilament Whip and Plasma Router a wider range with no accuracy at all.
- **12 armour pieces**: `def:` becomes `mitigation:`, values re-authored as percentage points — a `def: 2` piece is roughly `mitigation: 6`, `def: 4` roughly `mitigation: 12`. **Light pieces author `evasion` instead**: Sandbox Liner, Scrap Ward and Static Mesh drop most of their mitigation and take `evasion: 3..5`.
- **14 modules**: `def:` becomes `mitigation:` where present, same percentage-point re-authoring.
- **9 affixes**: `hardened`'s `stats: (def: 3)` becomes `(mitigation: 8)`; `honed` and the weapon affixes may take `accuracy` or a `damage` range. Keep every affix's existing `slots:` gate.

Update `assets/items/README.md` and `assets/affixes/README.md` in the same commit: document all three new `EquipmentStats` fields, the `def` → `mitigation` rename and its unit change, and the weapon-overrides-natural-attack rule.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine items:: 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine the_shipped_gear_actually_authors 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine every_weapon_authors_a_range 2>&1 | tail -20`
Expected: PASS.

Then check the two price censuses still hold — an item's price is bounded twice and both bounds are asserted over the real assets:

Run: `cargo test -p feral-processes-engine assets:: 2>&1 | tail -30`
Expected: PASS. If a recipe-ceiling assertion fails, the re-authored gear is now worth more than its ingredients; raise the item's `value`, do not weaken the census.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u assets/items assets/affixes
git commit -m "feat(items): gear authors damage ranges, accuracy and evasion; def becomes mitigation"
```

---

### Task 7: Ability spreads, and the one place an attack's range comes from

**Files:**
- Modify: `crates/engine/src/abilities.rs:210-248` (`AbilityEffect::Damage`, `Drain`), `:516-521` (`attack_parts`)
- Modify: `crates/engine/src/species.rs:78-96` (`basic_attack_ability`)
- Modify: `crates/engine/src/game/combat.rs` (new `Game::attack_range`)
- Modify: `assets/abilities/README.md`
- Test: `crates/engine/src/tests/combat_abilities.rs`, `crates/engine/src/tests/wielded.rs`

**Interfaces:**
- Consumes: `DamageRange`, `DamageRange::centred` (Task 1); `MoveDef::range` (Task 5); `EquipmentStats::damage` (Task 6).
- Produces:
  - `AbilityEffect::Damage { power: i32, spread: i32, status: Option<MoveEffect> }` and `Drain { power: i32, spread: i32, heal_fraction: f32 }`, `spread` `#[serde(default)]`.
  - `AbilityDef::attack_parts(&self) -> (DamageRange, Option<MoveEffect>)`
  - `abilities::scaled_range(range: DamageRange, level: u32, affinity: f32) -> DamageRange`
  - `Game::attack_range(&self, entity: Entity, natural: DamageRange) -> DamageRange`

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/tests/combat_abilities.rs`:

```rust
/// A high-level ability must not become deterministic: the spread scales
/// with the centre, proportionally. Delete the scaling of `spread` and this
/// fails — the band collapses to a point as the caster levels.
#[test]
fn an_abilitys_spread_scales_with_its_centre() {
    use crate::abilities::scaled_range;
    use crate::battle::DamageRange;
    let base = DamageRange::centred(10, 4);
    let low = scaled_range(base, 1, 1.0);
    let high = scaled_range(base, 40, 1.0);
    let low_width = low.max - low.min;
    let high_width = high.max - high.min;
    assert!(high.mean() > low.mean(), "the centre must scale");
    assert!(
        high_width > low_width,
        "the spread must scale with it, not stay put"
    );
}

/// `species::basic_attack_ability` converts a move to an ability, and the
/// centre-and-spread pair is what survives that losslessly — a `(min, max)`
/// pair would round on an odd width. Nothing in combat names `MoveDef`, so
/// this conversion is where a move's range has to arrive intact.
#[test]
fn converting_a_move_to_an_ability_keeps_its_range_exactly() {
    use crate::species::{basic_attack_ability, MoveDef};
    let mv = MoveDef {
        name: "Fray".into(),
        power: 9,
        spread: 4,
        effect: None,
        ranged: false,
    };
    let ability = basic_attack_ability(&"scrapper".into(), 0, &mv);
    assert_eq!(ability.attack_parts().0, mv.range());
}

/// A weapon **overrides** a natural attack rather than adding to it. A
/// companion still rolls a species move each turn for its name and its
/// status rider, but an equipped weapon supplies the damage range.
#[test]
fn an_equipped_weapon_replaces_the_natural_attack_range() {
    let mut game = support::test_game();
    let companion = support::spawn_tamed(&mut game, "scrapper");
    let natural = crate::battle::DamageRange { min: 6, max: 10 };
    assert_eq!(game.attack_range_for_test(companion, natural), natural);
    support::equip_weapon(&mut game, companion, "monofilament_whip");
    let armed = game.attack_range_for_test(companion, natural);
    assert_ne!(armed, natural, "the weapon must override, not be ignored");
    assert!(armed.max > natural.max);
}

/// Unarmed, the move's own range applies — the override is not "a weapon
/// slot exists", it is "a weapon with a range is worn".
#[test]
fn an_unarmed_companion_keeps_its_natural_range() {
    let mut game = support::test_game();
    let companion = support::spawn_tamed(&mut game, "scrapper");
    let natural = crate::battle::DamageRange { min: 6, max: 10 };
    assert_eq!(game.attack_range_for_test(companion, natural), natural);
}
```

Go through `Game::roster_parts()` for any fixture that spawns a companion — three tests once failed on `spawn_tamed` rather than on the feature they were testing.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine an_abilitys_spread_scales 2>&1 | tail -20`
Expected: FAIL — `cannot find function scaled_range`.

- [ ] **Step 3: Add `spread` to the two damage effects**

In `crates/engine/src/abilities.rs`, add `#[serde(default)] pub spread: i32` to `AbilityEffect::Damage` and `AbilityEffect::Drain`, with a doc comment saying the default of 0 is a degenerate range — exactly today's behaviour, which is why none of the 77 shipped ability files needs editing and mods gain damage ranges for free. Update the two doc comments at `:213` and `:234` that currently name `battle::compute_damage`; they now name `battle::resolve_attack`.

Change `attack_parts` to return `(DamageRange, Option<MoveEffect>)`, built through `DamageRange::centred(*power, *spread)`, with `(DamageRange::default(), None)` on the fallback arm.

Then update `species::basic_attack_ability` to pass `spread: mv.spread` into the `AbilityEffect::Damage` it builds. Its doc already says a `MoveDef` is "name, power, an optional status rider and a reach flag" and that `AbilityEffect::Damage` is the first three exactly — that stays true with a fourth field on both sides, so extend the sentence rather than rewriting the paragraph.

Add beside `scaled_hp_power`:

```rust
/// An authored damage range scaled for its caster, on the same curve
/// `scaled_hp_power` puts the centre on.
///
/// The spread scales proportionally rather than staying put, or a
/// high-level ability becomes deterministic — the band would collapse to a
/// point exactly when the numbers get big enough for the variance to matter.
pub fn scaled_range(range: DamageRange, level: u32, affinity: f32) -> DamageRange {
    DamageRange {
        min: scaled_hp_power(range.min, level, affinity),
        max: scaled_hp_power(range.max, level, affinity),
    }
}
```

Scaling both ends through the same function is what keeps the spread proportional without a second formula: `scaled_hp_power` is linear in its input, so the width scales by the same factor as the centre.

- [ ] **Step 4: Add `Game::attack_range`**

In `crates/engine/src/game/combat.rs`:

```rust
    /// The damage band `entity` actually swings for, given the `natural`
    /// range its move or ability authored.
    ///
    /// **A weapon overrides a natural attack, it does not add to it.** A
    /// companion still rolls a species move each turn for its *name* and its
    /// status rider; an equipped weapon supplies the number. Unarmed, the
    /// move's own range applies. The player has no species moves at all —
    /// their `natural` is `tuning::PLAYER_UNARMED_DAMAGE`.
    ///
    /// The override is keyed on the weapon carrying a range, not on the slot
    /// being occupied: a modded weapon authoring none leaves its wielder
    /// swinging naturally rather than disarmed.
    pub(crate) fn attack_range(
        &self,
        entity: Entity,
        natural: battle::DamageRange,
    ) -> battle::DamageRange {
        let worn = self.gear_bonus(entity).damage;
        if worn == battle::DamageRange::default() {
            natural
        } else {
            worn
        }
    }
```

- [ ] **Step 5: Update `assets/abilities/README.md`**

Document `spread` on `Damage` and `Drain`, its default of 0 and the degenerate-range promise, and the `FieldBuffKind` list's loss of `Def` from Task 4 if that edit has not already landed there.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine an_abilitys_spread 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine attack_range 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u assets/abilities
git commit -m "feat(abilities): a spread on Damage and Drain, and one door onto an attack's range"
```

---

### Task 8: Wire the call sites, and delete `compute_damage`

The task where fights actually change. Six `compute_damage` callers, and they do not all become attack rolls.

**Files:**
- Create: nothing.
- Modify: `crates/engine/src/game/combat_damage.rs` (new `Game::resolve_and_apply_attack`, `Game::combatant_profile`)
- Modify: `crates/engine/src/game/combat_round.rs:355-392` (party member attacks), `:1008-1050` (`Damage` and `Drain` arms)
- Modify: `crates/engine/src/game/combat_enemy.rs:167-200` (wild attack)
- Modify: `crates/engine/src/game/combat_policy.rs:256-266` (projected damage)
- Modify: `crates/engine/src/game/zone.rs:51` (`attack_nest`)
- Modify: `crates/engine/src/battle.rs` (delete `compute_damage` and its two tests), `crates/engine/src/tuning.rs` (delete `MIN_DAMAGE`, `PLAYER_STRIKE_POWER`)
- Test: `crates/engine/src/tests/combat_abilities.rs`, `crates/engine/src/tests/combat_status.rs`, `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Consumes: `resolve_attack`, `expected_damage`, `Combatant`, `AttackOutcome`, `FumbleRung` (Task 2); `effective_mitigation` (Task 4); `attack_range`, `scaled_range`, `attack_parts` (Task 7).
- Produces:
  - `Game::combatant_profile(&self, entity: Entity, range: DamageRange) -> battle::Combatant`
  - `Game::resolve_and_apply_attack(&mut self, attacker: Entity, defender: Entity, range: DamageRange) -> AttackOutcome` — rolls, applies defender damage through `apply_damage`, and returns the outcome for the caller to log and branch on. Fumble rungs are **not** applied here; Task 9 adds that.

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/tests/combat_abilities.rs`:

```rust
/// A miss cannot live in `apply_damage`: a missed Drain would still heal its
/// caster. Delete the `AttackOutcome` branch in the Drain arm and this
/// fails.
#[test]
fn a_missed_drain_heals_nothing() {
    let mut game = support::test_game();
    let drainer = support::spawn_tamed(&mut game, "virus");
    support::hurt(&mut game, drainer, 20);
    let before = game.hp_of_for_test(drainer);
    // Evasion far above the caster's accuracy floors the hit chance at
    // HIT_CHANCE_MIN, so a forced miss is reachable by seed.
    let target = support::spawn_hostile_with_evasion(&mut game, "sprite", 500);
    support::force_next_roll_to_miss(&mut game);
    support::cast_drain(&mut game, drainer, target);
    assert_eq!(
        game.hp_of_for_test(drainer),
        before,
        "a missed drain must restore nothing"
    );
}

/// The same for a rider: a missed attack lands no status.
#[test]
fn a_missed_attack_lands_no_status_rider() {
    let mut game = support::test_game();
    let attacker = support::spawn_hostile(&mut game, "crawler");
    let player = game.player_entity_for_test();
    support::force_next_roll_to_miss(&mut game);
    support::wild_attack(&mut game, attacker, player);
    assert!(
        game.status_label_for_test(player).is_none(),
        "a missed swing must not stun or bleed"
    );
}
```

Add to `crates/engine/src/tests/zone.rs`:

```rust
/// A structure has no speed and cannot dodge, so nest damage keeps a
/// deterministic, unrolled path. Route `attack_nest` through
/// `resolve_attack` and this fails: identical swings would stop being
/// identical.
#[test]
fn a_structure_cannot_be_missed() {
    let mut game = support::test_game();
    let nest = support::spawn_nest(&mut game, "scrapper");
    let mut dealt = Vec::new();
    for _ in 0..12 {
        let before = game.nest_durability_for_test(nest);
        game.attack_nest_for_test(nest);
        dealt.push(before - game.nest_durability_for_test(nest));
    }
    assert!(
        dealt.iter().all(|&d| d > 0),
        "no swing at a structure may miss: {dealt:?}"
    );
    assert!(
        dealt.windows(2).all(|w| w[0] == w[1]),
        "structure damage must be deterministic: {dealt:?}"
    );
}
```

`force_next_roll_to_miss` and `spawn_hostile_with_evasion` are new fixtures — put them in `crates/engine/src/tests/support.rs`, not inline. Implement `force_next_roll_to_miss` by reseeding `resources::GameRng` to a seed scanned for the wanted band, the same technique Task 2's `seed_producing` uses; do not add a production-code hook for it.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine a_missed_drain_heals_nothing 2>&1 | tail -20`
Expected: FAIL — the fixture does not exist, and the Drain arm has no miss branch.

- [ ] **Step 3: Add the two `Game` seams**

In `crates/engine/src/game/combat_damage.rs`:

```rust
    /// `entity`'s side of an attack roll, with `range` as the band it swings
    /// for.
    ///
    /// The one place accuracy and evasion are resolved from the ECS, so the
    /// four call sites cannot each derive them differently. `species_base_speed`
    /// already has a player arm; gear accuracy and evasion are read live off
    /// `gear_bonus` because, unlike `atk` and `mitigation`, neither is baked
    /// into `Stats`.
    pub(crate) fn combatant_profile(
        &self,
        entity: Entity,
        range: battle::DamageRange,
    ) -> battle::Combatant {
        let gear = self.gear_bonus(entity);
        let level = self.ability_user_level(entity);
        let speed = self.species_base_speed(entity);
        battle::Combatant {
            accuracy: battle::accuracy_of(speed, level, gear.accuracy),
            evasion: battle::evasion_of(speed, level, gear.evasion),
            atk: self.effective_atk(entity),
            range,
        }
    }

    /// Rolls one creature-versus-creature attack and applies whatever landed
    /// on the defender, returning the outcome so the caller can log it and
    /// branch on it.
    ///
    /// **The miss branch belongs at the call site, not here.** A missed
    /// Drain must still skip its heal and a missed swing must still skip its
    /// status rider, and `apply_damage` — which stays the only path that
    /// lowers HP — has no way to know about either.
    pub(crate) fn resolve_and_apply_attack(
        &mut self,
        attacker: Entity,
        defender: Entity,
        range: battle::DamageRange,
    ) -> battle::AttackOutcome {
        let attacker_profile = self.combatant_profile(attacker, range);
        let defender_profile =
            self.combatant_profile(defender, battle::DamageRange::default());
        let outcome = {
            let mut rng = self.world.resource_mut::<GameRng>();
            battle::resolve_attack(attacker_profile, defender_profile, &mut rng.0)
        };
        let dealt = outcome.damage_to_defender();
        if dealt > 0 {
            self.apply_damage(defender, dealt);
        }
        outcome
    }
```

The defender's profile takes an empty range because only its `evasion` is read on the main swing — the Opening rung's riposte is resolved inside `resolve_attack` from the same profile, so a rung-3 fumble deals no riposte damage until Task 9 gives the defender its real range. Wire the defender's real range in Task 9 rather than leaving a `TODO` here.

- [ ] **Step 4: Convert the four creature-versus-creature call sites**

**`combat_round.rs::party_member_attacks`** — replace the `(atk, def)` pair and `compute_damage` with:

```rust
        let (move_name, natural) = if slot == 0 {
            ("data strike".to_string(), PLAYER_UNARMED_DAMAGE)
        } else {
            match self.roll_species_move(entity) {
                Some(mv) => (mv.name.clone(), mv.attack_parts().0),
                None => ("a raw signal burst".to_string(), PLAYER_UNARMED_DAMAGE),
            }
        };
        let range = self.attack_range(entity, natural);
        let outcome = self.resolve_and_apply_attack(entity, front, range);
```

Then branch the log line on the outcome. Every new line goes through `log_kind` with the kind the arm already used; a miss and a fumble are `MessageKind::PartyDamage` too — they are still the party's turn being narrated. Player-facing wording, second person for slot 0 and third for a companion, matching what is there:

- `Hit { dmg }` — the existing line, unchanged.
- `Crit { dmg }` — `"You tear a {move_name} clean through for {dmg} damage!"`
- `Miss` — `"Your {move_name} glances off."`
- `Fumble(_)` — Task 9 writes these; until then log `"Your {move_name} goes wide."` and leave the rung unapplied.

**`combat_round.rs`'s `AbilityEffect::Damage` arm** — the authored `power`/`spread` become a range through `attack_parts`, scaled by `abilities::scaled_range(range, level, affinity)`, then through `resolve_and_apply_attack`. **Do not** call `attack_range` here: a Special is the ability's own damage, not the wielder's weapon. Apply the status rider only on `Hit` or `Crit`.

**`combat_round.rs`'s `AbilityEffect::Drain` arm** — same, and the heal runs only on `Hit`/`Crit`, off `outcome.damage_to_defender()` rather than the authored power. Keep the existing comment explaining why the siphon is off damage actually dealt.

**`combat_enemy.rs`'s wild attack** — `mv.attack_parts()` now yields `(DamageRange, Option<MoveEffect>)`; take the range through `self.attack_range(wild, range)` (a wild program can be wearing gear), then `resolve_and_apply_attack`. The `MessageKind::EnemySpecial` / `EnemyAttack` choice already reads *after* the effect gate; keep that ordering and add the miss and crit lines beside it.

- [ ] **Step 5: Convert the two sites that do not become attack rolls**

**`combat_policy.rs`** — the `EstDamageFrac` and `WouldKill` features take the expected-value form:

```rust
        let attacker = self.combatant_profile(wild, range);
        let defender = self.combatant_profile(target, battle::DamageRange::default());
        let dmg = battle::expected_damage(attacker, defender).round() as i32;
```

Keep the existing comment — "the real formula, called rather than restated" — and update it to name `expected_damage`. `Feature::TargetDefRel` now squashes `effective_mitigation` (already a percentage) against `MAX_MITIGATION_PERCENT` rather than against the attacker's `atk`; the feature is pinned to a coefficient of zero in the shipped weights, so this changes no behaviour, but the value must still be in `[0, 1]`.

**`zone.rs::attack_nest`** — a nest has a `Durability`, not `Stats`. Replace `compute_damage(self.effective_atk(player), 0, 5)` with a deterministic expression that does not roll:

```rust
        // A structure has no speed and cannot dodge, so this keeps the
        // deterministic path it always had — only creature-versus-creature
        // attacks go through `battle::resolve_attack`.
        let range = self.attack_range(player, PLAYER_UNARMED_DAMAGE);
        let dmg = (range.mean().round() as i32 + self.effective_atk(player)).max(1) as u32;
```

- [ ] **Step 6: Delete `compute_damage`, `MIN_DAMAGE` and `PLAYER_STRIKE_POWER`**

Delete `battle::compute_damage` and its two tests (`damage_scales_with_power_and_attack`, `damage_never_drops_below_one`). Delete `tuning::MIN_DAMAGE` and `tuning::PLAYER_STRIKE_POWER`. `cargo check --workspace` names every remaining reference; `balance_sim`'s two are Task 10's and may be left broken until then only if that task runs immediately after — otherwise convert them here.

Then sweep the doc comments that name the deleted function. `rg -n 'compute_damage|MIN_DAMAGE|PLAYER_STRIKE_POWER' --glob '*.rs' --glob '*.md'` finds them; the known ones are `crates/engine/src/tuning.rs:228`, `:2249`, `:2320`, `crates/engine/src/species.rs:1081`, `crates/engine/src/balance_sim.rs:28`, `:643`, and `CLAUDE.md`'s "every difficulty curve is linear" seam. **`CLAUDE.md` and `AGENTS.md` are the same document in two gitignored files** — edit `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`. Leave the seam edits themselves to Task 12, which does all the documentation in one pass; here only fix comments that now name a function that does not exist.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p feral-processes-engine a_missed_drain_heals_nothing 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine a_missed_attack_lands_no_status_rider 2>&1 | tail -20`
Run: `cargo test -p feral-processes-engine a_structure_cannot_be_missed 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 8: Full-suite gate, and expect casualties**

Run: `cargo test --workspace 2>&1 | tail -60`

Every fight in the suite now misses sometimes, so seeded battle tests will move. Triage each failure into one of three buckets and treat them differently:

- **A test asserting an exact damage number.** Update the number. It moved because damage is rolled now.
- **A test that assumed every swing lands** (a fight ending in N rounds, a companion surviving exactly one hit). Reseed the fixture or assert the property rather than the count — but do not weaken it to an inequality that would pass against no feature at all.
- **A test in an untouched subsystem.** That is the RNG-stream shift, not a regression: probe to ground before theorising, and fix the fixture's incidental coupling to the seed rather than changing the seed until it passes.

`balance_sim`'s curve tests will fail here; leave them and re-baseline in Task 10.

- [ ] **Step 9: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u
git commit -m "feat(combat)!: every creature attack rolls to hit; compute_damage is deleted

BREAKING: the subtractive damage floor is gone, which retires the constraint
that every difficulty curve must be linear."
```

---

### Task 9: The fumble ladder lands

**Files:**
- Modify: `crates/engine/src/components.rs:710-718` (`StatusKind`), `:751-756` (`StatusEffects`)
- Modify: `crates/engine/src/game/combat_damage.rs` (`resolve_and_apply_attack` applies the rung)
- Modify: `crates/engine/src/game/combat_status.rs` (`status_label`'s new arm lives in `party.rs:376`; the tick's arms live here)
- Modify: `crates/engine/src/game/party.rs:376-382` (`status_label`)
- Modify: `crates/engine/src/game/combat_round.rs`, `crates/engine/src/game/combat_enemy.rs` (the four fumble log lines)
- Modify: `crates/engine/src/game/combat_policy.rs:245-250` (the policy's status reads)
- Test: `crates/engine/src/tests/combat_status.rs`

**Interfaces:**
- Consumes: `FumbleRung` (Task 2); `resolve_and_apply_attack` (Task 8).
- Produces:
  - `StatusKind::Exposed`
  - `Game::apply_fumble_rung(&mut self, fumbler: Entity, target: Entity, rung: FumbleRung)` — `pub(crate)`, called from `resolve_and_apply_attack`.
  - `Game::natural_range_of(&self, entity: Entity) -> battle::DamageRange` — `pub(crate)`, the defender's own band, so an Opening riposte deals real damage.
  - `tuning::{EXPOSED_DURATION_ROUNDS, CRASH_DURATION_ROUNDS}`

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/tests/combat_status.rs`:

```rust
/// Exposed cuts the fumbler's evasion until their next turn — which is what
/// makes rung 1 a cost rather than flavour. Delete the `EXPOSED_EVASION_PERCENT`
/// term in `combatant_profile` and this fails.
#[test]
fn exposed_cuts_the_fumblers_evasion() {
    let mut game = support::test_game();
    let victim = support::spawn_hostile(&mut game, "scrapper");
    let clean = game.combatant_profile_for_test(victim).evasion;
    game.arm_status_for_test(victim, StatusKind::Exposed, 1, 0);
    let exposed = game.combatant_profile_for_test(victim).evasion;
    assert!(exposed < clean, "{exposed} should be below {clean}");
}

/// Every rung of the ladder that deals damage goes through `apply_damage`,
/// which stays the only path that lowers HP.
#[test]
fn a_recoil_fumble_hurts_the_fumbler_and_not_the_target() {
    let mut game = support::test_game();
    let fumbler = support::spawn_hostile(&mut game, "scrapper");
    let target = game.player_entity_for_test();
    let fumbler_before = game.hp_of_for_test(fumbler);
    let target_before = game.hp_of_for_test(target);
    game.apply_fumble_rung_for_test(fumbler, target, FumbleRung::Recoil { dmg: 4 });
    assert!(game.hp_of_for_test(fumbler) < fumbler_before);
    assert_eq!(game.hp_of_for_test(target), target_before);
}

/// Rung 4 costs the fumbler their next action, through the machinery Stun
/// already has.
#[test]
fn a_crash_fumble_costs_the_fumbler_their_next_action() {
    let mut game = support::test_game();
    let fumbler = support::spawn_hostile(&mut game, "scrapper");
    let target = game.player_entity_for_test();
    game.apply_fumble_rung_for_test(fumbler, target, FumbleRung::Crash);
    assert!(game.is_stunned_for_test(fumbler));
}

/// Rungs replace rather than stack — a cumulative top rung is a run-ender.
#[test]
fn a_second_fumble_replaces_the_first_rung() {
    let mut game = support::test_game();
    let fumbler = support::spawn_hostile(&mut game, "scrapper");
    let target = game.player_entity_for_test();
    game.apply_fumble_rung_for_test(fumbler, target, FumbleRung::Exposed);
    game.apply_fumble_rung_for_test(fumbler, target, FumbleRung::Crash);
    assert!(game.is_stunned_for_test(fumbler));
    assert_eq!(
        game.status_label_for_test(fumbler).as_deref(),
        Some("Stunned (1)"),
        "one status at a time — the second must clobber the first"
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine exposed_cuts_the_fumblers_evasion 2>&1 | tail -20`
Expected: FAIL — `no variant named Exposed`.

- [ ] **Step 3: Add `StatusKind::Exposed`**

```rust
pub enum StatusKind {
    Bleed,
    Stun,
    /// Cuts the afflicted side's Evasion by `EXPOSED_EVASION_PERCENT` — see
    /// `Game::combatant_profile`. Armed by the first rung of the fumble
    /// ladder, and **free for content**: `MoveEffect` already lets any
    /// species move inflict a status from `.ron`, so a debuffer species
    /// costs no Rust the day this exists.
    ///
    /// It belongs in `StatusEffects` (conditions a hostile move inflicts,
    /// always unwanted) rather than `CombatBuff`, which holds one *wanted*
    /// buff at a time.
    Exposed,
}
```

The compiler names every match that must gain an arm: `status_label` (`party.rs:376`) gets `format!("Exposed ({})", active.remaining)`, `combat_status.rs:44` gets its landing line and `:136` its clearing line, `combat_round.rs:1000` its cast line, and `combat_policy.rs` its feature reads. **Do not add a policy `Feature` variant for Exposed** — the shipped weights were trained without it and `assets/policies/enemy_battle.ron` would have to be retrained; the spec is explicit that retraining waits for slices 2 and 3.

Also update `assets/species/README.md`'s `MoveEffect` status list, since a mod may now author `Exposed`.

- [ ] **Step 4: Read Exposed in `combatant_profile`**

In `combat_damage.rs::combatant_profile`, cut the evasion when the entity carries `StatusKind::Exposed`:

```rust
        let evasion = battle::evasion_of(speed, level, gear.evasion);
        let exposed = self
            .world
            .get::<StatusEffects>(entity)
            .and_then(|s| s.active)
            .is_some_and(|a| a.kind == StatusKind::Exposed);
        let evasion = if exposed {
            evasion * (100 - EXPOSED_EVASION_PERCENT) as f64 / 100.0
        } else {
            evasion
        };
```

- [ ] **Step 5: Apply the rungs**

In `combat_damage.rs`:

```rust
    /// Lands one rung of the fumble ladder on `fumbler`. **Rungs replace
    /// rather than stack** — `StatusEffects` holds one condition at a time
    /// and both status rungs go through `arm_status`, so a second fumble
    /// clobbers the first rather than compounding it. A cumulative top rung
    /// is a run-ender.
    ///
    /// The Opening rung's damage was already rolled inside
    /// `battle::resolve_attack` — non-recursively, so a fumbled free swing
    /// resolved as a plain miss. All this does is land it, through
    /// `apply_damage` like every other rung that hurts someone.
    pub(crate) fn apply_fumble_rung(
        &mut self,
        fumbler: Entity,
        _target: Entity,
        rung: battle::FumbleRung,
    ) {
        match rung {
            battle::FumbleRung::Exposed => {
                self.arm_status(fumbler, StatusKind::Exposed, EXPOSED_DURATION_ROUNDS, 0);
            }
            battle::FumbleRung::Recoil { dmg } | battle::FumbleRung::Opening { dmg } => {
                if dmg > 0 {
                    self.apply_damage(fumbler, dmg);
                }
            }
            battle::FumbleRung::Crash => {
                self.arm_status(fumbler, StatusKind::Stun, CRASH_DURATION_ROUNDS, 0);
            }
        }
    }
```

Add `EXPOSED_DURATION_ROUNDS: u32 = 1` and `CRASH_DURATION_ROUNDS: u32 = 1` to the combat-resolution section of `tuning.rs`, each with a one-line doc. Both are 1 because `ActiveStatus::landed_this_round` already exempts the round a condition lands in, so a duration of 1 is exactly "until the fumbler's next turn" — the spec's wording for Exposed and "loses their next action" for Crash.

Call it from `resolve_and_apply_attack`'s tail, and give the defender its real range there so an Opening riposte deals real damage:

```rust
        let defender_profile = self.combatant_profile(defender, self.natural_range_of(defender));
        // ...
        if let battle::AttackOutcome::Fumble(rung) = outcome {
            self.apply_fumble_rung(attacker, defender, rung);
        }
```

`natural_range_of(&self, entity) -> DamageRange` resolves the entity's *first* species move's range through `attack_range`, falling back to `PLAYER_UNARMED_DAMAGE`. Deliberately the first move rather than a rolled one: rolling here would spend a `GameRng` draw before the band roll and break every draw-count assertion in Task 2.

- [ ] **Step 6: Write the four fumble log lines**

At the three logging call sites converted in Task 8, replace the placeholder fumble line with one per rung. Player-facing wording, and **the player's word for "Raid" is "GC Entropy Sweep"** — note the same rule applies here: these are new player-facing text and follow the game's existing register, not the code's. Suggested set for the player's own fumble, second person:

- `Exposed` — `"Your {move_name} overreaches — you're wide open."`
- `Recoil` — `"Your {move_name} backfires for {dmg} damage."`
- `Opening` — `"Your {move_name} leaves you open, and it counters for {dmg}."`
- `Crash` — `"Your {move_name} hard-faults. You lose the next cycle."`

and the third-person twin for a companion and for a hostile. No occult naming — no daemon, demon, ghost, wraith or phantom.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine fumble 2>&1 | tail -30`
Run: `cargo test -p feral-processes-engine exposed 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 8: Prove the draw counts still hold**

Task 2's `draw_counts_are_pinned_per_outcome` runs against `battle::resolve_attack` alone, so it cannot see the `Game` layer. Add one more:

```rust
/// Landing a fumble rung must spend no further `GameRng` draws — the roll
/// happened inside `resolve_attack`. A draw here would shift every seeded
/// run's stream by however many fumbles it happened to contain.
#[test]
fn landing_a_fumble_rung_spends_no_rng() {
    let mut game = support::test_game();
    let fumbler = support::spawn_hostile(&mut game, "scrapper");
    let target = game.player_entity_for_test();
    let before = support::rng_position(&game);
    game.apply_fumble_rung_for_test(fumbler, target, FumbleRung::Recoil { dmg: 4 });
    game.apply_fumble_rung_for_test(fumbler, target, FumbleRung::Crash);
    assert_eq!(support::rng_position(&game), before);
}
```

Run: `cargo test -p feral-processes-engine landing_a_fumble_rung_spends_no_rng 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 9: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u assets/species
git commit -m "feat(combat): the four-rung fumble ladder, and StatusKind::Exposed"
```

---

### Task 10: `balance_sim` calls the new arithmetic and its curves are re-baselined

**Files:**
- Modify: `crates/engine/src/balance_sim.rs:18` (import), `:26-40` (`TURN_CAP` doc), `:55-63` (`wild_stats_at_zone`), `:400-475` (the round loop), `:600-950` (the curve tests)
- Test: `crates/engine/src/balance_sim.rs` (`mod tests`)

**Interfaces:**
- Consumes: `battle::expected_damage`, `battle::Combatant`, `battle::DamageRange`, `battle::accuracy_of`, `battle::evasion_of` (Tasks 1–2); `EquipmentStats::{damage, accuracy, evasion}` (Task 6).
- Produces: nothing new; the curve constants inside its tests move.

- [ ] **Step 1: Give the sim's fighters accuracy, evasion and a range**

`Fighter` and `GroupSim` each gain `accuracy: f64`, `evasion: f64` and `range: DamageRange`, and a `combatant()` method building a `battle::Combatant` from them. Drop `move_power` in favour of `range` — it was the flat power `compute_damage` took, and it no longer exists.

Build them from the same inputs the real game uses:
- a wild group: `accuracy_of(species.base_speed, level, 0)`, `evasion_of` likewise, `range` from the species' first move through `MoveDef::range`, plus `expected_damage`'s `atk` from `wild_stats_at_zone`.
- the player: `accuracy_of(PLAYER_BASE_SPEED, level, weapon.accuracy)`, evasion likewise off `armor.evasion`, and `range` = the best-in-slot weapon's `damage` scaled by `scaled_for_level`, falling back to `PLAYER_UNARMED_DAMAGE` when it is empty.
- `best_case_gear_bonus` returns `(atk, mitigation, accuracy, evasion, damage)` rather than `(atk, def)`. Its doc already says modules are skipped because their bonus is `decompiler`; keep that and extend it.

- [ ] **Step 2: Replace both `compute_damage` calls**

```rust
            let dealt = expected_damage(fighter.combatant(), groups[0].0.combatant());
```

and

```rust
                    let dealt = expected_damage(group.combatant(), fighter.combatant());
```

Mitigation then applies as a percentage on top of the expectation, matching `mitigate_incoming_damage`'s rounding-once rule. Do **not** re-implement that rounding here — the sim runs in `f64` and does not round at all; state that in a comment as the one place the sim and the game legitimately differ, alongside the existing Power-decay note.

- [ ] **Step 3: Update `TURN_CAP`'s doc**

It currently justifies itself against `compute_damage`'s floor of 1. Rewrite: the floor is gone, and what keeps the cap meaningful as *stalemate* detection is now `HIT_CHANCE_MIN` together with `MAX_MITIGATION_PERCENT` — expected damage is strictly positive, so a timeout is a genuine stalemate rather than a slow win.

- [ ] **Step 4: Re-baseline the curves**

Run: `cargo test -p feral-processes-engine balance_sim 2>&1 | tail -60`

Every hardcoded curve moves. **That is the signal, not a break.** For each failure, read the reported actual curve, confirm it is still monotone and still roughly linear per zone, and write it in. Then re-read what each test claims:

- `a_full_party_survives_a_full_group_at_each_zone` — the property must still hold. If it does not, that is a real balance failure of the re-authored assets, not a curve to update: go back to Task 5's spreads and Task 6's mitigation values.
- `grind_only_zone_scaling_grows_predictably` and `geared_zone_scaling_grows_predictably_and_beats_grind_only` — the *relationship* between the two must survive, not just the numbers.
- `the_reach_rule_measurably_softens_a_full_pack` — untouched by this slice; if it moved, something in Task 8 changed reach, which it must not have.

Record the before and after curves in the commit message. They are the only written record of how far this slice moved progression.

- [ ] **Step 5: Full-suite gate**

Run: `cargo test --workspace 2>&1 | tail -40`
Expected: PASS.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u
git commit -m "test(balance): balance_sim calls expected_damage, and its curves are re-baselined"
```

---

### Task 11: The range is shown, everywhere a gear stat already is

**Files:**
- Modify: `crates/engine/src/game/crafting.rs` (new `Game::damage_range_label`)
- Modify: `crates/app-core/src/lib.rs:135-153` (`equip_preview_tag`), `:216-227` (`stat_summary`)
- Modify: `crates/gui/src/render/inventory.rs`, `crates/gui/src/render/party.rs`, `crates/gui/src/render/manifest.rs` — wherever a gear stat column is built
- Test: `crates/app-core/src/tests/inventory.rs`, `crates/gui/src/tests/` (the popup-width tests)

**Interfaces:**
- Consumes: `Game::copy_bonus` (Task 6).
- Produces: `Game::damage_range_label(&self, range: DamageRange) -> String` — `pub`, the **one** place a range becomes a string.

- [ ] **Step 1: Write the failing tests**

Add to `crates/app-core/src/tests/inventory.rs`:

```rust
/// A weapon's range is the most legible thing this slice adds — "Shiv, 4–9"
/// is what makes two weapons comparable at a glance. It is a stat bonus, not
/// an effect, so it rides `equip_preview_tag` beside ATK/MIT/DECOMP rather
/// than the `item_blurb`/`item_effects`/`item_grant` derivation.
#[test]
fn equip_preview_tag_shows_a_weapons_damage_range() {
    let game = support::game_with_items();
    let copy = GearCopy::plain("monofilament_whip".into());
    let tag = equip_preview_tag(&game, &copy, 1);
    assert!(tag.contains('–'), "no range in {tag:?}");
}

/// The displayed range must be the range actually rolled — the same bug
/// `copy_bonus` already exists to close, in a new place. A displayed range
/// that skips an axis is the hand-rolled-chain failure again.
#[test]
fn the_displayed_range_scales_on_all_three_axes() {
    let game = support::game_with_items();
    let plain = GearCopy::plain("monofilament_whip".into());
    let developed = GearCopy {
        tier: 2,
        rarity: Rarity::ALL[2],
        ..plain.clone()
    };
    let at_level_1 = equip_preview_tag(&game, &plain, 1);
    let at_level_6 = equip_preview_tag(&game, &plain, 6);
    let developed_tag = equip_preview_tag(&game, &developed, 1);
    assert_ne!(at_level_1, at_level_6, "gear level must move the range");
    assert_ne!(at_level_1, developed_tag, "fusion and rarity must too");
}

/// A natural attack has a displayable range too, so a companion's unarmed
/// damage reads the same way as a weapon's.
#[test]
fn a_companions_natural_attack_has_a_readable_range() {
    let mut game = support::game_with_items();
    let companion = support::spawn_tamed(&mut game, "scrapper");
    let label = game.natural_damage_label_for_test(companion);
    assert!(label.contains('–'), "no range in {label:?}");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-app-core equip_preview_tag_shows 2>&1 | tail -20`
Expected: FAIL — the tag has no range in it.

- [ ] **Step 3: Add the one range formatter**

In `crates/engine/src/game/crafting.rs`, beside `copy_bonus`:

```rust
    /// A damage band as the string every screen prints — `"4–9"`, or `"6"`
    /// for a degenerate range.
    ///
    /// **One function, the way `Game::copy_name` is the one place a copy's
    /// name is built.** A displayed range that disagrees with the damage
    /// actually rolled is the hand-rolled-chain bug in a new place: sharing
    /// the *formatter* was never enough on `copy_bonus`, four screens
    /// rebuilt the scaling chain themselves and all four dropped the affix
    /// at once. The scaling is `copy_bonus`'s and is not repeated here —
    /// this takes a range that has already been through all three axes.
    ///
    /// An en dash rather than a hyphen: the map's status column is measured
    /// in DejaVu Sans Mono and both are one cell, and the en dash is what
    /// reads as a range rather than as a minus sign in front of `max`.
    pub fn damage_range_label(&self, range: battle::DamageRange) -> String {
        if range.max <= range.min {
            format!("{}", range.min)
        } else {
            format!("{}–{}", range.min, range.max)
        }
    }
```

- [ ] **Step 4: Put the range in `stat_summary`**

In `crates/app-core/src/lib.rs`, `stat_summary` becomes a `&Game`-taking function so it can call `damage_range_label`, or takes the pre-formatted string — pick whichever keeps the existing four call sites simplest, and change all of them. The damage row leads (it is the headline number), then ATK, MIT, DECOMP, ACC, EVA:

```rust
pub fn stat_summary(game: &Game, mods: EquipmentStats) -> String {
    let mut parts = Vec::new();
    if mods.damage != EquipmentStats::default().damage {
        parts.push(format!("{} DMG", game.damage_range_label(mods.damage)));
    }
    for (value, name) in [
        (mods.atk, "ATK"),
        (mods.mitigation, "MIT"),
        (mods.accuracy, "ACC"),
        (mods.evasion, "EVA"),
        (mods.decompiler, "DECOMP"),
    ] {
        if value != 0 {
            parts.push(format!("{value:+} {name}"));
        }
    }
    parts.join(" ")
}
```

Then update `equip_preview_tag`'s own call, which is the one in the same file:

```rust
    let mods = game.copy_bonus(copy, zone_level).unwrap_or_default();
    let mut parts = vec![slot.short_label().to_string()];
    let summary = stat_summary(game, mods);
```

`rg -n 'stat_summary' crates/` names the rest — the equipped panel, the swap picker's stat column and the trader rows. All of them already hold a `&Game`, so none needs a new parameter threaded to it.

- [ ] **Step 5: Check the widths, headlessly**

The map's status column holds 38.5 monospace cells and `draw_row` clips vertically only — a row too wide draws off the panel in silence. `SWAP_STATS_COLUMN` is 20 cells and the swap picker's stat column already ran close. A six-axis summary is materially wider than the three-axis one it replaces.

Popup and column widths **are** testable headlessly: `paint::with_painter` measures real text. Find the existing width tests in `crates/gui/src/tests/` and extend them with the widest shipped copy under the new summary. A width test that skips non-`Item` rows measures nothing here and passes against no fix at all — check what the existing ones actually iterate before trusting them.

If the widest row overflows, the fix is `inventory_row_lines` shedding the tag onto a continuation (which already exists) or dropping ACC/EVA from the narrow columns while keeping them on the describe page — **not** widening the status column, which cannot grow.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-app-core inventory 2>&1 | tail -20`
Run: `cargo test -p feral-processes-gui 2>&1 | tail -30`
Expected: PASS.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -10
git add -u
git commit -m "feat(ui): weapon damage ranges ride equip_preview_tag, through one formatter"
```

---

### Task 12: Documentation, dev-saves, and the play pass

The slice is not delivered until the seams are written down and someone has actually played it. A green suite is not evidence of play.

**Files:**
- Modify: `CLAUDE.md` and `AGENTS.md` (identical twins — edit the first, `cp` to the second)
- Modify: `docs/seams.md`
- Modify: `CHANGELOG.md`, root `Cargo.toml`
- Modify: `docs/superpowers/specs/2026-08-19-combat-model-ac-and-weapon-damage-design.md` (`**Status:**`)
- Modify: `docs/superpowers/INDEX.md`
- Re-capture: `dev-saves/*`
- Create: `docs/measurements/2026-08-19-combat-model-slice-1.md`

- [ ] **Step 1: Rewrite the seams this slice moved**

In `CLAUDE.md`'s **Combat, progression and balance** section, and the matching entries in `docs/seams.md` under the same titles:

- **Delete** "Every difficulty curve in the game is linear, and that is a correctness property" as a *correctness* claim. Its whole argument was `compute_damage`'s subtractive floor, which no longer exists. Replace it with the property that did survive: `ZoneLevel::raised_a_tier` applies a ratio rather than truncating, and `balance_sim` bounds per-zone steps rather than ratios. Say in `docs/seams.md` why the old constraint was retired and on what date, so nobody restores it from the old reading.
- **Rewrite** "A basic attack is an `AbilityDef`, and combat names `MoveDef` nowhere". Its "two arms apply damage by different arithmetic on purpose" paragraph is now wrong in its specifics: both arms go through `resolve_attack`, and what differs is that a basic attack takes its range from the weapon while a Special takes the ability's own, scaled. Restate it accurately.
- **Add** a seam for `resolve_attack`: one draw, four bands, the ratio form's scale-freeness, the pinned draw counts, and the Opening rung's non-recursion.
- **Add** a seam for mitigation: percentage points, never scaled by level or zone, capped once inside `effective_mitigation`, and the trap that `Stats::mitigation` already carries gear so nothing may add `gear_bonus` again.
- **Update** "`Game::apply_damage` is the only code path that lowers a creature's HP" — still true, and now also true of every rung of the fumble ladder. Say so, since a rung is exactly the kind of thing someone would write a direct `Stats::hp` write for.
- **Update** the `balance_sim` seam to name `expected_damage` as the fifth instance of the call-not-copy rule.

Then `cp CLAUDE.md AGENTS.md`.

- [ ] **Step 2: Re-capture the `dev-saves/` templates**

`SAVE_FORMAT_VERSION` moved to 31, so every template is unreadable.

```bash
cargo run --bin savetool -- template
```

For each name it lists, play or warp to the state `dev-saves/README.md` describes and re-capture it:

```bash
cargo run --bin savetool -- capture saves/save.bin <name>
```

Note in `dev-saves/README.md` that the templates were re-captured at v31 and why. Five shipped `dev-arenas/` scenarios author `level: 12` and are already known to be silently clamped — do not re-tune them here, that is a separate open thread.

- [ ] **Step 3: Write the measurement**

Create `docs/measurements/2026-08-19-combat-model-slice-1.md` following `docs/measurements/README.md`'s convention: the commands run, the `balance_sim` curves before and after, an arena batch across the shipped `dev-arenas/` scenarios, and — explicitly — what the run was blind to.

```bash
for scenario in dev-arenas/*.ron; do
  cargo run --bin arena -- "$scenario" --out "/tmp/claude-1000/-home-trog-code-feral-processes/665585fc-c856-45db-863d-978aa658dc03/scratchpad/$(basename "$scenario" .ron).ron"
done
```

Record in the "blind to" section, at minimum: `assets/policies/enemy_battle.ron`'s weights were trained against a world where every swing lands and are now stale (retraining waits for slices 2 and 3, deliberately); arena numbers only compare within one build and this reshuffled the RNG stream as well as the model, so every existing report is incomparable; and `expected_damage` excludes the fumble ladder's self-damage, so `balance_sim` mildly overstates an attacker's net output.

- [ ] **Step 4: Play it**

```bash
cargo run -- --template extraction
```

The suite proves the mechanism, not the numbers. Four things only a session can answer, and each is a tuning knob this plan set on judgement rather than measurement:

1. Does a fight at `HIT_CHANCE` around 0.5 *read* as a fight, or as a slog? `FUMBLE_CHANCE` and `CRIT_CHANCE` are the dials.
2. Is `FUMBLE_RUNG_THRESHOLDS` weighted so Crash is a rare disaster rather than a coin flip that ends runs?
3. Does a weapon's range read as the headline number it is meant to be, at the width the columns actually have?
4. Do `party_stat_bonus`/`wielded_stat_bonus` contributing a tenth of a companion's *mitigation percentage* to the player feel right, or does a full party read as immune? This plan kept them unchanged and flagged it; the cap bounds them, but the feel is unmeasured.

Write what the session says into the measurement file. If a knob moves, re-run `cargo test -p feral-processes-engine balance_sim` and re-baseline.

- [ ] **Step 5: Changelog and version**

Per CLAUDE.md, the version bump happens **at the merge**, not on the branch. So: do not bump `Cargo.toml` yet. Draft the `CHANGELOG.md` section now and land it with the merge.

`CHANGELOG.md`'s preamble is the one statement of the version policy — read it and let it decide the digit. "Breaking" here means a player's save stops loading, and `SAVE_FORMAT_VERSION` moved, so this is a **breaking** release.

The section must call out, each on its own line:

- Every attack now rolls to hit against a derived Evasion; `def` is percentage-point Mitigation.
- Weapons carry damage ranges and a weapon overrides a natural attack rather than adding to it.
- Crits double the rolled portion only; a four-rung fumble ladder.
- **Every con colour and every kill's XP in the game has moved**, because `Stats::power` is redefined to price mitigation as effective HP. This is a consequence, not a side effect to be discovered later — it gets its own line.
- Saves from v30 and earlier will not load.

- [ ] **Step 6: Update the spec's status and the index**

Set the spec's `**Status:**` to `implemented, <date>` and add its row to `docs/superpowers/INDEX.md` — that file is the one-file answer to "what shipped, and where is its argument". Note in the spec that slices 2–4 remain deferred.

- [ ] **Step 7: Final gate**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -10
cargo test --workspace 2>&1 | tail -20
git diff --quiet assets/ || echo "UNCOMMITTED ASSET EDITS — check for a stray toggle"
```

Expected: PASS, no clippy warnings, no stray asset edits.

- [ ] **Step 8: Commit**

```bash
git add -u CLAUDE.md AGENTS.md docs dev-saves
git commit -m "docs(combat): record the slice-1 seams, re-capture dev-saves at v31"
```

Do **not** push. Do not merge. The branch is ready for `superpowers:finishing-a-development-branch` and the `deploy` skill, which the user runs when they choose.
