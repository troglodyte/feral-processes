# Species Danger Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Gate which species may spawn on the existing danger-step scalar, and make "boss" a per-individual roll available to every species instead of a property of two hand-authored ones.

**Architecture:** A species' rung is derived from its `growth_multiplier` (`SpeciesDef::danger_band`), and each rung is eligible only within a window of `Game::danger_steps` — zone on the surface, depth underground. `Game::habitat_pools` applies the window and hands the result to the four things that draw a species. Boss-hood splits into an apex marker on the species (`SpeciesDef::is_boss`, unchanged in meaning for the two that carry it) and a per-entity `components::Boss` written at spawn; `Game::is_boss_creature` is the one door both go through.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-08-18-species-danger-window-design.md`

## Global Constraints

- **Engine crate only.** `crates/gui` and `crates/app-core` must not need an edit. The renderer reads `views::EntityView::is_boss`, so the boss tag and the magenta glyph follow for free. If you find yourself editing gui, stop and report it.
- **No new `.ron` schema field.** The band is derived. `assets/species/*.ron` are not edited by this plan.
- **No `SAVE_FORMAT_VERSION` bump.** `CreatureSave::boss` is additive behind `#[serde(default)]`, which under field-named RON costs no bump.
- **New tuning values go in `crates/engine/src/tuning.rs`** as documented `pub const`, never inline in a formula. Exact values: `TIER_ENTRY_STEPS = 2`, `TIER_WINDOW_STEPS = 3`, `APEX_ENTRY_STEP = 4`, `BOSS_STAT_MULT = 1.75`.
- **`Game::is_boss_creature` is the one door** from an entity to boss-hood. After Task 3, no code outside `species.rs` may read `SpeciesDef::is_boss` to answer "is this creature a boss" — only to answer "is this species an apex species".
- **RNG stream discipline.** The boss roll must keep spending exactly one `random_bool` draw in exactly the positions it does today (see Task 5). Pool *contents* change, which will move seeded spawn tests — that is expected. Never "fix" a moved seeded test by changing its seed; find what it was incidentally coupled to.
- **Run `cargo fmt` and `cargo clippy --workspace` after every task**; fix warnings rather than silencing them.
- Commit messages end with:
  `Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>`
- **Do not push.** Landing is the user's call.

## Deviation from the spec, deliberate

The spec says `SpeciesDb::boss_habitat_matches` and the `boss_candidates` half of `habitat_pools` are deleted. This plan **keeps both**, windowed, and adds `windowed_matches` / `windowed_boss_matches` on top of them as the primitives. Reason: `habitat_pools`' two-pool shape is what lets `pick_habitat_species` keep its exact RNG draw positions (Task 5), and `boss_habitat_matches` is already asserted on by a census. Union-ing the two pools happens at the draw, in `pick_habitat_species`, which is where the spec's "drawn from the windowed pool like anything else" actually needs to be true. Report this to the user at the end; it is a smaller diff for the same behaviour, not a change of intent.

## File Structure

| File | Responsibility after this plan |
|---|---|
| `crates/engine/src/species.rs` | `DangerBand`, `SpeciesDef::danger_band`, window predicates, `SpeciesDb::windowed_matches` / `windowed_boss_matches` |
| `crates/engine/src/tuning.rs` | the four constants |
| `crates/engine/src/components.rs` | `Boss` marker |
| `crates/engine/src/game/party.rs` | `is_boss_creature` — the one door |
| `crates/engine/src/game/spawning.rs` | window applied at `habitat_pools`; boss flag threaded through `spawn_pack` → `spawn_group` → `spawn_wild_creature_scaled`; `BOSS_STAT_MULT`; `roll_rarity` refusal |
| `crates/engine/src/game/combat_rewards.rs` | payout gate through the door |
| `crates/engine/src/game/inspection.rs` | view builder through the door |
| `crates/engine/src/game/stack_features.rs` | `pick_lair_species` draws from the window and is always a boss; `orphan_species` threads depth |
| `crates/engine/src/game/stack.rs`, `game/turn.rs`, `arena/encounter.rs`, `arena/setup.rs` | thread the new parameters |
| `crates/engine/src/save.rs`, `game/lifecycle.rs` | `CreatureSave::boss` written and restored |
| `crates/engine/src/tests/spawning.rs`, `tests/stack.rs` | behavioural tests |
| `assets/species/README.md`, `CLAUDE.md`, `AGENTS.md`, `docs/seams.md`, `CHANGELOG.md` | docs |

---

### Task 1: The band is derived from the growth multiplier

Pure addition. Nothing calls it yet, so the suite must be green with no other change.

**Files:**
- Modify: `crates/engine/src/species.rs` (add `DangerBand` near `GROWTH_TIERS` at line ~413; add `danger_band` to the `impl SpeciesDef` block that starts at line 294)
- Test: `crates/engine/src/species.rs`, inside the existing `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `GROWTH_TIERS: [f32; 3]`, `SpeciesDef::growth_multiplier`, `SpeciesDef::is_boss`
- Produces: `pub enum DangerBand { Tier(usize), Apex }`, `pub fn SpeciesDef::danger_band(&self) -> DangerBand`

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `crates/engine/src/species.rs`:

```rust
#[test]
fn the_shipped_roster_fills_three_bands_and_an_apex() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
    let mut counts = [0usize; 4];
    for s in db.all() {
        match s.danger_band() {
            DangerBand::Tier(i) => counts[i] += 1,
            DangerBand::Apex => counts[3] += 1,
        }
    }
    assert_eq!(
        counts,
        [5, 5, 5, 2],
        "the ladder is five species a band and two apex; a roster that has \
         drifted off that is a content change, not a test failure"
    );
}

/// A boss is apex whatever it grows at. The two shipped ones sit at 2.0,
/// which is off the ladder entirely — reading the multiplier first would
/// snap them onto band 2 beside the ordinary hard species.
#[test]
fn is_boss_decides_the_band_before_the_multiplier_does() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
    for s in db.all().filter(|s| s.is_boss) {
        assert_eq!(s.danger_band(), DangerBand::Apex, "{} is apex", s.id);
    }
}

/// A modded multiplier between rungs snaps to the nearest, on the same
/// midpoints `tier_budget` already splits on — 1.125 and 1.375. A mod is
/// never refused for it; it just stops being readable against the ladder.
#[test]
fn an_off_ladder_multiplier_snaps_to_the_nearest_band() {
    let cases = [
        (1.0, 0usize),
        (1.1, 0),
        (1.2, 1),
        (1.25, 1),
        (1.3, 1),
        (1.45, 2),
        (1.5, 2),
        (9.0, 2),
    ];
    for (growth, band) in cases {
        let mut def = generic_species();
        def.is_boss = false;
        def.growth_multiplier = growth;
        assert_eq!(
            def.danger_band(),
            DangerBand::Tier(band),
            "growth {growth} should read as band {band}"
        );
    }
}
```

If `generic_species()` is not already in scope in this module, build the `SpeciesDef` the way the nearest existing test in this file does — do not add a new fixture helper.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine species::tests::the_shipped_roster_fills_three_bands`
Expected: FAIL to compile — `DangerBand` and `danger_band` do not exist.

- [ ] **Step 3: Implement**

Add beside `GROWTH_TIERS` in `crates/engine/src/species.rs`:

```rust
/// Which rung of the difficulty ladder a species stands on, and therefore
/// which danger steps it may spawn at.
///
/// Derived from `growth_multiplier` rather than authored, for the reason
/// `affinity_class` is derived from `affinities`: a species' rung is a fact
/// about numbers it already carries, and a second authored field is a second
/// thing that can disagree with the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DangerBand {
    /// A rung of `GROWTH_TIERS`, by index.
    Tier(usize),
    /// A boss species. Outside the ladder, as it is outside the class system.
    Apex,
}

impl SpeciesDef {
    /// This species' rung. `Apex` for a boss whatever it grows at — the two
    /// shipped ones sit at 2.0, off the ladder's top, so reading the
    /// multiplier first would file them beside the ordinary hard species.
    ///
    /// A multiplier between rungs snaps to the nearest. That is the same
    /// concession `tier_budget` makes on the same midpoints, and the same one
    /// `assets/species/README.md` already documents about the stat budget
    /// being a step function: a mod is never refused, it just stops being
    /// readable against the shipped ladder.
    pub fn danger_band(&self) -> DangerBand {
        if self.is_boss {
            return DangerBand::Apex;
        }
        let mut best = 0;
        let mut best_gap = f32::INFINITY;
        for (i, rung) in GROWTH_TIERS.iter().enumerate() {
            let gap = (self.growth_multiplier - rung).abs();
            if gap < best_gap {
                best_gap = gap;
                best = i;
            }
        }
        DangerBand::Tier(best)
    }
}
```

Put the `impl SpeciesDef` block beside the existing one rather than merging into it only if that reads better next to `GROWTH_TIERS`; either placement is fine.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine species::`
Expected: PASS, and every existing species test still passes.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -5
git add crates/engine/src/species.rs
git commit -m "feat(species): derive a danger band from the growth multiplier

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: The window, and the fallback that keeps every biome populated

Still pure addition — no spawner reads it yet.

**Files:**
- Modify: `crates/engine/src/tuning.rs` (new constants, in the section near `GROUP_SIZE_STEP_ZONES` at line ~271)
- Modify: `crates/engine/src/species.rs` (window predicates on `DangerBand`; `windowed_matches` / `windowed_boss_matches` on `SpeciesDb`, beside `habitat_matches` at line ~829)
- Test: `crates/engine/src/species.rs` `mod tests`

**Interfaces:**
- Consumes: `DangerBand`, `SpeciesDb::sorted_matches`, `SpeciesDb::habitat_matches`
- Produces:
  - `tuning::TIER_ENTRY_STEPS: u32`, `tuning::TIER_WINDOW_STEPS: u32`, `tuning::APEX_ENTRY_STEP: u32`
  - `DangerBand::entry_step(self) -> u32`
  - `DangerBand::exit_step(self) -> Option<u32>` — `None` means "never exits"
  - `DangerBand::live_at(self, step: u32) -> bool`
  - `SpeciesDb::windowed_matches(&self, biome: Biome, step: u32) -> Vec<&SpeciesDef>` — ordinary species only, with the fallback
  - `SpeciesDb::windowed_boss_matches(&self, biome: Biome, step: u32) -> Vec<&SpeciesDef>` — apex only, no fallback

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/engine/src/species.rs`:

```rust
/// The schedule the design is written against. Spelled out as a table
/// rather than recomputed from the constants, so moving a constant fails
/// here loudly instead of silently redefining what a zone means.
#[test]
fn the_window_schedule_matches_the_design() {
    let live = |step: u32| {
        let mut bands = Vec::new();
        for i in 0..GROWTH_TIERS.len() {
            if DangerBand::Tier(i).live_at(step) {
                bands.push(i as i32);
            }
        }
        if DangerBand::Apex.live_at(step) {
            bands.push(-1);
        }
        bands
    };
    assert_eq!(live(0), vec![0]);
    assert_eq!(live(1), vec![0]);
    assert_eq!(live(2), vec![0, 1]);
    assert_eq!(live(3), vec![0, 1]);
    assert_eq!(live(4), vec![1, 2, -1]);
    assert_eq!(live(5), vec![1, 2, -1]);
    assert_eq!(live(6), vec![2, -1]);
    assert_eq!(live(7), vec![2, -1]);
}

/// Steps are unbounded because zones and depth are, so a closed top band
/// empties the world past step 7. Both the top rung and apex stay open.
#[test]
fn the_top_band_and_apex_never_exit() {
    for step in [8u32, 40, 4_000] {
        assert!(DangerBand::Tier(GROWTH_TIERS.len() - 1).live_at(step));
        assert!(DangerBand::Apex.live_at(step));
        assert!(!DangerBand::Tier(0).live_at(step));
    }
}

/// StaticField ships no band-0 species and OpenGrid no band-2 species, so
/// the fallback is load-bearing against the real assets rather than
/// defensive. Asserted as a census over every biome and every step a run
/// can reach.
#[test]
fn every_biome_fields_something_at_every_danger_step() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
    for biome in [
        Biome::Mainframe,
        Biome::OpenGrid,
        Biome::NullSector,
        Biome::StaticField,
    ] {
        for step in 0..=crate::tuning::MAX_GROUP_SIZE_STEPS {
            assert!(
                !db.windowed_matches(biome, step).is_empty(),
                "{biome:?} fields nothing at step {step}; the window has \
                 emptied a biome the fallback was supposed to cover"
            );
        }
    }
}

/// The two known fallback sites, pinned by name so a content change that
/// closes one is visible rather than silent.
#[test]
fn the_fallback_fires_where_the_roster_has_a_hole() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();

    // StaticField has no band-0 species: step 0 falls upward to band 1.
    let early = db.windowed_matches(Biome::StaticField, 0);
    assert!(
        early.iter().all(|s| s.danger_band() == DangerBand::Tier(1)),
        "StaticField at step 0 should fall back to band 1, got {:?}",
        early.iter().map(|s| &s.id).collect::<Vec<_>>()
    );

    // OpenGrid has no band-2 species: a deep step falls back to band 1.
    let deep = db.windowed_matches(Biome::OpenGrid, 7);
    assert!(
        deep.iter().all(|s| s.danger_band() == DangerBand::Tier(1)),
        "OpenGrid at step 7 should fall back to band 1, got {:?}",
        deep.iter().map(|s| &s.id).collect::<Vec<_>>()
    );
}

/// Apex is a rare outcome the window admits, never a biome's last resort:
/// a boss must not be handed to a step the ladder has nothing else for.
#[test]
fn the_fallback_never_reaches_for_an_apex_species() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
    for biome in [
        Biome::Mainframe,
        Biome::OpenGrid,
        Biome::NullSector,
        Biome::StaticField,
    ] {
        for step in 0..=crate::tuning::MAX_GROUP_SIZE_STEPS {
            assert!(
                db.windowed_matches(biome, step).iter().all(|s| !s.is_boss),
                "windowed_matches leaked an apex species into {biome:?} at step {step}"
            );
        }
    }
}

/// Apex enters at its own step and is empty before it, which is what stops
/// a fresh run meeting Wintermute in zone 1.
#[test]
fn apex_species_are_absent_before_their_entry_step() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
    for step in 0..crate::tuning::APEX_ENTRY_STEP {
        assert!(
            db.windowed_boss_matches(Biome::Mainframe, step).is_empty(),
            "an apex species is eligible at step {step}, below APEX_ENTRY_STEP"
        );
    }
    assert!(
        !db.windowed_boss_matches(Biome::Mainframe, crate::tuning::APEX_ENTRY_STEP)
            .is_empty(),
        "no apex species is eligible at APEX_ENTRY_STEP, so bosses never arrive"
    );
}

/// The draw picks out of these pools by index, so an unsorted order makes
/// two `Game::new(seed)` runs diverge — the same reason `habitat_matches`
/// sorts.
#[test]
fn windowed_pools_are_sorted_by_id() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
    for step in 0..=crate::tuning::MAX_GROUP_SIZE_STEPS {
        let ids: Vec<&String> = db
            .windowed_matches(Biome::Mainframe, step)
            .iter()
            .map(|s| &s.id)
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "unsorted pool at step {step}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine species::tests::the_window_schedule`
Expected: FAIL to compile — the constants and methods do not exist.

- [ ] **Step 3: Add the constants**

In `crates/engine/src/tuning.rs`, immediately after `GROUP_SIZE_STEP_ZONES`:

```rust
/// Danger steps between one growth band entering the spawn pool and the
/// next, and how many steps a band stays in it once it has — the window
/// that decides which species a zone or a Stack depth may field.
///
/// Read against `Game::danger_steps`, the same scalar the two group-size
/// curves take, so there is no second difficulty axis to keep in step with
/// the first. Band `b` is live from `b * TIER_ENTRY_STEPS` through
/// `b * TIER_ENTRY_STEPS + TIER_WINDOW_STEPS` **inclusive**.
///
/// The top band never exits, whatever these say. Steps are unbounded
/// because zones and depth are, so a closed top empties the world.
pub const TIER_ENTRY_STEPS: u32 = 2;
pub const TIER_WINDOW_STEPS: u32 = 3;

/// The step a boss species (`SpeciesDef::is_boss`) becomes eligible at, and
/// it never exits either.
///
/// Apex is outside the growth ladder, so its entry is a constant rather than
/// a fourth rung of `TIER_ENTRY_STEPS`. Before this step a boss roll still
/// fires — it just draws an ordinary species and marks it, which is the
/// whole of "easy bosses early, hard bosses deep".
pub const APEX_ENTRY_STEP: u32 = 4;
```

- [ ] **Step 4: Implement the window predicates**

In `crates/engine/src/species.rs`, beside `DangerBand`:

```rust
impl DangerBand {
    /// The first danger step this band may spawn at.
    pub fn entry_step(self) -> u32 {
        match self {
            DangerBand::Tier(i) => i as u32 * crate::tuning::TIER_ENTRY_STEPS,
            DangerBand::Apex => crate::tuning::APEX_ENTRY_STEP,
        }
    }

    /// The last step this band may spawn at, or `None` for a band that never
    /// leaves. The top rung and apex both never leave: steps are unbounded
    /// because zones and depth are, so a closed top empties the world.
    pub fn exit_step(self) -> Option<u32> {
        match self {
            DangerBand::Apex => None,
            DangerBand::Tier(i) if i + 1 >= GROWTH_TIERS.len() => None,
            DangerBand::Tier(i) => {
                Some(i as u32 * crate::tuning::TIER_ENTRY_STEPS + crate::tuning::TIER_WINDOW_STEPS)
            }
        }
    }

    pub fn live_at(self, step: u32) -> bool {
        step >= self.entry_step() && self.exit_step().is_none_or(|last| step <= last)
    }

    /// How far `step` sits outside this band's window, zero inside it. The
    /// fallback's ranking key — never a difficulty number.
    fn window_distance(self, step: u32) -> u32 {
        let entry = self.entry_step();
        if step < entry {
            return entry - step;
        }
        match self.exit_step() {
            Some(last) if step > last => step - last,
            _ => 0,
        }
    }

    /// Rung index for tie-breaking; apex ranks above every tier.
    fn rank(self) -> usize {
        match self {
            DangerBand::Tier(i) => i,
            DangerBand::Apex => GROWTH_TIERS.len(),
        }
    }
}
```

- [ ] **Step 5: Implement the windowed pools**

In `crates/engine/src/species.rs`, beside `boss_habitat_matches` (line ~836):

```rust
/// The ordinary species `biome` may field at danger `step` — the pool the
/// per-tile spawn roll draws from, after the window and before any draw.
///
/// Never empty for a biome that has any ordinary species at all. Where the
/// window admits nothing the biome holds, this falls back to the band
/// **nearest** the window, ties resolving upward. That fallback is
/// load-bearing rather than defensive: StaticField ships no band-0 species
/// and OpenGrid no band-2 species, so it fires against the real assets at
/// both ends. `every_biome_fields_something_at_every_danger_step` is the
/// census; the honest fix for either hole is a species file, not a wider
/// window.
///
/// Apex is never a fallback. A boss is a rare outcome the window admits,
/// not a biome's last resort — see `windowed_boss_matches`.
pub fn windowed_matches(&self, biome: Biome, step: u32) -> Vec<&SpeciesDef> {
    let ordinary = self.habitat_matches(biome);
    let live: Vec<&SpeciesDef> = ordinary
        .iter()
        .copied()
        .filter(|s| s.danger_band().live_at(step))
        .collect();
    if !live.is_empty() {
        return live;
    }
    let key = |s: &SpeciesDef| {
        let band = s.danger_band();
        (
            band.window_distance(step),
            std::cmp::Reverse(band.rank()),
        )
    };
    let Some(best) = ordinary.iter().map(|s| key(s)).min() else {
        return Vec::new();
    };
    ordinary.into_iter().filter(|s| key(s) == best).collect()
}

/// The apex species `biome` may field at danger `step`. No fallback: below
/// `APEX_ENTRY_STEP` this is empty, which is what stops a fresh run meeting
/// a hand-authored boss in zone 1.
pub fn windowed_boss_matches(&self, biome: Biome, step: u32) -> Vec<&SpeciesDef> {
    self.boss_habitat_matches(biome)
        .into_iter()
        .filter(|s| s.danger_band().live_at(step))
        .collect()
}
```

Both build on the sorted primitives, so both come back sorted by `id`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine species::`
Expected: PASS, all of them.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -5
git add crates/engine/src/species.rs crates/engine/src/tuning.rs
git commit -m "feat(species): a danger window per growth band, with a per-biome fallback

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Boss becomes a per-entity fact

The component, the one door, the two direct readers, and the save. No spawner change yet — tests insert the component by hand, which is exactly what makes this task independently reviewable.

**Files:**
- Modify: `crates/engine/src/components.rs` (new `Boss` marker)
- Modify: `crates/engine/src/game/party.rs:178` (`is_boss_creature`)
- Modify: `crates/engine/src/game/combat_rewards.rs:438` (payout gate)
- Modify: `crates/engine/src/game/inspection.rs:822` (`is_boss: species.is_boss`)
- Modify: `crates/engine/src/save.rs` (`CreatureSave::boss`, beside `rarity` at line ~236)
- Modify: `crates/engine/src/game/lifecycle.rs` (the creature save query at ~880 and the load spawn at ~580)
- Test: `crates/engine/src/tests/spawning.rs`

**Interfaces:**
- Consumes: `Game::is_boss_creature(&self, Entity) -> bool` (existing)
- Produces: `components::Boss` (unit struct, `#[derive(Component, ...)]`); `save::CreatureSave::boss: bool`

- [ ] **Step 1: Write the failing tests**

Three go in `crates/engine/src/tests/spawning.rs` (which already has `use super::support::*;` and `use crate::*;`) and one in `crates/engine/src/tests/combat_rewards.rs` (which has `corpse_of` and `stand_in_the_stack`).

In `crates/engine/src/tests/spawning.rs`:

```rust
/// The one door. A creature carrying `Boss` is a boss even though its
/// species is not, and an apex species is a boss even without the component
/// — a fixture that hand-spawns one outside `spawn_pack` never gets one.
#[test]
fn is_boss_creature_reads_the_component_or_the_species() {
    let mut game = Game::new(4101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let wild = spawn_wild_on_player_tile(&mut game);
    assert!(
        !game.is_boss_creature(wild),
        "an ordinary species with no component is not a boss"
    );
    game.world.entity_mut(wild).insert(Boss);
    assert!(
        game.is_boss_creature(wild),
        "the component alone must make a creature a boss"
    );

    // The species half still has to answer: this fixture spawns outside the
    // boss path, so no component is written.
    let apex = spawn_boss_on_player_tile(&mut game);
    assert!(
        game.world.get::<Boss>(apex).is_none(),
        "this fixture spawns outside the boss path, so the component is the \
         wrong thing to be asserting on"
    );
    assert!(
        game.is_boss_creature(apex),
        "an apex species is a boss without a component"
    );
}

/// The receipt must survive a reload, or a boss killed after a save/load
/// pays nothing and reads as the drop rate having moved. A RON round-trip
/// cannot catch a load path that drops the component — this has to go
/// through `Game::save` and `Game::load`.
#[test]
fn a_rolled_boss_keeps_its_component_across_a_save_and_load() {
    let mut game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    let species = game.world.get::<Creature>(wild).unwrap().species.clone();
    let pos = *game.world.get::<Position>(wild).unwrap();
    game.world.entity_mut(wild).insert(Boss);

    let path = std::env::temp_dir().join(format!(
        "feral_rolled_boss_save_{}.sav",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let mut q = loaded.world.query::<(Entity, &Creature, &Position)>();
    let found: Vec<Entity> = q
        .iter(&loaded.world)
        .filter(|(_, c, p)| c.species == species && p.x == pos.x && p.y == pos.y)
        .map(|(e, _, _)| e)
        .collect();
    assert_eq!(found.len(), 1, "exactly one creature should match the saved one");
    assert!(
        loaded.is_boss_creature(found[0]),
        "a rolled boss must come back a boss — the load path dropped `Boss`"
    );
}

/// The field is additive behind `#[serde(default)]`, which is what buys this
/// change no `SAVE_FORMAT_VERSION` bump: a file written before rolled bosses
/// existed must load rather than be refused. Mirrors
/// `nemesis::a_save_without_the_grudge_field_loads`.
#[test]
fn a_save_without_the_boss_field_loads_un_bossed() {
    let mut game = Game::new(4103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.entity_mut(wild).insert(Boss);

    let path = std::env::temp_dir().join(format!(
        "feral_boss_default_save_{}.sav",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("boss: true"),
        "a fresh save must carry the field, or stripping it below proves nothing"
    );
    let stripped: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("boss:"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, stripped).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut q = loaded.world.query::<(Entity, &Creature)>();
    let creatures: Vec<Entity> = q.iter(&loaded.world).map(|(e, _)| e).collect();
    assert!(
        creatures
            .iter()
            .all(|&e| loaded.world.get::<Boss>(e).is_none()),
        "a file with the field stripped must load with nothing bossed"
    );
}
```

In `crates/engine/src/tests/combat_rewards.rs`:

```rust
/// The payout gate used to read `SpeciesDef::is_boss` directly, so a rolled
/// boss would have died underground paying nothing. Asserted on a species
/// that is deliberately **not** apex.
#[test]
fn a_rolled_boss_pays_the_stack_boss_cache() {
    let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    stand_in_the_stack(&mut game, 1);

    let wild = corpse_of(&mut game, &ordinary.id);
    game.world.entity_mut(wild).insert(Boss);
    game.award_loot(wild);

    let qty = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::PORTAL_FRAGMENT));
    assert!(
        STACK_BOSS_PORTAL_FRAGMENT_DROP.contains(&qty),
        "a rolled boss killed at depth 1 should pay a cache in \
         {STACK_BOSS_PORTAL_FRAGMENT_DROP:?}, got {qty}"
    );
}
```

`Boss` has to be reachable unqualified in these files. `lib.rs` carries a private `use components::{...}` list at line ~51 that the test modules pick up through `use crate::*;` — add `Boss` to it.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine is_boss_creature_reads_the_component`
Expected: FAIL to compile — `components::Boss` does not exist.

- [ ] **Step 3: Add the component**

In `crates/engine/src/components.rs`:

```rust
/// A creature that spawned as a boss.
///
/// Two things are bosses and only one of them is a species. An **apex**
/// species (`SpeciesDef::is_boss`) is always one and is hand-authored tough;
/// any other species can be **rolled** into one at `BOSS_SPAWN_CHANCE` and is
/// scaled by `tuning::BOSS_STAT_MULT` instead. This component is written at
/// both, so a query can ask without reaching for the db.
///
/// `Game::is_boss_creature` is still the one door, and it keeps the species
/// fallback: a fixture that hand-spawns an apex species outside `spawn_pack`
/// never gets a component, and must still be a boss.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Boss;
```

- [ ] **Step 4: Widen the one door**

`crates/engine/src/game/party.rs`, replacing the body of `is_boss_creature` (keep the existing doc comment and extend it to name the component):

```rust
pub(crate) fn is_boss_creature(&self, entity: Entity) -> bool {
    if self.world.get::<crate::components::Boss>(entity).is_some() {
        return true;
    }
    self.world
        .get::<Creature>(entity)
        .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
        .is_some_and(|s| s.is_boss)
}
```

- [ ] **Step 5: Route the two direct readers through it**

`crates/engine/src/game/combat_rewards.rs`, line ~438 — the gate currently reads `if species.is_boss {`. The entity being awarded for is `wild` (the same value passed to `mark_lair_cleared` a few lines above). Replace with:

```rust
if self.is_boss_creature(wild) {
```

Check the borrow: `species` is used inside that block for `species_id`. If the borrow checker objects, hoist the boss answer above the `species` borrow into a `let is_boss = self.is_boss_creature(wild);` and branch on that — small focused functions and hoisted lets are the fix here, never a `.clone()` of the db.

`crates/engine/src/game/inspection.rs`, line 822 — replace `is_boss: species.is_boss,` with:

```rust
is_boss: self.is_boss_creature(entity),
```

- [ ] **Step 6: Save and restore it**

`crates/engine/src/save.rs`, beside `rarity`:

```rust
/// Whether this creature spawned as a boss — see `components::Boss`.
///
/// Written for an apex species too, redundantly with its own `is_boss`
/// flag, so the field means one thing rather than "the rolled half".
///
/// Additive, named and defaulted, so this needs **no**
/// `SAVE_FORMAT_VERSION` bump — the save has been field-named RON since
/// v29, which is what retired migrations for exactly this shape of change.
/// A save written before rolled bosses existed loads with every creature
/// un-bossed, which is what it was.
#[serde(default)]
pub boss: bool,
```

`crates/engine/src/game/lifecycle.rs`:

- In the creature save query (~line 880), add `Option<&Boss>` to the inner nested tuple beside `pursuing`, and write `boss: boss.is_some(),` in the `CreatureSave` literal beside `pursuing: pursuing.is_some(),`.
- In the load path (~line 595, beside the `nemesis_grudges` insert), add:

```rust
// Inserted only when set, so an absent component keeps meaning "not a
// boss" — `is_boss_creature`'s species fallback still answers for an
// apex species loaded from a file written before this field existed.
// `Stats` above already carry `BOSS_STAT_MULT` from the spawn that rolled
// it; nothing here may re-apply it, the same trap `c.rarity` documents.
if c.boss {
    entity.insert(Boss);
}
```

Every other construction site of `CreatureSave` in the codebase needs `boss: false` — find them with `rg -n "CreatureSave *{" --type rust`.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p feral-processes-engine` then `cargo test --workspace`
Expected: PASS. Nothing here changes what spawns, so no seeded test should move. If one does, stop and report it rather than adjusting a seed.

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -5
git add -u crates/engine/src
git commit -m "feat(combat): boss becomes a per-entity fact behind one door

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: A rolled boss is scaled, alone, and rolls no rare tier

Threads the flag from `spawn_pack` down to the spawn. Behaviour on shipped assets is unchanged except that an apex spawn now also carries the component — which is harmless, and which the Task 3 tests already cover.

**Files:**
- Modify: `crates/engine/src/tuning.rs` (`BOSS_STAT_MULT`)
- Modify: `crates/engine/src/game/spawning.rs` — `spawn_wild_creature_scaled` (line 204), `roll_rarity` (line 462), `spawn_group` (line 1055), `spawn_pack` (line 959)
- Modify: `crates/engine/src/arena/setup.rs:138` (call site)
- Test: `crates/engine/src/tests/spawning.rs`

**Interfaces:**
- Consumes: `components::Boss` (Task 3), `Game::is_boss_creature` (Task 3)
- Produces:
  - `tuning::BOSS_STAT_MULT: f32`
  - `Game::spawn_wild_creature_scaled(&mut self, species_id: &str, x: i32, y: i32, depth_mult: f32, boss: bool) -> Option<Entity>`
  - `Game::roll_rarity(&mut self, species: &SpeciesDef, x: i32, y: i32, boss: bool) -> Rarity`
  - `Game::spawn_group(&mut self, species_id: &str, size: u32, x: i32, y: i32, esc: SpawnEscalation, boss: bool) -> Vec<Entity>`
  - `Game::spawn_pack` keeps its existing signature

- [ ] **Step 1: Write the failing tests**

All three go in `crates/engine/src/tests/spawning.rs`. Add `use crate::game::spawning::SpawnEscalation;` and `MAX_INDIVIDUAL_ROLL`, `BOSS_SPAWN_CHANCE` to the file's `crate::tuning::{...}` import list as each is needed.

```rust
/// An apex species is authored tough and must not be scaled on top of that;
/// an ordinary species rolled into a boss has nothing but the multiplier.
///
/// Asserted against the *ceiling* of an unbossed roll rather than against a
/// paired spawn, because `roll_potential` gives every spawn an independent
/// ±20% and nothing in the fixture can pin it. `MAX_INDIVIDUAL_ROLL` is 1.2
/// and `BOSS_STAT_MULT * MIN_INDIVIDUAL_ROLL` is 1.4, so the two bands do not
/// overlap and the comparison is exact rather than probabilistic.
#[test]
fn a_rolled_boss_is_scaled_and_an_apex_boss_is_not() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let zone_mult = game.world.resource::<ZoneLevel>().stat_multiplier() as f32;

    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    let plain_ceiling = (ordinary.base_hp as f32 * zone_mult * MAX_INDIVIDUAL_ROLL).round() as i32;
    let bossed = game
        .spawn_wild_creature_scaled(&ordinary.id, pos.x + 3, pos.y + 3, 1.0, true)
        .expect("a shipped species should spawn");
    assert!(
        game.world.get::<Stats>(bossed).unwrap().max_hp > plain_ceiling,
        "a rolled boss must out-scale the luckiest ordinary roll of its own species"
    );

    let apex = game
        .species_defs()
        .into_iter()
        .find(|s| s.is_boss)
        .expect("at least one apex species ships");
    let apex_ceiling = (apex.base_hp as f32 * zone_mult * MAX_INDIVIDUAL_ROLL).round() as i32;
    let apex_spawn = game
        .spawn_wild_creature_scaled(&apex.id, pos.x + 4, pos.y + 4, 1.0, true)
        .expect("a shipped apex species should spawn");
    assert!(
        game.world.get::<Stats>(apex_spawn).unwrap().max_hp <= apex_ceiling,
        "an apex species must not take BOSS_STAT_MULT on top of its authored stats"
    );
}

/// A boss's stats are the whole of what it is worth, and a rare tier on top
/// would be a second, invisible multiplier — the same reason an apex spawn
/// has always been excluded. Spawned well outside the opening ring, or the
/// ring's own exclusion would be what makes this pass.
#[test]
fn a_rolled_boss_never_rolls_a_rare_tier() {
    let mut game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    let far = OPENING_RING_TILES * 4;
    for i in 0..200 {
        let spawned = game
            .spawn_wild_creature_scaled(&ordinary.id, pos.x + far + i, pos.y + far, 1.0, true)
            .expect("a shipped species should spawn");
        assert_eq!(
            *game.world.get::<Rarity>(spawned).unwrap(),
            Rarity::Ordinary,
            "a rolled boss must never carry a rare tier"
        );
    }
}

/// A boss is one group; the escort standing with it is a second, and is
/// never itself a boss. Zone 1 has room for only one group, so this run
/// usually places the boss alone — the assertion is written as "exactly one
/// of whatever spawned" so it holds either way.
#[test]
fn a_boss_pack_marks_the_boss_and_not_its_escort() {
    let mut game = Game::new(4203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let ordinary = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss)
        .expect("the shipped roster is not all bosses");
    let pack = game.spawn_pack(
        &ordinary.id,
        true,
        pos.x + OPENING_RING_TILES * 4,
        pos.y,
        SpawnEscalation::surface(),
    );
    assert!(!pack.is_empty(), "a boss pack should place at least the boss");
    assert_eq!(
        pack.iter().filter(|&&e| game.is_boss_creature(e)).count(),
        1,
        "exactly one member of a boss pack is the boss"
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine a_rolled_boss_is_scaled`
Expected: FAIL to compile — the parameters do not exist.

- [ ] **Step 3: Add the constant**

In `crates/engine/src/tuning.rs`, beside `BOSS_SPAWN_CHANCE`:

```rust
/// Multiplier on every stat of an **ordinary** species rolled into a boss.
/// An apex species (`SpeciesDef::is_boss`) never takes it — its stats are
/// hand-authored, and a blanket multiplier would discard the authoring, the
/// same reason it rolls no rare tier.
///
/// Calibrated against the ladder rather than picked: apex totals are 206 and
/// 236 against a band-2 median of 140, so ~1.5x is "one band up". 1.75 puts a
/// rolled boss above an Overclocked spawn (`GOLD_STAT_MULT`), which is what
/// makes it read as a wall rather than as a shiny — and a boss rolls no rare
/// tier on top, so this is the whole of its elevation.
///
/// **Ungated by `balance_sim`**, which models no bosses at all: see
/// `toughest_ordinary_species`, which excludes them. `dev-arenas/` is the
/// instrument for this number.
pub const BOSS_STAT_MULT: f32 = 1.75;
```

- [ ] **Step 4: Thread the flag**

`spawn_wild_creature_scaled` — add `boss: bool` as the last parameter. Inside, after the species lookup:

```rust
// An apex species is authored tough; only a rolled boss takes the
// multiplier. The component goes on both, so a query need not ask which.
let boss_mult = if boss && !species.is_boss {
    crate::tuning::BOSS_STAT_MULT
} else {
    1.0
};
```

Fold `boss_mult` into the `scale` closure beside `depth_mult`, pass `boss` to `roll_rarity`, and insert the component after the spawn:

```rust
let entity = self.world.spawn(( /* unchanged tuple */ )).id();
if boss {
    self.world.entity_mut(entity).insert(Boss);
}
Some(entity)
```

Keep the component out of the spawn tuple — the tuple is already near bevy's arity limit and a conditional insert is what the load path does for `Nemesis`.

`spawn_wild_creature` (line 188) passes `false`.

`roll_rarity` — add `boss: bool` and refuse on it:

```rust
if boss || species.is_boss || self.in_opening_ring(x, y) {
    return Rarity::Ordinary;
}
```

Extend its doc comment: the carve-out is now "a boss, rolled or apex", and say why — a rolled boss's `BOSS_STAT_MULT` is the whole of what it is worth, and a rare tier would be a second invisible multiplier on top.

`spawn_group` — add `boss: bool` as the last parameter and pass it through to each `spawn_wild_creature_scaled`.

`spawn_pack` — the ordinary branch passes `false`; the boss branch passes `true` for `spawn_group(species_id, 1, ...)` and `false` for the escort's `spawn_group(&escort, size, ...)`.

`crates/engine/src/game/spawning.rs:315` (`adopt_program`'s spawn) and `crates/engine/src/game/spawning.rs:384` (`spawn_nest_guardian`) pass `false`. `crates/engine/src/arena/setup.rs:138` passes `false`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p feral-processes-engine spawning::` then `cargo test --workspace`
Expected: PASS. No seeded test should move — `roll_rarity` still spends the same draws in the same order, and nothing has changed which species is picked.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -5
git add -u crates/engine/src
git commit -m "feat(spawning): a rolled boss is scaled, alone, and rolls no rare tier

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: The window reaches the spawner

The behaviour-changing task. Seeded spawn tests will move here and nowhere else.

**Files:**
- Modify: `crates/engine/src/game/spawning.rs` — `habitat_pools` (line 886), `pick_habitat_species` (line 838), `pick_escort_species` (line 1005), `try_spawn_habitat_creature` (line 1086)
- Modify: `crates/engine/src/game/stack.rs:783`, `crates/engine/src/game/turn.rs:463`, `crates/engine/src/game/stack_features.rs:489` (`orphan_species`), `crates/engine/src/arena/encounter.rs:50`
- Test: `crates/engine/src/tests/spawning.rs`

**Interfaces:**
- Consumes: `SpeciesDb::windowed_matches`, `SpeciesDb::windowed_boss_matches` (Task 2); `Game::danger_steps` (existing, private to `spawning.rs`)
- Produces:
  - `Game::habitat_pools(&mut self, x: i32, y: i32, depth: Option<u32>) -> Option<(Vec<String>, Vec<String>)>` — ordinary pool, apex pool, both windowed
  - `Game::pick_habitat_species(&mut self, x: i32, y: i32, depth: Option<u32>, allow_boss: bool) -> Option<(String, bool)>`
  - `Game::pick_escort_species(&mut self, x: i32, y: i32, depth: Option<u32>) -> Option<String>`

- [ ] **Step 1: Write the failing tests**

All five go in `crates/engine/src/tests/spawning.rs`.

```rust
/// The headline. A fresh run's zone fields the easy end of the ladder and
/// nothing else — the thing that reads wrong today, where a level-1 player
/// can meet a band-2 species outside the seven-tile ring.
///
/// StaticField is the exception and is asserted rather than excused: it
/// ships no band-0 species, so the fallback reaches band 1 there.
#[test]
fn zone_one_fields_only_the_easiest_band() {
    let mut game = Game::new(4301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Hoisted: `species_defs()` clones the whole db, and this walk visits
    // thousands of tiles.
    let bands: std::collections::HashMap<String, DangerBand> = game
        .species_defs()
        .into_iter()
        .map(|s| (s.id.clone(), s.danger_band()))
        .collect();
    let mut checked = 0;
    for dx in -30..=30 {
        for dy in -30..=30 {
            let Some((ordinary, _)) = game.habitat_pools(dx, dy, None) else {
                continue;
            };
            let biome = game.world.resource_mut::<WorldMap>().tile(dx, dy).biome;
            let expected = if biome == Biome::StaticField {
                DangerBand::Tier(1)
            } else {
                DangerBand::Tier(0)
            };
            for id in &ordinary {
                let band = bands[id];
                assert_eq!(band, expected, "zone 1 offered {id} on {biome:?}");
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "the walk found no populated tiles at all");
}

/// A hand-authored boss must not turn up in a fresh run. It can today.
#[test]
fn zone_one_never_fields_an_apex_species() {
    let mut game = Game::new(4302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for dx in -30..=30 {
        for dy in -30..=30 {
            if let Some((_, apex)) = game.habitat_pools(dx, dy, None) {
                assert!(
                    apex.is_empty(),
                    "zone 1 offered apex species {apex:?} at ({dx}, {dy})"
                );
            }
        }
    }
}

/// The window follows depth underground, not the zone the entrance sat at —
/// the same rule `danger_steps` already applies to the two group curves.
#[test]
fn the_stack_window_follows_depth_not_the_surface_zone() {
    let mut game = Game::new(4303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bands: std::collections::HashMap<String, DangerBand> = game
        .species_defs()
        .into_iter()
        .map(|s| (s.id.clone(), s.danger_band()))
        .collect();
    let mut found = false;
    for dx in -30..=30 {
        for dy in -30..=30 {
            let (Some((deep, _)), Some((shallow, _))) = (
                game.habitat_pools(dx, dy, Some(6)),
                game.habitat_pools(dx, dy, None),
            ) else {
                continue;
            };
            if deep.iter().any(|id| bands[id] == DangerBand::Tier(2))
                && shallow.iter().all(|id| bands[id] != DangerBand::Tier(2))
            {
                found = true;
            }
        }
    }
    assert!(
        found,
        "no tile fielded a band-2 species at depth 6 that zone 1 withholds — \
         depth is not moving the window"
    );
}

/// The boss roll fires everywhere outside the opening ring, and before
/// `APEX_ENTRY_STEP` it can only produce a rolled boss — the whole of "easy
/// bosses on the surface, hard ones deep".
#[test]
fn an_early_boss_is_a_rolled_one() {
    let mut game = Game::new(4304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let far = OPENING_RING_TILES * 3;
    let apex: std::collections::HashSet<String> = game
        .species_defs()
        .into_iter()
        .filter(|s| s.is_boss)
        .map(|s| s.id.clone())
        .collect();
    let mut bosses = 0;
    for i in 0..4000 {
        let (x, y) = (pos.x + far + (i % 40), pos.y + far + (i / 40));
        let Some((species, is_boss)) = game.pick_habitat_species(x, y, None, true) else {
            continue;
        };
        if is_boss {
            bosses += 1;
            assert!(
                !apex.contains(&species),
                "zone 1 named the apex species {species} as a boss"
            );
        }
    }
    assert!(
        bosses > 0,
        "4000 picks produced no boss at all at a {BOSS_SPAWN_CHANCE} rate — \
         the roll is not firing"
    );
}

/// The opening ring turns a boss away, the same as it turns a rare tier
/// away, and for the same reason: a `BOSS_STAT_MULT` spawn in the nursery
/// falsifies `balance_sim::beatable_by_a_fresh_player`.
#[test]
fn the_opening_ring_refuses_a_boss() {
    let mut game = Game::new(4305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    for _ in 0..2000 {
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some((species, is_boss)) =
                    game.pick_habitat_species(pos.x + dx, pos.y + dy, None, true)
                {
                    assert!(!is_boss, "the opening ring produced a boss: {species}");
                }
            }
        }
    }
}
```

`stack.rs`'s existing `the_species_a_frame_offers_survives_a_save_and_load` is the orphan half of this and must keep passing untouched — the window is a pure function of `(biome, step)` and both are stable for a frame, so threading depth through `habitat_pools` must not have moved what an orphan is.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine zone_one_fields_only_the_easiest_band`
Expected: FAIL to compile — `habitat_pools` takes two arguments.

- [ ] **Step 3: Window the pools**

`habitat_pools` gains `depth: Option<u32>` and swaps the two primitives for the windowed ones:

```rust
pub(crate) fn habitat_pools(
    &mut self,
    x: i32,
    y: i32,
    depth: Option<u32>,
) -> Option<(Vec<String>, Vec<String>)> {
    let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
    if !tile.walkable {
        return None;
    }
    let step = self.danger_steps(depth);
    let species_db = self.world.resource::<SpeciesDb>();
    let mut candidates: Vec<String> = species_db
        .windowed_matches(tile.biome, step)
        .into_iter()
        .map(|s| s.id.clone())
        .collect();
    let mut boss_candidates: Vec<String> = species_db
        .windowed_boss_matches(tile.biome, step)
        .into_iter()
        .map(|s| s.id.clone())
        .collect();
    // ... the rest of the body is unchanged, opening ring included
}
```

Extend the doc comment: `depth` is handed in rather than read off the party's locale, for the reason `SpawnEscalation`'s doc already gives — ambient surface spawns and nest respawns keep rolling on every tick while the party is underground, so a step read inside here would size those from the party's depth.

- [ ] **Step 4: Union the pools at the draw**

`pick_habitat_species` gains `depth: Option<u32>` (before `allow_boss`) and changes two things and only two:

```rust
let (candidates, boss_candidates) = self.habitat_pools(x, y, depth)?;
// The ring turns a boss away the same way it turns a rare tier away: a
// `BOSS_STAT_MULT` spawn in the nursery falsifies
// `balance_sim::beatable_by_a_fresh_player`. This sits exactly where
// `!boss_candidates.is_empty()` used to, so the draw count is unchanged —
// every biome ships an apex species, so that guard was true everywhere
// except the ring, which is the only place it short-circuited.
let spawn_boss = allow_boss && !self.in_opening_ring(x, y) && {
    let mut rng = self.world.resource_mut::<GameRng>();
    rng.0.random_bool(BOSS_SPAWN_CHANCE)
};
let pool = if spawn_boss {
    // A boss is drawn from the whole window, apex included where the step
    // admits it. Below `APEX_ENTRY_STEP` that leaves the ordinary pool,
    // which is the point: an early boss is a rolled one.
    let mut both = candidates;
    both.extend(boss_candidates);
    both.sort();
    both
} else if candidates.is_empty() {
    return None;
} else {
    candidates
};
```

The old `if !allow_boss { return None }` arm goes away with the `&boss_candidates` fallback it guarded — `windowed_matches` never returns empty for a biome with any ordinary species, and a biome with none now returns `None` from the `candidates.is_empty()` arm above. Re-read the replaced block and make sure no other case it covered has been dropped.

Sorting the union matters: the draw picks by index, and concatenating two sorted vectors does not give a sorted one.

- [ ] **Step 5: Thread depth to the four other callers**

- `pick_escort_species(&mut self, x, y, depth: Option<u32>)` → `habitat_pools(x, y, depth)`. `spawn_pack`'s boss branch passes `esc.depth`.
- `try_spawn_habitat_creature` → `pick_habitat_species(x, y, None, true)` — a surface roll.
- `game/stack.rs:783` → `pick_habitat_species(ex, ey, Some(pos.depth), false)`. The function already has `pos` in scope; use `esc.depth` if that is the value already to hand, but do not recompute a depth from anything else.
- `game/turn.rs:463` → `pick_habitat_species(tx, ty, None, false)` — a surface ambush.
- `game/stack_features.rs:489` (`orphan_species`) → `habitat_pools(ex, ey, Some(pos.depth))`. It keeps its frame-seeded `StdRng`; the window is a pure function of `(biome, step)` and both are stable for a frame, so an orphan still survives a save/load unchanged.
- `arena/encounter.rs:50` → `pick_habitat_species(x, y, None, true)`.

- [ ] **Step 6: Run the tests, and expect seeded ones to move**

Run: `cargo test -p feral-processes-engine` then `cargo test --workspace`

Seeded spawn tests will fail here: the pool contents changed, so a seeded run picks a different species. That is the feature working. For each failure:

1. Read what the test actually asserts. If it asserts on a *specific species id* as a stand-in for "some wild creature", it was incidentally coupled to the old pool — fix the coupling (ask the db for a species that qualifies, the way `tests/combat_status.rs` does), not the seed.
2. If it asserts on a *count* or a *distribution*, re-derive the expected number and say in the commit message what moved and why.
3. **Never change a seed to make a test pass.** If you cannot explain a failure, stop and report it.

Note also: `crates/engine/src/arena/encounter.rs:252`'s `boss_count` helper counts bosses by species def. Change it to `game.is_boss_creature(e)` — it is in the same crate, so the `pub(crate)` door is reachable.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -5
git add -u crates/engine/src
git commit -m "feat(spawning): gate the species pool on the danger step

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: A lair guardian is drawn from the window and is always a boss

Closes the standing trap: a biome with no apex species currently gets a guardian that is not a boss and so pays no Portal Fragments.

**Files:**
- Modify: `crates/engine/src/game/stack_features.rs:190` (`pick_lair_species`)
- Test: `crates/engine/src/tests/stack.rs`

**Interfaces:**
- Consumes: `SpeciesDb::windowed_matches` (Task 2), `Game::danger_steps` (existing)
- Produces: `Game::pick_lair_species(&mut self, pos: StackPos) -> Option<(String, bool)>` — signature unchanged, second element now always `true`

`danger_steps` is private to `game/spawning.rs`. Make it `pub(crate)` so `stack_features.rs` can ask it; do **not** recompute `depth - 1` here, which is the duplicated-formula trap this repo has been bitten by four times.

- [ ] **Step 1: Write the failing tests**

Both go in `crates/engine/src/tests/stack.rs`, which has `game()`, `descend()` and `descend_through_a_real_link()` at the top of the file.

```rust
/// The trap, pinned. A guardian used to fall back to the toughest *ordinary*
/// species and come back `false`, which pays no Portal Fragments — a stack
/// that is unbreachable in everything but name. It is unreachable against
/// the shipped assets only because both apex species happen to list all four
/// biomes, so this asserts it against a db with them removed.
#[test]
fn a_lair_guardian_is_a_boss_even_where_the_biome_has_no_apex_species() {
    let mut game = game();
    let entrance = descend(&mut game);
    {
        let mut db = game.world.resource_mut::<SpeciesDb>();
        db.retain(|s| !s.is_boss);
    }
    let pos = game.stack_pos().expect("descend installed a frame");
    let (species, is_boss) = game
        .pick_lair_species(pos)
        .expect("a biome with ordinary species must still field a guardian");
    assert!(
        is_boss,
        "the guardian {species} at {entrance:?} came back not-a-boss, so it \
         pays no Portal Fragments and the stack cannot be breached"
    );
    assert!(
        game.species_defs()
            .into_iter()
            .any(|s| s.id == species && !s.is_boss),
        "with the apex species removed the guardian must be a rolled one"
    );
}

/// A guardian is drawn from the window at its own depth, which is what makes
/// a deep lair a different fight from a shallow one. Asserted as "never
/// easier deeper" rather than as an exact band, because a biome with a hole
/// in its ladder falls back and the fallback is allowed to repeat.
#[test]
fn a_deeper_lair_draws_a_guardian_no_easier_than_a_shallow_one() {
    let mut game = game();
    let entrance = descend_through_a_real_link(&mut game);
    let band_at = |game: &mut Game, depth: u32| {
        game.descend_to(depth, depth, entrance);
        let pos = game.stack_pos().expect("descend_to installed a frame");
        let (species, _) = game
            .pick_lair_species(pos)
            .expect("a walkable entrance fields a guardian");
        game.species_defs()
            .into_iter()
            .find(|s| s.id == species)
            .expect("a picked id is a loaded species")
            .danger_band()
    };
    let shallow = band_at(&mut game, 1);
    let deep = band_at(&mut game, 6);
    let rank = |b: DangerBand| match b {
        DangerBand::Tier(i) => i,
        DangerBand::Apex => usize::MAX,
    };
    assert!(
        rank(deep) >= rank(shallow),
        "a depth-6 guardian ({deep:?}) is easier than a depth-1 one ({shallow:?})"
    );
}
```

`SpeciesDb::retain` does not exist. Add it beside the `#[cfg(test)] pub(crate) fn insert` already on `SpeciesDb` (line ~820), with the same `#[cfg(test)]` gate and the same reasoning in its doc comment — it exists for a fixture that has to reach a case the shipped assets cannot produce:

```rust
#[cfg(test)]
pub(crate) fn retain(&mut self, keep: impl Fn(&SpeciesDef) -> bool) {
    self.species.retain(|_, s| keep(s));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine a_lair_guardian_is_a_boss_even_where`
Expected: FAIL — the guardian comes back `false` in a biome with no apex species.

- [ ] **Step 3: Implement**

Replace `pick_lair_species`' body:

```rust
pub(crate) fn pick_lair_species(&mut self, pos: StackPos) -> Option<(String, bool)> {
    let (ex, ey) = pos.entrance;
    let biome = self.world.resource_mut::<WorldMap>().tile(ex, ey).biome;
    let spec = self.frame_spec(pos.depth, pos.frames, pos.entrance);
    let step = self.danger_steps(Some(pos.depth));

    // Apex species first where the depth admits them — a hand-authored boss
    // is the better guardian when one is available — and the windowed
    // ordinary pool otherwise. Either way the guardian **is** a boss: the
    // fallback used to return `false`, which paid no Portal Fragments and
    // made a stack under a biome with no apex species unbreachable in
    // everything but name.
    let db = self.world.resource::<SpeciesDb>();
    let mut pool: Vec<String> = db
        .windowed_boss_matches(biome, step)
        .into_iter()
        .map(|s| s.id.clone())
        .collect();
    if pool.is_empty() {
        pool = db
            .windowed_matches(biome, step)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
    }
    if pool.is_empty() {
        return None;
    }
    // Salted off the level's own stream so the choice of guardian doesn't
    // correlate with the shape of the room it stands in.
    const LAIR_SALT: u64 = 0x1A19_B055;
    let mut rng = StdRng::seed_from_u64(spec.rng_seed() ^ LAIR_SALT);
    Some((pool[rng.random_range(0..pool.len())].clone(), true))
}
```

Note this replaces the old `max_by_key` on flat stat total with a seeded draw over the windowed pool — the window is what decides difficulty now, so picking the toughest of it as well would double-count.

- [ ] **Step 4: Correct the fixture doc comment the old fallback justified**

`crates/engine/src/tests/support.rs`'s `rouse_a_tameable_guardian` explains itself by pointing at `pick_lair_species`' fallback — "the toughest *ordinary* program a biome with no boss can field ... which carries no `is_boss` to refuse on". That fallback is gone. The fixture still works, because it hand-spawns its guardian and hand-writes `BattleState::lair` rather than calling `pick_lair_species` at all — but its doc now describes code that does not exist. Rewrite it to say that: a real `rouse_lair` can no longer produce a tameable guardian at all, so the fixture installs the state directly, and that is the point of it.

Do not "fix" this by making some guardians tameable. `Game::is_boss_creature` refusing a decompile is the rule; the fixture reaching around it is a fixture.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p feral-processes-engine stack::` then `cargo test --workspace`
Expected: PASS. Stack tests asserting on a specific guardian species will move; treat them the way Task 5 Step 6 says.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -5
git add -u crates/engine/src
git commit -m "feat(stack): a lair guardian is drawn from the window and always pays

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Docs, the balance gate, and the arena measurement

**Files:**
- Modify: `assets/species/README.md`
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md` — they are gitignored twins of the same document with nothing to catch drift
- Modify: `docs/seams.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: `assets/species/README.md`**

Two edits, no schema change:

1. In the `is_boss` block, change "it's excluded from the normal per-tile habitat spawn roll and spawns in its place only rarely" to describe what the flag now means: an **apex** species, always a boss, never engine-scaled, and eligible only from `APEX_ENTRY_STEP` down. Say plainly that **any** species can now spawn as a boss, that a rolled one is scaled by `BOSS_STAT_MULT` instead of being authored tough, and that the "a boss never rolls a rare tier" rule covers both.
2. Add a section, "Which zones and depths a species spawns in", under "The five classes": the band is derived from `growth_multiplier`, the window is `tuning.rs`'s, the fallback covers a biome with a hole in its ladder, and a mod that wants its species everywhere should sit on a shipped rung.

- [ ] **Step 2: `CLAUDE.md` and `docs/seams.md`**

Three entries change meaning. Edit the one-or-two-line rule in `CLAUDE.md` and the argument in `docs/seams.md` under the same title:

- **"`is_boss` means 'spawns as its own group', not 'is scaled up'."** Now: `is_boss` marks an **apex** species — always a boss, never engine-scaled — while any species can be *rolled* into one and takes `BOSS_STAT_MULT`. `Game::is_boss_creature` is the one door; `components::Boss` is the per-entity half.
- **"Which side of the ground a boss dies on decides what it pays"** — the sentence "removing a habitat from the last boss covering some terrain makes every stack under it unbreachable" is **no longer true** and must go: `pick_lair_species` falls back to the windowed ordinary pool and marks it a boss. Say what closed it.
- **New entry, under "Combat, progression and balance":** a species' danger band is derived, the window is `tuning.rs`'s, `habitat_pools` takes `depth` as a parameter for the same reason `SpawnEscalation` does, and the per-biome fallback is load-bearing at both ends of the shipped roster.

Then `cp CLAUDE.md AGENTS.md`.

- [ ] **Step 3: Run the balance gate and record what moved**

```bash
cargo test -p feral-processes-engine balance_sim
```

The curves may move. `toughest_ordinary_species` now over-states what zone 1 can field, which makes the gate **conservative rather than wrong** — leave it alone in this change. If a curve test fails, report the numbers to the user before touching it; narrowing the gate to the window is its own decision and wants its own argument.

- [ ] **Step 4: Measure `BOSS_STAT_MULT` in the arena**

```bash
cargo run --bin arena -- dev-arenas/opening-fight.ron
cargo run --bin arena -- dev-arenas/full-group.ron --out /tmp/claude-1000/-home-trog-code-feral-processes/1853870d-40bb-4a35-bbe9-0cb955fe2c0b/scratchpad/boss-mult.ron
```

Nothing else measures this number — `balance_sim` models no bosses. Arena numbers compare within one build only: a moved baseline is a reshuffled RNG stream, not a difficulty change, so read deltas against a baseline captured on this same build and never absolutes against an older report. Report the figures to the user rather than retuning on your own judgement.

- [ ] **Step 5: Full suite**

```bash
cargo test --workspace
```

Expected: PASS, all of it. This is the gate — passing only the tests written here is not evidence of correctness.

- [ ] **Step 6: `CHANGELOG.md` and commit**

Add the section at merge time, not now — the repo's rule is one release per change landing on `main`, and the bump happens once at the merge so a rebase cannot invalidate a tagged version. Write the section content into the commit body so it is ready.

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -5
git add -u
git commit -m "docs: the danger window, and what boss now means

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

- [ ] **Step 7: Report to the user**

Say plainly:

- that the feature has a green suite and **has not been played** — a green suite is not evidence of play — and offer to launch it (`FERAL_DEV_REVEAL=1 cargo run -- --template stack` for the Stack half, a fresh run for the zone-1 half);
- the arena figures for `BOSS_STAT_MULT` and whether they support 1.75;
- whether `balance_sim` moved, and by how much;
- the deviation recorded at the top of this plan (`boss_habitat_matches` kept as a primitive rather than deleted);
- the two content holes left open on purpose — a band-2 OpenGrid species and a band-0 StaticField species — and that the fallback is covering both.
