# Plan: dev keypad cheats

Spec: `docs/superpowers/specs/2026-10-20-dev-keypad-cheats-design.md`. Read
it first; this plan carries files, interfaces, test intent and gates, not
code.

Conventions for every task: TDD (failing test, then code), `cargo fmt`,
`cargo clippy --workspace --all-targets` clean, commit at green. Iterate with
`cargo test -p <crate> <name>`; the full gate is Phase 4. `unset
FERAL_DEV_NO_SIEGES` before any test run. Never `git checkout` another
agent's work; never push; stage explicit paths. Rules to read first:
`seams-base.md` (T1), `seams-research.md` (T4), `seams-species.md` (T5),
`seams-screens.md` (Phase 2), `drawing-seam.md` (Phase 3).

Anchors (0.19.18): `DevSieges` `engine/src/resources.rs:2091`,
`dev_set_sieges` `game/siege/clock.rs:39` (the resource pattern to copy);
`dev_force_raid` `game/base/upkeep.rs:405` (the `#[doc(hidden)] pub fn dev_*`
convention); `commit_for_build` / placement `game/base/building.rs:229`,
`commit_rebuild_program` `:547`, `upgrade_cost` `:1189`, upgrade commit
`:1310`; `raise_one_tick` `game/base/construction.rs:541` (`New` `:578`,
`Upgrade` `:599`); `settle_research` `game/unlocks.rs:1482`;
`warp_to_zone` `game/zone.rs:867`; `adopt_program` `game/spawning.rs:606`,
`habitat_pools` `:1898`, field-spawner call `:1833`; caravan adoption
`game/caravan.rs:1339-1341`. App-core: `DevAction`/`DEV_ROWS`
`app/dev_console.rs:31,63`; `handle_research_key`
`app/progression.rs:94`; `Mode::SpritePicker` `lib.rs:2381` + opener
`app/sprite_forge.rs:255` (the picker pattern); dispatch `app/input.rs:408`;
tests `src/tests/dev_console.rs`. Gui: `render/mod.rs:1235` (keypad),
`:1617-1626` (research), `render/progression.rs` `draw_research_menu`.

Decisions this plan makes (the spec's "left to the builder"):

- **Companion call site:** mirror `deliver_offer`'s `Program` arm
  (`caravan.rs:1339`): `stat_mult = ZoneLevel::stat_multiplier()`, position
  `anchor_position().unwrap_or((0,0))`. It is the one path that hands the
  player a companion from nothing, which is what the cheat does.
- **Research key:** `U` — free on the research screen (taken on inventory,
  outposts, tactical, party; none of them shares this handler). Target:
  graph view → the cursor node (`ids[menu_selected]`); list view → the
  active research, else refuse "Press G and put the cursor on a node". The
  list view has no cursor and a digit already selects, so a blocked node is
  reached through the graph.
- **Plain item:** an `ItemDb` entry that `Inventory::add` accepts and that
  is not a gear item — the T3 builder names the predicate that already
  exists (grep `is_gear`/`ItemKind`); do not add a new one if one exists.
- **Free build keeps the program commit.** Only materials are freed; the
  committed program still sets tier, as on a rebuild.
- **Paging:** 9 + 26 selectors per page; Up/Down moves the page by one
  row; `menu_selected` is the scroll offset.

## Phase 1: engine (one crate)

### T1. Free build
- `resources.rs`: `pub struct DevFreeBuild(pub bool)`, beside `DevSieges`,
  never inserted by `new`/`load`, never in the save struct.
- `Game::dev_set_free_build(bool)`, `Game::dev_free_build() -> bool`
  (missing resource = off), in `game/base/building.rs` or a new
  `game/base/dev_build.rs` if `building.rs` is over ~3k lines.
- Extract from `raise_one_tick`'s `New` branch:
  `fn complete_new_build(&mut self, kind, at: Position, program, recipe)`
  — `spawn_structure`, `restore_machine_recipe`, displace calls,
  `log_base`. Same for `Upgrade`: `fn complete_upgrade(&mut self, target,
  to_tier, program)`. `raise_one_tick` keeps `Task` removal +
  `consume_site`, then calls them.
- Placement, upgrade commit and `commit_rebuild_program`: when free build
  is on, material cost is empty and they call the `complete_*` function
  instead of filing a `BuildSite`. Founding Home: only the cost empties.
  `upgrade_cost` returns `Some(vec![])` while on.
- Tests (`engine/src/tests/construction.rs` or a new
  `tests/dev_cheats.rs` — prefer new; register in `tests/mod.rs`):
  placement with an empty inventory spawns a complete structure and no
  `BuildSite`; upgrade completes at once, `upgrade_cost` empty; rebuild
  still consumes its program; toggle off → normal cost and site;
  save→load of a game with the toggle on comes back off.

### T2. Give item
- `Game::dev_give_item(&ItemId, qty: u32) -> Result<String, String>`:
  unknown id → Err, gear → Err, else `Inventory::add` on the player, Ok
  "N × name". Plus `Game::dev_plain_items() -> Vec<(ItemId, String, u32)>`
  (id, display name, held count) sorted by name, for the picker — a view,
  so app-core never filters `ItemDb`.
- Tests: count rises by qty; gear refused; unknown refused; list sorted
  and contains no gear.

### T3. Warp
- `Game::dev_warp_to_zone(zone) -> Result<String, String>`: refuses in a
  battle, else `warp_to_zone`. `Game::dev_warp_targets() -> Vec<u32>`:
  next 20 zones after the current, capped at the last zone if one exists.
- Tests: targets start at current+1, len ≤ 20, respect a cap; refusal in
  battle; a warp lands on the zone (use a `dev-saves/` template, not
  `Game::new`, if one is near the start).

### T4. Unlock research
- Extract the grant half of `settle_research` (after bill and subject
  checks) into `fn grant_research(&mut self, def: &ResearchDef)`:
  `Research`/`KnownRoutines` insert, log, notify, clear `ActiveResearch`
  if it is this node. `settle_research` calls it.
- `Game::dev_unlock_research(&id) -> Result<String, String>`: unknown or
  already granted → Err; else `grant_research`, no bill, no subject.
- Tests: a node with unmet prereqs and no materials is granted; the active
  node clears when unlocked; already-granted refused; a routine node lands
  in `KnownRoutines`; existing research tests pass unedited (the extract
  moved nothing).

### T5. Spawn companion
- `Game::dev_spawn_companion() -> Result<String, String>`: player tile →
  `habitat_pools(x, y, depth, 0)` with the depth the field spawner passes
  at `spawning.rs:1833`; `None` or empty → Err. Sort `.0` by id, draw with
  the seeded game RNG, adopt per the caravan call site above, Ok "<label>
  is yours".
- Tests: species is in that tile's pool; two games from one seed pick the
  same species; non-walkable tile refused; companion is on the roster.

Phase 1 gate: `cargo test -p feral-processes-engine` green, clippy clean.

## Phase 2: app-core

### T6. Keypad rows
- `DevAction::{ToggleFreeBuild, GiveItem, WarpZone, SpawnCompanion}` and
  their `DEV_ROWS` entries, after `CycleEnemyStrength`, before
  `OpenSpriteForge` (the openers stay together at the bottom: GiveItem,
  WarpZone, then sprite forge). The table is `&'static` and the spec wants
  the state in the label, so add `fn dev_row_label(&self, row) -> String`
  for the renderer: the static label for every row except free build,
  which appends "(on)"/"(off)".
- Tests (`src/tests/dev_console.rs`): each row fires its engine call and
  says the result; free-build label flips; the existing no-duplicate-action
  guard covers the new variants.

### T7. Pickers
- `Mode::DevItemPicker`, `Mode::DevZonePicker` in `lib.rs` (variants
  only); handlers in new `app/dev_item_picker.rs`,
  `app/dev_zone_picker.rs`; dispatch in `app/input.rs`; add both to every
  mode list the compiler doesn't force (grep `Mode::SpritePicker` for the
  lists — see memory `all-modes-does-not-fail-to-compile`).
- Opened from T6's rows; `menu_origin` = the keypad; Esc returns there.
  Selectors 1-9 then a-z over the current page; Up/Down scroll.
- Item: a pick calls `dev_give_item(id, 10)`, says it, stays open.
  Zone: a pick calls `dev_warp_to_zone`, says it, closes to the map.
- Tests: open/Esc round trip; pick gives 10 and stays; scroll exposes row
  36; zone pick warps and closes; a refusal is said and leaves the mode.

### T8. Research `U`
- In `handle_research_key`, beside `G`/`A`: `GameKey::Char('U')` when the
  dev console is enabled → target per the decision above →
  `dev_unlock_research`, `report`. Disabled → falls through (unbound).
- Tests: enabled + graph cursor on a blocked node → granted; disabled →
  nothing changes; list view with no active research → refusal said.

Phase 2 gate: `cargo test -p feral-processes-app-core` green.

## Phase 3: gui

### T9. Renderers and hint
- `render/dev_pickers.rs` (both pickers, one file — they share layout):
  `draw_popup` + `Painter` only; rows "sel  name  ×count" / "sel  Zone N".
  Wire in `render/mod.rs` beside `:1235`.
- Keypad renderer reads the free-build row's label through T6's function.
- Research hint (`draw_research_menu` and the graph view's hint) adds
  `[U] unlock` only when the dev console is enabled.
- Tests: whatever headless popup-width test exists for the keypad
  (`popup-row-width-is-testable-headlessly`) extended to both pickers.
  One screenshot per picker via `--keys` with `FERAL_DEV=1`; Read the PNG.

## Phase 4: gate and review

- `cargo test --workspace`, `cargo clippy --workspace --all-targets`.
  No `tuning/` change, so no `balance_sim` run.
- Mutation-check T1's save→load test and T4's extraction (insert the
  toggle into the save; drop the `ActiveResearch` clear) — each must fail.
- Whole-branch opus review against the spec's anti-goals (no save leak,
  every cheat calls real code, RNG over a sorted list), diff as a file.
- `CHANGELOG.md` entry under the next patch version at landing (deploy
  skill); no save-format change, so not breaking.
