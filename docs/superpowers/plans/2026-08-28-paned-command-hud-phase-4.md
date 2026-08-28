# Paned Command HUD — Phase 4: the column shell and the attention model

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One engine derivation of "what needs the player right now", and the
three surfaces that read it — the status bar's badge, the info column's tab
markers, and the two collapsed bars — so that a closed pane cannot hide an
actionable state.

**Architecture:** `Game::attention` is the single derivation and lives in the
engine, because it is a claim about game state. It returns rows carrying a
`kind`, so the renderer's row→tab mapping is an exhaustive match rather than a
guess off a key character. app-core gains one field (`info_tab`) and the keys
`1`/`2`/`3`. gui gains `hud::column`, which frames the column, draws the tab
row and the two collapsed bars, and **returns the open pane's body rect** —
which phase 4 hands straight to today's `draw_status_panel` and phase 5
replaces with real BASE/CREW/PACK contents.

**Tech Stack:** Rust. All three workspace crates: `feral-processes-engine`,
`feral-processes-app-core`, `feral-processes-gui`. No schema change, no save
field, no `SAVE_FORMAT_VERSION` bump — `info_tab` is UI state, exactly as
`log_filter` is.

**Spec:** `docs/superpowers/specs/2026-08-27-paned-command-hud-design.md`
**Predecessors:** phase 1 (`2026-08-27-paned-command-hud-phase-1.md`, v0.13.37),
phase 2 (`2026-08-27-paned-command-hud-phase-2.md`, v0.13.38), phase 3 (landed
inline, v0.13.40).

**Branch:** `feat/hud-info-column`, already created off `main` at fe3f3626.

---

## Global Constraints

Phases 1–3's constraints all still apply — read phase 1's Global Constraints
section. In addition:

- **The palette is addressed by role.** `palette::ATTENTION` is br yellow and
  means *the player must act*; `palette::THREAT` is br red and means
  *hostility or inbound harm*; `palette::HEALTHY` is the calm `ALL NOMINAL`.
  Never a raw colour, and never one of those two for decoration.
- **Three consumers, one call.** `draw_playing_base` calls `game.attention()`
  **once** and hands the same slice to the status bar and the column. A second
  derivation anywhere is the bug this phase exists to make impossible, and
  `attention_drives_all_three_markers` is what says so.
- **The column does not scroll**, the same as the gear inspect and memories
  pages. Its height is therefore a layout constraint, and a row past the
  bottom is dropped in silence unless a census catches it.
- **`draw_status_panel` is not deleted this phase.** Phase 5 deletes it. Phase
  4 shrinks the rect it is handed and takes its two chrome calls away, because
  the column now draws the frame.
- **Every string a strip or a row draws is measured, never clipped.**
  `strip::fitting` is the one rule for what does not fit — dropped from the
  end, counted. `Painter` clips vertically and never horizontally.
- **`1`/`2`/`3` must work underground.** The spec names this as binding in two
  dispatches; in this codebase `handle_playing_key`'s top match runs **before**
  the `handle_stack_key` hand-off, so one binding there covers both — the same
  place `f` (log filter) sits. The *property* is what matters and the test
  asserts it from a Stack locale, not the placement.

### Two spec corrections, both settled before writing this plan

1. **There is no `pack full` row.** The player's pack has no capacity:
   `components::Inventory` is an unbounded `Vec<(ItemId, u32)>`, and
   `PlayerStatus::inventory_used`'s own doc says "The Buffer is unbounded, so
   this is just how much is stored." That row is the same class of invention as
   the raid countdown the spec already struck out. It is replaced by **roster
   at capacity**, off `pet_count`/`pet_capacity`, which are real and bounded and
   which the player can act on (sell, decompile, stand a program down).
2. **The spec's key column is the handoff's, not this game's.** `k` walks west
   and `m` opens the Excavation plan; neither opens what the row is about.
   Perks live behind the party menu (`PARTY_ROWS`' `Perks` row), and both
   structure rows land behind the base menu. The chips name the **top-level map
   key that starts the journey**.

The table this phase builds:

| Condition | Text | Key | Threat |
|---|---|---|---|
| a structure below full `Durability` | `<name> damaged` | `b` | yes |
| idle workable structures | `N nodes without a program` | `b` | no |
| unspent perk points | `N perk points unspent` | `p` | no |
| roster at capacity | `roster full (n/n)` | `p` | no |

**Threat rows sort first**, which is the third deviation and the smallest: the
badge shows the most urgent row, and a raid eating the base reading second to
"3 perk points unspent" is wrong on a HUD. Within a group the order is the
table's.

---

### Task 1: `Game::attention` — the one derivation

**Files:**
- Modify: `crates/engine/src/views.rs` — add `AttentionKind`, `AttentionRow`,
  and `StructureReport::is_idle`
- Modify: `crates/engine/src/game/inspection.rs` — add `Game::attention`
  beside `structure_report`
- Modify: `crates/gui/src/render/building.rs:908` — `structure_is_idle` becomes
  a call to `StructureReport::is_idle`
- Create: `crates/engine/src/tests/attention.rs`
- Modify: `crates/engine/src/tests/mod.rs` — `mod attention;`, alphabetically
  after `mod assets;`

**Interfaces:**
- Consumes: `Game::structure_report`, `Game::player_status`, `Game::pet_count`,
  `Game::pet_capacity`. All exist; none is new.
- Produces:

```rust
/// Which condition an `AttentionRow` reports. Carried so a frontend can sort
/// a row into a pane without matching on its prose or its keycap — an
/// exhaustive match here is `cell_mark`'s rule, and a `_ =>` arm is how a new
/// condition ships invisible.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AttentionKind {
    StructureDamaged,
    IdleStructures,
    PerkPoints,
    RosterFull,
}

/// One thing that needs the player right now — see `Game::attention`.
#[derive(Clone, Debug, PartialEq)]
pub struct AttentionRow {
    pub kind: AttentionKind,
    /// Player-facing, built here and never in a renderer.
    pub text: String,
    /// The map key that opens the screen this is acted on from.
    pub key: char,
    /// br red rather than br yellow: hostility or inbound harm, never an
    /// ordinary error.
    pub threat: bool,
}

impl StructureReport {
    pub fn is_idle(&self) -> bool;
}

impl Game {
    /// What needs the player right now, most urgent first.
    pub fn attention(&mut self) -> Vec<AttentionRow>;
}
```

`views::*` is re-exported wholesale from `lib.rs:93`, so both new types are
reachable from app-core and gui with no further edit.

**Row derivation, exactly:**

- `StructureDamaged` — the **first** structure in `structure_report()` order
  whose `durability` is `Some((cur, max))` with `cur < max`. `structure_report`
  is already sorted Home first, then by def id, then nearest, so "first" is
  stable across runs and needs no second sort. One row however many are
  damaged; the name is that structure's `label`.
- `IdleStructures` — `structure_report().iter().filter(|s| s.is_idle()).count()`,
  omitted at zero. Text `"{n} nodes without a program"`, singular
  `"1 node without a program"`.
- `PerkPoints` — `player_status().perk_points`, omitted at zero. Text
  `"{n} perk points unspent"`, singular `"1 perk point unspent"`.
- `RosterFull` — `pet_count() >= pet_capacity()` **and** `pet_capacity() > 0`.
  Text `"roster full ({count}/{capacity})"`.

Take `structure_report()` **once** and derive both structure rows off that one
Vec — it walks every structure and resolves a def per row, and this is called
every frame.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/attention.rs`. `support::stand_in_base` and
`support::from_inside_the_base` are how a test reaches base space; read
`crates/engine/src/tests/building.rs` for how that file stands structures up
before writing a fixture of your own.

- `a_calm_base_needs_nothing` — a fresh `Game::new(7, Forgiving, ...)` with the
  player's perk points spent and no base: `attention()` is empty. This is the
  row the `ALL NOMINAL` state is read off, so it has to be reachable.
- `an_idle_node_asks_for_a_program` — stand a workable structure up with nobody
  posted; assert exactly one `IdleStructures` row, its `key` is `'b'`, its
  `threat` is false, and its text names the count.
- `unspent_perk_points_ask_to_be_spent` — grant a level so `perk_points > 0`;
  assert a `PerkPoints` row keyed `'p'`. Assert it is **absent** at zero in the
  same test, since the omission is the half that regresses silently.
- `a_full_roster_says_so` — fill to `pet_capacity()`; assert a `RosterFull` row
  whose text carries both figures.
- `a_damaged_structure_is_the_threat_row` — damage a structure's `Durability`
  below max; assert one `StructureDamaged` row, `threat: true`, naming that
  structure's label.
- `a_threat_sorts_above_everything_else` — a base holding a damaged structure
  **and** idle nodes **and** unspent perk points: `attention()[0].kind ==
  StructureDamaged`. This is the deviation from the spec's table order and is
  the one thing about the ordering worth pinning.
- `one_row_however_many_are_damaged` — two damaged structures, one row.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine attention`

- [ ] **Step 3: Implement `AttentionKind`, `AttentionRow`, `is_idle` and `attention`**

- [ ] **Step 4: Point gui's `structure_is_idle` at the engine's**

`crates/gui/src/render/building.rs:908` currently spells the predicate out.
Make its body `s.is_idle()` and keep the doc comment — per `CLAUDE.md` a claim
that two places share a rule has to be a call, and here the two places are in
different crates, where nothing fails to compile when one drifts.

- [ ] **Step 5: Run them and watch them pass**

`cargo test -p feral-processes-engine attention` then
`cargo test -p feral-processes-gui building`

- [ ] **Step 6: Commit**

`git commit -m "feat(hud): what needs the player, derived once"`

---

### Task 2: `App::info_tab` and the keys `1` `2` `3`

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `InfoTab` beside `LogFilter` (~line
  637), and the field on `App` (~line 1471, beside `log_filter`)
- Modify: `crates/app-core/src/app/lifecycle.rs:38` — the initial value
- Modify: `crates/app-core/src/app/playing.rs` — the bindings, in
  `handle_playing_key`'s top match beside `GameKey::Char('f')`
- Create: `crates/app-core/src/tests/info_tab.rs`
- Modify: `crates/app-core/src/tests/mod.rs` — `mod info_tab;`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:

```rust
/// Which pane of the HUD's info column is open. UI state, like `LogFilter`:
/// not saved, not part of any run.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum InfoTab {
    #[default]
    Base,
    Crew,
    Pack,
}

impl InfoTab {
    /// Every tab, in the order the column draws them and the digits select
    /// them. `1` is `ALL[0]`.
    pub const ALL: [InfoTab; 3] = [InfoTab::Base, InfoTab::Crew, InfoTab::Pack];
    /// What the tab row calls it — `BASE`, `CREW`, `PACK`.
    pub fn label(self) -> &'static str;
}

// on App:
pub info_tab: InfoTab,
```

`LogFilter` at `lib.rs:637` is the model to follow down to the `ALL` constant
and the `label`, including the reason its cycle order and its header order are
held together by a test.

**The bindings** go in `handle_playing_key`'s top match, next to `f`, and
`return` rather than fall through — switching which pane you are reading is not
an action and must not cost a turn. That match runs before the
`is_underground()` hand-off to `handle_stack_key`, so one arm covers both
locales; say so in a comment there, because the spec asks for two bindings and
the next reader will look for the second one.

- [ ] **Step 1: Write the failing tests**

In `crates/app-core/src/tests/info_tab.rs`. `crates/app-core/src/tests/log_filter.rs`
is the fixture idiom — read it first.

- `the_column_opens_on_base` — a fresh `App` with a game: `info_tab` is
  `InfoTab::Base`.
- `the_digits_pick_a_pane` — `2` gives `Crew`, `3` gives `Pack`, `1` gives
  `Base`, and pressing the same digit twice is idempotent.
- `a_digit_costs_no_turn` — read `current_tick()`, press `2`, read it again:
  unchanged. The whole reason these arms `return`.
- `the_digits_work_underground` — **the load-bearing one.** Take the game into
  the Stack (`crates/app-core/src/tests/stack.rs` shows how), press `3`, assert
  `info_tab == InfoTab::Pack` **and** `status_line.is_none()`. A key bound in
  only one dispatch falls through `handle_stack_key`'s `_ => {}` as a swallowed
  keypress with no refusal and nothing in the log — which is exactly how `r`
  shipped broken underground.
- `the_tab_order_is_the_digit_order` — `InfoTab::ALL[i]` is what digit `i + 1`
  selects, walked over all three. `LogFilter`'s
  `the_header_order_is_the_cycle_order` is the same test for the same reason:
  the row the renderer draws and the key that picks it must not disagree.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-app-core info_tab`

- [ ] **Step 3: Implement `InfoTab`, the field and the bindings**

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Prove the underground test is not vacuous**

Move the three arms out of the top match and into the surface-only block below
the `handle_stack_key` hand-off. Confirm `the_digits_work_underground` fails
and the other four still pass. Restore. This repo has shipped two vacuous tests
that read as coverage, and this is the one assertion here that could be one.

- [ ] **Step 6: Commit**

`git commit -m "feat(hud): the info column's three panes, and the digits that pick them"`

---

### Task 3: the attention badge on the status bar

**Files:**
- Modify: `crates/gui/src/render/hud/status_bar.rs` — the reserved right zone,
  `StatusBarState`, and a `badge_pieces` derivation with its tests

**Interfaces:**
- Consumes: `feral_processes_engine::{AttentionRow, AttentionKind}` from Task 1;
  `strip::{Piece, label, value, sep, fitting}` and `palette` from phase 2/3.
- Produces:

```rust
// added to StatusBarState<'a>:
pub attention: &'a [AttentionRow],

/// The badge's pieces: the most urgent row and its keycap, with a `+N` when
/// more than one condition holds — or `ALL NOMINAL` in `palette::HEALTHY`
/// when none does.
fn badge_pieces(attention: &[AttentionRow]) -> Vec<strip::Piece>;
```

The module's own doc comment already says this zone "is reserved for the
attention badge and is deliberately empty until the attention model exists".
Delete that sentence as you fill it, and delete the matching clause about
adding the badge later not re-laying the bar out — it does not, and the census
below is what says so.

**Copy**, from the handoff: `4 NODES IDLE [b]` — the row's `text` upper-cased,
then its `key` in a `[...]` keycap. The count of *other* conditions rides as a
dim ` +2` after the keycap; at one condition there is no suffix. Calm is the
single piece `ALL NOMINAL` in `palette::HEALTHY`.

**Colour:** `palette::THREAT` when the leading row's `threat` is set, else
`palette::ATTENTION`. The keycap is `palette::EMPHASIS` either way — a keycap
is a keycap, and running it in the row's colour makes the reservation
decorative.

**Placement:** right-aligned inside `at.w * BADGE_FRAC`, so the identity block
and the stock strip do not move when the badge appears. What does not fit is
dropped from the end through `strip::fitting`, never clipped.

- [ ] **Step 1: Write the failing tests**

In `status_bar.rs`'s existing `#[cfg(test)] mod tests`.

- `a_calm_base_reads_all_nominal` — `badge_pieces(&[])` is one piece, the text
  `ALL NOMINAL`, coloured `palette::HEALTHY`.
- `the_badge_names_the_first_row_and_its_key` — one `IdleStructures` row gives
  pieces whose joined text is `4 NODES IDLE [b]` and no `+N`.
- `a_second_condition_is_counted_not_listed` — three rows give exactly one
  upper-cased text and a trailing `+2`.
- `a_threat_badge_is_red` — a row with `threat: true` colours its text piece
  `palette::THREAT`; the same row with `threat: false` colours it
  `palette::ATTENTION`. Both halves in one test, because either alone passes
  against a constant.
- `the_badge_stays_inside_its_zone` — draw the bar through `with_painter` at
  1280x720 with four rows, and assert every painted text's right edge is inside
  `at.x + at.w - m.inset`. `crates/gui/src/render/hud/log_frame.rs`'s keybar
  census is the idiom for measuring a strip against its region.
- `the_badge_does_not_move_the_stock_strip` — the x of the first stock pile is
  the same with an empty `attention` and with four rows. This is the claim the
  module's doc comment has been making since phase 1 with nothing checking it.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-gui status_bar`

- [ ] **Step 3: Implement `badge_pieces` and draw it**

Every existing caller of `draw_status_bar` gains `attention: &[]` until Task 5
wires the real one — there is exactly one, in `base.rs`, plus whatever the
tests build.

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Commit**

`git commit -m "feat(hud): the status bar says what needs you, or that nothing does"`

---

### Task 4: `hud::column` — the shell, the tabs and the collapsed bars

**Files:**
- Create: `crates/gui/src/render/hud/column.rs`
- Modify: `crates/gui/src/render/hud/mod.rs` — `pub(super) mod column;`,
  alphabetically after `bar`
- Test: inline `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `feral_processes_app_core::InfoTab` (Task 2),
  `feral_processes_engine::{AttentionRow, AttentionKind}` (Task 1),
  `hud::{palette, strip}`.
- Produces:

```rust
/// What the column draws around whatever is in its open pane.
pub(in crate::render) struct ColumnState<'a> {
    pub tab: InfoTab,
    pub attention: &'a [AttentionRow],
}

/// Frames the column, draws the tab row and the two collapsed bars, and
/// returns the **body rect** of the open pane — what phase 5 fills and what
/// phase 4 hands to `draw_status_panel`.
pub(in crate::render) fn draw_info_column(
    at: Rect,
    state: &ColumnState,
    painter: &Painter,
    m: &Metrics,
) -> Rect;

/// Which pane a condition belongs to. Exhaustive on `AttentionKind` — a
/// `_ =>` arm is how a new condition ships with no marker anywhere.
fn tab_of(kind: AttentionKind) -> InfoTab;
```

`tab_of`: `StructureDamaged` and `IdleStructures` → `Base`; `PerkPoints` and
`RosterFull` → `Crew`; nothing routes to `Pack` yet, and that is a fact about
the four conditions rather than a gap — the pack has no capacity and so has
nothing to ask for.

**Layout**, following the handoff's column table (tab row 0, active pane, two
collapsed bars at rows 33 and 35):

- The tab row is the column's **first body row**, drawn inside the frame — not
  a border strip. It is three cells, `1 BASE` `2 CREW` `3 PACK`, each preceded
  by its digit in `palette::EMPHASIS`. The open tab's label is
  `palette::PANE_TITLE` and bold; a closed one is `palette::LABEL`.
- The two collapsed bars are pinned to the **bottom** of the column, one line
  each with a hairline `palette::DIVIDER` rule above the pair.
- The body rect is what is left between them, minus one inset either way. It is
  the return value; nothing else computes it.

**Markers**, the whole point of the tabbed column: a tab reads `!` in
`palette::ATTENTION` when any row routes to it, and `·` in `palette::LABEL`
otherwise. The handoff reserves a cyan `·` for "merely notable" — there is no
notable-but-not-actionable condition in the model yet, so the dim `·` is the
calm marker and the cyan one is not drawn. Do not invent a third state to fill
it.

**Collapsed bar copy:** the tab's leading row's `text`, prefixed `! `, in
`palette::ATTENTION` (or `palette::THREAT` when that row is a threat). With
nothing routing to it, `· nominal` in `palette::FAINT`. Phase 5 replaces the
calm half with the live summary the handoff specifies (crew pips, pack count);
the `!` half is already correct and does not move.

- [ ] **Step 1: Write the failing tests**

- `the_open_tab_is_the_one_the_state_names` — with `tab: Crew`, the painted
  runs carry `CREW` in `palette::PANE_TITLE` bold and `BASE` in
  `palette::LABEL`. `crate::paint`'s styled-run reader (the one documented
  above `painted_text`) is what reads a colour+weight back out.
- `a_closed_tab_still_wears_its_marker` — with `tab: Crew` and one
  `IdleStructures` row, a `!` is painted in `palette::ATTENTION` **and** the
  collapsed bar carries `4 nodes without a program`. This is the sentence the
  whole design rests on: a closed pane cannot hide an actionable state.
- `a_calm_tab_reads_nominal` — empty `attention`: no `!` anywhere, and both
  collapsed bars read `· nominal`.
- `a_threat_colours_its_tab_and_its_bar` — a `StructureDamaged` row paints the
  BASE marker and its collapsed bar in `palette::THREAT`, not
  `palette::ATTENTION`.
- `the_body_rect_clears_the_tab_row_and_the_bars` — the returned rect's top is
  below the tab row's baseline and its bottom is above the first collapsed bar,
  at 1280x720 and at 1920x1080. This is the rect phase 5 lays five blocks into;
  wrong by a line here and every phase-5 census is measuring the wrong box.
- `the_column_frames_after_it_fills` — walk `paint_order`: the fill rect lands
  before any text. Phase 2's `the_map_frame_draws_after_the_map` is the same
  assertion one level up.
- `every_tab_is_reachable_from_a_kind` — for each `AttentionKind`, `tab_of`
  returns something; walked over an explicit list of all four, so adding a
  fifth kind without a home fails to compile at the match and fails here if
  someone reaches for a `_` arm.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-gui hud::column`

- [ ] **Step 3: Implement `draw_info_column`**

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Commit**

`git commit -m "feat(hud): the info column wears three tabs and hides nothing behind them"`

---

### Task 5: wire the column in, and the two censuses

**Files:**
- Modify: `crates/gui/src/render/base.rs` — `draw_playing_base` (~line 616) and
  `draw_status_panel` (~line 1414)
- Modify: `crates/gui/src/render/hud/palette.rs` — the doc comment's "a full
  pack" example, and the phase-4 line in the `dead_code` note
- Test: `base.rs`'s existing `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: everything from Tasks 1–4.
- Produces: nothing new. This is the wiring task.

**The wiring:**

- `draw_playing_base` calls `game.attention()` **once**, beside `base_stock()`
  and `player_status()`, and hands the same slice to `draw_status_bar` and
  `draw_info_column`.
- `draw_info_column(regions.info_column, …)` returns the body rect;
  `draw_status_panel` is called with **that** rect instead of
  `regions.info_column`.
- `draw_status_panel` loses its opening `painter.rect(...)` and
  `painter.rect_lines(...)` — the column draws the fill and the frame now, and
  leaving them in draws a second border inside the first. Everything below
  those two lines is untouched; phase 5 deletes the function whole.
- The column is drawn **before** `draw_status_panel`, for `border_strip`'s
  reason: the fill has to land before the text that sits on it.

**`palette.rs` housekeeping:** the module doc's ATTENTION example reads "An
idle structure, an unspent perk point, a full pack" — the third is the row that
does not exist. Make it "a full roster". And the `dead_code` note says "phase 4
ATTENTION and THREAT"; both are consumed now, so strike that clause. Leave the
attribute itself — phases 5 and 6 still owe it.

- [ ] **Step 1: Write the failing tests**

- `attention_drives_all_three_markers` — **the census the spec names.** Build a
  base holding one actionable condition, draw `draw_playing_base` through
  `with_painter`, and assert all three surfaces agree in one pass: the status
  bar painted the badge text, a `!` was painted in `palette::ATTENTION`, and the
  collapsed bar carried the row's own words. Then clear the condition and assert
  all three go quiet together — `ALL NOMINAL` painted, no `!` anywhere. Both
  halves, because either alone passes against a surface that is drawing a
  constant.
- `the_tallest_column_pane_fits_its_column` — at 1280x720, the smallest
  supported window: the number of rows `draw_status_panel` would draw for a
  fully-developed party fits the body rect Task 4 returned. Build the worst
  case out of a full party plus a full pet roster — `MAX_PARTY_SIZE` and
  `pet_capacity` are the two figures that bound it — and compare against
  `(body.h / m.line_height)`. The column has no scroll, so a row past the
  bottom is dropped in silence; this is `the_tallest_gear_page_fits_its_popup`'s
  trap in a taller box. **If it does not fit, that is the answer to the spec's
  first open question and belongs in the phase-5 plan, not in a scrollbar** —
  record the measured overflow in the commit message and leave the census
  asserting the real figure.
- `the_playing_screen_still_draws_one_status_panel` — exactly one border is
  painted around the column, not two. Cheap, and it is the one thing the
  chrome-line deletion in `draw_status_panel` could get wrong.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-gui base`

- [ ] **Step 3: Wire it up**

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Prove `attention_drives_all_three_markers` is not vacuous**

Make `draw_info_column` ignore its `attention` slice and draw `·` on every tab
unconditionally. Confirm the census fails. Restore. A census that passes with
one of its three surfaces disconnected is measuring two surfaces and claiming
three, which is precisely the failure "one derivation, three consumers" exists
to prevent.

- [ ] **Step 6: Full gate**

`cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expect 3478 tests plus this phase's additions, all green. `balance_sim` is
untouched by this phase — nothing here reads `tuning.rs` — so a curve moving is
a signal something unintended landed, not a retune.

- [ ] **Step 7: Commit**

`git commit -m "feat(hud): the column, the badge and the tabs read one derivation"`

---

### Task 6: the documentation the change falsifies

**Files:**
- Modify: `CHANGELOG.md` — a new `## X.Y.Z` section at the merge, per the
  versioning rule. **On the branch this stays unwritten**; it is the merge that
  bumps. Note it here so the deploy step is not a surprise.
- Modify: `docs/superpowers/specs/2026-08-27-paned-command-hud-design.md` —
  Part 3's table, and a fourth bullet under "What the handoff gets wrong"
- Modify: `docs/seams.md` and `CLAUDE.md` — one entry for the attention model

**The spec edits**, in the spec's own voice:

- Part 3's table becomes the four rows this plan builds, with the real keys.
- "What the handoff gets wrong about this game" gains: **There is no pack
  capacity.** `components::Inventory` is an unbounded `Vec` and
  `PlayerStatus::inventory_used` says so in its own doc comment. `pack full` is
  not derivable and is replaced by the roster's capacity, which is real.
- Part 3 gains the sort rule: threat rows first, then the table's order.
- The first open question ("whether the BASE tab's five blocks fit") is
  **not** answered by this phase — phase 5 answers it. Leave it.

**The seam entry**, under a new "### The HUD" heading in `docs/seams.md` with
the argument, and one or two lines in `CLAUDE.md` under Load-bearing seams with
the rule and the trap:

> **`Game::attention` is the one derivation of what needs the player, and
> three surfaces read the same call.** The badge, the tab markers and the two
> collapsed bars are handed one `Vec` by `draw_playing_base`; a second
> derivation is what makes "a closed pane cannot hide an actionable state" a
> coincidence instead of a construction. `AttentionRow::kind` exists so the
> renderer sorts a row into a pane by an **exhaustive match** rather than off
> its keycap. **The trap is the pack**: the spec's fourth row was `pack full`
> and there is no pack capacity — `Inventory` is an unbounded `Vec` — so the
> row is the *roster's* capacity, and anyone "restoring" the pack row is
> inventing a limit the simulation does not have.

Per `CLAUDE.md`: `docs/manual.md` and the root `README.md` are carved out of
the doc obligation, and `TODO.md` is the user's own. Do not touch any of the
three.

- [ ] **Step 1: Edit the spec**

- [ ] **Step 2: Write the seam entry in `docs/seams.md`, then the short form in `CLAUDE.md`**

`CLAUDE.md` is a gitignored twin of `AGENTS.md` — edit `CLAUDE.md`, then
`cp CLAUDE.md AGENTS.md`. Nothing tracks the drift between them.

- [ ] **Step 3: Commit**

`git commit -m "docs(hud): the attention model, and the pack limit that does not exist"`

---

## Self-review

**Spec coverage.** Part 3 (the attention model) is Task 1 plus the badge in
Task 3 and the markers in Task 4. Part 4's shell — three tabs, one open, two
collapsed, `App` gaining exactly one field, the keys in both dispatches — is
Tasks 2 and 4. Part 4's *contents* and the deletion of `draw_status_panel` are
phase 5 and are deliberately absent. Part 6's `attention_drives_all_three_markers`
and `the_tallest_column_pane_fits_its_column` are Task 5;
`every_border_strip_fits_its_pane` and `the_keybar_fits_the_log_pane` landed in
phases 2 and 3. Part 7's `ATTENTION` and `THREAT` are consumed here, which is
what the palette's own `dead_code` note predicted.

**Not covered, and why.** The handoff's cyan `·` "merely notable" marker has no
condition to drive it — recorded in Task 4 rather than invented. The log's
blinking cursor and actionable-line tint are phase 5's, being contents.

**Type consistency.** `AttentionRow`/`AttentionKind` are defined in Task 1 and
used unchanged in Tasks 3, 4 and 5. `InfoTab` is defined in Task 2 and consumed
in Task 4. `draw_info_column` returns the `Rect` Task 5 passes to
`draw_status_panel`. `StructureReport::is_idle` is defined in Task 1 and is the
body of gui's `structure_is_idle` in the same task.
