# Base output: what used it — plan

Spec: `docs/superpowers/specs/2026-11-05-base-output-used-design.md` (read it
first; its "Builder-check answers" settle the edge cases).
Branch/worktree: `worktree-base-output-used`. Gates off between phases; one
final opus review of the whole branch. Commit at each green phase. Never
push, stash, or `git checkout` anything.

Seams to respect (`.claude/rules/seams-instrumentation.md`): every production
seam reports through `base_ledger::emit`; the fold is unconditional;
`Game::base_output_report` is the page's one derivation;
`BASE_OUTPUT_MAX_ROWS` is a layout constraint.

## Phase 1 — engine: the ledger records why (TDD)

Files: `crates/engine/src/base_ledger.rs`, `game/telemetry.rs`,
`game/base/stock.rs`, `game/base/construction.rs`,
`game/base/research_supply.rs`, `systems.rs` (fuel burn ~1285),
`crates/engine/src/tests/base_ledger.rs`.

Interfaces:
- `ConsumeSource` derives `PartialOrd, Ord, Hash, Serialize, Deserialize`
  (serde names = variant names; they become save keys).
- `pub enum UsedBy { Assembled(ItemId), Source(ConsumeSource) }` — derives
  `Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize`.
- `ItemTotals` (lose `Copy` if needed) gains
  `#[serde(default)] pub used_by: BTreeMap<UsedBy, u32>`.
- `Event::Consume { item, qty, source: ConsumeSource }`.
- `fold`: the `Assemble` input loop adds to `used_by[Assembled(product)]`;
  the `Consume` arm to `used_by[Source(source)]`. Factor the repeated
  "consumed += / bucket += " into one private helper that also writes the
  tally, so the two arms cannot drift.

Tests (unit, in `base_ledger.rs`'s module):
- an `Assemble` attributes each input to its product;
- a `Consume` attributes to its source;
- after a mixed sequence, `used_by` values sum to `consumed` per item;
- an `ItemTotals` RON with no `used_by` field parses (mirror
  `a_ledger_saved_before_visits_existed_loads`).
Integration (`tests/base_ledger.rs`): extend
`what_the_base_produced_survives_a_real_save_and_load` (line 31) or add a
sibling that asserts `used_by` survives `save`→`Game::load`.

Gate: `cargo test -p feral-processes-engine base_ledger`; clippy clean.
Commit.

## Phase 2 — engine: the view

Files: `crates/engine/src/views/base.rs` (`BaseOutputRow` ~579),
`crates/engine/src/game/inspection.rs` (`base_output_report` ~1688),
`crates/engine/src/tests/base_ledger.rs`.

Interfaces:
- `BaseOutputRow` gains `pub used: u32` (= `totals.consumed`) and
  `pub used_by: Vec<(String, u32)>`, sorted qty desc then label asc.
- Labels resolved in the engine: `Assembled(p)` → `"into {ItemDb name of p,
  else id}"`; `Source(s)` → one `fn used_by_label(ConsumeSource) -> &'static
  str` (player words, plain language: e.g. fuel, built, crew jobs, crafted,
  refit, installed, lost in breach, sabotaged, defence, healing — final
  wording is the implementer's; no tick/occult vocabulary; each distinct).
  Put it beside `ConsumeSource::as_str` or in `views/base.rs`; it is the
  player-facing counterpart, not a replacement for the telemetry string.

Tests: `base_output_report` returns `used` and `used_by` ordered largest
first, with an assembler input named by its product (follow the fixtures at
`tests/base_ledger.rs:540-651`).

Gate: `cargo test -p feral-processes-engine base_ledger`. Commit.

## Phase 3 — gui: draw it

Files: `crates/gui/src/render/building/structure_sheet.rs`
(`base_output_rows` 359, `base_output_line` ~421),
`crates/gui/src/render/building/tests/base_output.rs`, and every other
`BaseOutputRow { .. }` literal in gui tests (compiler finds them).
Possibly `crates/engine/src/tuning/staff.rs` (`BASE_OUTPUT_MAX_ROWS`).

Behaviour:
- Header gains a right-aligned `used` column after `run`; the line gains
  `row.used` in the same width.
- After an item row with `used > 0`, push one `Row::TextColored(.., TEXT_DIM)`
  indented under the name: `"    ← "` + entries `"{label} {qty}"` joined
  `" · "`. Cut to the content-rect width with a trailing `…`, never wrap —
  measure the budget the way the existing widest-line test does.
- `used == 0` → no indented line.

Tests:
- a row with usage draws the `used` figure and its indented line in order;
  a row without draws neither;
- an over-long breakdown ends in `…` and fits the content rect (extend the
  existing widest-line test);
- `tallest_base_output_page` gives **every** row `used: 999_999` and a long
  `used_by` (≥6 entries with the longest labels), then
  `the_tallest_base_output_page_fits_its_content_rect` decides
  `BASE_OUTPUT_MAX_ROWS`: lower it (5 → largest that fits, expected 3) in
  `tuning/staff.rs`, updating its doc comment's reasoning.

Gate: `cargo test -p feral-processes-gui base_output` (check the crate name
in `crates/gui/Cargo.toml`). Commit.

## Phase 4 — finish

- `cargo fmt`; `cargo clippy --workspace --all-targets` clean.
- `cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES`), and
  `cargo test -p feral-processes-engine balance_sim` (expect no move).
- Screenshot check: `cargo run -- --template <a dev-save with an assembler
  chain> --keys "<open b, walk to Production>" --screenshot out.png` and
  Read the PNG; pick the template from `dev-saves/README`.
- Seam doc: the instrumentation rule "a source is recorded and never folded;
  a sink is both" now also folds *why* — update
  `.claude/rules/seams-instrumentation.md`, the `seams` skill and
  `seam:` graph entry per the `seams` skill's three-write order if the
  rule's wording no longer holds.
- Final whole-branch review (opus), diff given as a file. Then stop for
  deploy; `CHANGELOG.md` is written at release, not on the branch.
