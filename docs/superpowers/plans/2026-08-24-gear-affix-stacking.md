# Gear Affix Stacking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let two copies of a piece of gear fuse when they differ in
quality and affixes — averaging the quality, and carrying every affix
forward onto the result.

**Architecture:** `items::GearCopy` stops carrying one optional affix and
starts carrying a sorted `Vec<AffixId>`. Fusion's match relaxes from
whole-value equality to item/rarity/tier and unions the two lists. The
save keeps its old field shape behind a load-only compatibility field, so
no `SAVE_FORMAT_VERSION` bump. Naming and the inspect page grow bounded
renderings of an unbounded list.

**Tech Stack:** Rust, `bevy_ecs` (engine only), `serde`/RON saves, egui via
`crates/gui`.

**Spec:** `docs/superpowers/specs/2026-08-24-gear-affix-stacking-design.md`
— read it first. It carries the argument for every decision below; this
plan carries only the work.

## Global Constraints

- **No `SAVE_FORMAT_VERSION` bump.** If you reach a point where one seems
  necessary, stop and report rather than bumping — the whole save section
  of the spec exists to avoid it.
- **`GearCopy::affixes` is always sorted.** It is the key of the
  `GearCopies` ledger, of `EquippedItem` and of the buyback shelf, all
  found by `==`. Every construction site goes through the canonicalising
  constructor.
- **`cargo fmt` and `cargo clippy --workspace` after every task**, warnings
  fixed rather than silenced.
- **`cargo test --workspace` is the gate at the end of every task.** A
  targeted `cargo test -p feral-processes-engine <name>` is for the inner
  TDD loop only.
- **Every new test is mutation-proved**: delete or invert the fix, watch
  the test fail, restore. Record the mutation and the failure in the task
  report. A test that passes with the fix removed is not coverage.
- **Do not touch `docs/manual.md` or the root `README.md`** — both are
  carved out of the documentation obligation. `assets/*/README.md`,
  `dev-arenas/README.md` and `CHANGELOG.md` still apply.
- **No version bump and no `CHANGELOG.md` section on the branch.** Both
  happen once, at the merge.
- **Do not push.** Commit freely; the user asks for pushes.
- Balance is unmeasured until an arena run happens (Task 6). Do not claim
  the numbers are safe.

---

### Task 1: Flip `GearCopy::affix` to a sorted `affixes` list

This is the atomic task. Rust will not let the field's type change land
in pieces — every construction site, the save, and the arena schema move
together or the workspace does not compile. **Behaviour must not change
in this task**: every copy still carries zero or one affix, and every
existing test stays green without being edited, except where a test names
the field directly.

**Files:**
- Modify: `crates/engine/src/items.rs` — the field, `plain`, `is_plain`,
  the new constructor, the new `fusable_with` predicate.
- Modify: `crates/engine/src/game/combat_rewards.rs` — `affix_of` becomes
  `affixes_of`; `copy_bonus`'s caller; `grant_gear_drop` (~line 87) still
  rolls at most one.
- Modify: `crates/engine/src/game/crafting.rs:442` — `copy_bonus` folds a
  list instead of matching an `Option`.
- Modify: `crates/engine/src/save.rs` — `EquippedItemSave`, the three flat
  `PlayerSave` affix fields, and `SaveData::gear_copies`.
- Modify: `crates/engine/src/game/lifecycle.rs` — `worn_from_save` /
  `worn_to_save` (lines 34–63) are the named conversion pair the shim
  belongs in; also the `gear_copies` read/write sites and the three
  `PlayerSave` slot sites (~399, 543, 813, 1306, 1320, 1334).
- Modify: `crates/engine/src/arena/scenario.rs:120–150` — `EquipSpec`.
- Modify: `crates/engine/src/arena/setup.rs` — five `GearCopy` literals.
- Modify: `crates/app-core/src/app/arena.rs:781`,
  `crates/app-core/src/tests/support.rs` (`affixed_gear`, ~832),
  `crates/app-core/src/tests/inventory.rs`,
  `crates/gui/src/render/inventory.rs:792,922`.
- Modify: `assets/affixes/README.md`, `dev-arenas/README.md`.
- Test: inline `#[cfg(test)]` in `crates/engine/src/save.rs` (beside the
  existing `GearCopyProbe` RON-fragment tests, ~line 900–980);
  `crates/engine/src/tests/equipment.rs`.

**Interfaces:**

- Consumes: nothing.
- Produces, for every later task:
  - `GearCopy { item, rarity, tier, affixes: Vec<AffixId>, quality }`
  - `GearCopy::plain(item: ItemId) -> Self` — unchanged signature, empty
    list.
  - `GearCopy::with_affixes(item, rarity, tier, affixes, quality) -> Self`
    — sorts on the way in. **The only way a non-empty list is built.**
  - `GearCopy::is_plain(&self) -> bool` — now includes
    `self.affixes.is_empty()`.
  - `GearCopy::fusable_with(&self, other: &Self) -> bool` — item, rarity
    and tier equal. Written here, first used in Task 2.
  - `Game::affixes_of(&self, copy: &GearCopy) -> Vec<&AffixDef>` —
    `pub(crate)`, replaces `affix_of`, skips ids the build cannot
    resolve, preserves input order.
  - `save::EquippedItemSave { .., affix: Option<AffixId>, affixes: Vec<AffixId> }`
  - `save::GearCopySave { item, rarity, tier, affix, affixes, quality }`
  - `arena::scenario::EquipSpec { item, tier, rarity, affixes: Vec<AffixId> }`

- [ ] **Step 1: Read the spec's "The data shape" and "Save compatibility" sections.**
  They carry the reasoning; do not re-derive it.

- [ ] **Step 2: Write the failing save-compatibility tests first.**
  Inline in `save.rs`, in the shape of the existing `GearCopyProbe`
  tests, which parse RON fragments by hand rather than round-tripping a
  whole save. Three tests, each named for what it asserts:
  - a fragment carrying the legacy singular `affix: Some("of_static")`
    deserializes into a struct whose `affixes` holds exactly that one id;
  - a fragment carrying `affixes: ["hardened", "of_static"]` and no
    `affix` key deserializes with both;
  - a fragment carrying neither key deserializes empty.
  These will not compile yet. That is the failure.

- [ ] **Step 3: Run them and confirm they fail to compile**, naming the
  missing field. `cargo test -p feral-processes-engine save::`

- [ ] **Step 4: Change the field on `GearCopy`.**
  `#[serde(default)] pub affixes: Vec<AffixId>` replacing `affix`. Update
  the field's doc comment: keep the existing paragraph about
  `#[serde(default)]` being purely additive and about an unknown id
  reading as unaffixed, and **add** the sorted invariant and why (the
  three `==`-keyed stores). Update `plain`, `is_plain`, and add
  `with_affixes` and `fusable_with` with doc comments stating their
  single-definition role.

  The constructor is the one piece worth spelling out, because the sort
  is the invariant:

```rust
pub fn with_affixes(
    item: ItemId,
    rarity: Rarity,
    tier: u32,
    mut affixes: Vec<AffixId>,
    quality: u8,
) -> Self {
    // Sorted, not deduped: `[A, B]` and `[B, A]` are the same copy to a
    // player and must be the same copy to `Eq`, or one is written to a
    // row and looked up at another. Duplicates are the feature.
    affixes.sort();
    Self { item, rarity, tier, affixes, quality }
}
```

- [ ] **Step 5: Add the save-side shim.**
  On `EquippedItemSave` and the new `GearCopySave`, keep
  `#[serde(default)] pub affix: Option<AffixId>` beside
  `#[serde(default)] pub affixes: Vec<AffixId>`. On `PlayerSave`, add
  `weapon_affixes` / `armor_affixes` / `module_affixes` beside the three
  retained singular fields. Document each legacy field as load-only, with
  `PlayerSave::fused_gear` named as the precedent.

  One shared helper does the lift, so the four sites cannot drift:

```rust
/// A save's affixes, taking the list when it has one and lifting the
/// pre-stacking singular field when it does not. Load-only: the write
/// side fills `affixes` and leaves `affix` empty.
fn affixes_from_save(affix: Option<AffixId>, affixes: Vec<AffixId>) -> Vec<AffixId> {
    if affixes.is_empty() { affix.into_iter().collect() } else { affixes }
}
```

- [ ] **Step 6: Change `SaveData::gear_copies` from `Vec<(GearCopy, u32)>`
  to `Vec<(GearCopySave, u32)>`.** RON absorbs this because the tuple's
  first element is still a field-named struct with the same field names
  and one new defaulted field. Convert at the `Game::save` / `Game::load`
  boundary in `lifecycle.rs`, and put the conversion in named functions
  beside `worn_from_save` / `worn_to_save` for that pair's stated reason.

- [ ] **Step 7: Run the save tests and confirm they pass.**
  `cargo test -p feral-processes-engine save::`

- [ ] **Step 8: Sweep every remaining `affix:` construction site.**
  `rg -n '\.affix\b|affix:' --type rust crates/` finds them. Sites that
  build an unaffixed copy pass an empty vec or use `plain`; the two test
  helpers (`app-core/src/tests/support.rs::affixed_gear`,
  `gui/src/render/inventory.rs:792`) go through `with_affixes`.

- [ ] **Step 9: Replace `affix_of` with `affixes_of` and fold `copy_bonus`.**
  `copy_bonus` sums every resolvable affix's `stats` onto the base
  **before** the four scaling axes — the ordering comment already there
  is still correct and must survive. With at most one affix in play this
  is behaviour-identical, which is what Step 11 checks.

- [ ] **Step 10: Rename `EquipSpec::affix` to `affixes: Vec<AffixId>`.**
  Outright, no shim — nothing in `dev-arenas/` authors it, and a silently
  ignored scenario field is a known trap here. Update
  `dev-arenas/README.md`'s schema section and `assets/affixes/README.md`
  where it describes what a copy carries.

- [ ] **Step 11: Write the behaviour-unchanged test.**
  In `tests/equipment.rs`: a copy built with one affix through
  `with_affixes` produces exactly the `copy_bonus` a pre-change
  single-affix copy did, and `copy_name` produces the identical string.
  This is the task's own claim — that nothing moved — made checkable.

- [ ] **Step 12: `cargo fmt`, `cargo clippy --workspace`, then
  `cargo test --workspace`.** Every pre-existing test must pass
  unedited except where it names the field. If a test's *assertion*
  needed changing, stop and report it — that is behaviour moving, which
  this task forbids.

- [ ] **Step 13: Commit.** `refactor(items): a gear copy carries a sorted list of affixes`

---

### Task 2: Fuse across quality and affixes

**Files:**
- Modify: `crates/engine/src/game/crafting.rs` — `fuse_copy` (~line 766)
  and its doc comment; a new private partner search beside it.
- Modify: `crates/engine/src/tuning.rs` — only if a constant is genuinely
  needed; the formula uses the existing `QUALITY_STEP`.
- Test: `crates/engine/src/tests/equipment.rs`

**Interfaces:**
- Consumes: `GearCopy::fusable_with`, `GearCopy::with_affixes` (Task 1).
- Produces: no new public API. `Game::fuse_item` and
  `Game::fuse_all_items` keep their signatures.

- [ ] **Step 1: Read the spec's "Fusion" section**, including the
  numbered sequence and the total-order tie-break, and the "Decisions
  taken" table.

- [ ] **Step 2: Write the failing tests.** Six, each named for its claim:
  - two copies differing **only** in quality fuse, and the result's
    quality is the average snapped down. Drive a table of pairs through
    it including both tie cases: `(75, 90) -> 80`, `(85, 85) -> 85`,
    `(90, 95) -> 90`, `(70, 130) -> 100`.
  - two copies carrying **different** affixes fuse, and the result holds
    both, sorted.
  - two copies carrying the **same** affix fuse, and the result holds it
    **twice** — the duplicate case, and the one the feature was asked
    for.
  - the partner chosen is the highest-quality eligible copy. Stand up
    **three** eligible spares at different qualities, or "it picked the
    only other one" passes and proves nothing.
  - a worn copy that is eligible but not equal is folded in, and the
    player is still wearing the result afterwards — assert on
    `Equipment`, not on a log line.
  - `fuse_all_items` pairs across quality and affixes in one pass and
    does not cascade: four tier-0 copies at mixed qualities come out as
    two tier-1 copies, not one tier-2.

  Existing tests that must stay green **unedited**:
  `a_rare_copy_will_not_fuse_with_a_plain_one` (rarity is still matched)
  and `fusing_all_pairs_promotes_every_stack_once`.

- [ ] **Step 3: Run them and confirm each fails**, and confirm the two
  named existing tests still pass. `cargo test -p feral-processes-engine fuse`

- [ ] **Step 4: Implement the partner search** as a private method on
  `Game`. It gathers eligible cargo copies plus the worn copy in that
  slot if eligible, and returns the best under a **total** order:
  highest quality, then cargo before worn, then fewest affixes, then the
  affix ids. The order must be total because `GearCopies::copies` is an
  insertion-ordered `Vec` whose order is play-history dependent — say so
  in the doc comment.

- [ ] **Step 5: Rewrite `fuse_copy`'s body** to the spec's six numbered
  points. Keep both existing refusals **above** the first `take_copies`,
  so a refused fusion still spends nothing. Reword the insufficient-stock
  refusal to count the whole eligible group; today's wording counts exact
  matches and would now be a lie.

  The quality average, ties down, in integers over the existing
  `QUALITY_STEP`:

```rust
let q = (a as u32 + b as u32 + QUALITY_STEP as u32 - 1)
    / (2 * QUALITY_STEP as u32)
    * QUALITY_STEP as u32;
```

- [ ] **Step 6: Rewrite `fuse_copy`'s and `fuse_item`'s doc comments.**
  The long paragraph asserting "the two copies must match on every
  property, rare tier included" is now false and must go; what replaces
  it is the rarity half of the argument, which still stands, plus the
  survivor/partner rule. Do not leave a "was" or "used to" note.

- [ ] **Step 7: Run the new tests and the two named existing ones.**
  All green.

- [ ] **Step 8: Mutation-prove each new test.** For each, break the thing
  it claims — round the average up, drop the partner's affixes, sort the
  partner search by insertion order — and confirm the matching test fails
  and no other test catches it first. Restore, and record the table.

- [ ] **Step 9: `cargo fmt`, `cargo clippy --workspace`,
  `cargo test --workspace`.**

- [ ] **Step 10: Commit.** `feat(items): fusion crosses quality and stacks affixes`

---

### Task 3: Name a multi-affix copy

**Files:**
- Modify: `crates/engine/src/game/combat_rewards.rs:184` — `copy_name`.
- Modify: `crates/app-core/src/lib.rs:360` — `SWAP_NAME_COLUMN` and the
  doc comment above it. (Path: `crates/app-core/src/lib.rs`.)
- Test: `crates/app-core/src/tests/inventory.rs`
  (`no_shipped_copy_name_outgrows_the_swap_name_column`, ~line 658);
  `crates/gui/src/render/inventory.rs`
  (`the_widest_swap_row_still_fits_its_popup`, ~line 951).

**Interfaces:**
- Consumes: `Game::affixes_of` (Task 1).
- Produces: `Game::copy_name` unchanged in signature.

- [ ] **Step 1: Read the spec's "Naming" section.**

- [ ] **Step 2: Extend the width census first, and watch it fail.**
  `no_shipped_copy_name_outgrows_the_swap_name_column` currently sweeps
  one affix at a time. Extend it to sweep the **longest prefix and
  longest suffix together** on the longest item name at the widest rare
  tier and an off-spec quality, with the affix count at its ceiling.
  That ceiling is `ITEM_FUSION_COST` to the power of `MAX_FUSIONS` — 8 —
  so `+7` is the widest marker reachable. Read both constants from
  `tuning`, do not hardcode 8.

- [ ] **Step 3: Run it and confirm it fails** against the current 57-cell
  column.

- [ ] **Step 4: Implement the naming rule.** Over the resolvable affixes
  in the copy's (sorted) order, take the first with a `prefix` and the
  first with a `suffix`, decorate as today, then append `+N` for the
  resolvable affixes not named. **Omit `+N` at zero** — so a copy with
  one prefix and one suffix names both and gains nothing, and no name in
  any existing save moves. The quality figure stays last.

- [ ] **Step 5: Raise `SWAP_NAME_COLUMN`** to whatever the census now
  measures, and rewrite its doc comment: it currently names
  "Overclocked Singularity Matrix of Quiet Handshakes (130%)" as the
  worst case, which is no longer true.

- [ ] **Step 6: Run both censuses** — the app-core character-count one
  and the gui `the_widest_swap_row_still_fits_its_popup`, which measures
  real text and is the authority.

- [ ] **Step 7: Mutation-prove.** Remove the `+N` from the name and
  confirm a test asserting on a three-affix copy's name fails.

- [ ] **Step 8: `cargo fmt`, `cargo clippy --workspace`,
  `cargo test --workspace`.**

- [ ] **Step 9: Commit.** `feat(items): a multi-affix copy names two and counts the rest`

---

### Task 4: Show the affixes on the inspect page

Without this the player cannot see what `+3` bought them, and an affix
may be a trade-off carrying negative stats — a hidden drawback they can
never account for.

**Files:**
- Modify: `crates/engine/src/views.rs:167` — `GearDetailView` gains
  `affixes: Vec<String>`.
- Modify: `crates/engine/src/game/catalog.rs:402` — `gear_detail` fills
  it; a private helper folds duplicates and sorts.
- Modify: `crates/gui/src/render/inventory.rs:394` — `gear_inspect_rows`
  draws the block.
- Test: `crates/gui/src/render/inventory.rs` (the census module around
  line 690–760); `crates/engine/src/tests/gear_detail.rs`.

**Interfaces:**
- Consumes: `Game::affixes_of` (Task 1).
- Produces: `views::GearDetailView::affixes: Vec<String>` — one entry per
  distinct affix, duplicates folded as `of Static ×3`, trade-offs first.

- [ ] **Step 1: Read the spec's "The inspect page" section.** Note in
  particular that the page has **no scroll** and that the decision is to
  bound the *drawing*, not the storage.

- [ ] **Step 2: Write the failing engine tests** in `tests/gear_detail.rs`:
  - a copy carrying the same affix three times yields one entry reading
    `×3`;
  - a copy carrying a trade-off affix (any negative component in its
    `stats`) and an ordinary one lists the trade-off **first**;
  - an id the build cannot resolve contributes no entry, and the rest of
    the list still appears.

- [ ] **Step 3: Run them and confirm they fail.**

- [ ] **Step 4: Implement the view half** — the field, and the private
  helper in `catalog.rs` that resolves, folds, orders and formats. Format
  in the engine, not the renderer, for the reason `copy_name` is the
  engine's job.

- [ ] **Step 5: Draw the block** in `gear_inspect_rows`, after the
  quality/accuracy block and before `effects`. Cap it by what fits, in
  `cap_entries`' idiom, ending in `+N more` — trade-offs sort first
  precisely so an overflow can never hide a drawback.

- [ ] **Step 6: Extend `the_tallest_gear_page_fits_its_popup`.** It
  currently sweeps `GearCopy::plain(def.id)`, which has no affixes and so
  measures nothing about this block. Give it the worst case: the tallest
  item's page carrying a full affix block. Watch it fail before the cap
  is in, then pass with it.

- [ ] **Step 7: Add the width census**, `no_gear_row_overflows_its_popup`,
  in the shape of `no_memory_row_overflows_its_popup` in
  `crates/gui/src/render/party.rs:1089` — `draw_row` clips vertically and
  never horizontally, so a long affix line is lost in silence. Measure
  real text through `with_painter`.

- [ ] **Step 8: Mutation-prove.** Remove the cap and confirm the height
  census fails; remove the trade-off-first sort and confirm the ordering
  test fails.

- [ ] **Step 9: `cargo fmt`, `cargo clippy --workspace`,
  `cargo test --workspace`.**

- [ ] **Step 10: Commit.** `feat(gui): the gear page lists what a copy's affixes do`

---

### Task 5: The player-facing text

**Files:**
- Modify: `assets/help/70-supplies.md:28–31` — the fusion paragraph.
- Test: the help censuses already in the suite (page parsing, link
  resolution, the easter-egg census). No new test.

**Interfaces:** none.

- [ ] **Step 1: Rewrite the fusion paragraph.** Three things it now says
  wrongly or not at all: that two copies must be "of the same piece"
  without qualifying that quality and affixes may differ; that "if you
  are wearing one of the two, it is the copy that survives", which is no
  longer how the survivor is chosen; and nothing at all about quality
  averaging or affixes stacking. Rarity still being the one thing that
  must match is worth a clause — it is the refusal a player will hit.

- [ ] **Step 2: Check the surrounding pages for claims this falsifies.**
  `rg -n -i 'fus' assets/help/` — `40-getting-stronger.md:14` and
  `90-before-you-breach.md:42` both mention fusing and may be fine;
  read them rather than assuming.

- [ ] **Step 3: `cargo test --workspace`** — the help directory is
  parsed and censused by the suite, so a malformed page fails there.

- [ ] **Step 4: Commit.** `docs(help): fusion crosses quality and stacks affixes`

---

### Task 6: Measure the balance move

Not optional, and not a test. Affix stats are added to the **base**
before all four scaling axes, so eight of them on a tier-3 copy compound
through level, quality, fusion tier and rarity together. `balance_sim`
models no fusion and will not see any of it, so a green suite is not
evidence here.

**Files:**
- Create: `docs/measurements/2026-08-24-stacked-affix-power.md`
- Read first: `docs/measurements/README.md` for the convention and the
  bar.

**Interfaces:** none.

- [ ] **Step 1: Read `dev-arenas/README.md`** for the scenario schema,
  and note that `EquipSpec` now takes `affixes:` (Task 1, Step 10).

- [ ] **Step 2: Author three scenarios** off one shipped baseline,
  identical but for the equipped copy: no affix, one affix, four affixes
  (the same affix four times is the cleanest signal, since it isolates
  the stacking from which affixes were picked).

- [ ] **Step 3: Run each** with `cargo run --bin arena -- <file> --out
  <report>.ron`. Compare **within this one build only** — arena absolutes
  are not comparable across builds, because any RNG-stream shift moves
  the baseline. Deltas are the readable figure.

- [ ] **Step 4: Write the measurement up** — the commands, the numbers,
  and what the run was blind to (it models one authored fight, not a run;
  it does not model how often a player actually accumulates four
  affixes).

- [ ] **Step 5: Report the finding to the user with a recommendation.**
  If four stacked affixes swing the fight more than a rare tier does,
  say so plainly — the spec's decision was "no cap", and revisiting it is
  the user's call, not yours.

- [ ] **Step 6: Commit.** `docs(measurements): what stacked affixes are worth`

---

## Final gate

- [ ] `cargo test --workspace` green from a clean state.
- [ ] `cargo clippy --workspace` with no warnings.
- [ ] `rg -n 'affix_of|\.affix\b' --type rust crates/` returns nothing
      but the save shim's load-only fields.
- [ ] The mutation table for every new test, collected across tasks, is
      in the final report.
- [ ] Whole-branch code review before any merge — per-task gates are
      optional here, the final one is not, and it must see the mutation
      table.
