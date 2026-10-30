# Patch Node Radius Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Patch Node repairs only what its tier-scaled circle covers. It pulses heals in a fought siege. Its light-green circle shows on hover and in previews, and a green + marks what it heals.

**Architecture:** The Shield's coverage core (`shields/`) becomes a shared `reach/` module (`RadiusDef`), which both `shield_radius` and the reshaped `RepairDef` use. Off-battle regen sums per-structure coverage instead of a base-wide rate. The siege pulse is a round-upkeep step modelled on `fire_turrets`. Display reuses the Shield circle path, generalised with a kind for colour.

**Tech Stack:** Rust, bevy_ecs (engine), app-core, bevy/egui gui through `Painter`.

**Spec:** `docs/adr/0036-patch-nodes-cover-a-radius.md` (Agreed). Prior art: ADR 0035 and `docs/superpowers/plans/2026-10-30-shield-radius.md`.

## Global Constraints

- Patch Node radius: `base: 4, per_tier: 2` (4/6/8/10/12). Off-battle `amount: 1` per covering node per `STRUCTURE_REGEN_INTERVAL`; it does not scale with tier.
- Siege pulse: `PATCH_SIEGE_PULSE_ROUNDS = 2`, `PATCH_SIEGE_HEAL = 2`, in `tuning/base.rs`. Heals non-`Hostile` bodies and structures only. Overlapping nodes stack. Capped at the maximum.
- Off-battle a node heals structures only.
- `Perk::Failover`: not offered; the variant, its `.ron` def and `repair_rate_bonus` stay; a held level still adds base-wide.
- No save-format change. The pulse is decided by `TacticalBattle::round`.
- Circle colour is light green, `(0.6, 1.0, 0.6)`. The Shield stays `SHIELD_BLUE`.
- Never `git checkout`/`stash`/`add -A`/push. Stage explicit paths. `cargo fmt` and `cargo clippy --workspace --all-targets` clean after each task.
- Do not update `docs/manual.md`, `README.md` or `TODO.md`.

## Review Focus

1. **A multi-cell structure on a circle's edge.** If any footprint cell is covered, the structure is healed (`covers_any`, same as Shields). Pinned in Task 1.
2. **Enemies inside a circle in a siege.** They are never healed. Pinned in Task 3.
3. **A Patch Node destroyed mid-siege.** It stops pulsing from the next round. Pinned in Task 3.
4. **Full-health targets.** No green + and no heal cue when nothing was restored, off-battle and in a siege. Pinned in Tasks 2 and 3.
5. **A save loaded mid-siege on a pulse round.** Pulsing is round-driven and runs only at round wrap, so reloading must not double-pulse. No new state; Task 3 asserts that the pulse fires only on rounds where `round % 2 == 0`.

---

### Task 1: Shared reach core, reshaped `RepairDef`, regen by coverage

**Files:**
- Move: `crates/engine/src/shields/` → `crates/engine/src/reach/` (`git mv`). Update `lib.rs` and the `structures.rs:9` re-export.
- Modify: `crates/engine/src/structures.rs` (`RepairDef` at ~381, doc at ~755), `game/base/defences.rs` (paths only), `game/base/upkeep.rs` (`structure_regen`, `total_repair_rate`)
- Modify: `assets/structures/patch_node.ron`, `assets/structures/README.md` (`repair` block ~683, `shield_radius` doc ~636), `assets/help/60-your-base.md:170`, `docs/structures.md:59`, doc comments in `tuning/perks.rs:192-200`, `tuning/economy.rs:156`, `tuning/base.rs:327`
- Test: `crates/engine/src/reach/tests.rs`, `crates/engine/src/tests/raids.rs` (patch tests ~1393-1780)

**Interfaces:**
- Produces:
  - `reach::RadiusDef { pub base: u32, pub per_tier: u32 }` (derive `Clone, Copy, Debug, Serialize, Deserialize`)
  - `reach::{radius_at, covers, covers_any, covered_tiles}`, with signatures unchanged
  - `RepairDef { pub amount: u32, pub radius: RadiusDef }` (stays `Copy`)
  - `pub(crate) struct PatchReach { pub entity: Entity, pub anchor: (i32,i32), pub radius: u32, pub amount: u32 }`
  - `fn patch_reaches(&self) -> Vec<PatchReach>`, sorted by entity (in `game/base/defences.rs` beside `shield_reach`)
  - Pure `pub(crate) fn patch_mend(reaches: &[PatchReach], cells: &[(i32,i32)]) -> u32`, which sums `amount` over the reaches covering any cell
  - `pub fn patch_radius_for(&self, kind: &str, tier: u32) -> Option<u32>`
- `total_repair_rate` is deleted. Its replacement: each structure gains `patch_mend(&reaches, &self.structure_cells(e)) + perks::repair_rate_bonus(..) + medic`.

- [ ] **Step 1: Move the module and rename the type.** `ShieldRadiusDef` becomes `RadiusDef` everywhere; `shield_radius` keeps its field name. Update the module doc so it no longer says Shield-only. Run `cargo test -p feral-processes-engine reach` and `shield`. Expected: PASS (pure rename).
- [ ] **Step 2: Write failing tests** in `tests/raids.rs`. Rewrite the existing patch tests against the new rule, and replace `patch_node_per_tier()` with an `amount` reader:
  - `a_patch_node_mends_a_structure_inside_its_circle`: node at tier 1, a damaged structure 3 tiles away, one regen interval. Assert hp `+= 1`.
  - `a_patch_node_leaves_a_structure_outside_its_circle`: same, but 6 tiles away at tier 1. Assert hp unchanged.
  - `upgrading_a_patch_node_widens_its_circle_not_its_mend`: tier 3 (radius 8), structure 7 tiles away. Assert `+= 1`, not 3.
  - `overlapping_patch_nodes_stack`: two nodes covering one structure. Assert `+= 2`.
  - `a_patch_node_mends_a_structure_whose_edge_cell_it_covers`: a multi-cell footprint with only the far cell outside. Assert healed.
  - Keep `a_patch_node_repairs_itself`, `structure_regen_does_not_exceed_max_durability`, and `failover_repairs_the_base_with_no_patch_node_standing` (base-wide, unchanged).
  - Delete `patch_node_repair_scales_with_its_upgrade_tier` and `a_deployed_patch_node_upgrades_and_repairs_harder_for_it`. Their behaviour is now the third test above.
  - In `game/base/defences.rs` tests or `reach/tests.rs`: a unit test of `patch_mend`, with `[]` → 0, one covering → amount, and one not covering → 0.
- [ ] **Step 3: Run them.** `cargo test -p feral-processes-engine patch_node`. Expected: FAIL (compile error on `amount`, or wrong hp).
- [ ] **Step 4: Implement** the `RepairDef` reshape, `patch_reaches`, `patch_mend`, the `structure_regen` rewrite and `patch_radius_for`. In `patch_node.ron`, set `repair: Some((amount: 1, radius: (base: 4, per_tier: 2)))` and the description "Repairs damaged structures within its circle, which grows with each upgrade. 1 Durability every 20 ticks." Rewrite the `RepairDef` and `structure_regen` doc comments; the "deliberately has no radius" and "works base-wide" claims go. Update the README, help and tuning doc lines listed above.
- [ ] **Step 5: Run.** `cargo test -p feral-processes-engine raids` and `cargo test -p feral-processes-engine assets`. Expected: PASS.
- [ ] **Step 6: Commit.** "Patch Nodes mend only what their circle covers (ADR 0036)".

### Task 2: Green + on the field for a Patch Node mend

**Files:**
- Modify: `crates/engine/src/resources.rs` (`EffectKind` ~783), `game/base/upkeep.rs` (`structure_regen`)
- Modify: `crates/gui/src/fx.rs` (`begin_frame` ~1090, `cell_marks`), `crates/gui/src/render/base/playing.rs` (field draw ~285)
- Test: `crates/engine/src/tests/raids.rs`, `crates/gui/src/fx.rs` tests

**Interfaces:**
- Consumes: `patch_mend` from Task 1.
- Produces: `EffectKind::Patched { amount: u32 }`. It is pushed at the structure's anchor when the patch-node share actually restored hp (`min(patch share, hp actually gained) > 0`). It is not pushed for Failover-only or Medic-only gains.

- [ ] **Step 1: Write failing engine tests.**
  - `a_patch_mend_cues_a_green_plus_on_the_mended_structure`: drain `take_effects()` and assert exactly one `Patched { amount: 1 }` at the structure's position.
  - `a_full_structure_under_a_patch_node_cues_nothing`.
  - `a_failover_only_mend_cues_nothing`.
- [ ] **Step 2: Run.** Expected: FAIL.
- [ ] **Step 3: Implement the cue in `structure_regen`.** In gui, route `EffectKind::Patched { amount }` to `cell_marks` as `MarkKind::Heal(amount as i32)` at the field tile. Call `Fx::draw_cell_marks` on the surface map. Check its tile-to-pixel mapping against `render/tactical/map.rs:385`, and adapt it the way the map pane passes its origin.
- [ ] **Step 4: Write a gui test** (in `fx.rs` tests, modelled on the shield-flash test): a `Patched` effect produces one `MarkKind::Heal(1)` cell mark. Run `cargo test -p feral-processes-engine patch` and `cargo test -p feral-processes-gui fx`. Expected: PASS.
- [ ] **Step 5: Screenshot check.** Use `cargo run -- --template <a base template with a Patch Node; README lists them> --keys "…" --screenshot out.png` after damaging a structure, or skip it if no template has a node, and say so. Read the PNG.
- [ ] **Step 6: Commit.** "A Patch Node mend shows a green + on the field".

### Task 3: Siege pulse

**Files:**
- Create: `crates/engine/src/game/siege/patch.rs` (register it in `game/siege/mod.rs`)
- Modify: `crates/engine/src/tuning/base.rs` (two constants), `tactical/turn.rs:1935` (`tactical_round_upkeep`, after `fire_turrets`)
- Test: `crates/engine/src/tests/siege/patch.rs` (new; register it beside `siege/shields.rs`)

**Interfaces:**
- Consumes: `RepairDef.radius`, `reach::{radius_at, covers_any}`, `battle.cells_of`, `siege_origin`, `restore_hp`, `cue_tactical_fx`.
- Produces:
  - `pub const PATCH_SIEGE_PULSE_ROUNDS: u32 = 2;`
  - `pub const PATCH_SIEGE_HEAL: u32 = 2;`
  - `pub(crate) fn pulse_patch_nodes(&mut self)`

Behaviour:
- Return unless `battle.siege_pack != 0` and `battle.round % PATCH_SIEGE_PULSE_ROUNDS == 0`.
- The nodes are the board bodies whose structure def has `repair`. A destroyed node is no longer a body, so it never pulses.
- The targets are the non-`Hostile` bodies, sorted by entity. For each, count the covering nodes, using field cells (`cells_of` + `siege_origin`), and set `heal = count * PATCH_SIEGE_HEAL`.
- For a body with `Structure`: add to `Durability.hp`, capped at `max_hp`. If gained > 0, `cue_tactical_fx(target, TacticalFxKind::Heal(gained))`.
- Otherwise: `restore_hp(target, heal)`. It cues on its own and respects `HealBlock`.
- No RNG.

- [ ] **Step 1: Write failing tests** using the siege fixtures that `tests/siege/shields.rs` uses:
  - `a_patch_node_heals_a_staff_body_in_its_circle_on_a_pulse_round`: assert `+2` HP after a round wrap to an even round, and no change after a wrap to an odd round.
  - `a_patch_node_heals_a_structure_on_the_board`: `+2` Durability and a `Heal(2)` tactical fx.
  - `a_patch_node_never_heals_a_besieger`.
  - `two_patch_nodes_heal_twice`.
  - `a_destroyed_patch_node_stops_pulsing`.
  - `a_full_body_under_a_patch_node_gets_no_heal_cue`.
- [ ] **Step 2: Run.** `cargo test -p feral-processes-engine siege::patch`. Expected: FAIL.
- [ ] **Step 3: Implement and hook it** into `tactical_round_upkeep` immediately after `fire_turrets`, so a heal lands before the reap.
- [ ] **Step 4: Run.** `cargo test -p feral-processes-engine siege` and `balance_sim`. Expected: PASS (balance_sim does not model sieges, so it should not move; if it does, stop and report).
- [ ] **Step 5: Commit.** "Patch Nodes heal their circle every other siege round".

### Task 4: Light-green circle on hover and in previews

**Files:**
- Modify: `crates/engine/src/game/base/defences.rs` (a `view_patch_nodes`), `crates/engine/src/views/` (wherever `ShieldView` lives; add `PatchNodeView`)
- Rename: `crates/app-core/src/app/shields.rs` → `app/reach_circles.rs`; `ShieldCircle` → `ReachCircle`, `ShieldCircleStyle` → `ReachCircleStyle`, `shield_circles` → `reach_circles`. Update the re-export at `lib.rs:52` and `app/mod.rs:54`. The tests file `app-core/src/tests/shields.rs` → `tests/reach_circles.rs`.
- Rename: `crates/gui/src/render/shield_circle.rs` → `render/reach_circle.rs`. Update the callers at `render/base/playing.rs:134/216/285`.
- Add rule-glob coverage in any `.claude/rules/*.md` that names `shields` paths (see memory: file→directory renames break rule globs).

**Interfaces:**
- Produces:
  - `pub struct PatchNodeView { pub entity, pub anchor: (i32,i32), pub radius: u32, pub next_radius: Option<u32> }`
  - `pub fn view_patch_nodes(&self) -> Vec<PatchNodeView>`, sorted `(anchor, entity)`
  - `pub enum ReachKind { Shield, Patch }` in app-core, and `ReachCircle { anchor, radius, style, kind: ReachKind }`
  - In gui: `const PATCH_GREEN: (f32,f32,f32) = (0.6, 1.0, 0.6)`, with the colour picked by `kind` (the Shield flash path stays blue)

- [ ] **Step 1: Write failing app-core tests**, mirroring the existing shield ones:
  - `hovering_a_patch_node_shows_a_patch_circle` (kind `Patch`, radius 4)
  - `deploy_preview_of_a_patch_node_shows_radius_4`
  - `upgrade_preview_of_a_tier_1_patch_node_shows_radius_6`
  - `a_shield_circle_is_still_kind_shield`
- [ ] **Step 2: Run.** `cargo test -p feral-processes-app-core reach_circles`. Expected: FAIL.
- [ ] **Step 3: Implement.** Hover, deploy and upgrade each gather Shields and Patch Nodes. Deploy uses `patch_radius_for(kind, 1)`, with the same "player's 4 neighbours" rule as Shields. Add a gui test: a `Patch` circle draws with green, not blue (model it on `a_hovered_shield_draws_a_circle_on_the_surface_map`).
- [ ] **Step 4: Run.** `cargo test -p feral-processes-app-core` and `-p feral-processes-gui`. Expected: PASS. Take a screenshot with a Patch Node hovered if a template allows it, and Read it.
- [ ] **Step 5: Commit.** "Patch Node circles draw light green on hover and in previews".

### Task 5: Stop offering Failover

**Files:**
- Modify: `crates/engine/src/perks.rs` (`grouped` ~558, tests ~645), `assets/perks/groups.ron` (remove `Failover` from Workshop), `assets/help/55-perks.md:27-28`

**Interfaces:**
- Produces: `impl Perk { pub fn is_offered(self) -> bool }`. It is false only for `Failover`, with a doc comment pointing to ADR 0036. `grouped()`'s trailing bucket skips `!is_offered()`.

- [ ] **Step 1: Write a failing test,** `failover_is_not_offered`: `!PerkDb::load(..).catalogue().any(|d| d.perk == Perk::Failover)`. Also adjust `every_shipped_perk_sits_under_a_heading` to compare against `Perk::all().iter().filter(|p| p.is_offered()).count()`.
- [ ] **Step 2: Run.** `cargo test -p feral-processes-engine perks`. Expected: FAIL.
- [ ] **Step 3: Implement.** Check every caller of `catalogue()`/`grouped()` (`game/catalog.rs:1086`, `:1120`, app-core `progression.rs:11`). If any of them names a perk the player already holds, it must still resolve Failover's def; use `defs` for that, not the catalogue. Remove the Failover lines from the help file.
- [ ] **Step 4: Run.** `cargo test -p feral-processes-engine perks` and `raids` (`failover_repairs_…` still unlocks it directly). Expected: PASS.
- [ ] **Step 5: Commit.** "Failover is no longer offered (ADR 0036)".

### Task 6: Seams, rules and index; final gate

**Files:**
- Modify: `.claude/rules/seams-base.md` and `seams-sieges.md` (one line each), `.claude/skills/seams/references/base-structures.md` and `references/sieges.md`, `docs/superpowers/INDEX.md` (one row), `docs/adr/0036-…` (Evidence: replace `ShieldRadiusDef` with `reach::RadiusDef`)
- Memory graph: `seam:` entities via the `seams` skill.

Rules to add:
- seams-base: "**A structure's Patch Node mend is `patch_mend(patch_reaches, structure_cells)`**: a node heals only what its circle covers. Failover is the only base-wide repair."
- seams-sieges: "**A Patch Node pulses in `tactical_round_upkeep` after `fire_turrets`, on rounds divisible by `PATCH_SIEGE_PULSE_ROUNDS`**: bodies heal through `restore_hp`, and board structures cue `TacticalFxKind::Heal` by hand."

- [ ] **Step 1:** Write the graph entries, then the skill references, then the rules lines, in the order the `seams` skill documents.
- [ ] **Step 2:** Run `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` (with `FERAL_DEV_NO_SIEGES` unset). Expected: all green.
- [ ] **Step 3: Commit.** "Patch radius: rules, seams and docs".

The final whole-branch review (opus) follows. The release (version bump, CHANGELOG, tag) happens at landing via the deploy skill, not on the branch.
