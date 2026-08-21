# Item quality — Phase 2: drops roll it, and it reads

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the axis Phase 1 built actually vary and actually read. A
field drop rolls its quality off a poor flat floor; `Game::copy_name` says
what a copy compiled at; the `[I]` inspect page carries the figure as a
row of its own; and the two popup width guards are re-measured against the
new worst case.

**Architecture:** One new roll — `Game::roll_quality(floor)` beside
`Game::roll_gear_rarity`, since that is already the shared home of a
per-copy axis roll that `grant_gear_drop` calls from another file. It draws
the spread **in steps** and clamps, and Phase 3's crafting roll will hand
it a floor built out of a `CraftOrder` rather than growing a second
formula. The visible half is three sites: the name, the inspect row, and
the swap screen's row layout.

**Tech Stack:** Rust, `bevy_ecs` 0.19, `serde` + RON saves, `bevy_egui`
(gui width measurement in tests via `paint::with_painter`).

**Spec:** `docs/superpowers/specs/2026-08-21-item-quality-design.md`

**Roadmap:** `docs/superpowers/plans/2026-08-21-item-quality-plan.md` — its
**Global Constraints** section applies to every task here. Read it first.

**Branch:** `item-quality` (already checked out). Check
`git branch --show-current` before every commit — another session has
fast-forwarded and deleted a branch mid-task in this repo before.

---

## What measurement changed about this phase

The roadmap says Phase 2 "makes the axis visible with no UI restructuring,
because names are already built in one place." **That premise is false and
was measured before this plan was written**, not discovered during it.

At 900px the `PopupSize::Large` body is 1243.2px and one UI cell is
10.8438px — 114.65 cells. The swap screen's worst row builds its head as
`[a] {name:<50} {stats:<20}`, which `wrapped_row_lines` never breaks
because the head always leads. Today that head plus `draw_row`'s two-space
prefix is 111 cells (1202.9px) and it fits with 3.7 cells to spare. The
worst name is `Overclocked Singularity Matrix of Quiet Handshakes`, 50
cells exactly, which is where `SWAP_NAME_COLUMN = 50` comes from.

Appending ` (130%)` makes that name 57 cells, so the column must widen to
57, and the head becomes 118 cells = **1278.8px, 35.6px over the body**.
No shorter format rescues it: dropping the parentheses saves two cells and
still overflows.

So Task 4 pays a restructure the roadmap did not price: the swap row's
stat column stops being part of the un-wrappable head and becomes a
shed-able tag, exactly the treatment `inventory_row_lines` already gives
an equip tag and `draw_equip_swap` already gives the delta. Ordinary rows
are unchanged — the packer only sheds what will not fit.

The other two surfaces were measured at the same time and have room:

| Surface | Measured today | With quality | Room |
| --- | --- | --- | --- |
| Widest inventory line | 1105.4px | 1181.3px | 1243.2px |
| Tallest gear inspect page | 17 rows (Crash Handler) | 18 rows | 23 rows |
| Widest swap head | 1202.9px | 1278.8px | 1243.2px ← Task 4 |

## Fallout to expect, not to fix

**Equipment drops stop stacking in `Inventory`.** A rolled quality is
almost never `QUALITY_DEFAULT`, so `GearCopy::is_plain` goes false and
`add_copies` files the drop in `GearCopies` instead. That is the same
consequence the spec accepts for crafting, arriving one phase early on the
drop side. Both stores are already listed together on every screen that
names gear, and `count_copies` / `take_copies` route by the same
predicate, so nothing is lost — but a **fixture** that drops a weapon and
then reads `Inventory` will fail. Fix the fixture, never the feature.

**The `GameRng` stream shifts.** Every equipment drop now spends one more
draw. A seeded test elsewhere that changes its answer is that, not a
regression — see the standing note on RNG-stream shifts. A *material* drop
must still spend **no** draw, which `a_material_drop_spends_no_rarity_roll`
already guards; the new roll goes after the early return and after the
rarity and affix rolls, so for a given seed a dropped copy's rarity and
affix are unchanged.

---

### Task 1: The drop roll

**Files:**
- Modify: `crates/engine/src/tuning.rs` (append to the "Item quality"
  section that ends at `QUALITY_ABOVE_MAX`, around line 1424)
- Modify: `crates/engine/src/game/spawning.rs` (after `roll_gear_rarity`,
  which ends at line 528)
- Modify: `crates/engine/src/game/combat_rewards.rs:80-96`
  (`grant_gear_drop`)
- Test: `crates/engine/src/tests/combat_rewards.rs`

**Interfaces:**
- Consumes: `tuning::QUALITY_DEFAULT`, `QUALITY_MIN`, `QUALITY_MAX` (Phase
  1); `items::GearCopy::quality`.
- Produces: `tuning::QUALITY_STEP: u8` (= 5), `tuning::QUALITY_SPREAD: u8`
  (= 20), `tuning::QUALITY_DROP_BASE: u8` (= 70);
  `Game::roll_quality(&mut self, floor: u8) -> u8`, `pub(crate)`, which
  Phase 3 calls with a craft floor.

- [x] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/combat_rewards.rs`. Add
`QUALITY_DROP_BASE, QUALITY_MAX, QUALITY_MIN, QUALITY_SPREAD, QUALITY_STEP`
to the `crate::tuning::{...}` import at the top of the file.

```rust
/// **The world does not make good gear; your base does.** A field drop
/// rolls off `QUALITY_DROP_BASE`, below the crafting floor Phase 3 will
/// add, so an average drop loses to an average craft — which is the whole
/// design intent the axis exists to express.
///
/// The band is asserted rather than a single sample because the roll is
/// the point: a drop that always landed on its floor would satisfy any
/// bound test and still be the flat 100 this replaces.
#[test]
fn a_dropped_weapon_rolls_its_quality_off_the_drop_floor() {
    let mut game = Game::new(4402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let weapon = game
        .item_defs()
        .into_iter()
        .find(|d| d.equipment.is_some())
        .expect("shipped assets include equippable gear")
        .id;

    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..200 {
        let copy = game.grant_gear_drop(weapon.clone(), Rarity::Ordinary);
        assert!(
            (QUALITY_DROP_BASE..=QUALITY_DROP_BASE + QUALITY_SPREAD).contains(&copy.quality),
            "a drop rolls its spread off the drop floor, got {}",
            copy.quality
        );
        assert_eq!(
            copy.quality % QUALITY_STEP,
            0,
            "the spread is drawn in steps, never drawn fine and rounded: {}",
            copy.quality
        );
        seen.insert(copy.quality);
    }
    assert!(
        seen.len() > 1,
        "every drop rolled {seen:?} — the spread is not being drawn"
    );
}

/// The clamp is the band and both of its ends are reachable, so it is
/// asserted at both. A floor above the ceiling is what Phase 3's developed
/// base produces (`QUALITY_BASE` + bench + perk + care already exceeds
/// `QUALITY_MAX`), and it must saturate rather than wrap a `u8`.
#[test]
fn the_quality_roll_clamps_at_both_ends_of_the_band() {
    let mut game = Game::new(4403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    for _ in 0..50 {
        assert_eq!(game.roll_quality(QUALITY_MAX), QUALITY_MAX);
        assert!(game.roll_quality(0) >= QUALITY_MIN);
    }
}
```

- [x] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine quality_roll -- --include-ignored`
and `cargo test -p feral-processes-engine a_dropped_weapon_rolls`
Expected: FAIL to compile — `no method named roll_quality`, and the
tuning constants are undefined.

- [x] **Step 3: Add the constants**

Append to the "Item quality" section of `crates/engine/src/tuning.rs`,
after `QUALITY_ABOVE_MAX`:

```rust
/// The granularity of a rolled quality. Every term in the roll is a
/// multiple of this and the spread is drawn **in steps** of it, so the sum
/// is on-step by construction and the clamp cannot produce an off-step
/// value.
///
/// Drawn in steps rather than drawn fine and rounded: rounding a uniform
/// draw onto a lattice gives the two end buckets half the width of the
/// others, which biases exactly the ends of the band the player is
/// reading for.
pub const QUALITY_STEP: u8 = 5;

/// The luck term — how far above its floor any one compile can roll,
/// drawn as `0..=QUALITY_SPREAD` in `QUALITY_STEP`s.
///
/// It is the same width at every floor, so improving a bench or taking the
/// perk moves the whole band up rather than narrowing it. That is what
/// keeps compiling a batch and keeping the best worth doing at every stage
/// of a run rather than only at the start of one.
pub const QUALITY_SPREAD: u8 = 20;

/// The floor a **found** copy rolls off, giving drops a 70–90 band against
/// a crafted piece's 80–100 at a bare bench.
///
/// Deliberately below `QUALITY_BASE` (Phase 3). Leaving drops at a flat
/// `QUALITY_DEFAULT` was rejected because an average find would then beat
/// a bad craft, which cuts against the whole intent; giving them the
/// crafting band was rejected because the base would then confer no
/// reliability advantage. A lucky find can still beat an unlucky craft,
/// which is what keeps a drop a lottery ticket rather than a disappointment.
pub const QUALITY_DROP_BASE: u8 = 70;
```

- [x] **Step 4: Add the roll**

In `crates/engine/src/game/spawning.rs`, directly after
`roll_gear_rarity`:

```rust
    /// The quality one copy compiles or drops at: its `floor` plus a
    /// stepped spread, clamped to the band.
    ///
    /// **One formula and one clamp for every source of a copy.** It lives
    /// beside `roll_gear_rarity` for that one's reason — a per-copy axis is
    /// rolled from more than one file, so the ladder belongs where both
    /// callers reach it rather than in whichever of them was written first.
    /// `grant_gear_drop` passes `QUALITY_DROP_BASE`; crafting passes a
    /// floor it builds out of a bench tier, a perk and the careful toggle.
    ///
    /// The spread is drawn **in steps** of `QUALITY_STEP` rather than drawn
    /// fine and rounded, so every reachable value is on the lattice and the
    /// end buckets are the same width as the middle ones.
    ///
    /// `floor` is a `u8` and may legitimately exceed `QUALITY_MAX` — a
    /// developed base's floor does — so the sum is taken in `u16` before
    /// the clamp rather than saturating twice.
    pub(crate) fn roll_quality(&mut self, floor: u8) -> u8 {
        let steps = (QUALITY_SPREAD / QUALITY_STEP) as u32;
        let luck = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..=steps) as u16 * QUALITY_STEP as u16
        };
        (floor as u16 + luck).clamp(QUALITY_MIN as u16, QUALITY_MAX as u16) as u8
    }
```

Add `QUALITY_MAX, QUALITY_MIN, QUALITY_SPREAD, QUALITY_STEP` to
`spawning.rs`'s `use crate::tuning::{...}` list.

- [x] **Step 5: Call it from the drop**

In `crates/engine/src/game/combat_rewards.rs`, replace the hardcoded
`quality` in `grant_gear_drop`:

```rust
        let rarity = self.roll_gear_rarity().max(floor);
        let affix = self.roll_affix(&item);
        let quality = self.roll_quality(crate::tuning::QUALITY_DROP_BASE);
        let copy = GearCopy {
            item,
            rarity,
            tier: 0,
            affix,
            quality,
        };
```

Extend that function's doc comment with a paragraph naming the third roll
and why it is last:

```rust
    /// Three rolls in a fixed order — rarity, affix, quality — and the new
    /// one is last on purpose: for a given seed a dropped copy's tier and
    /// affix are exactly what they were before quality existed, so only
    /// what follows the drop in the stream moves.
```

- [x] **Step 6: Run the tests and the neighbours that guard the stream**

Run:
```sh
cargo test -p feral-processes-engine a_dropped_weapon_rolls
cargo test -p feral-processes-engine the_quality_roll_clamps
cargo test -p feral-processes-engine a_material_drop_spends_no_rarity_roll
cargo test -p feral-processes-engine combat_rewards
```
Expected: PASS. `a_material_drop_spends_no_rarity_roll` is the guard that
the roll sits below the early return — if it fails, the roll was put above
it.

- [x] **Step 7: Mutation-check both new tests**

For each: make the fix wrong, watch the test fail, restore.
- Replace `roll_quality`'s body with `QUALITY_DEFAULT` →
  `a_dropped_weapon_rolls_its_quality_off_the_drop_floor` must fail on the
  band assertion.
- Draw fine instead of stepped (`rng.0.random_range(0..=QUALITY_SPREAD)`,
  no `* QUALITY_STEP`) → the `% QUALITY_STEP` assertion must fail.
- Drop the `.clamp(...)` → `the_quality_roll_clamps_at_both_ends_of_the_band`
  must fail.
Record each in the commit body.

- [x] **Step 8: Run the full suite and repair fixtures**

Run: `cargo test --workspace`

Expect failures in two families and fix them as fixtures, not as feature:
- a test that drops equipment and then reads `Inventory` — read
  `count_copies` or `GearCopies` instead;
- a seeded test whose stream moved — re-derive its expectation, do not
  re-seed it to hide the shift.

- [x] **Step 9: Commit**

```bash
git branch --show-current   # must be item-quality
git add crates/engine/src/tuning.rs crates/engine/src/game/spawning.rs \
        crates/engine/src/game/combat_rewards.rs \
        crates/engine/src/tests/combat_rewards.rs
git commit -m "feat(engine): a field drop rolls the quality it compiled at"
```

---

### Task 2: The figure in the name

**Files:**
- Modify: `crates/engine/src/game/combat_rewards.rs:160-178` (`copy_name`)
- Test: `crates/engine/src/tests/combat_rewards.rs`

**Interfaces:**
- Consumes: `Game::roll_quality` (Task 1) only for its test fixtures.
- Produces: no new signature. `Game::copy_name` gains a trailing
  ` ({quality}%)` segment for any copy off `QUALITY_DEFAULT`.

- [x] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/combat_rewards.rs`:

```rust
/// **A name is what lets two otherwise identical copies be told apart**,
/// which is the whole point of a fourth axis — five compiles of one blade
/// are five rows in the ledger and the player has to be able to pick the
/// good one.
///
/// A copy at spec shows **no** figure, the call `Rarity::label` makes for
/// `Ordinary`. Everything in every existing save is at
/// `QUALITY_DEFAULT`, so nothing already on screen gets wider.
#[test]
fn a_name_carries_the_quality_only_when_it_is_off_spec() {
    let game = Game::new(4404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let whip = ItemId::from(ids::MONOFILAMENT_WHIP);
    let at_spec = GearCopy::plain(whip.clone());
    let bare = game.copy_name(&at_spec);

    assert!(
        !bare.contains('%'),
        "a copy compiled to spec names no figure: {bare}"
    );

    let poor = GearCopy {
        quality: 85,
        ..at_spec.clone()
    };
    assert_eq!(game.copy_name(&poor), format!("{bare} (85%)"));

    // The figure goes last, after the rare tier's word and the affix's
    // decoration — one segment appended to a name already built, so the
    // three axes cannot come to fight over the order.
    let decorated = GearCopy {
        rarity: Rarity::Gold,
        quality: 130,
        ..at_spec
    };
    let name = game.copy_name(&decorated);
    assert!(name.ends_with(" (130%)"), "{name}");
    assert!(
        name.starts_with(Rarity::Gold.label().expect("Gold reads as a word")),
        "{name}"
    );
}
```

`ids` and `GearCopy` are already in scope through `use crate::*;` and
`super::support::*`; add `use crate::items::GearCopy;` if the compiler
disagrees.

- [x] **Step 2: Run it and watch it fail**

Run: `cargo test -p feral-processes-engine a_name_carries_the_quality`
Expected: FAIL — `assertion failed: left == right`, the name has no
figure.

- [x] **Step 3: Append the segment**

In `crates/engine/src/game/combat_rewards.rs`, `copy_name`:

```rust
    pub fn copy_name(&self, copy: &GearCopy) -> String {
        let base = self.item_name(&copy.item);
        let named = match self.affix_of(copy) {
            Some(affix) => affix.decorate(base),
            None => base.to_string(),
        };
        let tiered = match copy.rarity.label() {
            Some(tier) => format!("{tier} {named}"),
            None => named,
        };
        match copy.quality {
            crate::tuning::QUALITY_DEFAULT => tiered,
            q => format!("{tiered} ({q}%)"),
        }
    }
```

Extend the doc comment:

```rust
    /// The quality figure goes **last**, after the tier word and the
    /// affix's decoration, and is omitted at `QUALITY_DEFAULT` — the call
    /// `Rarity::label` makes for `Ordinary`, and the reason nothing already
    /// on screen gets wider when this ships. It costs seven cells on the
    /// worst case, which is why `SWAP_NAME_COLUMN` moved with it.
```

- [x] **Step 4: Run the test**

Run: `cargo test -p feral-processes-engine a_name_carries_the_quality`
Expected: PASS

- [x] **Step 5: Mutation-check**

Delete the `match copy.quality` and return `tiered` → the test must fail.
Move the figure in front of the tier word → the `starts_with` assertion
must fail. Restore.

- [x] **Step 6: Run the engine suite**

Run: `cargo test -p feral-processes-engine`

Expected fallout: any test asserting a literal drop-line or log string for
a piece of equipment. Those names now carry a figure — update the
expectation, since the name is the deliverable.

- [x] **Step 7: Commit**

```bash
git branch --show-current
git add crates/engine/src/game/combat_rewards.rs crates/engine/src/tests/combat_rewards.rs
git commit -m "feat(engine): a copy's name says what it compiled at"
```

---

### Task 3: The row on the inspect page

**Files:**
- Modify: `crates/engine/src/views.rs:185-204` (`WornDetailView`)
- Modify: `crates/engine/src/game/catalog.rs` (`worn_detail`, just below
  `gear_detail` at line 402)
- Modify: `crates/gui/src/render/inventory.rs:344-370` (`gear_inspect_rows`)
- Test: `crates/engine/src/tests/gear_detail.rs`,
  `crates/gui/src/render/inventory.rs` tests module

**Interfaces:**
- Consumes: `items::GearCopy::quality`.
- Produces: `views::WornDetailView::quality: u8`.

- [x] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/gear_detail.rs`:

```rust
/// **The inspect page is where two copies get compared**, so it states the
/// quality outright rather than leaving the player to read it off the name
/// — including at spec, where the name says nothing. A figure missing from
/// a detail page reads as *unknown*, not as 100.
///
/// It rides `WornDetailView` and so is absent for a consumable or a
/// currency, which is honest: only equipment rolls quality.
#[test]
fn the_inspect_page_states_what_a_copy_compiled_at() {
    let game = Game::new(4405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    let at_spec = GearCopy::plain(ItemId::from("kinetic_edge"));
    assert_eq!(
        game.gear_detail(&at_spec, player)
            .worn
            .expect("a weapon is worn")
            .quality,
        QUALITY_DEFAULT
    );

    let off_spec = GearCopy {
        quality: 115,
        ..at_spec
    };
    assert_eq!(
        game.gear_detail(&off_spec, player)
            .worn
            .expect("a weapon is worn")
            .quality,
        115
    );

    let material = GearCopy::plain(ItemId::from("core_fragment"));
    assert!(
        game.gear_detail(&material, player).worn.is_none(),
        "a material has no slot and so no quality to state"
    );
}
```

Add `use crate::tuning::QUALITY_DEFAULT;` to that file's imports.

Append to the tests module in `crates/gui/src/render/inventory.rs`:

```rust
    /// The figure reaches the page as a row of its own. `copy_name` puts it
    /// in the title too, but only off spec — so without this row a copy at
    /// spec has no statement of its quality anywhere on the one screen
    /// built for comparing two copies.
    #[test]
    fn the_gear_page_carries_a_quality_row() {
        let assets = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"));
        let game = Game::new(43, DifficultyMode::Forgiving, assets).expect("shipped assets");

        let quality_row = |copy: GearCopy| {
            let inspect = GearInspect {
                copy,
                wearer: None,
                from: Mode::Inventory,
            };
            gear_inspect_rows(&game, &inspect).iter().any(|row| {
                matches!(row, Row::Text(t) | Row::TextColored(t, _) if t.contains("Compiled at 115%"))
            })
        };

        assert!(quality_row(GearCopy {
            quality: 115,
            ..GearCopy::plain("kinetic_edge".into())
        }));
        assert!(
            !quality_row(GearCopy::plain("kinetic_edge".into())),
            "a copy at spec must not claim 115%"
        );
    }
```

- [x] **Step 2: Run them and watch them fail**

Run: `cargo test -p feral-processes-engine the_inspect_page_states` then
`cargo test -p feral-processes-gui the_gear_page_carries_a_quality_row`
Expected: FAIL to compile — `WornDetailView` has no field `quality`.

- [x] **Step 3: Add the field and fill it**

In `crates/engine/src/views.rs`, in `WornDetailView`, directly under
`level`:

```rust
    /// How well this copy compiled, as a percentage of the item's authored
    /// bonus — `items::GearCopy::quality`. Carried on the view rather than
    /// left to the renderer to read off the copy, because
    /// `GearDetailView`'s promise is that the page is one call: a renderer
    /// reaching past it for one figure is how the four screens that
    /// rebuilt `copy_bonus` by hand each started.
    ///
    /// On the *worn* half deliberately: only equipment rolls quality, so a
    /// consumable's page has nothing to state rather than a defaulted 100.
    pub quality: u8,
```

In `crates/engine/src/game/catalog.rs`, `worn_detail`, add `quality:
copy.quality,` to the `WornDetailView` literal it returns.

- [x] **Step 4: Draw the row**

In `crates/gui/src/render/inventory.rs`, in `gear_inspect_rows`, directly
after the `"{}: {}"` slot/stats row and before the accuracy block:

```rust
        // Under the stats it explains, and unconditional — the name carries
        // the figure only when it is off spec, so this is the only place a
        // copy at spec says what it compiled at.
        rows.push(text_row(format!(
            "Compiled at {}% of spec",
            worn.quality
        )));
```

- [x] **Step 5: Run the tests**

Run:
```sh
cargo test -p feral-processes-engine the_inspect_page_states
cargo test -p feral-processes-gui the_gear_page_carries_a_quality_row
cargo test -p feral-processes-gui the_tallest_gear_page_fits_its_popup
```
Expected: PASS. The height test has six rows of headroom (17 → 18 against
a 23-row cap, measured); if it fails, the page grew somewhere else too and
that is the thing to look at.

- [x] **Step 6: Mutation-check**

Delete the `rows.push` → the gui test must fail. Hardcode
`quality: QUALITY_DEFAULT` in `worn_detail` → the engine test must fail on
the 115 case. Restore both.

- [x] **Step 7: Commit**

```bash
git branch --show-current
git add crates/engine/src/views.rs crates/engine/src/game/catalog.rs \
        crates/engine/src/tests/gear_detail.rs crates/gui/src/render/inventory.rs
git commit -m "feat(gui): the inspect page states what a copy compiled at"
```

---

### Task 4: The swap row's stats become a shed-able tag

**Files:**
- Modify: `crates/app-core/src/lib.rs:330-350` (`SWAP_NAME_COLUMN` and its
  doc), `:476-481` (`swap_columns`), and `SwapRow`'s definition and the
  loop that builds it (around `:390-420`)
- Modify: `crates/gui/src/render/inventory.rs:262-274` (`draw_equip_swap`)
- Test: `crates/app-core/src/tests/inventory.rs:646-700` and `:880-935`;
  `crates/gui/src/render/inventory.rs:755` and `:612`

**Interfaces:**
- Consumes: `Game::copy_name` (Task 2).
- Produces: `SwapRow::stats: String` (the padded stat column, carrying its
  own leading space so `wrapped_row_lines` can pack it);
  `SwapRow::label` narrows to the padded **name only**;
  `SWAP_NAME_COLUMN` becomes 57.

**Why this shape.** `wrapped_row_lines` never breaks the head, so anything
in the head has to fit at the worst case. The delta is already a tag for
exactly this reason and the file already says so ("six stat axes and their
six deltas do not fit one popup line"). The stat column joins it. Padding
stays *inside* the tag (`" {stats:<SWAP_STATS_COLUMN$}"`) so the delta
still lands in the same column on every row that keeps both on one line —
which is every ordinary row: a 61-cell head plus a 21-cell stat tag is 82
against `ROW_WRAP_COLUMNS`'s 100.

- [x] **Step 1: Widen the census guards first and watch them fail**

In `crates/app-core/src/tests/inventory.rs`, extend
`no_shipped_copy_name_outgrows_the_swap_name_column` to sweep quality —
the worst name is the widest figure, so `QUALITY_MAX` is the case, and
`QUALITY_DEFAULT` is swept alongside it because that one names no figure
at all:

```rust
    let mut worst = (String::new(), 0usize);
    for item in &equippables {
        for rarity in Rarity::ALL {
            for affix in &affixes {
                for quality in [QUALITY_DEFAULT, QUALITY_MIN, QUALITY_MAX] {
                    let name = game.copy_name(&GearCopy {
                        rarity,
                        tier: 0,
                        affix: affix.clone(),
                        quality,
                        ..GearCopy::plain(item.clone())
                    });
                    if name.chars().count() > worst.1 {
                        worst = (name.clone(), name.chars().count());
                    }
                }
            }
        }
    }
```

and extend `no_shipped_gear_summary_outgrows_the_swap_stats_column` the
same way, adding `for quality in [QUALITY_DEFAULT, QUALITY_MAX]` around
its copy construction and setting `quality` on the literal — a copy at
`QUALITY_MAX` prices 30% higher through `copy_bonus`, which is where a
stat figure could gain a digit.

Add `use feral_processes_engine::tuning::{QUALITY_DEFAULT, QUALITY_MAX,
QUALITY_MIN};` to that test file.

Run: `cargo test -p feral-processes-app-core outgrows_the_swap`
Expected: the name test FAILS with `"Overclocked Singularity Matrix of
Quiet Handshakes (130%)" is 57 cells and the column is 50`. If the stats
test also fails, take the number it reports — that is the new
`WIDEST_MEASURED_SWAP_STATS`, and the gui's hand-written worst-case string
in Step 4 must be re-measured to match it.

- [x] **Step 2: Widen the column and split the stats out**

In `crates/app-core/src/lib.rs`:

```rust
const SWAP_NAME_COLUMN: usize = 57;
```

and rewrite its doc comment's measurement paragraph:

```rust
/// Wide enough for the longest name `Game::copy_name` can build out of the
/// shipped assets — a rare tier's word, an affix's prefix or suffix, the
/// item's own name, and the quality figure. "Overclocked Singularity
/// Matrix of Quiet Handshakes (130%)" is 57 cells.
```

Replace `swap_columns` with a name-only padder:

```rust
/// The name column of one swap row, padded so the names line up down the
/// list. The stat column is `SwapRow::stats` and is packed on afterwards
/// rather than built in here — see `SwapRow::stats`.
fn swap_name_column(name: &str) -> String {
    format!("{name:<SWAP_NAME_COLUMN$}")
}
```

On `SwapRow`, narrow `label`'s doc to say it is the padded name, and add:

```rust
    /// What the player would be wearing, padded to `SWAP_STATS_COLUMN` and
    /// carrying its own leading space.
    ///
    /// **A tag rather than part of `label`**, because `wrapped_row_lines`
    /// never breaks the head: at the worst case — the longest name, its
    /// quality figure, and six stat axes at zone 10 on a maxed Gold copy —
    /// a joined head is 118 cells against a 114-cell popup body, so the
    /// text at the right edge is simply lost. As a tag it sheds onto a
    /// continuation exactly when it has to, which is the treatment the
    /// delta beside it already gets and `inventory_row_lines` gives an
    /// equip tag. The padding lives inside the tag so the delta still lands
    /// in one column on every row that keeps both on one line.
    pub stats: String,
```

Update the loop that builds `SwapRow` to set `label:
swap_name_column(&name)` and `stats: format!(" {:<SWAP_STATS_COLUMN$}",
stat_summary(...))` — keep whatever expression currently feeds
`swap_columns`' second argument, moved verbatim.

- [x] **Step 3: Pack it in the renderer**

In `crates/gui/src/render/inventory.rs`, `draw_equip_swap`:

```rust
        for line in wrapped_row_lines(
            format!("[{}] {}", menu_shortcut(i), row.label),
            &[row.stats.clone(), format!(" {}", row.delta)],
        ) {
```

and rewrite the comment above it:

```rust
        // Wrapped, not joined: the name column alone can run 57 cells, and
        // six stat axes plus their six deltas do not fit the same line — so
        // both trailing columns are tags and shed onto a continuation when
        // they have to. Ordinary rows keep all three on one line; the
        // packer only sheds what will not fit. Both lines carry the row's
        // selection and tier colour, so a wrapped entry still highlights as
        // one thing.
```

- [x] **Step 4: Re-measure the gui worst case**

Rewrite `the_widest_swap_row_still_fits_its_popup` so it builds its lines
the way `draw_equip_swap` now does — a name-only head and two tags — and
keep it hand-written so it stays the census's counterpart:

```rust
        let head = format!(
            "[a] {:<57}",
            format!(
                "{} Singularity Matrix of Quiet Handshakes (130%)",
                Rarity::Gold.label().expect("Gold reads as a word")
            ),
        );
        let lines = wrapped_row_lines(
            head,
            &[
                " 268–403 DMG +134 ATK +134 MIT +90 ACC +134 DECOMP T3/3".to_string(),
                " -206–-310 DMG -103 ATK -103 MIT -69 ACC -103 DECOMP".to_string(),
            ],
        );
        assert_eq!(
            lines.len(),
            3,
            "the worst case sheds both tags, or this measures the easy case: {lines:#?}"
        );
```

The measurement block underneath it is unchanged — every line still has to
fit `1440.0 * 0.88 - m.pad * 2.0`.

Update the doc comment above the test to record the measurement:

```rust
    /// **Measured, not counted.** At 900px the body is 1243.2px and a UI
    /// cell is 10.8438px — 114.65 cells. The old joined head was 111 cells
    /// and fitted with 3.7 to spare; the quality figure costs seven, which
    /// put it 35.6px past the edge. That is why the stat column is a tag
    /// now and not part of the head.
```

- [x] **Step 5: Extend the inventory census to quality**

In the same tests module, extend
`no_shipped_inventory_row_overflows_its_popup`'s copy construction with
`quality: QUALITY_MAX,` (import
`feral_processes_engine::tuning::QUALITY_MAX`), since the widest inventory
line grows by the same seven cells and by whatever the 30% stat uplift
costs. Measured today it lands at 1181.3px against 1243.2px, so it
passes — the point is that a future asset cannot regress it silently.

Leave `only_the_overflowing_inventory_row_spends_a_second_line` alone: its
plain copy is at `QUALITY_DEFAULT` and names no figure, so it is still one
line, and its worst case is still two.

- [x] **Step 6: Run every width and layout test**

Run:
```sh
cargo test -p feral-processes-app-core outgrows_the_swap
cargo test -p feral-processes-gui the_widest_swap_row
cargo test -p feral-processes-gui no_shipped_inventory_row_overflows
cargo test -p feral-processes-gui only_the_overflowing_inventory_row
cargo test -p feral-processes-app-core inventory
```
Expected: PASS.

- [x] **Step 7: Mutation-check the restructure**

Put the stats back inside the head (`format!("[{}] {}{}", …,
row.label, row.stats)` in `draw_equip_swap`, and pass only the delta as a
tag) → `the_widest_swap_row_still_fits_its_popup` must fail on the
overflow. Restore. This is the check that the test is measuring the thing
the restructure exists for.

Then set `SWAP_NAME_COLUMN` back to 50 → the app-core census must fail.
Restore.

- [x] **Step 8: Commit**

```bash
git branch --show-current
git add crates/app-core/src/lib.rs crates/app-core/src/tests/inventory.rs \
        crates/gui/src/render/inventory.rs
git commit -m "fix(gui): the swap row sheds its stat column rather than losing it"
```

---

### Task 5: The gates, the seam and the rule

**Files:**
- Modify: `docs/seams.md` (the items/gear section, beside the Phase 1
  quality entry)
- Modify: `CLAUDE.md` (**Items, gear and economy**, the quality bullet
  Phase 1 added), then `cp CLAUDE.md AGENTS.md`
- Modify: `docs/superpowers/plans/2026-08-21-item-quality-plan.md` (tick
  Phase 2 in the phase table)

**Interfaces:**
- Consumes: everything above.
- Produces: nothing executable.

- [x] **Step 1: Run the full gate**

```sh
cargo fmt
cargo clippy --workspace
cargo test --workspace
```
Fix warnings rather than silencing them. Do **not** run `balance_sim` as a
gate on this phase's numbers — quality sits outside it by the same
documented exclusion that keeps `Rarity` out, and if its curves moved, the
exclusion is wrong rather than the test.

- [x] **Step 2: Write the argument into `docs/seams.md`**

Under the existing quality entry, add the two things this phase decided
and their reasoning:

- **`Game::roll_quality` is the one formula and the one clamp**, and it
  lives beside `roll_gear_rarity` rather than in either caller because two
  files roll the same axis. The spread is drawn in steps of `QUALITY_STEP`
  so the end buckets are not half-width. It is the **third** roll in
  `grant_gear_drop`, which is what keeps a seeded copy's rarity and affix
  where they were. `QUALITY_DROP_BASE` sits below the crafting floor
  deliberately: the world does not make good gear.
- **The swap row's stat column is a tag, not part of the head.** With the
  quality figure the joined head measured 118 cells against a 114.65-cell
  body — 35.6px lost off the right edge, silently, since `draw_row` clips
  vertically only. Record the numbers, since the next person to widen a
  column will want them.
- **An equipment drop no longer stacks in `Inventory`**, because a rolled
  quality makes it non-plain. Note that this is `is_plain`'s fourth `&&`
  doing its job and that the fix for a surprised fixture is the fixture.

- [x] **Step 3: Put the rule in `CLAUDE.md`**

Extend the quality bullet under **Items, gear and economy** with one
sentence each for the roll's home, the stepped draw, and the swap row's
tag. Keep it to the rule and the trap — the argument belongs in
`seams.md`, which the bullet already points at.

Then: `cp CLAUDE.md AGENTS.md` (they are gitignored twins with no tracking
to catch drift).

- [x] **Step 4: Tick the roadmap**

In `docs/superpowers/plans/2026-08-21-item-quality-plan.md`, mark Phase 2
done in the phase table and point its **Plan file** cell at
`2026-08-21-item-quality-phase-2.md`.

- [x] **Step 5: Commit**

```bash
git branch --show-current
git add docs/seams.md CLAUDE.md docs/superpowers/plans/2026-08-21-item-quality-plan.md \
        docs/superpowers/plans/2026-08-21-item-quality-phase-2.md
git commit -m "docs: the drop roll, the stepped spread, and the swap row's shed"
```

---

## Phase exit

Green on `cargo test --workspace`, and the axis is visible: a dropped
weapon names a figure, the inspect page states one for everything
wearable, and the swap screen still fits its popup at the worst case the
shipped assets can build.

**Not yet true, and deliberately:** crafting still produces
`QUALITY_DEFAULT`, so a base does not yet out-produce the world — that is
Phase 3, and it is the phase to play before calling any of these numbers
correct.

**Worth a look in a session before Phase 3:** whether a 70–90 drop band
reads as "found gear is junk" rather than as "found gear is a lottery
ticket". The comparison it is meant to lose to does not exist until Phase
3 ships, so the honest read is only available afterwards.
