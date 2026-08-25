# Item Power and the Caravan Basket — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every gear copy one absolute power scalar shown on list rows
and broken down on the inspect page, group the visiting caravan's two lists
by category, and replace its per-item quantity page with one arrow-driven
basket committed by Enter.

**Architecture:** Three independent deliverables riding one branch. The
power scalar is a pure engine derivation from `Game::copy_bonus`, priced by
*calling* `Stats::power` and `battle::hit_chance` rather than restating
them. Presentation reaches every screen at once through the existing
`with_tag` seam. The caravan basket is modelled on `app/basket.rs`, with one
engine commit door that sells before it buys.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine only), bevy + bevy_egui (gui).
4-crate workspace: `engine` → `app-core` → `gui` → `launcher`.

**Spec:** `docs/superpowers/specs/2026-08-25-trade-screen-power-and-basket-design.md`

## Global Constraints

Read the spec before Phase 1 and re-read the relevant section at the top of
each phase. These apply to every task:

- **`CLAUDE.md` governs.** Read it. It is loaded every turn for a reason;
  the seams it names are load-bearing and each has its argument in
  `docs/seams.md` under the same title.
- **TDD, no exceptions.** Failing test first, watch it fail, minimal
  implementation, watch it pass, commit. A test that passes with the fix
  removed is not coverage — delete the fix and watch it fail.
- **`cargo test --workspace` is the final gate** for each phase.
  `cargo clippy --workspace` and `cargo fmt` after every task; fix
  warnings rather than silencing them.
- **`cargo test -p feral-processes-engine balance_sim` is a required gate
  for any change to `tuning.rs`** (Phase 1). A moved curve means
  progression changed — that is the signal, not a broken test.
- **No `SAVE_FORMAT_VERSION` bump.** Nothing here is stored. `ItemPower` is
  derived on every read; the basket amounts live on `App`, not in the save.
- **No new asset schema.** The reference block is tuning, and tuning is code.
- **Commits are free; pushing is not.** Do not push. Do not merge. Do not
  tag. The version bump and changelog section happen once, at the merge,
  and are not this plan's business.
- **Branch:** `feat/item-power-and-caravan-basket`. Confirm with
  `git branch --show-current` before **every** commit — a concurrent session
  has fast-forwarded and deleted a branch mid-task in this repo before.
- **Stage explicit paths, never `git add -A`.** Another agent's worktree
  gitlink under `.claude/worktrees/` gets swept up otherwise.

---

# Phase 1 — The power scalar (engine only)

No UI. Deliverable: `Game::copy_power` and its reference block, gated by
`cargo test -p feral-processes-engine`. A subagent can do this phase with
no knowledge of Phases 2-4.

**Spec sections:** 1 and 2.

### Task 1: The reference block in `tuning.rs`

**Files:**
- Modify: `crates/engine/src/tuning.rs` — a new labelled section
- Test: `crates/engine/src/tuning.rs` (its own `mod tests`, if one exists;
  otherwise the constants are exercised by Task 2's tests)

**Interfaces:**
- Consumes: nothing
- Produces:
  - `pub const POWER_REFERENCE_MAX_HP: i32`
  - `pub const POWER_REFERENCE_ATK: i32`
  - `pub const POWER_REFERENCE_MITIGATION: i32`
  - `pub const POWER_REFERENCE_DAMAGE: crate::battle::DamageRange`
  - `pub const POWER_REFERENCE_LEVEL: u32`
  - `pub const POWER_REFERENCE_ZONE: u32`

**Choosing the numbers is the task**, not a detail of it. Derive them, do
not invent them: read `PLAYER_BASE_STATS` and `stats_after_levels` and pick
a mid-run player — a reference far from where players actually are makes
every power figure in the game wrong in the same direction, which is hard to
notice and easy to ship. Document in the section header which level and zone
the block corresponds to and how it was derived, so a later retune can move
it deliberately.

`POWER_REFERENCE_MITIGATION` must be strictly below `MAX_MITIGATION_PERCENT`
— `Stats::power` divides by `1 - mitigation/100`, and the whole point of the
clamp is keeping that denominator off zero.

- [ ] **Step 1:** Read `crates/engine/src/tuning.rs`'s existing section
      structure and `PLAYER_BASE_STATS` / `stats_after_levels`. Pick the
      reference level and zone and write down the derivation.
- [ ] **Step 2:** Add the section with the six constants and the derivation
      comment.
- [ ] **Step 3:** `cargo test -p feral-processes-engine balance_sim` —
      expected: PASS unchanged. These constants feed nothing yet, so any
      movement here means something else was touched.
- [ ] **Step 4:** `cargo clippy --workspace && cargo fmt`
- [ ] **Step 5:** Commit `crates/engine/src/tuning.rs`.

---

### Task 2: `ItemPower` and `Game::copy_power`

**Files:**
- Modify: `crates/engine/src/views.rs` — add `ItemPower`
- Create: `crates/engine/src/game/gear_power.rs` — the derivation
- Modify: `crates/engine/src/game/mod.rs` — declare the module
- Test: in `crates/engine/src/game/gear_power.rs`'s own `mod tests`

**Interfaces:**
- Consumes: Task 1's six constants.
- Produces:
  - `pub struct views::ItemPower { pub total: i32, pub offense: i32, pub survivability: i32, pub accuracy: i32, pub evasion: i32 }`
  - `impl Game { pub fn copy_power(&self, copy: &items::GearCopy) -> Option<views::ItemPower> }`

Its own module rather than beside `copy_bonus` in `game/crafting.rs`: this
is a distinct derivation with a breakdown type and a reference block, and
`crafting.rs` is already large. It must **call** `Game::copy_bonus` — never
re-walk affixes, quality or fusion tier. `copy_bonus` is the one expression
for what gear is worth, and four screens have already silently dropped the
affix by rebuilding its chain by hand.

**The four terms.** This is the one place in the plan where the formula is
spelled out, because it is the genuinely non-obvious part:

```
let mods = self.copy_bonus(copy, POWER_REFERENCE_LEVEL)?;

// Reference wearer, and the same wearer with the piece on.
let bare  = Stats { max_hp: REF_MAX_HP, atk: REF_ATK, mitigation: REF_MIT, .. };
let worn  = Stats { atk: bare.atk + mods.atk,
                    mitigation: bare.mitigation + mods.mitigation, ..bare };

// atk + mitigation, in Stats::power's own currency.
let stat_delta = worn.power() - bare.power();

// A weapon REPLACES the natural band; it does not add to it.
let band_delta = if mods.damage != DamageRange::default() {
    (mods.damage.mean() - POWER_REFERENCE_DAMAGE.mean()).round() as i32
} else { 0 };

// Accuracy and evasion are PROPORTIONAL. A probability is not a quantity
// and must never be summed into the total.
let foe = balance_sim::median_ordinary_species(db) at POWER_REFERENCE_ZONE;
let acc_delta = offense_term * (hit_chance(ref_acc + mods.accuracy, foe_eva)
                              / hit_chance(ref_acc, foe_eva) - 1.0);
let eva_delta = soak_term    * (hit_chance(foe_acc, ref_eva)
                              / hit_chance(foe_acc, ref_eva + mods.evasion) - 1.0);
```

`offense` is `stat_delta`'s attack half plus `band_delta`; `survivability`
is its soak half. `decompiler` gets **no term** — it buys taming, not
combat. Return `None` when all five axes are zero.

- [ ] **Step 1: Write the failing tests.** Six, each naming one property:
      - an atk-only copy rates its atk;
      - a mitigation-only copy rates **more** than its raw number (it is
        priced as the effective HP it buys, not summed as a percentage);
      - a weapon whose band is *below* the reference band rates its offense
        term **negative** — this is the test that fails if the band is added
        rather than substituted, and it is the whole reason the term exists;
      - an accuracy-only copy rates above zero and scales with the reference
        offense term, not with the raw accuracy number;
      - an evasion-only copy likewise on the soak term;
      - a decompiler-only copy rates `None`.
- [ ] **Step 2:** Run them. Expected: FAIL, `copy_power` not found.
- [ ] **Step 3:** Add `views::ItemPower`, create `game/gear_power.rs`,
      declare it in `game/mod.rs`, implement.
- [ ] **Step 4:** Run them. Expected: PASS.
- [ ] **Step 5: Mutation-prove each term.** Delete one term at a time and
      confirm the matching test — and only that test — goes red. Record the
      table. A term nothing catches is a term that can be deleted later by
      accident.
- [ ] **Step 6:** `cargo test -p feral-processes-engine && cargo clippy
      --workspace && cargo fmt`
- [ ] **Step 7:** Commit.

---

### Task 3: The census over the real assets

**Files:**
- Modify: `crates/engine/tests/assets.rs`

**Interfaces:**
- Consumes: `Game::copy_power`, `views::ItemPower`
- Produces: nothing consumed later.

Two censuses, run against the shipped item set rather than a fixture,
because what this asserts is a property of the assets *and* of the formula:

1. **Every shipped equipment item rates `Some`.** A weapon, armour or module
   that rates `None` is one whose whole bonus sits on axes the formula does
   not price — that is a formula gap, and it must fail the build rather than
   draw an em dash in the shipping game.
2. **Every shipped non-equipment item rates `None`.** Consumables,
   materials and currency have no combat axis; a `Some` here means the
   formula is reading something it should not.

- [ ] **Step 1:** Write both censuses. Expected: they may legitimately fail
      on a real asset — if so, that is a finding about the formula or the
      item, and it goes to the user before either is changed.
- [ ] **Step 2:** Run `cargo test -p feral-processes-engine --test assets`.
- [ ] **Step 3:** `cargo test --workspace` — the Phase 1 gate.
- [ ] **Step 4:** Commit.

---

# Phase 2 — The power column and the inspect breakdown

**Spec section:** 3. Depends on Phase 1's `Game::copy_power`.

### Task 4: The third column on a tagged row

**Files:**
- Modify: `crates/gui/src/render/popup.rs` — `ItemTag`, `with_tag`,
  `item_text`, `tagged_text`, `tag_pieces`, `draw_row`
- Modify: every `with_tag` call site — `render/trade.rs`,
  `render/caravan.rs`, `render/inventory.rs`, `render/stack_market.rs`,
  `render/crafting.rs` and any others `rg 'with_tag'` finds
- Test: `crates/gui/src/render/popup.rs`'s `mod tests`

**Interfaces:**
- Consumes: `Game::copy_power`
- Produces:
  - `pub(super) enum PowerCell { Blank, Unrated, Rated(i32) }`
  - `with_tag` gains a fifth parameter, `power: PowerCell`

**A fifth parameter and not a defaulted builder**, deliberately: every call
site must be made to decide, and the compiler is what makes them. This is
the same move `Row::Item`'s tag column itself made.

**It must sit between the category tag and the name**, so it inherits the
fixed-width `row_lead` and the figures form a straight edge down the list.
That edge is the entire feature. **It must not be the `suffix` field** —
`suffix_x` places a suffix one inset past each row's *own* right edge, so
the numbers would stagger with the name lengths above them.

Three cells, three meanings, and they are not interchangeable:
- `Rated(n)` — a combat rating.
- `Unrated` — an em dash. This item has no combat axis (a Decompiler
  module, a consumable). There is no answer, not a bad answer.
- `Blank` — this row is not an item at all (the wagon's Routine and Program
  offers). A dash here would claim the disk was rated and found wanting.

- [ ] **Step 1: Write the failing tests.**
      - `a_tagged_rows_pieces_join_back_into_its_text` still holds with the
        third piece — extend it, do not replace it. A row measured from one
        set of pieces and drawn from another is a row whose suffix lands on
        its own tail.
      - The three `PowerCell` variants each draw their own glyph, asserted
        on the row's **pieces**, never on a substring of the joined text: a
        substring test passes against a renderer that formatted the column
        into the middle of a string and left no span to paint.
      - Two rows with different name lengths put their power cells at the
        **same x**. This is the alignment property, and nothing else asserts
        it.
      - **The swap picker's column and its delta may disagree, and that is
        correct.** Pin it: on `Mode::EquipSwap`, a candidate whose absolute
        power exceeds the worn piece's can still show a *negative*
        `stat_summary` delta, because gear locks in `EquippedItem::level`
        and the two are scaled at different levels. The column is a property
        of the **copy**; the delta is a property of the **swap**. Without a
        test saying so in as many words, the next reader "fixes" it by
        making the column contextual — which undoes the absolute-rating
        decision and gives one copy two numbers on two screens.
- [ ] **Step 2:** Run. Expected: FAIL to compile (`PowerCell` not found).
- [ ] **Step 3:** Implement `PowerCell`, widen `with_tag`, update every call
      site. Compiler-driven: `cargo check --workspace` lists them all.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: Width censuses.** `no_caravan_row_overflows_its_popup` and
      any sibling width census must be re-run and will likely need the new
      column budgeted. **If a census now fails, that is the census working**
      — the column is a real width spend on rows that are already tight (the
      map's status column holds 38.5 monospace cells and the widest shipped
      buff row spends all but 3.8). Narrow the column or shorten the cell;
      do not raise the census's bound without saying so.
- [ ] **Step 6:** `cargo test --workspace && cargo clippy --workspace &&
      cargo fmt`
- [ ] **Step 7:** Commit.

---

### Task 5: The breakdown on the inspect page

**Files:**
- Modify: `crates/engine/src/views.rs` — `WornDetailView` gains the power
- Modify: `crates/engine/src/game/catalog.rs` — `gear_detail` / `worn_detail`
- Modify: `crates/gui/src/render/inventory.rs` — the only renderer that
  draws `GearDetailView`
- Test: `crates/gui/src/render/inventory.rs`'s `mod tests`, and
  `crates/app-core/src/tests/gear_inspect.rs` for the view field

**Interfaces:**
- Consumes: `views::ItemPower`, `Game::copy_power`
- Produces: `WornDetailView` gains `pub power: Option<views::ItemPower>`

On the **worn** half deliberately, matching `quality`: only equipment rates,
so a consumable's page has nothing to state rather than a defaulted zero.

Carried on the view rather than left to the renderer to call `copy_power`
itself — `GearDetailView`'s stated promise is that the page is one call, and
a renderer reaching past it for one figure is how all four hand-rolled
`copy_bonus` chains started.

- [ ] **Step 1: Write the failing test.** `the_tallest_gear_page_fits_its_popup`
      extended to include the breakdown block. **That page has no scroll** —
      `draw_popup` pages a `Row::Item` span and this page has none, so a row
      past the bottom is dropped in silence. This census is the only thing
      that says it fits.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Add the field, populate it in `worn_detail`, draw the
      block: the total, then one line per contributing axis, omitting axes
      that contributed nothing.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** `cargo test --workspace && cargo clippy --workspace &&
      cargo fmt`
- [ ] **Step 6:** Commit.

---

# Phase 3 — Grouping the wagon (bug 10)

**Spec section:** 4. Independent of Phases 2 and 4; depends on neither.

### Task 6: Sort the offers, head both lists

**Files:**
- Modify: `crates/engine/src/game/caravan.rs` — sort in `caravan_shelf`'s
  tail or in `caravan_view`
- Modify: `crates/gui/src/render/caravan.rs` — `caravan_page_rows`
- Test: `crates/engine/src/game/caravan.rs` tests and
  `crates/gui/src/render/caravan.rs`'s `mod tests`

**Interfaces:**
- Consumes: nothing from earlier phases.
- Produces: nothing consumed later.

Sell rows already arrive category-sorted through `player_status().inventory`.
Offers do not — `caravan_shelf` re-reads its weights per row, so the drawn
order is shuffled by construction. **Sort the offers; head both lists.**

The group key is over `CaravanOfferKind`, **not `ItemCategory`**, because
two of the four kinds are not items: `Gear(copy)` heads under that item's
category, `Material(item)` under `MAT`, and `Routine` and `Program` each
under their own. **Exhaustive on the kind**, `cell_mark`'s rule — as a `_ =>`
arm a fifth kind ships into an unlabelled run and nothing fails to compile.

Two invariants, both already verified in source, that the tests must pin:

- **`CaravanOffer::index` survives the sort.** It is assigned by
  `.enumerate()` before display, `caravan_spent` keys on it, and
  `buy_caravan_offer` resolves by `find(|o| o.index == index)`. Sorting the
  `Vec` moves rows on screen and must move no shelf identity.
- **Headers are `Row::TextColored`.** They must never touch the `idx` that
  `caravan_row` resolves — that counter increments only on item rows.

- [ ] **Step 1: Write the failing tests.**
      - A shelf holding all four kinds comes back grouped, with each kind
        contiguous.
      - **Buying the row drawn last buys the shelf slot it names**, not the
        slot that happens to sit at that position. Build the fixture so the
        sorted order differs from the drawn order — if it does not, the test
        passes against no fix at all.
      - `caravan_row` maps a picked row to the right section with headers
        present.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** Re-run `the_caravan_pages_chrome_leaves_room_for_a_list`.
      Headers are chrome inside the list; if the page now shows too few rows
      at the smallest window, that is the census working.
- [ ] **Step 6:** `cargo test --workspace && cargo clippy --workspace &&
      cargo fmt`
- [ ] **Step 7:** Commit.

---

# Phase 4 — The caravan basket (bug 12)

**Spec section:** 5. Depends on Phase 3 only for merge order, not for
behaviour. The largest phase; do not start it in the same context as
Phases 1-3.

### Task 7: The engine commit door

**Files:**
- Modify: `crates/engine/src/game/caravan.rs`
- Test: `crates/engine/src/game/caravan.rs`'s `mod tests`

**Interfaces:**
- Consumes: nothing from earlier phases.
- Produces:
  - `impl Game { pub fn commit_caravan_basket(&mut self, sells: Vec<(items::GearCopy, u32)>, buys: Vec<usize>) -> Result<String, String> }`

`buys` are **shelf indices** (`CaravanOffer::index`), not row positions.

**Two ordering rules, and both are the whole point of the function:**

1. **Every refusal lands before anything is spent.** `buy_caravan_offer`
   already holds this and states why: a purchase that took the Credits and
   then failed is the one bug the player cannot undo, and a caravan has no
   buyback to put it right with. A basket makes this stricter, not looser —
   a partially committed basket is the same bug wearing a bigger coat.
2. **Sells land before buys.** `transfer_items`' take-before-give rule.
   Here it is what lets a basket be funded by its own sales, which is the
   entire reason the two sections are one basket. The other order clamps
   the buy to zero *silently*.

**One tick for the whole commit**, not one per line. This is a deliberate
economy change recorded in the spec — the basket is the visit. `close_if_gone`
still runs after it: the tick spent may be the one the trader leaves on.

- [ ] **Step 1: Write the failing tests.**
      - **A basket funded by its own sales commits whole.** Start the player
        below the purchase price, with cargo whose sale covers it. This is
        the test that fails if the order is reversed, and it is the reason
        the function exists.
      - An unaffordable basket refuses and spends **nothing** — no cargo
        gone, no Credits gone, no shelf slot spent.
      - A committed buy marks its shelf index in `CaravanMemory`.
      - The commit costs exactly one tick regardless of line count.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement, reusing `sell_to_caravan` and
      `buy_caravan_offer`'s existing refusal logic rather than restating it.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: Mutation-prove the ordering.** Swap sells and buys and
      confirm the funding test goes red. If it stays green the test is
      vacuous and the fixture's starting Credits are too high.
- [ ] **Step 6:** `cargo test -p feral-processes-engine && cargo clippy
      --workspace && cargo fmt`
- [ ] **Step 7:** Commit.

---

### Task 8: Basket state and the key table

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `App` fields, remove
  `Mode::CaravanQuantity` and `pending_caravan_sale`
- Modify: `crates/app-core/src/app/caravan.rs` — the key table
- Modify: `crates/app-core/src/app/input.rs:130` — the modifier fold
- Delete: `handle_caravan_quantity_key` and its mode arm
- Test: `crates/app-core/src/tests/caravan.rs`

**Interfaces:**
- Consumes: `Game::commit_caravan_basket`
- Produces:
  - `pub caravan_amounts: Vec<u32>` on `App`
  - `pub fn App::caravan_sell_available(&self, row: usize) -> u32`
  - `pub fn App::caravan_budget(&self, row: usize) -> u32`

`pub`, not `pub(crate)`, for the two availability functions: the renderer
draws these same figures, and recomputing them in `gui` would be a second
copy of the rule rather than a call to the one governing the keys. That is
exactly why `App::take_available` is `pub`.

**Amounts are index-aligned and unsigned.** The sign is fixed by which
section a row is in, so there is no signed amount as there is on the
transfer picker. Index alignment is safe because **editing costs no tick**
and neither list can change without one — write that invariant down beside
the field, and clear the amounts on commit.

**The two ceilings differ in shape**, mirroring the transfer picker:
- `caravan_sell_available` — **per row and static**, `held`.
- `caravan_budget` — **one budget shared across the buy rows**:
  `credits + proceeds of pending sells − other pending buys`. Subtracting
  only the *other* rows is what lets the highlighted row be lowered and
  raised while it is being edited; counting itself makes every key a no-op
  the moment the basket reaches the budget.
- Offer rows clamp `0..1`. A shelf slot is spent whole, and
  `CaravanOffer::qty` is part of the price the player was quoted.

**Keys.** Enter commits, clears the amounts, rebuilds the view, and **leaves
the screen open**. Esc leaves. `[N]` clears. `[I]` inspect unchanged.
Left/Right step by one, Shift jumps to an end, Ctrl halves the gap through
`half_way_to` — reuse it, do not copy it; its `div_ceil` on the magnitude is
what makes the step terminate, and rounded down a gap of one gives a step of
zero and the key goes dead.

**`[A]` fills the sell rows only.** On the picker it writes the *take*
ceiling over every row, and the take side is the one with a per-row ceiling;
here that is the sell side. Filling the offer side would spend the entire
purse on one keypress on a screen with no buyback.

**Right increases, Left decreases — not the picker's inversion.** That
inversion is specified for a single row spanning both directions and can be
signed. Here the sign is fixed by section, so inverting would read as a slip.
`left_puts_in_and_right_takes_out` is about `Mode::Transfer` and must stay
untouched.

**The modifier fold.** `Mode::Caravan` joins the one condition at
`app/input.rs:130`. Miss it and the four modified-arrow variants are folded
to bare `Left`/`Right` before the caravan handler sees them, so Shift and
Ctrl silently become plain steps and nothing fails.

- [ ] **Step 1: Write the failing tests.**
      - Setting one sell row does not move another's available figure.
      - Setting one buy row **does** lower the others' budget, and the
        highlighted row can still be lowered and raised at the ceiling.
      - Enter commits and the mode is still `Mode::Caravan` with amounts
        cleared.
      - Shift+Left reaches the row's floor in **one** press — this is the
        test that fails if the modifier fold was not widened, and nothing
        else catches it.
      - `[A]` fills the sell rows and leaves every offer row at zero.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement. Remove `Mode::CaravanQuantity`,
      `pending_caravan_sale` and `handle_caravan_quantity_key` outright —
      no shims, no `// removed` comments.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5:** `cargo test --workspace && cargo clippy --workspace &&
      cargo fmt`
- [ ] **Step 6:** Commit.

---

### Task 9: The wagon draws its amounts

**Files:**
- Modify: `crates/gui/src/render/caravan.rs` — amounts in the rows, new
  key hints; delete `draw_caravan_quantity`
- Modify: `crates/gui/src/render/mod.rs` — remove the
  `Mode::CaravanQuantity` draw arm
- Test: `crates/gui/src/render/caravan.rs`'s `mod tests`

**Interfaces:**
- Consumes: `App::caravan_amounts`, `App::caravan_sell_available`,
  `App::caravan_budget`
- Produces: nothing consumed later.

Draw each row's chosen amount and its ceiling from the app-core calls above
— never recomputed here. Replace the two hint lines: `[S]` is gone, and the
new table needs saying (arrows, Shift, Ctrl, `[A]`, `[N]`, Enter, `[I]`).

The basket total is worth a header line — what this visit will cost or pay,
and what the purse will be after. That is the figure the screen now exists
to show, and without it a player sets six rows blind.

- [ ] **Step 1: Write the failing tests.**
      - `no_caravan_row_overflows_its_popup`, extended: the amount column
        and the widened hint lines are both real width spends on a page
        whose shipped descriptions already ran 350px past the body once.
      - `the_caravan_pages_chrome_leaves_room_for_a_list`: the extra hint
        lines and the total header are chrome, and chrome eats the list.
      - `a_caravan_page_says_a_refusal_exactly_once` still holds.
- [ ] **Step 2:** Run. Expected: FAIL.
- [ ] **Step 3:** Implement; delete `draw_caravan_quantity` and its arm.
- [ ] **Step 4:** Run. Expected: PASS.
- [ ] **Step 5: The refusal census.** `every_screen_draws_a_refusal_exactly_once`
      drives all `Mode`s through `draw` and counts what was painted. Removing
      a `Mode` changes its roster — confirm it still passes and that the
      removed mode is gone from whatever list it enumerates.
- [ ] **Step 6:** `cargo test --workspace` — the final gate.
- [ ] **Step 7:** `cargo clippy --workspace && cargo fmt`
- [ ] **Step 8:** Commit.

---

## After the plan

- **Documentation.** `CHANGELOG.md` gets its section at the **merge**, not
  per commit. `docs/manual.md` and the root `README.md` are carved out of
  the doc obligation — leave them. `assets/*/README.md` are untouched: no
  schema changed.
- **`CLAUDE.md` and `docs/seams.md`.** Three new seams are worth a row each:
  `Game::copy_power` as the one door to a rating (and that it is a *call*
  into `Stats::power` and `hit_chance`, never a copy); `PowerCell`'s three
  meanings; and the caravan basket's sell-before-buy commit order. Write the
  argument in `docs/seams.md` and the rule alone in `CLAUDE.md`. Note that
  `CLAUDE.md` and `AGENTS.md` are gitignored twins with no tracking to catch
  drift — edit `CLAUDE.md`, then `cp` it.
- **Playtest.** A green suite is not evidence of play. The user is remote
  and cannot playtest; do not offer it. Say plainly which parts have had no
  screen time — on this branch that will be all of them.
- **Release.** One release per change landing on `main`: version bump,
  changelog section, annotated tag, at the merge. Not this plan's business.
