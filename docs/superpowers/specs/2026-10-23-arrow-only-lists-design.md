# Arrow-only list selection

**Status:** Proposed design, 2026-10-23.

## Intent

Every list the player picks a row from is navigated by a highlight:
Up/Down (PageUp/PageDown for long lists) move it, Enter chooses. The
positional row labels — `[1]`–`[9]`, then `[a]`, `[b]`… from
`menu_shortcut` — go away everywhere.

Why: long lists (the dev `` ` `` Give item picker is the trigger) are
miserable to drive by reading off a letter, and one ubiquitous interaction
is simpler than two. Freeing lowercase letters inside lists is also the
groundwork for a later feature: typing a name filters the list.

## Goals

- One list interaction everywhere: highlight + Enter.
- No positional row labels drawn on any list in any screen, the tactical
  map included. The one exception is the classic battle screen (below).
- The dev pickers (`Give item`, dev zone picker) behave like every other
  list: a highlighted row, Enter fires it, Up/Down move the highlight, the
  visible window follows the highlight.
- Lowercase letters do nothing inside lists (until the filter exists).

## Non-goals

- **Type-to-filter.** A later, separate piece of work. This change only
  makes sure nothing stands in its way.
- **Word letters stay.** Keys that stand for a word rather than a row
  position are not selectors and are kept: the main menu's `n`/`l`/`a`/
  `o`/`r`/`d`/`q`, confirms' `y`/`n`, and any screen's own mnemonic
  lowercase keys.
- **UPPERCASE action keys stay** (`S`/`B` quick-trade on the highlighted
  row, and the rest).
- Digit or letter keys outside lists (map-mode keys, hotkeys that are not
  row picks) are untouched.
- No mouse work.
- **The classic battle screen keeps its number keys** (user, 2026-10-23).
  Its ability, target and party-slot lists still show and accept
  `menu_shortcut` labels. The tactical map does not.

## Anti-goals

- A list whose highlight is not drawn. Once labels are gone the highlight
  is the only way to see what Enter will do; a list without one is
  unusable.
- A confirm that destroys something on a stray Enter. `opening_row`'s
  "opens on No" rule for irreversible confirms stays.
- Tests weakened by the migration: a test that picked row 3 must still
  prove row 3 is what got picked.

## Current state (verified 2026-10-23)

- `App::selected_index` (`crates/app-core/src/app/input.rs:113`) is the
  shared picker, ~94 call sites over 33 modules. It already does Up/Down
  (wrapping), PageUp/PageDown (clamped, `MENU_PAGE_ROWS`) and Enter; it
  *also* resolves digits and lowercase letters to rows. Removing the
  digit/letter branch is the core of the app-core change.
- `menu_shortcut` / `DIGIT_ROWS` (`crates/app-core/src/lib.rs:153-180`)
  produce the labels; ~40 gui renderer files print them
  (`rg -c menu_shortcut crates/gui`).
- Dev pickers (`app/dev_console.rs:390-423`, `app/dev_item_picker.rs`)
  bypass `selected_index`: `menu_selected` is a scroll offset, Up/Down slide
  the page, a selector names a row on the page, Enter does nothing.
- `yes_no` swallows digits deliberately so a repeated `1` cannot answer a
  confirm unseen. With digits no longer picking rows anywhere, that guard's
  reason is gone; the guard itself is harmless and may stay or go.
- Comments in `app/party.rs`, `app/progression.rs`, `app/menus.rs` reason
  about avoiding collisions with the digits-then-lowercase scheme; those
  comments become stale and must be rewritten or deleted.
- ~227 app-core tests press `Char('1')`…`Char('9')`, plus tests pressing
  `menu_shortcut(i)` (including `tests/support.rs:2801,2835`).

## Decided

- Scope is positional row selectors only (user).
- The tactical map's lists go arrow-only. The classic battle screen keeps
  digits (user).
- Letters inside lists are inert for now (user).

## Settled after builder check

- **Highlight state already exists.** `selected_index` keeps the highlight
  in the one shared `App::menu_selected`, reset to `opening_row` whenever a
  screen opens. No new state is needed per mode. Tests read
  `menu_selected` and step from it. They must not assume row 0, because
  irreversible confirms open on No.
- **Wrap stays** (my call): Up/Down wrap and PageUp/PageDown clamp, in long
  lists and the dev pickers too. That is one behaviour everywhere, and
  wrapping from the top is the quick way to the bottom of a long list.
- **Enter already means "choose the highlighted row"** wherever
  `selected_index` is called. The plan lists any screen that gives Enter a
  separate job and settles each one there (my call).
- **Mnemonic collisions are bugs, not design** (my call). The plan lists
  every screen that maps its own lowercase letters, and a test pins each
  one.
- **No dual path** (my call). Commits may land in any order that keeps the
  suite green; drawing no labels while digits still work is a harmless
  in-between state. Nothing keeps the old scheme alive behind a flag.

## Found by the inventory (2026-10-23)

- **World map (`g`)** — the arrow keys pan the map, so the place list
  can't share them. Decided by the user: **Tab switches focus between the
  map and the place list.** The screen opens with the map focused, where
  the arrows pan. With the list focused, Up/Down/PageUp/PageDown move the
  highlight. The highlighted row *is* the picked place, as a letter press
  made it before, so `C` acts on it; Enter adds nothing there (my call).
  If the Stack's frame map (`g` in the Stack) has a picked list too, it
  gets the same treatment. `C`/`P`/
  Esc and mouse clicks work under either focus (my call). The renderer
  shows which side has focus by drawing the list highlight only while the
  list is focused, plus a one-word header cue (my call; follow whatever
  pattern a two-pane screen already uses). Update the `g` paragraph in
  `assets/help/20-controls.md`.
- **Outposts** (`app/outposts.rs:73-77`, `:123-126`) run their own `a..z`
  crew selector, and in OutpostPost the letter posts immediately. They move
  to `selected_index`: highlight, then Enter posts.
- **Inspection manifest** (`app/inspection.rs:333-342`,
  `manifest_nav_key`) jumps to a subject on a digit or letter. That jump
  goes; the arrows already move through the manifest.
- **The battle target screen's group letters** (`app/battle.rs:299`) stay.
  They belong to the classic battle screen.
- **`render/mod.rs::emulation_row_label`** is shared by battle and
  tactical, so the label must become a choice the caller makes.
- **In-game help** (`assets/help/`) is not the manual, so it is updated in
  step with this change wherever it names row keys.

## Left to the builder

- Where digit picking lives once `selected_index` stops resolving
  digits. The classic battle screen still needs it, so `menu_shortcut` and
  `DIGIT_ROWS` survive, scoped to that screen. Expected: a battle-only
  resolver beside `selected_index`, not a flag on it. Also how renderer row
  layout reflows without the label column.
- How each renderer that lacks a highlight draws one — reuse the existing
  popup highlight style; sweep all list renderers to find the ones missing
  it.
- Test migration shape. Expected: a support helper `pick_row(app, n)` that
  sends Down×n (from the opening row) then Enter, or sets the highlight and
  presses Enter; plus at least one test per removed path asserting that a
  digit and a lowercase letter **no longer** pick a row.
- Dev picker window: a highlight within a scrolled window, the window
  scrolling to keep the highlight visible, like every other popup.
- Screens with mnemonic lowercase keys that previously coexisted with row
  letters only because they had ≤9 rows: confirm each still works once
  `selected_index` stops eating letters (it should, since those screens map
  letters themselves).

## Process

Two crates (app-core, gui), no schema or save change → spec-and-plan
pipeline. No ADR needed (no data, no crate boundary, no Agreed record
bent). Retire the memory rule "row selectors: digits 1-9, then lowercase"
when this lands. Update `CHANGELOG.md`; do not touch the manual or README.
