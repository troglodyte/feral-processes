# Manifest redesign: roster rail, summary band, three-column ledger

The manifest is the character sheet for you or one of your programs. It is
a two-column grid of boxes, drawn on the popup palette (`render/mod.rs`),
inside a frame that also carries three full-width bars. Values are
right-aligned to the box edge, often 40 or more cells from their labels.
Nothing on the sheet compares its subject with anyone else. The sheet also
uses a different green, yellow and cyan from the HUD it sits over.

This change redraws every face of the manifest and the dossier in the HUD
palette:

- a roster rail on the left, which replaces the "Read whose manifest?"
  picker;
- a summary band, with vitals on one line and the one thing to act on;
- a three-column ledger, with values beside their labels and a rank
  against the roster.

The visual target is the **Manifest proposal** group in the feral-processes
design system (https://claude.ai/artifact/E2bgGHgeY1VBxjTREzwaFa). The
cards there are `ProposalPlayerSheet`, `ProposalProgramSheet`,
`ProposalPlayerSheet720`, `ProposalScoreFace`, `ProposalSocialFace`,
`ProposalTalkFace` and `ProposalDossier`. Their sample values are
illustrations; this spec is the contract.

## Decisions

| Question | Decision |
|---|---|
| Palette | Every manifest face and the dossier draw from `hud::palette` only. |
| Picker | `Mode::ManifestPick` is retired. Its list becomes a roster rail on the manifest itself. |
| Rail order | You first, then party members by slot, then every other owned program A–Z by display name. ←/→ and ↑/↓ cycle in this order. |
| Rail width | A full rail (30 cells) on STATS when the window has room for it. Otherwise, and on SCORE, SOCIAL, TALK and the dossier, a glyph column 4 cells wide. |
| Rank scope | Against everyone in the rail, i.e. `manifest_subjects()`, so `rank of N`. A stat the player lacks is ranked among programs only. |
| Layout | A fixed three-column ledger (FIGHT · GROWTH · DUTY/RUN) with full-width bands below it. This replaces the two-column packer. |
| Bars | One vitals line of HUD block meters (`hud::bar::bar`). This replaces the three `draw_bar` meters. |
| Call to act | One line in `hud-attention`, taken from `Game::attention`'s derivation, with its key. |
| Keys | The keybar is HUD-style. `w` watch becomes `W`. Digits and lowercase letters jump to a rail row. |
| Scope out | `Mode::StructureManifest`, the TALK data model (no partner, topic or cycle on `ExchangeView`), any save-format change. |

Rejected:

- **Keeping the box grid and only tightening it.** The user gave free rein
  and asked for comparison. Boxes packed by height put related numbers in
  whichever column fits, so they can't hold a stable FIGHT/GROWTH/DUTY
  reading order.
- **A face per question (FIGHT/GROW/WORK/SELF tabs).** It loses the
  general-status overview, which is the first reason the user opens the
  sheet.
- **Ranking against the party only.** The user chose all owned programs.

## The sheet

### Frame

- `hud-status-bg` fill and a `stroke-frame` (2px) `hud-pane-border`
  outline, at today's `FRAME_W_PCT` × `FRAME_H_PCT`.
- The rail and the main area are split by a 1px `hud-divider`.
- A keybar runs along the bottom, above a 1px `hud-divider`.
- Everything is drawn through `Painter`. Metrics stay `text::Metrics`, so
  the whole sheet still scales from the one UI font, 16–40px.

### Roster rail

- **Title row:** `ROSTER` (pane title bold), then `N · party first` in
  `hud-faint`.
- **One row per subject:**
  - the row's shortcut from `menu_shortcut(i)` (`1–9`, then `a–z`) in
    `hud-emphasis`;
  - the glyph in `glyph_color`;
  - the name, then `Lv N`;
  - a `!` in `hud-attention` when the subject needs you (see Engine).
- **Party rule:** a `─ party above ─` row in `hud-faint` follows the last
  party member.
- **Selected row:** filled `hud-pane-border`, with a `▸` caret in
  `hud-pane-title`.
- **Collapsed form:** glyph and `!` only, at 4 cells. The selection fill
  still marks the subject.
- **When the rail is full:** only when the face is STATS **and** the frame's
  inner width minus 30 cells still fits three ledger columns of at least
  32 cells. Otherwise it collapses.
- **Overflow:** a rail longer than the frame shows a window that keeps the
  selected row visible. It reports what is cut with `↑ N more` and
  `↓ N more` rows, counted the way `fitting_rows` counts. It never
  scrolls past the selection.

### Summary band

- **Line 1:** the glyph at `title() × 2`. The name in `title()` bold
  `hud-emphasis`. The class or tier in `hud-body`. A boss keeps `[BOSS]` in
  `hud-threat`.
- **Line 2:** tags joined by ` · ` in `hud-faint`. These are today's tag
  strings, from `draw_header`'s builders.
- **Right side:**
  - the face tabs (`Tab STATS · SCORE`, or `Tab STATS · SOCIAL · TALK`) at
    `small()`, with the open face in `hud-pane-title` bold and the rest in
    `hud-field-label`;
  - under them, the call to act, if any, in `hud-attention` bold, for
    example `▸ 5 perk pts  L level up` or `▸ 2 dev points  S spend`.
- **Vitals line,** between two `hud-divider` rules:
  - `INTEG` with 16 `hud-healthy` cells;
  - `PWR` with 10 `hud-attention` cells (player only);
  - `XP` with 14 `hud-ch-gain` cells;
  - each with its readout, and XP followed by `N to Lv M` in `hud-faint`.
  - The cell counts are `log_frame.rs`'s `INTEG_CELLS`, `POWER_CELLS` and
    `XP_CELLS`. Call those constants rather than repeating the numbers.

### Ledger

There are three equal columns, `space-pad × 2` apart. Each column is a
stack of subheads (`hud-pane-title` bold at `small()`, over a 1px
`hud-divider`) and rows. Rows advance by `font_size`, as
`section_row_h` does today. Each row has three parts:

- the label, left, in `hud-field-label`;
- the value, right-aligned to the label column's end, in `hud-body`. The
  subject's own standout figures are in `hud-emphasis`;
- a compare cell: a 5-cell pip ruler (`▮` in `hud-body` over
  `hud-bar-trough`) and an ordinal (`2nd`) in `hud-field-label`. It is
  blank when the row is not ranked.

`fitted_stat_row`'s rule carries over to every row: the value is kept
whole, and the label is middle-elided. The compare cell is dropped before
the label is cut.

**Player STATS:**

| FIGHT `rank of N` | GROWTH | RUN |
|---|---|---|
| Damage, Attack, Accuracy, Evasion, Mitigation, Power. Damage, Attack, Mitigation and Power are ranked. | Next level (`N xp`, `to Lv M`), Perk points (in `hud-attention` when above 0, `unspent`), Decompiler | Credits, Portal Fragments, Contracts, Cargo carried, Difficulty, Cycle, Position |
| GEAR: `SLOT item condition%` → stat delta | CLASS · <class>: one row per multiplier | |
| ROUTINES `n of 6`: the filled slots, then `N slots open` in `hud-attention` | PERKS: name → `Lv n` | |

Below the ledger is a **PARTY band**, a table with aligned columns: glyph,
NAME, INTEG (a 6-cell block meter at `small()` plus the readout), ATK, MIT,
LV, DOING. Your own row is in `hud-emphasis`.

**Program STATS:**

| FIGHT `rank of N` | GROWTH | DUTY |
|---|---|---|
| Damage, Attack, Mitigation, Power, ranked | POTENTIAL: Overall (ranked among programs), then each roll with its `+`/`−` mark | WORK: Base job, Work aptitude (ranked among programs), the need rows (`seams-base.md`: they share WORK) |
| MOVES: the first move, then `+N more on the dossier` | DEVELOPMENT, with `H hold · S spend` in the subhead: Points, Talents | AFFINITIES |
| GEAR | | SPECIES |

Below the ledger is a **LATELY band**: the two newest memory lines, today's
`MANIFEST_MEMORY_ROWS`, with `R all memories` in the subhead.

**Overflow:** each column keeps today's per-section row caps
(`MAX_SECTION_ROWS`, `MAX_AFFINITY_ROWS`, `MAX_MOVE_ROWS`,
`MAX_NEED_ROWS`, `MAX_POTENTIAL_ROWS`), each closing with its `+N more`
note. A column that is still too tall for the space above the bands drops
whole trailing sections and counts them in one `+N more` row. The bands
are never dropped.

### The other faces

All of them use a collapsed rail and the same summary band, without a
vitals line or a call to act.

- **SCORE (player).** Two ledgers, THE FIGHT and THE RUN. They keep
  `score.rs`'s split, `FIRST_BOX_LINES = 4`:
  - THE FIGHT: Programs defeated, Bosses defeated, Deepest Stack depth,
    Phase keys held.
  - THE RUN: Escaped the Basin, Structures standing, Programs compiled,
    Achievements earned this run.

  Each row reads `label  count = points`, with `= points` in
  `hud-field-label`. Below them is a TOTAL band: Difficulty multiplier,
  then Run score in `hud-emphasis` bold, in today's order.
- **SOCIAL (program).**
  - **RELATIONSHIPS:** `SocialView.relationships` in their given order
    (strongest first). Each row is the glyph (a faint `x` for `gone`), the
    name, `Bond  ±opinion`, and a 5-cell ruler of `opinion` clamped to
    `BOND_ENEMY_AT ..= BOND_CLOSE_AT`.
  - **KNOWN FOR and TEMPERAMENT (Sociability)** sit in a narrower column
    beside it.
  - The empty-state lines are kept verbatim.
- **TALK (program).** CONVERSATIONS, newest first, one block per
  `ExchangeView`, separated by a `hud-divider` rule.
  - The speaker sits in a fixed 12-cell column: the subject's own lines in
    `hud-pane-title`, others in `hud-body`.
  - The text wraps with a hanging indent through `text::wrap`.
  - No topic or cycle headings; the view does not carry them.
- **Dossier.** It is drawn in the manifest frame and summary band, not as a
  Large popup. The face tabs are replaced by `D DOSSIER`.
  - **PROVENANCE:** the revision and checksum in the subhead, then the text
    wrapped at `DESCRIBE_WRAP_COLUMNS`.
  - **ATTRIBUTES:** a table of name, legacy name (`WAS`), value and short
    descriptor.
  - The keybar offers `↑/↓ subject · 1–9 jump │ Esc back to manifest`.

### Keybar

It sits along the bottom and is drawn with the `hud::panes::chip` style: the
key in `hud-emphasis` bold, the verb in `hud-field-label`, ` · ` in
`hud-faint`, and `│` in `hud-keybar-divider` between groups. It shows only
the keys valid now, in this order:

1. **Group 1:** `↑/↓ subject · 1–9 jump · Tab face`.
2. **Group 2:** `D dossier · R memories · W watch · L level up · H hold · S spend`.
   - `L` shows only on your own sheet. It is in `hud-faint` when it can't
     reopen, as today.
3. **Group 3:** `Esc back`.

It must drop whole groups from the end with `strip::fitting` rather than
clip.

## Input (app-core)

- **The picker is retired.**
  - Removed: `Mode::ManifestPick`, `handle_manifest_pick_key`, and
    `draw_manifest_pick`/`manifest_pick_label` in gui.
  - The group-menu row "Read a manifest" opens `Mode::Manifest` on the
    first subject.
  - `ManifestOrigin::Picker` becomes `ManifestOrigin::Menu`, whose Esc does
    what the picker's Esc did (`close_screen`).
- **In `handle_manifest_key` and the dossier's handler:**
  - `↑/↓` page the subjects, as `←/→` do today; `←/→` keep working.
  - A digit or lowercase letter goes through `selected_index` over
    `manifest_subjects()` and jumps to that subject. It keeps the open face
    if the subject has it (`fit_manifest_tab`).
  - Watch moves from `w` to `W`. Lowercase `w` is now a row shortcut.
  - D, R, Tab, L, H, S and Esc are unchanged.
- **Paging and jumping from the dossier** stay in `Mode::Dossier`, on the
  new subject.

## Engine

Each per-row derivation lives in the engine (`seams-screens.md`: "a
read-only screen's row count is owned by app-core and drawn by gui, so any
per-row transform must live in the engine"):

- **`Game::manifest_subjects`** returns the rail order: you; party members
  by `party_slot`; then every other owned program by display name,
  case-insensitively, with `ProgramId` breaking ties. `owned_pets`'
  `roster_rank` order is unchanged for the roster screen.
- **`Game::manifest_roster() -> Vec<RosterRow>`.** One row per subject, in
  `manifest_subjects` order, with these fields:
  - `entity`, `name`, `glyph`, `color`, `level`;
  - `in_party: bool`;
  - `needs_you: Option<CallToAct>`.
- **`CallToAct`** is derived where `Game::attention` derives its program
  rows, so the two can't disagree (`seams-hud.md`: "`Game::attention` is
  the one derivation of what needs the player"). It has three variants:
  - `PerkPoints(n)` (player)
  - `DevPoints(n)` (program with banked points)
  - `Idle` (`program_activity` is idle)

  `Game::attention`'s aggregate `ProgramPoints` row becomes a count over
  the same per-program function. `programs_holding_points` is folded into
  it, not duplicated.
- **`Game::subject_ranks(e) -> Option<SubjectRanks>`.** Rows of
  `{ stat: RankedStat, place: u32, of: u32, pips: u8 }` for these stats:
  - `RankedStat::{Damage, Attack, Mitigation, Power}` over every subject;
  - `{Potential, WorkAptitude}` over programs only.

  The rules:
  - Damage ranks by the mean of its range.
  - Ties share a place (1st, 1st, 3rd).
  - `pips = 5` when every value is equal; otherwise
    `1 + floor(4 × (v − min) / (max − min))`.
  - The function is pure over a snapshot of values, so it is unit-tested
    without a `World`. `Game::subject_ranks` only gathers the snapshot.

## Palette scope

- **`hud/palette.rs:27-29`** is rewritten. The popup screens keep
  `render/mod.rs`'s colours; the manifest faces and the dossier draw from
  this palette.
- **`manifest.rs`, `social.rs`, `talk.rs`, `score.rs` and `dossier.rs`**
  stop importing `render/mod.rs`'s colour constants. The exceptions are
  `glyph_color`, which is already a call into `hud::palette::glyph`, and
  the tier colours for a rarity tag.
- **`draw_popup` keeps `ui-*`.** It is still used by the structure manifest
  and every menu.

## Tests

TDD per step. The gate is `cargo test --workspace` and
`cargo clippy --workspace --all-targets`.

- **Engine:**
  - rail order: party by slot before others, others A–Z, you first;
  - `subject_ranks` on hand-built snapshots: ties, all-equal, a
    single-member roster, programs-only stats;
  - `CallToAct` agrees with `Game::attention`'s count;
  - a reproducer that a program with banked points is flagged and stops
    being flagged once spent.
- **app-core:**
  - every `ManifestPick` test is rewritten against the group-menu row
    opening `Manifest`, about 13 in `tests/menus.rs`;
  - ↑/↓ page, a digit or letter jumps and keeps the face, `W` watches,
    lowercase `w` jumps;
  - `lowercase_d_does_nothing_on_a_manifest` becomes
    `lowercase_d_jumps_to_row_d`;
  - Esc from `Menu` closes the screen;
  - paging inside the dossier.
- **gui (the width and height censuses):**
  - Today's census tests are re-pointed at the new layout, about 60 across
    `manifest.rs`, `manifest_layout.rs`, `social.rs`, `talk.rs`,
    `score.rs` and `dossier.rs`. They keep the sweep of 9 window heights
    (720–1440) × 4 widths.
  - New cases:
    - the full rail appears only when three 32-cell columns still fit;
    - no ledger row or rail row overflows its column at any swept size;
    - the keybar fits by dropping groups;
    - the dossier fits the frame.
  - The column-packer tests (`best_column_split`) are deleted with the
    packer.
- **Visual check:** `--template stack --keys … --screenshot` for player
  STATS at 1440×900 and 1280×720, and a program's STATS. Read each PNG
  against the design-system cards.

## Files

- **engine:**
  - `game/party.rs` (`manifest_subjects`, `manifest_roster`);
  - `game/inspection.rs` (`subject_ranks`, the call-to-act derivation
    beside `attention`);
  - `views.rs` (`RosterRow`, `CallToAct`, `SubjectRanks`, `RankedStat`);
  - `derived.rs` (`programs_holding_points` folded in).
- **app-core:**
  - `lib.rs` (`Mode::ManifestPick` removed, `ManifestOrigin::Menu`);
  - `app/inspection.rs` (keys);
  - `app/group_menu.rs` (target);
  - `app/input.rs` (dispatch, mode pair);
  - the tests named above.
- **gui:**
  - `render/manifest.rs` (rewritten draw; the section builders keep their
    row strings);
  - `render/manifest_layout.rs` (the rail, band, ledger and band geometry
    replace the packer);
  - `render/social.rs`, `talk.rs`, `score.rs`, `dossier.rs`;
  - `render/hud/palette.rs` (the scope comment);
  - `render/hud/log_frame.rs` (expose `keycap` or reuse `panes::chip`);
  - `render/mod.rs` (dispatch).
- **docs:** `CHANGELOG.md` at landing. The design-system cards are already
  published.

## Risks

- **`manifest.rs` is 2,854 lines.** The rewrite splits it: the rail, band
  and ledger go into their own modules under `render/manifest/`, and the
  section builders stay. The `module-boundaries` skill runs at plan time.
- **Rail order changes paging order.** Paging is the order you step
  through programs, so muscle memory changes. It matches the rail you can
  now see.
- **Ranks across a large roster are noise.** At 40 programs, `17th`
  carries little meaning. The pip ruler carries the reading. Revisit if
  play says otherwise.
