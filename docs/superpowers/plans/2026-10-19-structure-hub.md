# Plan: `c` opens one hub per structure

Spec: `docs/superpowers/specs/2026-10-19-structure-hub-design.md`. Read it
first; this plan carries files, interfaces, test intent and gates, not code.

Conventions for every task: TDD (failing test, then code), `cargo fmt`,
`cargo clippy --workspace --all-targets` clean, commit at green. Iterate with
`cargo test -p <crate> <name>`; the full gate is Phase 4. `unset
FERAL_DEV_NO_SIEGES` before any test run. Never `git checkout` another
agent's work; never push. Read `.claude/rules/seams-base.md` and
`seams-screens.md` before Phase 2; `drawing-seam.md` before Phase 3.

Anchors (0.19.12): `adjacent_interactions`, `InteractionKind`,
`transfer_direction` `engine/src/game/base/interact.rs`; `transfer_offer`
`base/transfer.rs:79`, `adjacent_racks` `:160`, `rack_offer` `:192`,
`transfer_room` `:283`, `transfer_items` `:302`, `refuse_transfer` `:522`;
`adjacent_stock`/`reachable_stock`/`indexes_storage_here`/`take_from_adjacent`
`base/collect.rs:96-187`; `adjacent_depots`/`supply_sinks`/`deposit_room`/
`depots_for_put`/`give_to_adjacent` `base/deposit.rs`; `adjacent_machines`
`base/machine_recipe.rs:36`; `adjacent_depot_entities`
`base/depot_filter.rs:238`; `adjacent_siphons` `base/siphon.rs:114`;
`adjacent_splice_rig` `implants.rs:234`; `structure_manifest`
`inspection.rs:711`. App-core: `TransferSource` `lib.rs:87`, `Mode::*`
`lib.rs:1587-1785`, `HubPage` `:2748`, `Hub`/`hub_of` `:2857-2900`;
`handle_hub_nav` `app/base_report.rs:21`; `run_interaction` /
`handle_interact_direction_key` `app/building.rs:745,776`; `Interact::Ask`
`app/playing.rs:680`, map keys `F`/`P`/`I` `:551-560`; openers
`app/{transfer,rig_tool,siphon,splice_rig,depot_filter,machine_recipe}.rs`;
`leave_basket` `app/basket.rs:280`. Gui: `render/mod.rs:1275` (direction
prompt), `:1622` (hub dispatch); `render/base_report/mod.rs:58,93`,
`frame.rs:73` `paint_frame`, `:391`.

## Phase 1: engine — the cycle, its pages, a scoped transfer

One crate. Delete nothing app-core still calls until Phase 2 (keep the
pooled functions compiling; Phase 2 T5 deletes them).

### T1. `Game::adjacent_structures`
- New `engine/src/game/base/structure_hub.rs` (replaces `interact.rs` at
  Phase 2's end), re-exported from `lib.rs`:
  - `pub enum StructurePage { Inventory, Filter, Recipe, Tool, Siphon,
    Splice, Grow, Rebuild, Info }` — declaration order is rail order.
  - `pub struct AdjacentStructure { entity: Entity, dir: (i32, i32), name:
    String, pages: Vec<StructurePage> }`.
  - `pub fn adjacent_structures(&self) -> Vec<AdjacentStructure>`: guards as
    `adjacent_interactions` (base space, no game over, no battle). Every
    structure entity with a footprint cell on an `ORTHOGONAL` neighbour,
    once, ordered by its first side in N,E,S,W order (`(0,-1),(1,0),(0,1),
    (-1,0)` — check `ORTHOGONAL`'s own order; add a `CLOCKWISE` const if it
    differs). Entities whose `pages` is `[Info]` alone are dropped.
  - `fn structure_pages(&self, e) -> Vec<StructurePage>`, capability-driven:
    Inventory = has a shelf/stock it takes from or puts to, is a rack, or
    indexes storage (Index Terminal); Filter = a Depot (whatever
    `adjacent_depot_entities` tests); Recipe = `adjacent_machines`' test;
    Tool = def has `strips`; Siphon / Splice = whatever `adjacent_siphons` /
    `adjacent_splice_rig` test, per entity; Grow = farm plot with crops
    loaded; Rebuild = awaiting rebuild site; Info always, last. Reuse the
    existing predicates — extract a per-entity `fn is_x(&self, e) -> bool`
    from each `adjacent_*` and have both call it (CLAUDE.md "a call, not a
    copy"). Teardown Rig's Inventory only when it holds stock.
- Tests (`engine/src/tests/structure_hub.rs`, a `dev-saves/` template or
  existing base fixture): Depot E + machine N → order [machine, Depot],
  pages `[Inventory, Recipe, Info]` / `[Inventory, Filter, Info]`; a 2-wide
  structure touching N and E appears once at N; a wall beside you → empty;
  siphon → `[Siphon, Info]`; Index Terminal → `[Inventory, Info]`; none in
  battle.

### T2. The transfer scoped to one structure
- `pub enum TransferAt { Structure(Entity), PackOnly }` in `transfer.rs`.
- Add `_at(at: TransferAt)` forms: `transfer_offer_at`, `rack_offer_at`,
  `transfer_room_at`, `transfer_items_at(at, &TransferBasket)`. One private
  resolver `fn shelves_of(&self, at) -> Shelves { stock: Vec<Entity>,
  depots: Vec<Entity>, sinks: …, racks: Vec<Entity> }`: Index Terminal →
  today's terminal-wide sets (`reachable_stock`/`reachable_depots`); any
  other structure → just itself in whichever set it qualifies for;
  `PackOnly` → empty sets (pack-only carrier rows as today). The pooled
  bodies become calls into the `_at` bodies with today's adjacency sets so
  both coexist until T5.
- Machine shelf keeps today's take/put rules, scoped. Put budget = that
  structure's room. Carrier indices index `rack_offer_at(at)`.
- Tests (`tests/transfer.rs`): Depot A and B both adjacent — A's offer lists
  only A's stock, a commit takes from A only, A's room is the put cap (not
  A+B); a machine's offer is its own shelf; terminal offer equals today's
  terminal offer; rack offer lists that rack's slots and pack carriers.
  Mutation-check: widen `shelves_of` to the pooled set → a test fails.

Gate: `cargo test -p feral-processes-engine`.

## Phase 2: app-core — the hub, `c`, the removals

Run the `design-patterns` dialog before T3 (dynamic page list). Recommended
shape, to confirm in the dialog: `HubPage.screens` stays `&'static [Mode]`;
`Hub` gains `Structure`; `App` gains `structure_hub: Option<StructureHub {
cycle: Vec<AdjacentStructure>, at: usize }>`; one `const` table maps each
`StructurePage` to a static `HubPage` (title + one-screen slice); `Hub::pages`
becomes `fn pages(self, app: &App) -> Vec<&'static HubPage>` (static hubs
return their table, Structure filters by `cycle[at].pages`); `hub_of`
becomes `App::hub_at(&self)`. Modes with an `Entity` payload
(`Rebuild { site }`) match by discriminant in the table.

### T3. Hub state, open, nav
- `app/structure_hub.rs`: `open_structure_hub()` (from `c`), `step(±1)`
  (Tab/Shift+Tab, wrapping), `refresh()` each frame/turn (current entity
  gone from `adjacent_structures` → step to next, none → close to map).
  Opening, stepping and paging spend no turn. Opens on first page, cursor
  in the page (`hub_on_rail = false`); no `hub_last` slot.
- Entering a page calls that page's `_for(entity)` opener (T4).
- `handle_hub_nav`: a per-hub rule, `Hub::Structure` → in-page Left/Right
  fall through to the screen; Esc page → rail → map; `[`/`]`/rail Up/Down
  unchanged; Tab/Shift+Tab → `step`. Leaving Inventory (Tab, page turn,
  Esc) goes through `leave_basket`.
- Tests (`app-core/src/tests/structure_hub.rs`): `c` beside Depot opens on
  Inventory in one press; Tab cycles N→E→S→W and wraps; Esc page→rail→map;
  Left/Right on Inventory move stock, not pages; walking away closes; a
  basket left by Tab is discarded; no turn spent (turn counter unchanged).

### T4. Pages point at the hub's structure
- Openers become `_for(entity)`: `open_transfer_for(TransferAt)`,
  `open_rig_tool_for`, `open_siphon_for`, `open_splice_rig_for`,
  `open_depot_filter_for`, `open_machine_recipe_for` (exists); Grow / Rebuild
  / Info reuse what `run_interaction` opens today. Each screen drops its
  `of` field, own `Tab` cycle and own "close when not adjacent" check (the
  hub's `refresh` owns that). A screen's own Esc (confirm popup,
  sub-list) still wins before hub nav.
- `commit_transfer` with `TransferSource::Base` calls `transfer_items_at`;
  `TransferSource::Base` carries the `TransferAt`. Outpost unchanged.
- Rack Inventory keeps pack carriers (put side).

### T5. `c` and the removals
- `c`: `adjacent_structures` non-empty → hub; else pack-only carriers →
  `open_transfer_for(PackOnly)`; else `refuse_transfer()`.
- Delete: `Mode::InteractDirection`, `Interact::Ask`/`Run`,
  `handle_interact_direction_key`, `run_interaction`, the c-again rule, map
  keys `F`/`P`/`I`, `InteractionKind`/`Interaction`/`adjacent_interactions`
  /`transfer_direction`/`interact.rs`, the pooled `transfer_offer`/
  `rack_offer`/`transfer_room`/`transfer_items` if now uncalled (rg every
  caller first — staff/hauling may use `take_from_adjacent`; keep what
  is still used). Same commit: delete gui `draw_direction_prompt` and its
  `Mode` arms (`render/mod.rs:1275,1750`) so the workspace compiles.
- Reshape `tests/{interact,transfer,depot_filter,machine_recipe,extraction,
  siphon,splice_rig}.rs`: two-`c` and `F`/`P`/`I` presses → `c` (+Tab /
  `]`). Add: pressing `F`, `P`, `I` on the map does nothing.

Gate: `cargo test -p feral-processes-app-core`, then `cargo build
--workspace`.

## Phase 3: gui — draw the fourth hub

### T6. Structure hub frame
- `render/base_report/mod.rs` and `frame.rs:391`: read pages from
  `app.hub_at()` instead of the static `hub_of`; title from
  `Hub::Structure` = `"{name} — {side} ({n} of {m})"`, rail labels from the
  page table, key bar adds `Tab next structure`. `render/mod.rs:1622` hub
  dispatch covers it. Only `paint.rs` names egui.
- Respect `gui-text-never-clips-or-wraps` (long modded names): truncate the
  name, never the "n of m".
- Visual check: `cargo run -- --template <base template> --keys "c"
  --screenshot out.png` beside a Depot and beside a machine+Depot (then
  `Tab`); Read both PNGs.

## Phase 4: docs, seams, gate, review

### T7. Docs and seams
- `assets/help/20-controls.md`, `60-your-base.md`,
  `62-production-lines.md`, `64-implants.md`, `75-extraction.md`;
  `description` in `assets/structures/power_siphon.ron`,
  `teardown_rig.ron` (and rg `assets/` for any other "press F/P/I" or
  "press c twice"). `CHANGELOG.md` unreleased entry. Not the manual or
  README.
- Seams, all three places (graph `seam:*`, `seams` skill,
  `.claude/rules/seams-base.md`), per the spec's four bullets.

### T8. Gate and review
- `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
  `cargo test -p feral-processes-engine balance_sim` (should not move).
- Final whole-branch review (opus), diff as a file. Reviewer re-derives:
  no pooled offer survives on any structure page; Tab order is N,E,S,W;
  Info-only structures never enter the cycle; no stale `F`/`P`/`I` text.
