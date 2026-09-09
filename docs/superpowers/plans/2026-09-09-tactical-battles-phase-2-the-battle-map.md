# Tactical surface battles — Phase 2: the battle map

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `crates/engine/src/tactical/` — a `BattleCell` grid, a
generator that is pure in a `BattleSpec`, a connectivity carve, deployment of
two sides onto starting cells, and the `TacticalBattle` resource that holds
where every body stands — fully unit-tested and called by nothing.

**Architecture:** `stack::generate`'s precedent, one space over. A board is
derived per cell from an FNV-1a fold of `(world seed, site, tick, zone,
biome)` reduced through `derive::index` against a per-biome weight table,
exactly as `rock::RockDb::kind_at` derives base space; it is then carved so
no body can be stranded. `TacticalBattle` is a bare `Resource` — never
serialised, never inserted by anything in this phase — and it is the only
place a battle coordinate lives. No world `Position` is written anywhere in
this module.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (`Resource`, `Entity`), no `serde` on
any type introduced here.

**Spec:** `docs/superpowers/specs/2026-09-09-tactical-surface-battles-design.md`
(§2 the `tactical/` layout, §3 the battle map, §12 item 2).

**Depends on:** Phase 1, landed — `achievements::Profile::tactical_battles`
(`crates/engine/src/achievements.rs:264`) and `Mode::Options`
(`crates/app-core/src/lib.rs:1700`). Nothing in this phase reads either.

---

## Decisions this plan locks in

Taken with the user before writing, and not to be relitigated during
execution:

| | |
|---|---|
| **Board extent** | Three fixed tiers by body count: 1–3 → 14x14, 4–6 → 20x20, 7+ → 28x28. Square, not a frontage. |
| **Terrain** | Derived per cell — no `StdRng`, no allocation of a generator — from a biome weight table. Single cells, not grown blobs. |
| **Connectivity** | Guaranteed. Flood-fill, then carve a corridor through blockers until the walkable ground is one region. |
| **Phase edge** | Module only. `Game::start_battle` is **not** touched; nothing constructs a `TacticalBattle`. The router is a later phase. |

---

## Global constraints

- **Nothing here draws from `resources::GameRng`, and nothing here seeds an
  `StdRng` either.** A board is a property of *where and when the fight
  opened*, folded and reduced — `derive.rs`'s whole subject. A `GameRng` draw
  would shift every later roll in the run; an `StdRng` sequence is not
  guaranteed stable across a `rand` upgrade, and the tests in this phase
  compare boards for equality.
- **No `SAVE_FORMAT_VERSION` movement, and nothing new in `save.rs`.**
  Battles are not serialised today. `BattleState` is the precedent: bare
  `#[derive(Resource)]`, no `Serialize`. `TacticalBattle` matches it. Saving
  is opt-in by a field appearing in `save.rs`'s hand-written structs — a
  resource simply not appearing there *is* the mechanism.
- **No world `Position` is written by anything in this module.** This is the
  seam Task 8 writes down. `tactical/` does not import
  `crate::components::Position` at all.
- **`crates/gui` is not touched, and neither is `crates/app-core`.**
  `biome_tint` lives in `crates/gui/src/render/terrain.rs:116` and stays
  there; the engine has no colour concept for a biome and this phase does not
  give it one.
- **Nothing calls `tactical::` outside its own tests.** `pub mod tactical;`
  at the crate root makes every item reachable, so there are no `dead_code`
  warnings to suppress and none may be suppressed. If a step tempts you to
  wire this into `start_battle` to "prove it works", stop — the tests are the
  proof, and a `TacticalBattle` opened with no turn model behind it is a
  fight the player cannot act in.
- **Lowercase letters are row selectors** — irrelevant here, no keys are
  bound in this phase, and none may be.
- **No version bump on the branch.** The workspace version, the
  `CHANGELOG.md` section and the tag happen once, at the merge.
- **Gates:** `cargo fmt`, `cargo clippy --workspace` (warnings are fixed, not
  silenced), and `cargo test --workspace` before the phase is called done.
  Per-task runs narrow with `-p feral-processes-engine`.
- **Commit at every green step**, as the tasks below spell out.

---

## File structure

| file | responsibility |
|---|---|
| `crates/engine/src/derive.rs` (modify) | gains `fold` and `FNV_BASIS` — the byte-wise FNV-1a continuation three call sites currently each hold their own copy of |
| `crates/engine/src/stack.rs` (modify) | `FrameSpec::salted` becomes a call to `derive::fold` |
| `crates/engine/src/rock.rs` (modify) | `block_seed` becomes a call to `derive::fold` |
| `crates/engine/src/disposition.rs` (modify) | `seed` becomes a call to `derive::fold` |
| `crates/engine/src/tuning.rs` (modify) | a new "Tactical battle grid" section: the three extents, their thresholds, `Rough`'s cost, the deployment gap |
| `crates/engine/src/tactical/mod.rs` (create) | `TacticalBattle` — the resource, and where every body stands |
| `crates/engine/src/tactical/map.rs` (create) | `BattleCell`, `BattleSpec`, `Board`, the weight table, `generate`, the carve |
| `crates/engine/src/tactical/deploy.rs` (create) | `bearing`, `plan` — the two sides onto starting cells |
| `crates/engine/src/lib.rs` (modify) | `pub mod tactical;` |
| `docs/seams.md`, `.claude/skills/seams/references/combat.md`, `CLAUDE.md` (modify) | the three writes for the one seam this phase mints |

Tests live inline as `#[cfg(test)] mod tests` at the bottom of each new file,
matching `stack.rs:1619` and `rock.rs:279`. Nothing here needs
`tests/support.rs` — every function in this phase is pure or takes a
`&mut World` the test builds itself.

---

### Task 1: One fold, three callers

`derive.rs` already owns the *reduction* step (`index`) that every derived
value in the game shares, and its doc says so. It does not own the *fold*,
and there are three hand-written copies of the identical byte-wise FNV-1a
loop: `stack::FrameSpec::salted` (`stack.rs:357`), `rock::block_seed`
(`rock.rs:264`) and `disposition::seed` (`disposition.rs:142`).
`BattleSpec::cell_seed` in Task 3 would be the fourth. This repo's own rule —
"a doc comment claiming to mirror other code must be a call, not a copy" —
says extract it now rather than add the fourth.

This task is **behaviour-preserving**. Every existing test must stay green
with no edits.

**Files:**
- Modify: `crates/engine/src/derive.rs`
- Modify: `crates/engine/src/stack.rs:334-366` (`FrameSpec::salted`)
- Modify: `crates/engine/src/rock.rs:264-277` (`block_seed`)
- Modify: `crates/engine/src/disposition.rs:142-149` (`Disposition::seed`)

**Interfaces:**
- Produces: `crate::derive::FNV_BASIS: u64` and
  `crate::derive::fold(seed: u64, words: &[u64]) -> u64`, both
  `pub(crate)`.
- Consumes: nothing.

`FrameSpec::rng_seed` (`stack.rs:312`) folds **whole words, one multiply per
word**, which is a different fold and is deliberately so — its doc argues for
it. Do not touch it and do not route it through `fold`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/derive.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The specification of the fold, written out longhand rather than
    /// called: this test is what pins the three call sites that used to
    /// hold their own copy of this loop, so an "optimisation" that folds
    /// whole words again fails here rather than silently re-correlating
    /// every derived value in the game.
    #[test]
    fn fold_is_fnv_1a_one_byte_at_a_time() {
        let mut expected = FNV_BASIS;
        for word in [7_u64, 0xdead_beef_u64] {
            for byte in word.to_le_bytes() {
                expected ^= byte as u64;
                expected = expected.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        assert_eq!(fold(FNV_BASIS, &[7, 0xdead_beef]), expected);
    }

    /// Salting is what `FrameSpec::salted` needs of it: continuing an
    /// existing fold has to equal folding the whole list at once, or a
    /// cell's seed would depend on how many calls it took to build.
    #[test]
    fn fold_continues_from_a_seed_rather_than_restarting() {
        assert_eq!(
            fold(FNV_BASIS, &[1, 2]),
            fold(fold(FNV_BASIS, &[1]), &[2]),
        );
    }

    #[test]
    fn fold_distinguishes_the_order_of_its_words() {
        assert_ne!(fold(FNV_BASIS, &[1, 2]), fold(FNV_BASIS, &[2, 1]));
    }

    /// The property `index` depends on and the reason the fold is
    /// byte-wise: two inputs differing only in the low bits of the *last*
    /// word must still differ in the bit `index` reads.
    #[test]
    fn a_low_bit_difference_in_the_last_word_reaches_the_top_bit() {
        let a = fold(FNV_BASIS, &[99, 4]);
        let b = fold(FNV_BASIS, &[99, 5]);
        assert_ne!(a >> 63, b >> 63, "a one-bit change never reached bit 63");
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine derive::tests`

Expected: FAIL to compile — `cannot find function 'fold' in this scope` and
`cannot find value 'FNV_BASIS' in this scope`.

- [ ] **Step 3: Add `fold` and `FNV_BASIS`**

In `crates/engine/src/derive.rs`, above `index`, add:

```rust
/// FNV-1a's 64-bit offset basis — where a fold with nothing behind it
/// starts.
pub(crate) const FNV_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Continues an FNV-1a fold with further words, **one byte at a time**.
///
/// The counterpart to `index`: that function reduces, this one mixes, and
/// the two are always used together. Shared rather than copied because a
/// fold is precisely the thing a comment cannot keep in sync — three call
/// sites held a character-for-character copy of this loop before it was
/// extracted (`stack::FrameSpec::salted`, `rock::block_seed`,
/// `disposition::seed`), and `tactical::BattleSpec` would have been the
/// fourth.
///
/// Byte-at-a-time rather than XOR-ing each word in whole and multiplying
/// through the prime once, because `index` reads the **high** bits. A
/// whole-word XOR gets exactly one multiply round to spread it, and one
/// round cannot carry a low-bit difference much past the prime's own width
/// (`FNV_PRIME` is ~41 bits) before the fold ends — measured against
/// `FrameSpec::salted`'s history, a whole-word fold leaves many of the 64
/// output bits, including several of the highest, identical across most
/// adjacent input pairs, with the bottom bit alternating in lockstep with
/// one input's parity instead. This is a property of the fold, not a fixed
/// count: three separate measurements came back 21/64 with the top 6 fixed,
/// 22/64 with the top 8, and 23/64 with the top 7, depending on which pairs
/// were sampled, so no single number is asserted by a test. `[a, b]`
/// diverging from `[b, a]` is not the same claim as "adjacent inputs cannot
/// rhyme" — both can be true at once, because `assert_ne!` needs only one
/// bit of difference. Giving every byte its own XOR-then-multiply pass
/// leaves no output bit a fixed function of the input.
pub(crate) fn fold(seed: u64, words: &[u64]) -> u64 {
    let mut h = seed;
    for &word in words {
        for byte in word.to_le_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
    }
    h
}
```

Then widen the module doc at `derive.rs:1-11`: change its closing sentence
`So each folds the values it is derived from and reduces the result to an
index, and this module owns the reduction step both of them share.` to:

```rust
//! So each folds the values it is derived from and reduces the result to an
//! index, and this module owns **both** steps: `fold` mixes, `index`
//! reduces, and no caller may hand-write either.
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p feral-processes-engine derive::tests`

Expected: PASS, 4 tests.

- [ ] **Step 5: Route `FrameSpec::salted` through it**

In `crates/engine/src/stack.rs`, replace the body of `salted` (`stack.rs:357-366`)
and the middle of its doc. Keep the first paragraph and the closing
`LAIR_SALT` paragraph exactly as they are; replace the long
byte-at-a-time paragraph (the one beginning "Each word is folded in **one
byte at a time**") with:

```rust
    /// The fold itself is `derive::fold` — byte-at-a-time FNV-1a, for the
    /// reason spelled out there, which used to be spelled out here.
```

and the body with:

```rust
    pub(crate) fn salted(self, words: &[u64]) -> u64 {
        crate::derive::fold(self.rng_seed(), words)
    }
```

- [ ] **Step 6: Route `rock::block_seed` through it**

In `crates/engine/src/rock.rs`, replace `block_seed` (`rock.rs:264-277`) —
keeping whatever doc comment sits above it — with:

```rust
fn block_seed(seed: u32, x: i32, y: i32) -> u64 {
    crate::derive::fold(
        crate::derive::FNV_BASIS,
        &[
            seed as u64,
            x.div_euclid(VEIN_BLOCK) as i64 as u64,
            y.div_euclid(VEIN_BLOCK) as i64 as u64,
        ],
    )
}
```

The `as i64 as u64` double cast is load-bearing — it sign-extends a negative
block coordinate rather than zero-extending it. Keep it exactly.

- [ ] **Step 7: Route `disposition::seed` through it**

In `crates/engine/src/disposition.rs`, replace the body of `seed`
(`disposition.rs:142-149`), keeping its doc comment, with:

```rust
    pub fn seed(program_id: u32) -> Self {
        let h = crate::derive::fold(crate::derive::FNV_BASIS, &[program_id as u64]);
        Disposition::ALL[crate::derive::index(h, Disposition::ALL.len())]
    }
```

- [ ] **Step 8: Prove nothing moved**

Run: `cargo test -p feral-processes-engine`

Expected: PASS, the whole engine suite, **with no test edited**. Every one of
`stack.rs`'s `salting_*` tests, `rock.rs`'s tests and `disposition.rs`'s tests
exercises a value that would change if the extraction were not exact. If any
of them fails, the extraction is wrong — do not adjust the test.

- [ ] **Step 9: Run the gates and commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/derive.rs crates/engine/src/stack.rs \
        crates/engine/src/rock.rs crates/engine/src/disposition.rs
git commit -m "refactor: one byte-wise FNV-1a fold, three callers

\`derive.rs\` owned the reduction step every derived value shares and not the
fold, so \`FrameSpec::salted\`, \`rock::block_seed\` and \`disposition::seed\`
each held a character-for-character copy of the same loop. The tactical
battle map would have been the fourth.

Behaviour-preserving: no test edited."
```

---

### Task 2: `BattleCell` and the module

Four kinds, and what makes them four rather than an arbitrary list is that
they are the complete 2x2 of "can you cross it" against "can you shoot
through it". `Blocked` is a chasm — see over, cannot cross. `Cover` is a
boulder — neither. The Stack already establishes that `walkable()` and
`blocks_sight()` are not complements; this is the same asymmetry.

**Files:**
- Create: `crates/engine/src/tactical/mod.rs`
- Create: `crates/engine/src/tactical/map.rs`
- Modify: `crates/engine/src/lib.rs` (the module list, `lib.rs:1-48`)
- Modify: `crates/engine/src/tuning.rs` (a new section)

**Interfaces:**
- Produces: `crate::tactical::map::BattleCell` with `movement_cost() ->
  Option<u32>`, `walkable() -> bool`, `blocks_sight() -> bool`; and
  `tuning::TACTICAL_ROUGH_COST`.
- Consumes: nothing.

`movement_cost` returns `Option<u32>` rather than `u32` because that is
exactly the shape Phase 3 widens `walk_field`'s step rule to — `None` is
blocked, `Some(c)` is the cost. `walkable` is defined in terms of it so the
two cannot disagree.

- [ ] **Step 1: Write the failing tests**

Create `crates/engine/src/tactical/map.rs` containing only this test module
for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The pair that makes four kinds necessary rather than arbitrary: one
    /// you can see over and cannot cross, one you can do neither with. If
    /// these ever agree, two of the kinds are the same kind.
    #[test]
    fn blocked_is_seen_over_and_cover_is_not() {
        assert!(!BattleCell::Blocked.walkable());
        assert!(!BattleCell::Blocked.blocks_sight());
        assert!(!BattleCell::Cover.walkable());
        assert!(BattleCell::Cover.blocks_sight());
    }

    #[test]
    fn rough_is_crossed_and_costs_more_than_open() {
        assert_eq!(BattleCell::Open.movement_cost(), Some(1));
        assert!(BattleCell::Rough.walkable());
        assert!(
            BattleCell::Rough.movement_cost() > BattleCell::Open.movement_cost(),
            "Rough that costs one is Open with a different name"
        );
    }

    /// `walkable` is derived from `movement_cost` rather than matched
    /// separately, so the two can never come to disagree about a kind.
    #[test]
    fn walkable_is_exactly_having_a_movement_cost() {
        for kind in [
            BattleCell::Open,
            BattleCell::Rough,
            BattleCell::Cover,
            BattleCell::Blocked,
        ] {
            assert_eq!(kind.walkable(), kind.movement_cost().is_some());
        }
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p feral-processes-engine tactical::map`

Expected: FAIL — `file not found for module 'tactical'`, because `lib.rs`
does not declare it yet.

- [ ] **Step 3: Add the tuning section**

In `crates/engine/src/tuning.rs`, append a new section at the end of the
file:

```rust
// ─────────────────────────────────────────────────────────────────────────
// Tactical battle grid
// ─────────────────────────────────────────────────────────────────────────

/// What crossing a `BattleCell::Rough` cell costs, against `Open`'s one.
///
/// Two rather than three because a body's whole allowance is small: at
/// three, one rough cell eats most of a turn and the terrain stops being a
/// choice and becomes a wall with extra steps.
pub const TACTICAL_ROUGH_COST: u32 = 2;
```

- [ ] **Step 4: Write `BattleCell`**

Put this at the **top** of `crates/engine/src/tactical/map.rs`, above the
test module:

```rust
//! The battle map: a grid generated per encounter and destroyed at teardown.
//!
//! `stack::generate`'s precedent, one space over. A board is **pure in its
//! spec** — the same `BattleSpec` yields the same board, cell for cell — and
//! is never saved, because a fight is never saved. Nothing here draws from
//! `resources::GameRng` (a draw does not survive a save/load and shifts
//! every later roll in the run) and nothing here seeds an `StdRng` either
//! (its sequence is not guaranteed stable across a `rand` upgrade, and the
//! tests below compare whole boards for equality). Every cell is *derived*:
//! folded through `derive::fold` and reduced through `derive::index`, the
//! way `rock::RockDb::kind_at` derives base space.

use crate::tuning::TACTICAL_ROUGH_COST;

/// What a cell of a battle map is made of.
///
/// Four kinds, and the four are the complete 2x2 of "can you cross it"
/// against "can you shoot through it" — which is what keeps them from being
/// an arbitrary list. `Blocked` is a chasm you can see over and cannot
/// cross; `Cover` is a boulder that stops both. Note this is the same
/// asymmetry the Stack already establishes, where `walkable()` and
/// `blocks_sight()` are deliberately not complements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BattleCell {
    Open,
    Rough,
    Cover,
    Blocked,
}

impl BattleCell {
    /// What crossing this cell costs, or `None` where it cannot be crossed.
    ///
    /// An `Option<u32>` rather than a `u32` with a sentinel because that is
    /// the shape the movement field's step rule takes: `None` is blocked,
    /// `Some(c)` is the cost.
    pub fn movement_cost(self) -> Option<u32> {
        match self {
            BattleCell::Open => Some(1),
            BattleCell::Rough => Some(TACTICAL_ROUGH_COST),
            BattleCell::Cover | BattleCell::Blocked => None,
        }
    }

    /// Derived from `movement_cost` rather than matched separately, so the
    /// two cannot come to disagree about a kind.
    pub fn walkable(self) -> bool {
        self.movement_cost().is_some()
    }

    /// `Cover` is the only kind that stops a line or a cone.
    pub fn blocks_sight(self) -> bool {
        matches!(self, BattleCell::Cover)
    }
}
```

Create `crates/engine/src/tactical/mod.rs`:

```rust
//! The tactical battle model: a disposable grid a surface fight is fought
//! on.
//!
//! Opt-in, off by default, and the second of the game's two combat models —
//! see
//! `docs/superpowers/specs/2026-09-09-tactical-surface-battles-design.md`.
//! Nothing outside this module's own tests calls into it yet; the router
//! that chooses between the two models is a later phase.
//!
//! **A battle coordinate lives here and nowhere else.** No world `Position`
//! is ever written for a body standing on a battle map, the same way the
//! Stack keeps its coordinates in `resources::Locale` and base space keeps
//! its own in `Locale::Base`. This module does not import `Position`.

pub mod map;
```

Declare the module in `crates/engine/src/lib.rs`, in alphabetical position
between `pub mod systems;` and `pub mod talents;`:

```rust
pub mod tactical;
```

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p feral-processes-engine tactical::map`

Expected: PASS, 3 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add crates/engine/src/lib.rs crates/engine/src/tuning.rs crates/engine/src/tactical/
git commit -m "feat: BattleCell — the battle map's four kinds

The complete 2x2 of crossing against seeing: Blocked is a chasm you see over
and cannot cross, Cover is a boulder that stops both. \`walkable\` is derived
from \`movement_cost\` so the two cannot disagree, and \`movement_cost\` is
already the Option shape the movement field's step rule will take."
```

---

### Task 3: `BattleSpec` and the board's extent

The spec a board is pure in. It carries the fight's site and moment so two
fights on the same tile are not the same board, its biome and zone so the
ground varies, and its body count so the board is sized for the fight.

**Files:**
- Modify: `crates/engine/src/tactical/map.rs`
- Modify: `crates/engine/src/tuning.rs` (the "Tactical battle grid" section
  from Task 2)

**Interfaces:**
- Produces: `BattleSpec { world_seed, site, tick, zone, biome, bodies }` with
  `side() -> i32` and `pub(crate) cell_seed(x, y) -> u64`; and
  `tuning::TACTICAL_BOARD_SMALL` / `_MEDIUM` / `_LARGE`,
  `TACTICAL_MEDIUM_BODIES`, `TACTICAL_LARGE_BODIES`.
- Consumes: `crate::derive::{fold, FNV_BASIS}` (Task 1),
  `crate::world::Biome`.

`bodies` is the **total** — party plus wild. With `MAX_PARTY_SIZE = 5`
(`tuning.rs:2609`) and `MAX_PACK_BODIES = 8` (`tuning.rs:699`) the reachable
range is 2 (the player and one wild) through 13, so all three tiers are
reachable in play and none of them is a gate that is green and unreachable.

`tick` is in the spec, and that is safe here for a reason that would not hold
elsewhere: a battle is never saved and never regenerated, so a board that
depends on the moment cannot come back wrong after a reload. It is what stops
two fights on one tile from being the same board.

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/tactical/map.rs`'s `mod tests`:

```rust
    use crate::world::Biome;

    fn spec(bodies: u32) -> BattleSpec {
        BattleSpec {
            world_seed: 1234,
            site: (12, -7),
            tick: 900,
            zone: 3,
            biome: Biome::OpenGrid,
            bodies,
        }
    }

    #[test]
    fn the_board_steps_up_a_tier_at_four_bodies_and_at_seven() {
        assert_eq!(spec(1).side(), TACTICAL_BOARD_SMALL);
        assert_eq!(spec(3).side(), TACTICAL_BOARD_SMALL);
        assert_eq!(spec(4).side(), TACTICAL_BOARD_MEDIUM);
        assert_eq!(spec(6).side(), TACTICAL_BOARD_MEDIUM);
        assert_eq!(spec(7).side(), TACTICAL_BOARD_LARGE);
        assert_eq!(spec(13).side(), TACTICAL_BOARD_LARGE);
    }

    /// Every tier is reachable in play: the smallest fight the game can
    /// field is the player and one wild body, and the largest is a full
    /// party against a full pack.
    #[test]
    fn all_three_tiers_are_reachable_between_the_smallest_and_largest_fight() {
        let smallest = 1 + 1;
        let largest = crate::tuning::MAX_PARTY_SIZE as u32 + crate::tuning::MAX_PACK_BODIES;
        assert_eq!(spec(smallest).side(), TACTICAL_BOARD_SMALL);
        assert_eq!(spec(largest).side(), TACTICAL_BOARD_LARGE);
        assert!(
            (smallest..=largest).any(|n| spec(n).side() == TACTICAL_BOARD_MEDIUM),
            "the middle tier can never be reached"
        );
    }

    #[test]
    fn a_cell_seed_is_a_property_of_the_spec_and_the_cell() {
        assert_eq!(spec(4).cell_seed(3, 5), spec(4).cell_seed(3, 5));
        assert_ne!(spec(4).cell_seed(3, 5), spec(4).cell_seed(3, 6));
        assert_ne!(spec(4).cell_seed(3, 5), spec(4).cell_seed(5, 3));
    }

    /// Two fights on the same tile are not the same fight. A board is never
    /// saved and never regenerated, so leaning on the clock here cannot come
    /// back wrong after a reload.
    #[test]
    fn a_later_fight_on_the_same_tile_gets_a_different_board() {
        let mut later = spec(4);
        later.tick += 1;
        assert_ne!(spec(4).cell_seed(0, 0), later.cell_seed(0, 0));
    }

    #[test]
    fn the_biome_and_the_zone_both_reach_the_cell_seed() {
        let mut marsh = spec(4);
        marsh.biome = Biome::Deadlock;
        assert_ne!(spec(4).cell_seed(0, 0), marsh.cell_seed(0, 0));

        let mut deeper = spec(4);
        deeper.zone += 1;
        assert_ne!(spec(4).cell_seed(0, 0), deeper.cell_seed(0, 0));
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine tactical::map`

Expected: FAIL to compile — `cannot find struct 'BattleSpec' in this scope`.

- [ ] **Step 3: Add the extents to tuning**

Append to the "Tactical battle grid" section of `crates/engine/src/tuning.rs`:

```rust
/// The three board extents, in cells on a side.
///
/// Three fixed tiers rather than a per-body formula so the player learns
/// their shapes: a board is a place you fight in repeatedly, and one that
/// is a slightly different size every time is one you can never read at a
/// glance. Sized for manoeuvre — deployment is what puts the two sides in
/// contact, so a board large enough to flank on does not open every fight
/// with a walk.
pub const TACTICAL_BOARD_SMALL: i32 = 14;
pub const TACTICAL_BOARD_MEDIUM: i32 = 20;
pub const TACTICAL_BOARD_LARGE: i32 = 28;

/// Total bodies — party plus wild — at which the board steps up a tier.
///
/// Both are reachable and so is the tier between them: the smallest fight
/// the game fields is the player and one wild body, and the largest is
/// `MAX_PARTY_SIZE` against `MAX_PACK_BODIES`.
pub const TACTICAL_MEDIUM_BODIES: u32 = 4;
pub const TACTICAL_LARGE_BODIES: u32 = 7;
```

- [ ] **Step 4: Write `BattleSpec`**

In `crates/engine/src/tactical/map.rs`, extend the `use` at the top to:

```rust
use crate::derive::{fold, FNV_BASIS};
use crate::tuning::{
    TACTICAL_BOARD_LARGE, TACTICAL_BOARD_MEDIUM, TACTICAL_BOARD_SMALL,
    TACTICAL_LARGE_BODIES, TACTICAL_MEDIUM_BODIES, TACTICAL_ROUGH_COST,
};
use crate::world::Biome;
```

`derive::index` is **not** imported yet — nothing reduces a seed until Task
4, and an unused import fails this task's `clippy` gate. Task 4 widens the
line to `use crate::derive::{fold, index, FNV_BASIS};`.

Add below `BattleCell`:

```rust
/// Everything a battle map is derived from.
///
/// `stack::FrameSpec`'s counterpart: a board is a pure function of this and
/// nothing else, which is what makes it unit-testable without a `Game` and
/// what makes it safe never to save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleSpec {
    pub world_seed: u32,
    /// The world tile the fight opened on.
    pub site: (i32, i32),
    /// `GameClock` at the moment the fight opened.
    ///
    /// In the spec so that two fights on one tile are not the same board.
    /// Safe here and nowhere else: a battle is never saved and never
    /// regenerated, so a board that depends on the moment cannot come back
    /// wrong after a reload — the trap `stack::generate`'s doc warns about.
    pub tick: u64,
    pub zone: u32,
    pub biome: Biome,
    /// Party plus wild. Decides the board's extent, nothing else.
    pub bodies: u32,
}

impl BattleSpec {
    /// The board's extent, in cells on a side.
    pub fn side(self) -> i32 {
        if self.bodies >= TACTICAL_LARGE_BODIES {
            TACTICAL_BOARD_LARGE
        } else if self.bodies >= TACTICAL_MEDIUM_BODIES {
            TACTICAL_BOARD_MEDIUM
        } else {
            TACTICAL_BOARD_SMALL
        }
    }

    /// The fold every cell of this board starts from.
    ///
    /// `bodies` is deliberately absent: it decides the extent, and folding
    /// it in as well would mean one more companion in the party changed the
    /// ground under a fight on the same tile at the same moment.
    fn base_seed(self) -> u64 {
        fold(
            FNV_BASIS,
            &[
                self.world_seed as u64,
                self.site.0 as u32 as u64,
                self.site.1 as u32 as u64,
                self.tick,
                self.zone as u64,
                self.biome as u64,
            ],
        )
    }

    /// A stable seed for one cell of this board.
    pub(crate) fn cell_seed(self, x: i32, y: i32) -> u64 {
        fold(self.base_seed(), &[x as u32 as u64, y as u32 as u64])
    }
}
```

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p feral-processes-engine tactical::map`

Expected: PASS, 8 tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/tuning.rs crates/engine/src/tactical/map.rs
git commit -m "feat: BattleSpec — what a battle map is derived from

Three fixed board tiers by total body count, all three reachable between the
smallest fight the game fields and the largest. The clock is in the spec so
two fights on one tile are not the same board, which is safe here and only
here: a battle is never saved and never regenerated."
```

---

### Task 4: The ground

Each cell is derived, not rolled: a fold of the spec and the coordinate,
reduced through `derive::index` against a per-biome weight table. This is
`rock::RockDb::kind_at`'s shape exactly, with the table in `tuning.rs` rather
than in `.ron` because `Biome` is a fixed Rust enum that `WorldMap::classify`
sorts noise into — the same argument `Biome::name` already makes for being an
exhaustive match rather than data.

**Files:**
- Modify: `crates/engine/src/tactical/map.rs`

**Interfaces:**
- Produces: `Board { side }` with `in_bounds`, `cell`, `walkable`,
  `blocks_sight`, `cells`; `map::generate(spec: BattleSpec) -> Board`;
  private `terrain_weights(Biome) -> [u32; 4]` and `kind_at`.
- Consumes: `BattleSpec`, `BattleCell`, `derive::index`.

Out of bounds reads as `Blocked` — see over, cannot cross. That is the right
answer for generation, the carve and deployment, all of which must not place
or path outside the board. Walking *off* the edge is a departure from the
fight, not a step, and belongs to the turn model in a later phase; nothing
here may treat the edge as an exit.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    /// The weights read as percentages at a glance, which is the only
    /// reason they are worth reading at all.
    #[test]
    fn every_biomes_ground_sums_to_a_hundred() {
        for biome in [
            Biome::DataVoid,
            Biome::Deadlock,
            Biome::NullSector,
            Biome::Backplane,
            Biome::OpenGrid,
            Biome::BlackIce,
            Biome::Platform,
            Biome::Excavated,
            Biome::Entropy,
        ] {
            let total: u32 = terrain_weights(biome).iter().sum();
            assert_eq!(total, 100, "{biome:?} does not sum to 100");
        }
    }

    #[test]
    fn the_same_spec_yields_an_identical_board() {
        let a = generate(spec(4));
        let b = generate(spec(4));
        assert_eq!(a.side, b.side);
        assert_eq!(a.cells().collect::<Vec<_>>(), b.cells().collect::<Vec<_>>());
    }

    #[test]
    fn a_board_is_its_tier_square() {
        let board = generate(spec(9));
        assert_eq!(board.side, TACTICAL_BOARD_LARGE);
        assert_eq!(
            board.cells().count(),
            (TACTICAL_BOARD_LARGE * TACTICAL_BOARD_LARGE) as usize
        );
    }

    fn blockers(board: &Board) -> usize {
        board.cells().filter(|(_, kind)| !kind.walkable()).count()
    }

    /// The ground is the biome's, not one texture everywhere. Backplane is
    /// a circuit board and is dense with cover; Open Grid is a plain.
    #[test]
    fn a_backplane_fight_has_more_to_hide_behind_than_an_open_grid_one() {
        let mut plain = spec(4);
        plain.biome = Biome::OpenGrid;
        let mut city = spec(4);
        city.biome = Biome::Backplane;
        assert!(
            blockers(&generate(city)) > blockers(&generate(plain)),
            "the biome does not reach the ground"
        );
    }

    #[test]
    fn out_of_bounds_is_seen_over_and_never_stepped_on() {
        let board = generate(spec(4));
        assert!(!board.in_bounds(-1, 0));
        assert!(!board.in_bounds(board.side, 0));
        assert_eq!(board.cell(-1, 0), BattleCell::Blocked);
        assert!(!board.walkable(-1, 0));
        assert!(!board.blocks_sight(-1, 0));
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine tactical::map`

Expected: FAIL to compile — `cannot find function 'terrain_weights'`,
`cannot find function 'generate'`, `cannot find type 'Board'`.

- [ ] **Step 3: Write the weight table, `Board`, `kind_at` and `generate`**

First widen the `derive` import at the top of the file to bring in the
reducer:

```rust
use crate::derive::{fold, index, FNV_BASIS};
```

Then add, below `BattleSpec`:

```rust
/// The four kinds in the order `terrain_weights` gives their weights.
const KINDS: [BattleCell; 4] = [
    BattleCell::Open,
    BattleCell::Rough,
    BattleCell::Cover,
    BattleCell::Blocked,
];

/// What each biome's ground is made of, as `[Open, Rough, Cover, Blocked]`
/// weights summing to 100.
///
/// **Exhaustive on `Biome`** — `cell_mark`'s rule. A `_ =>` arm would ship a
/// new biome's fights on whatever the fallback happened to be, and terrain
/// that is quietly the wrong terrain reads as the generator being bland
/// rather than as a missing row.
///
/// In `tuning.rs`'s neighbourhood rather than in `.ron` for the reason
/// `Biome::name` already gives: mods extend species, structures, items and
/// environments, but the biome set is a fixed enum `WorldMap::classify`
/// sorts noise into, and ground for a variant that cannot exist is not a
/// thing a file can usefully say.
fn terrain_weights(biome: Biome) -> [u32; 4] {
    match biome {
        // A plain. Position matters least here, which is the point of it.
        Biome::OpenGrid => [82, 12, 4, 2],
        // Interference underfoot: slow going, little to hide behind.
        Biome::Deadlock => [60, 30, 7, 3],
        // Holes in the substrate — see across them, cannot cross them.
        Biome::NullSector => [62, 16, 8, 14],
        // A circuit board: dense with things to put between you and a shot.
        Biome::Backplane => [58, 14, 20, 8],
        // Laid and carved base floor. A fight does not open here today, but
        // if one ever does it opens on a floor, which is what these are.
        Biome::Platform | Biome::Excavated => [92, 6, 2, 0],
        // Unwalkable ground: `Biome::walkable()` is false for all three, so
        // nothing is ever placed there and no fight opens there. Plain open
        // rather than solid, so that if one ever did it would be a board and
        // not a wall.
        Biome::DataVoid | Biome::BlackIce | Biome::Entropy => [100, 0, 0, 0],
    }
}

/// The ground of one cell, derived and never rolled.
fn kind_at(spec: BattleSpec, x: i32, y: i32) -> BattleCell {
    let weights = terrain_weights(spec.biome);
    let total: u32 = weights.iter().sum();
    let mut n = index(spec.cell_seed(x, y), total as usize) as u32;
    for (kind, weight) in KINDS.iter().zip(weights) {
        if n < weight {
            return *kind;
        }
        n -= weight;
    }
    BattleCell::Open
}

/// A generated battle map. Never saved; discarded at teardown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub side: i32,
    cells: Vec<BattleCell>,
}

impl Board {
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.side && y < self.side
    }

    /// Off the board reads as `Blocked` — seen over, never stepped on.
    ///
    /// The right answer for generation, the carve and deployment, none of
    /// which may reach outside the board. Walking *off* the edge is a
    /// departure from the fight rather than a step, and belongs to the turn
    /// model; nothing here treats the edge as an exit.
    pub fn cell(&self, x: i32, y: i32) -> BattleCell {
        if !self.in_bounds(x, y) {
            return BattleCell::Blocked;
        }
        self.cells[(y * self.side + x) as usize]
    }

    pub fn walkable(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).walkable()
    }

    pub fn blocks_sight(&self, x: i32, y: i32) -> bool {
        self.cell(x, y).blocks_sight()
    }

    fn set(&mut self, x: i32, y: i32, kind: BattleCell) {
        if self.in_bounds(x, y) {
            let i = (y * self.side + x) as usize;
            self.cells[i] = kind;
        }
    }

    pub fn cells(&self) -> impl Iterator<Item = ((i32, i32), BattleCell)> + '_ {
        let side = self.side;
        self.cells
            .iter()
            .enumerate()
            .map(move |(i, &kind)| (((i as i32) % side, (i as i32) / side), kind))
    }
}

/// The whole generator: derive every cell, then make sure the walkable
/// ground is one piece.
pub fn generate(spec: BattleSpec) -> Board {
    let side = spec.side();
    let cells = (0..side * side)
        .map(|i| kind_at(spec, i % side, i / side))
        .collect();
    Board { side, cells }
}
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p feral-processes-engine tactical::map`

Expected: PASS, 13 tests.

If `a_backplane_fight_has_more_to_hide_behind_than_an_open_grid_one` fails,
the weight table is wrong — Backplane's blockers total 28 against Open Grid's
6 over the same 400 cells, so the margin is large and a failure means the
reduction or the zip is mis-wired, not that the test is flaky.

- [ ] **Step 5: Commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/tactical/map.rs
git commit -m "feat: the battle map's ground, derived per cell

rock::RockDb::kind_at's shape: a fold of the spec and the coordinate reduced
through derive::index against a per-biome weight table, exhaustive on Biome
so a new one cannot ship its fights on a fallback. Off the board reads as
Blocked — seen over, never stepped on; walking off the edge is the turn
model's business, not the board's."
```

---

### Task 5: Nobody starts in a pocket

A body that cannot leave the cell it deployed on is a fight that cannot
finish. At these blocker rates a sealed pocket is rare rather than
impossible, and rare is the worst kind: it survives every test run and
happens to a player. So the generator guarantees it — flood-fill, and carve a
corridor through the blockers until the walkable ground is one region.

**Files:**
- Modify: `crates/engine/src/tactical/map.rs`

**Interfaces:**
- Produces: private `NEIGHBOURS`, `region(&Board, (i32, i32)) ->
  BTreeSet<(i32, i32)>`, `carve_to_connect(&mut Board)`; `generate` now calls
  the carve.
- Consumes: `Board`.

`region` is **cost-blind on purpose** and is not the movement field Phase 3
builds. It asks "can a body get there at all", which `Rough`'s cost cannot
change; the movement field asks "how far can this body get this turn", which
is a different question with a different answer. Do not merge them.

Eight-way, with corner-cutting allowed, matching `walk_field`'s "all eight
directions" — the movement field will step the same way, so connectivity here
and reachability there agree.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    /// A hand-built board with a wall straight down the middle: two regions
    /// before the carve, one after.
    #[test]
    fn a_wall_across_the_board_is_carved_through() {
        let mut board = Board {
            side: 7,
            cells: vec![BattleCell::Open; 49],
        };
        for y in 0..7 {
            board.set(3, y, BattleCell::Cover);
        }
        assert_eq!(
            region(&board, (0, 0)).len(),
            21,
            "the fixture is not actually split"
        );

        carve_to_connect(&mut board);

        assert_eq!(
            region(&board, (0, 0)).len(),
            board.cells().filter(|(_, k)| k.walkable()).count(),
            "the carve left ground the rest of the board cannot reach"
        );
    }

    /// The carve opens a way through and does not flatten the board.
    #[test]
    fn the_carve_spends_as_few_cells_as_it_can() {
        let mut board = Board {
            side: 7,
            cells: vec![BattleCell::Open; 49],
        };
        for y in 0..7 {
            board.set(3, y, BattleCell::Cover);
        }
        carve_to_connect(&mut board);
        let opened = board
            .cells()
            .filter(|((x, _), k)| *x == 3 && k.walkable())
            .count();
        assert_eq!(opened, 1, "the carve took out more of the wall than it needed");
    }

    /// The property the whole task exists for, over every shipped biome a
    /// fight can open on and every tier.
    #[test]
    fn no_generated_board_strands_anybody() {
        for biome in [
            Biome::OpenGrid,
            Biome::Deadlock,
            Biome::NullSector,
            Biome::Backplane,
        ] {
            for bodies in [2_u32, 5, 9] {
                for tick in 0..60_u64 {
                    let board = generate(BattleSpec {
                        world_seed: 77,
                        site: (4, 4),
                        tick,
                        zone: 2,
                        biome,
                        bodies,
                    });
                    let walkable = board.cells().filter(|(_, k)| k.walkable()).count();
                    let start = board
                        .cells()
                        .find(|(_, k)| k.walkable())
                        .map(|(c, _)| c)
                        .expect("a board with no ground at all");
                    assert_eq!(
                        region(&board, start).len(),
                        walkable,
                        "{biome:?} at {bodies} bodies, tick {tick}: ground is in pieces"
                    );
                }
            }
        }
    }
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine tactical::map`

Expected: FAIL to compile — `cannot find function 'region'`, `cannot find
function 'carve_to_connect'`.

- [ ] **Step 3: Write the flood fill and the carve**

Add `use std::collections::{BTreeSet, VecDeque};` to the top of
`crates/engine/src/tactical/map.rs`, and add below `Board`:

```rust
/// Eight-way, in a fixed order so every walk over a board is deterministic.
const NEIGHBOURS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Every walkable cell reachable from `from`, eight-way.
///
/// **Cost-blind on purpose**, and not the movement field. This asks "can a
/// body get there at all", which `Rough`'s cost cannot change; the movement
/// field asks "how far can this body get this turn", which is a different
/// question with a different answer. Corner-cutting is allowed, matching
/// `walk_field`'s eight directions, so connectivity here and reachability
/// there agree.
fn region(board: &Board, from: (i32, i32)) -> BTreeSet<(i32, i32)> {
    let mut seen = BTreeSet::new();
    if !board.walkable(from.0, from.1) {
        return seen;
    }
    let mut queue = VecDeque::from([from]);
    seen.insert(from);
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in NEIGHBOURS {
            let next = (x + dx, y + dy);
            if board.walkable(next.0, next.1) && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    seen
}

/// Opens blockers until the walkable ground is one piece.
///
/// A sealed pocket is rare at these blocker rates rather than impossible,
/// and rare is the worst kind: it survives every test run and then happens
/// to a player, who finds a body that cannot leave the cell it deployed on
/// and a fight that cannot finish. Each pass grows the mainland by at least
/// one cell, so this terminates in at most `side * side` passes.
fn carve_to_connect(board: &mut Board) {
    loop {
        let Some(start) = board.cells().find(|(_, k)| k.walkable()).map(|(c, _)| c) else {
            return;
        };
        let mainland = region(board, start);
        let stranded = board
            .cells()
            .find(|((x, y), k)| k.walkable() && !mainland.contains(&(*x, *y)))
            .map(|(c, _)| c);
        let Some(stranded) = stranded else {
            return;
        };
        for cell in corridor(board, &mainland, stranded) {
            board.set(cell.0, cell.1, BattleCell::Open);
        }
    }
}

/// The shortest run of blockers between `from` and any cell of `mainland`.
///
/// A breadth-first walk that ignores walkability entirely and keeps
/// predecessors, so the path it reports back is the fewest cells that have
/// to be opened — the carve takes a corridor, not a demolition.
fn corridor(
    board: &Board,
    mainland: &BTreeSet<(i32, i32)>,
    from: (i32, i32),
) -> Vec<(i32, i32)> {
    let mut came_from: std::collections::BTreeMap<(i32, i32), (i32, i32)> =
        std::collections::BTreeMap::new();
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(at) = queue.pop_front() {
        if mainland.contains(&at) {
            let mut path = Vec::new();
            let mut step = at;
            while step != from {
                if !board.walkable(step.0, step.1) {
                    path.push(step);
                }
                step = came_from[&step];
            }
            return path;
        }
        for (dx, dy) in NEIGHBOURS {
            let next = (at.0 + dx, at.1 + dy);
            if board.in_bounds(next.0, next.1) && seen.insert(next) {
                came_from.insert(next, at);
                queue.push_back(next);
            }
        }
    }
    Vec::new()
}
```

Then call it from `generate` — replace `generate`'s last two lines
(`.collect();` onward) so the whole function reads:

```rust
pub fn generate(spec: BattleSpec) -> Board {
    let side = spec.side();
    let cells = (0..side * side)
        .map(|i| kind_at(spec, i % side, i / side))
        .collect();
    let mut board = Board { side, cells };
    carve_to_connect(&mut board);
    board
}
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p feral-processes-engine tactical::map`

Expected: PASS, 16 tests. `no_generated_board_strands_anybody` walks 720
boards and takes a few seconds; that is the whole point of it and it is not
flaky — every board it generates is derived, not rolled.

- [ ] **Step 5: Prove the carve is load-bearing**

Temporarily comment out the `carve_to_connect(&mut board);` line in
`generate` and run the suite again.

Run: `cargo test -p feral-processes-engine tactical::map::tests::no_generated_board_strands_anybody`

Expected: FAIL. If it **passes**, the sweep is not finding a pocketed board
and the test is vacuous as written — widen the tick range from `0..60` to
`0..400` until it fails, then restore the carve. A test that passes with the
fix removed is not coverage. Restore the line before committing.

- [ ] **Step 6: Commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/tactical/map.rs
git commit -m "feat: no battle map strands a body

Flood-fill the walkable ground and carve the shortest run of blockers between
any stranded region and the mainland. A sealed pocket is rare at these rates
rather than impossible, and rare is the worst kind — it survives every test
run and then happens to a player, who finds a fight that cannot finish.

The flood fill is cost-blind on purpose and is not the movement field: 'can a
body get there at all' is a different question from 'how far this turn'."
```

---

### Task 6: The two sides onto the board

Deployment is sized for contact where the board is sized for manoeuvre: the
two sides go down a handful of cells apart, on the real bearing the pack was
found at, so walking into a pack from the side starts you flanked and no
fight opens with a walk toward the enemy.

**Files:**
- Create: `crates/engine/src/tactical/deploy.rs`
- Modify: `crates/engine/src/tactical/mod.rs` (declare the module)
- Modify: `crates/engine/src/tuning.rs` (the deployment gap)

**Interfaces:**
- Produces: `deploy::bearing(from, to) -> (i32, i32)`,
  `deploy::Deployment { party, wild }`, `deploy::plan(&Board, bearing,
  party: u32, wild: u32) -> Deployment`; `tuning::TACTICAL_DEPLOY_GAP`.
- Consumes: `map::Board`.

**The bearing is an argument, not something this module works out.** There is
no facing on a surface `Position` (`components.rs:21` is `{ x, y }` and
nothing else) and `gather_pack` (`game/combat.rs:155`) returns bare entities,
so the caller — a later phase — derives it from the player's tile and the
pack's centroid and hands it in. Keeping it a parameter is what lets every
test here be pure.

- [ ] **Step 1: Write the failing tests**

Create `crates/engine/src/tactical/deploy.rs` with only this test module for
now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactical::map::{generate, BattleSpec};
    use crate::world::Biome;

    fn board(bodies: u32) -> Board {
        generate(BattleSpec {
            world_seed: 31,
            site: (2, 2),
            tick: 44,
            zone: 2,
            biome: Biome::Backplane,
            bodies,
        })
    }

    #[test]
    fn a_bearing_is_the_eight_way_step_from_one_tile_toward_another() {
        assert_eq!(bearing((0, 0), (5, 0)), (1, 0));
        assert_eq!(bearing((0, 0), (0, -3)), (0, -1));
        assert_eq!(bearing((4, 4), (1, 9)), (-1, 1));
    }

    /// Two bodies on one tile is not a direction. A fixed answer rather
    /// than a panic, because a wild body standing exactly where the player
    /// stands is a bump, not a bug.
    #[test]
    fn a_bearing_to_your_own_tile_is_north() {
        assert_eq!(bearing((3, 3), (3, 3)), (0, -1));
    }

    #[test]
    fn everybody_gets_their_own_walkable_cell() {
        let board = board(9);
        let plan = plan(&board, (1, 0), 4, 5);
        assert_eq!(plan.party.len(), 4);
        assert_eq!(plan.wild.len(), 5);

        let mut all: Vec<(i32, i32)> = plan.party.iter().chain(plan.wild.iter()).copied().collect();
        let placed = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), placed, "two bodies were deployed onto one cell");
        for (x, y) in all {
            assert!(board.in_bounds(x, y), "({x}, {y}) is off the board");
            assert!(board.walkable(x, y), "({x}, {y}) cannot be stood on");
        }
    }

    /// Sized for contact, not for a walk — but not on top of each other
    /// either. The anchors are `TACTICAL_DEPLOY_GAP` apart along the
    /// bearing and the ranks spread perpendicular to it, so the closest
    /// pair is the gap less whatever drift a taken cell forced.
    #[test]
    fn the_two_sides_start_apart_on_every_bearing() {
        let board = board(9);
        for bearing in [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)] {
            let plan = plan(&board, bearing, 4, 5);
            let closest = plan
                .party
                .iter()
                .flat_map(|p| plan.wild.iter().map(move |w| (p.0 - w.0).abs().max((p.1 - w.1).abs())))
                .min()
                .expect("an empty deployment");
            assert!(closest >= 2, "bearing {bearing:?} deployed the sides at {closest}");
        }
    }

    /// The bearing is what makes a flank a flank: come at a pack from a
    /// different quarter and the two sides stand somewhere else.
    #[test]
    fn a_different_bearing_deploys_a_different_fight() {
        let board = board(9);
        assert_ne!(
            plan(&board, (1, 0), 4, 5).party,
            plan(&board, (0, 1), 4, 5).party
        );
    }

    #[test]
    fn the_same_board_and_bearing_deploy_identically() {
        let board = board(5);
        assert_eq!(plan(&board, (1, 1), 2, 3).party, plan(&board, (1, 1), 2, 3).party);
        assert_eq!(plan(&board, (1, 1), 2, 3).wild, plan(&board, (1, 1), 2, 3).wild);
    }

    /// The largest fight the game can field, on the smallest board it will
    /// ever be fought on, still seats everybody — which is the invariant
    /// `place` leans on when it says the board has room.
    #[test]
    fn the_smallest_board_seats_the_largest_fight() {
        let party = crate::tuning::MAX_PARTY_SIZE as u32;
        let wild = crate::tuning::MAX_PACK_BODIES;
        let board = board(2);
        assert_eq!(board.side, crate::tuning::TACTICAL_BOARD_SMALL);
        let plan = plan(&board, (1, 0), party, wild);
        assert_eq!(plan.party.len() + plan.wild.len(), (party + wild) as usize);
    }
}
```

- [ ] **Step 2: Run it and watch it fail**

Run: `cargo test -p feral-processes-engine tactical::deploy`

Expected: FAIL — `file not found for module 'deploy'`, because `mod.rs` does
not declare it yet.

- [ ] **Step 3: Add the gap to tuning**

Append to the "Tactical battle grid" section of `crates/engine/src/tuning.rs`:

```rust
/// Cells between the two sides' anchors at deployment.
///
/// The board is sized for manoeuvre and the deployment is sized for
/// contact: six cells is a step or two of closing rather than a march, so
/// no fight opens with both sides walking toward each other for a turn.
pub const TACTICAL_DEPLOY_GAP: i32 = 6;
```

- [ ] **Step 4: Write `deploy.rs`**

Put this at the top of `crates/engine/src/tactical/deploy.rs`, above the test
module:

```rust
//! Putting the two sides on the board.
//!
//! The board is sized for manoeuvre; this is sized for contact. The two
//! sides go down `TACTICAL_DEPLOY_GAP` cells apart on the bearing the pack
//! was actually found at, so walking into a pack from the side starts you
//! flanked and no fight opens with a march.

use std::collections::{BTreeSet, VecDeque};

use crate::tactical::map::Board;
use crate::tuning::TACTICAL_DEPLOY_GAP;

/// The eight-way step from one tile toward another.
///
/// A signum per axis rather than an angle: at these board sizes eight
/// directions are all the resolution a deployment can express, and it keeps
/// the whole thing integer. Two bodies on one tile answer north — a fixed
/// answer rather than a panic, because a wild body standing exactly where
/// the player stands is a bump, not a bug.
pub fn bearing(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    match ((to.0 - from.0).signum(), (to.1 - from.1).signum()) {
        (0, 0) => (0, -1),
        step => step,
    }
}

/// Where each side's bodies start.
///
/// Two lists rather than a map of entity to cell: this module never sees an
/// `Entity`, which is what keeps every test over it pure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deployment {
    pub party: Vec<(i32, i32)>,
    pub wild: Vec<(i32, i32)>,
}

/// Seats both sides.
///
/// The two anchors sit `TACTICAL_DEPLOY_GAP` apart either side of the
/// board's centre along `bearing`, and each side's rank fans out
/// perpendicular to it, nearest the anchor first. A body whose place is
/// blocked or already taken takes the nearest free walkable cell instead.
///
/// The board always has room: the smallest tier is 14x14 with at least 86%
/// of it walkable and the largest fight the game fields is
/// `MAX_PARTY_SIZE + MAX_PACK_BODIES` bodies, which
/// `the_smallest_board_seats_the_largest_fight` pins.
pub fn plan(board: &Board, bearing: (i32, i32), party: u32, wild: u32) -> Deployment {
    let centre = board.side / 2;
    let half = TACTICAL_DEPLOY_GAP / 2;
    let party_anchor = (centre - bearing.0 * half, centre - bearing.1 * half);
    let wild_anchor = (centre + bearing.0 * half, centre + bearing.1 * half);
    let across = (-bearing.1, bearing.0);

    let mut taken = BTreeSet::new();
    Deployment {
        party: rank(board, party_anchor, across, party, &mut taken),
        wild: rank(board, wild_anchor, across, wild, &mut taken),
    }
}

/// One side's line, fanning out from its anchor: the anchor itself, then a
/// cell to either side of it, then two, and so on.
fn rank(
    board: &Board,
    anchor: (i32, i32),
    across: (i32, i32),
    bodies: u32,
    taken: &mut BTreeSet<(i32, i32)>,
) -> Vec<(i32, i32)> {
    (0..bodies)
        .map(|i| {
            let step = fan(i);
            let want = (anchor.0 + across.0 * step, anchor.1 + across.1 * step);
            let cell = nearest_free(board, taken, want)
                .expect("the board has room for every body; see `plan`'s doc");
            taken.insert(cell);
            cell
        })
        .collect()
}

/// 0, +1, -1, +2, -2, … — the anchor first, then out alternately, so a
/// short rank is centred on its anchor rather than trailing off one side.
fn fan(i: u32) -> i32 {
    let step = (i as i32 + 1) / 2;
    if i % 2 == 1 {
        step
    } else {
        -step
    }
}

/// The nearest walkable, unclaimed cell to `want`, breadth-first.
///
/// `want` is clamped onto the board first: a long rank on a diagonal
/// bearing runs its outer bodies off the edge, and a search that starts
/// outside has nowhere to start from.
fn nearest_free(
    board: &Board,
    taken: &BTreeSet<(i32, i32)>,
    want: (i32, i32),
) -> Option<(i32, i32)> {
    let from = (
        want.0.clamp(0, board.side - 1),
        want.1.clamp(0, board.side - 1),
    );
    let mut seen = BTreeSet::from([from]);
    let mut queue = VecDeque::from([from]);
    while let Some(at) = queue.pop_front() {
        if board.walkable(at.0, at.1) && !taken.contains(&at) {
            return Some(at);
        }
        for (dx, dy) in crate::tactical::map::NEIGHBOURS {
            let next = (at.0 + dx, at.1 + dy);
            if board.in_bounds(next.0, next.1) && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    None
}
```

`NEIGHBOURS` is private to `map.rs` as written in Task 5. Widen it there —
change `const NEIGHBOURS` to `pub(crate) const NEIGHBOURS` — rather than
writing a second copy of the eight offsets in this file.

Declare the module in `crates/engine/src/tactical/mod.rs`, above `pub mod
map;`:

```rust
pub mod deploy;
```

- [ ] **Step 5: Run the tests and watch them pass**

Run: `cargo test -p feral-processes-engine tactical::deploy`

Expected: PASS, 7 tests.

If `the_two_sides_start_apart_on_every_bearing` fails on a diagonal bearing,
check `across`: it must be `(-bearing.1, bearing.0)`, the perpendicular. With
the anchors `GAP` apart along the bearing and the ranks purely perpendicular
to it, the closest pair before drift is `GAP` on every one of the eight
bearings, so a failure is a wiring error and not a tolerance to loosen.

- [ ] **Step 6: Commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/tuning.rs crates/engine/src/tactical/
git commit -m "feat: deploy the two sides onto the board

Anchors six cells apart either side of centre along the bearing the pack was
found at, ranks fanning out perpendicular, nearest free walkable cell when a
place is blocked or taken. Sized for contact where the board is sized for
manoeuvre: walk into a pack from the side and you start flanked.

The bearing is a parameter, not something this module works out — a surface
Position has no facing and gather_pack returns bare entities, so the caller
derives it and every test here stays pure."
```

---

### Task 7: `TacticalBattle` — where everybody stands

The resource the whole feature hangs off. In this phase it holds the spec,
the board and the occupancy, and nothing constructs it.

**Files:**
- Modify: `crates/engine/src/tactical/mod.rs`

**Interfaces:**
- Produces: `TacticalBattle` with `open`, `place`, `cell_of`, `occupant`,
  `move_to`, `remove`, `bodies`.
- Consumes: `map::{BattleSpec, Board}`, `bevy_ecs::prelude::{Entity,
  Resource}`.

Bare `#[derive(Resource)]` — no `Default`, no `Serialize` — matching
`BattleState` (`resources.rs:973`) for the same reason: a fight is never
saved, so nothing here reaches `save.rs` and `SAVE_FORMAT_VERSION` does not
move.

Occupancy is a `Vec<(Entity, (i32, i32))>` and not a `HashMap`. Two reasons,
both already established here: iteration order has to be stable, because
bevy's own query order is not and a system that walks the bodies must not
resolve differently between runs (`Stock`'s `BTreeMap` is the same rule); and
a fight holds at most thirteen bodies, so a linear scan is the simpler thing
that is also the faster thing.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tactical/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactical::map::{generate, BattleSpec};
    use crate::world::Biome;
    use bevy_ecs::world::World;

    fn fight() -> (TacticalBattle, Vec<Entity>) {
        let spec = BattleSpec {
            world_seed: 5,
            site: (0, 0),
            tick: 10,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 4,
        };
        let board = generate(spec);
        let mut world = World::new();
        let bodies = (0..3).map(|_| world.spawn_empty().id()).collect();
        (TacticalBattle::open(spec, board), bodies)
    }

    /// The one place a battle coordinate lives.
    #[test]
    fn a_body_stands_where_it_was_placed() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        assert!(battle.place(bodies[0], cell));
        assert_eq!(battle.cell_of(bodies[0]), Some(cell));
        assert_eq!(battle.occupant(cell), Some(bodies[0]));
    }

    fn first_open(battle: &TacticalBattle) -> (i32, i32) {
        battle
            .board
            .cells()
            .find(|(_, k)| k.walkable())
            .map(|(c, _)| c)
            .expect("a board with no ground")
    }

    #[test]
    fn two_bodies_never_share_a_cell() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        assert!(battle.place(bodies[0], cell));
        assert!(!battle.place(bodies[1], cell), "the cell was taken twice");
        assert_eq!(battle.occupant(cell), Some(bodies[0]));
    }

    #[test]
    fn nobody_stands_on_ground_they_cannot_stand_on() {
        let (mut battle, bodies) = fight();
        let blocked = battle
            .board
            .cells()
            .find(|(_, k)| !k.walkable())
            .map(|(c, _)| c)
            .expect("a board with nothing on it");
        assert!(!battle.place(bodies[0], blocked));
        assert_eq!(battle.cell_of(bodies[0]), None);
    }

    #[test]
    fn a_body_placed_twice_is_refused_rather_than_duplicated() {
        let (mut battle, bodies) = fight();
        let first = first_open(&battle);
        assert!(battle.place(bodies[0], first));
        let elsewhere = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != first)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        assert!(!battle.place(bodies[0], elsewhere), "the body was placed twice");
        assert_eq!(battle.bodies().count(), 1);
    }

    #[test]
    fn a_body_moves_and_leaves_its_cell_behind() {
        let (mut battle, bodies) = fight();
        let from = first_open(&battle);
        let to = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != from)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        battle.place(bodies[0], from);
        assert!(battle.move_to(bodies[0], to));
        assert_eq!(battle.cell_of(bodies[0]), Some(to));
        assert_eq!(battle.occupant(from), None);
    }

    #[test]
    fn a_body_cannot_move_onto_somebody_else() {
        let (mut battle, bodies) = fight();
        let a = first_open(&battle);
        let b = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != a)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        battle.place(bodies[0], a);
        battle.place(bodies[1], b);
        assert!(!battle.move_to(bodies[0], b));
        assert_eq!(battle.cell_of(bodies[0]), Some(a));
    }

    /// A body that dies or walks off the edge leaves, and takes its cell
    /// with it.
    #[test]
    fn a_removed_body_frees_its_cell() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        battle.place(bodies[0], cell);
        battle.remove(bodies[0]);
        assert_eq!(battle.cell_of(bodies[0]), None);
        assert_eq!(battle.occupant(cell), None);
        assert_eq!(battle.bodies().count(), 0);
    }

    /// Placement order is what `bodies` reports, every time. Bevy's own
    /// query order is not stable, so anything that walks a fight's bodies
    /// walks this instead.
    #[test]
    fn bodies_come_back_in_the_order_they_were_placed() {
        let (mut battle, bodies) = fight();
        let cells: Vec<(i32, i32)> = battle
            .board
            .cells()
            .filter(|(_, k)| k.walkable())
            .map(|(c, _)| c)
            .take(3)
            .collect();
        for (body, cell) in bodies.iter().zip(&cells) {
            battle.place(*body, *cell);
        }
        let order: Vec<Entity> = battle.bodies().map(|(e, _)| e).collect();
        assert_eq!(order, bodies);
    }
}
```

- [ ] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine tactical::tests`

Expected: FAIL to compile — `cannot find type 'TacticalBattle' in this
scope`.

- [ ] **Step 3: Write `TacticalBattle`**

In `crates/engine/src/tactical/mod.rs`, between the module declarations and
the test module:

```rust
use bevy_ecs::prelude::{Entity, Resource};

use crate::tactical::map::{BattleSpec, Board};

/// A tactical fight's spatial state: the map it is fought on and where
/// every body stands.
///
/// **The one place a battle coordinate lives.** No world `Position` is
/// written for a body on a battle map, the same way the Stack keeps its
/// coordinates in `resources::Locale` and base space keeps its own there
/// too. A body's `Position` stays exactly where the fight opened.
///
/// Bare `#[derive(Resource)]` — no `Default`, no `Serialize` — matching
/// `BattleState` for the same reason: a fight is never saved, so nothing
/// here appears in `save.rs` and `SAVE_FORMAT_VERSION` does not move.
#[derive(Resource)]
pub struct TacticalBattle {
    pub spec: BattleSpec,
    pub board: Board,
    /// A `Vec` and not a `HashMap`. Iteration order has to be stable —
    /// bevy's own query order is not, and anything that walks a fight's
    /// bodies must not resolve differently between runs, which is `Stock`'s
    /// `BTreeMap` rule again — and a fight holds at most thirteen bodies,
    /// so a linear scan is the simpler thing and also the faster one.
    bodies: Vec<(Entity, (i32, i32))>,
}

impl TacticalBattle {
    pub fn open(spec: BattleSpec, board: Board) -> Self {
        TacticalBattle {
            spec,
            board,
            bodies: Vec::new(),
        }
    }

    /// Puts a body on a cell, or refuses.
    ///
    /// Refused when the cell cannot be stood on, when somebody is already
    /// there, or when this body is already on the board — the last so a
    /// double placement is a refusal rather than a second entry that
    /// `cell_of` would answer from and `occupant` would not.
    pub fn place(&mut self, body: Entity, cell: (i32, i32)) -> bool {
        if !self.board.walkable(cell.0, cell.1)
            || self.occupant(cell).is_some()
            || self.cell_of(body).is_some()
        {
            return false;
        }
        self.bodies.push((body, cell));
        true
    }

    pub fn cell_of(&self, body: Entity) -> Option<(i32, i32)> {
        self.bodies
            .iter()
            .find(|(e, _)| *e == body)
            .map(|(_, cell)| *cell)
    }

    pub fn occupant(&self, cell: (i32, i32)) -> Option<Entity> {
        self.bodies
            .iter()
            .find(|(_, at)| *at == cell)
            .map(|(e, _)| *e)
    }

    /// Moves a placed body, or refuses. Standing still is allowed.
    pub fn move_to(&mut self, body: Entity, cell: (i32, i32)) -> bool {
        if !self.board.walkable(cell.0, cell.1) {
            return false;
        }
        match self.occupant(cell) {
            Some(other) if other != body => return false,
            _ => {}
        }
        let Some(slot) = self.bodies.iter_mut().find(|(e, _)| *e == body) else {
            return false;
        };
        slot.1 = cell;
        true
    }

    /// Takes a body off the board — killed, or walked off the edge.
    pub fn remove(&mut self, body: Entity) {
        self.bodies.retain(|(e, _)| *e != body);
    }

    /// Every body and where it stands, in placement order.
    pub fn bodies(&self) -> impl Iterator<Item = (Entity, (i32, i32))> + '_ {
        self.bodies.iter().copied()
    }
}
```

- [ ] **Step 4: Run the tests and watch them pass**

Run: `cargo test -p feral-processes-engine tactical::tests`

Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/tactical/mod.rs
git commit -m "feat: TacticalBattle — where every body stands

The one place a battle coordinate lives; no world Position is written for a
body on a battle map. Bare Resource, no Serialize, matching BattleState: a
fight is never saved, so nothing reaches save.rs.

Occupancy is a Vec and not a HashMap because iteration order has to be
stable — bevy's query order is not — and a fight holds at most thirteen
bodies."
```

---

### Task 8: The seam, and the gates

This phase mints one load-bearing seam. A seam is **three writes, in this
order** — invoke the `seams` skill and follow it rather than working from
memory here.

**Files:**
- Modify: `docs/seams.md` (a new `###` section)
- Modify: `.claude/skills/seams/references/combat.md` (a bullet)
- Modify: `CLAUDE.md` (a new `### Tactical battles` heading with one bullet,
  placed after the `### Combat, progression and balance` section)

**The seam:** *A battle map's coordinates live in `TacticalBattle`;
`Position` is never written.*

Only this one. The other three the spec lists belong to later phases and must
not be written now: `ability_recipients` as the shared door is Phase 5,
`start_battle` as the router is the phase that adds the router, and
`walk_field`'s cost rule is Phase 3. A seam doc that describes code which
does not exist yet is a trap this repo has already been caught by.

- [ ] **Step 1: Invoke the skill**

Run the `seams` skill and read
`.claude/skills/seams/references/combat.md` in full — the traps
cross-reference each other and a single bullet read alone loses them.

- [ ] **Step 2: The argument, to `docs/seams.md`**

Add a new `###` section titled exactly **A battle map's coordinates live in
`TacticalBattle`; `Position` is never written**, carrying: that this is the
third instance of a rule the Stack (`resources::Locale`) and base space
(`Locale::Base`) already settled; that a body in a fight keeps the `Position`
it had when the fight opened, which is what lets teardown be a matter of
dropping a resource rather than of putting everyone back; that the trap is
specifically the *convenience* of writing a battle cell into `Position` so
existing map code can draw it, which would move a creature on the world map
and, on a fight that ends badly, leave it there; and that the compiler holds
none of this — `tactical/` simply does not import `Position`, and that
omission is the whole enforcement.

- [ ] **Step 3: The trap, to the skill's reference**

Add a bullet to `.claude/skills/seams/references/combat.md` in that file's
existing house style — the bold rule sentence, then the trap in a sentence or
two. Do not restate the argument; point at it.

- [ ] **Step 4: The rule, to `CLAUDE.md`**

Add a new section after `### Combat, progression and balance`:

```markdown
### Tactical battles

- **A battle map's coordinates live in `TacticalBattle`; `Position` is never
  written** — the third space after the Stack's `Locale` and base space's
  own, and `tactical/` not importing `Position` is the whole enforcement.
```

**Exactly one sentence.** That budget is the point — `CLAUDE.md` is loaded on
every turn and reached 151 KB by letting each seam's trap creep back in
beside its rule.

- [ ] **Step 5: Run the full gates**

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

Expected: clean `fmt`, no `clippy` warnings, and the whole workspace suite
green. The count is above the 5061 in `CLAUDE.md`'s Build & test section by
the tests this phase added; do not update that number — it is the user's, and
it moves at the merge.

- [ ] **Step 6: Commit**

```bash
git add docs/seams.md .claude/skills/seams/references/combat.md CLAUDE.md
git commit -m "docs: the seam phase 2 mints

A battle map's coordinates live in TacticalBattle and no Position is ever
written for a body on one — the third instance of a rule the Stack and base
space already settled. Three writes: the argument to docs/seams.md, the trap
to the seams skill, the one-sentence rule to CLAUDE.md.

The other three seams the spec lists belong to later phases and are
deliberately not written yet."
```

---

## What this phase deliberately does not do

Stated so a reviewer does not read them as gaps:

- **Nothing calls `tactical::`.** `Game::start_battle` is untouched. The
  router that chooses between the two combat models arrives with the turn
  model that can drive one.
- **No movement field.** `region` answers connectivity and nothing else;
  `walk_field`'s widening from a predicate to a cost function is Phase 3, and
  `BattleCell::movement_cost` is already the shape it widens to.
- **No initiative, no turns, no teardown.** Phase 4.
- **No shapes, no ranges, no `ability_recipients` arm.** Phase 5.
- **No AI.** Phase 6.
- **Nothing drawn.** `biome_tint` stays in `crates/gui`; the engine gains no
  colour concept for a battle cell. Phase 7.
- **No `dev-saves` template.** Phase 8, where there is something to look at.
