# `?` shortcut popup, `v` on the bar, manual in the game menu

**Date:** 2026-10-30

## Why

The user went looking for how to lay floor and couldn't find `v`. It's bound
on the map (`crates/app-core/src/app/playing.rs`, `GameKey::Char('v')` →
`Game::lay_tile`), but no bottom bar names it. The only place it appears is
the manual. Because the bar has to fit a narrow window, it can only ever show
a few keys. Something needs to show the rest of what works where you're
standing, one keypress away.

## What gets built

1. **`?` opens a shortcuts popup.** It's a popup over the current screen
   that lists every keyboard shortcut relevant to where the player is now.
   There are four areas:
   - **Base:** on the surface, standing on laid floor (`Game::on_base_floor`).
   - **Field:** on the surface, anywhere else.
   - **Stack:** underground (`Game::is_underground`).
   - **Tactical:** in a battle. This covers both battle screens, the classic
     one (`render/battle.rs`) and the tactical map (`render/tactical.rs`).
     The popup lists that screen's own keys.

   `?` no longer opens the manual.
2. **The manual moves into the game menu** (Esc on the map; `[O]` on the
   main menu), as a new hub page, "Manual". Enter or Right on its rail entry
   opens today's manual index (`Mode::Help`), the same way the Sprites
   entry opens its picker (`GameMenu::key`). Esc from the manual index
   returns to the game menu.
3. **`?` is the first segment of every bottom bar that has one**, and every
   bar that names it binds it. The bars are the map bar
   (`App::map_keybar`: the locale bar, the excavation plan bar, the
   watching bar and the walking bar), the tactical battle bar
   (`tactical::action_bar` / `keybar_rows`) and the classic battle bar
   (`render/battle.rs`). `?` comes first, and bars cut segments from the end
   (`fitting`), so it is the last thing to go.
4. **`v lay tile` is on the map bar while the player is in the base**:
   standing on laid floor, or on a cell orthogonally beside laid floor
   (decided by the user). That covers every cell where `v` can succeed.
   It still shows on a cell where the press will be refused (already
   floored, rock, no fragments), and the refusal message is what explains
   why.
5. **The help pages are corrected.** `assets/help/60-your-base.md:25` says
   a tile costs two Core Fragments, but `FLOOR_TILE_COST` is 1. The pages
   that say `?` opens the manual (`20-controls.md` and any others) now say
   `?` lists shortcuts and the manual is in the Esc menu.

## Constraints

- **app-core decides what keys mean and where they work, and gui only
  draws** (`map_keybar.rs`'s module doc). The popup's contents for an area
  come from app-core, not from a list written out in gui.
- **The bar and the popup cannot disagree.** Every key a bar shows in an
  area is listed in that area's popup, and a test holds this for each
  locale and battle state the bars produce. The popup may list more than
  the bar. Its purpose is to list everything.
- **The bar must fit.** `the_keybar_fits_the_log_pane`
  (`gui/src/render/hud/log_frame.rs`) measures what survives at the
  smallest supported window and fails if a key is dropped. Adding `?` and
  `v` will push something past the cut. The fix is to reorder or drop
  low-priority segments, never to widen the pane (the test's own message).
  `?` replacing the `? help` segment that already exists costs nothing. The
  segments that may be cut are the builder's call, since the popup now
  lists anything a bar can't fit.
- **GUI text never clips or wraps silently.** The popup must fit at the
  smallest supported window (1280x720) with the longest area's list. If it
  can't fit in one column, use two columns or groups, and add a width and
  height census like the existing popup tests
  (`popup-row-width-is-testable-headlessly`).
- **Lists are highlight and Enter, with no row digits** (standing
  decision). The popup is read-only, so it has no selection at all.
- Opening the popup spends no ticks and never pauses or unpauses the
  clock. The world keeps whatever pause state it had.
- Keyboard-only. Nothing here touches mouse input.

## Behaviour details (my calls; the user can override)

- **The popup shows only while `?` is held down** (decided by the user).
  Releasing the key hides it, and a quick tap flashes it. It follows Alt's
  label overlay: gui's frame loop knows a key is held (`reveal`,
  `crates/gui/src/lib.rs`, passed to `render::draw`), and the overlay is
  drawn from that alone. No `Mode` and no press goes to app-core. `?` is
  typed text and layout-dependent, so gui records the physical `KeyCode`
  whose press produced `?` and clears it when that key's release arrives
  (`keys.rs`'s `TextGate` already tracks press and release by `KeyCode`).
  Releasing Shift first does not hide it. The `?` character is consumed in
  gui and never reaches `App::handle_key`, so app-core binds `?` nowhere.
- **What it shows is app-core's**: one call (e.g. `App::shortcut_sheet()
  -> Option<…>`) returns the current area's table, or `None` where `?` is
  not live. gui draws whatever comes back and nothing when it is `None`.
- Keys pressed while `?` is held go through as normal. Nothing is
  swallowed, because a held overlay is not a screen.
- It's titled with the area, e.g. "SHORTCUTS — BASE", and grouped the way
  the manual's controls page groups keys (moving, actions, screens).
- Base inherits field: the Base list is the Field list plus the base-only
  keys (`v`, the excavation plan `m`, and so on), not a separate list that
  has to be kept in step by hand.
- A battle's popup lists every key that battle screen binds, across all
  of its states (aim, a charge decision, auto-attack), rather than only the
  current state's bar. That way it doesn't flicker while held through the
  opponent's turn.
- On the main menu, `?` does nothing new. The manual is reached through
  `[O]` → Manual.
- **`?` is live only on the map (`Mode::Playing`) and the two battle
  screens**, which means `shortcut_sheet()` is `Some` only there. gui
  swallows the `?` character only while the sheet is `Some`. Elsewhere it
  passes through as today, so text entry (names, filters) still takes a
  typed `?`.
- **The manual returns to the game menu.** Today Esc from `Mode::Help`
  goes through `close_screen` to wherever `?` was pressed. Now the manual's
  only door is the game menu, so Esc from the index reopens the game menu
  on its Manual page. That covers both entries (in a run, and from `[O]` on
  the main menu, which opens the same `Hub::GameMenu`). The tests that
  expect `?` to give `Mode::Help` (`tests/menus.rs`, `tests/stack.rs`) are
  rewritten to reach the manual through the game menu. New app-core tests
  assert `shortcut_sheet()` for each area and `None` off the map. A gui
  test drives the hold: press shows it, release hides it, and Shift
  released first still shows it.
- **"In the base" for `v` is an engine predicate**, not app-core
  recomputing it. Add something like `Game::in_base_reach` (laid floor, or
  orthogonally beside it), with the adjacency answered by the same
  `BaseGrid` reads `floor_request_refusal` uses. This adds a small engine
  change and an engine unit test.
- **Where the agreement test lives.** app-core exports each area's key
  table. The map-bar test is in app-core. The battle-bar test is in gui,
  because the battle bars are built there: for each battle bar state it
  checks that every bar key is in the Tactical table.
- **Stack does not inherit Field.** Stack has its own list, and only Base
  inherits from Field.

## Non-goals

- Moving the battle bars' state logic out of gui into app-core. That would
  be better structure, but it is not part of this task. The battle bars
  only gain a leading `?`. The battle popup's key list lives in app-core
  either way.
- Making keys rebindable, or generating the manual's controls page from
  the tables.
- Adding `?` popups to the hub screens (base report, manifest, pack).
  Those have their own keybars. They stay as they are.

## Anti-goals

- A popup that lists a key that does nothing in that area. This repeats
  the bar's existing rule: a key the bar names and the place refuses reads
  as a bug.
- Losing access to the manual. It has to stay reachable in a run and from
  the main menu.

## Process

This touches app-core and gui, so it gets a written plan
(`superpowers:writing-plans`), then TDD. No ADR: no data schema, no new
`assets/<kind>/`, no crate boundary change. The key tables are plain Rust
in app-core, next to `map_keybar.rs`. Release per `CHANGELOG.md`'s preamble
when it lands (no save format change).

## Left to the builder

- The exact key lists for each area, built from `20-controls.md` and the
  real handlers. The handlers win where they disagree, and the popup lists
  what the handlers actually bind.
- Which segments get cut from the bars to fit `?` and `v`.
- The popup's module name and table type. It has to draw over the battle
  screens as well as the map.
