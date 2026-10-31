# Batteries (ADR 0037)

ADR: [0037](../../adr/0037-batteries-store-spare-grid-supply.md) (Agreed).
Worktree `.claude/worktrees/ungate-power-cells`, branch `worktree-ungate-power-cells`.
Already on the branch: `caa8366f`, which ungates the Extractor's Power Cell recipe.
`no_research_node_is_left_unlocking_nothing` is **red** until Phase 1 lands. That is expected.

Read before starting: `.claude/rules/seams-base.md` (the grid entries) and `.claude/rules/content-schema.md`.
Gates per phase: `cargo fmt`, `cargo clippy --workspace --all-targets`, and the named tests.
Commit at every green step. Never push, never `git checkout`/`stash` over uncommitted work, and stage explicit paths.

## Phase 1: engine (one context)

**1a. Schema + content.** Write a failing test first: a census in `tests/assets/` that loads `battery.ron`, asserts `battery == Some(BatteryDef { capacity: 400, rate: 4 })`, checks that `power_grid` lists it in `unlocks_structures`, and checks that the def `runs_a_job()` is false.

- `structures.rs`: add `pub battery: Option<BatteryDef>` with `#[serde(default)]`, and `BatteryDef { capacity: u32, rate: u32 }`, deriving what `RepairDef` derives. Its doc comment states the supply-tick unit.
- `assets/structures/battery.ron`: id `battery`, name "Battery", a free glyph (check that the census of glyphs allows it), Cyan or Yellow, `build_cost: [("core_fragment", 12), ("power_cell", 2)]`, `room_tags: ["workshop"]`, plus a one-line description.
- `assets/research/power_grid.ron`: add `unlocks_structures: ["battery"]` and set the description to "Lets you build Batteries, which store the Grid's spare power for when supply falls short."
- `assets/structures/README.md`: document `battery`.
- The red census goes green here. Run `cargo test -p feral-processes-engine research` and fix any research-ladder or catalogue census that counts structures.

**1b. The pure decision.** Put it in `game/base/power.rs` (or `power/battery.rs` if `power.rs` is over about 1k lines):

```rust
pub(crate) struct Cell { charge: u32, capacity: u32, rate: u32 }   // one per battery, (x, y)-sorted
pub(crate) fn battery_flow(supply: u32, draw: u32, cells: &[Cell]) -> Vec<i32>  // + charge, - discharge
```

Surplus is shared out in order, each battery taking up to `min(rate, capacity - charge)`. A shortfall is covered in order, each battery giving up to `min(rate, charge)`, and never more than the shortfall.
Test the edge cases directly: no surplus, a full battery, an empty battery, a shortfall bigger than the total rate, and two batteries where the first fills before the second.

**1c. Wire it.**

- `components::BatteryCharge(pub u32)`, added in `Game::spawn_structure` (the one writer of component lists; see the seam) when `def.battery.is_some()`, starting at 0.
- `ledger`: collect batteries `(x, y)`-sorted, call `battery_flow` on pre-cut supply/draw, add the discharge to `supply` **before** the cut, and return the flow on `PowerLedger`.
  `Game::base_power` calls `ledger` fresh, so the header picks up the discharge for free.
- `power_grid_system`: apply the flow to each `BatteryCharge`. The ledger stays read-only.
- Test (engine, `tests/` grid file): a base with surplus charges a battery tick by tick and stops at the cap. Then drain the Recharger Node's fuel and check that the battery keeps a machine lit that would otherwise be cut, until it runs empty.
  Mutation-check this by removing the discharge line; the test must fail.

**1d. Save.** Add `StructureSave::battery_charge: u32` with `#[serde(default)]` and a doc comment in `power_fuel`'s style (zero is the correct default). Wire it in `lifecycle/save.rs` and `restore.rs` beside `power_fuel`.
Test a real save→load (a RON round trip can't catch a skipped field): charge a battery, save, load, and check that the charge is the same.

**1e. View.** `Game::base_battery() -> Option<(u32, u32)>` returns `(stored, capacity)` summed, or `None` with no battery. Add the per-structure charge to whatever view feeds the structure sheet (the `views/base.rs` row that carries `power_upkeep` fuel is the precedent).

Then run `cargo test -p feral-processes-engine balance_sim` and the full `cargo test --workspace`.

## Phase 2: gui + sprite (one context)

- `structure_sheet.rs`: `grid_header_row` gains the stored total when `base_battery()` is `Some`, e.g. "Grid 12/15 · Battery 220/400". Extend the existing tests in `render/building/tests/structure_sheet.rs`, including the 9999 width case, against `gui-text-never-clips-or-wraps`.
- The Battery's own sheet line shows `charge/capacity`.
- `assets/sprites/battery.png`: 16x16 RGBA. Follow `assets/sprites/README.md`'s palette and outline rules, and match the look of `extractor.png` and `recharger_node.png`. Draw it with a short Python/PIL script in the scratchpad, not in the repo.
  Check it on screen with `cargo run -- --template stack --screenshot out.png` after placing one (or any template with a base) and Read the PNG.
- `CHANGELOG.md`: an entry with no version number (the deploy picks the version). It covers both changes: Power Cells need no research, and Power Grid unlocks the Battery.

Final gate: `cargo test --workspace`, then an opus whole-branch review (diff written to a file), then deploy.
