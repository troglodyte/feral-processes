# Honeypot Traps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A crafted consumable the player drops on an adjacent surface tile, which blocks them, catches one non-boss program over game-time from the species window its own tile would spawn from, and holds it until collected by hand.

**Architecture:** Four seams and nothing between them. **Data** is one `#[serde(default)]` field on `ItemDef` plus three `.ron` files — the research→structure→recipe chain is `requires_structure`, so there is no new gating code. **The entity** is `components::Trap` on a surface `Position` and a `Glyph`, which the existing generic draw and examine queries already pick up. **The tick** is `Game::run_traps`, one `&mut Game` pass beside `maybe_spawn_wild_creature`, whose countdown is what keeps fifty honeypots off the seeded RNG stream. **The verbs** are one new arm in `Game::move_player`'s ladder (collect or bump) and one new `Mode` in app-core (place), plus lifting the surface gate on `d`.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, `serde`.

**Spec:** `docs/superpowers/specs/2026-09-21-honeypot-traps-design.md` — read it before Task 1. Its §2 "Decisions taken", §8 "The tick" and §13 "Test intent" are the contract this plan implements.

---

## Three corrections to the spec, stated up front

Each was found by reading the code the spec describes. None changes a decision; each changes a file list or a claim.

1. **§12 says gui touches "`ALL_MODES` and `needs_status_banner` only. No drawing code." Both halves are wrong.** `Mode::TrapDirection` needs a *draw arm* in `render/mod.rs`'s `match app.mode` or it ships as a blank screen — a new `Mode` variant does not fail to compile there, which this repo has already been bitten by. The arm is one call to the existing `draw_direction_prompt`, exactly as `Mode::RemoveDirection`'s is (`crates/gui/src/render/mod.rs:1146`), so it is not new drawing code but it is not nothing either. And it does **not** join `needs_status_banner` — §7 is right about that and §12's summary is wrong. See Task 9.

2. **§6 says "Drawing and examining are free." Drawing is; examining is not.** `Game::view_entities_at` really is a generic `(Entity, &Position, &Glyph)` query, so the glyph reaches the map for free. But `Game::entity_label` (`crates/engine/src/game/inspection.rs:806`) is an if-else ladder over component kinds — squad, creature, `Structure`, `BuildSite`, `Nest`, `BaseAnchor`, `SurfaceLink`, `Settlement` — and a `Trap` matches none of them and **falls through to "You"**. A `Trap` arm is required. Task 3.

3. **§7 and §9 need one engine door each that the spec names only in passing.** `inventory_item_actions` (app-core) must ask the engine whether an item is placeable, mirroring `Game::is_consumable`; and destroying needs `Game::destroy_trap`. Both are named in this plan as `Game::is_placeable` (Task 1) and `Game::destroy_trap` (Task 6).

One deliberate deviation, stated so it is not read as drift: **`Game::find_trap_at` returns `Option<Entity>` and nothing else.** §9 asks for "the `Entity` as well as the data", and gives collecting-despawns-it as the reason — which the bare `Entity` already satisfies. `Game::find_nest_at` (`game/zone.rs:25`) is the precedent for an entity-returning finder on this ladder, and a second return shape would be a second place the read of `Trap` is spelled.

---

## Global constraints

- **`#[serde(default)]` on both new fields** — `ItemDef::trap` and `SaveData::traps`. An item file that authors no `trap:` is not a trap; a save written before this loads with no honeypots, which is exactly what that run had.
- **No `save::SAVE_FORMAT_VERSION` bump.** The save is field-named RON and the addition is additive. Do not bump it, and do not recapture `dev-saves/`.
- **No new `Resource`.** Everything this feature stores lives on the `Trap` component. A new resource shifts bevy's query iteration order across the whole engine, which this repo has recorded as a cause of seeded tests moving.
- **`TrapSave` is a named struct, never a tuple struct.** Field-named RON protects an added field; a positional tuple gains a legacy slot the next time a property is added.
- **`run_traps` is the one writer of a `Trap`'s `caught`, `next_roll` and `Glyph::ch`.** `place_trap` writes them once at spawn and never again.
- **The trap's def is resolved live by id on every read, never copied onto the component.** `restore_nests`' precedent, against `ActiveContract`'s: a honeypot is a device standing in the world, not an agreement already signed, so a retuned `.ron` retunes honeypots already on the ground and a deleted one drops them silently.
- **Gates before every commit:** `cargo fmt`, then `cargo clippy --workspace --all-targets` (`--all-targets` is load-bearing — a bare run leaves test modules unlinted), then the task's own tests. `cargo test --workspace` is the gate at Task 7 and Task 10 only.
- **Do not bump the workspace version or write a `CHANGELOG.md` section on this branch.** That happens once, at the merge.
- **Vocabulary.** Player-facing copy says *honeypot*, never "trap", "snare" or "bait". The engine's type is `components::Trap` because the concept is general and a mod may ship a second one — that asymmetry is deliberate and documented in the spec's §1.
- **Glyphs, decided here because the spec leaves the sprung char open.** Armed `'^'`, sprung `'U'`, both `GlyphColor::Yellow`. Verified against every glyph in `assets/` and every `Glyph { ch: … }` in the engine: `'^'` is authored by `assets/structures/shield.ron` and `armory.ron`, which is safe because a `Structure` stands in base space and `view_entities_at` filters on `stands_in_base_space`, so the two never share a map. `'U'` appears nowhere as a glyph in any crate or asset (only as a keybinding), and is inside the ASCII range `crates/gui/tests/font_rasterization.rs` already covers. Both chars go in named constants — `TRAP_GLYPH_ARMED` / `TRAP_GLYPH_SPRUNG` in `components.rs` beside the `Trap` struct — so the capture site and the spawn site cannot disagree.
- **The hue is checked against three reservations:** bright cyan is the player's `@` and nothing else may take it, bright yellow is unreachable from content by census, and `GlyphColor::Orange` is a settlement's. `GlyphColor::Yellow` clears all three, and magenta/cyan (boss and nemesis marks) and red (reads as a threat) are avoided.
- **`EntityView::difficulty` stays `None` and no corner mark is spent.** A honeypot is not `Hostile`, so it gets no con reading — the existing rule for anything non-hostile, not a new exception. The tile's four corners, both edges and the background wash are all already claimed; the sprung/armed distinction is carried by the centre glyph's *char*, never by a mark and never by a tint. A glyph's hue comes from `hud::palette::glyph` (a content table); `ATTENTION`/`WARN`/`THREAT` are overlay *roles*. The two systems must not be crossed.

---

## File structure

| File | Task | Responsibility |
|---|---|---|
| `crates/engine/src/items_db.rs` | 1 | `TrapDef`, `ItemDef::trap` |
| `crates/engine/src/game/catalog.rs` | 1 | `Game::is_placeable`, the `item_effects_besides_grant` line |
| `assets/items/README.md` | 1 | the schema doc |
| `assets/research/deception.ron` | 2 | the research gate |
| `assets/structures/decoy_bench.ron` | 2 | the assembler gate |
| `assets/items/honeypot.ron` | 2 | the recipe and the rarity ceiling |
| `crates/engine/src/tests/assets.rs` | 2 | the new censuses |
| `crates/engine/src/components.rs` | 3 | `Trap`, `TRAP_GLYPH_ARMED`, `TRAP_GLYPH_SPRUNG` |
| `crates/engine/src/game/inspection.rs` | 3 | the `entity_label` arm |
| `crates/engine/src/game/zone.rs` | 3, 4, 6 | `find_trap_at`, `trap_count`, `place_trap`, `destroy_trap` |
| `crates/engine/src/tuning.rs` | 4, 5 | the four constants |
| `crates/engine/src/game/spawning.rs` | 5 | `run_traps`, `wild_body_level` |
| `crates/engine/src/game/combat.rs` | 5 | `ability_user_level` calls the extracted helper |
| `crates/engine/src/game/turn.rs` | 5, 6 | the `tick_inner` call, the `move_player` arm |
| `crates/engine/src/save.rs` | 7 | `TrapSave`, `SaveData::traps` |
| `crates/engine/src/game/lifecycle.rs` | 7 | `trap_saves_for`, `restore_traps` |
| `crates/engine/src/tests/traps.rs` | 3–7 | the feature's engine tests (new module) |
| `crates/app-core/src/lib.rs` | 8 | `Mode::TrapDirection`, `inventory_item_actions`' `[P]lace` row |
| `crates/app-core/src/app/inventory.rs` | 8 | the `p` dispatch |
| `crates/app-core/src/app/building.rs` | 8 | `handle_trap_direction_key`, the surface branch of `handle_remove_direction_key` |
| `crates/app-core/src/app/playing.rs` | 8 | the `d` gate lift |
| `crates/app-core/src/app/input.rs` | 8 | the `Mode::TrapDirection` route |
| `crates/app-core/src/tests/traps.rs` | 8 | the app-core tests (new module) |
| `crates/gui/src/render/mod.rs` | 9 | `ALL_MODES` 113 → 114, the draw arm |
| `assets/help/` | 10 | the manual page |

**Why `place_trap`/`run_traps` are split across `game/zone.rs` and `game/spawning.rs`.** `run_traps` reads `pick_habitat_species`, `roll_rarity` and `in_opening_ring`, all private to `spawning.rs`; putting the whole feature in `zone.rs` means widening three gates to buy a file boundary. The verbs the *player* drives (`place_trap`, `destroy_trap`, `find_trap_at`, `trap_count`) live in `zone.rs` beside `find_nest_at`, which is the ladder they join.

---

### Task 1: `TrapDef` and the door that reads it

**Files:**
- Modify: `crates/engine/src/items_db.rs` (`ItemDef`, after `craftable` at line 142)
- Modify: `crates/engine/src/game/catalog.rs` (`is_placeable` beside `is_consumable` at line 685; one arm in `item_effects_besides_grant`)
- Modify: `assets/items/README.md`
- Test: `crates/engine/src/tests/catalog.rs`

**Interfaces:**
- Produces: `items_db::TrapDef { rarity_cap: Rarity }`, `ItemDef::trap: Option<TrapDef>` (`#[serde(default)]`), `Game::is_placeable(&ItemId) -> bool`.
- Consumes: nothing.

The shape, which is the non-obvious part — a struct and not a bare `Option<Rarity>`, because it matches how every other capability on `ItemDef` is authored and is where a second tier's own period or chance would land:

```rust
/// Authored on any item the player may place on the ground as a trap —
/// see `Game::place_trap` and `components::Trap`.
#[serde(default)]
pub trap: Option<TrapDef>,

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrapDef {
    /// The best rarity this honeypot will ever catch. A roll above it is
    /// clamped down, never rerolled.
    pub rarity_cap: Rarity,
}
```

- [ ] **Step 1: Write the failing tests.** In `crates/engine/src/tests/catalog.rs`, three of them, against a fixture item db rather than the shipped assets (the shipped honeypot does not exist until Task 2):
  - an item file authoring `trap: Some((rarity_cap: Silver))` parses and `Game::is_placeable` answers `true` for it;
  - an item file authoring no `trap:` parses and `is_placeable` answers `false` — this is the `#[serde(default)]` claim, and without it the field ships mandatory and every mod's item files stop loading;
  - `Game::item_effects` for the first names the ceiling. Assert on the *substring* the rarity contributes (`Rarity::Silver`'s own label, asked of the type, never a hand-typed `"Silver"`), so a renamed tier moves the test with the code.
- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine catalog::` — expected: does not compile, `no field 'trap'`.
- [ ] **Step 3: Add the field, the struct and the two doors.** `is_placeable` is a three-line mirror of `is_consumable` (`catalog.rs:685`). The `item_effects_besides_grant` arm copies `def.trap` out of the `ItemDb` borrow the way `consume`/`upgrade`/`potency` already are at the head of that function, then pushes one line. Wording: `Catches: up to {rarity} programs`.
- [ ] **Step 4: Run them and watch them pass.**
- [ ] **Step 5: Document the field in `assets/items/README.md`,** in the same change — that file is the schema reference for anyone modding, and the rule is that it moves whenever a field is added. Say what `rarity_cap` bounds, that period and capture chance are `tuning.rs`'s and deliberately not authorable (difficulty is not content), and that an item with `trap:` is placeable from the pack.
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 2: The three shipped assets and the censuses that hold them

**Files:**
- Create: `assets/research/deception.ron`, `assets/structures/decoy_bench.ron`, `assets/items/honeypot.ron`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: `ItemDef::trap` (Task 1).
- Produces: the ids `deception`, `decoy_bench`, `honeypot`, used verbatim by every later task.

**The three files are data and must be exact.** Every figure below was checked against the shipped tree.

```ron
// assets/research/deception.ron
(
    id: "deception",
    name: "Deception",
    description: "A bench that compiles honeypots — decoys you leave on the ground to catch a program without fighting it.",
    cost: 20,
    materials: [("bytecode_block", 10)],
    requires: ["automation"],
    unlocks_structures: ["decoy_bench"],
)
```

```ron
// assets/structures/decoy_bench.ron
(
    id: "decoy_bench",
    name: "Decoy Bench",
    description: "Compiles Honeypots out of ICE Breakers and Core Fragments.",
    glyph: 'y',
    color: Yellow,
    build_cost: [("bytecode_block", 12)],
    work: None,
    capacity: 20,
    assembles: Some((item: "honeypot", ticks_per_unit: 20)),
    power_draw: 2,
)
```

```ron
// assets/items/honeypot.ron
(
    id: "honeypot",
    name: "Honeypot",
    description: "A decoy left on the ground. Given time it catches a program that wanders into it, and holds it until you come back.",
    value: Some(1),
    craftable: Some((cost: [("ice_breaker", 1), ("core_fragment", 4)], requires_structure: Some("decoy_bench"))),
    trap: Some((rarity_cap: Silver)),
)
```

**The four checks these were built to clear, so a worker changing a figure knows what it costs:**

- `requires: ["automation"]` satisfies `every_research_material_is_reachable_through_that_nodes_own_prerequisites` (`tests/assets.rs:4683`) — though in fact `bytecode_block` needs only the `refinery`, which no research node gates, so the bill would pass with an empty `requires`. Automation is the *honest* prerequisite: the Compiler is what assembles the ICE Breakers a honeypot eats, and `teardown.ron` takes the same prerequisite for the same kind of reason.
- `materials` is non-empty, for `every_shipped_base_research_node_costs_materials` (`tests/assets.rs:4898`).
- `discoverable` is **left unauthored** (so `false`, visible from turn one). `exactly_the_twenty_named_research_nodes_are_discoverable` (`tests/assets.rs:5125`) asserts an exact set, so making Deception discoverable means editing that list, and nothing in the spec asks for a fourth gate on top of its three.
- `value: Some(1)` against ingredients worth 5 (ICE Breaker 1, Core Fragment 1 × 4) clears the craftable-price bound. A craftable worth more than its ingredients is an infinite Credit loop, and `decoy_bench` sets no `work.produces`, so the second, harder bound — a `work.produces` structure making an item out of nothing on a timer — does not apply.
- `glyph: 'y'` is unclaimed: no shipped structure or species authors it.

- [ ] **Step 1: Write the failing census.** Append to `crates/engine/src/tests/assets.rs` one test, `the_honeypot_is_gated_by_deception_and_nothing_else`, asserting four things against the real shipped assets — that the `honeypot` def carries a `trap:` with `rarity_cap == Rarity::Silver`; that its `craftable.requires_structure` is `Some("decoy_bench")`; that exactly one research node names `decoy_bench` in `unlocks_structures` and it is `deception`; and that `decoy_bench.assembles` names `honeypot`. That last is the chain's load-bearing link and the one nothing else checks: without it the bench is buildable and the recipe is unassemblable, which reads at the keyboard as the work-order screen simply not listing the item.
- [ ] **Step 2: Run it and watch it fail.** `cargo test -p feral-processes-engine assets::the_honeypot` — expected: FAIL, no such item def.
- [ ] **Step 3: Write the three `.ron` files** exactly as above.
- [ ] **Step 4: Run the new census and the four existing ones it sits beside.** `cargo test -p feral-processes-engine assets::` in full — the whole module, not just the new name. Adding a research node, a structure and an item touches censuses that count shipped content, and the module is the only honest gate on which.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 3: The entity, its name, and the two finders

**Files:**
- Modify: `crates/engine/src/components.rs` (`Trap`, the two glyph constants)
- Modify: `crates/engine/src/game/inspection.rs` (`entity_label`, line 806 — the arm goes **after** `SurfaceLink` and before `Settlement`)
- Modify: `crates/engine/src/game/zone.rs` (`find_trap_at`, `trap_count`, beside `find_nest_at` at line 25)
- Create: `crates/engine/src/tests/traps.rs`; register it in `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Consumes: `ItemDef::trap` (Task 1), the `honeypot` id (Task 2).
- Produces: `components::Trap`, `components::TRAP_GLYPH_ARMED`, `components::TRAP_GLYPH_SPRUNG`, `Game::find_trap_at(&mut self, x: i32, y: i32) -> Option<Entity>`, `Game::trap_count(&mut self) -> usize`.

```rust
#[derive(Component, Clone)]
pub struct Trap {
    /// Which item def this was placed from. The rarity ceiling is resolved
    /// from the def on every read rather than copied here — `restore_nests`'
    /// precedent and against `ActiveContract`'s: a honeypot is a device
    /// standing in the world, not an agreement already signed, so a retuned
    /// `.ron` retunes one already on the ground.
    pub item: ItemId,
    /// Ticks until the next capture roll. The countdown is what keeps fifty
    /// honeypots off the seeded RNG stream — see `Game::run_traps`.
    pub next_roll: u32,
    pub caught: Option<DownedProgram>,
}
```

- [ ] **Step 1: Write the failing tests** in the new `crates/engine/src/tests/traps.rs`. Use `crates/engine/src/tests/support.rs`'s existing fixtures — look there before writing a new one. Four tests:
  - a hand-spawned armed `Trap` on a surface tile is named by `entity_label` as the item's own name, asked through `Game::item_name` and never a literal;
  - a sprung one (a `caught: Some(…)`) is named differently from an armed one — assert the two labels are *not equal* rather than pinning the sprung wording, since the wording is copy and the distinction is the contract;
  - `find_trap_at` answers `Some` on the honeypot's tile and `None` one tile over;
  - `trap_count` answers 0 on a fresh game and 2 after two hand-spawns.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-engine traps::`
- [ ] **Step 3: Implement.** The `Trap` struct and the two `pub const` glyph chars in `components.rs`; the `entity_label` arm (read `Game::item_name(&trap.item)`, append the sprung/armed distinction — the spec's §6 rule is that the *char* carries it on the map, which says nothing about the examine line, and a line saying only "Honeypot" for both states is the one thing `x` exists to answer); `find_trap_at` as `find_nest_at`'s shape one component over; `trap_count` as a `query_filtered::<(), With<Trap>>().iter(&self.world).count()`.
- [ ] **Step 4: Run and watch them pass.**
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 4: `Game::place_trap` and its six refusals

**Files:**
- Modify: `crates/engine/src/game/zone.rs`
- Modify: `crates/engine/src/tuning.rs` (a new labelled section, `TRAP_PLACEMENT_CAP` alone for now)
- Test: `crates/engine/src/tests/traps.rs`

**Interfaces:**
- Consumes: `Game::is_placeable` (Task 1), `components::Trap` + `TRAP_GLYPH_ARMED` + `trap_count` (Task 3).
- Produces: `Game::place_trap(&mut self, item: &ItemId, dx: i32, dy: i32) -> Result<(), String>` — `place_structure`'s return shape (`game/base/building.rs:24`), which is what `App::report` takes.

**Every refusal lands before anything is spent, and each gets its own test.** A single test over one of six passes against five paths that never spend anyway — this is the rule `commit_caravan_basket` and `Game::extract_program` both hold, asserted per refusal. The six, in the order the function checks them:

1. not on the open zone surface — `require_surface` (`game/stack.rs:434`) plus `is_game_over` and `has_active_battle`, exactly `move_player`'s own guard triple;
2. the item is not held (`Inventory` count is zero);
3. the item has no `trap:` def — `Game::is_placeable`;
4. `TRAP_PLACEMENT_CAP` honeypots already stand;
5. the target tile is not `walkable`;
6. the target tile is occupied — a wild creature, a nest, a Stack on-ramp, a settlement, or another honeypot.

**Refusal 6 is asked through the same finders `move_player`'s ladder uses, never a second set** — `find_wild_creature_at`, `find_nest_at`, `find_surface_link_at`, `find_settlement_at`, `find_trap_at`. A second set of occupancy queries is how a tile becomes placeable to one function and occupied to another.

**`TRAP_PLACEMENT_CAP` is checked before walkability deliberately**: the cap is about the per-tick cost and the carpet, and a player at the cap should be told they are at the cap wherever they point.

On success: take one unit from `Inventory`, spawn `(Trap { item, next_roll: TRAP_PERIOD_TICKS, caught: None }, Position { x: nx, y: ny }, Glyph { ch: TRAP_GLYPH_ARMED, color: GlyphColor::Yellow })`, log the placement, and spend one tick. **`TRAP_PERIOD_TICKS` is introduced in Task 5**; for this task, spawn with `next_roll: 0` and let Task 5 change the initialiser — or introduce the constant here and use it. Prefer the latter; it is one line and avoids a commit whose behaviour is wrong on purpose.

**app-core owes no new `after_tick()`.** Placement is reached through `App::handle_key`, whose tail already calls it — one of the three paths that do. Say so in `place_trap`'s doc comment so nobody adds a second.

- [ ] **Step 1: Write six failing refusal tests plus one success test.** Each refusal test asserts *both* that the call is `Err` and that nothing moved: the `Inventory` count is unchanged and `trap_count` is unchanged. A refusal test that only checks the `Err` is the vacuous half.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-engine traps::`
- [ ] **Step 3: Implement `place_trap` and the tuning section.** New labelled section in `tuning.rs` — a `//! `-style section header comment matching the neighbours, then `TRAP_PLACEMENT_CAP: usize = 50` with a doc comment saying it is hidden from the player and exists to bound the per-tick cost and the carpet, not to be played around.
- [ ] **Step 4: Run and watch them pass.**
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 5: The tick

**Files:**
- Modify: `crates/engine/src/game/spawning.rs` (`run_traps`, `wild_body_level`)
- Modify: `crates/engine/src/game/combat.rs` (`ability_user_level` at line 1215 — its fallback becomes a call)
- Modify: `crates/engine/src/game/turn.rs` (`tick_inner` at line 214)
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/traps.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–4.
- Produces: `Game::run_traps(&mut self)`, `Game::wild_body_level(&self) -> u32`.

**The level helper is an extraction, not a copy.** `ability_user_level` (`combat.rs:1215`) answers `self.world.resource::<ZoneLevel>().0` for anything with no `Experience`. `run_traps` has no entity to ask, so it needs the same answer — and a second `resource::<ZoneLevel>().0` read is two formulas to drift. Extract that `unwrap_or_else` body into `Game::wild_body_level` and have `ability_user_level` call it. This is the spec's §8.5 instruction and it is the one place the feature would otherwise restate a formula.

**`run_traps` goes in `tick_inner` immediately after `maybe_spawn_wild_creature` (`turn.rs:233`)** — beside the ambient roll because it is the same kind of work: a `&mut Game` pass over the surface that a bevy system cannot express, keyed to a place rather than to the party.

**Honeypots are walked in `(x, y)` order, sorted before any draw.** `assembler_system`'s rule. Bevy's query iteration order is not stable, so two honeypots whose periods elapse on the same tick would consume the shared `GameRng` in an order that varies between runs — and that surfaces as an intermittent seeded-test failure somewhere else entirely, which this repo has spent sessions on before. Collect `(Entity, Position)` in one pass, sort by `(x, y)`, then act.

**Per honeypot: if `next_roll > 0`, decrement and return, spending no RNG draw.** This is the whole point of the countdown and it is what keeps fifty honeypots at about a tenth of a draw a tick rather than fifty. When the period elapses, reset `next_roll = TRAP_PERIOD_TICKS`, skip if already `caught`, then roll once.

**The draw order is part of the contract**, in this sequence, and a seeded test pins it:

```
1. TRAP_CAPTURE_CHANCE                                  — one draw; miss ends here
2. Game::pick_habitat_species(x, y, None, false)        — draws (allow_boss: false skips the boss roll)
3. Game::roll_rarity(&species_def, x, y, false)         — one draw
4. rarity = rarity.min(def.rarity_cap)                  — CLAMP HERE, before condition
5. DownedProgram::roll_condition(clamped, false, 0.0)   — NO draw; a pure formula
6. condition = condition.saturating_sub(TRAP_CONDITION_PENALTY)
7. level = wild_body_level(); boss = false; carried = None
```

**Step 4 before step 5, never after.** `downed_program_for`'s boss-rarity-floor rule, second caller: `roll_condition` prices condition against the rarity the program actually ships with, and `DownedProgram::grade()` folds both, so a condition rolled against the pre-clamp rarity leaves the grade overstated for exactly the catches the ceiling exists to hold down.

**Step 5 spends no `GameRng`.** Verified: `DownedProgram::roll_condition` (`items.rs:429`) is a pure integer formula over `CONDITION_BASE`, `CONDITION_PER_RARITY_STEP` and an overkill term, with no RNG in it at all. The spec's §8 calls it a roll; it is not one, and the seeded draw-order test must not budget a draw for it.

**No entity is spawned at any point.** The catch is a `DownedProgram` value written onto the component.

**`run_traps` deliberately does not call `Game::field_escalation`.** The escalation terms exist to scale a spawned body's `Stats`, and a `DownedProgram` carries none — its level is `ZoneLevel` and its worth is `grade()`. An escalation term here would be a number with nothing to apply to, and joining that function's caller census would misreport what the feature does. Say this in the doc comment; an omission with no stated reason reads as an oversight to the next reader.

**The log line's two axes are both chosen, not defaulted.** It is **not** base news — `battle_rows` drops `MessageSource::Base` unconditionally, and `run_traps` runs every tick including while a fight is open, so a base-sourced line is invisible exactly when it fires. As a plain `Info` line it is pruned by `retain_outcomes_since_battle`, which is the right lifetime for a transient event. And `resources::condense` folds repeats across all three log surfaces, so **the line must name the species or the tile** or two catches read as one row — which also means a test counting entries to prove one line fired must sum `repeats` or it is vacuous.

Four constants join the tuning section, each with a doc comment:

| constant | value | note |
|---|---|---|
| `TRAP_PERIOD_TICKS` | `480` | 4 minutes at 2 ticks/sec |
| `TRAP_CAPTURE_CHANCE` | `0.15` | one catch per ~27 minutes per honeypot |
| `TRAP_CONDITION_PENALTY` | `15` | points off a 0..=100 condition, after `roll_condition` |

- [ ] **Step 1: Write the failing tests.** Seven, all seeded:
  - **the countdown spends no `GameRng`** — place a honeypot, tick for fewer ticks than one period, and assert the RNG stream is exactly where an identical run with no honeypot left it. Probe the stream by drawing from `GameRng` before and after rather than by inspecting it; this is the assertion the whole "existing seeded tests are unaffected" claim rests on;
  - **the draw order is the documented one** — pinned against a seed by asserting the exact species/rarity/condition a known seed produces, with a comment saying that a change here is deliberate and not a refactor;
  - **the catch never exceeds the ceiling** — many seeded rolls, not one, since `Silver` is a common outcome and a single roll proves nothing;
  - **the catch is never a boss** — `caught.boss` is false and the species is never an `is_boss` apex, over the same span;
  - **`carried` is always `None`**;
  - **two honeypots elapsing on the same tick resolve by `(x, y)`** — the assertion is that a *second run of the same seed* produces the same two catches in the same two places. Asserting an order directly cannot fail, because bevy's order is stable within one run;
  - **a sprung honeypot's glyph is `TRAP_GLYPH_SPRUNG` and an armed one's is `TRAP_GLYPH_ARMED`**, and a second elapsed period does not overwrite an existing `caught`.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-engine traps::`
- [ ] **Step 3: Implement `wild_body_level`, rewire `ability_user_level`, write `run_traps`, wire `tick_inner`, add the three constants.**
- [ ] **Step 4: Run and watch them pass.** Then run the two suites most exposed to an RNG-stream shift — `cargo test -p feral-processes-engine spawning::` and `cargo test -p feral-processes-engine combat`. Nothing should move: no honeypot exists in those fixtures, and the countdown draws nothing. If something does move, the countdown is drawing, which is the bug this task is most likely to ship.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 6: Collecting, blocking and destroying

**Files:**
- Modify: `crates/engine/src/game/turn.rs` (`move_player`, line 676)
- Modify: `crates/engine/src/game/zone.rs` (`destroy_trap`)
- Test: `crates/engine/src/tests/traps.rs`

**Interfaces:**
- Consumes: `find_trap_at` (Task 3), `run_traps` (Task 5).
- Produces: `Game::destroy_trap(&mut self, dx: i32, dy: i32) -> Result<(), String>`.

**The ladder arm goes after the settlement arm and before `let from = …`** — that is, between `turn.rs`'s settlement block (which ends `self.tick(); return 0;`) and the line reading the departure biome. Every arm above it is a thing that is not a step; this is the fifth, and it must return before the walkability read the way the four above it do.

- **Sprung:** `Game::push_downed_program` (`game/combat_rewards.rs:739`). On `true`, despawn the entity and log. On `false` the store is full — that door already logs its own refusal, so this must not log a second one, and the honeypot stays sprung. **The player does not move either way.**
- **Armed:** a bump. The player does not move. It spends a tick, exactly as the settlement arm does.

**Player-only blocking costs nothing to enforce.** `pursuit_field` and the patrol walk never consult this ladder, so there is nothing to opt out of — wild programs, pursuing guardians and town patrols ignore a honeypot by construction.

**Placement refuses an occupied tile, but nothing stops a wild program spawning onto a honeypot later.** `standable_near` and `scatter_open_tile` check walkable, not empty — which has already put a settlement on a Stack entrance once in this repo. That is accepted and not fixed: the creature arm of the ladder sits above the honeypot arm, so the player fights the program and the honeypot is still there afterwards. Say so in the arm's comment.

**`Game::destroy_trap` returns nothing to the player** — no material refund, and a sprung one loses what it caught. Reusing the demolish gesture with no confirmation is a taken decision.

- [ ] **Step 1: Write the failing tests.** Six:
  - walking into an armed honeypot leaves `Position` unchanged, spends a tick, and leaves the honeypot standing;
  - walking into a sprung one takes the program into `DownedPrograms`, despawns the honeypot, and still leaves `Position` unchanged;
  - with the downed store at `MAX_DOWNED_PROGRAMS`, walking into a sprung one moves nothing: the store is unchanged, the honeypot is still sprung and still standing, and the player has not moved;
  - `destroy_trap` on a neighbouring armed honeypot despawns it and grants no item — assert the `Inventory` count of `honeypot` is unchanged, which is the half a "did it despawn" test misses;
  - `destroy_trap` on a sprung one despawns it and does **not** push to `DownedPrograms`;
  - `destroy_trap` pointed at an empty tile is `Err` and despawns nothing.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Implement the ladder arm and `destroy_trap`.**
- [ ] **Step 4: Run and watch them pass.** Then `cargo test -p feral-processes-engine turn::` — `move_player` is one of the most-tested functions in the engine and a new arm in its ladder is exactly the kind of change that reorders a return.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 7: Save and load

**Files:**
- Modify: `crates/engine/src/save.rs` (`TrapSave` beside `NestSave` at line 828; `SaveData::traps` beside `nests` at line 1168)
- Modify: `crates/engine/src/game/lifecycle.rs` (`trap_saves_for` beside `nest_saves_for` at line 2154; `restore_traps` beside `restore_nests` at line 688; the two call sites at lines 1352 and ~2440)
- Test: `crates/engine/src/tests/traps.rs`

**Interfaces:**
- Consumes: `components::Trap` (Task 3).
- Produces: `save::TrapSave`, `SaveData::traps: Vec<TrapSave>` (`#[serde(default)]`).

```rust
/// A honeypot's state on disk — see `components::Trap`.
#[derive(Serialize, Deserialize, Clone)]
pub struct TrapSave {
    pub item: ItemId,
    /// **Zone-surface** coordinates. A honeypot stands nowhere else.
    pub position: (i32, i32),
    pub next_roll: u32,
    pub caught: Option<DownedProgram>,
}
```

**A named struct and not a tuple struct.** That is the one shape the additive-field rule does not protect: a positional tuple gains a legacy slot the next time a property is added.

**No `pending_cronjobs` deferral.** Nothing forward-references a honeypot, and a honeypot references nothing but its own item id — dropped silently if the def no longer resolves, `restore_nests`' handling of a missing species exactly. `restore_traps` therefore returns nothing and can sit anywhere in load order.

**`restore_traps` re-derives the glyph from `caught.is_some()` rather than saving it.** The char is display, the `caught` is state, and a saved char is a second copy of the same fact that a retune would strand.

- [ ] **Step 1: Write the failing tests.** Three:
  - **a real save to a real file and a real load back** — `Game::save` to a temp path, `Game::load`, then assert position, `next_roll` and a held `caught` (species, level, rarity, condition, `carried: None`) all survive. **A RON round trip cannot catch a skipped field**; only a save-then-load can, and this repo has shipped a `#[serde(skip)]` that left the round-trip test green. Write the temp file into the process's own temp dir with a name unique to this test — a fixed path is shared with the `dev_template` loop, which deletes files mid-run and produces a panic that names the load line rather than the collision;
  - a honeypot whose item def no longer resolves is dropped on load rather than restored glyphless — build the load-side `ItemDb` without `honeypot` and assert `trap_count` is 0 and the load succeeded;
  - the restored glyph is `TRAP_GLYPH_SPRUNG` for a saved sprung honeypot and `TRAP_GLYPH_ARMED` for an armed one.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Implement `TrapSave`, the `SaveData` field, `trap_saves_for`, `restore_traps` and both call sites.**
- [ ] **Step 4: Write the breach-survival test.** **A honeypot survives a zone breach** — `Game::clear_local_wild` is `With<Hostile>` and a honeypot carries no `Hostile`, so survival is an *omission* and nothing in the compiler holds it. **The test must breach on populated ground:** a breach test that despawns the wild by hand first is vacuous, and this repo has written that test before. Stock the ground, assert the wild population is non-zero, breach, then assert the wild population fell and `trap_count` did not.
- [ ] **Step 5: Run the whole engine suite.** `cargo test -p feral-processes-engine` — a `SaveData` field touches `save_roundtrip.rs` and every `dev-saves/` template load.
- [ ] **Step 6: Run the full workspace.** `cargo test --workspace`. Do not pipe it to `tail` or `grep`; cargo's exit code is lost through a pipe and the run reports the pipeline's status.
- [ ] **Step 7: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 8: app-core — placing, and lifting the `d` gate

**Files:**
- Modify: `crates/app-core/src/lib.rs` (`Mode::TrapDirection` near `Mode::BuildDirection` at line 1439; `inventory_item_actions` at line 158)
- Modify: `crates/app-core/src/app/input.rs` (the route, beside line 226)
- Modify: `crates/app-core/src/app/inventory.rs` (`handle_inventory_item_action_key`, line 253)
- Modify: `crates/app-core/src/app/building.rs` (`handle_trap_direction_key`; the surface branch of `handle_remove_direction_key` at line 469)
- Modify: `crates/app-core/src/app/playing.rs` (the `d` gate, line ~132)
- Create: `crates/app-core/src/tests/traps.rs`; register it in `crates/app-core/src/tests/mod.rs`

**Interfaces:**
- Consumes: `Game::is_placeable`, `Game::place_trap`, `Game::destroy_trap`.
- Produces: `Mode::TrapDirection`, `App::handle_trap_direction_key`, `App::pending_trap: Option<ItemId>`.

**`Mode::TrapDirection` is `handle_build_direction_key`'s exact shape** (`building.rs:249`): four cardinal keys plus `hjkl`, the direction key itself commits, Esc cancels and clears `pending_trap`. It is a popup like every other picker, so it does **not** join `needs_status_banner`.

**The `[P]lace` row.** `inventory_item_actions` gains `('p', "[P]lace")` when `game.is_placeable(item)`, between the `[C]onsume` row and `[S]ell`. Lowercase `p` is the existing convention on this page — every row there is a lowercase char with an uppercase label, dispatched by `handle_inventory_item_action_key`'s `to_ascii_lowercase` match — so `p` is consistent, not a violation of the uppercase-for-new-actions rule, which is about screens whose lowercase letters are row selectors.

**Lifting the `d` gate is the delicate half.** Today `d` is refused outside base space (`playing.rs:~132`), with a comment explaining that out there the four directions point at nothing ownable: the structures are all in base space and the player's surface `Position` is either their tile or pinned to the Stack entrance. **Honeypots make that false for the first time.** So:

- the gate goes and `d` opens `Mode::RemoveDirection` in both spaces;
- the comment is **replaced with the new reason, not deleted** — that two spaces now have something ownable to aim at, and which one decides what `d` means;
- `handle_remove_direction_key` gains a surface branch: **in base space it is structure-then-build-site exactly as today; on the surface it is honeypot only.**

**The surface branch must not call `Game::adjacent_structure`.** That query is over base-space `Position`s, and a base-space query asked about a surface tile answers by numeric coincidence — and the coincidence is the *common* case, because `find_walkable_start` returns `(0, 0)` whenever it can, so the anchor, the zone spawn point and base space's origin all carry the same numbers. `move_player`'s own long comment documents what that cost last time: the base's Home made the anchor unwalkable and its machines were invisible walls. The surface branch calls `Game::destroy_trap` and nothing else.

**The refusal line is rewritten** for the surface: `"Nothing of yours to demolish there."` The base-space refusal (`"Nothing to demolish that way."`) is unchanged.

**No new `after_tick()` obligation.** Both new paths are reached through `App::handle_key`, whose tail already calls it.

Adding a `Mode` variant may fail to compile at app-core's exhaustive matches over `Mode` (there are several around `lib.rs:2017`–`2066`). Follow the compiler; place `TrapDirection` in the same groups `BuildDirection` and `RemoveDirection` sit in.

- [ ] **Step 1: Write the failing tests** in the new `crates/app-core/src/tests/traps.rs`. Seven:
  - an inventory row holding a honeypot offers `[P]lace`, and a row holding an ICE Breaker does not;
  - `p` from `Mode::InventoryItemAction` opens `Mode::TrapDirection` and stores the item in `pending_trap`;
  - a direction key from `Mode::TrapDirection` places one, returns to `Mode::Playing`, and lowers the pack count by one;
  - Esc from `Mode::TrapDirection` returns to `Mode::Playing`, clears `pending_trap` and places nothing;
  - a refused placement (the target tile is a wall) leaves `App::status_line` set and places nothing;
  - `d` on the **surface** now opens `Mode::RemoveDirection` rather than refusing — the assertion is on the mode, since the old behaviour was a refusal and the new one is a screen;
  - `d` + a direction on the surface, aimed at a honeypot, destroys it; aimed at empty ground it refuses and destroys nothing. **And one more, which is the regression this task is most likely to ship:** with a base standing and its structures at base-space coordinates that alias onto the party's surface tile, `d` + a direction on the surface must **not** destroy a structure. That is the numeric-coincidence bug, and nothing else in the suite would catch it.
- [ ] **Step 2: Run and watch them fail.** `cargo test -p feral-processes-app-core traps::`
- [ ] **Step 3: Implement.** Note that `crates/app-core/src/tests/creation.rs` flakes on an unmodified binary and the failing name varies — if it fails during this task, re-run that module rather than chasing it inside this work.
- [ ] **Step 4: Run and watch them pass,** then `cargo test -p feral-processes-app-core` in full.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 9: gui — the mode is drawn

**Files:**
- Modify: `crates/gui/src/render/mod.rs` (`ALL_MODES` at line 1502; the draw arm beside `Mode::RemoveDirection` at line 1146)

**Interfaces:**
- Consumes: `Mode::TrapDirection` (Task 8).
- Produces: nothing.

**This task must land immediately after Task 8 and must not be skipped.** A new `Mode` variant does not fail to compile in gui: `ALL_MODES` is a hand-written array and the draw dispatch ends in `_ => {}`, so a variant left out of both ships as a **blank screen** that every test passes. The array's declared length (`[Mode; 113]`) does fail to compile once an entry is added, which is the only part of this the compiler helps with.

The two edits are:

```rust
// in ALL_MODES, beside Mode::RemoveDirection — and bump 113 to 114
Mode::TrapDirection,
```

```rust
// in the `match app.mode` dispatch, beside Mode::RemoveDirection
Mode::TrapDirection => draw_direction_prompt(
    "Place Direction",
    "Place the honeypot which way? (arrows/hjkl, Esc to cancel)",
    refusal,
    painter,
    m,
),
```

`draw_direction_prompt` (`render/mod.rs:1438`) is the shared helper `Mode::RemoveDirection` already uses — a `PopupSize::Small` popup with one text row. Nothing new is drawn.

- [ ] **Step 1: Add the `ALL_MODES` entry and bump the length.** Build: `cargo build -p feral-processes-gui`. Expected before the bump: FAIL, `expected an array with a fixed size of 113 elements, found one with 114`.
- [ ] **Step 2: Run the gui censuses.** `cargo test -p feral-processes-gui` — `every_screen_draws_a_refusal_exactly_once` drives every `Mode` in `ALL_MODES` through `draw`, so it fails on a mode with no arm. Expected: FAIL.
- [ ] **Step 3: Add the draw arm.**
- [ ] **Step 4: Run the gui suite again.** Expected: PASS.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 10: The manual page, the seams, and the gate

**Files:**
- Create: `assets/help/<nn>-honeypots.md` — or a section in an existing page; see below
- Modify: `.claude/skills/seams/` (one reference file section per seam)
- Modify: `CLAUDE.md` (the one-sentence rules)
- Memory graph: `seam:<slug>` entries

**The help page.** `assets/help/README.md` is the schema — five block rules, no front matter, and the **filename is both the ordering and the id a link points at**. Read it before writing. Four censuses bite: every page parses and every link resolves; no page carries more than nine links; no page names a hidden key (the `W` wield key is deliberately undocumented); and the prose census at `tests/assets.rs:2642`. **Decide first whether this earns its own page or a section in `70-supplies.md`** — a honeypot is a crafted consumable and the manual's existing pages may already be the right home. A new page changes the index's length, which the shipped-page censuses count.

**The four seams, three writes each, in the order the `seams` skill documents** (the argument to the memory graph, the trap to the skill's reference file, the one-sentence rule to `CLAUDE.md`). Each rule in `CLAUDE.md` is **one sentence — a budget, not a style**; that file is loaded on every turn and reached 151 KB by letting each seam's trap creep back in beside its rule.

1. **A trap's quality is authored on the item def, never rolled per copy.** *Argument for the graph:* an instanced consumable cannot stack in `Inventory` (whose `count`/`take` read the first matching row), cannot sit in a `Stock` output buffer (keyed by a bare `ItemId`), and cannot be the subject of a work order. It would need a fourth player store on `DownedPrograms`' precedent to buy nothing the player can see. A better honeypot is a second `.ron` file behind a later research node.
2. **A honeypot clamps rarity before rolling condition.** `downed_program_for`'s boss-floor rule, second caller — `roll_condition` prices condition against the shipped rarity and `grade()` folds both.
3. **A honeypot's capture spends no `GameRng` until its period elapses, and honeypots roll in `(x, y)` order.** The countdown is what keeps fifty of them off the seeded stream; the sort is what keeps two elapsing together from resolving by bevy's unstable query order.
4. **A honeypot resolves its def live by id, against `ActiveContract`'s precedent.** It is a device standing in the world, like a nest, not an agreement already signed — which is also what lets a retuned `.ron` retune one already on the ground.

**Two things to record as known and deliberate, not as defects:**

- **The extraction leak.** `extraction_yield` multiplies `DownedProgram::grade()`, and the drop-neutrality gate behind `TOOL_BASE_UNITS` measures per *extraction*, not per kill. Honeypots are a new source of downed programs outside the kill path, so that gate cannot see them. The rarity ceiling, the condition penalty and `carried: None` are what hold the economy; no test does. **This is the number a playtest is for.**
- **A sprung honeypot does not join `Game::attention`, deliberately.** It would be a legitimate row — the downed store has a real capacity in `MAX_DOWNED_PROGRAMS` — but it costs a new `AttentionRow::kind` arm threaded through `hud::column::tab_of`'s exhaustive match, and threat rows sort ahead of it so it would rarely lead the badge anyway. The refusal already logs when collection fails. This is the natural follow-up *after* the feature has been played, not part of it.

- [ ] **Step 1: Write the help content** and run `cargo test -p feral-processes-engine assets::` for the four help censuses.
- [ ] **Step 2: Write the four seams,** all three places each, in the skill's order.
- [ ] **Step 3: Run the full workspace.** `cargo test --workspace`, unpiped. This is the final gate; passing only the tests this branch wrote is not evidence of correctness.
- [ ] **Step 4: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**
- [ ] **Step 5: Say plainly, once, that a green suite is not evidence of play.** Nothing in this feature has been on a screen: the placement prompt, the honeypot's `^` and `U` against a real biome's ground, whether `TRAP_PERIOD_TICKS` and `TRAP_CAPTURE_CHANCE` feel like anything at the keyboard, and whether the field fills the downed store faster than the player can walk it. Hand those to the user.

---

## Self-review against the spec

Checked section by section.

| Spec § | Covered by |
|---|---|
| §1 what it is | Tasks 4, 5, 6 |
| §2 decisions 1–8 | 1 (§6 hold): Task 5 + 6. 2 (never expires): omission, no expiry code in Task 5. 3 (destroy returns nothing): Task 6 test. 4 (cap): Task 4. 5 (blocks the player only): Task 6, an omission with a stated reason. 6 (worse than a kill): Task 5 (`carried: None`, condition penalty, no loot path). 7 (authored quality): Tasks 1–2. 8 (survives a breach): Task 7 |
| §3 why quality is authored | Task 1 shape + Task 10 seam 1 |
| §4 the gating | Task 2, with the four checks named |
| §5 the item field | Task 1, including the `item_effects` obligation and the README |
| §6 the entity | Task 3, plus the `entity_label` correction stated up front |
| §7 placement | Tasks 4 and 8 |
| §8 the tick | Task 5, with the `roll_condition`-is-not-a-roll correction |
| §9 collecting/blocking/destroying | Tasks 6 and 8 |
| §10 save | Task 7 |
| §11 tuning | Tasks 4 and 5 |
| §12 what this touches | corrected up front (gui) and in the file table |
| §13 test intent | all eleven items placed: ceiling and boss in Task 5, `carried` in Task 5, no-draw in Task 5, draw order in Task 5, `(x,y)` determinism in Task 5, full store in Task 6, breach in Task 7, `d` on the surface in Tasks 6 and 8, no-`trap:`-def refusal in Task 4, the two asset censuses in Task 2 |
| §14 seams | Task 10 |
