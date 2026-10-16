# Floor Items and Stockpile Zones Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The player can drop and pick up single items, gear copies and downed programs on the base floor, one per tile. Stockpile zones are marked with the Excavation box. Idle programs tidy loose items onto them, and stockpiled plain items count as base stock.

**Architecture:** One `FloorItem(FloorThing)` entity per tile replaces `FloorPile`. Every set-down (a carrier's load, the player's drop, cleanup, the old-save migration) goes through `Game::set_down`. That is one breadth-first walking-distance search, grown out of `building.rs`'s `nearest_free_floor`, with a store fallback that never destroys anything. Stockpile cells are a `BTreeSet` field on `BaseGrid`, the precedent `finishes` set, so no new `Resource` is registered. Cleanup is a `drift_idle_staff` arm that carries a `Tidying(FloorThing)` component. Stockpiled plain items join the stock walk through one `stock::stockpiled` read.

**Tech Stack:** Rust workspace (bevy_ecs engine, app-core state machine, bevy+egui gui), RON assets and saves.

**Spec:** `docs/superpowers/specs/2026-10-16-floor-items-and-stockpiles-design.md`

## Global Constraints

- A floor tile holds **at most one** floor item, and that item is **one unit**. No code path leaves two on a tile or a quantity above 1.
- **An item is never destroyed** to make room or to free a body (`seams-base.md`, "A carrier's load is never destroyed").
- Glyphs come from `assets/items/*.ron`. No content is hardcoded in Rust.
- Floor items never block movement or building placement. Bodies walk over them, and `blocked_tiles`/`placement_refusal` are untouched.
- Floor items and stockpiles exist in base space only.
- The new `ItemDef` field, the new `SaveData` field and every other new save field carry `#[serde(default)]`. `SAVE_FORMAT_VERSION` stays 37, and dev-saves are not recaptured.
- Determinism: no RNG anywhere in this feature. Each search ties in `world::NEIGHBOURS` order, and each list is sorted `(distance, x, y)` or `(x, y)`.
- New entities and components can shift query iteration order and seed-luck tests (memories `new-resource-shifts-query-iteration-order`, `rng-stream-shift-exposes-seed-luck-tests`). If an unrelated test fails, read its code path and fix the fixture. Never re-run it hoping for a pass.
- Keys in base-space playing mode (checked against `crates/app-core/src/app/playing.rs`):
  - `z`: stockpile zone mode (free).
  - `o`: pick up. The spec's `g` is the world map at playing.rs:444.
  - `y`: put down. The spec's `t` is trade at playing.rs:256.

  All three are lowercase map actions. `o` is also the Stack's adopt-orphan key, so it must be bound **after** the `is_underground()` hand-off.
- Branch commits are unversioned: no CHANGELOG section and no version bump. Do not touch `docs/manual.md`, `README.md` or `TODO.md`.
- Every commit step: `cargo fmt`, `cargo clippy --workspace --all-targets` with zero warnings, and the targeted `cargo test -p <crate> <filter>`. Executors never `git push`, `git checkout`, `git stash`, `git reset` or `git add -A`/`.`. Stage explicit paths.

## Review Focus

1. **A base with no reachable free floor.** A load dropped in a full pocket, or on a body standing off-floor, lands in the player's stores with a log line, and nothing is lost. Tested in Task 3, `set_down_with_no_free_floor_puts_items_in_the_pack_and_says_so`.
2. **A drop beside a wall.** Items never land behind a wall, even when the far side is nearer by Chebyshev. Tested in Task 2 (`nearest_free_cells_never_reaches_past_what_cannot_be_crossed`) and Task 3 (`a_drop_beside_a_wall_never_lands_behind_it`).
3. **An old save whose pile cannot fit its pocket.** The pile spreads, the overflow goes to the pack, and the total is conserved. Tested in Task 4, `an_old_pile_too_big_for_its_pocket_spills_into_the_pack_on_load`.
4. **Two tidiers racing for one stockpile cell.** The second re-targets and the cell never holds two items. Tested in Task 8, `two_tidiers_never_stack_one_stockpile_cell`.
5. **A save taken mid-carry.** A tidier's item survives save and load. Tested in Task 8, `a_tidiers_item_survives_save_and_load`.

## Spec-vs-code notes (plan follows the code)

These were checked against the source. None of them changes what gets built.

- **Callers:** "eight callers" of `drop_load` is really 15 call sites in 11 files: trade, sortie, reinforcement, work_orders ×2, study, party ×3, siphon, hauling, upkeep, turn and building ×2.
- **Keys:** `g` and `t` are taken, so the plan uses `o` and `y` (see Global Constraints).
- **Search start:** `nearest_free_floor` never returns its own `from`. The grown helper tests `from` first, which `displace_bodies_from` cannot observe because its `free` closure rejects the footprint.
- **`drop_load` becomes a `Game` method.** It is `fn(&mut World, Entity)` today, and its only `World`-only caller is `haul_step_system`'s stranded set-down (`commands.queue`). The fallback rungs (`rack_room`, `downed_program_label`, `log_base`) are `Game` methods, so that one caller moves to a `Game` pass, `set_down_stranded_loads`, run right after `schedule.run`. Its timing is unchanged: same tick, same `Stranded::since`.
- **Rack rung:** the spec says "Quarantine Rack hopper", but a rack's shelf is `components::Racked` (`Hopper` belongs to the rig). The rung uses `Racked` and `Game::rack_room`, as `return_carried_program` does.
- **Storage location:** stockpile cells are stored on `BaseGrid` and ride `SaveData::base_grid` rather than a second new `SaveData` field. This is allowed by the spec's "storage shape is the builder's".
- **Saving the carried item:** a tidier's carried item needs a third save field, `CreatureSave::tidying`, or a save mid-carry destroys it.
- **Downed-program fallback:** for a program whose body is leaving (a `drop_load` path), "refused" is not available. When no tile, no store room and no rack room remain, it goes into `DownedPrograms` **past the cap**, with a log line, because "never destroyed" outranks the cap. The player's own drop is refused before anything moves, so that path never reaches the fallback.

## File structure

| File | Responsibility |
|---|---|
| `crates/engine/src/items.rs` | `FloorThing` value enum |
| `crates/engine/src/items_db.rs` | `ItemDef::glyph`, `floor_glyph()`, `DEFAULT_FLOOR_GLYPH` |
| `crates/engine/src/components.rs` | `FloorItem` (replaces `FloorPile`), `Tidying` |
| `crates/engine/src/game/base/floor.rs` | set-down search, the single spawn writer, take, `Game::set_down`/`drop_load`, views, saves |
| `crates/engine/src/game/base/tidy.rs` (new) | cleanup errand: `TidyBoard`, `may_tidy`, `tidy_step`, `set_down_posted_tidiers` |
| `crates/engine/src/game/base/floor_hands.rs` (new) | player `put_down_options` / `put_down` / `pick_up` |
| `crates/engine/src/base_grid.rs` | stockpile set |
| `crates/engine/src/game/base_space.rs` | `toggle_stockpile_box`, `stockpile_cells`, examine |
| `crates/engine/src/game/base/stock.rs` | `stockpiled` read; spend and stock-block tails |
| `crates/engine/src/game/base/construction.rs` | `Source::Stockpile` |
| `crates/engine/src/save.rs`, `game/lifecycle.rs` | `FloorItemSave`, legacy `floor_piles`, `CreatureSave::tidying` |
| `crates/app-core/src/app/excavate.rs` | shared `box_cursor_key` |
| `crates/app-core/src/app/stockpile.rs` (new) | `Mode::Stockpile` |
| `crates/app-core/src/app/floor_hands.rs` (new) | `Mode::PutDown`, `o`/`y` |
| `crates/gui/src/render/base.rs` | glyph draw, stockpile wash, header |
| `crates/gui/src/render/put_down.rs` (new) | put-down popup |

---

### Task 1: Per-item floor glyphs as data

**Files:**
- Modify: `crates/engine/src/items_db.rs:123` (`ItemDef`), `:592` and `:676` (the two synthesised literals)
- Modify: every `assets/items/*.ron` (82 files)
- Modify: `assets/items/README.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces:
  - `ItemDef.glyph: Option<char>` (`#[serde(default)]`)
  - `pub fn ItemDef::floor_glyph(&self) -> char`
  - `pub const DEFAULT_FLOOR_GLYPH: char = '*'` in `items_db.rs`

- [ ] **Step 1: Write the failing census tests** in `tests/assets.rs`:

```rust
const FLOOR_GLYPHS: [char; 8] = ['.', '*', ',', '"', '\'', '`', '~', ':'];

#[test]
fn every_shipped_item_authors_a_floor_glyph_from_the_generic_set() {
    let game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.item_defs() { // includes the synthesised disks and carriers
        let g = def.glyph.unwrap_or_else(|| panic!("{} authors no glyph", def.id));
        assert!(FLOOR_GLYPHS.contains(&g), "{} glyph {g:?}", def.id);
    }
}

#[test]
fn an_item_without_a_glyph_floors_as_the_default() {
    let def: ItemDef = ron::from_str(r#"(id: "x", name: "X")"#).unwrap();
    assert_eq!(def.floor_glyph(), DEFAULT_FLOOR_GLYPH);
}
```

- [ ] **Step 2: Run** `cargo test -p feral-processes-engine floor_glyph`. Expected: compile FAIL (no `glyph`).
- [ ] **Step 3: Add the field and accessor.** Put `glyph` after `abbrev`. Its doc says what it is for (the base-floor mark) and that it is optional. `floor_glyph` returns `self.glyph.unwrap_or(DEFAULT_FLOOR_GLYPH)`. The etched-disk and tool-carrier literals get `glyph: Some(',')` with a one-line why.
- [ ] **Step 4: Author a glyph in each RON file**, after `name`. Apply the first rule that matches:

  | rule | glyph |
  |---|---|
  | `role: Some(_)` (currencies) | `.` |
  | `trap: Some(..)` | `` ` `` |
  | `consume: Some(..)` | `~` |
  | `equipment` Weapon | `'` (RON: `Some('\'')`) |
  | `equipment` Armor | `"` (RON: `Some('"')`) |
  | `equipment` Module | `:` |
  | `routine_disk.ron` | `,` |
  | everything else | `*` |

- [ ] **Step 5: Document it in `assets/items/README.md`.** Add a `glyph` block to the schema, in the house style: optional, one character, the set above as suggestions, a default of `*`, and a note that a downed program is drawn by the game and never by an item.
- [ ] **Step 6: Run** `cargo test -p feral-processes-engine assets` and expect PASS. Then run fmt and clippy.
- [ ] **Step 7: Commit:** `git add crates/engine/src/items_db.rs crates/engine/src/tests/assets.rs assets/items && git commit -m "Items: a floor glyph per item, as data"`

---

### Task 2: One set-down search

**Files:**
- Modify: `crates/engine/src/game/base/floor.rs` (receives the helper)
- Modify: `crates/engine/src/game/base/building.rs:425` (`displace_bodies_from`), `:2006` (delete `nearest_free_floor`)
- Test: `crates/engine/src/tests/floor.rs`

**Interfaces:**
- Produces: `pub(crate) fn nearest_free_cells(from: (i32, i32), count: usize, free: impl Fn((i32, i32)) -> bool, crossable: impl Fn((i32, i32)) -> bool) -> Vec<(i32, i32)>` in `floor.rs`.
  - Returns up to `count` cells.
  - `from` is first if `free(from)`, then cells in breadth-first order over `world::NEIGHBOURS`, expanding only through `crossable` cells (`from` itself is always expanded).
  - One search, stopping as soon as `count` cells are found.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn nearest_free_cells_takes_the_drop_point_first_then_walks_outward() {
    let corridor = |c: (i32, i32)| c.1 == 0 && (0..5).contains(&c.0);
    assert_eq!(nearest_free_cells((2, 0), 3, corridor, corridor), vec![(2, 0), (1, 0), (3, 0)]);
}

#[test]
fn nearest_free_cells_never_reaches_past_what_cannot_be_crossed() {
    let free = |c: (i32, i32)| c == (2, 0) || c == (0, 2);
    let crossable = |c: (i32, i32)| c.0 == 0 && (0..=2).contains(&c.1);
    assert_eq!(nearest_free_cells((0, 0), 2, free, crossable), vec![(0, 2)]);
}
```

- [ ] **Step 2: Run** `cargo test -p feral-processes-engine nearest_free_cells`. Expected: compile FAIL.
- [ ] **Step 3: Move `nearest_free_floor`'s body into `nearest_free_cells`.** Add the `from` check and the count, and keep the "grid is finite" doc. `displace_bodies_from` calls `nearest_free_cells(.., 1, ..).first().copied()`. Delete `nearest_free_floor`.
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine nearest_free_cells` and `cargo test -p feral-processes-engine displace`. Expected: PASS, and the existing displacement tests stay green.
- [ ] **Step 5: Commit:** `git add crates/engine/src/game/base/floor.rs crates/engine/src/game/base/building.rs crates/engine/src/tests/floor.rs && git commit -m "Floor: grow the displacement search into the shared set-down search"`

---

### Task 3: The floor-item model and `Game::set_down` (additive, beside `FloorPile`)

**Files:**
- Modify: `crates/engine/src/items.rs` (after `DownedProgram`, ~393)
- Modify: `crates/engine/src/components.rs:953` (add beside `FloorPile`; the deletion is Task 4)
- Modify: `crates/engine/src/game/base/floor.rs`
- Modify: `crates/engine/src/tests/support.rs` (helpers)
- Test: `crates/engine/src/tests/floor.rs`

**Interfaces:**
- Consumes: `nearest_free_cells` (Task 2), `Game::structure_tiles`, `Game::build_site_tiles`, `Game::blocked_tiles`, `Game::rack_room(&self, Entity) -> u32`, `Game::downed_program_label`, `Game::copy_name`, `Game::item_name`, `Game::log_base`.
- Produces:
  - `#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)] pub enum FloorThing { Item(ItemId), Gear(GearCopy), Program(DownedProgram) }` in `items.rs`.
  - `#[derive(Component, Clone, Debug)] pub struct FloorItem(pub FloorThing)` in `components.rs`. It has no `Glyph`; the doc carries `FloorPile`'s "no Glyph" reason plus the one-per-tile rule.
  - `floor.rs`:
    - `pub(crate) fn spawn_floor_item(world: &mut World, pos: Position, thing: FloorThing) -> Entity`, the single writer, with `debug_assert!` that the tile is empty.
    - `pub(crate) fn floor_item_at(world: &mut World, pos: Position) -> Option<Entity>`
    - `pub(crate) fn take_floor_item(world: &mut World, e: Entity) -> Option<FloorThing>`, which despawns.
    - `pub(crate) fn take_floor_unit(world: &mut World, e: Entity, item: &ItemId) -> bool`, which despawns only an `Item(item)`.
  - `pub(crate) struct FreeFloor { taken: HashSet<(i32, i32)>, walls: Occupancy }` with:
    - `fn free(&self, grid: &BaseGrid, cell: (i32, i32)) -> bool`: `is_floor && walkable && !taken`. `taken` is `structure_tiles ∪ build_site_tiles ∪ floor-item cells`.
    - `fn crossable(&self, grid: &BaseGrid, cell: (i32, i32)) -> bool`: `walkable && !walls.walls(cell)`.
  - `impl Game`:
    - `pub(crate) fn free_floor(&mut self) -> FreeFloor`
    - `pub(crate) fn free_floor_cells(&mut self, from: Position, count: usize) -> Vec<(i32, i32)>`
    - `pub(crate) fn set_down(&mut self, from: Position, things: Vec<FloorThing>)`
    - `fn stow(&mut self, thing: FloorThing, from: Position)`
  - `set_down` runs **one** search for `things.len()` cells, places in order, and passes the rest to `stow`. The `stow` rungs, each with one `log_base` line:
    - Item → `Inventory`
    - Gear → `GearCopies::add(copy, 1)`
    - Program → `DownedPrograms` if under `MAX_DOWNED_PROGRAMS`, else the nearest `Racked` rack by `(chebyshev, x, y)` with `rack_room > 0`, else `DownedPrograms` past the cap.
  - `support.rs` test helpers:
    - `pub(crate) fn lay_floor_box(game: &mut Game, a: (i32, i32), b: (i32, i32))`
    - `pub(crate) fn floor_things(game: &mut Game) -> Vec<((i32, i32), FloorThing)>`, sorted.

- [ ] **Step 1: Write the failing tests** in `tests/floor.rs`:

```rust
#[test]
fn set_down_spreads_one_unit_per_tile_nearest_first() {
    let mut g = game();
    lay_floor_box(&mut g, (0, 0), (4, 0));
    let scrap = FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT));
    g.set_down(Position { x: 2, y: 0 }, vec![scrap.clone(); 3]);
    let cells: Vec<_> = floor_things(&mut g).into_iter().map(|(c, _)| c).collect();
    assert_eq!(cells, vec![(1, 0), (2, 0), (3, 0)]);
}

#[test]
fn set_down_skips_a_tile_that_already_holds_an_item_or_a_structure() {
    let mut g = game();
    lay_floor_box(&mut g, (0, 0), (3, 0));
    let scrap = FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT));
    g.set_down(Position { x: 0, y: 0 }, vec![scrap.clone()]);
    spawn_structure_at(&mut g, "wall", 1, 0); // anchors a wall: not free and not crossable
    g.set_down(Position { x: 0, y: 0 }, vec![scrap.clone()]);
    assert_eq!(floor_things(&mut g).len(), 1, "nothing past the wall is reachable");
    assert_eq!(held(&g, &ItemId::from(ids::CORE_FRAGMENT)), before + 1); // `before` read at the top
}

#[test]
fn a_drop_beside_a_wall_never_lands_behind_it() { /* floor (0,0),(0,1),(2,0); wall at (1,0) and (1,1); drop 2 at (0,0); assert (2,0) empty */ }

#[test]
fn set_down_with_no_free_floor_puts_items_in_the_pack_and_says_so() {
    let mut g = game(); // empty BaseGrid: nothing is floor
    let before = held(&g, &ItemId::from(ids::CORE_FRAGMENT));
    g.set_down(Position { x: 5, y: 5 }, vec![FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT)); 2]);
    assert_eq!(held(&g, &ItemId::from(ids::CORE_FRAGMENT)), before + 2);
    assert!(g.message_history(5).iter().any(|e| e.text.contains("goes into your pack")));
}

#[test]
fn a_program_with_no_floor_goes_to_the_downed_store_then_a_rack() { /* fill DownedPrograms to MAX, place a racks-capable structure, assert Racked grew by 1 */ }
```

- [ ] **Step 2: Run** `cargo test -p feral-processes-engine set_down`. Expected: compile FAIL.
- [ ] **Step 3: Implement the interfaces above.**
  - Log copy:
    - item or gear: `"No free floor for {name} — it goes into your pack."`
    - program: `"… — it goes to your downed store."` / `"… — it goes onto the Quarantine Rack."` / `"… — it squeezes into your downed store past its limit."`
  - Each doc states the why. `FreeFloor` is "the one predicate for 'an item may be set down here'", and bodies don't make a tile unfree.
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine floor`. Expected: PASS.
- [ ] **Step 5: Commit:** `git add crates/engine/src/items.rs crates/engine/src/components.rs crates/engine/src/game/base/floor.rs crates/engine/src/tests/support.rs crates/engine/src/tests/floor.rs && git commit -m "Floor: one item per tile and the one set-down path"`

---

### Task 4: Swap every pile path onto floor items, and delete `FloorPile`

This is the largest gate. Every `FloorPile` reader changes in one commit, because the component is deleted.

**Files:**
- Modify: `crates/engine/src/game/base/floor.rs` (delete `spawn_floor_pile`/`floor_pile_at`/`take_from_pile`/free `drop_load`)
- Modify: `crates/engine/src/components.rs:941-955` (delete `FloorPile`; update `Carrying` doc, `:1237` doc)
- Modify: the `drop_load` call sites (`rg -n 'drop_load' crates/engine/src`):
  - `game/trade.rs:638`
  - `game/sortie.rs:293`
  - `game/reinforcement.rs:139`
  - `game/base/work_orders.rs:1491,1503`
  - `game/base/study.rs:319`
  - `game/party.rs:1154,1734,1735`
  - `game/base/siphon.rs:67`
  - `game/base/upkeep.rs:1096`
  - `game/turn.rs:728`
  - `game/base/building.rs:1588,1868`
- Modify: `crates/engine/src/game/base/hauling.rs`:
  - `:808`, the `piles` query → `(Entity, &Position, &FloorItem), Without<Tamed>`
  - `:877`, `Errand::Pickup`
  - `:1027`, `pickup_errand`
  - `:1484`, the take
  - `:1632-1650`, the stranded branch loses its `commands.queue`
- Modify: `crates/engine/src/game/turn.rs:~357` (call `set_down_stranded_loads` after `schedule.run`)
- Modify: `crates/engine/src/views.rs:1437` (`FloorPileView` → `FloorItemView`, `FloorLook`)
- Modify: `crates/engine/src/game/base_space.rs:336` (examine)
- Modify:
  - `crates/engine/src/save.rs:1407,1604,2414`
  - `crates/engine/src/game/lifecycle.rs:1930,3333`
  - `crates/engine/src/tests/spawning.rs:1800`
- Modify: `crates/engine/src/tuning.rs:2653`, `crates/engine/src/rooms.rs:359` (doc words)
- Modify (minimal, to compile): `crates/gui/src/render/base.rs:761` (`floor_items()`), `:6116-6130` (fixture pushes `FloorItemSave`)
- Test:
  - `tests/floor.rs`
  - the pile assertions in `tests/{party.rs:2742, drop_pods.rs:546, sorties.rs:732, hauling.rs:2108,2177,2386, building.rs:1432, trade.rs:1312}`

**Interfaces:**
- Consumes: Task 3's model.
- Produces:
  - `impl Game`:
    - `pub(crate) fn drop_load(&mut self, who: Entity)`: takes `Carrying` and sets its `qty` units down from `who`'s tile in one `set_down`.
    - `pub(crate) fn set_down_stranded_loads(&mut self)`: every `(Carrying, Stranded)` with `tick - since >= STRANDED_SET_DOWN_TICKS` → `drop_load`.
    - `pub fn floor_items(&self) -> Vec<FloorItemView>`: empty outside base space, `pos`-sorted.
    - `pub(crate) fn describe_floor_item(&self, x: i32, y: i32) -> Option<String>`
    - `pub(crate) fn floor_item_saves(&mut self) -> Vec<FloorItemSave>`
    - `pub(crate) fn restore_floor_items(&mut self, items: Vec<FloorItemSave>, legacy: Vec<FloorPileSave>)`
  - `views.rs`:
    - `pub struct FloorItemView { pub pos: (i32, i32), pub look: FloorLook, pub name: String }`
    - `pub enum FloorLook { Item(char), Program }`. Gear uses its item's `floor_glyph()`.
  - `save.rs`:
    - `#[derive(Serialize, Deserialize)] pub struct FloorItemSave { pub position: (i32, i32), pub thing: FloorThing }`
    - `SaveData::floor_items: Vec<FloorItemSave>` (`#[serde(default)]`)
    - `SaveData::floor_piles` becomes `#[serde(default, skip_serializing)]` (legacy, load-only), with a doc saying so.
  - `hauling.rs`: `Errand::Pickup { thing: Entity, at: Position, item: ItemId }`, with `want` removed. The take is `take_floor_unit` → `Carrying { item, qty: 1 }`. `pickup_errand` considers only `FloorThing::Item` and returns one unit.
  - Restore: every saved floor item and every legacy unit (`qty` × `Item`) is set down **from its own position** through `set_down`. A free saved tile lands exactly, a legacy pile spreads, and an overflow is stowed. This happens at lifecycle.rs's existing line, after structures (and therefore after the grid and build sites).
  - Examine copy:
    - item: `"{name} on the floor"`
    - gear: `"{copy_name} on the floor"`
    - program: `"{label}, downed, on the floor"`

    Joined after the finish with `"; "` and capitalised, as today.

- [ ] **Step 1: Write the failing tests** in `tests/floor.rs`. Replace the pile tests:

```rust
#[test]
fn a_dropped_load_lands_one_unit_per_tile_and_ends_the_hold() {
    let mut g = game_with_home(); // pocket floored
    let who = loaded_staff(&mut g, 6, 6); // LOAD = 3
    g.drop_load(who);
    assert!(g.world.get::<Carrying>(who).is_none());
    let things = floor_things(&mut g);
    assert_eq!(things.len(), LOAD as usize);
    assert!(things.iter().any(|(c, _)| *c == (6, 6)), "its own tile first");
    assert!(things.iter().all(|(_, t)| *t == FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT))));
}

#[test]
fn floor_items_of_every_kind_survive_save_and_load() { /* Item, Gear(a GearCopy), Program(a DownedProgram) at three cells; save → Game::load; floor_things equal */ }

#[test]
fn an_old_pile_spreads_one_unit_per_tile_on_load() { /* save; load_from_file; push FloorPileSave{(6,6), [(core_fragment,4)]}; save_to_file; Game::load; 4 items, 4 distinct cells */ }

#[test]
fn an_old_pile_too_big_for_its_pocket_spills_into_the_pack_on_load() { /* bare game, lay_floor_box (0,0)-(1,0); legacy pile 10 at (0,0); after load: 2 on floor, pack +10-2 */ }

#[test]
fn a_save_without_floor_items_loads_with_none() { /* strip "floor_items:[]," from ron text; parsed.floor_items.is_empty() */ }

#[test]
fn examining_a_floor_item_names_it() { /* "Core Fragment on the floor" at the start of the line */ }

#[test]
fn examining_a_downed_program_on_the_floor_names_it() { /* contains ", downed, on the floor" */ }
```

  In `tests/hauling.rs`, add `a_hauler_lifts_one_unit_per_pickup`: three `Item` units on three tiles, one depot, and after the first pickup `Carrying.qty == 1`. Rewrite each listed pile assertion to "a `FloorThing::Item(item)` on the carrier's tile and `LOAD` units in `floor_things`".
- [ ] **Step 2: Run** `cargo test -p feral-processes-engine floor`. Expected: compile FAIL.
- [ ] **Step 3: Implement the interfaces above.**
  - Every call site becomes `self.drop_load(e)`.
  - In `haul_step_system`'s stranded branch, keep `continue` and drop only the queued call. Update its comment to name the `Game` pass.
  - Update every doc that says "pile" or "merges" to the one-per-tile rule.
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine floor`, then `hauling`, then `cargo test -p feral-processes-engine` (whole crate) and `cargo test -p feral-processes-gui floor`. Expected: PASS. Any unrelated failure gets the RNG/iteration-order reading in Global Constraints.
- [ ] **Step 5: Save mutation check.** Temporarily change `floor_items` to `#[serde(skip)]`, confirm `floor_items_of_every_kind_survive_save_and_load` fails, then restore it.
- [ ] **Step 6: Commit.** Stage the explicit paths listed in Files. Message: `Floor: loads spread one unit per tile; FloorPile is gone`.

---

### Task 5: Draw floor items by their glyph

**Files:**
- Modify: `crates/gui/src/render/base.rs:104-107` (delete `FLOOR_PILE_MARK`), `:760` (map), `:1003-1007` (draw), `:6105-6165` (tests)
- Modify: `crates/gui/src/render/tactical.rs:777`

**Interfaces:**
- Consumes: `Game::floor_items`, `FloorLook` (Task 4).
- Produces:
  - `pub(crate) const FALLEN_MARK: (char, Color) = ('x', palette::FALLEN)` in `tactical.rs`, which tactical's fallen loop draws.
  - `pub(crate) fn floor_item_mark(look: &FloorLook) -> (char, Color)` in `base.rs`: `Item(c)` → `(c, hud::palette::WARN)`, `Program` → `FALLEN_MARK`.

- [ ] **Step 1: Write the failing tests** in base.rs's test module:
  - `a_floor_item_draws_its_items_glyph_in_base_space`: push `FloorItemSave { thing: Item(core_fragment) }` and count painted `'.'` glyphs at the item's colour WARN, comparing with and without the item. Use the colour so the floor's own marks cannot alias the glyph.
  - `a_downed_program_on_the_floor_draws_the_fallen_mark`: painted `"x"` in `palette::FALLEN`.
- [ ] **Step 2: Run** `cargo test -p feral-processes-gui floor_item`. Expected: FAIL.
- [ ] **Step 3: Implement.** The map's per-tile set becomes `HashMap<(i32, i32), (char, Color)>`, and the draw keeps "under programs and structures, above the floor" (`ch.is_none()`).
- [ ] **Step 4: Run** `cargo test -p feral-processes-gui floor` and `cargo test -p feral-processes-gui fallen`. Expected: PASS.
- [ ] **Step 5: Commit:** `git add crates/gui/src/render/base.rs crates/gui/src/render/tactical.rs && git commit -m "Base map: a floor item draws its own glyph, a downed program the fallen x"`

---

### Task 6: The player picks up and puts down

**Files:**
- Create: `crates/engine/src/game/base/floor_hands.rs` (register in `game/base/mod.rs`)
- Modify: `crates/engine/src/views.rs`
- Create: `crates/app-core/src/app/floor_hands.rs` (register in `app/mod.rs`)
- Modify:
  - `crates/app-core/src/lib.rs` (`Mode::PutDown` beside `Mode::ExcavateBrush`, plus every exhaustive mode list: `rg -n 'Mode::ExcavateBrush' crates`)
  - `crates/app-core/src/app/input.rs:310-336` (dispatch), `:196` (opening row = 0, the default)
  - `crates/app-core/src/app/playing.rs` (`o` in the surface/base `acted` match; `y` after the `g` world-map check at :444)
- Create: `crates/gui/src/render/put_down.rs`
- Modify: `crates/gui/src/render/mod.rs`:
  - draw arm beside `Mode::ExcavateBrush` (:1191)
  - `ALL_MODES` 152→153 (:1677)
- Test:
  - `crates/engine/src/tests/floor.rs`
  - `crates/app-core/src/tests/floor_hands.rs` (new, register in `tests/mod.rs`)
  - the gui census (existing)

**Interfaces:**
- Consumes: `Game::free_floor_cells`, `spawn_floor_item`, `take_floor_item`, `floor_item_at` (Tasks 3–4).
- Produces:
  - `views.rs`:
    - `pub enum PutDownChoice { Item(ItemId), Gear(GearCopy), Program(usize) }`
    - `pub struct PutDownOption { pub label: String, pub choice: PutDownChoice }`
  - `impl Game`:
    - `pub fn put_down_options(&self) -> Vec<PutDownOption>`
      - Rows: `Inventory` rows with qty > 0 and not `banked` (`"{name} x{qty}"`), then `GearCopies` (`"{copy_name} x{qty}"`), then `DownedPrograms` in index order (`"{label}"` capitalised).
    - `pub fn put_down(&mut self, choice: &PutDownChoice) -> Result<(), String>`
      - Guards, in order:
        - `"Not right now."` (game over or battle)
        - `require_base()`
        - one free cell from `free_floor_cells(party, 1)`, else `Err("There's no free floor in reach to put that down on.")`
        - the unit still held, else `Err("That's no longer in your pack.")`

        Nothing moves before every guard passes.
      - Then it takes one unit, spawns, logs `"You put down {name}."` and calls `self.tick()`.
    - `pub fn pick_up(&mut self) -> Result<(), String>`
      - Guards: the same first two, then `Err("There's nothing here to pick up.")`, then a program at `MAX_DOWNED_PROGRAMS` → `Err("Your downed store is full — extract or rack one first.")`. The item stays.
      - Then it stores the thing, logs `"You pick up {name}."` and ticks.
  - `App`:
    - `pub(crate) fn open_put_down(&mut self)`
      - Outside base: `refuse("Nothing to put down out here — the floor is through the anchor.")`
      - No rows: `refuse("You're carrying nothing to put down.")`
      - Otherwise `Mode::PutDown`, `menu_selected = 0`.
    - `pub(crate) fn handle_put_down_key(&mut self, key: GameKey)`
      - Esc → Playing.
      - `selected_index` → `game.put_down(&choice)`. `Ok` → Playing plus `after_world_action(true, false, 0)`. `Err` → `refuse`, staying in the mode.
    - `pub fn put_down_rows(&self) -> Vec<PutDownOption>`: the one list keys and gui read.
  - `o` in the `acted` match: `match game.pick_up() { Ok(()) => true, Err(r) => { refusal = Some(r); false } }`.

- [ ] **Step 1: Write the failing engine tests:**
  - `putting_down_takes_one_unit_onto_the_players_tile` (pack 3 → 2, item at party cell)
  - `putting_down_overflows_to_the_nearest_free_tile_when_underfoot_is_taken`
  - `putting_down_with_no_free_floor_moves_nothing` (`Err`, pack unchanged)
  - `picking_up_returns_the_thing_underfoot_to_its_store` (one each for Item, Gear and Program)
  - `picking_up_a_program_at_the_cap_is_refused_and_leaves_it` (Review Focus)
  - `nothing_underfoot_refuses_pick_up`
- [ ] **Step 2: Write the failing app-core tests:**
  - `y_opens_the_put_down_picker_and_a_row_puts_it_down`
  - `y_with_an_empty_pack_refuses`
  - `o_picks_up_underfoot`
  - `y_on_the_zone_surface_refuses`
- [ ] **Step 3: Run** `cargo test -p feral-processes-engine putting_down picking_up` and `cargo test -p feral-processes-app-core floor_hands`. Expected: FAIL.
- [ ] **Step 4: Implement the engine half, then app-core, then gui.**
  - gui `draw_put_down(rows: &[PutDownOption], selected, refusal, painter, m)` copies `draw_excavate_brush`'s shape: title `"Put down"`, one hint row (`"Pick what to set on the floor. Esc to go back."`), and `[{menu_shortcut(i)}] {label}` item rows.
  - Keep the comments that say why at each key arm: `o` sits after the Stack hand-off because the Stack's `o` adopts an orphan, and `y` because `t` is trade.
- [ ] **Step 5: Run** the two filters above plus `cargo test -p feral-processes-gui every_screen`. Expected: PASS.
- [ ] **Step 6: Commit.** Stage the listed paths. Message: `Base: the player puts things down and picks them up`.

---

### Task 7: Stockpile zones

**Files:**
- Modify: `crates/engine/src/base_grid.rs:77` (field), methods beside `set_finish` (:206)
- Modify: `crates/engine/src/game/base_space.rs:547` (beside `toggle_mark_box`), `:336` (examine)
- Create: `crates/app-core/src/app/stockpile.rs`
- Modify: `crates/app-core/src/app/excavate.rs` (extract the shared cursor)
- Modify:
  - `crates/app-core/src/lib.rs` (`Mode::Stockpile` plus exhaustive lists; the docs of `excavate_cursor`/`excavate_anchor` now name both box modes)
  - `crates/app-core/src/app/input.rs`
  - `crates/app-core/src/app/playing.rs` (`z` in the top match, beside `m` at :243)
- Modify: `crates/gui/src/render/base.rs`:
  - `:273-292` (plan gate and header)
  - `:1585` and `:1790` (`draw_excavation_plan` gains the stockpile wash)
  - `:55-68` (wash consts)
- Modify: `crates/gui/src/render/hud/palette.rs` (`STOCKPILE`)
- Modify: `crates/gui/src/render/mod.rs` (`ALL_MODES` 153→154, `NEEDS_PENDING_STATE` 28→29, beside `Mode::Excavate`)
- Test:
  - `crates/engine/src/tests/stockpiles.rs` (new)
  - `crates/app-core/src/tests/stockpile.rs` (new)
  - gui tests in base.rs

**Interfaces:**
- Produces:
  - `BaseGrid`:
    - `#[serde(default)] stockpiles: BTreeSet<(i32, i32)>`, doc'd as a fourth mark beside `finishes` and never in `cells`
    - `pub fn is_stockpile(&self, x: i32, y: i32) -> bool`
    - `pub(crate) fn set_stockpile(&mut self, x: i32, y: i32, on: bool) -> bool`: refuses marking a non-floor cell, and clearing always succeeds.
    - `pub fn stockpile_cells(&self) -> impl Iterator<Item = (i32, i32)> + '_`, in `(x, y)` order.
  - `impl Game`:
    - `pub fn toggle_stockpile_box(&mut self, a: (i32, i32), b: (i32, i32))`: the anchor decides, `toggle_mark_box`'s rule. If `a` is a stockpile, the box clears; otherwise it marks laid floor only. The box is normalised.
    - `pub fn stockpile_cells(&self) -> Vec<(i32, i32)>`: empty outside base space.
  - Examine appends `"a stockpile"` to the underfoot list after the floor item.
  - In `excavate.rs`:
    - `pub(crate) enum BoxKey { Commit { anchor: (i32, i32), corner: (i32, i32) }, Done, Unhandled(GameKey) }`
    - `pub(crate) fn box_cursor_key(&mut self, key: GameKey) -> BoxKey`

    `box_cursor_key` handles Esc (anchor first, then the mode), the party leaving base, the arrows/`hjkl` clamp, and Space/Enter (anchor, then commit). `handle_excavate_key` keeps its behaviour through it, with `F` arriving as `Unhandled`.
  - `App::handle_stockpile_key(&mut self, key: GameKey)`: `Commit` → `game.toggle_stockpile_box(anchor, corner)`.
  - `z` (top match): with base_pos, set `excavate_cursor = Some(party)` and `excavate_anchor = None`, then `Mode::Stockpile`. Outside base: `refuse("Nowhere to mark a stockpile out here — the floor is through the anchor.")`.
  - gui:
    - `palette::STOCKPILE: Color = rgb(0x5f8a5a)`, with a doc stating why it is not `PLAN` (a plan is work ordered; a stockpile is a standing zone)
    - `STOCKPILE_FILL` at 0.18 alpha off it
    - `draw_excavation_plan(painter, stockpiles: &[(i32, i32)], marked, plan, at, tile_px, pane)` draws stockpile fills first.
    - The plan-cursor gate and header include `Mode::Stockpile`, header `"Stockpile zone — Space marks a box, Esc leaves"`.

- [ ] **Step 1: Write the failing engine tests:**

```rust
#[test]
fn a_box_marks_only_laid_floor_as_stockpile() { /* lay (0,0)-(1,0); toggle (0,0)-(2,0); stockpile_cells == [(0,0),(1,0)] */ }
#[test]
fn a_box_anchored_on_a_stockpile_clears_it() { /* mark, then toggle from a marked anchor; empty */ }
#[test]
fn stockpile_marks_and_dig_marks_are_independent() { /* mark both on a floor cell; clear stockpile; marked_cells unchanged */ }
#[test]
fn stockpile_cells_survive_save_and_load() { /* save → Game::load → equal */ }
#[test]
fn examining_a_stockpile_says_so() { /* contains "a stockpile" */ }
#[test]
fn a_stockpile_cell_built_over_stays_marked_but_takes_nothing() { /* mark (1,0); spawn_structure_at wall (1,0); is_stockpile true; set_down from (1,0) does not land on (1,0) */ }
```

- [ ] **Step 2: Write the failing app-core tests:**
  - `z_opens_the_stockpile_cursor_on_the_party`
  - `two_spaces_mark_a_stockpile_box_and_spend_no_tick`
  - `esc_takes_the_anchor_then_the_mode`
  - `z_on_the_zone_surface_refuses`

  Every test in `tests/excavate.rs` must stay green unchanged.
- [ ] **Step 3: Run** `cargo test -p feral-processes-engine stockpile` and `cargo test -p feral-processes-app-core stockpile excavate`. Expected: FAIL (excavate passes).
- [ ] **Step 4: Implement engine → app-core → gui.**
  - gui test `a_stockpile_cell_draws_its_wash`: a painted rect fill matching `STOCKPILE_FILL`.
- [ ] **Step 5: Save mutation check.** Mark `stockpiles` `#[serde(skip)]`, confirm the save→load test fails, then restore it.
- [ ] **Step 6: Run** the filters above plus `cargo test -p feral-processes-gui stockpile every_screen`. Expected: PASS.
- [ ] **Step 7: Commit.** Stage the listed paths. Message: `Base: stockpile zones, marked with the Excavation box`.

---

### Task 8: Idle programs tidy loose items onto stockpiles

**Files:**
- Create: `crates/engine/src/game/base/tidy.rs` (register in `game/base/mod.rs`)
- Modify: `crates/engine/src/components.rs` (`Tidying`)
- Modify: `crates/engine/src/game/base/floor.rs` (`drop_load` also sets down `Tidying`)
- Modify: `crates/engine/src/game/base/work_orders.rs`:
  - `:2355-2560` (`drift_idle_staff`: board, preemption, new arm between the respite arm and the wander)
  - `:1107` (`assign_base_labour` ends with `set_down_posted_tidiers`)
- Modify: `crates/engine/src/game/base/morale.rs:306` and `offshift.rs:577` (extract the shared walk tail)
- Modify:
  - `crates/engine/src/save.rs:760` (`CreatureSave::tidying`)
  - `crates/engine/src/game/lifecycle.rs:2442,2785` (`rg -n 'carrying_program:' crates` lists the `CreatureSave` literals)
- Test: `crates/engine/src/tests/stockpiles.rs`

**Interfaces:**
- Consumes:
  - `FreeFloor`, `spawn_floor_item`, `take_floor_item`, `Game::drop_load` (Tasks 3–4)
  - `BaseGrid::is_stockpile` (Task 7)
  - `hauling::{crew_reach, reaches, step_to_post, at_station}`
  - `Game::on_respite`
- Produces:
  - `#[derive(Component, Clone, Debug)] pub struct Tidying(pub FloorThing)`. Its doc: the cleanup carry state, kept separate from `Carrying`, which stays one `(item, qty)`.
  - `pub(crate) fn Game::walk_idle_toward(&mut self, worker: Entity, here: Position, site: Position) -> Result<(), NoPost>`. This is the shared tail of `step_respite`/`step_off_shift`: rigid `blocked_tiles`, `step_to_post` side 1, the party-cell refusal, and the `Position` write. Both callers switch to it.
  - In `tidy.rs`:
    - `pub(crate) struct TidyBoard { cells: Vec<Position>, loose: Vec<(Entity, Position)> }`, both `(x, y)`-sorted. `cells` holds free stockpile cells; `loose` holds floor items whose tile is not a stockpile.
    - `pub(crate) fn Game::tidy_board(&mut self) -> Option<TidyBoard>`: `None` when no cell is marked, so a base without stockpiles pays nothing and behaves exactly as before.
    - `pub(crate) fn Game::may_tidy(&self, worker: Entity, amenities: &Amenities) -> bool`: true when the body has no `Task`, stands on laid floor, has no `Downed`/`OffShift`/`UnderStudy`, and is not `on_respite`.
    - `pub(crate) fn Game::tidy_step(&mut self, worker: Entity, board: &mut TidyBoard) -> bool`: returns whether the body spent the beat on the errand.
    - `pub(crate) fn Game::set_down_posted_tidiers(&mut self)`: `drop_load` on every `(Tidying, Task)` body.
  - `CreatureSave::tidying: Option<FloorThing>` (`#[serde(default)]`).
- `tidy_step` algorithm. Neither the signature nor the tests decide it, so it is spelled out here. `reach = crew_reach(grid, here, &blocked.rigid(), radius)`, and "reachable" means `reaches(.., side 1, ..)`. Candidates are ranked by `(chebyshev(here, p), p.x, p.y)`.
  - **Holding `Tidying`:**
    1. `dest` = the first reachable `board.cells` entry. If there is none: `drop_load(worker)` and return true.
    2. If `at_station(here, dest, 1)`:
       - If `free_floor().free(grid, dest)`: `spawn_floor_item(dest)`, remove `Tidying`, and drop `dest` from the board.
       - Otherwise: drop `dest` from the board and keep holding. This is the re-target on arrival.

       Return true.
    3. Otherwise `walk_idle_toward(dest)`. On `Err` → `drop_load`. Return true.
  - **Empty-handed:**
    1. If no `board.cells` entry is reachable → return false.
    2. `target` = the first reachable `board.loose` entry. If there is none → return false.
    3. If `at_station(here, target, 1)`: `take_floor_item` → insert `Tidying`, and drop it from the board. Otherwise walk; on `Err` return false.

       Return true.
- `drift_idle_staff` changes:
  - Build `let mut board = self.tidy_board();` beside `crowded`.
  - In the per-body loop, after the `Task` skip: if the body holds `Tidying && !sharing && !self.may_tidy(worker, amenities)` → `self.drop_load(worker)`. A sharing body keeps its item through the wander beat.
  - New arm after the respite arm:
    ```rust
    if !sharing && self.may_tidy(worker, amenities)
        && board.as_mut().is_some_and(|b| self.tidy_step(worker, b))
    {
        held.insert(pos);
        continue;
    }
    ```
  - Update the function doc's errand list.

- [ ] **Step 1: Write the failing tests** in `tests/stockpiles.rs`. The fixture: `game_with_home`, a stockpile box of 2 cells at (4,0)-(5,0), one idle tamed program, and loose items. The program is a `spawn_tamed` positioned on floor, with no orders queued so it stays idle.

```rust
#[test]
fn an_idle_program_carries_a_loose_item_onto_a_stockpile() {
    // one loose Item at (-3, 0); tick until the item stands on (4,0) or (5,0), bounded at 200 ticks
}
#[test]
fn a_program_handed_a_task_mid_carry_sets_its_item_down() {
    // give it Tidying, then queue a work order it can staff; after one tick: Task, no Tidying, item on the floor
}
#[test]
fn a_downed_or_off_shift_tidier_sets_its_item_down_first() { /* insert Downed; one schedule pass; no Tidying; item on floor */ }
#[test]
fn nothing_is_carried_when_no_stockpile_cell_is_free() { /* fill both cells; loose item stays put; no Tidying ever */ }
#[test]
fn an_item_already_on_a_stockpile_is_not_moved() {}
#[test]
fn two_tidiers_never_stack_one_stockpile_cell() { /* one free cell, two programs, two loose items; run 300 ticks; every tile has ≤1 item, total units conserved */ }
#[test]
fn a_tidiers_item_survives_save_and_load() { /* insert Tidying(Gear(copy)); save → load; same body (by ProgramId) holds it */ }
#[test]
fn a_tidier_sold_mid_carry_sets_its_item_down() { /* the trade.rs sale path → item on floor */ }
#[test]
fn a_base_without_stockpiles_never_tidies() { /* no marks, loose items present: tidy_board() is None, and after 100 ticks no entity holds Tidying and every item is where it was */ }
```

- [ ] **Step 2: Run** `cargo test -p feral-processes-engine stockpiles`. Expected: FAIL.
- [ ] **Step 3: Implement** the shared walk extraction first, and confirm `cargo test -p feral-processes-engine respite offshift` stays green. Then add `Tidying` and its save field, then `tidy.rs`, then the drift arm.
- [ ] **Step 4: Save mutation check** on `CreatureSave::tidying`, as in Task 4 Step 5.
- [ ] **Step 5: Run** `cargo test -p feral-processes-engine stockpiles`, then `drift`, then `cargo test -p feral-processes-engine`. Expected: PASS. Read any seed-luck failure; don't re-run it.
- [ ] **Step 6: Commit.** Stage the listed paths. Message: `Base: idle programs tidy loose items onto stockpiles`.

---

### Task 9: Stockpiled items count as base stock

**Files:**
- Modify: `crates/engine/src/game/base/stock.rs`:
  - new `stockpiled`
  - `spend_from_base` (:91) tail
  - `base_stock` (:257) fold
  - module doc
- Modify: `crates/engine/src/game/base/work_orders.rs:759` (`base_holding`)
- Modify: `crates/engine/src/game/base/construction.rs:44-58` (`Source`), `:338` (`nearest_source`), `:409` (`pick_up_for_site`), `:253`/`:270` (side and walk match arms)
- Test: `crates/engine/src/tests/stockpiles.rs`

**Interfaces:**
- Consumes: `take_floor_unit` (Task 3), `BaseGrid::is_stockpile` (Task 7).
- Produces:
  - `pub(crate) fn stockpiled(game: &Game) -> Vec<(Position, Entity, ItemId)>`: every `FloorThing::Item` on a stockpile cell, `(x, y)`-sorted. It is **the one read**:
    - `base_holding` adds the matching count.
    - `spend_from_base` takes from it after the shelves, in its order, one `take_floor_unit` per unit, before the ledger report.
    - `base_stock` folds it into `totals`.
    - `nearest_source` adds candidates.

    `stores_hold`/`spend_from_stores` need no edit, because they call `base_holding`/`spend_from_base`.
  - `Source::Stockpile(Entity, Position)`, ranked `store = 1`, with the pack moving to `2`. The doc's "pack is the fallback" rule holds. `pick_up_for_site` lifts one unit (`take_floor_unit` → `Carrying { qty: 1 }`). Side is `1`.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn a_stockpiled_item_counts_and_a_loose_one_does_not() { /* one Item on a stockpile cell, one off it: base_holding == shelves + 1 */ }
#[test]
fn spending_takes_shelves_first_then_stockpiles_in_tile_order() { /* shelf 1, stockpile (5,0) and (4,0); spend 2 → shelf empty, (4,0) gone, (5,0) stays */ }
#[test]
fn the_stock_block_counts_stockpiled_items() { /* base_stock row qty includes it */ }
#[test]
fn a_stockpiled_gear_copy_or_program_is_not_stock() {}
#[test]
fn a_builder_fetches_from_a_stockpile_before_the_pack() { /* site needs 1 core_fragment, pack holds some, stockpile holds 1 nearer-or-equal → stockpile emptied, pack unchanged */ }
#[test]
fn a_pack_still_feeds_a_build_when_no_shelf_or_stockpile_holds_it() {}
```

- [ ] **Step 2: Run** `cargo test -p feral-processes-engine stockpiles`. Expected: FAIL.
- [ ] **Step 3: Implement the interfaces above.** Update `Source`'s doc ("two stores" → three) and `stock.rs`'s module doc, which says which buffers count.
- [ ] **Step 4: Run** `cargo test -p feral-processes-engine stockpiles`, `stock`, `construction` and `work_orders`. Expected: PASS.
- [ ] **Step 5: Commit.** Stage the listed paths. Message: `Base: stockpiled items count as base stock`.

---

### Task 10: Help lines, the seam, and the final gate

**Files:**
- Modify: `assets/help/20-controls.md:~63` (the `z`, `o` and `y` lines beside `m`), `assets/help/60-your-base.md:~28` (one stockpile paragraph). Plain language, and never naming `W`, `T` or `Z`.
- Modify: `.claude/skills/seams/references/base-production.md`:
  - add the new trap bullet
  - correct the `drop_load` bullet (:418-445, :531-552): `Game::drop_load`, spread, no merge
- Modify: `.claude/rules/seams-base.md`:
  - add one rule sentence
  - correct the stranded rule (:257) and the never-destroyed rule (:441) to `Game::drop_load`

- [ ] **Step 1: Write the help lines.** Run `cargo test -p feral-processes-engine help`, and expect PASS. This includes `no_shipped_help_page_names_a_hidden_key`.
- [ ] **Step 2: Write the trap bullet** in `references/base-production.md`, in the house style.
  - Bold rule: **A base tile holds at most one `FloorItem` of one unit; `floor::spawn_floor_item` is its one writer and `Game::set_down` its one door.**
  - Then the trap:
    - A merge or quantity field compiles and silently reintroduces stacking.
    - A second search drifts from walking distance and lands items behind walls.
    - The fallback is a store, never the void, and the over-cap program rung is why.
    - Tidying is separate from `Carrying`.
    - Stockpiles live on `BaseGrid` so no `Resource` shifts iteration order.
- [ ] **Step 3: Write the one-sentence rule** in `seams-base.md`, matching Step 2's bold sentence. `paths:` already matches `**/base/**`.
- [ ] **Step 4: Note for the controller.** The controller writes `seam:<slug>` (the argument) to the memory graph **before** Steps 2–3 land, per the seams skill's order. The executor does not call the memory tools.
- [ ] **Step 5: Run the full gate:**
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets` with zero warnings
  - `unset FERAL_DEV_NO_SIEGES; cargo test --workspace`
  - `cargo test -p feral-processes-engine balance_sim`. It is unaffected, but run it anyway.

  Expected: all PASS.
- [ ] **Step 6: Commit:** `git add assets/help/20-controls.md assets/help/60-your-base.md .claude/skills/seams/references/base-production.md .claude/rules/seams-base.md && git commit -m "Docs: floor-item keys in help; the one-item-per-tile seam"`
