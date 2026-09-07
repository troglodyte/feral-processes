# A Server Growing Into A Mainframe — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `Server` on the world map grows into a `Mainframe` on a randomized, seed-derived schedule that trade with it pulls forward, and a grown city's shelf thins when it is neglected or hated.

**Architecture:** Two axes hung off `settlements::Relation`. `grown: bool` is a one-way latch, set in exactly one place by `Game::settlement_growth_tick` when `now >= due_tick(seed, key) − commerce × PULL`. `commerce: i32` is a signed scalar that trade raises, a decay epoch lowers, and Hostile standing lowers faster; it bands (never stored) into Thriving/Steady/Starved, which sets a grown city's shelf rows between the Server floor and the Mainframe ceiling. Every reader goes through one door, `Game::settlement_kind`, so a `def.kind` can never leak back into a call site.

**Tech Stack:** Rust 2021, `bevy_ecs` world/resources, `serde` + RON saves, `cargo test`. No new dependencies. **No `GameRng` or `StdRng` anywhere in this feature** — the world clock is an FNV-1a fold, not a roll.

**Spec:** `docs/superpowers/specs/2026-09-06-settlement-growth-design.md` — read it before Task 1. This plan argues from it.

## Global Constraints

- **Crate:** all engine work is `crates/engine`, package `feral-processes-engine`. GUI work is `crates/gui`, package `feral-processes-gui`.
- **Iterate with `cargo test -p feral-processes-engine <filter>`; gate every task with `cargo test --workspace`.** These are different builds and a single-crate run shifts the RNG stream relative to a workspace run — a test that is green under `-p` can be red under `--workspace` and vice versa.
- **Never pipe a cargo command into `tail`/`grep`.** The pipeline's exit status replaces cargo's, so a failing build reads as a pass. Run it bare and read the output.
- **No `%` in any derivation.** `derive::index(seed, len)` is the only reducer — it reads the *high* bits. A `%` against a small span silently anti-correlates neighbouring regions. This is `placement.rs`'s stated rule and it applies to `growth.rs` unchanged.
- **No `GameRng`, no `StdRng` sequence** in the growth derivation, for `placement.rs`'s two stated reasons: a `GameRng` draw does not survive a save/load and shifts every later roll, and an `StdRng` sequence is not stable across a `rand` upgrade.
- **All new `Relation` fields carry `#[serde(default)]`.** No `SAVE_FORMAT_VERSION` bump in this plan. If you find yourself wanting one, stop and raise it.
- **Every new tuning constant carries a doc comment saying why that shape**, matching the prose density of its neighbours in `crates/engine/src/tuning.rs`. A bare `pub const X: i32 = 3;` will be rejected at review.
- **Commit after every task.** End every commit message with:
  `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`
- **Do not push.** Landing is a separate, explicitly-requested step.

---

### Task 1: The run-length measurement

The spec's §7 in full: `SETTLEMENT_GROWTH_DUE_MIN`/`_MAX` are the two numbers that can make this whole feature ship dead. Too small and growth is instant and weightless; too large and the ambient half never fires inside a real session. This task establishes the anchor **before** any constant is written, and it is the one task that produces no code.

**Files:**
- Create: `docs/measurements/2026-09-06-run-length.md`
- Modify: `docs/measurements/README.md` (add the new row to its index)

**Interfaces:**
- Consumes: nothing.
- Produces: ratified values for `SETTLEMENT_GROWTH_DUE_MIN` and `SETTLEMENT_GROWTH_DUE_MAX`, which Task 2 writes into `tuning.rs`.

- [ ] **Step 1: Read the tick every shipped dev-save sits at**

```bash
grep -o 'tick: *[0-9]*' dev-saves/*.ron | sort -u
```

Expected, as of this writing:

```
dev-saves/chains.ron:tick: 5344
dev-saves/contracts.ron:tick: 6944
dev-saves/deep-lair.ron:tick: 6363
dev-saves/extraction.ron:tick: 5000
dev-saves/extraction.ron:tick: 5350
dev-saves/rarity-preview.ron:tick: 5344
dev-saves/settlements.ron:tick: 5000
dev-saves/settlements.ron:tick: 5422
dev-saves/stack.ron:tick: 5344
```

- [ ] **Step 2: Record what that evidence is and is not**

These templates were produced by *driving* the engine to a mature state — a base raised, programs tamed, research topped up — not by playing. A driver skips the ticks a player spends walking, resting and reading screens. So **5,300–6,900 is a floor on how long a mature run takes**, not an estimate of one. Say this explicitly in the measurement doc; a later reader who takes it as an estimate will tune the constants too low.

- [ ] **Step 3: Cross-check against the longest cadence already shipped**

```bash
grep -n 'pub const [A-Z_]*TICKS: u64' crates/engine/src/tuning.rs
```

The longest is `SETTLEMENT_BOARD_ROTATION_TICKS = 1800`. Record it: a growth clock must be several multiples of the slowest thing the player already waits on, or growth reads as another rotation rather than as the world changing.

- [ ] **Step 4: Write the measurement doc**

Create `docs/measurements/2026-09-06-run-length.md` covering, in prose:

1. The question — how many ticks does a run actually reach, and therefore what span can an ambient world clock occupy without firing constantly or never firing.
2. The evidence from Step 1, with the floor caveat from Step 2.
3. The cadence comparison from Step 3.
4. **The conclusion and the two numbers**, with this derivation stated:
   - `SETTLEMENT_GROWTH_DUE_MIN = 3000` — below the floor at which a run matures, so the earliest-dated towns in a world grow inside a run that never trades with them. Still 1.7x the longest existing cadence, so growth cannot read as a rotation.
   - `SETTLEMENT_GROWTH_DUE_MAX = 12000` — roughly twice the observed maturity floor. Towns dated in the upper half of the span do **not** grow on the clock alone within a typical run; they grow only if the player trades them forward, which is the whole point of the trade accelerant.
   - Therefore, in a mature ~6,000-tick run, roughly a **third** of a world's towns are past their date on the clock alone. That fraction is the number a playtest should challenge.
5. **What would falsify this.** State it plainly: if real sessions turn out to run 15,000+ ticks, `_MAX` is too low and every town grows; if they run under 3,000, nothing ever does. Name the observation that would settle it — a real play session's tick at save.

- [ ] **Step 5: If the evidence contradicts the numbers, change the numbers here and only here**

If Step 1's ticks come back materially different from the figures above (the templates get rebuilt, or the tick meaning changes), pick `_MIN`/`_MAX` from the evidence you actually have, using Step 4's reasoning — below the maturity floor, and about twice it — and write **your** numbers into the doc. Task 2 quotes this document, not this plan.

- [ ] **Step 6: Add the index row**

Open `docs/measurements/README.md`, find how existing rows are formatted, and add one for `2026-09-06-run-length.md` in the same shape. Match the surrounding style exactly; do not invent a new column.

- [ ] **Step 7: Commit**

```bash
git add docs/measurements/2026-09-06-run-length.md docs/measurements/README.md
git commit -m "docs(measurement): how many ticks a run actually reaches

The anchor for SETTLEMENT_GROWTH_DUE_MIN/_MAX. Shipped dev-save
templates sit at ticks 5344-6944, which is a floor on a mature run
rather than an estimate of one: they were driven, not played.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: `settlements/growth.rs` — the pure half

Everything derivable, with no `Game` and no world: the due-tick fold, the commerce clamp, the vitality banding, and the row/share mapping. All twelve constants and all four compile-time assertions land here too, because a constant with no reader is a constant nobody can check.

**Files:**
- Create: `crates/engine/src/settlements/growth.rs`
- Modify: `crates/engine/src/settlements/mod.rs` (declare and re-export)
- Modify: `crates/engine/src/settlements/placement.rs` (raise two helpers to `pub(super)`)
- Modify: `crates/engine/src/tuning.rs` (twelve constants)
- Test: inside `growth.rs`, a `#[cfg(test)] mod tests` — `placement.rs`'s own precedent for a derivation's tests living beside it

**Interfaces:**
- Consumes: `SettlementKey`, `SettlementKind` (`settlements/mod.rs`); `derive::index`; Task 1's ratified `_MIN`/`_MAX`.
- Produces:
  - `pub fn due_tick(seed: u32, key: SettlementKey) -> u64`
  - `pub fn clamp_commerce(commerce: i32) -> i32`
  - `pub fn pull_ticks(commerce: i32) -> i64`
  - `pub enum Vitality { Starved, Steady, Thriving }` with `pub fn label(self) -> &'static str`, `pub(crate) fn rows(self) -> u32`, `pub(crate) fn bonus_share(self) -> u32`
  - `pub fn vitality(commerce: i32) -> Vitality`
  - `pub(super) fn region_seed(...)` and `pub(super) fn salted(...)` in `placement.rs`

- [ ] **Step 1: Raise the two fold helpers in `placement.rs`**

In `crates/engine/src/settlements/placement.rs`, change exactly two signatures. `fold` stays private — only `placement` needs the raw fold.

```rust
pub(super) fn region_seed(seed: u32, key: SettlementKey) -> u64 {
```

```rust
pub(super) fn salted(base: u64, salt: u64) -> u64 {
```

Add one sentence to `salted`'s doc comment:

```rust
/// Continues the fold with one more word, so each question off a region
/// gets its own answer without a second scheme.
///
/// `pub(super)` because growth is a fifth such question and asks it off the
/// same base — a second fold for the growth clock would be exactly the
/// colliding scheme this function exists to prevent.
```

- [ ] **Step 2: Write the twelve constants into `tuning.rs`**

Place them immediately after `SETTLEMENT_MAINFRAME_BONUS_SHARE` (currently line 3367), so the shelf-scale constants and the ones that now modulate them read together.

```rust
/// A grown-but-Steady city's shelf rows — `growth::Vitality::rows`.
///
/// Between the shipped `SETTLEMENT_SERVER_ROWS` (6) and
/// `SETTLEMENT_MAINFRAME_ROWS` (14), and it is the value an *untouched*
/// city draws: `SETTLEMENT_COMMERCE_*` bands 0 as Steady, and 0 is where
/// every authored Mainframe starts in every fresh world. So this, not 14,
/// is the number a player meets first.
pub const SETTLEMENT_STEADY_ROWS: u32 = 10;

/// A Steady city's standout share, `SETTLEMENT_STEADY_ROWS`' companion and
/// midway between the two shipped shares for its reason.
pub const SETTLEMENT_STEADY_BONUS_SHARE: u32 = 25;

/// The earliest tick a Server can grow into a Mainframe on the clock alone
/// — `growth::due_tick`'s floor.
///
/// **Measured, not guessed**: see `docs/measurements/2026-09-06-run-length.md`.
/// Shipped dev-save templates reach a mature run state at ticks 5344-6944,
/// which is a *floor* on how long a real run takes (a driven template skips
/// the ticks a player spends walking and reading). This sits below that
/// floor so the earliest-dated towns in a world grow inside a run that
/// never trades with them, and still 1.7x `SETTLEMENT_BOARD_ROTATION_TICKS`
/// — the longest cadence otherwise shipped — so growth cannot read as one
/// more rotation.
pub const SETTLEMENT_GROWTH_DUE_MIN: u64 = 3000;

/// The latest tick a Server can grow on the clock alone.
///
/// Roughly twice the measured maturity floor, which is deliberate: towns
/// dated in the upper half of this span do **not** grow within a typical run
/// unless the player trades them forward. A span that every town cleared on
/// time would make `SETTLEMENT_COMMERCE_PULL_TICKS` decorative.
pub const SETTLEMENT_GROWTH_DUE_MAX: u64 = 12000;

/// Salts the growth clock apart from `placement.rs`' four questions and from
/// `SETTLEMENT_MARKET_SALT`. Own constant, `CARAVAN_SALT`'s rule: one fold,
/// salted per question, so no two questions off a region can collide.
pub const SETTLEMENT_GROWTH_SALT: u64 = 0x5E77_1E5E_5EED_0005;

/// How many Credits of trade buy one point of commerce —
/// `Game::credit_trade_volume`'s second reading of the same volume.
///
/// **Cheaper than `SETTLEMENT_TRADE_CREDITS_PER_POINT` (250) on purpose.**
/// If the two were equal, commerce and standing would move in lockstep off
/// one input and the second axis would be a second spelling of the first.
/// At 150, commerce responds visibly faster than goodwill does — which is
/// the right asymmetry: a town notices your money before it likes you.
pub const SETTLEMENT_COMMERCE_CREDITS_PER_POINT: u32 = 150;

/// How many ticks one point of commerce pulls a Server's growth date
/// earlier — `growth::pull_ticks`.
///
/// At `SETTLEMENT_COMMERCE_MAX` (100) this is 2500 ticks, which is most of
/// the way from `SETTLEMENT_GROWTH_DUE_MAX` toward `_MIN` but provably
/// never past it — see the `const _` in `growth.rs`. Trade is meant to be
/// the difference between a town that grows this run and one that does not,
/// not a button that founds a city.
pub const SETTLEMENT_COMMERCE_PULL_TICKS: u64 = 25;

/// How long one drift epoch is — `Game::settlement_growth_tick` settles
/// commerce against `now / this`, `static_epoch`'s shape, so a fast-forward
/// cannot be outrun and no per-tick arithmetic runs over every town.
pub const SETTLEMENT_COMMERCE_DECAY_TICKS: u64 = 600;

/// Points of commerce a town loses per epoch to plain neglect.
///
/// Two per 600 ticks costs a city about 20 points across a mature run — a
/// drift the player can outpace with modest trade and will not notice if
/// they are using the town at all. Neglect starves a city over roughly
/// 12000 ticks, which is a run's worth of ignoring it, not a punishment for
/// a quiet week.
pub const SETTLEMENT_COMMERCE_DECAY: i32 = 2;

/// **Extra** points lost per epoch while the town's band is `Hostile` — on
/// top of `SETTLEMENT_COMMERCE_DECAY`, not instead of it.
///
/// Triples the drift, so a city you have turned Hostile starves in about a
/// third of the time neglect alone would take. Read off the *current* band
/// and never a history: repairing standing stops the acceleration the tick
/// it lands.
pub const SETTLEMENT_COMMERCE_HOSTILE_DECAY: i32 = 4;

/// The floor every commerce writer clamps to — `growth::clamp_commerce`,
/// `relations::clamp`'s rule and the same reason: one clamp is only enough
/// because there is one door.
///
/// Mirrors `SETTLEMENT_MIN_STANDING`/`_MAX_STANDING`'s +/-100 axis so the two
/// per-town numbers read on the same scale.
pub const SETTLEMENT_COMMERCE_MIN: i32 = -100;

/// The ceiling. See `SETTLEMENT_COMMERCE_MIN`.
pub const SETTLEMENT_COMMERCE_MAX: i32 = 100;

/// At or above this, a grown city is Thriving and draws a full Mainframe
/// shelf — `growth::vitality`.
///
/// **Must be strictly above zero**, and `growth.rs` asserts it at compile
/// time. Zero is where an untouched authored Mainframe sits, and a
/// threshold at or below zero would band Tally Yard and Kernel Reach as
/// Thriving in every fresh world before anyone had traded a Credit.
pub const SETTLEMENT_COMMERCE_THRIVING: i32 = 40;

/// At or below this, a grown city is Starved and its shelf falls to the
/// Server floor.
///
/// **Must be strictly below zero**, asserted in `growth.rs`, for
/// `SETTLEMENT_COMMERCE_THRIVING`'s mirrored reason: a threshold at or above
/// zero would open every authored Mainframe Starved.
pub const SETTLEMENT_COMMERCE_STARVED: i32 = -40;
```

If Task 1 ratified different `_MIN`/`_MAX` values, use those and reword the two doc comments to match the measurement you actually recorded.

- [ ] **Step 3: Write the failing tests in a new `growth.rs`**

Create `crates/engine/src/settlements/growth.rs` containing **only** the module doc and the test module, so it fails to compile for the right reason.

```rust
//! When a Server becomes a Mainframe, and how well a Mainframe is doing.
//!
//! Two axes and only one of them is stored. **The clock is derived** — a
//! due tick folded out of the world seed and a region, on
//! `placement.rs`'s machinery and under its three prohibitions: no
//! `GameRng` (a draw does not survive a save/load and shifts every later
//! roll), no `StdRng` sequence (not stable across a `rand` upgrade), and
//! **never `%`** (see `derive::index`; a `%` against a small span reads the
//! low bits and anti-correlates neighbouring regions).
//!
//! **The scalar is signed and 0 means "as it was found."** That is the
//! whole reason it is not a counter: an unsigned commerce starting at zero
//! would band every authored Mainframe as Starved in every fresh world,
//! before anyone had traded with it. Signed, zero bands Steady and the
//! player meets a city as its author wrote it.
//!
//! Nothing here reads a `Game`. `game/settlement_growth.rs` is the half
//! that does.

use crate::tuning::{
    SETTLEMENT_COMMERCE_MAX, SETTLEMENT_COMMERCE_MIN, SETTLEMENT_COMMERCE_PULL_TICKS,
    SETTLEMENT_COMMERCE_STARVED, SETTLEMENT_COMMERCE_THRIVING, SETTLEMENT_GROWTH_DUE_MAX,
    SETTLEMENT_GROWTH_DUE_MIN, SETTLEMENT_GROWTH_SALT, SETTLEMENT_MAINFRAME_BONUS_SHARE,
    SETTLEMENT_MAINFRAME_ROWS, SETTLEMENT_SERVER_BONUS_SHARE, SETTLEMENT_SERVER_ROWS,
    SETTLEMENT_STEADY_BONUS_SHARE, SETTLEMENT_STEADY_ROWS,
};

use super::SettlementKey;

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(n: i32) -> impl Iterator<Item = SettlementKey> {
        (-n..n).flat_map(move |ry| (-n..n).map(move |rx| SettlementKey { rx, ry }))
    }

    /// The whole point of a derived clock: ask twice, get the same answer.
    /// A due tick that drifted between calls would make a town grow and
    /// un-grow as the player walked in and out of range.
    #[test]
    fn a_region_is_due_at_the_same_tick_every_time() {
        for key in keys(6) {
            assert_eq!(due_tick(4242, key), due_tick(4242, key), "{key:?} drifted");
        }
    }

    /// Two worlds are two different schedules. Without this the seed is not
    /// reaching the fold and every run would grow the same towns at the
    /// same moments.
    #[test]
    fn a_different_seed_schedules_the_towns_differently() {
        let schedule = |seed| keys(8).map(|k| due_tick(seed, k)).collect::<Vec<_>>();
        assert_ne!(schedule(4242), schedule(99));
    }

    /// Every date is inside the authored span. A fold that escaped the span
    /// would put a town's date at tick 0 (grown from the first frame, in
    /// every world) or past `u64` sanity.
    #[test]
    fn every_due_tick_lands_inside_the_authored_span() {
        for key in keys(10) {
            let due = due_tick(4242, key);
            assert!(
                (SETTLEMENT_GROWTH_DUE_MIN..SETTLEMENT_GROWTH_DUE_MAX).contains(&due),
                "{key:?} is due at {due}, outside \
                 {SETTLEMENT_GROWTH_DUE_MIN}..{SETTLEMENT_GROWTH_DUE_MAX}"
            );
        }
    }

    /// The `%` trap, `placement.rs`'s own test one axis over. Reduced with
    /// `%` against a span, neighbouring regions read one low bit of a
    /// coordinate that differs by one and anti-correlate — which would show
    /// up as the map growing in stripes. Asserted as span coverage: a
    /// stripe cannot fill every tenth of the range.
    #[test]
    fn due_ticks_spread_across_the_span_rather_than_clustering() {
        let span = SETTLEMENT_GROWTH_DUE_MAX - SETTLEMENT_GROWTH_DUE_MIN;
        let mut buckets = [0usize; 10];
        for key in keys(16) {
            let offset = due_tick(4242, key) - SETTLEMENT_GROWTH_DUE_MIN;
            buckets[((offset * 10 / span) as usize).min(9)] += 1;
        }
        for (i, count) in buckets.iter().enumerate() {
            assert!(*count > 0, "no town is ever due in tenth {i}: {buckets:?}");
        }
    }

    /// Zero is Steady, and it is the assertion the whole signed design
    /// rests on: zero is where every authored Mainframe starts.
    #[test]
    fn an_untouched_town_is_steady() {
        assert_eq!(vitality(0), Vitality::Steady);
    }

    #[test]
    fn the_bands_answer_at_their_own_thresholds() {
        assert_eq!(vitality(SETTLEMENT_COMMERCE_THRIVING), Vitality::Thriving);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_THRIVING - 1), Vitality::Steady);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_STARVED), Vitality::Starved);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_STARVED + 1), Vitality::Steady);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_MAX), Vitality::Thriving);
        assert_eq!(vitality(SETTLEMENT_COMMERCE_MIN), Vitality::Starved);
    }

    /// A starved city is never worse than a town. The floor is the label's
    /// warrant: an `M` that drew fewer rows than an `s` would be a lie the
    /// page keeps telling.
    #[test]
    fn a_starved_city_never_draws_fewer_rows_than_a_server() {
        for commerce in [SETTLEMENT_COMMERCE_MIN, -1000, SETTLEMENT_COMMERCE_STARVED] {
            assert!(vitality(clamp_commerce(commerce)).rows() >= SETTLEMENT_SERVER_ROWS);
            assert!(
                vitality(clamp_commerce(commerce)).bonus_share() >= SETTLEMENT_SERVER_BONUS_SHARE
            );
        }
    }

    /// And never better than the ceiling the shipped constant already sets.
    #[test]
    fn a_thriving_city_never_draws_more_rows_than_a_mainframe() {
        for commerce in [SETTLEMENT_COMMERCE_MAX, 1000] {
            assert_eq!(vitality(clamp_commerce(commerce)).rows(), SETTLEMENT_MAINFRAME_ROWS);
            assert_eq!(
                vitality(clamp_commerce(commerce)).bonus_share(),
                SETTLEMENT_MAINFRAME_BONUS_SHARE
            );
        }
    }

    /// The clamp is the one every writer goes through, so it has to hold at
    /// both ends and be idempotent.
    #[test]
    fn commerce_clamps_at_both_ends() {
        assert_eq!(clamp_commerce(i32::MAX), SETTLEMENT_COMMERCE_MAX);
        assert_eq!(clamp_commerce(i32::MIN), SETTLEMENT_COMMERCE_MIN);
        assert_eq!(clamp_commerce(0), 0);
        assert_eq!(clamp_commerce(clamp_commerce(999)), SETTLEMENT_COMMERCE_MAX);
    }

    /// Negative commerce pushes the date *later*, which is how a neglected
    /// or Hostile Server stalls without a second rule for stalling.
    #[test]
    fn negative_commerce_pushes_the_date_later() {
        assert!(pull_ticks(-10) < 0);
        assert_eq!(pull_ticks(0), 0);
        assert!(pull_ticks(10) > 0);
    }
}
```

- [ ] **Step 4: Run the tests and watch them fail to compile**

```bash
cargo test -p feral-processes-engine settlements::growth
```

Expected: FAIL. `growth.rs` is not declared in `settlements/mod.rs` yet, and once it is, `due_tick`, `vitality`, `clamp_commerce`, `pull_ticks` and `Vitality` are undefined. That is the correct failure — the tests name the interface before it exists.

- [ ] **Step 5: Declare the module**

In `crates/engine/src/settlements/mod.rs`, beside the three existing declarations and re-exports:

```rust
pub mod catalogue;
pub mod growth;
pub mod placement;
pub mod relations;

pub use catalogue::{SettlementDb, SettlementDef};
pub use growth::Vitality;
pub use placement::{SettlementKey, settlement_at};
pub use relations::{Relation, Standing};
```

- [ ] **Step 6: Write the implementation into `growth.rs`**

Insert above the `#[cfg(test)] mod tests` block:

```rust
/// The tick this region's Server is due to become a Mainframe on the clock
/// alone, before any trade pulls it forward.
///
/// A fifth salted question off `placement::region_seed`, and deliberately
/// not a fifth fold: `salted`'s doc states why one base with per-question
/// salts is the scheme, and a second base here could collide with the four
/// that decide whether a town exists, where, and which.
pub fn due_tick(seed: u32, key: SettlementKey) -> u64 {
    let span = (SETTLEMENT_GROWTH_DUE_MAX - SETTLEMENT_GROWTH_DUE_MIN).max(1) as usize;
    let base = super::placement::region_seed(seed, key);
    let offset = crate::derive::index(
        super::placement::salted(base, SETTLEMENT_GROWTH_SALT),
        span,
    );
    SETTLEMENT_GROWTH_DUE_MIN + offset as u64
}

/// How many ticks `commerce` moves a Server's due date, signed.
///
/// Positive pulls it earlier; **negative pushes it later**, which is how a
/// neglected or Hostile Server stalls without a second rule that could
/// disagree with this one. `i64` because the answer is a signed offset
/// against a `u64` clock and the caller does the saturating arithmetic.
pub fn pull_ticks(commerce: i32) -> i64 {
    commerce as i64 * SETTLEMENT_COMMERCE_PULL_TICKS as i64
}

/// The bounds every commerce writer clamps to.
///
/// `relations::clamp`'s rule and its reason: one clamp is enough only
/// because there is one door. If a second writer ever appears, it goes
/// through `Game::adjust_commerce`, not around this.
pub fn clamp_commerce(commerce: i32) -> i32 {
    commerce.clamp(SETTLEMENT_COMMERCE_MIN, SETTLEMENT_COMMERCE_MAX)
}

/// How well a grown city is doing — **derived on every read, never
/// stored**.
///
/// `relations::band`'s rule exactly, and for its reason: a retune of the
/// thresholds re-bands every existing save rather than leaving cities filed
/// under a boundary that has moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Vitality {
    Starved,
    Steady,
    Thriving,
}

/// The one banding. Ordered from the bottom, `relations::band`'s shape.
pub fn vitality(commerce: i32) -> Vitality {
    if commerce <= SETTLEMENT_COMMERCE_STARVED {
        Vitality::Starved
    } else if commerce < SETTLEMENT_COMMERCE_THRIVING {
        Vitality::Steady
    } else {
        Vitality::Thriving
    }
}

impl Vitality {
    pub fn label(self) -> &'static str {
        match self {
            Vitality::Starved => "Starved",
            Vitality::Steady => "Steady",
            Vitality::Thriving => "Thriving",
        }
    }

    /// How many shelf rows a grown city at this band draws.
    ///
    /// Exhaustive on purpose, `Temperament::buy_mult`'s reason: a fourth
    /// band with no row count is one that reads as broken rather than as
    /// neutral. **A Server never asks this** — it has nowhere to fall to,
    /// and its commerce is spoken for by `due_tick`.
    pub(crate) fn rows(self) -> u32 {
        match self {
            Vitality::Starved => SETTLEMENT_SERVER_ROWS,
            Vitality::Steady => SETTLEMENT_STEADY_ROWS,
            Vitality::Thriving => SETTLEMENT_MAINFRAME_ROWS,
        }
    }

    /// This band's standout share. Exhaustive, `rows`' reason.
    pub(crate) fn bonus_share(self) -> u32 {
        match self {
            Vitality::Starved => SETTLEMENT_SERVER_BONUS_SHARE,
            Vitality::Steady => SETTLEMENT_STEADY_BONUS_SHARE,
            Vitality::Thriving => SETTLEMENT_MAINFRAME_BONUS_SHARE,
        }
    }
}

/// Zero must band Steady, at both ends.
///
/// **A compile-time assertion and not a test**, `SETTLEMENT_GARRISON_MAX`'s
/// precedent: the module doc's whole argument for a signed scalar is that an
/// untouched authored Mainframe reads as its author wrote it, and closing
/// that by retune must fail the *build* rather than one test in a suite
/// somebody could mark ignored.
const _: () = assert!(SETTLEMENT_COMMERCE_STARVED < 0);
const _: () = assert!(0 < SETTLEMENT_COMMERCE_THRIVING);

/// The dwindle never falls through the Server floor, and never rises past
/// the Mainframe ceiling. An `M` drawing fewer rows than an `s` would make
/// the label a lie. A `const _` for the reason above.
const _: () = assert!(SETTLEMENT_SERVER_ROWS <= SETTLEMENT_STEADY_ROWS);
const _: () = assert!(SETTLEMENT_STEADY_ROWS <= SETTLEMENT_MAINFRAME_ROWS);
const _: () = assert!(SETTLEMENT_SERVER_BONUS_SHARE <= SETTLEMENT_STEADY_BONUS_SHARE);
const _: () = assert!(SETTLEMENT_STEADY_BONUS_SHARE <= SETTLEMENT_MAINFRAME_BONUS_SHARE);

/// The span `derive::index` reduces into must be non-empty, or the fold
/// divides into nothing.
const _: () = assert!(SETTLEMENT_GROWTH_DUE_MIN < SETTLEMENT_GROWTH_DUE_MAX);

/// **Trade can never found a city at tick zero.** The maximum pull is
/// strictly less than the earliest possible due date, so a player with
/// unlimited Credits still cannot have a Mainframe before
/// `SETTLEMENT_GROWTH_DUE_MIN - (MAX x PULL)` — the clock is always doing
/// some of the work. A `const _` because a retune that closed this would
/// make the ambient half decorative, which is exactly the failure the whole
/// design is arranged against.
const _: () = assert!(
    SETTLEMENT_COMMERCE_MAX as u64 * SETTLEMENT_COMMERCE_PULL_TICKS < SETTLEMENT_GROWTH_DUE_MIN
);
```

- [ ] **Step 7: Run the tests and verify they pass**

```bash
cargo test -p feral-processes-engine settlements::growth
```

Expected: PASS, 9 tests.

If `due_ticks_spread_across_the_span_rather_than_clustering` fails, do **not** widen the tolerance. It means the fold is not reaching the high bits — re-read `derive::index` and `region_seed`'s doc comment in `placement.rs`, which records exactly this measured failure.

- [ ] **Step 8: Gate on the workspace**

```bash
cargo test --workspace
```

Expected: PASS. Nothing outside `growth.rs` has changed behaviour yet, so a failure here is a compile error or a genuine surprise — investigate rather than adjusting.

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/settlements/growth.rs crates/engine/src/settlements/mod.rs crates/engine/src/settlements/placement.rs crates/engine/src/tuning.rs
git commit -m "feat(settlements): the growth clock and the vitality banding

A due tick folded off placement's region seed with a fifth salt, and a
signed commerce scalar that bands Starved/Steady/Thriving. Zero bands
Steady, which is what lets an authored Mainframe open as its author
wrote it -- asserted at compile time, not in a test.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: `Relation`'s three new fields, and the save that carries them

**Files:**
- Modify: `crates/engine/src/settlements/relations.rs` (the `Relation` struct)
- Test: `crates/engine/src/tests/settlement_growth.rs` (create)
- Modify: `crates/engine/src/tests/mod.rs` (register it)

**Interfaces:**
- Consumes: `growth::clamp_commerce` (Task 2).
- Produces: `Relation { grown: bool, commerce: i32, commerce_epoch: u64 }`, all `#[serde(default)]`.

- [ ] **Step 1: Write the failing save test**

Create `crates/engine/src/tests/settlement_growth.rs`:

```rust
//! A Server growing into a Mainframe: the latch, the drift and the shelf.
//!
//! `docs/superpowers/specs/2026-09-06-settlement-growth-design.md`.

use crate::settlements::SettlementKey;

fn game(seed: u32) -> crate::Game {
    crate::Game::new(
        seed,
        crate::DifficultyMode::Forgiving,
        &crate::tests::support::test_assets_dir(),
    )
    .unwrap()
}

/// The first key `Game::new` materialized, which is a town that actually
/// exists on this seed's map rather than a coordinate we hoped held one.
fn a_known_key(game: &crate::Game) -> SettlementKey {
    *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .keys()
        .next()
        .expect("test premise: a new run materializes at least one settlement")
}

/// **Not a RON round-trip.** A `#[serde(default)]` field that no writer ever
/// sets round-trips perfectly while carrying nothing, so a round-trip test
/// would pass with the feature deleted. This drives a real save to disk and
/// loads it back.
#[test]
fn the_growth_fields_survive_a_save_and_load() {
    let dir = crate::tests::support::scratch_assets_dir("settlement_growth_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = game(4242);
    let key = a_known_key(&game);
    {
        let mut standings = game.world.resource_mut::<crate::resources::Standings>();
        let relation = standings.0.entry(key).or_default();
        relation.grown = true;
        relation.commerce = 37;
        relation.commerce_epoch = 5;
    }
    game.save(&path).unwrap();

    let loaded = crate::Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
    let relation = loaded
        .world
        .resource::<crate::resources::Standings>()
        .0
        .get(&key)
        .copied()
        .expect("the relation came back");
    assert!(relation.grown, "the latch did not survive the save");
    assert_eq!(relation.commerce, 37, "commerce did not survive the save");
    assert_eq!(relation.commerce_epoch, 5, "the epoch did not survive the save");
}
```

- [ ] **Step 2: Register the test module**

In `crates/engine/src/tests/mod.rs`, between `settlement_boards` and `settlement_market`:

```rust
mod settlement_boards;
mod settlement_growth;
mod settlement_market;
```

- [ ] **Step 3: Run it and watch it fail**

```bash
cargo test -p feral-processes-engine the_growth_fields_survive_a_save_and_load
```

Expected: FAIL to compile — `Relation` has no field `grown`.

- [ ] **Step 4: Add the three fields**

In `crates/engine/src/settlements/relations.rs`, at the end of `Relation`'s field list:

```rust
    /// Whether this town has grown into a Mainframe.
    ///
    /// **One-way, and that is the whole discipline.** Written in exactly one
    /// place — `Game::settlement_growth_tick` — and never cleared. Commerce
    /// decays; if the growth condition were re-evaluated on every read, a
    /// city would un-grow when its trade dried up, which the design
    /// explicitly refuses. Latching here is what makes every reader a plain
    /// `||` instead of a repeated inequality that could drift.
    ///
    /// An authored `SettlementKind::Mainframe` never sets this and never
    /// needs to: `Game::settlement_kind` reads the def first.
    #[serde(default)]
    pub grown: bool,
    /// How this town is doing — trade raises it, time lowers it, and
    /// `Standing::Hostile` lowers it faster.
    ///
    /// **Signed, and 0 means "as it was found."** An unsigned counter would
    /// band every authored Mainframe as Starved in a fresh world, before
    /// anyone had traded a Credit with it. Bounds are
    /// `growth::clamp_commerce`; the banding is `growth::vitality` and is
    /// derived on every read, never stored.
    #[serde(default)]
    pub commerce: i32,
    /// The last drift epoch folded into `commerce`.
    ///
    /// `static_epoch`'s shape: the decay is settled lazily against
    /// `current_tick() / SETTLEMENT_COMMERCE_DECAY_TICKS` rather than
    /// applied per tick, so a fast-forward cannot be outrun and no
    /// arithmetic runs over every town every tick.
    #[serde(default)]
    pub commerce_epoch: u64,
```

- [ ] **Step 5: Run it and verify it passes**

```bash
cargo test -p feral-processes-engine the_growth_fields_survive_a_save_and_load
```

Expected: PASS.

- [ ] **Step 6: Gate on the workspace**

```bash
cargo test --workspace
```

Expected: PASS. `Relation` is `Copy` and `Default`; the three additions keep both. If a test comparing whole `Standings` values fails, read it — it is telling you something real about a resource that is now written where it was not before, and Task 5 will make that louder.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/settlements/relations.rs crates/engine/src/tests/settlement_growth.rs crates/engine/src/tests/mod.rs
git commit -m "feat(settlements): Relation carries the growth latch and commerce

Three additive #[serde(default)] fields, so no SAVE_FORMAT_VERSION bump.
Covered by a real save-to-disk test rather than a RON round-trip, which
cannot catch a defaulted field no writer sets.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: `Game::settlement_kind` — the one door, and the four sites rewired

Nothing sets `grown` yet, so this task changes no behaviour. That is deliberate: it isolates the riskiest edit — the load path — into a task a reviewer can check without also reasoning about a clock.

**Files:**
- Create: `crates/engine/src/game/settlement_growth.rs`
- Modify: `crates/engine/src/game/mod.rs:42-44` (declare the module)
- Modify: `crates/engine/src/game/settlement_market.rs:113-114`, `:429-448`
- Modify: `crates/engine/src/game/spawning.rs:948`, `:1020-1043`, `:1000-1006`
- Modify: `crates/engine/src/game/lifecycle.rs:1402`, `:1428` — **the load-order fix**
- Modify: `crates/engine/src/game/inspection.rs:838`
- Test: `crates/engine/src/tests/settlement_growth.rs`

**Interfaces:**
- Consumes: `Relation.grown` (Task 3); `growth::vitality`, `Vitality::rows`, `Vitality::bonus_share` (Task 2).
- Produces:
  - `pub fn settlement_kind(&self, key: SettlementKey) -> Option<SettlementKind>`
  - `pub(crate) fn settlement_vitality(&self, key: SettlementKey) -> Option<Vitality>` — `None` for a key that is not a grown or authored Mainframe
  - `spawn_settlement_at(&mut self, key, tile)` — **the `kind` parameter is removed**

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/settlement_growth.rs`:

```rust
/// The load path is where this feature is most likely to break silently.
/// `restore_settlements` rebuilds every town's map entity from the record,
/// and if it draws from the authored `def.kind` a grown city reads `M` all
/// run and comes back from a save reading `s`. Nothing that never saves
/// would catch it.
#[test]
fn a_grown_town_still_draws_its_mainframe_glyph_after_a_load() {
    let dir = crate::tests::support::scratch_assets_dir("settlement_growth_glyph");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");

    let mut game = game(4242);
    // A town the catalogue authored as a Server, so the glyph under test is
    // the grown one and not one it always had.
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .grown = true;
    game.save(&path).unwrap();

    let mut loaded = crate::Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
    let mut query = loaded
        .world
        .query::<(&crate::components::Settlement, &crate::components::Glyph)>();
    let glyph = query
        .iter(&loaded.world)
        .find(|(s, _)| s.key == key)
        .map(|(_, g)| g.ch)
        .expect("the grown town has an entity to draw");
    assert_eq!(
        glyph, 'M',
        "a grown town came back from a save drawing its authored Server glyph"
    );
}

/// An authored Mainframe is a Mainframe with nobody having done anything,
/// and a fresh Server is a Server. The door's base case, which every other
/// assertion in this file rests on.
#[test]
fn the_door_answers_the_authored_kind_before_anything_grows() {
    let game = game(4242);
    for (key, known) in &game.world.resource::<crate::resources::Settlements>().0 {
        assert_eq!(
            game.settlement_kind(*key),
            Some(known.def.kind),
            "{key:?} does not read as the catalogue authored it"
        );
    }
}

/// A grown Server draws a Mainframe's shelf. Without this the latch is a
/// flag nothing consumes.
#[test]
fn a_grown_server_draws_more_shelf_rows_than_it_did() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    let before = game.settlement_shelf(key, 0).len();
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .grown = true;
    let after = game.settlement_shelf(key, 0).len();
    assert!(
        after > before,
        "a grown town's shelf did not deepen: {before} rows before, {after} after"
    );
}

/// A Server has no vitality band and must ignore commerce entirely. If it
/// read one, a town nobody trades with would quietly thin below the six
/// rows the shipped constant promises — a dwindle on the one settlement
/// that has nowhere to fall to.
#[test]
fn a_servers_shelf_ignores_commerce_at_every_band() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    let rows = |game: &mut crate::Game| game.settlement_shelf(key, 0).len();
    let baseline = rows(&mut game);
    for commerce in [
        crate::tuning::SETTLEMENT_COMMERCE_MIN,
        crate::tuning::SETTLEMENT_COMMERCE_STARVED,
        0,
        crate::tuning::SETTLEMENT_COMMERCE_THRIVING,
        crate::tuning::SETTLEMENT_COMMERCE_MAX,
    ] {
        game.world
            .resource_mut::<crate::resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .commerce = commerce;
        assert_eq!(
            rows(&mut game),
            baseline,
            "a Server's shelf moved at commerce {commerce}"
        );
    }
    assert_eq!(
        baseline as u32,
        crate::tuning::SETTLEMENT_SERVER_ROWS,
        "a Server draws something other than its own row count"
    );
}
```

Before running: open `crates/engine/src/game/settlement_market.rs` and confirm the shelf door's real name and signature (it is the function containing line 113's `settlement_rows(def.kind)`). If it is not `settlement_shelf(key, epoch)`, use the actual name and arity in the test above — do not add a wrapper to make the test compile.

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p feral-processes-engine settlement_growth
```

Expected: FAIL to compile — `Game::settlement_kind` does not exist. `a_grown_town_still_draws_its_mainframe_glyph_after_a_load` is the one that must still fail *after* the door exists but *before* the load-order fix in Step 6; that ordering is the point of this task.

- [ ] **Step 3: Create `game/settlement_growth.rs` with the two read doors**

```rust
//! Whether a town has grown, and how well it is doing.
//!
//! **One door for the effective kind.** `SettlementKind` is now two things
//! folded together — what the catalogue authored and what the run has done
//! — and four sites read it (the shelf's rows, its standout share, the map
//! glyph, the town page's label). Each of them asking
//! `known.def.kind` directly is how they drift apart, and the load path is
//! where that drift is invisible: `restore_settlements` would rebuild a
//! grown city's entity from the authored kind and redraw it as a town.
//! `settlement_kind` is the only reader of `Relation::grown` outside the
//! tick that writes it.

use crate::Game;
use crate::settlements::{SettlementKey, SettlementKind, Vitality, growth};

impl Game {
    /// What this settlement *is* right now — the catalogue's answer, or a
    /// `Mainframe` if the run has grown it there.
    ///
    /// `None` for a key with no materialized record, which is the same
    /// condition `town_garrisons` and `raiding_towns` exclude by
    /// construction: a town whose tile has never been resolved has no
    /// entity to draw and no shelf to stock.
    pub fn settlement_kind(&self, key: SettlementKey) -> Option<SettlementKind> {
        let authored = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)?
            .def
            .kind;
        if authored == SettlementKind::Mainframe {
            return Some(SettlementKind::Mainframe);
        }
        let grown = self
            .world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .is_some_and(|relation| relation.grown);
        Some(if grown {
            SettlementKind::Mainframe
        } else {
            SettlementKind::Server
        })
    }

    /// How well this city is doing, or `None` if it is not a city.
    ///
    /// A Server answers `None` rather than `Steady`: it has no band, and a
    /// band it never falls out of would put a word on its page that never
    /// changes. See `Vitality::rows`' note on why a Server never asks.
    pub(crate) fn settlement_vitality(&self, key: SettlementKey) -> Option<Vitality> {
        if self.settlement_kind(key)? != SettlementKind::Mainframe {
            return None;
        }
        let commerce = self
            .world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .map_or(0, |relation| relation.commerce);
        Some(growth::vitality(commerce))
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/engine/src/game/mod.rs`, beside the three existing settlement declarations (currently lines 42-44), in alphabetical order:

```rust
pub(crate) mod settlement_growth;
pub(crate) mod settlement_market;
pub(crate) mod settlement_patrol;
pub(crate) mod settlement_relations;
```

- [ ] **Step 5: Rewire the shelf**

In `crates/engine/src/game/settlement_market.rs`, replace lines 113-114:

```rust
        let rows = settlement_rows(self.settlement_kind(key), self.settlement_vitality(key));
        let bonus_share =
            settlement_bonus_share(self.settlement_kind(key), self.settlement_vitality(key));
```

and replace the two helper functions (currently lines 429-448):

```rust
/// How many shelf rows this settlement draws.
///
/// A Server draws `tuning::SETTLEMENT_SERVER_ROWS` flat — it has nowhere to
/// fall to, and its commerce is spoken for by the growth clock. A Mainframe,
/// authored or grown, draws its `Vitality`'s answer, which floors at the
/// Server's count: an `M` that drew fewer rows than an `s` would make the
/// label a lie.
///
/// Takes the *effective* kind from `Game::settlement_kind` and never
/// `def.kind` — see `game/settlement_growth.rs`' module doc.
fn settlement_rows(kind: Option<SettlementKind>, vitality: Option<Vitality>) -> u32 {
    match (kind, vitality) {
        (Some(SettlementKind::Mainframe), Some(vitality)) => vitality.rows(),
        _ => crate::tuning::SETTLEMENT_SERVER_ROWS,
    }
}

/// What share of this settlement's gear rows are standout stock —
/// `bonus_row_count`'s `share` argument. `settlement_rows`' shape and its
/// reason.
fn settlement_bonus_share(kind: Option<SettlementKind>, vitality: Option<Vitality>) -> u32 {
    match (kind, vitality) {
        (Some(SettlementKind::Mainframe), Some(vitality)) => vitality.bonus_share(),
        _ => crate::tuning::SETTLEMENT_SERVER_BONUS_SHARE,
    }
}
```

Add `Vitality` to that file's `use crate::settlements::{...}` line.

- [ ] **Step 6: Rewire both spawn sites and fix the load order**

**6a.** In `crates/engine/src/game/spawning.rs`, drop `spawn_settlement_at`'s `kind` parameter so a caller cannot pass the wrong one:

```rust
    /// Draws the map entity for a settlement whose record already exists.
    ///
    /// **Takes no kind.** It asks `Game::settlement_kind`, which is the only
    /// place the authored kind and the run's latch are folded together. The
    /// parameter used to be passed in by both callers, and the load path's
    /// caller passed the authored one — a grown city drew `M` all run and
    /// came back from a save drawing `s`. A door that cannot be handed the
    /// wrong answer is the fix; a second correct call site is not.
    ///
    /// The record must be in `Settlements` **before** this is called, and
    /// `Standings` must be too, or the latch reads as unset.
    fn spawn_settlement_at(&mut self, key: crate::settlements::SettlementKey, (x, y): (i32, i32)) {
        let ch = self
            .settlement_kind(key)
            .map_or(crate::settlements::SettlementKind::Server, |kind| kind)
            .glyph();
        // `GlyphColor::Yellow` was `palette::WARN` and, worse, the authored
        // colour of the Scrapper — a settlement and a scrapper nest were the
        // same hue on the same map. `Orange` is the one variant no species
        // authors: every hue a species declares is reachable on the surface
        // (a nest takes its guardian's colour), which makes "unclaimed"
        // mean unclaimed by a species rather than unclaimed outright, and
        // Orange's only other uses are base space's `BuildSite` glyph and
        // three base structures — a coordinate space that can never share a
        // tile with a town.
        self.world.spawn((
            crate::components::Settlement { key },
            Position { x, y },
            Glyph {
                ch,
                color: GlyphColor::Orange,
            },
        ));
    }
```

**6b.** At `spawning.rs:948`, the record is inserted immediately above the call, so only the argument changes:

```rust
                self.spawn_settlement_at(key, tile);
```

**6c.** In `restore_settlements` (around `spawning.rs:1000`), the resource is inserted *after* the loop today, so the door would read an empty `Settlements`. Invert it:

```rust
    pub(crate) fn restore_settlements(&mut self, known: crate::resources::Settlements) {
        // The record goes in **first**: `spawn_settlement_at` asks
        // `Game::settlement_kind`, which reads this resource and
        // `Standings`. Spawning first and inserting after would draw every
        // town at its authored kind, which is the exact bug this door
        // exists to close.
        let sites: Vec<(crate::settlements::SettlementKey, (i32, i32))> =
            known.0.iter().map(|(key, s)| (*key, s.tile)).collect();
        self.world.insert_resource(known);
        for (key, tile) in sites {
            self.spawn_settlement_at(key, tile);
        }
    }
```

**6d.** **The load-order fix.** In `crates/engine/src/game/lifecycle.rs`, `game.restore_settlements(data.settlements)` is at line 1402 and `game.world.insert_resource(data.standings)` is at line 1428 — so today the towns are drawn before their relationships exist. Move the standings line to immediately *above* the `restore_settlements` call, and leave a comment where it lands:

```rust
        // **Before `restore_settlements`, not after.** That call draws every
        // known town's glyph through `Game::settlement_kind`, which reads
        // `Relation::grown` out of this resource. Restored after, every
        // grown city would come back from the save drawing the `s` its
        // catalogue authored — correct all run, wrong on every load.
        game.world.insert_resource(data.standings);
        game.world.insert_resource(data.populated_chunks);
        game.restore_settlements(data.settlements);
```

Delete the now-duplicated `insert_resource(data.standings)` at its old position.

- [ ] **Step 7: Rewire the town page's label**

In `crates/engine/src/game/inspection.rs`, `settlement_report` currently reads `def.kind.label()` at line 838. The effective kind must be read **before** the `&self.world...def` borrow, beside the `standing` and `aid` lines that already do this for the same reason:

```rust
        let standing = self.standing_band(key).label();
        // Before the borrow below, with `standing` and for its reason: both
        // are doors of their own onto other resources.
        let kind = self
            .settlement_kind(key)
            .map_or(crate::settlements::SettlementKind::Server, |kind| kind)
            .label();
        let aid = self.settlement_aid_lines(key);
```

and in the struct literal replace `kind: def.kind.label(),` with `kind,`.

- [ ] **Step 8: Run the tests and verify they pass**

```bash
cargo test -p feral-processes-engine settlement_growth
```

Expected: PASS, all four tests including the save-and-load one from Task 3.

**Then prove the load-order fix is load-bearing.** Temporarily move `insert_resource(data.standings)` back below `restore_settlements` and re-run:

```bash
cargo test -p feral-processes-engine a_grown_town_still_draws_its_mainframe_glyph_after_a_load
```

Expected: **FAIL**, with the glyph coming back as `s`. If it still passes, the test is vacuous — it is not actually exercising the load path. Fix the test before restoring the line. Then restore the line and confirm PASS again.

- [ ] **Step 9: The one-door census**

The whole point of Task 4 is that no site reads `def.kind` any more. Prove it,
and record the count so a later reader knows what the number should be:

```bash
grep -rn 'def\.kind\|\.def\.kind' crates/ --include='*.rs'
```

Expected: **only** the reads inside `Game::settlement_kind`
(`game/settlement_growth.rs`) and the test-premise filters in
`tests/settlement_growth.rs` that deliberately select an *authored* Server.
Any other hit in `crates/engine/src/game/` or `crates/gui/` is a site that
will draw a grown city as a town — fix it before committing rather than
noting it.

- [ ] **Step 10: Gate on the workspace**

```bash
cargo test --workspace
```

Expected: PASS. Behaviour is unchanged for every existing world — nothing sets `grown` yet, and an authored kind still answers as itself.

- [ ] **Step 11: Commit**

```bash
git add crates/engine/src/game/settlement_growth.rs crates/engine/src/game/mod.rs crates/engine/src/game/settlement_market.rs crates/engine/src/game/spawning.rs crates/engine/src/game/lifecycle.rs crates/engine/src/game/inspection.rs crates/engine/src/tests/settlement_growth.rs
git commit -m "refactor(settlements): one door for a settlement's effective kind

Game::settlement_kind folds the authored kind and the run's latch, and
spawn_settlement_at no longer takes a kind at all -- a door that cannot
be handed the wrong answer, rather than two call sites that agree today.

Also fixes the load order: standings are restored before settlements,
since drawing a town's glyph now reads its relation.

No behaviour change; nothing sets the latch yet.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: `Game::settlement_growth_tick` — the drift and the flip

**Files:**
- Modify: `crates/engine/src/game/settlement_growth.rs`
- Modify: `crates/engine/src/game/turn.rs:173-179`
- Modify: `crates/engine/src/game/spawning.rs` (`ensure_local_settlements`, the silent evaluation)
- Test: `crates/engine/src/tests/settlement_growth.rs`

**Interfaces:**
- Consumes: `growth::due_tick`, `growth::pull_ticks`, `growth::clamp_commerce` (Task 2); `Game::current_tick`, `Game::standing_band`.
- Produces:
  - `pub(crate) fn settlement_growth_tick(&mut self)`
  - `pub(crate) fn settle_commerce_drift(&mut self, key: SettlementKey)`
  - `pub(crate) fn latch_growth(&mut self, key: SettlementKey) -> bool` — `true` if this call flipped it
  - `pub(crate) fn adjust_commerce(&mut self, key: SettlementKey, delta: i32)`

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/settlement_growth.rs`:

```rust
use crate::settlements::growth;

/// The whole feature in one assertion: a town past its date is a city.
#[test]
fn a_server_past_its_due_tick_grows() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Server),
        "test premise: it has not grown already"
    );
    // Enough commerce to pull the date to now, rather than ticking the
    // clock for three thousand turns.
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = crate::tuning::SETTLEMENT_COMMERCE_MAX;
    let due = growth::due_tick(game.world_seed_for_test(), key);
    game.set_tick_for_test(due);
    game.settlement_growth_tick();
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Mainframe)
    );
}

/// The latch is one-way. Commerce decays; if the inequality were
/// re-evaluated on every read, a city would un-grow the moment its trade
/// dried up -- which the design explicitly refuses.
#[test]
fn a_grown_city_never_falls_back_to_a_server() {
    let mut game = game(4242);
    let key = a_known_key(&game);
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .grown = true;
    {
        let mut standings = game.world.resource_mut::<crate::resources::Standings>();
        standings.0.get_mut(&key).unwrap().commerce = crate::tuning::SETTLEMENT_COMMERCE_MIN;
    }
    for _ in 0..40 {
        game.settlement_growth_tick();
    }
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Mainframe),
        "a starved city fell back to a town"
    );
}

/// Trade is what makes the difference. This is the test that must fail with
/// the pull removed -- run it that way before believing it.
#[test]
fn trade_pulls_a_growth_date_forward() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    let due = growth::due_tick(game.world_seed_for_test(), key);
    // One tick short of the date, which is where the pull has to do the
    // work or nothing does.
    game.set_tick_for_test(due - 1);
    game.settlement_growth_tick();
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Server),
        "it grew a tick early with no commerce at all"
    );
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = 1;
    game.settlement_growth_tick();
    assert_eq!(
        game.settlement_kind(key),
        Some(crate::settlements::SettlementKind::Mainframe),
        "a point of commerce bought nothing"
    );
}

/// Hostility triples the drift. Same elapsed epochs, two bands, two
/// answers.
#[test]
fn a_hostile_town_starves_faster_than_a_neglected_one() {
    let mut game = game(4242);
    let keys: Vec<_> = game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .keys()
        .copied()
        .take(2)
        .collect();
    assert_eq!(keys.len(), 2, "test premise: two towns materialized");
    let (neglected, hated) = (keys[0], keys[1]);
    game.adjust_standing(hated, crate::tuning::SETTLEMENT_MIN_STANDING);
    assert_eq!(
        game.standing_band(hated),
        crate::settlements::Standing::Hostile,
        "test premise: it is actually Hostile"
    );
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 10);
    game.settlement_growth_tick();
    let commerce = |game: &crate::Game, key| {
        game.world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .map_or(0, |r: &crate::settlements::Relation| r.commerce)
    };
    assert!(
        commerce(&game, hated) < commerce(&game, neglected),
        "Hostile drifted no faster: {} vs {}",
        commerce(&game, hated),
        commerce(&game, neglected)
    );
}

/// Repairing standing stops the acceleration the tick it lands -- the band
/// is read live, never as a history.
#[test]
fn repairing_standing_restores_the_slower_drift() {
    let mut game = game(4242);
    let key = a_known_key(&game);
    game.adjust_standing(key, crate::tuning::SETTLEMENT_MIN_STANDING);
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 5);
    game.settlement_growth_tick();
    let after_hostile = game
        .world
        .resource::<crate::resources::Standings>()
        .0
        .get(&key)
        .unwrap()
        .commerce;

    game.adjust_standing(key, crate::tuning::SETTLEMENT_MAX_STANDING * 2);
    assert_ne!(game.standing_band(key), crate::settlements::Standing::Hostile);
    game.set_tick_for_test(crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS * 10);
    game.settlement_growth_tick();
    let after_repair = game
        .world
        .resource::<crate::resources::Standings>()
        .0
        .get(&key)
        .unwrap()
        .commerce;

    let hostile_rate = -after_hostile / 5;
    let repaired_rate = -(after_repair - after_hostile) / 5;
    assert!(
        repaired_rate < hostile_rate,
        "the repaired town kept the Hostile rate: {repaired_rate} vs {hostile_rate}"
    );
}

/// Discovery is not an event. A town materialized already past its date was
/// simply always a city -- the clock has been running whether or not anyone
/// was watching. Announcing here would name a place the party has never
/// seen.
#[test]
fn a_town_found_past_its_date_arrives_grown_and_silent() {
    let mut game = game(4242);
    let before = game.message_history().len();
    game.ensure_local_settlements();
    assert_eq!(
        game.message_history().len(),
        before,
        "materializing a settlement wrote a line"
    );
}
```

`world_seed_for_test`, `set_tick_for_test` and `message_history` may not exist under those names. Before writing implementation, find the real ones:

```bash
grep -rn 'fn seed(' crates/engine/src/world.rs
grep -rn 'fn message_history\|fn set_tick\|_for_test' crates/engine/src/game/turn.rs crates/engine/src/game/mod.rs
```

Use whatever the engine already exposes. If there is genuinely no way to set the clock from a test, add **one** `#[cfg(test)]` helper on `Game` in `game/turn.rs` beside `current_tick`, and no more:

```rust
    /// Sets the clock, for tests that would otherwise have to tick a
    /// thousand turns to reach a scheduled event.
    #[cfg(test)]
    pub(crate) fn set_tick_for_test(&mut self, tick: u64) {
        self.world.resource_mut::<crate::resources::GameClock>().tick = tick;
    }
```

Read `GameClock` at `crates/engine/src/resources.rs:19` for its real field name first.

- [ ] **Step 2: Run them and watch them fail**

```bash
cargo test -p feral-processes-engine settlement_growth
```

Expected: FAIL to compile — `settlement_growth_tick` does not exist.

- [ ] **Step 3: Write the tick**

Append to `crates/engine/src/game/settlement_growth.rs`:

```rust
impl Game {
    /// Settles every known town's commerce drift, then latches any Server
    /// past its date.
    ///
    /// **Materialized towns only** — the walk is over `Settlements`, which
    /// is `town_garrisons`' and `raiding_towns`' rule and the same reason: a
    /// town whose tile has never been resolved has no entity to repaint and
    /// no name a log line could use. A region nobody has walked into is
    /// evaluated when they do, silently, by `ensure_local_settlements`.
    pub(crate) fn settlement_growth_tick(&mut self) {
        let keys: Vec<SettlementKey> = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .keys()
            .copied()
            .collect();
        for key in keys {
            self.settle_commerce_drift(key);
            self.latch_growth(key);
        }
    }

    /// Folds every drift epoch since the last one into `commerce`.
    ///
    /// Lazy against an epoch rather than applied per tick, `static_epoch`'s
    /// shape: a fast-forward cannot be outrun, and no arithmetic runs over
    /// every town on a tick where nothing has changed. The Hostile
    /// surcharge reads the **current** band, never a history — repairing
    /// standing stops the acceleration the tick it lands.
    pub(crate) fn settle_commerce_drift(&mut self, key: SettlementKey) {
        let epoch = self.current_tick() / crate::tuning::SETTLEMENT_COMMERCE_DECAY_TICKS;
        let hostile = self.standing_band(key) == crate::settlements::Standing::Hostile;
        let mut standings = self.world.resource_mut::<crate::resources::Standings>();
        let relation = standings.0.entry(key).or_default();
        if epoch <= relation.commerce_epoch {
            return;
        }
        let elapsed = (epoch - relation.commerce_epoch).min(i32::MAX as u64) as i32;
        let rate = crate::tuning::SETTLEMENT_COMMERCE_DECAY
            + if hostile {
                crate::tuning::SETTLEMENT_COMMERCE_HOSTILE_DECAY
            } else {
                0
            };
        relation.commerce =
            growth::clamp_commerce(relation.commerce.saturating_sub(rate.saturating_mul(elapsed)));
        relation.commerce_epoch = epoch;
    }

    /// Throws the latch if this Server is past its date. `true` if **this
    /// call** flipped it, which is what lets a caller announce a change
    /// without announcing a discovery.
    ///
    /// The inequality lives here and nowhere else. Every reader asks
    /// `settlement_kind`, which is a `||` over a stored bool — so a
    /// decaying commerce can never un-grow a city, and a later change to
    /// the formula cannot leave two sites disagreeing about what a town is.
    pub(crate) fn latch_growth(&mut self, key: SettlementKey) -> bool {
        if self.settlement_kind(key) != Some(SettlementKind::Server) {
            return false;
        }
        let seed = self.world.resource::<crate::world::WorldMap>().seed();
        let commerce = self
            .world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .map_or(0, |relation| relation.commerce);
        let due = growth::due_tick(seed, key) as i64 - growth::pull_ticks(commerce);
        if (self.current_tick() as i64) < due {
            return false;
        }
        self.world
            .resource_mut::<crate::resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .grown = true;
        true
    }

    /// The one door commerce is written through — `adjust_standing`'s shape
    /// and its reason: one clamp is enough only because there is one writer.
    pub(crate) fn adjust_commerce(&mut self, key: SettlementKey, delta: i32) {
        if delta == 0 {
            return;
        }
        let mut standings = self.world.resource_mut::<crate::resources::Standings>();
        let relation = standings.0.entry(key).or_default();
        relation.commerce = growth::clamp_commerce(relation.commerce.saturating_add(delta));
    }
}
```

Add `use crate::settlements::Standing;` if the drift's band comparison needs it, and confirm `WorldMap::seed()` is the accessor `settlement_market.rs:112` already uses.

- [ ] **Step 4: Call it from the turn**

In `crates/engine/src/game/turn.rs`, immediately after `self.maybe_field_patrol();` (currently line 179):

```rust
        self.maybe_field_patrol();
        // After `ensure_local_settlements`, for that call's own reason: this
        // walks the towns that pass resolves, and a region materialized this
        // tick must be evaluated silently by the resolver rather than
        // announced by this. Beside the patrol roll because both are
        // settlement work keyed to the map rather than to the base.
        self.settlement_growth_tick();
```

- [ ] **Step 5: Make materialization silent**

In `ensure_local_settlements` (`crates/engine/src/game/spawning.rs`), the record is inserted and then `spawn_settlement_at(key, tile)` is called. Insert the silent evaluation **between** them, so the glyph is drawn already correct and no line is written:

```rust
                self.world
                    .resource_mut::<crate::resources::Settlements>()
                    .0
                    .insert(
                        key,
                        crate::resources::KnownSettlement {
                            tile,
                            def: def.clone(),
                            visited: false,
                        },
                    );
                // **Discovery is not an event.** Evaluated here, before the
                // glyph is drawn, so a town found past its date was simply
                // always a city — the clock has been running whether or not
                // anyone was watching. Announcing a flip here would name a
                // place the party has never seen, and would fire on the
                // first tick after walking into any region.
                self.latch_growth(key);
                self.spawn_settlement_at(key, tile);
```

- [ ] **Step 6: Run the tests and verify they pass**

```bash
cargo test -p feral-processes-engine settlement_growth
```

Expected: PASS, all ten tests.

- [ ] **Step 7: Prove `trade_pulls_a_growth_date_forward` is not vacuous**

Temporarily change `latch_growth`'s `due` line to ignore commerce:

```rust
        let due = growth::due_tick(seed, key) as i64;
```

Re-run:

```bash
cargo test -p feral-processes-engine trade_pulls_a_growth_date_forward
```

Expected: **FAIL**. If it passes, the test proves nothing — the pull is not what makes it flip. Fix the test, then restore the line and confirm PASS.

- [ ] **Step 8: Gate on the workspace**

```bash
cargo test --workspace
```

Expected: PASS. Two things may legitimately move here and both need reading rather than silencing:

1. **`Standings` now holds a record for every materialized town**, not only every town dealt with, because `settle_commerce_drift` uses `entry().or_default()`. A test asserting `Standings` is empty, or comparing whole `Standings` values across a save, will see this. `Relation::default()` is standing 0, which bands `Neutral`, so no consequence changes — but confirm that in the failing test rather than assuming it.
2. **A save's bytes change**, since those records serialize. `dev-saves/settlements.ron` sits at tick 5422, past `SETTLEMENT_GROWTH_DUE_MIN`, so some of its towns will now load grown. Run the launcher's template tests specifically and read what they say:

```bash
cargo test -p feral-processes-launcher
```

If `the_settlements_template_opens_at_an_allied_town` fails, it is asserting on a band or an aid line, not a kind — read the failure before touching the template.

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/game/settlement_growth.rs crates/engine/src/game/turn.rs crates/engine/src/game/spawning.rs crates/engine/src/tests/settlement_growth.rs
git commit -m "feat(settlements): the growth tick, the drift and the one-way latch

The inequality lives in latch_growth and nowhere else, so a decaying
commerce can never un-grow a city. Materialization evaluates it
silently: a town found past its date was always a city.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Trade feeds commerce

**Files:**
- Modify: `crates/engine/src/game/settlement_relations.rs:67-79` (`credit_trade_volume`)
- Test: `crates/engine/src/tests/settlement_growth.rs`

**Interfaces:**
- Consumes: `Game::adjust_commerce` (Task 5); `SETTLEMENT_COMMERCE_CREDITS_PER_POINT` (Task 2).
- Produces: nothing new. This wires an existing door to a new consequence.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/settlement_growth.rs`:

```rust
/// Trade is the accelerant, and `credit_trade_volume` is the one door every
/// counter sale and route delivery already goes through. Wiring the shelf
/// and leaving this unwired would ship a pull nothing can reach.
#[test]
fn trading_with_a_town_raises_its_commerce() {
    let mut game = game(4242);
    let key = a_known_key(&game);
    let before = game
        .world
        .resource::<crate::resources::Standings>()
        .0
        .get(&key)
        .map_or(0, |r: &crate::settlements::Relation| r.commerce);
    game.credit_trade_volume(key, crate::tuning::SETTLEMENT_COMMERCE_CREDITS_PER_POINT * 3);
    let after = game
        .world
        .resource::<crate::resources::Standings>()
        .0
        .get(&key)
        .unwrap()
        .commerce;
    assert_eq!(after - before, 3, "three points' worth of trade bought {}", after - before);
}

/// A basket under the threshold buys nothing yet, and the remainder is not
/// lost -- `Relation::credit_trade`'s rule, restated on the commerce axis.
/// Without it, ten small baskets earn nothing while one large basket of the
/// same volume earns the lot.
#[test]
fn small_baskets_and_one_large_basket_buy_the_same_commerce() {
    let per = crate::tuning::SETTLEMENT_COMMERCE_CREDITS_PER_POINT;
    let mut split = game(4242);
    let mut whole = game(4242);
    let key = a_known_key(&split);
    for _ in 0..10 {
        split.credit_trade_volume(key, per / 10);
    }
    whole.credit_trade_volume(key, per);
    let commerce = |game: &crate::Game| {
        game.world
            .resource::<crate::resources::Standings>()
            .0
            .get(&key)
            .map_or(0, |r: &crate::settlements::Relation| r.commerce)
    };
    assert_eq!(commerce(&split), commerce(&whole));
    assert_eq!(commerce(&whole), 1);
}
```

- [ ] **Step 2: Run and watch fail**

```bash
cargo test -p feral-processes-engine trading_with_a_town_raises_its_commerce small_baskets
```

Expected: FAIL — commerce does not move.

- [ ] **Step 3: Add the remainder field and the fold**

`credit_trade`'s remainder problem applies identically here, so commerce needs its own remainder. In `crates/engine/src/settlements/relations.rs`, add a fourth field beside the three from Task 3:

```rust
    /// The commerce remainder, `trade_credits`' companion and its reason:
    /// without somewhere to keep what is left over, a player who trades in
    /// ten small baskets feeds a town nothing while one who trades the same
    /// volume in a single basket feeds it the lot. A rounding rule, not a
    /// volume rule.
    #[serde(default)]
    pub commerce_credits: u32,
```

and a method beside `credit_trade`:

```rust
    /// Folds `credits` of trade in and answers how many commerce points it
    /// bought, keeping the remainder for the next basket. `credit_trade`'s
    /// shape on the second axis, and a separate remainder because the two
    /// thresholds differ — see `SETTLEMENT_COMMERCE_CREDITS_PER_POINT`.
    pub(crate) fn credit_commerce(&mut self, credits: u32) -> i32 {
        self.commerce_credits += credits;
        let points = self.commerce_credits / crate::tuning::SETTLEMENT_COMMERCE_CREDITS_PER_POINT;
        self.commerce_credits -= points * crate::tuning::SETTLEMENT_COMMERCE_CREDITS_PER_POINT;
        points as i32
    }
```

Add `SETTLEMENT_COMMERCE_CREDITS_PER_POINT` to that file's `use crate::tuning::{...}` list.

- [ ] **Step 4: Wire it into the existing door**

In `crates/engine/src/game/settlement_relations.rs`, `credit_trade_volume`:

```rust
    pub(crate) fn credit_trade_volume(&mut self, key: SettlementKey, credits: u32) {
        if credits == 0 {
            return;
        }
        // Two readings of one volume, both through this door: goodwill and
        // prosperity. They are separate axes on purpose — commerce decays
        // and standing does not, and commerce is bought cheaper
        // (`SETTLEMENT_COMMERCE_CREDITS_PER_POINT` against
        // `SETTLEMENT_TRADE_CREDITS_PER_POINT`), because a town notices your
        // money before it likes you.
        let (points, commerce) = {
            let mut standings = self.world.resource_mut::<resources::Standings>();
            let relation = standings.0.entry(key).or_default();
            (relation.credit_trade(credits), relation.credit_commerce(credits))
        };
        self.adjust_standing(key, points);
        self.adjust_commerce(key, commerce);
    }
```

- [ ] **Step 5: Run and verify**

```bash
cargo test -p feral-processes-engine trading_with_a_town_raises_its_commerce small_baskets
```

Expected: PASS.

- [ ] **Step 6: Extend the save test**

In `the_growth_fields_survive_a_save_and_load` (Task 3), set and assert `commerce_credits` too — a remainder that did not survive a save would silently reset a player's progress toward the next point on every load:

```rust
        relation.commerce_credits = 61;
```

```rust
    assert_eq!(relation.commerce_credits, 61, "the remainder did not survive the save");
```

- [ ] **Step 7: Gate on the workspace**

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/engine/src/settlements/relations.rs crates/engine/src/game/settlement_relations.rs crates/engine/src/tests/settlement_growth.rs
git commit -m "feat(settlements): trade volume feeds a town's commerce

One volume, two readings, both through credit_trade_volume. Commerce
has its own remainder for credit_trade's stated reason: ten small
baskets must buy exactly what one large basket does.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: The moment itself — the glyph repaint, the line and the notification

Until now growth is invisible: the shelf deepens and nothing tells the player why. This is the task that makes it an event.

**Files:**
- Modify: `crates/engine/src/game/settlement_growth.rs`
- Modify: `crates/engine/src/notifications.rs` (the variant, `all()`, `def()`)
- Modify: `crates/engine/src/tests/notifications.rs` (the census site)
- Test: `crates/engine/src/tests/settlement_growth.rs`

**Interfaces:**
- Consumes: `Game::notify`, `Game::log_kind`, `Game::settlement_name`, `latch_growth`'s `bool` (Task 5).
- Produces: `NotificationKind::SettlementGrown`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/settlement_growth.rs`:

```rust
/// The glyph is baked into the entity at materialization, so a flip that
/// does not repaint it leaves the map saying `s` about a city until the
/// next load. Nothing else in this feature would notice.
#[test]
fn growing_repaints_the_map_glyph_in_place() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = crate::tuning::SETTLEMENT_COMMERCE_MAX;
    game.set_tick_for_test(growth::due_tick(game.world_seed_for_test(), key));
    game.settlement_growth_tick();

    let mut query = game
        .world
        .query::<(&crate::components::Settlement, &crate::components::Glyph)>();
    let glyph = query
        .iter(&game.world)
        .find(|(s, _)| s.key == key)
        .map(|(_, g)| g.ch)
        .expect("the town has an entity");
    assert_eq!(glyph, 'M', "the map still draws the town it used to be");
}

/// A change the player can read. `message_history` condenses repeats, so
/// this asserts the line is present rather than counting entries.
#[test]
fn growing_writes_a_line_naming_the_town() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    let name = game.settlement_name(key);
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = crate::tuning::SETTLEMENT_COMMERCE_MAX;
    game.set_tick_for_test(growth::due_tick(game.world_seed_for_test(), key));
    game.settlement_growth_tick();
    assert!(
        game.message_history().iter().any(|entry| entry.text.contains(&name)),
        "no line named {name}"
    );
}

/// It fires once. `settlement_growth_tick` runs every tick and the latch is
/// already set on the second one -- a missing "did this call flip it" check
/// would write the line every tick for the rest of the run.
#[test]
fn growing_announces_once_and_not_every_tick_after() {
    let mut game = game(4242);
    let key = *game
        .world
        .resource::<crate::resources::Settlements>()
        .0
        .iter()
        .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
        .map(|(key, _)| key)
        .expect("test premise: this seed materializes at least one authored Server");
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .commerce = crate::tuning::SETTLEMENT_COMMERCE_MAX;
    game.set_tick_for_test(growth::due_tick(game.world_seed_for_test(), key));
    game.settlement_growth_tick();
    let after_first = game.message_history().len();
    for _ in 0..20 {
        game.settlement_growth_tick();
    }
    assert_eq!(
        game.message_history().len(),
        after_first,
        "the growth line is still being written"
    );
}

/// The notification is gated on the party having actually stood there. A
/// notification takes the screen, and a city on the far side of the map
/// that the player has never reached interrupting them is the failure this
/// gate exists to prevent — and it is invisible without a test, because the
/// log line fires either way.
#[test]
fn only_a_visited_towns_growth_takes_the_screen() {
    let grow = |visited: bool| {
        let mut game = game(4242);
        let key = *game
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .iter()
            .find(|(_, known)| known.def.kind == crate::settlements::SettlementKind::Server)
            .map(|(key, _)| key)
            .expect("test premise: this seed materializes at least one authored Server");
        game.world
            .resource_mut::<crate::resources::Settlements>()
            .0
            .get_mut(&key)
            .unwrap()
            .visited = visited;
        game.world
            .resource_mut::<crate::resources::Standings>()
            .0
            .entry(key)
            .or_default()
            .commerce = crate::tuning::SETTLEMENT_COMMERCE_MAX;
        game.set_tick_for_test(growth::due_tick(game.world_seed_for_test(), key));
        game.settlement_growth_tick();
        game.world
            .resource_mut::<crate::resources::Notifications>()
            .pop()
            .is_some()
    };
    assert!(grow(true), "a visited town's growth never reached the screen");
    assert!(
        !grow(false),
        "a town the party has never stood in interrupted them"
    );
}
```

`Notifications::pop` is the drain the screen uses — confirm its visibility from
a test in `crates/engine/src/resources.rs:843` and use whatever the frontend
already calls if `pop` is not reachable.

Confirm `message_history`'s real accessor and its entry field name (`text`) before running — `crates/engine/src/resources.rs` around the `MessageLog` type. Use the real names.

- [ ] **Step 2: Run and watch fail**

```bash
cargo test -p feral-processes-engine growing_
```

Expected: FAIL — the glyph is still `s` and no line is written.

- [ ] **Step 3: Add the notification kind**

In `crates/engine/src/notifications.rs`, add the variant to `NotificationKind` in the milestones group (not the tutorials group), with a doc comment:

```rust
    /// A town the party knew as a Server has become a Mainframe —
    /// `Game::announce_growth`.
    ///
    /// A milestone rather than a tutorial: it teaches nothing, and it can
    /// happen more than once in a run because a world holds more than one
    /// town. `Repeat::Always`.
    SettlementGrown,
```

Bump `all()` from 12 to 13 and add the variant. **This array's length is hand-written** — a new variant that compiles fine while missing from here is invisible to every census that walks it:

```rust
    pub fn all() -> [NotificationKind; 13] {
        [
            // … the twelve existing …
            NotificationKind::SettlementGrown,
        ]
    }
```

Add its `def()` arm:

```rust
            NotificationKind::SettlementGrown => NotificationDef {
                title: "It Grew",
                body: "A place you knew as a stop has become a destination. Its shelves run \
                       deeper now, and what it keeps on them is worth the walk in a way it was \
                       not before.\n\nTowns grow on their own, in their own time. They grow \
                       sooner where somebody has been spending.",
                sprite: None,
                glyph: 'M',
                color: GlyphColor::Orange,
                repeat: Repeat::Always,
            },
```

`GlyphColor::Orange` is a call onto the same hue the map draws a settlement in — see `spawn_settlement_at`'s comment. Do not hand-pick a different one.

- [ ] **Step 4: Add the census site**

In `crates/engine/src/tests/notifications.rs`, in `every_notification_kind_is_fired_by_a_named_site`'s `site` match:

```rust
            NotificationKind::SettlementGrown => "Game::announce_growth",
```

There is a second exhaustive match in that file asserting tutorials are `OnceEver` and milestones are `Always`. Find it and add the arm on the `Always` side.

- [ ] **Step 5: Write the announcement**

In `crates/engine/src/game/settlement_growth.rs`, change `settlement_growth_tick`'s loop and add the announcer:

```rust
        for key in keys {
            self.settle_commerce_drift(key);
            if self.latch_growth(key) {
                self.announce_growth(key);
            }
        }
```

```rust
    /// Repaints the map and tells the player, exactly once.
    ///
    /// Called only on `latch_growth`'s `true` — the flip, not the state.
    /// `settlement_growth_tick` runs every tick and the latch is already set
    /// on the second one, so announcing on the state would write this line
    /// for the rest of the run.
    ///
    /// **The repaint is not optional.** The glyph is baked into the entity
    /// at materialization, so without this the map keeps drawing the town
    /// the city used to be until the next load rebuilds it.
    fn announce_growth(&mut self, key: SettlementKey) {
        let glyph = SettlementKind::Mainframe.glyph();
        let entity = {
            let mut query = self
                .world
                .query::<(bevy_ecs::entity::Entity, &crate::components::Settlement)>();
            query
                .iter(&self.world)
                .find(|(_, settlement)| settlement.key == key)
                .map(|(entity, _)| entity)
        };
        if let Some(entity) = entity
            && let Some(mut drawn) = self.world.get_mut::<crate::components::Glyph>(entity)
        {
            drawn.ch = glyph;
        }
        let name = self.settlement_name(key);
        self.log_kind(
            crate::resources::MessageKind::Info,
            format!("{name} has grown. It is a Mainframe now."),
        );
        // Only for a town the party has actually stood in. A notification
        // takes the screen, and a place they have never reached has not
        // earned that — `KnownSettlement::visited` is the same flag the
        // compass uses to decide whether a town has a name worth showing.
        let visited = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .get(&key)
            .is_some_and(|known| known.visited);
        if visited {
            self.notify(crate::notifications::NotificationKind::SettlementGrown);
        }
    }
```

Match the file's existing import style rather than fully-qualifying `bevy_ecs::entity::Entity` if `Entity` is already in scope there. If the `let ... && let ...` chain does not compile on this edition, use a nested `if let`.

- [ ] **Step 6: Run and verify**

```bash
cargo test -p feral-processes-engine growing_ only_a_visited_towns_growth
```

Expected: PASS, four tests.

- [ ] **Step 7: Gate on the workspace**

```bash
cargo test --workspace
```

Expected: PASS. The notification screen has a **height census** with no scroll to forgive an overlong body — if a test in `crates/gui` fails measuring the tallest notification, shorten `SettlementGrown`'s body rather than raising the bound.

- [ ] **Step 8: Commit**

```bash
git add crates/engine/src/game/settlement_growth.rs crates/engine/src/notifications.rs crates/engine/src/tests/notifications.rs crates/engine/src/tests/settlement_growth.rs
git commit -m "feat(settlements): a town growing is an event you can see

Repaints the baked map glyph, writes a line naming the town, and
notifies for a town the party has actually visited. Fires on the flip,
never on the state.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: The town page says how a city is doing

**Files:**
- Modify: `crates/engine/src/views.rs:1922-1935` (`SettlementView`)
- Modify: `crates/engine/src/game/inspection.rs` (`settlement_report`)
- Modify: `crates/gui/src/render/settlement.rs:42-60` (the row), `:130-200` (both censuses)
- Test: `crates/gui/src/render/settlement.rs`'s own test module

**Interfaces:**
- Consumes: `Game::settlement_vitality` (Task 4).
- Produces: `SettlementView.vitality: Option<&'static str>`.

- [ ] **Step 1: Write the failing test**

In `crates/gui/src/render/settlement.rs`'s test module, beside `the_header_wears_the_map_glyphs_own_orange`:

```rust
    /// A city that has been starved has to say so, or the only signal is a
    /// shelf that used to be longer — which the player cannot compare
    /// against anything.
    #[test]
    fn a_citys_page_names_its_vitality_and_a_towns_does_not() {
        let city = SettlementView {
            name: "Tally Yard".to_string(),
            kind: "Mainframe",
            specialty: "Materials",
            temperament: "Mercantile",
            blurb: "Everything that passes through is counted twice.".to_string(),
            standing: "Neutral",
            vitality: Some("Starved"),
            aid: Vec::new(),
        };
        let text = |rows: Vec<Row>| {
            rows.iter()
                .filter_map(|r| match r {
                    Row::Text(t) | Row::TextColored(t, _) => Some(t.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert!(
            text(settlement_page_rows(&city)).contains("Starved"),
            "a starved city's page never says so"
        );

        let town = SettlementView {
            kind: "Server",
            vitality: None,
            ..city.clone()
        };
        let drawn = text(settlement_page_rows(&town));
        for band in ["Starved", "Steady", "Thriving"] {
            assert!(
                !drawn.contains(band),
                "a town's page reports a vitality band it cannot have"
            );
        }
    }
```

`SettlementView` needs `Clone` for the `..city.clone()` spread — check whether it already derives it, and add `Clone` to its derive list if not.

- [ ] **Step 2: Run and watch fail**

```bash
cargo test -p feral-processes-gui a_citys_page_names_its_vitality
```

Expected: FAIL to compile — `SettlementView` has no field `vitality`.

- [ ] **Step 3: Add the field**

In `crates/engine/src/views.rs`, after `standing`:

```rust
    /// How this city is doing — `growth::Vitality::label()`, on
    /// `kind`/`specialty`/`temperament`'s precedent of calling onto the
    /// band's own enum rather than wording it here.
    ///
    /// **`None` for a Server**, which has no band: a word that never
    /// changes is worse than no word, and the row is dropped entirely
    /// rather than drawn saying "Steady" forever. See
    /// `Game::settlement_vitality`.
    pub vitality: Option<&'static str>,
```

- [ ] **Step 4: Fill it in `settlement_report`**

In `crates/engine/src/game/inspection.rs`, beside the `kind` line added in Task 4 and before the `def` borrow:

```rust
        let vitality = self.settlement_vitality(key).map(|v| v.label());
```

and add `vitality,` to the struct literal.

- [ ] **Step 5: Draw the row**

In `crates/gui/src/render/settlement.rs`, `settlement_page_rows`, after the standing row:

```rust
        text_row(format!("They regard you as {}.", view.standing)),
    ];
    // Its own row, and only for a city. A Server has no band, and the row
    // is dropped rather than drawn with a word that could never change —
    // `SettlementView::vitality`'s own reason.
    if let Some(vitality) = view.vitality {
        rows.push(text_row(format!("The place is {vitality}.")));
    }
    rows.push(text_row(""));
```

Restructure the `vec![...]` so the blank spacer row moves out of the literal and after the conditional, as shown.

- [ ] **Step 6: Update both censuses**

The page has **no scroll**, so it is measured at its worst case on both axes — and the worst case just grew by a row.

**6a.** In `the_header_wears_the_map_glyphs_own_orange`, add `vitality: None,` to its `SettlementView` literal.

**6b.** In `tallest_settlement_page`, add the longest band label so the height census measures the real worst case:

```rust
            standing: "Neutral",
            // The longest band label a city can carry — the census measures
            // the worst case, and a page a row short of the tallest one is
            // a census that passes while the real page overflows.
            vitality: Some("Thriving"),
```

and add `def.kind.label().chars().count()`'s neighbour to the `max_by_key` width fold so the widest row accounts for it:

```rust
                    + "Thriving".chars().count()
```

**6c.** Grep for every other construction of `SettlementView` — the field is required and each one must be updated:

```bash
grep -rn 'SettlementView {' crates/
```

- [ ] **Step 7: Run and verify**

```bash
cargo test -p feral-processes-gui settlement
```

Expected: PASS.

- [ ] **Step 8: Gate on the workspace**

```bash
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/views.rs crates/engine/src/game/inspection.rs crates/gui/src/render/settlement.rs
git commit -m "feat(settlements): a city's page says how it is doing

One row, and only for a city -- a Server has no band, and a word that
never changes is worse than no word. Both page censuses updated: the
worst case grew by a row and the page has no scroll.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 9: The documentation obligation, and the release

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/seams.md`
- Modify: `Cargo.toml` (version)
- Check: `assets/settlements/README.md`, `docs/*-gen.py`

**Interfaces:**
- Consumes: everything.
- Produces: a branch ready to land.

- [ ] **Step 1: Find every doc claim this change falsifies**

The obligation is not "add a changelog line" — it is to grep for statements that are now false.

```bash
grep -rn 'Mainframe\|Server' assets/settlements/README.md docs/seams.md | grep -iv 'biome\|backplane'
grep -rn 'SETTLEMENT_SERVER_ROWS\|SETTLEMENT_MAINFRAME_ROWS\|settlement.*kind' docs/
```

Two known claims to check by hand:

- `crates/engine/src/settlements/mod.rs:11-16`'s module doc states a settlement's placement is derived and *"nothing in the save that could disagree with the ground."* That is still true of **where** a town stands, and now false of **what** it is. Amend it to say so precisely rather than deleting it — the placement rule is load-bearing and the growth exception is the interesting part.
- `crates/engine/src/settlements/mod.rs:43-54`'s `SettlementKind` doc says the two kinds exist so the difference is *"legible at a glance on the map rather than compared."* Add that one can now become the other, and that `Game::settlement_kind` is the only door that answers which.

Note: `docs/manual.md` and the root `README.md` are **carved out** of the documentation obligation. Do not touch either.

- [ ] **Step 2: Add the seams entry**

`docs/seams.md` documents load-bearing seams. This change adds one and the plan is explicit about what it is:

> **A settlement's kind is read through `Game::settlement_kind`, never `def.kind`.** Four sites read it — the shelf's rows, its standout share, the map glyph, the town page's label — and the authored kind is only half the answer once a run can grow a town. The load path is where a direct read is invisible: `restore_settlements` rebuilds every town's entity from the record, so a grown city drawn from `def.kind` reads `M` all run and comes back from a save reading `s`. `spawn_settlement_at` therefore takes **no kind at all**. Related: `Standings` must be restored *before* `Settlements` in `lifecycle.rs`, because drawing a town now reads its relation.

Match the file's existing entry format; read two neighbouring entries first.

- [ ] **Step 3: Write the CHANGELOG section**

Add a new section at the top of `CHANGELOG.md` for `0.13.117`, in the shape the existing sections use. Cover, in player-facing language: towns grow into cities on their own schedule; trading with a town brings that forward; a city you neglect or anger thins out but never stops being a city. Name the new tuning constants in whatever way the existing sections name constants.

- [ ] **Step 4: Bump the version**

In the workspace `Cargo.toml`, `0.13.116` → `0.13.117`. **A patch bump**, which is what this repo uses for features — a minor bump would move the internal dependency requirements and read as a broken workspace.

- [ ] **Step 5: Full gate**

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo fmt --all --check
```

Expected: all three clean. Run them bare; do not pipe.

- [ ] **Step 6: Commit**

```bash
git add CHANGELOG.md docs/seams.md Cargo.toml Cargo.lock crates/engine/src/settlements/mod.rs
git commit -m "docs: settlement growth in the changelog and the seams

Amends the settlements module doc: where a town stands is still derived
and unstorable, but what it *is* is now run state. Records the one-door
rule and the load-order dependency it created.

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

- [ ] **Step 7: Stop**

Do **not** merge, tag or push. Report back with the test output and hand the landing decision over.

---

## What this plan does not do

Named so a reviewer does not read them as oversights:

- **No playtest.** Nobody in this session can run the game — there is no display. Every task above ends at a green suite, and a green suite is not evidence that an `s` turning into an `M` reads as anything at all. The thirteen constants in Task 2 are answerable only at a keyboard.
- **A grown city changes only its shelf.** It posts no more contracts, fields no larger garrison and sends no heavier raiders. Each of those is a new query on an existing exhaustive match, none is blocked, and shipping four at once would leave none of them legible. Deliberately deferred by the spec.
- **Towns in regions nobody visits do not grow.** They are *found* grown, silently, which is the design's stated position and not a gap.
