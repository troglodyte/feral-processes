# Manifest Redesign Implementation Plan

> **For agentic workers:** use superpowers:subagent-driven-development (or
> superpowers:executing-plans). Steps are `- [ ]` checkboxes. TDD each task;
> commit per green step.

**Spec:** `docs/superpowers/specs/2026-10-11-manifest-redesign-design.md`.
Read it first; it is the contract for every colour, glyph, row and key. This
plan gives the order, the seams and the tests. It does not restate the spec.

**Branch / worktree:** `claude/design-system-manifest-redesign-2d16d2` at
`.claude/worktrees/design-system-manifest-redesign-2d16d2`.

## Global constraints

- The renderer never touches `World`. Every per-row derivation (rail order,
  call to act, ranks) is an engine call. `seams-screens.md`.
- `render/` draws only through `Painter` (`.claude/rules/drawing-seam.md`).
  Metrics stay `text::Metrics`.
- `Game::attention` is the one derivation of what needs the player
  (`seams-hud.md`). `CallToAct` is computed by the same per-program function,
  not beside it.
- No save-format change, no asset schema change. `StructureManifest` and the
  `ExchangeView` model are untouched.
- Reuse, don't copy: `hud::bar::bar`, `hud::panes::chip`, `hud::strip::fitting`,
  `log_frame`'s `INTEG_CELLS`/`POWER_CELLS`/`XP_CELLS` (make them
  `pub(in crate::render)`), `menu_shortcut`, `selected_index`,
  `fitted_stat_row`'s elision rule.
- Actions are UPPERCASE; lowercase letters select rows.
- `cargo fmt` and `cargo clippy --workspace --all-targets` clean after each
  task. Unset `FERAL_DEV_NO_SIEGES` before tests.
- Subagents never push, never `git checkout`/`git stash`/`reset` another's
  work, and never `git add -A`. Stage explicit paths.

## Decisions this plan adds (not in the spec)

1. **A wild subject (`ManifestOrigin::Map`, not in `manifest_subjects`)
   draws no rail and no compare cells.** Its keybar drops group 1, and a
   digit or letter does nothing. This is today's rule that paging is absent
   for a wild subject, carried over.
2. **Ledger placement is data on the section.** `Section` gains
   `column: LedgerColumn { Fight, Growth, Duty }`, set by each builder.
   `SectionRow` gains `rank: Option<RankedStat>`. The draw step looks the
   rank up in one `SubjectRanks` fetched per frame. The builders keep their
   row strings.
3. **Module split** (module-boundaries): `render/manifest.rs` and
   `render/manifest_layout.rs` become `render/manifest/`:
   - `mod.rs`: the entry draw and face dispatch;
   - `sections.rs`: today's builders, unchanged except for the column and
     rank tags;
   - `layout.rs`: frame, rail, band, ledger and band geometry, pure over
     `Metrics`;
   - `rail.rs`, `band.rs`, `ledger.rs`, `keybar.rs`: one drawer each.

   `score.rs`, `social.rs`, `talk.rs` and `dossier.rs` stay where they are
   and call `manifest::{band, rail, keybar}`.

## Review focus (final whole-branch review, opus)

1. **Attention agreement.** A program with banked points, then the same
   program after it spends them. The rail `!`, the band's call to act, and
   `Game::attention`'s `ProgramPoints` count must all flip together.
2. **Rank edge cases.** One-member roster, all-equal values, ties
   (1st, 1st, 3rd), and the player excluded from Potential and WorkAptitude.
3. **Key collisions.** Lowercase `w`, `d` and `r` on the manifest jump to a
   row. Uppercase `W`, `D` and `R` act. Nothing in the dossier's handler
   swallows a digit.
4. **Census at the extremes.** 720×(4 widths) at 40px font: no rail row,
   ledger row or keybar overflows; the bands are never dropped; the rail
   window keeps the selection visible with 40+ programs.
5. **Palette leak.** No `render/mod.rs` colour constant is imported by the
   manifest modules, `score.rs`, `social.rs`, `talk.rs` or `dossier.rs`,
   apart from `glyph_color` and the rarity tier colours.

---

## Phase 1: Engine (one subagent, sonnet)

Files: `crates/engine/src/game/party.rs`, `game/inspection.rs`,
`game/derived.rs`, `views.rs`, plus a pure-function home for ranking. Put it
in `game/inspection.rs` beside `subject_ranks`, or in a new `ranking.rs` if
module-boundaries says so.

### Task 1.1: rail order
- [ ] Tests:
  - you are first;
  - party members follow in `party_slot` order;
  - then the others A–Z, case-insensitive, with `ProgramId` breaking ties;
  - `owned_pets` order is unchanged.
- [ ] Rewrite `manifest_subjects` (party.rs:968). Update its doc comment:
  it no longer shares `owned_pets`' order. It keeps the same membership.

### Task 1.2: `CallToAct` and the attention fold
- [ ] Views: `CallToAct { PerkPoints(u32), DevPoints(u32), Idle }`.
- [ ] Add one per-subject function, `call_to_act(e) -> Option<CallToAct>`,
  in `inspection.rs` next to `attention`.
- [ ] Rewrite `programs_holding_points` (derived.rs:218) as a count over
  that function's `DevPoints`. Delete it if nothing else is left calling it.
- [ ] Tests:
  - the reproducer: banked points are flagged, and the flag clears once
    they are spent;
  - the `ProgramPoints` row count equals the number of `DevPoints` across
    `manifest_subjects`;
  - an idle staff program gets `Idle`;
  - a player with unspent perk points gets `PerkPoints`.
- [ ] Watch `DevPoints`' gate. Today's count needs `hold && points > 0`.
  Decide whether the rail flags points that aren't held, and keep the
  attention count identical to today. State the choice in the commit.

### Task 1.3: `manifest_roster`
- [ ] Views: `RosterRow { entity, name, glyph, color, level, in_party,
  needs_you: Option<CallToAct> }`.
- [ ] Add `Game::manifest_roster()` in party.rs.
- [ ] Test: the rows come back in `manifest_subjects` order, with
  `in_party` and `needs_you` filled.

### Task 1.4: ranks
- [ ] Views: `RankedStat { Damage, Attack, Mitigation, Power, Potential,
  WorkAptitude }` and `SubjectRanks { rows: Vec<RankRow { stat, place, of,
  pips }> }`.
- [ ] Add a pure `rank(values: &[(Entity, f32)], subject) -> Option<RankRow
  parts>`. Unit-test it on hand-built slices:
  - ties share a place;
  - all equal gives `pips = 5`;
  - a single member;
  - the pip formula at min, max and midpoint.
- [ ] Add `Game::subject_ranks(e)`, which gathers the snapshot only:
  - Damage ranks by the mean of its range;
  - Potential and WorkAptitude rank over programs only;
  - it returns `None` when `e` is not in `manifest_subjects`.
- [ ] Integration test: the player and two programs give the expected
  `of` for each stat.

**Phase 1 gate:** `cargo test -p feral-processes-engine`, then clippy.
Commit.

---

## Phase 2: app-core input (one subagent, sonnet)

Files: `crates/app-core/src/lib.rs`, `app/inspection.rs`,
`app/group_menu.rs`, `app/input.rs`, `app/arena.rs` (doc comment only),
`tests/{menus,level_up,party}.rs`. In gui, only `render/mod.rs`'s
`ManifestPick` arm and `manifest_pick_label` are deleted, so the workspace
compiles.

### Task 2.1: retire the picker
- [ ] Rewrite the ~13 `ManifestPick` tests first, so they fail: the
  group-menu row opens `Mode::Manifest` on subject 0 with
  `ManifestOrigin::Menu`, and Esc runs `close_screen`.
- [ ] Remove `Mode::ManifestPick` and `handle_manifest_pick_key`. Rename
  `ManifestOrigin::Picker` to `Menu`. Point the group-menu target at
  `Manifest`, setting `pending_manifest`.
- [ ] Remove the dispatch and mode-pair entries in `input.rs`, and the gui
  arm and its label function.
- [ ] Fix the `arena.rs` doc reference.

### Task 2.2: keys
- [ ] Tests:
  - ↑ and ↓ page the same way ← and → do;
  - a digit or lowercase letter jumps through `selected_index` and keeps
    the face via `fit_manifest_tab`;
  - `W` watches, and `w` jumps to row `w`;
  - rename `lowercase_d_does_nothing_on_a_manifest` to
    `lowercase_d_jumps_to_row_d`;
  - a wild subject ignores digits and letters;
  - in the dossier, ↑/↓ page and digits jump while staying in
    `Mode::Dossier`.
- [ ] Factor the paging tail of `handle_manifest_key` into one
  `page_manifest(step)` and one `jump_manifest(idx)`, shared by the
  manifest and dossier handlers.

**Phase 2 gate:** `cargo test -p feral-processes-app-core`, then clippy for
the workspace. Commit.

---

## Phase 3: gui scaffolding (one subagent, opus)

Opus here because this phase fixes the geometry every later phase draws
into.

### Task 3.1: module split, no behaviour change
- [ ] `git mv` the two files into `render/manifest/{mod,sections,layout}.rs`.
  Adjust paths. All existing manifest tests stay green with no edits. Commit
  before going on.

### Task 3.2: palette scope and shared HUD pieces
- [ ] Rewrite the scope comment at `hud/palette.rs:27-29`.
- [ ] Make `INTEG_CELLS`, `POWER_CELLS` and `XP_CELLS`
  `pub(in crate::render)`.
- [ ] Use `panes::chip` for the keybar. Drop `log_frame::keycap` unless its
  `&str` key is needed for `↑/↓` and `Esc`. If it is, expose `keycap` and
  have `chip` call it.

### Task 3.3: geometry (pure, censused)
- [ ] `layout.rs`: delete `best_column_split`, `section_height` (if
  unused) and their tests.
- [ ] Add `ManifestLayout::new(frame: Rect, m: &Metrics, face, rail_rows) ->
  { rail: Option<RailGeom{ full: bool, .. }>, band, vitals: Option<Rect>,
  ledger: [Rect; 3], bands: Rect, keybar: Rect }`.
  - The rule for a full rail is the spec's: STATS, and three 32-cell
    columns still fit.
  - Add a rail window function: `(len, selected, fits) -> (start, end,
    above, below)`, counted like `fitting_rows`.
- [ ] Add a column-fit function: given each section's rows and a column
  height, return the sections kept, the per-section caps, and the
  `+N more` count. The bands are never dropped.
- [ ] Census tests over the existing sweep of 9 heights × 4 widths:
  - a full rail only when three 32-cell columns fit;
  - the rail window always contains the selection (40 subjects);
  - the ledger columns are equal width, `space-pad × 2` apart.

**Phase 3 gate:** gui tests and clippy. Commit.

---

## Phase 4: STATS face (one subagent, sonnet; spec open beside it)

### Task 4.1: tag the builders
- [ ] `sections.rs`: each `player_sections`/`program_sections` push gets
  its `LedgerColumn`, following the spec's two tables.
- [ ] Ranked rows carry `rank`.
- [ ] Add the new rows: Next level, Cargo carried, the PARTY band rows and
  the LATELY band. Pull them from the existing views where they exist. If
  the view lacks a field, stop and report rather than reaching into another
  call.
- [ ] Tests: the column assignment for the player and for a program; the
  ranked rows are exactly the spec's set.

### Task 4.2: draw
- [ ] Each drawer reads a `RosterRow`, `CallToAct` or `SubjectRanks` slice
  and a `Rect`:
  - `rail.rs`: full and collapsed;
  - `band.rs`: the summary lines, face tabs, call to act, and the vitals
    line via `bar::bar`;
  - `ledger.rs`: the label, value and compare cell, with the elision order
    "compare cell, then label";
  - `keybar.rs`: the groups via `strip::fitting`.
- [ ] Remove `draw_bar`, `meter_rows`, `footer_text`, `draw_header`'s box
  path and the `render/mod.rs` colour imports from the manifest modules.
- [ ] Re-point the width and height censuses (~60 tests) at the new layout:
  - no ledger or rail row overflows its column;
  - the keybar fits by dropping groups;
  - the `L` greying rule is unchanged.

**Phase 4 gate:** gui tests and clippy. Then take screenshots:
- `--template stack` at 1440×900 and 1280×720 for the player's STATS;
- one program's STATS.

Read each PNG against the design-system cards. Report, don't tune by eye
past the spec. Commit.

---

## Phase 5: other faces and the dossier (one subagent, sonnet)

These can run in parallel with Phase 4 only if they touch none of its files.
They call `rail`, `band` and `keybar`, so run them after Phase 4.

- [ ] **Task 5.1, `score.rs`:** two ledgers in the `FIRST_BOX_LINES` split,
  plus the TOTAL band. Re-point its census.
- [ ] **Task 5.2, `social.rs`:**
  - RELATIONSHIPS uses the opinion ruler clamped to
    `BOND_ENEMY_AT..=BOND_CLOSE_AT`, with the ruler logic shared with
    `ledger.rs`'s pip ruler;
  - KNOWN FOR and TEMPERAMENT go in the narrow column;
  - the empty-state lines stay verbatim.
- [ ] **Task 5.3, `talk.rs`:** a 12-cell speaker column, a hanging indent
  via `text::wrap`, and divider rules between exchanges.
- [ ] **Task 5.4, `dossier.rs`:**
  - it moves from the Large popup into the manifest frame, band and
    collapsed rail, with a `D DOSSIER` tag;
  - PROVENANCE wraps at `DESCRIBE_WRAP_COLUMNS`, and ATTRIBUTES is a table;
  - add a dossier-fits-frame census across the sweep.
- [ ] For each: palette imports go to `hud::palette` only. Keep the
  `seams-screens` row-count ownership (app-core counts, gui draws).

**Phase 5 gate:** `cargo test --workspace` and clippy for the workspace.
Commit.

---

## Phase 6: close (controller)

- [ ] Final whole-branch review (opus) against the Review focus above. Give
  it the diff as a file.
- [ ] Fix the findings. Review each fix (a fix needs its own review).
- [ ] `CHANGELOG.md` at landing (the deploy skill). Set the INDEX row to
  built.
