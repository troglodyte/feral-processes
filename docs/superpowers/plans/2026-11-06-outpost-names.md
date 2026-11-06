# Outpost Names: Implementation Plan

**Spec:** `docs/superpowers/specs/2026-11-06-outpost-names-design.md`.

**Branch:** `worktree-sites-hub`, on top of the Sites hub.

**Size:** two tasks, engine first and then app-core with gui. One subagent can run both in order.

## Global Constraints

- Every Global Constraint in `2026-11-06-sites-hub.md` applies. In particular:
  - Use `#[serde(default)]` and do not bump the save format version.
  - Add a real `Game::save` → `Game::load` test.
  - Add no RNG draws.
  - Text never clips or wraps.
  - Key hints use the handler's case.
  - Refusals go through `App::refuse`.
- Git:
  - Stage explicit paths only, and commit each task at green.
  - Never push.
  - Never use `checkout`, `stash`, `reset` or `add -A`.

## Task 1: Engine — stored name, default, rename door, save

**Files:**
- `crates/engine/src/outposts.rs`: add `pub name: String` to `Outpost`.
- `crates/engine/src/tuning/outposts.rs`: add `OUTPOST_NAME_MAX: usize = 24`. If that file doesn't exist, find where the outpost tuning lives.
- `crates/engine/src/game/outposts.rs`:
  - Add a pure function `default_outpost_name(tile, anchor: Option<(i32,i32)>, taken: &BTreeSet<String>) -> String`. It uses `stack::heading` and returns the first free name among `D`, `D 2`, …
  - `found_outpost` stores that name.
  - Add `pub fn rename_outpost(&mut self, tile, name: &str) -> Result<(), String>`. It trims the name and truncates it to `OUTPOST_NAME_MAX` chars. An empty name gets the default, computed with this outpost's own name excluded from `taken`.
  - `outpost_destination_name` returns the stored name. Rewrite its doc comment.
- `crates/engine/src/save/mod.rs` `OutpostSave`: add `#[serde(default)] pub name: String`.
- `game/lifecycle/save.rs`: write the name.
- The load path that rebuilds `Outposts`: restore the name. If a restored name is empty, assign the default in tile order.
- Add the field to the RON fixture.

**Tests:** inline for the pure function, in `tests/sites.rs` or the existing outpost tests for the rest.
- `default_name_is_the_heading_from_base`: north-east → "Outpost north-east". With no anchor → "Outpost".
- `a_second_outpost_on_one_heading_is_numbered`: "Outpost north" taken while "Outpost north-east" is free → the north-east name gets no number. This is the prefix trap.
- `founding_never_renames_an_older_outpost`.
- `rename_trims_caps_and_empty_restores_the_default`.
- `rename_refuses_an_unknown_tile`.
- `an_outpost_name_survives_save_and_load`: a real save → load.
- `an_old_save_without_names_gets_defaults_in_tile_order`: RON text with the field removed.
- Fix every existing test that asserted "Outpost (x, y)".
- **Mutation check:** put `#[serde(skip)]` on `name`, confirm the save test fails, then revert it.

**Gate:** `cargo test -p feral-processes-engine`. Commit.

## Task 2: app-core and gui — `N` opens the name field

**Files:**
- `crates/app-core/src/lib.rs`:
  - Add a `Mode::OutpostName` variant with a one-line doc.
  - Add an `App` field `outpost_name: Option<OutpostNameDraft>`, where the draft is `{ tile, field: TextField, back: Mode }`. If the Sites hub needs its held page restored on Esc, `back` can carry it as `(Mode, Option<HubPageKey>)`.
  - Initialise the field in `lifecycle.rs`.
- `crates/app-core/src/app/outposts.rs`, the mode logic:
  - On `N`, fill the draft with `TextField::set(current name)`.
  - Enter calls `rename_outpost` and refuses on `Err`. Esc changes nothing.
  - Both Enter and Esc return to `back`. A Sites return re-enters with `enter_hub_at(Hub::Sites, key, Mode::OutpostStatus)`.
  - Typing goes through `field.edit(key, OUTPOST_NAME_MAX)`.
- `N` arms:
  - On the `OutpostVisit` handler (`app/outposts.rs`). `N` is free there today; check.
  - On `OutpostStatus` in `app/sites_hub.rs`.
  - Add `N`, "rename" to `sites_screen_keys(OutpostStatus)` and to the visit page's keybar, in the handler's case.
- `crates/app-core/src/app/input.rs`: route `Mode::OutpostName`.
- gui:
  - Draw `OutpostName` with the same row and caret drawing the routine or gear name field uses. Find it from `Mode::RoutineBuildName` or `DesignName` in `render/`, and extract a shared function if it is inline.
  - Add the mode to `ALL_MODES` in `render/tests/refusals.rs`.
  - Width census: a 24-character name on the Sites rail with a glyph and a `!`.
- Help: `assets/help/72-outposts.md`. One sentence: "Press N on an outpost's page to rename it." Plain language.

**Tests** in `crates/app-core/src/tests/outposts.rs`, or `sites_hub.rs` for the hub cases:
- `n_opens_the_field_holding_the_current_name`: the cursor is at the end, and one Backspace deletes one char.
- `enter_renames_and_returns_to_the_visit_page`.
- `esc_leaves_the_name_and_returns`.
- `renaming_from_sites_returns_to_that_outposts_status_tab`. `hub_page()` names the same outpost even when two outposts exist.
- `the_rail_shows_the_new_name`.
- `naming_never_advances_the_game`: the tick is unchanged.

**Gate:**
1. `cargo fmt --check`.
2. `cargo clippy --workspace --all-targets`, which must be clean.
3. `cargo test --workspace`, with `FERAL_DEV_NO_SIEGES` unset.
4. Take a screenshot of the Sites rail showing a named outpost and Read it.

Commit. Then add a line to the Sites hub entry in `docs/superpowers/INDEX.md`.

## Review

Run an Opus review of this diff only. Check the prefix trap, the return path to the held Sites page, save → load, and that no "(x, y)" name survives anywhere a player sees. Search with `rg -n 'outpost_destination_name|\({}, {}\)' crates`.
