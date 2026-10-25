# Workbench Compile page — plan

Spec: `docs/superpowers/specs/2026-10-25-workbench-compile-page-design.md`.
Branch `workbench-compile-page`, worktree `.claude/worktrees/workbench-compile-page`.
Three crates plus assets; no save-format change, no new `.ron` field, no ADR.

## Decision the spec left open

**Filter in app-core; no engine API change.** `CraftRecipe` already carries
`requires_structure: Option<String>`, so the field list is
`requires_structure.is_none()`. One app-core function owns the context:

```rust
// crates/app-core/src/app/crafting.rs
pub fn craft_list(app_hub_open: bool, game: &Game) -> Vec<CraftRecipe>
```

`at_bench` (hub open) returns `game.craft_recipes()` unchanged, otherwise only
field recipes. Signature is the builder's call (`&App` method vs free fn); the
rule is that **every** reader goes through it: `craft_visible`,
`handle_craft_key`, `list_filter.rs`'s Craft count, `COMPILE_ROWS`'s
`available` (always field context — the `[b]` path), and gui
`draw_craft_menu` (`render/crafting.rs:20`). gui tests at
`render/crafting.rs:523/677/703` use a fresh game with no hub, so they read
`craft_list(false, …)` or stay on `craft_recipes()`; either is fine as long as
it's deliberate.

## Phases

Each phase is one subagent (sonnet), TDD, commit per green step, ends at
fmt + clippy `--all-targets` + its crate's tests. Dispatch prompts forbid
push, `git checkout`/`stash`/`reset` of uncommitted work, and `git add -A`.

### Phase 1 — engine: page + data

Files: `crates/engine/src/game/base/structure_hub.rs`,
`assets/items/{scrap_ward,shiv_routine,packet_buffer,kinetic_edge,probe_service,handshake_forge}.ron`
(verify filenames), `assets/items/README.md` if it lists field recipes,
engine tests.

1. `StructurePage::Compile` between `Built` and `Info` (declaration order is
   rail order); `adjacent_structures` adds it for the Workbench beside the
   `Subroutines` line (~:115). Test: Workbench pages end
   `[Build, Subroutines, Built, Compile, Info]`.
2. Add `requires_structure: Some("workbench")` to the six `craftable` blocks.
   Census test: on a fresh game with everything researched, the set of
   recipes with `requires_structure == None` is exactly
   `{power_cell, ice_breaker, outlet}` (check real ids).
3. Fix engine tests that compile one of the six without a bench
   (`rg` the six ids in `crates/engine/src/tests/`) → bench fixture from
   `tests/support.rs` or a field recipe.
4. `cargo test -p feral-processes-engine balance_sim` (data changed).

Gate: `cargo test -p feral-processes-engine`.

### Phase 2 — app-core: one list, hub page, popups, exits

Files: `app/crafting.rs`, `app/structure_hub.rs`, `app/hub.rs`,
`app/input.rs` (`advance_compile`), `app/group_menu.rs`, `app/list_filter.rs`,
`lib.rs` (`base_popup_parent` only — no new logic in `lib.rs`), tests in
`src/tests/crafting.rs`, `src/tests/list_filter/pickers.rs`.

Read `.claude/rules/seams-screens.md` (hub nav: `handle_hub_nav`,
`screen_claims`, `HubKind`) first.

1. `craft_list` + route all four app-core readers through it. Test: base-menu
   list beside a Workbench holds only field recipes.
2. `structure_hub.rs`: `HubPage` entry for `Compile` and
   `StructurePage::Compile => self.mode = Mode::Craft`, copied from
   `Subroutines` (Esc/rail/arrows/`take_filter_key`/`screen_claims`).
   `handle_craft_key`'s own Esc → `close_screen` only when no hub.
   Test: `hub_at()` is `None` for `Mode::Craft` with `structure_hub == None`.
3. `base_popup_parent`: `CraftQuantity | Compiling => Some(Mode::Craft)`.
4. Exits: `advance_compile` finish, `handle_compiling_key` abort,
   `commit_craft` refusal/zero, missing `pending_craft` → `Mode::Craft` when
   `structure_hub.is_some()`, else unchanged `Mode::Playing`. Prefer one
   helper (`fn compile_exit_mode(&self) -> Mode`) over four inline branches.
   Tests: hub → Compile → quantity → bar finishes → `Mode::Craft`, hub open;
   abort same; base-menu path still ends in `Playing`.
5. Move tests that craft one of the six from the base menu to a bench
   fixture or a field recipe.

Gate: `cargo test -p feral-processes-app-core`.

### Phase 3 — gui + help

Files: `crates/gui/src/render/crafting.rs`, `render/mod.rs` (mode census
~1956/2188, hub-frame-behind-popup), `assets/help/20-controls.md`,
`assets/help/51-building-routines.md`.

1. `draw_craft_menu` reads `craft_list`. Hub frame draws behind
   `CraftQuantity`/`Compiling` via `base_popup_parent` (check it already
   does; add only if not). Census still covers all three modes.
2. Help text per spec's Docs section. Plain language; no CHANGELOG yet.
3. Screenshot check: `cargo run -- --template bench-economy --keys "…" --screenshot out.png`
   if a template stands beside a Workbench; else skip and say so.

Gate: `cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES`).

### Phase 4 — review and land (controller)

- Final whole-branch review, opus, diff given as a file.
  Reviewer checks: no reader of the recipe list bypasses `craft_list`; hub
  survives both popups; base-menu exits unchanged.
- Fixes get their own review.
- Land with the `deploy` skill: merge → version bump + `CHANGELOG.md`
  (patch/minor per its preamble; no save break) → tag → push
  `--follow-tags` (ask first) → `git worktree remove`.
