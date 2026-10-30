# `?` Shortcut Popup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Checkbox (`- [ ]`) steps.

**Goal:** Holding `?` on the map or a battle screen shows every shortcut for the current area; `?` leads every map/battle bar; `v lay tile` shows in the base; the manual moves into the game menu.

**Spec:** `docs/superpowers/specs/2026-10-30-shortcut-popup-design.md` — read it; this plan does not repeat its behaviour rules.

**Phasing (token cost):** two phases, one subagent each, so each loads one crate's context. Phase A (engine + app-core) produces the API Phase B (gui) draws. B starts only after A is committed and green. Then one opus whole-branch review.

## Global Constraints

- No save format, asset schema or ADR change. `Game.world` stays private.
- app-core decides keys; gui draws only. Key tables live in app-core.
- Key strings in the tables are **byte-identical** to the bar's (`"Esc"`, `"↑↓←→"`, `"< >"`), so the agreement tests compare plain strings.
- `render/` draws only through `Painter`; text never clips or wraps silently.
- Opening the popup spends no ticks and never touches pause.
- `cargo fmt` + `cargo clippy --workspace --all-targets` clean after each task. Unset `FERAL_DEV_NO_SIEGES` before tests.
- Before the new module's types, run the `design-patterns` dialog (user CLAUDE.md). Expected answer: plain data + one pure fn per area, no trait.
- Subagents: never `git checkout`/`stash`/`push`; stage explicit paths.

## Review Focus

1. Every key a map bar shows (each `MapLocale` × mining, plan bar, watching bar, walking bar) is in that area's table. Each battle bar state's keys are in its battle table.
2. Every table key is actually bound by the handler for that area (anti-goal: a listed key that does nothing). Check by reading handlers, not `20-controls.md`.
3. `?` reaches `App::handle_key` only where `shortcut_sheet()` is `None`, so text entry still types `?`.
4. Shift released before `?` keeps the popup up; `?`'s key release hides it.
5. Esc from the manual index returns to the game menu's Manual page, both in a run and from `[O]`.
6. `the_keybar_fits_the_log_pane` passes without widening the pane.

---

## Phase A — engine + app-core (one subagent, sonnet)

### A1: Engine `Game::in_base_reach`

**Files:** `crates/engine/src/game/base/building.rs` (beside `lay_tile`, ~1165), test in `crates/engine/src/tests/base_space.rs`.

- [ ] Failing test: true on laid floor, true on an orthogonal neighbour of floor, false diagonally beside it, false far away, false underground.
- [ ] Implement with the same `BaseGrid` reads `floor_request_refusal` uses (`stack.rs:396 on_base_floor` is the floor half). Green, commit.

### A2: `MapLocale::Base` and `v` on the bar

**Files:** `crates/app-core/src/app/map_keybar.rs`, its tests.

- [ ] Add `MapLocale::Base` (chosen in `map_keybar` when `!is_underground() && in_base_reach()`). It gets the Field verbs plus `v lay tile`, placed high enough to survive the cut.
- [ ] Rename the `("?", "help")` segment to `("?", "keys")` (or similar) and move it to the **first** segment of every map bar: the locale bar, `plan_keybar`, the watching bar and the walking bar.
- [ ] Update the module/fn doc comments that say `?` is the manual.
- [ ] Tests: Base locale names `v`; Field does not; `?` is first in each bar. Commit.

### A3: Shortcut tables + `App::shortcut_sheet`

**Files:** new `crates/app-core/src/app/shortcuts.rs` (register in `app/mod.rs`, re-export types from `lib.rs`), tests in `crates/app-core/src/tests/` (new `shortcuts.rs`).

Interface (names are the builder's; shape is fixed):

```rust
pub enum ShortcutArea { Base, Field, Stack, TacticalMap, ClassicBattle }
pub struct ShortcutGroup { pub title: &'static str, pub keys: Vec<(&'static str, &'static str)> }
pub struct ShortcutSheet { pub title: String, pub groups: Vec<ShortcutGroup> } // "SHORTCUTS — BASE"; both battles say TACTICAL
pub fn shortcut_sheet_for(area: ShortcutArea) -> ShortcutSheet;   // pure
impl App { pub fn shortcut_sheet(&self) -> Option<ShortcutSheet> } // Some only in Mode::Playing and the battle modes
```

- [ ] Build each list from the real handlers (`app/playing.rs`, `app/battle.rs`, `app/tactical.rs`); `20-controls.md` is a cross-check. Groups follow the controls page (moving, actions, screens). Base = Field groups + base-only keys appended (`v`, plan `m`, …) by code, not a copy. Stack is its own list. Battle lists cover every state of that screen.
- [ ] Tests: `shortcut_sheet()` per area from `dev-saves/` templates (stack, base, field) and a battle; `None` on main menu, a hub screen, `Mode::Help`.
- [ ] **Map agreement test:** for each `MapLocale` × `mining`, plus plan/watching/walking bars, every bar key ∈ the matching area's table (watching/walking keys must be in all three map tables). Mutation-check: delete one table entry, see it fail. Commit.

### A4: Manual page in the game menu; `?` unbound in app-core

**Files:** `crates/app-core/src/app/game_menu.rs`, `crates/app-core/src/lib.rs` (`Mode` enum + mode lists near 2452/2673/2693), `app/input.rs` (~271), `app/playing.rs:377`, tests `tests/menus.rs`, `tests/stack.rs:175`, `tests/playing.rs:879`.

- [ ] New `Mode::ManualOptions` and `const MANUAL: HubPage`, after Sprites in `pages()`. Mirror Sprites exactly: `extra_keys` gives `("Enter","open")` on its rail; `key` opens the manual on Enter/Right. Opening sets `menu_origin = Some(Mode::ManualOptions)` and `mode = Mode::Help`, so the existing Esc → `close_screen` in `handle_help_key` returns to the page with no new code (see `open_sprite_forge`, `sprite_forge.rs:259`). Check `all-modes-*` memory traps when adding the variant.
- [ ] Delete the `GameKey::Char('?')` arm in `playing.rs`. No other app-core `?` binding exists.
- [ ] Rewrite the `?`→`Mode::Help` tests through a test helper `open_manual(app)` (Esc → game menu → Manual → Enter). Add: Esc from the index lands on `ManualOptions` in a run and from the main menu `[O]`; leaving the menu from there still reaches Playing / MainMenu respectively. Commit.

### A5: Help pages

**Files:** `assets/help/60-your-base.md:25` (two → one fragment, `FLOOR_TILE_COST`), `assets/help/20-controls.md` and any page `rg -n '\?' assets/help` shows saying `?` opens the manual, `assets/help/README.md:4`.

- [ ] Say `?` (held) lists shortcuts; the manual is in the Esc menu. Plain language. Commit.

**Phase A gate:** `cargo test -p feral-processes-engine in_base_reach`, `cargo test -p feral-processes-app-core`, clippy clean. Report the final public API signatures in the handback.

---

## Phase B — gui (one subagent, sonnet; starts after A lands)

### B1: Hold tracking

**Files:** `crates/gui/src/keys.rs` (new small struct beside `TextGate`, with tests), `crates/gui/src/lib.rs` (~855–890 text loop, `render::draw` call at ~949).

- [ ] Pure struct (e.g. `SheetHold { key: Option<KeyCode> }`) with `press(key)`/`release(key)`/`held()`. Tests copy `TextGate`'s test style: press shows, release of that key hides, release of another key (Shift) does not, repeat press stays held.
- [ ] In the typed-text loop: if the char is `?` and `fe.app.shortcut_sheet().is_some()`, record the key in the hold and do **not** call `handle_key`; otherwise deliver as today. Feed every release to the hold. Do not route `?` through `TextGate`'s screen-change block.
- [ ] Pass the held flag into `render::draw` next to `reveal`. Commit.

### B2: The popup

**Files:** new `crates/gui/src/render/shortcuts.rs` (or under `render/hud/`), hook in `render/mod.rs::draw` after the screen is drawn, for Playing and both battle screens.

- [ ] Draw `app.shortcut_sheet()` when held: title, groups, key/label rows; two columns if one won't fit. Read-only, no selection.
- [ ] Width and height census at 1280x720 for every area's sheet (pattern: `popup-row-width-is-testable-headlessly` memory and existing popup tests). Commit.

### B3: `?` on the battle bars + battle agreement test

**Files:** `crates/gui/src/render/tactical.rs` (`action_bar` ~1566, `keybar_rows` ~1676), `crates/gui/src/render/battle.rs` (`action_bar_line` ~89), their tests.

- [ ] Prepend `?` as the first segment of each battle bar.
- [ ] Agreement test in gui: for each tactical bar state (aim, charge decision, auto, plain) every bar key ∈ `TacticalMap` table; for the classic bar every key ∈ `ClassicBattle` table. The classic bar is a joined label string, so extract keys however its labels encode them; if that is fragile, say so in the handback rather than weakening the test. Commit.

### B4: Fit the bars

**Files:** `crates/app-core/src/app/map_keybar.rs` order only (allowed in B), `gui/src/render/hud/log_frame.rs:645` test.

- [ ] Run `the_keybar_fits_the_log_pane`. If it fails, reorder or drop low-priority segments (never widen the pane). Record which segments were cut in the commit message.
- [ ] Screenshot check: `cargo run -- --template <base template> --screenshot out.png` with the popup forced on (add a `FERAL_DEV_SHORTCUTS=1` hold-down alongside `dev_alt()` only if needed; otherwise screenshot the bar). Read the PNG. Commit.

**Phase B gate:** `cargo test --workspace`, clippy clean.

---

## Finish

- [ ] Opus whole-branch review against the Review Focus above; diff passed as a file.
- [ ] Land via the `deploy` skill: patch release (no save change), `CHANGELOG.md` entry, INDEX.md row.
