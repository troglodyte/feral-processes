# Plan: Compile crafts the missing parts

Spec: [2026-10-28-compile-crafts-missing-parts-design.md](../specs/2026-10-28-compile-crafts-missing-parts-design.md).
Branch `compile-crafts-missing-parts`, primary checkout (the user plays from
here). No save, schema or `.ron` change; no ADR.

Two phases, each one green commit or more. Phase 1 adds behaviour and keeps
every existing caller compiling; Phase 2 deletes the page. Running both in one
context is fine. A subagent per phase works too: brief it with this file plus
the spec. Forbid push, checkout of others' work, `git add -A` and stash.

## Phase 1: engine (`crates/engine`)

### Interfaces

`routine_craft/plan.rs`: the pure decide step, shared by preview and Compile.

```rust
pub struct BuildInput<'a, H, K, R> {   // or plain fields; generics are the builder's call
    pub name: &'a str,
    pub parts: &'a [Subroutine],
    pub cap: usize,
    pub held: H,     // Fn(&Subroutine) -> u32, parts in reach
    pub known: K,    // Fn(&Subroutine) -> bool, researched
    pub in_reach: R, // Fn(&ItemId) -> u32, disks and materials in reach
}

pub struct BuildPlan {
    pub name: String,
    pub spend: Vec<(Subroutine, u32)>,   // held parts used
    pub craft: Vec<(Subroutine, u32)>,   // shortfall crafted
    pub bill: Vec<(ItemId, u32)>,        // summed: Routine Disk first, then materials
}

pub enum PlanError { Name(String), Build(BuildError),
                     Unresearched(Vec<Subroutine>),        // replaces Missing
                     Short(Vec<(ItemId, u32, u32)>) }      // item, have, need
```

- `plan_build(input) -> Result<BuildPlan, PlanError>`. Refusal order: name,
  `validate_build`, unresearched shortfall, short items. One pure helper,
  `craft_bill(craft: &[(Subroutine,u32)]) -> Vec<(ItemId,u32)>`, sums disks
  (one per crafted part) and `material()` across parts, merging equal ids.
  The preview and `plan_build` both call it.
- `part_options`: `PartOption` gains `craft_cost: Option<(ItemId, u32)>`, which
  is `Some(material(part))` when `known(part)`, else `None`. The disk is
  implied (always 1). Game maps the `ItemId` to a name: either change the field
  to `Option<(String, u32)>` in `Game::build_part_options`, or keep `ItemId`
  and add a name. That's the builder's call.

`views/routine_craft.rs` `BuildPreview`:
- `parts_missing` stays (it is the shortfall).
- Add `materials: Vec<MaterialLine { label: String, have: u32, need: u32 }>`
  (the bill, with names and reach counts) and `unresearched: Vec<String>`
  (labels of missing parts that can't be crafted).

`game/routine_craft.rs`:
- `build_routine`: gather → `plan_build` → apply. Apply means
  `pay_from_reach(bill)`, then `grant_loot(part, n, LootSource::Craft)` for each
  crafted part (silent; it writes the `Record::Acquire` receipt), then
  `pay_from_reach(spend + craft)`. The plan already proved reach, so the second
  pay cannot fail; `?` on it anyway. Log `"You compile {n} missing part(s) and
  build {name}."` when `n > 0`, otherwise the existing line. Keep "part"/"parts"
  grammatical.
- Crafting needs `require_workbench_free` and nothing else beyond what
  `craft_subroutine` checks today.
- `build_preview`: fill `materials` / `unresearched` from the same
  `plan_build` gather (or `craft_bill` over the shortfall when the plan
  errors earlier). `blocked` uses `PlanError`'s `Display`. Add the new
  variants: "Research {label} first." and "You need {need} {item} (you have
  {have})." for the first short item.
- Leave `craft_subroutine` / `subroutine_rows` in place until Phase 2.

### Tests (`tests/routine_craft.rs`, `tests/bench_stores.rs`; red first)

1. Compile with no parts held but disks and materials in reach builds the
   routine, spends exactly the bill, and leaves no part items.
2. Some parts held: only the shortfall is crafted (held parts spent, bill
   counts the rest only).
3. Two crafted parts sharing a material: the bill sums them. With stock for
   one but not both, Compile refuses and spends nothing (all-or-nothing).
4. Missing unresearched part: refused with the "Research … first." reason,
   nothing spent. Held-unresearched part used once is fine; needed twice is
   refused.
5. Refusal order: an empty name still wins over shortages.
6. Preview `materials` equals what Compile spends (assert against inventory
   deltas), and is empty when every part is held.
7. `PartOption.craft_cost` is `Some` for researched parts and `None` for
   held-but-unresearched ones.
8. bench_stores: Compile crafts using Depot stock first, then the pack, and a
   Depot is out of reach outside base space. Port the three
   `craft_subroutine` tests to the Compile path. Do it here, not in Phase 2,
   so their intent survives.
9. A `Record::Acquire` with source Craft is written per crafted part. Find the
   test that already checks `craft_subroutine`'s receipt, if there is one, and
   retarget it.

Mutation-check tests 3 and 6 (drop the summing, drop a bill line).

### Gate
`cargo test -p feral-processes-engine routine_craft bench_stores`, then
`cargo clippy --workspace --all-targets`, `cargo fmt`. Commit.

## Phase 2: remove the Subroutines page; show the new figures

### Delete (grep each first: `rg -n "craft_subroutine|subroutine_rows|SubroutineRow|Subroutines\b|subroutines_visible" crates`)
- engine: `StructurePage::Subroutines` and its `page(bench, …)` line
  (`game/base/structure_hub.rs`), `Game::craft_subroutine`,
  `Game::subroutine_rows`, `views::SubroutineRow`, and the remaining
  `tests/routine_craft.rs` tests on them (the ~178–200 refusals, the `:824`
  cost row: port its cost assertion to `PartOption.craft_cost` if test 7 does
  not already cover it). Fix doc comments that name `craft_subroutine`
  (`items.rs:139`, `items_db.rs:679`) to name `build_routine`. Keep
  `blank_disks_held` only if something still calls it.
- app-core: `Mode::Subroutines` (`lib.rs:1731`, `:2524`), `subroutines_visible`
  and its re-export (`lib.rs:50`), `handle_subroutines_key`, the
  `input.rs:300` arm, the `list_filter.rs` arms, the `structure_hub.rs`
  `HubPage` and dispatch arms, and the `tests/routine_craft.rs` cases at
  ~56/156/168. Update the module doc (now two pages).
- gui: `subroutine_page_rows`, the `render/mod.rs` arms (~1255, 2023, 2256),
  and the module doc.

### Add (gui `render/routine_craft.rs`)
- `preview_rows`: after the existing lines, a "Materials" heading and one row
  per `MaterialLine`, `"{label} {have}/{need}"`, in the theme's good colour when
  `have >= need`, otherwise its bad colour. Use the colours the page already
  uses for blocked/ready; add no new colours. One red row per `unresearched`
  label, "… — research first". Nothing when both lists are empty.
- `pick_rows`: append the cost to each option, e.g. `held 0   1 disk + 4
  Bytecode`. Respect `gui-text-never-clips-or-wraps` and the popup row-width
  test. Read `.claude/rules/seams-screens.md` and `drawing-seam.md` first.

### Help
Rewrite `assets/help/51-building-routines.md`: the page list becomes Compile,
Build routine and Built routines; drop the Subroutines bullet; Build routine
describes Compile crafting missing parts from disks and materials, the
Materials list (green/red), and costs in the part list. Plain language; no
tick vocabulary.

### Tests
- app-core: the Workbench rail has no Subroutines page. Compile on a draft
  with materials but no parts builds it (one end-to-end key test through
  `handle_routine_build_key`).
- gui: if `pick_rows`/`preview_rows` have headless row tests, add one for a
  red and a green material line.

### Gate
`cargo clippy --workspace --all-targets`, `cargo fmt`, `cargo test
--workspace` (unset `FERAL_DEV_NO_SIEGES`). Screenshot check: `cargo run --
--template <a dev-save at a Workbench> --keys "…" --screenshot out.png`
(see `dev-saves/README`), then Read the PNG to confirm the Materials rows and
part-list costs render. Commit.

## Finish
Opus whole-branch review (diff as a file). Then stop. Landing (version bump,
CHANGELOG, tag) waits for the user's "deploy".
