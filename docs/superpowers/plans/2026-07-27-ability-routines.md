# Ability Routines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make abilities installable "routines" that occupy level-derived slots on the player and every companion, extractable from a program you own at the cost of that program.

**Architecture:** A new `Routines(Vec<AbilityId>)` component replaces both ability-sourcing paths (species list for companions, researched set for the player) with one stored list that `Game::actor_abilities` reads for everyone. Loose routines are ordinary inventory items whose `ItemDef`s are synthesized from `AbilityDb` at load, so a modder's new ability becomes extractable and installable without a second file. Decompile becomes an ability the player starts with pre-installed, which deletes its bespoke `BattleAction` variant.

**Tech Stack:** Rust 2024, `bevy_ecs` (standalone), `ron` for assets, `bincode` for saves, `macroquad` for the renderer.

**Spec:** `docs/superpowers/specs/2026-07-27-ability-routines-design.md`

## Global Constraints

- **Every difficulty constant goes in `crates/engine/src/tuning.rs`**, as a documented `pub const` inside a labelled section. Never inline a tuning number in a formula.
- **New `.ron` schema fields must be `#[serde(default)]`** so existing files — including anyone's mods — keep parsing untouched.
- **A malformed `.ron` file is skipped with a logged warning, never a panic.** Follow `AbilityDb::load_dir`.
- **Update the matching `assets/*/README.md` in the same task** whenever a field is added, removed, or changes meaning.
- **The engine's `Game` struct is the entire public API the renderer talks to via app-core.** The renderer never touches the ECS `World`.
- **A doc comment claiming to "mirror"/"match"/"be shared with" another module's formula must be a call, not a copy.**
- **Run `cargo fmt` and `cargo clippy --workspace` after every task**; fix warnings rather than silencing them.
- **`cargo test --workspace` is the final gate.** Baseline before this plan: **527 tests green** on branch `feat/ability-routines`.
- **`cargo test -p feral-processes-engine balance_sim`** after any task that touches `tuning.rs` or an asset file. A moved curve means progression changed — that is the signal, not a broken test.
- Commit at the end of each task. Do not push; pushing needs an explicit ask from the user.
- Save format has **no migration by design** (`crates/engine/src/save.rs:150-175`). `SAVE_FORMAT_VERSION` is bumped exactly once, in Task 2 (10 → 11).

---

## File Structure

**Created**
- `assets/abilities/decompile.ron` — the decompile ability (Task 7)
- `crates/app-core/src/app/routines.rs` — key handling for the six routine/extract screens (Task 8)
- `crates/gui/src/render/routines.rs` — drawing for those screens (Task 8)

**Modified — engine**
- `crates/engine/src/tuning.rs` — six slot constants (Task 1)
- `crates/engine/src/abilities.rs` — slot functions, `DECOMPILE_ABILITY_ID`, `AbilityEffect::Decompile`, `routine_item_id` (Tasks 1, 4, 7)
- `crates/engine/src/components.rs` — `Routines` component (Task 2)
- `crates/engine/src/save.rs` — `routines` on `PlayerSave`/`CreatureSave`, version bump (Task 2)
- `crates/engine/src/game/lifecycle.rs` — spawn/load/save wiring, routine-item synthesis, decompile validation (Tasks 2, 4, 7)
- `crates/engine/src/game/combat_rewards.rs` — install on tame, level-up unlocks, `attempt_decompile` signature (Tasks 2, 7)
- `crates/engine/src/game/party.rs` — install on fuse, `ability_label` comment (Tasks 2, 6)
- `crates/engine/src/game/routines.rs` **(new)** — the install/uninstall/extract API and its views (Tasks 4, 5)
- `crates/engine/src/game/mod.rs` — declare the new `routines` module (Task 4)
- `crates/engine/src/game/combat.rs` — `actor_abilities` reads `Routines`; `player_abilities` deleted; Special row hidden; decompile refusals (Tasks 2, 6, 7)
- `crates/engine/src/game/combat_round.rs` — decompile resolves as a Special (Task 7)
- `crates/engine/src/game/unlocks.rs` — research grants routine items (Task 6)
- `crates/engine/src/game/catalog.rs` — `structure_description` deleted (Task 3)
- `crates/engine/src/items_db.rs` — `ItemDef::description`, `ItemDef::routine` (Tasks 3, 4)
- `crates/engine/src/structures.rs` — `StructureDef::description`, `StructureDef::extracts_routines` (Tasks 3, 5)
- `crates/engine/src/battle.rs` — `BattleAction::Decompile` / `ActionKind::Decompile` deleted (Task 7)
- `crates/engine/src/views.rs` — `RoutineSlotView`, `RoutineHolderView`, `RoutineItemView` (Task 4)
- `crates/engine/src/tests/support.rs` — helpers install routines (Task 2)
- `crates/engine/src/tests/routines.rs` **(new)** — the feature's own tests (Tasks 2, 4, 5, 6, 7)
- `crates/engine/src/tests/mod.rs` — declare it (Task 2)

**Modified — app-core / gui**
- `crates/app-core/src/lib.rs` — six new `Mode` variants, `action_from` arm deleted (Tasks 7, 8)
- `crates/app-core/src/app/mod.rs`, `input.rs`, `playing.rs` — route the new keys (Task 8)
- `crates/gui/src/render/mod.rs`, `building.rs`, `battle.rs` — draw the new modes, read authored descriptions (Tasks 3, 8)

**Modified — docs**
- `assets/items/README.md`, `assets/structures/README.md` (Task 3), `assets/abilities/README.md` (Tasks 5, 7)
- `README.md`, `CHANGELOG.md` (Task 9)

---

### Task 1: Slot constants and the slot formula

Pure arithmetic with no wiring. Nothing reads it yet; Task 2 does.

**Files:**
- Modify: `crates/engine/src/tuning.rs` (append a new section at the end of the file)
- Modify: `crates/engine/src/abilities.rs`
- Test: `crates/engine/src/abilities.rs` (its own `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `abilities::companion_routine_slots(level: u32) -> usize`, `abilities::player_routine_slots(level: u32) -> usize`, and the six `tuning::*_ROUTINE_SLOT_*` constants.

- [ ] **Step 1: Write the failing test**

Append to the `mod tests` block at the bottom of `crates/engine/src/abilities.rs`:

```rust
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
        assert_eq!(player_routine_slots(1), 1, "the starting slot holds decompile");
        assert_eq!(player_routine_slots(9), 1, "still nothing free at 9");
        assert_eq!(player_routine_slots(10), 2, "the first free slot arrives at 10");
        assert_eq!(player_routine_slots(49), 5);
        assert_eq!(player_routine_slots(50), 6);
        assert_eq!(
            player_routine_slots(9_999),
            crate::tuning::PLAYER_ROUTINE_SLOT_CAP as usize,
            "the player has no level cap, so only this clamp bounds their slots"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine abilities::tests::companion_slots`
Expected: FAIL — `cannot find function 'companion_routine_slots' in this scope`.

- [ ] **Step 3: Add the constants**

Append to the end of `crates/engine/src/tuning.rs`:

```rust
// ─────────────────────────────────────────────────────────────────────────
// Routine slots
// ─────────────────────────────────────────────────────────────────────────

/// Slots a companion has at level 1 before any per-level growth, then one
/// more for every `COMPANION_ROUTINE_SLOT_PER_LEVEL` levels. The floor of 1
/// in `abilities::companion_routine_slots` is what keeps a level-1 program
/// from having nowhere to hold its innate kit.
pub const COMPANION_ROUTINE_SLOT_BASE: u32 = 0;

/// Levels a companion needs per additional routine slot.
pub const COMPANION_ROUTINE_SLOT_PER_LEVEL: u32 = 2;

/// Most routines a companion can hold at once, reached at level 12.
pub const COMPANION_ROUTINE_SLOT_CAP: u32 = 6;

/// Slots the player has at level 1. One, and `decompile` occupies it — a new
/// game pre-installs that ability, so the player's first *free* slot is the
/// one `PLAYER_ROUTINE_SLOT_PER_LEVEL` grants.
pub const PLAYER_ROUTINE_SLOT_BASE: u32 = 1;

/// Levels the player needs per additional routine slot. Deliberately far
/// slower than a companion's: researched routines are meant to be a choice
/// between programs, not a second kit the player accumulates for free.
pub const PLAYER_ROUTINE_SLOT_PER_LEVEL: u32 = 10;

/// Most routines the player can hold at once, reached at level 50. The
/// player has no level ceiling (`progression::add_xp` takes `None`), so this
/// clamp is the only thing bounding their slots.
pub const PLAYER_ROUTINE_SLOT_CAP: u32 = 6;
```

- [ ] **Step 4: Add the slot functions**

In `crates/engine/src/abilities.rs`, insert directly below the `FALLBACK_ABILITY_ID` constant:

```rust
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
```

- [ ] **Step 5: Run the tests and clippy**

Run: `cargo test -p feral-processes-engine abilities::tests`
Expected: PASS, including the two new tests.

Run: `cargo fmt && cargo clippy --workspace`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/tuning.rs crates/engine/src/abilities.rs
git commit -m "feat: routine slot constants and the level curve"
```

---

### Task 2: The `Routines` component

Stores the list, installs a species' innate kit, persists it, and switches the **companion** read path over. The player still reads `player_abilities` until Task 6 — that keeps the tree green while the two sides are converted separately.

**Files:**
- Modify: `crates/engine/src/components.rs`
- Modify: `crates/engine/src/save.rs:12-45` (`PlayerSave`), `:47-94` (`CreatureSave`), `:175` (`SAVE_FORMAT_VERSION`), `:224-262` (`sample_data`)
- Modify: `crates/engine/src/game/lifecycle.rs:40-84` (player spawn), `:240-268` (creature load), `:400-452` (creature save), and the `PlayerSave` construction in `save`
- Modify: `crates/engine/src/game/combat_rewards.rs:146-186` (`award_party_xp`), `:241-262` (tame)
- Modify: `crates/engine/src/game/party.rs:378-405` (fuse)
- Modify: `crates/engine/src/game/combat.rs:388-412` (`companion_abilities`)
- Modify: `crates/engine/src/tests/support.rs:57-60`, `:403-422`, `:554-578`
- Create: `crates/engine/src/tests/routines.rs`
- Modify: `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Consumes: `abilities::companion_routine_slots` (Task 1).
- Produces:
  - `components::Routines(pub Vec<AbilityId>)` — a `Component`, `Default`, `Clone`.
  - `Game::install_innate_routines(&mut self, entity: Entity)` — `pub(crate)`; inserts `Routines` holding every species ability already unlocked at the entity's level, truncated to its slots, falling back to `FALLBACK_ABILITY_ID` when the species declares none.
  - `Game::install_unlocked_routines(&mut self, entity: Entity, from_level: u32, to_level: u32)` — `pub(crate)`; installs species abilities whose unlock level lands in `(from_level, to_level]`.
  - `Game::routine_slots(&self, entity: Entity) -> usize` — `pub`; player curve for the player entity, companion curve for anything else.

- [ ] **Step 1: Write the failing tests**

Create `crates/engine/src/tests/routines.rs`:

```rust
//! Routines: the slots abilities occupy, and how they get there.

use super::support::*;
use crate::*;
use crate::components::Routines;

/// The generic test species declares no abilities, so its kit is the
/// fallback — which must be a real installed routine, not an empty list
/// resolved at read time.
#[test]
fn a_tamed_program_with_no_species_kit_starts_with_the_fallback_installed() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    let installed = game.world.get::<Routines>(pet).expect("a tamed program has routines");
    assert_eq!(
        installed.0,
        vec![crate::abilities::FALLBACK_ABILITY_ID.to_string()],
        "a species declaring no abilities implicitly installs the fallback"
    );
}

#[test]
fn a_species_kit_is_installed_at_tame_time_in_declared_order() {
    let (game, medic) = game_with_two_ability_companion();
    let installed = &game.world.get::<Routines>(medic).unwrap().0;
    assert_eq!(
        installed.len(),
        1,
        "only the level-1 unlock is installed on a level-1 program: {installed:?}"
    );
}

#[test]
fn a_level_up_that_reaches_an_unlock_installs_it_into_a_free_slot() {
    let (mut game, medic) = game_with_two_ability_companion();
    let before = game.world.get::<Routines>(medic).unwrap().0.len();
    // `TWO_ABILITY_SPECIES` gates `sandbox` at level 5, and level 5 is worth
    // two slots, so the unlock has somewhere to land.
    set_level(&mut game, medic, 5);
    let after = &game.world.get::<Routines>(medic).unwrap().0;
    assert_eq!(
        after.len(),
        before + 1,
        "reaching the second unlock should install it: {after:?}"
    );
}

#[test]
fn slot_count_follows_the_tuning_curve_for_both_sides() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    for level in 1..=12u32 {
        set_level(&mut game, pet, level);
        assert_eq!(
            game.routine_slots(pet),
            crate::abilities::companion_routine_slots(level),
            "companion slots at level {level}"
        );
    }
    for level in [1u32, 9, 10, 25, 50] {
        set_level(&mut game, player, level);
        assert_eq!(
            game.routine_slots(player),
            crate::abilities::player_routine_slots(level),
            "player slots at level {level}"
        );
    }
}

#[test]
fn installed_routines_survive_a_save_load_round_trip() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    let before = game.world.get::<Routines>(pet).unwrap().0.clone();
    let path = std::env::temp_dir().join(format!(
        "feral_routines_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .owned_pets()
        .first()
        .map(|p| loaded.world.get::<Routines>(p.entity).unwrap().0.clone())
        .expect("the tamed program should come back");
    assert_eq!(restored, before, "a save must carry installed routines");
}
```

Register it in `crates/engine/src/tests/mod.rs`, in alphabetical position (after `mod research;`):

```rust
mod routines;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine tests::routines`
Expected: FAIL — `cannot find type 'Routines' in module 'crate::components'`.

- [ ] **Step 3: Add the component**

In `crates/engine/src/components.rs`, next to `Inventory` (around line 195):

```rust
/// The abilities installed on this entity, in menu order — the player's and
/// every companion's entire kit. Length is bounded by
/// `Game::routine_slots`; position is what `BattleAction::Special::ability`
/// indexes.
///
/// A companion's species kit is *pre-installed* here rather than read from
/// `SpeciesDef` at menu time, which is what lets an innate ability be popped
/// out and plugged into a different program.
#[derive(Component, Default, Clone)]
pub struct Routines(pub Vec<AbilityId>);
```

Add `use crate::abilities::AbilityId;` to that file's imports if it isn't already there.

- [ ] **Step 4: Add the install helpers and the slot query**

Create the helpers in `crates/engine/src/game/combat.rs`, immediately above `companion_abilities`:

```rust
    /// How many routines `entity` can hold right now. The player and a
    /// companion grow slots at different rates on purpose — see
    /// `tuning::PLAYER_ROUTINE_SLOT_PER_LEVEL`.
    pub fn routine_slots(&self, entity: Entity) -> usize {
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        if entity == self.player_entity() {
            abilities::player_routine_slots(level)
        } else {
            abilities::companion_routine_slots(level)
        }
    }

    /// Installs the kit `entity`'s species grants at its current level,
    /// replacing whatever it holds. Called once when a program comes into
    /// existence — a decompile or a fusion — never afterwards.
    ///
    /// A species declaring no abilities gets `FALLBACK_ABILITY_ID` instead,
    /// which is what keeps an ability-less species commandable and keeps
    /// that ability obtainable by extraction: nothing else grants it.
    pub(crate) fn install_innate_routines(&mut self, entity: Entity) {
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        let slots = self.routine_slots(entity);
        let declared: Vec<AbilityId> = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.abilities.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.level <= level)
            .map(|a| a.id)
            .filter(|id| self.world.resource::<AbilityDb>().get(id).is_some())
            .take(slots)
            .collect();
        let installed = if declared.is_empty() {
            vec![abilities::FALLBACK_ABILITY_ID.to_string()]
        } else {
            declared
        };
        self.world.entity_mut(entity).insert(Routines(installed));
    }

    /// Installs every species ability whose unlock level lands in
    /// `(from_level, to_level]` — the ones this level-up just reached.
    ///
    /// An unlock with no free slot is logged and dropped for good: the
    /// window it could have been installed in has passed. No shipped species
    /// can reach that state (the most any declares is two abilities, the
    /// latest at level 8, and four slots exist by then), so this is
    /// mod-safety, and failing loudly beats carrying a pending-installs list
    /// nothing ships to exercise.
    pub(crate) fn install_unlocked_routines(
        &mut self,
        entity: Entity,
        from_level: u32,
        to_level: u32,
    ) {
        let reached: Vec<AbilityId> = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.abilities.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.level > from_level && a.level <= to_level)
            .map(|a| a.id)
            .filter(|id| self.world.resource::<AbilityDb>().get(id).is_some())
            .collect();
        if reached.is_empty() {
            return;
        }
        let slots = self.routine_slots(entity);
        let name = self.creature_label(entity);
        for id in reached {
            let mut installed = self
                .world
                .get::<Routines>(entity)
                .map(|r| r.0.clone())
                .unwrap_or_default();
            if installed.contains(&id) {
                continue;
            }
            if installed.len() >= slots {
                self.log(format!(
                    "{name} has no free routine slot for {id} — the unlock is lost."
                ));
                continue;
            }
            installed.push(id);
            self.world.entity_mut(entity).insert(Routines(installed));
        }
    }
```

Add `Routines` to the `components::{...}` import list in `crates/engine/src/lib.rs:39-45`.

- [ ] **Step 5: Switch the companion read path to `Routines`**

Replace the body of `companion_abilities` in `crates/engine/src/game/combat.rs:388-412` with:

```rust
    /// Every ability `entity` can be commanded to use right now, in menu
    /// order — whatever is installed in its routine slots.
    ///
    /// A companion's kit is installed at tame/fuse time and topped up on the
    /// level-ups that reach a species unlock (see `install_innate_routines`
    /// and `install_unlocked_routines`); nothing is resolved here.
    pub(crate) fn companion_abilities(&self, entity: Entity) -> Vec<AbilityDef> {
        let db = self.world.resource::<AbilityDb>();
        self.world
            .get::<Routines>(entity)
            .map(|r| r.0.as_slice())
            .unwrap_or_default()
            .iter()
            .filter_map(|id| db.get(id).cloned())
            .collect()
    }
```

Also update the doc comment on `SpeciesDef::abilities` (`crates/engine/src/species.rs:104-110`), which currently points at `Game::companion_abilities` for fallback resolution:

```rust
    /// The abilities a tamed member of this species is created holding, in
    /// menu order, each gated on the companion's level. Left empty, the
    /// companion is created holding `abilities::FALLBACK_ABILITY_ID`
    /// instead — see `Game::install_innate_routines`, which is the only
    /// place that fallback is resolved. `#[serde(default)]` so existing
    /// species files (including mods) without this field keep parsing.
```

- [ ] **Step 6: Wire the three creation points and the level-up**

In `crates/engine/src/game/combat_rewards.rs`, in `attempt_decompile` (around line 247), after the `Tamed`/`Experience` insert:

```rust
        self.world
            .entity_mut(front)
            .insert((Tamed { owner: player }, Experience::default()));
        self.install_innate_routines(front);
```

In `crates/engine/src/game/party.rs`, in `fuse_companions` after the `spawn(...)` block resolves to an entity — capture the id and install:

```rust
        let fused_entity = fused.id();
        if let Some(name) = &final_name {
            self.world.entity_mut(fused_entity).insert(CustomName(name.clone()));
        }
        self.install_innate_routines(fused_entity);
```

(Replace the existing `if let Some(name) = &final_name { fused.insert(...) }` — `fused` is an `EntityWorldMut` and must be dropped before `self` is borrowed again.)

In `crates/engine/src/game/combat_rewards.rs`, in `award_party_xp`, replace the `leveled` block (lines ~164-185) with:

```rust
            let before_level = self
                .world
                .get::<Experience>(companion)
                .map(|e| e.level)
                .unwrap_or(1);
            {
                let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
                let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, companion) else {
                    continue;
                };
                progression::add_xp(
                    &mut exp,
                    &mut stats,
                    amount,
                    growth_multiplier,
                    Some(crate::tuning::CREATURE_MAX_LEVEL),
                );
            }
            let level = self
                .world
                .get::<Experience>(companion)
                .map(|e| e.level)
                .unwrap_or(before_level);
            if level > before_level {
                self.install_unlocked_routines(companion, before_level, level);
                let name = self.creature_label(companion);
                self.log_kind(
                    MessageKind::LevelUp,
                    format!("{name} gains {amount} XP and levels up to {level}!"),
                );
            }
```

In `crates/engine/src/game/lifecycle.rs`, add `Routines::default()` to the player's spawn tuple (around line 60, next to `Perks::default()`). The player's kit stays empty until Task 7 pre-installs decompile.

- [ ] **Step 7: Persist it**

In `crates/engine/src/save.rs`, add to `PlayerSave` (after `item_fusions`):

```rust
    /// The abilities installed in the player's routine slots, in menu order
    /// — see `components::Routines`.
    pub routines: Vec<crate::abilities::AbilityId>,
```

Add to `CreatureSave` (after `fusions`):

```rust
    /// The abilities installed in this program's routine slots, in menu
    /// order — see `components::Routines`. Persisted rather than re-derived
    /// from its species, because an innate routine can be popped out and a
    /// foreign one plugged in.
    pub routines: Vec<crate::abilities::AbilityId>,
```

Bump the version:

```rust
pub const SAVE_FORMAT_VERSION: u32 = 11;
```

Add `routines: Vec::new(),` to `sample_data()`'s `PlayerSave` literal in that file's tests.

In `crates/engine/src/game/lifecycle.rs`:
- **save** — add `&Routines` as an `Option` to the creature query tuple and write `routines: routines.map(|r| r.0.clone()).unwrap_or_default()`; write the player's the same way from `self.world.get::<Routines>(player)`.
- **load** — insert `Routines(c.routines.clone())` on every creature (tamed or not; a wild one just carries an empty list) and `Routines(data.player.routines.clone())` on the player.

- [ ] **Step 8: Update the test helpers**

`crates/engine/src/tests/support.rs:57-60`:

```rust
/// Sets `entity`'s level directly, for tests that need a level-gated
/// ability unlocked without grinding XP into it. Installs whatever species
/// unlocks that jump reaches, exactly as a real level-up would — otherwise
/// a test that raises a level would see a kit the game never leaves behind.
pub(super) fn set_level(game: &mut Game, entity: Entity, level: u32) {
    let before = game.world.get::<Experience>(entity).unwrap().level;
    game.world.get_mut::<Experience>(entity).unwrap().level = level;
    if level > before {
        game.install_unlocked_routines(entity, before, level);
    }
}
```

In `spawn_tamed` (line ~403) and `game_with_two_ability_companion` (line ~554), capture the spawned id and call `game.install_innate_routines(id);` before returning it. Both bypass the real tame path, so without this every companion in the suite loses its Special.

- [ ] **Step 9: Run the tests**

Run: `cargo test -p feral-processes-engine tests::routines`
Expected: PASS — all five.

Run: `cargo test --workspace`
Expected: PASS. Save-related tests that construct `PlayerSave`/`CreatureSave` literals need the new field; fix each by adding `routines: Vec::new()`. Two are known: `crates/engine/src/save.rs`'s `sample_data` (already covered above) and `crates/app-core/src/tests/support.rs:26-45`'s `app_owning_distant_programs`, which builds tamed programs by editing a save and reloading it. That one must push a real kit rather than an empty list, or every program it creates arrives with no Special:

```rust
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
```

Run: `cargo test -p feral-processes-engine balance_sim`
Expected: PASS unchanged — `balance_sim` is RNG-free and does not read `Routines`.

- [ ] **Step 10: Commit**

```bash
git add crates/engine/src
git commit -m "feat: store abilities as installed routines on every party member"
```

---

### Task 3: Item and structure descriptions move into `.ron`

Independent of routines except that Task 4's synthesized routine items need `ItemDef::description` to exist. Deletes `Game::structure_description` and its two tests.

**Files:**
- Modify: `crates/engine/src/items_db.rs:55-77` (`ItemDef`)
- Modify: `crates/engine/src/structures.rs:113-120` (`StructureDef`)
- Modify: all 36 files in `assets/items/`, all 13 in `assets/structures/`
- Delete: `Game::structure_description` — `crates/engine/src/game/catalog.rs:150-230`
- Delete: `crates/engine/src/tests/building.rs:364-379` and `:381-395` (the two derivation tests)
- Modify: `crates/gui/src/render/building.rs:11`
- Modify: `assets/items/README.md`, `assets/structures/README.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `ItemDef::description: String`, `StructureDef::description: String`. `Game::structure_description` no longer exists — renderers read `def.description` directly.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/assets.rs`:

```rust
/// Authored text replaced a derivation that could not go blank. The only
/// thing left to guard mechanically is that nothing shipped is *missing*
/// text — a wrong number in an authored line is a review problem, not a
/// test one.
#[test]
fn every_shipped_item_and_structure_has_description_text() {
    let mut game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.item_defs() {
        assert!(
            !def.description.trim().is_empty(),
            "item {} ships with no description",
            def.id.as_str()
        );
    }
    for def in game.structure_defs() {
        assert!(
            !def.description.trim().is_empty(),
            "structure {} ships with no description",
            def.id
        );
    }
}
```

If `Game::item_defs()` does not exist, add it beside `structure_defs` in `crates/engine/src/game/catalog.rs`:

```rust
    /// Every loaded item definition, id-sorted (see `ItemDb::all`).
    pub fn item_defs(&self) -> Vec<ItemDef> {
        self.world.resource::<ItemDb>().all().cloned().collect()
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine every_shipped_item_and_structure`
Expected: FAIL — `no field 'description' on type '&ItemDef'`.

- [ ] **Step 3: Add both fields**

In `crates/engine/src/items_db.rs`, in `ItemDef` after `name`:

```rust
    /// One line on what this item is for, shown wherever the item is listed.
    /// Authored here rather than derived from the other fields so a modder
    /// controls exactly how their item reads. `#[serde(default)]` so an
    /// existing mod file without it still parses — as an empty line, which
    /// the shipped-assets test refuses for anything in this repo.
    #[serde(default)]
    pub description: String,
```

In `crates/engine/src/structures.rs`, in `StructureDef` after `name`, the same field with the same rationale ("shown in the build menu").

- [ ] **Step 4: Author the 13 structure descriptions**

Add a `description:` line to each file in `assets/structures/`. Worked example — `assets/structures/terminal.ron` becomes:

```ron
(
    id: "terminal",
    name: "Terminal",
    description: "Converts Core Fragments into Power Cells on its own while you stand within 2 tiles. The cheapest thing you can deploy.",
    glyph: 'T',
    color: Cyan,
    build_cost: [("core_fragment", 3)],
    work: None,
    passive_process: Some((
        consumes: "core_fragment",
        produces: "power_cell",
        ticks_per_unit: 1,
        radius: 2,
    )),
)
```

The remaining twelve, verbatim:

| file | `description:` |
|---|---|
| `home.ron` | `"Anchors your base — everything else must be deployed within 7 tiles of it, and demolishing it cascades. Raids can't touch it, you can recharge beside it, and you can symlink home from anywhere for 4 Power Cells."` |
| `mining_node.ron` | `"Assign a program here to mine Core Fragments. Yield is a chance per cycle that improves as the node is upgraded."` |
| `research_node.ron` | `"Assign a program here to gather Research Data. Slow, chancy, and the only source of the currency the research tree runs on."` |
| `power_conduit.ron` | `"Assign a program here to produce Power Cells — the fastest cronjob you can run."` |
| `compiler.ron` | `"Assign a program here to compile ICE Breakers. Also the bench that extracts a routine out of a program you own."` |
| `recharger_node.ron` | `"Trickles Power back into you every tick while you stand within 7 tiles. No worker, no input."` |
| `data_cache.ron` | `"Widens your roster by two while it stands, so a decompile has somewhere to land."` |
| `shield.ron` | `"Soaks 2 damage off every raid against every structure you own, not just itself. Stacks with a second one."` |
| `armory.ron` | `"Compile bench for researched weapon and armor recipes — the gear won't appear in the craft menu until one is standing."` |
| `fabricator.ron` | `"Compile bench for researched module and utility recipes — the gear won't appear in the craft menu until one is standing."` |
| `market.ron` (`black_market.ron`) | `"Sell anything here for 1 Core Fragment a unit, sell a program for a tenth of its power, and buy back ICE Breakers, Power Cells and Portal Fragments."` |
| `portal.ron` | `"Breaches into the next zone. Deeper zones cost more Portal Fragments to open, and neither fragments nor cores survive the trip."` |

- [ ] **Step 5: Author the 36 item descriptions**

Same edit in each file in `assets/items/`. Worked example — `assets/items/power_cell.ron` becomes:

```ron
(
    id: "power_cell",
    name: "Power Cell",
    description: "Restores 25 Power. The staple of staying on the Grid.",
    craftable: Some((cost: [("core_fragment", 2)])),
    consume: Some((power: 25.0)),
)
```

The rest, verbatim:

| id | `description:` |
|---|---|
| `core_fragment` | `"The Grid's currency. Every build cost, most recipes, and everything a trader pays you."` |
| `research_data` | `"Spent on the research tree. Banked separately, so up to 200 of it rides free of your cargo cap."` |
| `portal_fragment` | `"Opens a Zone Portal. Bosses drop caches of it; ordinary kills drop the odd one."` |
| `ice_breaker` | `"A taming catalyst. Spent on a decompile to raise the odds of the capture landing."` |
| `shiv_routine` | `"Scavenged weapon. Barely a blade, but it beats bare hands."` |
| `kinetic_edge` | `"Scavenged weapon. A cleaner strike than anything else you'll find lying around."` |
| `scrap_ward` | `"Scavenged armor. Thin plating bolted over the worst of the gaps."` |
| `packet_buffer` | `"Scavenged armor. Absorbs the first shock of an incoming packet."` |
| `probe_daemon` | `"Scavenged module. A small edge on decompile odds."` |
| `handshake_forge` | `"Scavenged module. Fakes the greeting a hostile program expects, easing a capture."` |
| `arc_lance` | `"Standard weapon. A straight, dependable damage upgrade."` |
| `recursion_blade` | `"Standard weapon. Trades some bite for plating that folds back over you."` |
| `daemon_fang` | `"Standard weapon. Cuts and pries at ICE at the same time."` |
| `hardened_shell` | `"Standard armor. Solid, unglamorous protection."` |
| `null_weave` | `"Standard armor. Weave that hits back a little as it absorbs."` |
| `static_mesh` | `"Standard armor. Bleeds off the charge that makes a capture fail."` |
| `trace_sniffer` | `"Standard module. Reads a target's weak points before you commit."` |
| `logic_probe` | `"Standard module. Sharpens both your strike and your capture."` |
| `entropy_damper` | `"Standard module. Steadies you while you work an ICE wall open."` |
| `sync_governor` | `"Standard module. A small lift to everything rather than a lot of one thing."` |
| `overclock_core` | `"Researched weapon. Pushes your clock past spec."` |
| `monofilament_whip` | `"Researched weapon. A filament one molecule wide, and the hardest hit at its tier."` |
| `firewall_plating` | `"Researched armor. Purpose-built to stop hostile traffic."` |
| `ablative_plating` | `"Researched armor. Sheds a layer per hit so you don't."` |
| `neural_amplifier` | `"Researched module. Amplifies the signal a decompile rides in on."` |
| `cortex_hack` | `"Researched module. Speaks to a hostile program in its own dialect."` |
| `plasma_router` | `"Premium weapon. Routes raw plasma through the target."` |
| `black_ice_pick` | `"Premium weapon. Built for black ICE, and it captures as well as it kills."` |
| `siege_compiler` | `"Premium weapon. Heavy enough to shield you while it swings."` |
| `bastion_lattice` | `"Premium armor. The best flat protection on the Grid."` |
| `phase_carapace` | `"Premium armor. Phases with you, so it strikes as well as it holds."` |
| `wraithsteel_plate` | `"Premium armor. Plating that leaves hostile ICE unsure you're there."` |
| `kernel_key` | `"Premium module. Kernel access — the single biggest lift to decompile odds."` |
| `oracle_core` | `"Premium module. Predicts the opening before it appears."` |
| `singularity_matrix` | `"Premium module. Everything at once, and the rarest thing you can compile."` |

- [ ] **Step 6: Delete the derivation and repoint the renderer**

Delete `Game::structure_description` entirely (`crates/engine/src/game/catalog.rs:150-230`).

`crates/gui/src/render/building.rs:11` becomes:

```rust
        .map(|def| def.description.clone())
```

Delete `crates/engine/src/tests/building.rs:364-379` and `:381-395` — the two tests over the deleted function.

- [ ] **Step 7: Document the new field**

In `assets/items/README.md` and `assets/structures/README.md`, add `description` to the field table:

```markdown
| `description` | string | `""` | One line on what this does, shown wherever it is listed. Optional for a mod, but every shipped file has one. |
```

Add a line to each README's prose noting that structure text used to be derived from the capability fields and is now authored — so a modder editing capabilities must edit the text too.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p feral-processes-engine tests::assets`
Expected: PASS.

Run: `cargo test --workspace`
Expected: PASS. `items_db::tests::the_shipped_items_load_cleanly_with_all_roles_and_fields` still asserts 36 items — unchanged, since nothing was added or removed.

- [ ] **Step 9: Commit**

```bash
git add assets crates/engine/src crates/gui/src
git commit -m "feat: author item and structure descriptions in .ron"
```

---

### Task 4: Routine items, install, and uninstall

**Files:**
- Modify: `crates/engine/src/abilities.rs` (add `routine_item_id`)
- Modify: `crates/engine/src/items_db.rs` (`ItemDef::routine`, `ItemDb::synthesize_routines`)
- Modify: `crates/engine/src/game/lifecycle.rs:577-615` (`load_asset_dbs`)
- Create: `crates/engine/src/game/routines.rs`
- Modify: `crates/engine/src/game/mod.rs`
- Modify: `crates/engine/src/views.rs`
- Test: `crates/engine/src/tests/routines.rs`

**Interfaces:**
- Consumes: `Routines`, `Game::routine_slots` (Task 2); `ItemDef::description` (Task 3).
- Produces:
  - `abilities::routine_item_id(ability: &str) -> ItemId`
  - `ItemDef::routine: Option<AbilityId>`
  - `ItemDb::synthesize_routines(&mut self, abilities: &AbilityDb)`
  - `Game::is_routine(&self, item: &ItemId) -> bool`
  - `Game::install_routine(&mut self, entity: Entity, item: &ItemId) -> Result<(), String>`
  - `Game::uninstall_routine(&mut self, entity: Entity, slot: usize) -> Result<(), String>`
  - `Game::routine_view(&self, entity: Entity) -> Vec<RoutineSlotView>`
  - `Game::routine_holders(&mut self) -> Vec<RoutineHolderView>`
  - `Game::loose_routines(&self) -> Vec<RoutineItemView>`
  - `views::{RoutineSlotView, RoutineHolderView, RoutineItemView}`

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/routines.rs`:

```rust
#[test]
fn every_loaded_ability_gets_a_routine_item_carrying_its_own_text() {
    let game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for ability in game.ability_defs() {
        let item = crate::abilities::routine_item_id(&ability.id);
        let def = game
            .item_def(&item)
            .unwrap_or_else(|| panic!("{} should have a synthesized routine item", ability.id));
        assert_eq!(def.routine.as_deref(), Some(ability.id.as_str()));
        assert_eq!(
            def.description, ability.description,
            "a routine item reads its text from the ability, never a copy"
        );
        assert!(def.name.ends_with(" Routine"), "{}", def.name);
    }
}

#[test]
fn install_then_uninstall_returns_the_same_item() {
    let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 4); // two slots, one of them free
    let item = crate::abilities::routine_item_id("sandbox");
    set_inventory(&mut game, &[(item.as_str(), 1)]);

    game.install_routine(pet, &item).unwrap();
    assert_eq!(count_item(&game, item.as_str()), 0, "installing spends the item");
    assert!(
        game.routine_view(pet).iter().any(|s| s.ability.as_deref() == Some("sandbox")),
        "the routine should occupy a slot"
    );

    let slot = game
        .routine_view(pet)
        .into_iter()
        .position(|s| s.ability.as_deref() == Some("sandbox"))
        .unwrap();
    game.uninstall_routine(pet, slot).unwrap();
    assert_eq!(count_item(&game, item.as_str()), 1, "uninstalling gives it back");
}

#[test]
fn install_is_refused_with_no_free_slot_without_the_item_and_during_battle() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3); // level 1: exactly one slot, already full
    let item = crate::abilities::routine_item_id("sandbox");

    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("don't have"), "no copy held: {err}");

    set_inventory(&mut game, &[(item.as_str(), 1)]);
    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("no free routine slot"), "slots full: {err}");

    set_level(&mut game, pet, 4);
    start_battle_with_a_wild_program(&mut game);
    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("right now"), "mid-battle: {err}");
}

#[test]
fn an_innate_routine_can_be_popped_out_and_plugged_into_another_program() {
    let (mut game, medic) = game_with_two_ability_companion();
    let popped = game.routine_view(medic)[0].ability.clone().unwrap();
    game.uninstall_routine(medic, 0).unwrap();
    assert!(
        game.routine_view(medic).iter().all(|s| s.ability.is_none()),
        "the innate slot should now be empty"
    );

    let host = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, host, 4);
    let item = crate::abilities::routine_item_id(&popped);
    game.install_routine(host, &item).unwrap();
    assert!(
        game.routine_view(host).iter().any(|s| s.ability.as_deref() == Some(popped.as_str())),
        "a foreign species' routine should install fine"
    );
}
```

`Game::ability_defs()` and `Game::item_def(&ItemId)` are needed by these tests. Add both to `crates/engine/src/game/catalog.rs`:

```rust
    /// Every loaded ability definition, id-sorted (see `AbilityDb::all`).
    pub fn ability_defs(&self) -> Vec<AbilityDef> {
        self.world.resource::<AbilityDb>().all().cloned().collect()
    }

    /// One item definition by id, or `None` if nothing declares it.
    pub fn item_def(&self, item: &ItemId) -> Option<ItemDef> {
        self.world.resource::<ItemDb>().get(item.as_str()).cloned()
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine tests::routines`
Expected: FAIL — `cannot find function 'routine_item_id'`.

- [ ] **Step 3: Add the id helper and the item field**

In `crates/engine/src/abilities.rs`:

```rust
/// The inventory item a loose (uninstalled) copy of `ability` takes. Minted
/// by `ItemDb::synthesize_routines` rather than authored, so a modder's new
/// ability is extractable and installable with no second file to write.
pub fn routine_item_id(ability: &str) -> crate::items::ItemId {
    crate::items::ItemId(format!("routine_{ability}"))
}
```

In `crates/engine/src/items_db.rs`, in `ItemDef` after `droppable`:

```rust
    /// The ability a loose copy of this item installs, for a routine item.
    /// Set only on the defs `ItemDb::synthesize_routines` mints; an authored
    /// file leaving it `None` is an ordinary item. `#[serde(default)]` like
    /// every other optional field.
    #[serde(default)]
    pub routine: Option<crate::abilities::AbilityId>,
```

- [ ] **Step 4: Synthesize the routine items**

In `crates/engine/src/items_db.rs`, on `impl ItemDb`:

```rust
    /// Mints one item per loaded ability, so a loose routine is an ordinary
    /// inventory item that stores, stacks and sells with no new machinery.
    ///
    /// Called after both databases load rather than inside `load_dir`, which
    /// has no view of `AbilityDb`. The description is *read* from the
    /// ability rather than copied into a second authored file, so the two
    /// cannot drift.
    ///
    /// An ability whose routine id collides with an authored item is skipped
    /// with a warning: the authored file wins, exactly as a duplicate
    /// economy role does.
    pub fn synthesize_routines(&mut self, abilities: &AbilityDb) -> Vec<String> {
        let mut warnings = Vec::new();
        for ability in abilities.all() {
            let id = crate::abilities::routine_item_id(&ability.id);
            if self.items.contains_key(id.as_str()) {
                warnings.push(format!(
                    "ability {} wants routine item {}, which an authored item already claims; \
                     the ability will not be extractable",
                    ability.id,
                    id.as_str()
                ));
                continue;
            }
            self.items.insert(
                id.0.clone(),
                ItemDef {
                    id,
                    name: format!("{} Routine", ability.name),
                    description: ability.description.clone(),
                    routine: Some(ability.id.clone()),
                    bank_limit: None,
                    role: None,
                    equipment: None,
                    taming_potency: None,
                    consume: None,
                    craftable: None,
                    droppable: None,
                },
            );
        }
        warnings
    }
```

Add `use crate::abilities::AbilityDb;` to that file.

In `crates/engine/src/game/lifecycle.rs`, in `load_asset_dbs`, after the `ItemDb::load_dir` block and before the `missing_roles` check:

```rust
    let mut items = items;
    warnings.extend(items.synthesize_routines(&abilities));
```

(`items` is already `let (items, item_warnings) = ...`; make it `let (mut items, item_warnings) = ...` and drop the shadowing line.)

- [ ] **Step 5: Add the views**

In `crates/engine/src/views.rs`:

```rust
/// One row of an entity's routine panel — a slot, filled or not.
pub struct RoutineSlotView {
    pub index: usize,
    /// `None` for a free slot.
    pub ability: Option<crate::abilities::AbilityId>,
    /// The ability's name, or "(empty)" for a free slot.
    pub name: String,
    /// The ability's own authored description; empty for a free slot.
    pub description: String,
}

/// One row of the "whose routines?" picker — you and every program you own.
pub struct RoutineHolderView {
    pub entity: Entity,
    /// "You" for the player, the program's display name otherwise.
    pub name: String,
    pub level: u32,
    pub filled: usize,
    pub slots: usize,
}

/// One row of the install picker — a loose routine held in inventory.
pub struct RoutineItemView {
    pub item: ItemId,
    pub name: String,
    pub description: String,
    pub count: u32,
}
```

- [ ] **Step 6: Add the install/uninstall API**

Create `crates/engine/src/game/routines.rs`:

```rust
//! Installing, removing and inspecting routines — the abilities that occupy
//! a party member's slots. Extraction lives here too.

use crate::*;
use crate::components::Routines;

impl Game {
    /// Whether `item` is a loose routine rather than ordinary cargo.
    pub fn is_routine(&self, item: &ItemId) -> bool {
        self.world
            .resource::<ItemDb>()
            .get(item.as_str())
            .is_some_and(|d| d.routine.is_some())
    }

    /// `entity`'s slots in menu order, filled and empty alike.
    pub fn routine_view(&self, entity: Entity) -> Vec<RoutineSlotView> {
        let db = self.world.resource::<AbilityDb>();
        let installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        (0..self.routine_slots(entity))
            .map(|index| match installed.get(index).and_then(|id| db.get(id)) {
                Some(def) => RoutineSlotView {
                    index,
                    ability: Some(def.id.clone()),
                    name: def.name.clone(),
                    description: def.description.clone(),
                },
                None => RoutineSlotView {
                    index,
                    ability: None,
                    name: "(empty)".to_string(),
                    description: String::new(),
                },
            })
            .collect()
    }

    /// You, then every program you own — everyone who has routine slots.
    pub fn routine_holders(&mut self) -> Vec<RoutineHolderView> {
        let player = self.player_entity();
        let level = self
            .world
            .get::<Experience>(player)
            .map(|e| e.level)
            .unwrap_or(1);
        let mut holders = vec![RoutineHolderView {
            entity: player,
            name: "You".to_string(),
            level,
            filled: self
                .world
                .get::<Routines>(player)
                .map(|r| r.0.len())
                .unwrap_or(0),
            slots: self.routine_slots(player),
        }];
        for pet in self.owned_pets() {
            holders.push(RoutineHolderView {
                entity: pet.entity,
                name: pet.name.clone(),
                level: pet.level,
                filled: self
                    .world
                    .get::<Routines>(pet.entity)
                    .map(|r| r.0.len())
                    .unwrap_or(0),
                slots: self.routine_slots(pet.entity),
            });
        }
        holders
    }

    /// Loose routines held in inventory, id-sorted so the picker's numbering
    /// is stable between sessions.
    pub fn loose_routines(&self) -> Vec<RoutineItemView> {
        let db = self.world.resource::<ItemDb>();
        let Some(inv) = self.world.get::<Inventory>(self.player_entity()) else {
            return Vec::new();
        };
        let mut rows: Vec<RoutineItemView> = inv
            .items
            .iter()
            .filter(|(_, count)| *count > 0)
            .filter_map(|(item, count)| {
                let def = db.get(item.as_str())?;
                def.routine.as_ref()?;
                Some(RoutineItemView {
                    item: item.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    count: *count,
                })
            })
            .collect();
        rows.sort_by(|a, b| a.item.as_str().cmp(b.item.as_str()));
        rows
    }

    /// Spends one loose `item` and fills `entity`'s first free slot with the
    /// routine it carries. Free and unrestricted outside battle.
    pub fn install_routine(&mut self, entity: Entity, item: &ItemId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let ability = self
            .world
            .resource::<ItemDb>()
            .get(item.as_str())
            .and_then(|d| d.routine.clone())
            .ok_or_else(|| "That isn't a routine.".to_string())?;
        let player = self.player_entity();
        if self
            .world
            .get::<Inventory>(player)
            .map(|i| i.count(item))
            .unwrap_or(0)
            == 0
        {
            return Err("You don't have that routine.".into());
        }
        let mut installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        if installed.len() >= self.routine_slots(entity) {
            return Err("There's no free routine slot — pop one out first.".into());
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item.clone(), 1);
        installed.push(ability.clone());
        self.world.entity_mut(entity).insert(Routines(installed));
        let name = self.routine_holder_label(entity);
        let ability_name = self
            .world
            .resource::<AbilityDb>()
            .get(&ability)
            .map(|a| a.name.clone())
            .unwrap_or(ability);
        self.log(format!("{name} now runs {ability_name}."));
        Ok(())
    }

    /// Frees `slot` and returns its routine to inventory as an item.
    ///
    /// Checked for cargo room *before* the slot is cleared, for the reason
    /// `sell_item` documents about its own ordering: discovering there was
    /// no room afterwards would eat the routine.
    pub fn uninstall_routine(&mut self, entity: Entity, slot: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        let ability = installed
            .get(slot)
            .cloned()
            .ok_or_else(|| "That slot is empty.".to_string())?;
        let item = abilities::routine_item_id(&ability);
        self.check_room(&item, 1)?;

        let mut installed = installed;
        installed.remove(slot);
        self.world.entity_mut(entity).insert(Routines(installed));
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(item, 1);
        let name = self.routine_holder_label(entity);
        self.log(format!("{name} stops running that routine."));
        Ok(())
    }

    /// "You" for the player, the program's display name otherwise — the one
    /// place that distinction is worded, so every routine log line reads the
    /// same.
    pub(crate) fn routine_holder_label(&self, entity: Entity) -> String {
        if entity == self.player_entity() {
            "You".to_string()
        } else {
            self.creature_label(entity)
        }
    }
}
```

Declare it in `crates/engine/src/game/mod.rs`:

```rust
mod routines;
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p feral-processes-engine tests::routines`
Expected: PASS — all nine.

Run: `cargo test --workspace`
Expected: PASS. `items_db::tests::the_shipped_items_load_cleanly_with_all_roles_and_fields` still passes: synthesis happens in `load_asset_dbs`, not `load_dir`, so that test's `db.all().count() == 36` is unaffected.

- [ ] **Step 8: Commit**

```bash
git add crates/engine/src
git commit -m "feat: loose routines are inventory items you can install and pop out"
```

---

### Task 5: Extraction

**Files:**
- Modify: `crates/engine/src/structures.rs` (`extracts_routines`)
- Modify: `assets/structures/compiler.ron`
- Modify: `crates/engine/src/game/routines.rs`
- Modify: `assets/structures/README.md`
- Test: `crates/engine/src/tests/routines.rs`

**Interfaces:**
- Consumes: `Game::is_routine`, `routine_view` (Task 4); `Game::has_structure` (`crates/engine/src/game/crafting.rs:56`, unchanged).
- Produces:
  - `StructureDef::extracts_routines: bool`
  - `Game::can_extract_routines(&self) -> bool`
  - `Game::extractable_routines(&self, creature: Entity) -> Vec<AbilityDef>`
  - `Game::extract_routine(&mut self, creature: Entity, index: usize) -> Result<(), String>`

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/routines.rs`:

```rust
#[test]
fn extraction_needs_a_bench_built_somewhere_but_not_nearby() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.can_extract_routines(), "no bench, no extraction");

    let pet = spawn_tamed(&mut game, 10, 3);
    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(err.contains("Compiler"), "the refusal should name the bench: {err}");

    spawn_structure_at(&mut game, "compiler", 30, 30);
    assert!(
        game.can_extract_routines(),
        "a bench 30 tiles away still counts — extraction has no proximity rule"
    );
}

#[test]
fn extracting_yields_the_picked_routine_destroys_the_program_and_loses_the_rest() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 5); // both of its unlocks installed
    spawn_structure_at(&mut game, "compiler", 30, 30);

    let offered = game.extractable_routines(medic);
    assert_eq!(offered.len(), 2, "both installed routines are on offer");
    let kept = offered[1].id.clone();
    let lost = offered[0].id.clone();

    game.extract_routine(medic, 1).unwrap();

    assert_eq!(
        count_item(&game, crate::abilities::routine_item_id(&kept).as_str()),
        1,
        "the picked routine lands in inventory"
    );
    assert_eq!(
        count_item(&game, crate::abilities::routine_item_id(&lost).as_str()),
        0,
        "everything else on the program is lost with it"
    );
    assert!(
        game.owned_pets().iter().all(|p| p.entity != medic),
        "the program is consumed"
    );
}

#[test]
fn extraction_is_refused_for_a_program_you_dont_own_and_during_battle() {
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_structure_at(&mut game, "compiler", 30, 30);
    let wild = spawn_wild_on_player_tile(&mut game);
    let err = game.extract_routine(wild, 0).unwrap_err();
    assert!(err.contains("control"), "{err}");

    let pet = spawn_tamed(&mut game, 10, 3);
    start_battle_with_a_wild_program(&mut game);
    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(err.contains("right now"), "{err}");
}
```

Add the helper to `crates/engine/src/tests/support.rs`, beside `spawn_data_cache`:

```rust
/// Deploys a structure of `kind` at an absolute position, bypassing
/// `place_structure`'s Home, cost and distance rules — for tests about what
/// a standing structure *enables*, not about the build rules.
pub(super) fn spawn_structure_at(game: &mut Game, kind: &str, x: i32, y: i32) {
    game.world.spawn((
        Structure {
            kind: kind.to_string(),
        },
        Position { x, y },
    ));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine tests::routines::extraction`
Expected: FAIL — `no method named 'can_extract_routines'`.

- [ ] **Step 3: Add the structure field and set it**

In `crates/engine/src/structures.rs`, in `StructureDef`:

```rust
    /// If true, owning one of these anywhere lets you extract a routine out
    /// of a program you own (see `Game::extract_routine`). Deliberately
    /// ownership, not proximity: the check is `Game::has_structure`, the
    /// same "have you built one" test a researched recipe's bench uses.
    /// `#[serde(default)]` so existing structure files (including mods)
    /// grant no extraction, exactly as before this field existed.
    #[serde(default)]
    pub extracts_routines: bool,
```

In `assets/structures/compiler.ron`, add `extracts_routines: true,` after `work:`.

- [ ] **Step 4: Add the extraction API**

Append to `impl Game` in `crates/engine/src/game/routines.rs`:

```rust
    /// Whether a routine-extraction bench is standing anywhere. Ownership,
    /// not proximity — see `StructureDef::extracts_routines`.
    pub fn can_extract_routines(&self) -> bool {
        self.world
            .resource::<StructureDb>()
            .all()
            .filter(|def| def.extracts_routines)
            .any(|def| self.has_structure(&def.id))
    }

    /// Display name of a bench that would allow extraction, for the refusal
    /// message — no code names a structure id.
    fn extraction_bench_name(&self) -> String {
        self.world
            .resource::<StructureDb>()
            .all()
            .find(|def| def.extracts_routines)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| "an extraction bench".to_string())
    }

    /// The routines installed on `creature`, in slot order — what an
    /// extraction offers to salvage.
    pub fn extractable_routines(&self, creature: Entity) -> Vec<AbilityDef> {
        let db = self.world.resource::<AbilityDb>();
        self.world
            .get::<Routines>(creature)
            .map(|r| r.0.clone())
            .unwrap_or_default()
            .iter()
            .filter_map(|id| db.get(id).cloned())
            .collect()
    }

    /// Destroys `creature` and salvages exactly one of its routines — the
    /// one at `index` in `extractable_routines`. Everything else installed
    /// on it is lost with it.
    ///
    /// Room for the payout is checked before the program is despawned, for
    /// the reason `sell_companion` documents about its own ordering.
    pub fn extract_routine(&mut self, creature: Entity, index: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.can_extract_routines() {
            return Err(format!(
                "You need {} standing somewhere to extract a routine.",
                self.extraction_bench_name()
            ));
        }
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        let ability = self
            .extractable_routines(creature)
            .get(index)
            .map(|def| def.id.clone())
            .ok_or_else(|| "That program has no such routine.".to_string())?;
        let item = abilities::routine_item_id(&ability);
        self.check_room(&item, 1)?;

        let name = self.creature_label(creature);
        for detached in self.sale_detachments(creature) {
            self.log(format!("{name} {detached}."));
        }
        self.world
            .resource_mut::<Party>()
            .0
            .retain(|&e| e != creature);
        self.world.entity_mut(creature).remove::<Task>();
        self.world.despawn(creature);
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(item, 1);
        let ability_name = self
            .world
            .resource::<AbilityDb>()
            .get(&ability)
            .map(|a| a.name.clone())
            .unwrap_or(ability);
        self.log(format!(
            "You break {name} down and salvage its {ability_name} routine."
        ));
        self.tick();
        Ok(())
    }
```

`sale_detachments` is `pub(crate)` on `Game` (`crates/engine/src/game/trade.rs`). If it is private to that module, widen it to `pub(crate)`.

- [ ] **Step 5: Document the field**

In `assets/structures/README.md`, add:

```markdown
| `extracts_routines` | bool | `false` | Owning one of these anywhere lets you extract a routine from a program you own. No proximity requirement. |
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p feral-processes-engine tests::routines`
Expected: PASS — all twelve.

Run: `cargo test --workspace && cargo test -p feral-processes-engine balance_sim`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add assets crates/engine/src
git commit -m "feat: extract a routine from a program you own, consuming it"
```

---

### Task 6: The player reads routines, and research grants them

Deletes `Game::player_abilities`. After this the player's Special comes from `Routines` like everyone else's — and is empty until Task 7 pre-installs decompile, so the Special row hides itself.

**Files:**
- Modify: `crates/engine/src/game/combat.rs:414-446` (delete `player_abilities`, collapse `actor_abilities`), `:572-585` (hide the Special row)
- Modify: `crates/engine/src/game/unlocks.rs:178-215` (`unlock_research`)
- Modify: `crates/engine/src/game/party.rs:122-134` (`ability_label`)
- Modify: `crates/engine/src/research.rs:40-50` (doc comment pointing at `Game::player_abilities`)
- Modify: `crates/engine/src/tests/combat_abilities.rs:395-595` (the `player_abilities` tests)
- Test: `crates/engine/src/tests/routines.rs`

**Interfaces:**
- Consumes: `Routines` (Task 2), `abilities::routine_item_id`, `Game::check_room` (Task 4).
- Produces: `Game::player_abilities` **no longer exists**. `Game::actor_abilities` is the single read path for both sides.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/routines.rs`:

```rust
#[test]
fn researching_a_node_grants_routine_items_rather_than_the_ability_itself() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = game
        .research_nodes()
        .into_iter()
        .find(|n| !n.unlocks_abilities.is_empty())
        .expect("some shipped node grants an ability");
    let ability = node.unlocks_abilities[0].clone();

    unlock_research_chain(&mut game, &node.id);

    assert_eq!(
        count_item(&game, crate::abilities::routine_item_id(&ability).as_str()),
        1,
        "the routine arrives as an item, not as an installed ability"
    );
    assert!(
        game.actor_abilities(game.player_entity()).is_empty(),
        "researching does not install — that is a separate act"
    );
}

#[test]
fn a_member_with_no_routines_is_offered_no_special_at_all() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    assert!(
        game.battle_action_options(0)
            .iter()
            .all(|o| o.kind != ActionKind::Special),
        "the row is hidden, not greyed, when nothing is installed"
    );
}
```

`ResearchStatus` needs `unlocks_abilities` and `id` exposed for that first test. `id` already exists; add to `views::ResearchStatus`:

```rust
    /// Abilities this node hands over as routine items when researched.
    pub unlocks_abilities: Vec<crate::abilities::AbilityId>,
```

and populate it in `Game::research_nodes` from `def.unlocks_abilities.clone()`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine tests::routines::researching`
Expected: FAIL — `no field 'unlocks_abilities' on type 'ResearchStatus'`, then a count of 0 rather than 1.

- [ ] **Step 3: Collapse `actor_abilities`**

`companion_abilities` and `actor_abilities` now do the same thing, so there should be one function. Delete `player_abilities` (lines 414-434) and `companion_abilities` (lines 388-412) outright, and leave `actor_abilities` as:

```rust
    /// Every ability the combatant at `entity` can be commanded to use, in
    /// menu order: whatever is installed in its routine slots. Menu and
    /// resolution both go through this, so the two cannot disagree about
    /// what a slot knows.
    ///
    /// May be empty for anyone — a member with nothing installed is offered
    /// no Special at all (see `battle_action_options`). A companion's kit is
    /// installed at tame/fuse time and topped up on the level-ups that reach
    /// a species unlock (`install_innate_routines`,
    /// `install_unlocked_routines`); nothing is resolved here.
    pub(crate) fn actor_abilities(&self, entity: Entity) -> Vec<AbilityDef> {
        let db = self.world.resource::<AbilityDb>();
        self.world
            .get::<Routines>(entity)
            .map(|r| r.0.as_slice())
            .unwrap_or_default()
            .iter()
            .filter_map(|id| db.get(id).cloned())
            .collect()
    }
```

Four doc comments name `Game::companion_abilities` and must be repointed at `Game::actor_abilities`: `crates/engine/src/tests/support.rs:529`, `crates/engine/src/species.rs:106` (rewritten in Task 2 — check it), `crates/engine/src/battle.rs:84`, `crates/engine/src/battle.rs:180`. `grep -rn "companion_abilities" crates/` must come back empty when this step is done.

- [ ] **Step 4: Hide the Special row when nothing is installed**

In `crates/engine/src/game/combat.rs:572-585`, replace the unconditional `options.push(...)` for `ActionKind::Special` with:

```rust
        // Hidden, not greyed: with routines installable at will, an empty
        // kit is a state the player chose, and a permanently greyed row
        // teaches nothing they don't already know.
        if !self.actor_abilities(entity).is_empty() {
            options.push(ActionOption {
                kind: ActionKind::Special,
                key: 's',
                label: "[s]pecial".to_string(),
                detail: self.ability_label(entity),
                target: TargetSpec::SpecialAbility,
                unavailable: None,
            });
        }
```

- [ ] **Step 5: Update `ability_label`**

`crates/engine/src/game/party.rs:122-134`:

```rust
    /// Terse label for what commanding `entity` in battle would do right
    /// now. A member with several routines reads as a count, since no one of
    /// them is *the* answer until the player picks in `Mode::BattleSpecial`.
    pub(crate) fn ability_label(&self, entity: Entity) -> String {
        match self.actor_abilities(entity).as_slice() {
            // Anyone can be empty now: an innate routine can be popped out,
            // and the player starts with only decompile installed.
            [] => "No routines installed".to_string(),
            [only] => only.name.clone(),
            many => format!("{} routines", many.len()),
        }
    }
```

- [ ] **Step 6: Grant routine items on research**

In `crates/engine/src/game/unlocks.rs`, inside `unlock_research`, after the prerequisite check and **before** the Research Data is spent:

```rust
        // Checked before anything is spent: a researched routine that can't
        // fit in cargo would otherwise be lost outright, and there is no
        // second chance to take a node.
        for ability in &def.unlocks_abilities {
            self.check_room(&abilities::routine_item_id(ability), 1)?;
        }
```

and after `self.log(format!("Research complete: {}.", def.name));`:

```rust
        for ability in &def.unlocks_abilities {
            let item = abilities::routine_item_id(ability);
            let name = self.item_name(&item);
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .add(item, 1);
            self.log(format!("A {name} is compiled into your cargo."));
        }
```

Update the doc comment at `crates/engine/src/research.rs:40-50` — it currently points at `Game::player_abilities`:

```rust
    /// Abilities this node hands over, as routine items dropped into cargo
    /// the moment it is researched (see `Game::unlock_research`). Researching
    /// a routine and installing it are two separate acts. The abilities
    /// themselves are data in `assets/abilities/`.
```

- [ ] **Step 7: Rewrite the `player_abilities` tests**

In `crates/engine/src/tests/combat_abilities.rs`, the tests at lines 395-595 assert on `game.player_abilities()`. Convert each: replace `game.player_abilities()` with `game.actor_abilities(game.player_entity())`, and where a test researched a node and then expected the ability to be commandable, add an explicit install:

```rust
    let item = crate::abilities::routine_item_id("priority_boost");
    game.install_routine(game.player_entity(), &item).unwrap();
```

The test at line 584 (`game.player_abilities().is_empty()`) becomes an assertion that an un-installed player has no Special, which the new `a_member_with_no_routines_is_offered_no_special_at_all` already covers — delete it rather than keeping a duplicate.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p feral-processes-engine tests::routines tests::combat_abilities tests::research`
Expected: PASS.

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src
git commit -m "feat: the player's kit is installed routines, research grants the items"
```

---

### Task 7: Decompile becomes an ability

**Files:**
- Create: `assets/abilities/decompile.ron`
- Modify: `crates/engine/src/abilities.rs` (`AbilityEffect::Decompile`, `DECOMPILE_ABILITY_ID`)
- Modify: `crates/engine/src/game/lifecycle.rs` (startup validation, player's starting `Routines`)
- Modify: `crates/engine/src/battle.rs:78-100`, `:134-142` (delete both `Decompile` variants)
- Modify: `crates/engine/src/game/combat.rs:301-345` (`battle_set_action` target arm), `:467-492` (`ability_unavailable`), `:587-602` (delete the Decompile option)
- Modify: `crates/engine/src/game/combat_round.rs:109-175` (`resolve_one_action`), `:241-273` (`action_label`)
- Modify: `crates/engine/src/game/combat_rewards.rs:188-262` (`attempt_decompile` signature)
- Modify: `crates/app-core/src/lib.rs:332-349` (`action_from`)
- Modify: `assets/abilities/README.md`
- Test: `crates/engine/src/tests/routines.rs`, `crates/engine/src/tests/taming.rs`, `crates/engine/src/tests/combat.rs:186-200` (the action-key table, which pins `ActionKind::Decompile` to `'c'`)

**Interfaces:**
- Consumes: `Routines` (Task 2), `Game::install_routine` (Task 4), `actor_abilities` (Task 6).
- Produces:
  - `abilities::DECOMPILE_ABILITY_ID: &str = "decompile"`
  - `AbilityEffect::Decompile` (no fields)
  - `Game::attempt_decompile(&mut self, group: usize, player: Entity) -> bool` — returns whether the battle ended. The `Option` wrapper is gone.
  - `BattleAction::Decompile` and `ActionKind::Decompile` **no longer exist**.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/routines.rs`:

```rust
#[test]
fn a_new_game_starts_with_decompile_installed_in_the_players_only_slot() {
    let game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let slots = game.routine_view(game.player_entity());
    assert_eq!(slots.len(), 1, "level 1 gives the player exactly one slot");
    assert_eq!(
        slots[0].ability.as_deref(),
        Some(crate::abilities::DECOMPILE_ABILITY_ID)
    );
}

#[test]
fn decompile_is_reached_through_special_not_its_own_command() {
    let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    let options = game.battle_action_options(0);
    assert!(
        options.iter().any(|o| o.kind == ActionKind::Special),
        "the player's Special row carries decompile"
    );
    assert!(
        game.battle_special_options(0)
            .iter()
            .any(|o| o.name.to_lowercase().contains("decompile")),
        "decompile is one of the abilities on offer"
    );
}

#[test]
fn decompile_greys_with_a_reason_rather_than_refunding_the_round() {
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_inventory(&mut game, &[]); // no taming catalyst
    start_battle_with_a_wild_program(&mut game);
    let row = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("decompile is installed");
    assert_eq!(row.unavailable.as_deref(), Some("no taming catalyst"));

    let err = game
        .battle_set_action(
            0,
            BattleAction::Special {
                ability: row.index,
                target: battle::SpecialTarget::EnemyGroup { group: 0 },
            },
        )
        .unwrap_err();
    assert!(err.contains("no taming catalyst"), "{err}");
}

#[test]
fn popping_decompile_out_leaves_the_player_with_no_special() {
    let mut game = Game::new(54, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.uninstall_routine(game.player_entity(), 0).unwrap();
    start_battle_with_a_wild_program(&mut game);
    assert!(
        game.battle_action_options(0)
            .iter()
            .all(|o| o.kind != ActionKind::Special),
        "giving up decompile really does cost you the command"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine tests::routines::a_new_game_starts`
Expected: FAIL — `cannot find value 'DECOMPILE_ABILITY_ID'`.

- [ ] **Step 3: Add the effect variant, the id, and the asset**

In `crates/engine/src/abilities.rs`, add to `AbilityEffect`:

```rust
    /// Spends a taming catalyst and rolls `taming::capture_chance` against
    /// the target group's front program — see `Game::attempt_decompile`.
    /// Carries no numbers of its own: the whole formula is `taming`'s, and
    /// duplicating any of it here would be a second copy to drift.
    Decompile,
```

and beside `FALLBACK_ABILITY_ID`:

```rust
/// The ability a new game pre-installs into the player's first routine slot
/// — capturing a program is reached through the Special menu like anything
/// else. Validated at startup the same way `FALLBACK_ABILITY_ID` is.
pub const DECOMPILE_ABILITY_ID: &str = "decompile";
```

`AbilityDef::non_finite_field` needs no new arm — `Decompile` carries no floats.

Create `assets/abilities/decompile.ron`:

```ron
(
    id: "decompile",
    name: "Decompile",
    description: "Spend a taming catalyst to break a group's front program and run it yourself.",
    target: OneEnemyGroupFront,
    effect: Decompile,
    cooldown: 0,
    // Zero, deliberately: capturing has never cost Fatigue, and folding it
    // into the ability system is not the place to start charging for it.
    fatigue_cost: 0.0,
)
```

In `crates/engine/src/game/lifecycle.rs`'s `load_asset_dbs`, extend the existing fallback check to cover both mandatory abilities:

```rust
    for required in [
        abilities::FALLBACK_ABILITY_ID,
        abilities::DECOMPILE_ABILITY_ID,
    ] {
        if abilities.get(required).is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "ability set is missing the mandatory ability {required:?} — the game \
                     pre-installs it and cannot start without it"
                ),
            ));
        }
    }
```

And give the player their starting routine in `Game::new`, replacing `Routines::default()`:

```rust
                Routines(vec![abilities::DECOMPILE_ABILITY_ID.to_string()]),
```

- [ ] **Step 4: Move the refusals into `ability_unavailable`**

In `crates/engine/src/game/combat.rs`, extend `ability_unavailable` — after the cooldown check, before the fatigue check:

```rust
        // Decompile is refused for two reasons no other ability has. They
        // used to live in `attempt_decompile`, which refunded the round
        // silently; here the row greys with the reason instead.
        if matches!(ability.effect, AbilityEffect::Decompile) {
            if self.taming_catalyst().is_none() {
                return Some("no taming catalyst".to_string());
            }
            if self.pet_count() >= self.pet_capacity() {
                return Some("roster is full".to_string());
            }
        }
```

Delete the whole `ActionKind::Decompile` `options.push(...)` block from `battle_action_options` (lines 587-602), keeping the `UseItem` block that follows it.

- [ ] **Step 5: Delete the bespoke action and route resolution**

In `crates/engine/src/battle.rs`, delete the `Decompile { group: usize }` arm from `BattleAction` and the `Decompile` variant from `ActionKind`. Update the doc comments at `:84` and `:180` that name `Game::companion_abilities` to name `Game::actor_abilities`.

In `crates/engine/src/game/combat.rs:310-318`, the target-group match loses its `Decompile` alternative:

```rust
        let target_group = match &action {
            BattleAction::Attack { group } => Some(*group),
            // A party-facing Special has no group to validate at all.
            BattleAction::Special {
                target: battle::SpecialTarget::EnemyGroup { group },
                ..
            } => Some(*group),
            _ => None,
        };
```

In `crates/engine/src/game/combat_round.rs`, delete the `BattleAction::Decompile` arm from `resolve_one_action` and from `action_label`. In the `BattleAction::Special` arm, after fatigue is paid, split on the effect:

```rust
                    // Decompile needs the *group index*, not the recipient
                    // entity: a successful capture drops the target out of
                    // its group. Every other effect only ever touches the
                    // recipients it lands on.
                    if matches!(ability.effect, AbilityEffect::Decompile) {
                        if let battle::SpecialTarget::EnemyGroup { group } = target
                            && let Some(group) = self.retarget(group)
                        {
                            self.attempt_decompile(group, player);
                        }
                    } else {
                        let recipients = self.ability_recipients(ability.target, &target);
                        self.use_ability(&ability, entity, &name, &recipients);
                        // An area effect can drop members from any rank, and
                        // a corpse left in a group would be promoted to front
                        // and then attacked as though alive.
                        self.reap_dead_members(player);
                    }
```

In `crates/engine/src/game/combat_rewards.rs`, change `attempt_decompile`'s signature and both early returns:

```rust
    /// One decompile attempt against `group`'s front program: spends a
    /// catalyst, rolls `taming::capture_chance`, and on success converts the
    /// target into a tamed program and drops it from the group. Returns
    /// whether that ended the battle.
    ///
    /// The roster-full and no-catalyst refusals live in
    /// `ability_unavailable` now: a greyed row can't be planned, and
    /// `battle_set_action` refuses one that somehow is, so neither state can
    /// reach a resolving round.
    pub(crate) fn attempt_decompile(&mut self, group: usize, player: Entity) -> bool {
```

**Delete both early-return guards** — the roster-capacity block (lines ~197-206) and the `taming_catalyst()` `else` arm's refusal. They guard states `ability_unavailable` now makes unreachable, and this repo does not write error handling for scenarios that can't happen. The catalyst lookup still has to happen, so keep it as a plain destructure:

```rust
        let (catalyst, potency) = self
            .taming_catalyst()
            .expect("ability_unavailable greys decompile when no catalyst is held");
```

Then `let Some(front) = self.front_of_group(group) else { return false; };` for the `?`, `return Some(false)` → `return false`, `return Some(true)` → `return true`, and the trailing `Some(false)` → `false`.

In `crates/app-core/src/lib.rs:346`, delete the `ActionKind::Decompile` arm from `action_from`.

- [ ] **Step 6: Document the new effect**

In `assets/abilities/README.md`, add `Decompile` to the `effect` variant table:

```markdown
| `Decompile` | none | Spends a taming catalyst and rolls a capture against the target group's front program. Only meaningful with `target: OneEnemyGroupFront`. Greys out with a reason when you hold no catalyst or your roster is full. |
```

Also note in that README that `priority_boost` and `decompile` are both mandatory — startup aborts without them.

- [ ] **Step 7: Fix the taming tests**

`crates/engine/src/tests/support.rs:90`'s `player_decompiles` builds a `BattleAction::Decompile`. Rewrite it:

```rust
/// Plans the player's decompile as the Special it now is, and resolves the
/// round.
pub(super) fn player_decompiles(game: &mut Game) {
    let index = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the player starts with decompile installed")
        .index;
    resolve_round_with(
        game,
        BattleAction::Special {
            ability: index,
            target: crate::battle::SpecialTarget::EnemyGroup { group: 0 },
        },
    );
}
```

Any test in `crates/engine/src/tests/taming.rs` or `combat_rewards.rs` that asserts on `attempt_decompile`'s `Option` return needs its expectation flattened (`Some(true)` → `true`).

`crates/engine/src/tests/combat.rs:189-199` builds a table of `ActionKind` → hotkey and asserts `key_for(ActionKind::Decompile) == 'c'`. Delete that one assertion; the remaining four (`Attack`/`Defend`/`Special`/`UseItem`) still hold, and `'c'` is now unbound in battle.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p feral-processes-engine tests::routines tests::taming tests::combat_rewards`
Expected: PASS.

Run: `cargo test --workspace && cargo test -p feral-processes-engine balance_sim`
Expected: PASS. `balance_sim` never modelled decompile, so its curves must not move — if they do, something else changed.

- [ ] **Step 9: Commit**

```bash
git add assets crates
git commit -m "feat: decompile is an ability the player starts with installed"
```

---

### Task 8: The screens

Six new modes. Install/uninstall is reached from the routines panel (not the inventory item page), so there is exactly one path to each action.

**Files:**
- Modify: `crates/app-core/src/lib.rs:161-302` (`Mode` + `is_battle`)
- Create: `crates/app-core/src/app/routines.rs`
- Modify: `crates/app-core/src/app/mod.rs`, `crates/app-core/src/app/input.rs`, `crates/app-core/src/app/playing.rs`
- Create: `crates/gui/src/render/routines.rs`
- Modify: `crates/gui/src/render/mod.rs`
- Modify: `crates/gui/src/render/meta.rs` (the help screen's key list)
- Test: `crates/app-core/src/tests/`

**Interfaces:**
- Consumes: `Game::routine_holders`, `routine_view`, `loose_routines`, `install_routine`, `uninstall_routine`, `can_extract_routines`, `extractable_routines`, `extract_routine` (Tasks 4-5).
- Produces: `Mode::{RoutineTarget, Routines, RoutineInstall, Extract, ExtractPick, ExtractConfirm}` and `App` fields `pending_routine_holder: Option<Entity>`, `pending_routine_slot: Option<usize>`, `pending_extract_program: Option<Entity>`, `pending_extract_index: Option<usize>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/app-core/src/tests/routines.rs` (register it in that directory's `mod.rs`):

```rust
use super::support::*;
use crate::*;

#[test]
fn m_opens_the_routine_target_picker_and_esc_backs_all_the_way_out() {
    let mut app = test_app(61);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(app.mode, Mode::RoutineTarget);
    app.handle_key(GameKey::Char('1')); // "You"
    assert_eq!(app.mode, Mode::Routines);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::RoutineTarget);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn picking_a_filled_slot_uninstalls_and_picking_an_empty_one_opens_the_install_list() {
    let mut app = test_app(62);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('1')); // You — slot 1 holds decompile
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Routines, "uninstalling stays on the panel");
    let game = app.game.as_ref().unwrap();
    assert!(
        game.routine_view(game.player_entity())[0].ability.is_none(),
        "decompile should have been popped out"
    );

    app.handle_key(GameKey::Char('1')); // now an empty slot
    assert_eq!(app.mode, Mode::RoutineInstall);
}

#[test]
fn the_extract_flow_requires_confirmation_before_the_program_is_destroyed() {
    let mut app = app_owning_a_program_and_a_compiler(63);
    app.handle_key(GameKey::Char('M'));
    assert_eq!(app.mode, Mode::Extract);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::ExtractPick);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::ExtractConfirm);

    let before = app.game.as_mut().unwrap().owned_pets().len();
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        before,
        "backing out must not destroy the program"
    );

    app.handle_key(GameKey::Char('M'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.game.as_mut().unwrap().owned_pets().len(),
        before - 1,
        "confirming consumes it"
    );
}
```

Add this to `crates/app-core/src/tests/support.rs`. It follows `app_owning_distant_programs`' save-edit-and-reload trick, which exists because the engine deliberately exposes no way to hand-place a tamed program or a structure from outside the crate:

```rust
/// A game where the player owns one tamed program and has a Compiler
/// standing, so the extraction flow has both of its preconditions. Built by
/// editing a save and reloading it, for the same reason
/// `app_owning_distant_programs` is.
pub(crate) fn app_owning_a_program_and_a_compiler(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_extract_{seed}.sav"));
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        species,
        position: (px + 1, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        def: 1,
        tamed: true,
        level: 1,
        xp: 0,
        xp_to_next: 20,
        cronjob: None,
        party_slot: None,
        zone: 1,
        custom_name: None,
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        fusions: 0,
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
    });
    data.structures.push(save::StructureSave {
        kind: "compiler".to_string(),
        position: (px + 30, py + 30),
        resource_amount: None,
        durability: None,
        tier: None,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}
```

Copy the `CreatureSave` field list from `app_owning_distant_programs` if it has drifted — that helper is the reference for the current shape.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-app-core routines`
Expected: FAIL — `no variant named 'RoutineTarget' found for enum 'Mode'`.

- [ ] **Step 3: Add the modes**

In `crates/app-core/src/lib.rs`, add after `Mode::FuseName`:

```rust
    /// Picking whose routines to manage — you, or any program you own.
    /// Reached with `m` from `Mode::Playing`.
    RoutineTarget,
    /// The chosen member's slot list. A filled slot pops its routine back
    /// into cargo; an empty one opens `Mode::RoutineInstall`.
    Routines,
    /// Picking which loose routine to drop into the slot chosen in
    /// `Mode::Routines`.
    RoutineInstall,
    /// Picking which program to break down for a routine. Reached with `M`
    /// from `Mode::Playing`.
    Extract,
    /// Picking which of that program's routines to salvage.
    ExtractPick,
    /// Confirming the extraction. Programs take a confirmation for the same
    /// reason a sale does: it is irreversible, and every *other* routine on
    /// the program is lost with it — this screen is the only place that is
    /// said out loud.
    ExtractConfirm,
```

Add all six to the `false` arm of `Mode::is_battle`. That match is exhaustive on purpose (see its doc comment) — the compiler will demand it.

Add the four pending fields to `App` beside `pending_fuse_first`, defaulted to `None` in its constructor.

- [ ] **Step 4: Handle the keys**

Create `crates/app-core/src/app/routines.rs`:

```rust
//! The routine panel — installing and popping out abilities — and the
//! three-page extraction flow.

use crate::*;

impl App {
    pub(crate) fn handle_routine_target_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let holders = game.routine_holders();
        if let Some(idx) = self.selected_index(key, holders.len()) {
            self.pending_routine_holder = Some(holders[idx].entity);
            self.mode = Mode::Routines;
        }
    }

    pub(crate) fn handle_routines_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_routine_holder = None;
            self.mode = Mode::RoutineTarget;
            return;
        }
        let Some(entity) = self.pending_routine_holder else {
            self.mode = Mode::RoutineTarget;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let slots = game.routine_view(entity);
        let Some(idx) = self.selected_index(key, slots.len()) else {
            return;
        };
        if slots[idx].ability.is_some() {
            match game.uninstall_routine(entity, idx) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            return;
        }
        self.pending_routine_slot = Some(idx);
        self.mode = Mode::RoutineInstall;
    }

    pub(crate) fn handle_routine_install_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_routine_slot = None;
            self.mode = Mode::Routines;
            return;
        }
        let Some(entity) = self.pending_routine_holder else {
            self.mode = Mode::RoutineTarget;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let loose = game.loose_routines();
        if let Some(idx) = self.selected_index(key, loose.len()) {
            let item = loose[idx].item.clone();
            match game.install_routine(entity, &item) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.pending_routine_slot = None;
            self.mode = Mode::Routines;
        }
    }

    pub(crate) fn handle_extract_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let programs = game.owned_pets();
        if let Some(idx) = self.selected_index(key, programs.len()) {
            self.pending_extract_program = Some(programs[idx].entity);
            self.mode = Mode::ExtractPick;
        }
    }

    pub(crate) fn handle_extract_pick_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_extract_program = None;
            self.mode = Mode::Extract;
            return;
        }
        let Some(program) = self.pending_extract_program else {
            self.mode = Mode::Extract;
            return;
        };
        let Some(game) = &self.game else { return };
        let offered = game.extractable_routines(program);
        if let Some(idx) = self.selected_index(key, offered.len()) {
            self.pending_extract_index = Some(idx);
            self.mode = Mode::ExtractConfirm;
        }
    }

    /// Enter destroys the program; anything else backs out. Deliberately not
    /// a numbered menu — this is the last stop before an irreversible act.
    pub(crate) fn handle_extract_confirm_key(&mut self, key: GameKey) {
        let (Some(program), Some(index)) =
            (self.pending_extract_program, self.pending_extract_index)
        else {
            self.mode = Mode::Extract;
            return;
        };
        if key != GameKey::Enter {
            self.pending_extract_index = None;
            self.mode = Mode::ExtractPick;
            return;
        }
        self.pending_extract_program = None;
        self.pending_extract_index = None;
        self.mode = Mode::Playing;
        let Some(game) = &mut self.game else { return };
        match game.extract_routine(program, index) {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
    }
}
```

Declare it in `crates/app-core/src/app/mod.rs` (`mod routines;`), add the six dispatch arms to `handle_key` in `input.rs`, and add the two openers to `playing.rs`:

```rust
            GameKey::Char('m') => {
                self.mode = Mode::RoutineTarget;
                return;
            }
            GameKey::Char('M') => {
                self.mode = Mode::Extract;
                return;
            }
```

Both keys are currently unbound on `Mode::Playing` — verified against the full match at `crates/app-core/src/app/playing.rs:7-87`.

- [ ] **Step 5: Draw them**

Create `crates/gui/src/render/routines.rs` following `crates/gui/src/render/party.rs`'s shape exactly — `draw_popup(title, PopupSize::Large, &rows, fonts, m)` built from `text_row`/`item_row`, one function per mode:

```rust
//! The routine panel and the extraction flow.

use super::popup::*;
use super::*;

pub(super) fn draw_routine_target(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let holders = game.routine_holders();
    let mut rows = vec![text_row("Whose routines?")];
    for (i, h) in holders.iter().enumerate() {
        rows.push(item_row(
            format!(
                "[{}] {} Lv{} - {}/{} slots",
                menu_shortcut(i),
                h.name,
                h.level,
                h.filled,
                h.slots
            ),
            i == selected,
        ));
    }
    draw_popup("Routines", PopupSize::Large, &rows, fonts, m);
}

pub(super) fn draw_routines(
    game: &Game,
    holder: Option<Entity>,
    selected: usize,
    fonts: &Fonts,
    m: &Metrics,
) {
    let Some(holder) = holder else { return };
    let slots = game.routine_view(holder);
    let mut rows = vec![text_row(
        "Pick a filled slot to pop its routine back into cargo, or an empty one to install.",
    )];
    for (i, s) in slots.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), s.name),
            i == selected,
        ));
        if !s.description.is_empty() {
            rows.push(text_row(format!("    {}", s.description)));
        }
    }
    draw_popup("Routines", PopupSize::Large, &rows, fonts, m);
}

pub(super) fn draw_routine_install(game: &Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let loose = game.loose_routines();
    let mut rows = vec![text_row("Install which routine?")];
    if loose.is_empty() {
        rows.push(text_row(
            "(no loose routines — research one, or extract one from a program)",
        ));
    }
    for (i, r) in loose.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {} x{}", menu_shortcut(i), r.name, r.count),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", r.description)));
    }
    draw_popup("Install Routine", PopupSize::Large, &rows, fonts, m);
}

pub(super) fn draw_extract(game: &mut Game, selected: usize, fonts: &Fonts, m: &Metrics) {
    let programs = game.owned_pets();
    let mut rows = vec![text_row(
        "Break down which program? Extraction destroys it and salvages one routine.",
    )];
    if !game.can_extract_routines() {
        rows.push(text_row("(you need a Compiler standing somewhere first)"));
    }
    for (i, p) in programs.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {} Lv{}", menu_shortcut(i), p.name, p.level),
            i == selected,
        ));
    }
    draw_popup("Extract", PopupSize::Large, &rows, fonts, m);
}

pub(super) fn draw_extract_pick(
    game: &Game,
    program: Option<Entity>,
    selected: usize,
    fonts: &Fonts,
    m: &Metrics,
) {
    let Some(program) = program else { return };
    let offered = game.extractable_routines(program);
    let mut rows = vec![text_row("Salvage which routine? The rest are lost with it.")];
    for (i, a) in offered.iter().enumerate() {
        rows.push(item_row(
            format!("[{}] {}", menu_shortcut(i), a.name),
            i == selected,
        ));
        rows.push(text_row(format!("    {}", a.description)));
    }
    draw_popup("Extract", PopupSize::Large, &rows, fonts, m);
}

pub(super) fn draw_extract_confirm(
    game: &Game,
    program: Option<Entity>,
    index: Option<usize>,
    fonts: &Fonts,
    m: &Metrics,
) {
    let (Some(program), Some(index)) = (program, index) else {
        return;
    };
    let offered = game.extractable_routines(program);
    let Some(kept) = offered.get(index) else { return };
    let lost: Vec<&str> = offered
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, a)| a.name.as_str())
        .collect();
    let mut rows = vec![
        text_row(format!("Salvage {} and destroy the program?", kept.name)),
        text_row(""),
    ];
    if !lost.is_empty() {
        rows.push(text_row(format!("This loses: {}.", lost.join(", "))));
    }
    rows.push(text_row("Enter to confirm, Esc to back out."));
    draw_popup("Extract", PopupSize::Large, &rows, fonts, m);
}
```

Add the six arms to `crates/gui/src/render/mod.rs`'s mode match, and add `m`/`M` to the help screen's key list in `crates/gui/src/render/meta.rs`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p feral-processes-app-core`
Expected: PASS.

Run: `cargo test --workspace && cargo clippy --workspace`
Expected: PASS, no warnings. The GUI is not launched to verify drawing — this repo verifies rendering by reading and unit test, and a visual pass is the user's to make.

- [ ] **Step 7: Commit**

```bash
git add crates
git commit -m "feat: routine panel, install picker and the extraction flow"
```

---

### Task 9: Documentation and the final gate

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `assets/abilities/README.md`

- [ ] **Step 1: Find every claim this change falsifies**

Run:

```bash
grep -rni "decompile\|ability\|abilities\|routine" README.md CHANGELOG.md
```

Read each hit. Anything describing decompile as its own battle command, or abilities as fixed to a species, is now false.

- [ ] **Step 2: Update the root README**

Rewrite the affected passages to say: abilities are routines occupying level-derived slots; a companion gets one slot per two levels to a cap of six, the player one per ten; routines are extracted from a program you own at a Compiler, destroying it; decompile is a routine the player starts with.

- [ ] **Step 3: Add the CHANGELOG entry**

```markdown
### Added
- **Ability routines.** Abilities are now installable routines occupying
  level-derived slots on the player and every companion — one slot per two
  levels for a program, one per ten for you, six at most either way. Pop an
  innate routine out of one program and plug it into another.
- **Routine extraction.** With a Compiler standing anywhere, break a program
  you own down into exactly one of its routines. The program and its other
  routines are gone.
- Item and structure descriptions are authored in their `.ron` files, so a
  mod controls its own text.

### Changed
- Decompile is an ability the player starts with installed, reached through
  Special rather than its own battle command. It greys out with a reason when
  you hold no catalyst or your roster is full, instead of silently refusing
  the round.
- Research hands over routine *items* rather than the ability itself —
  researching and installing are separate acts.
- Save format v11. Older saves are rejected with a clear message, as always.
```

- [ ] **Step 4: Check the abilities README is complete**

Confirm `assets/abilities/README.md` documents the `Decompile` effect (Task 7) and explains that an ability automatically gets a `routine_<id>` item minted for it, so a mod needs no second file.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
cargo test -p feral-processes-engine balance_sim
```

Expected: all green. Record the new test count in the commit message; report the actual number seen, not an expected one.

- [ ] **Step 6: Commit**

```bash
git add README.md CHANGELOG.md assets/abilities/README.md
git commit -m "docs: routines, extraction and the decompile change"
```

---

## Balance note for the play-test

No shipped species is squeezed by the slot formula: the most any declares is two abilities, the latest unlock is level 8, and four slots exist by then. `balance_sim`'s companion curves should not move; if they do, something other than slots changed.

The player is the part to watch. One slot until level 10, and `decompile` occupies it — so a researched routine sits in cargo, unusable, until level 10 or until decompile is popped out at the cost of every capture. That is the design as specified, and the three `PLAYER_ROUTINE_SLOT_*` constants are where to move it if it plays badly.
