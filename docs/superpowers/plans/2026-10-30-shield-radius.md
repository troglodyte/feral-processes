# Shields Cover a Radius Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Shield protects a circle that grows with its tier, the circle shows on hover and in the deploy/upgrade prompts, a sweep and a fought siege are softened only under it, and a softened hit flashes the circle.

**Architecture:** One pure coverage module in the engine (`shields/`) answers "does this Shield cover this tile", and every consumer calls it: the sweep, the board's damage paths, the outline flag, and the views. Engine views carry each Shield's anchor, radius and armed state. app-core turns hover, deploy and upgrade into a list of circles, and gui draws them through new `Painter` circle primitives. Flashes travel as new `EffectKind`/`TacticalFxKind` variants.

**Tech Stack:** Rust, bevy_ecs (engine), bevy + bevy_egui (gui).

**Spec:** `docs/superpowers/specs/2026-10-30-shield-radius-design.md`; ADR `docs/adr/0035-shields-cover-a-radius.md` (Agreed).

## Global Constraints

- Radius at tier `t` = `base + per_tier * (t - 1)`; shield.ron `base: 4, per_tier: 2`, so 4/6/8/10/12 for Mk1–Mk5.
- A tile is covered when the anchor tile's centre is closer than `r` to the nearest point of the tile's unit square. A structure is covered when any of its footprint cells is covered.
- The per-hit cut is the Shield's `raid_defense` (2) per covering armed Shield.
- Body hits: the cut is added to deflection, sharing `DEFLECTION_MAX` and the floor of 1. Structure hits on the board: never below 1. Sweeps: may reach 0.
- Turrets, town raids, the garrison and the off-screen siege stay base-wide and unchanged.
- Flash: fill alpha rises to 0.25 over ~0.12 s, falls to 0 over ~0.35 s, eased; a new hit restarts that Shield's flash.
- Hover circle: solid blue edge (~2 px) + fill at ~12% alpha. Preview circles (deploy/upgrade): faint edge only, no fill.
- `crates/gui/src/paint.rs` is the only file that names egui.
- The save is field-named RON: a new field behind `#[serde(default)]` needs no `SAVE_FORMAT_VERSION` bump, but does need a save→load test.
- RNG draw order in `run_raid` is unchanged; run `cargo test -p feral-processes-engine balance_sim` after Task 3.
- Gate per task: `cargo fmt`, `cargo clippy --workspace --all-targets` (no warnings), and the task's tests. Final gate: `cargo test --workspace` with `FERAL_DEV_NO_SIEGES` unset.
- Never `git checkout`/`stash`/`reset --hard`, never push, stage explicit paths only.

## Review Focus

1. **A Shield placed after other structures in a 2×2 footprint row.** A footprint-2 structure whose far corner alone is inside the circle must count as covered. Task 1's test pins this.
2. **A save written mid-siege, then reloaded.** A Shield that softened a hit before the save must still spend its one coil at the siege's end, exactly once. Task 6's test pins this.
3. **A Shield destroyed mid-siege.** It must stop protecting at once, and if it softened a hit earlier, it spends nothing at the end because there is no stock left to spend from. It must not panic. Task 6's test pins this.
4. **An unarmed Shield.** No circle cut, no flash and no outline pulse, but the hover circle still shows so the player can see what it would cover. Tasks 3, 4 and 5 assert it.
5. **Hovering a Shield on the siege board.** The circle must sit on the same field tiles as on the base map, shifted by `siege_origin`. Task 4's board-coordinate test pins this.

---

### Task 1: Coverage core, data and views

**Files:**
- Create: `crates/engine/src/shields/mod.rs` (pure coverage), `crates/engine/src/shields/tests.rs`
- Modify: `crates/engine/src/structures.rs` (`ShieldRadiusDef`, `StructureDef::shield_radius`, the `RepairDef`/`raid_defense` docs that say defences are base-wide), `crates/engine/src/lib.rs` (`mod shields`)
- Modify: `crates/engine/src/game/base/defences.rs` (`ArmedDefence::shield`, `Game::shield_reaches`, `Game::view_shields`)
- Modify: `crates/engine/src/views/` (new `views/shields.rs` re-exported from `views/mod.rs`)
- Modify: `assets/structures/shield.ron`, `assets/structures/README.md`
- Modify: `crates/engine/src/tests/assets/zones_weapons.rs` (`every_upgrade_path_asks_for_a_zone_material`: 6 → 7, add the Shield to the message)

**Interfaces:**
- Produces:
  - `pub struct ShieldRadiusDef { pub base: u32, pub per_tier: u32 }`, which derives `Clone, Debug, Serialize, Deserialize`.
  - `StructureDef::shield_radius: Option<ShieldRadiusDef>`, `#[serde(default)]`.
  - `pub fn shields::radius_at(def: &ShieldRadiusDef, tier: u32) -> u32`
  - `pub fn shields::covers(anchor: (i32, i32), radius: u32, tile: (i32, i32)) -> bool`
  - `pub fn shields::covers_any(anchor: (i32, i32), radius: u32, cells: &[(i32, i32)]) -> bool`
  - `pub fn shields::covered_tiles(anchor: (i32, i32), radius: u32) -> Vec<(i32, i32)>`, sorted by `(x, y)`.
  - `pub(crate) struct ShieldReach { pub anchor: (i32, i32), pub radius: u32 }`
  - `ArmedDefence` gains `pub shield: Option<ShieldReach>`, set for a def with `shield_radius`.
  - `pub(crate) fn Game::defence_covers(&self, d: &ArmedDefence, cells: &[(i32, i32)]) -> bool`. A `None` shield (base-wide) is always `true`.
  - `pub(crate) fn Game::structure_cells(&self, e: Entity) -> Vec<(i32, i32)>`. It is the footprint via `tactical::footprint_cells_at` and is empty with no `Position`.
  - `pub struct ShieldView { pub entity: Entity, pub anchor: (i32, i32), pub radius: u32, pub next_radius: Option<u32>, pub armed: bool, pub raid_defense: u32 }`
  - `pub fn Game::view_shields(&self) -> Vec<ShieldView>`, sorted by anchor. `next_radius` is `Some` while `tier < min(max_tier, upgrade_ceiling)`.
  - `pub fn Game::shield_radius_for(&self, kind: &str, tier: u32) -> Option<u32>`, used by the deploy preview.

- [ ] **Step 1: Write the failing pure tests** in `shields/tests.rs`:
  - `radius_grows_two_a_tier`:
    ```rust
    let d = ShieldRadiusDef { base: 4, per_tier: 2 };
    assert_eq!((1..=5).map(|t| radius_at(&d, t)).collect::<Vec<_>>(), vec![4, 6, 8, 10, 12]);
    ```
  - `a_touched_tile_is_covered`, with `r = 4` and the anchor at `(0, 0)`:
    - `covers(.., (0, 0))`: covered.
    - `(4, 0)`: covered. The nearest point is 3.5 away.
    - `(5, 0)`: not covered (4.5).
    - `(3, 3)`: covered (√(2.5²+2.5²) ≈ 3.54).
    - `(4, 3)`: not covered (√(3.5²+2.5²) ≈ 4.30).
  - `covered_tiles_matches_covers`: for `r` in 1..=12, every tile in a 30×30 box is in `covered_tiles` iff `covers` says so. The list is sorted.
  - `a_footprint_corner_is_enough`: `covers_any((0,0), 4, &footprint_cells_at((4, 3), 2))` is false, while `covers_any((0,0), 4, &footprint_cells_at((3, 3), 2))` is true.

- [ ] **Step 2:** Run `cargo test -p feral-processes-engine shields`. Expected: it fails to compile because `shields` is not defined.

- [ ] **Step 3: Implement the module and the data field.** Distance is computed per axis: `dx = max(|tx - ax| - 0.5, 0)`, same for `dy`, and the tile is covered iff `dx² + dy² < r²`. Use exact rational math (doubled integers) so no float comparison is involved.

- [ ] **Step 4: Update `shield.ron`:**
  - `shield_radius: Some((base: 4, per_tier: 2)),`
  - `upgrade: Some((max_tier: 5, cost: [("core_fragment", 12), ("cache_grain", 1)])),`
  - Reword the description to: "Soaks 2 damage off every GC Entropy Sweep against a structure inside its circle. Upgrading widens the circle. Stacks with a second one."
  - Document `shield_radius` in `assets/structures/README.md` next to `raid_defense`, saying that a defence without it is base-wide.

- [ ] **Step 5: Write the failing `Game` tests** in `crates/engine/src/tests/defences.rs`, using the existing `raidable`/spawn helpers from `tests/support/`:
  - `view_shields_reports_radius_by_tier`: a deployed Shield reads `radius == 4`. After setting `StructureTier(3)` it reads `8`. `next_radius` is `None` at tier 5.
  - `an_empty_shield_is_listed_unarmed`: with no coil in its input, `armed == false`.
  - `a_shield_upgrades_without_a_program`: `upgrade_structure(shield, None)` returns `Ok` and files an upgrade site.

- [ ] **Step 6: Implement `ArmedDefence::shield`, `structure_cells`, `defence_covers`, `view_shields` and `shield_radius_for`.** Update the `zones_weapons.rs` census to 7.

- [ ] **Step 7:** Run `cargo test -p feral-processes-engine shields defences zones_weapons` and expect PASS. Then run fmt and clippy.

- [ ] **Step 8: Commit.** Message: "Shields: coverage core, radius per tier, upgrade path".

### Task 2: Flash cues and the circle primitive

**Files:**
- Modify: `crates/engine/src/resources.rs` (`EffectKind::Shielded`, `TacticalFxKind::Shielded`)
- Modify: `crates/gui/src/paint.rs` (`Painter::circle`, `Painter::circle_stroke`, test reader `painted_circles`)
- Create: `crates/gui/src/render/shield_circle.rs` (style and draw)
- Modify: `crates/gui/src/render/mod.rs` (`mod shield_circle`)
- Modify: `crates/gui/src/fx.rs` (shield flash lists, alpha curve, `spark_burst`/`effect_duration`/`effect_color` arms)

**Interfaces:**
- Produces:
  - `EffectKind::Shielded { radius: u32 }`. Its `pos` is the Shield's field anchor.
  - `TacticalFxKind::Shielded { radius: u32 }`. Its `pos` is the Shield's board anchor.
  - `pub(crate) fn Game::cue_shield_flash(&mut self, anchor: (i32, i32), radius: u32)`. It pushes `EffectKind::Shielded` to the field queue, and also a board-space `TacticalFxKind::Shielded` (anchor − `siege_origin`) when a siege battle is active.
  - `Painter::circle(&self, cx: f32, cy: f32, r: f32, color: Color)`
  - `Painter::circle_stroke(&self, cx: f32, cy: f32, r: f32, thickness: f32, color: Color)`
  - `pub(crate) enum CircleStyle { Hover, Preview, Flash(f32) }`. `Flash` carries the fill alpha.
  - `pub(crate) fn draw_shield_circle(p: &Painter, centre: (f32, f32), radius_px: f32, style: CircleStyle)`
  - `pub(crate) fn shield_flash_alpha(elapsed: f64) -> f32`
  - `Fx::shield_flashes(&self, in_battle: bool) -> Vec<((i32, i32), u32, f32)>`, returning anchor, radius and alpha.
  - The blue is `Color::new(0.4, 0.8, 1.0, a)`, the hue `Fx::shield_outline` already uses.

- [ ] **Step 1: Write the failing gui tests:**
  - `shield_flash_alpha_peaks_soft_and_ends`:
    - `shield_flash_alpha(0.0) == 0.0`
    - at `0.12` it is within 0.01 of `0.25`
    - at `0.30` it is between 0 and 0.25
    - at `0.47` and later it is `0.0`
    - it never exceeds `0.25` on a 0.01-step sweep
  - `a_second_flash_restarts_rather_than_stacks`: two `Shielded` cues at one anchor 0.1 s apart leave one entry in `shield_flashes`, timed from the second.
  - `hover_style_draws_edge_and_fill`: `with_painter(|p| draw_shield_circle(p, (50.0, 50.0), 64.0, CircleStyle::Hover))` paints one filled circle with alpha ≈ 0.12 and one stroke of width 2.0, both at radius 64. `Preview` paints only a stroke.

- [ ] **Step 2:** Run `cargo test -p feral-processes-gui shield` (use the gui crate's real package name from its `Cargo.toml`). Expected: it fails to compile.

- [ ] **Step 3: Implement.**
  - `Shielded` cues go to two new lists keyed by anchor, field and board. Each list has its own retention of 0.47 s; they are not `TileFlash`.
  - `spark_burst` returns `(0, 0.0)` for `Shielded`.
  - The curve is smoothstep up over 0.12 s and smoothstep down over 0.35 s, scaled to 0.25.

- [ ] **Step 4:** Run the tests and expect PASS. Then fmt and clippy across the workspace.

- [ ] **Step 5: Commit.** Message: "Shield flash cue and circle drawing".

### Task 3: Sweeps count only covering Shields

**Files:**
- Modify: `crates/engine/src/game/base/upkeep.rs` (`run_raid`)
- Modify: `crates/engine/src/game/base/defences.rs` (`Game::sweep_defense_for`)
- Test: `crates/engine/src/tests/raids.rs`, `crates/engine/src/tests/defences.rs`
- Fix: existing tests in `raids.rs`, `defences.rs`, `supply.rs`, `settlement_relations.rs`, `nemesis_siege.rs` and `siege/board_and_opening.rs` that put a Shield far from its target. Move the Shield within range. Never weaken an assertion.

**Interfaces:**
- Consumes: `Game::defence_covers`, `Game::structure_cells` and `Game::cue_shield_flash` from Tasks 1–2.
- Produces: `pub(crate) fn Game::sweep_defense_for(&self, target: Entity) -> u32`. It sums the `raid_defense` of armed defences that cover the target's cells, plus `garrison_defense()`.

- [ ] **Step 1: Write the failing tests:**
  - `a_sweep_outside_every_circle_takes_full_damage`: a Shield 10 tiles from the only swept structure. The structure loses `RAID_DAMAGE`.
  - `a_sweep_inside_the_circle_is_cut_by_two`: the Shield 3 tiles away. The structure loses `RAID_DAMAGE - 2`.
  - `two_covering_shields_hold_the_sweep`: the structure takes 0 damage and the `Deflected` effect is queued.
  - `only_covering_shields_spend_a_coil`: one Shield near the target and one far away. After the sweep, only the near one's coil count dropped by 1.
  - `a_softened_sweep_flashes_its_shield`: `take_effects()` contains `EffectKind::Shielded { radius: 4 }` at the covering Shield's anchor and nothing at the far one.
  - `an_unarmed_shield_covers_nothing_in_a_sweep`: a covering Shield with no coil gives a cut of 0 and no flash.
  - `turret_raid_defense_stays_base_wide`: a Turret far from the target still cuts the sweep by 1.

- [ ] **Step 2:** Run `cargo test -p feral-processes-engine raids defences`. Expected: the new tests FAIL.

- [ ] **Step 3: Implement.**
  - In `run_raid`, after the target is picked (keep the RNG draw where it is), replace `total_raid_defense()` with `sweep_defense_for(target)`.
  - Change the spend to `|d| d.raid_defense > 0 && self.defence_covers(d, &cells)`, computing the cells first so the closure borrows nothing mutable.
  - Cue a flash per covering armed Shield when its cut reduced the damage.
  - `total_raid_defense` keeps its other callers (town raid, off-screen siege, `raid_defense_active`).

- [ ] **Step 4:** Run the new tests and expect PASS. Then fix the existing tests that relied on base-wide Shields, and run `cargo test -p feral-processes-engine` until it is green.

- [ ] **Step 5:** Run `cargo test -p feral-processes-engine balance_sim`. Expected: PASS with no curve moved. If a curve moves, stop and report it rather than retuning.

- [ ] **Step 6: Commit.** Message: "Sweeps count only Shields whose circle covers the target".

### Task 4: Circles on hover, deploy and upgrade; outline only under a circle

**Files:**
- Modify: `crates/engine/src/views/map.rs` (`EntityView::shielded: bool`), `crates/engine/src/game/inspection.rs` (`view_entities_at` sets it)
- Modify: `crates/engine/src/tactical/view.rs` (`TacticalView::siege_origin: (i32, i32)`)
- Create: `crates/app-core/src/app/shields.rs`; Modify `crates/app-core/src/lib.rs` (module line only)
- Modify: `crates/app-core/src/app/building.rs` (`handle_upgrade_direction_key`)
- Modify: `crates/gui/src/render/base/surface_map.rs`, `crates/gui/src/render/tactical/map.rs`, `crates/gui/src/render/base/playing.rs`

**Interfaces:**
- Consumes: `Game::view_shields`, `Game::shield_radius_for` and `shields::covers_any` (Task 1); `draw_shield_circle`, `CircleStyle` and `Fx::shield_flashes` (Task 2).
- Produces:
  - `EntityView::shielded`: true when the entity is a structure covered by an armed Shield.
  - `pub struct ShieldCircle { pub anchor: (i32, i32), pub radius: u32, pub style: ShieldCircleStyle }`
  - `pub enum ShieldCircleStyle { Hover, Preview }`
  - `pub fn App::shield_circles(&mut self) -> Vec<ShieldCircle>`. Coordinates are those of the pane being drawn: board cells in `Mode::TacticalBattle`, field tiles otherwise. A circle is returned:
    - **Hover:** `map_hover` lies on a Shield's footprint. On the board, the hover is compared after adding `siege_origin`.
    - **Deploy:** in `Mode::BuildDirection` with `pending_structure == Some("shield")` (any def with `shield_radius`, via `shield_radius_for(kind, 1)`), four `Preview` circles, one at each of the four neighbours of the party's tile.
    - **Upgrade:** in `Mode::UpgradeDirection`, a `Preview` circle at `next_radius` for each adjacent Shield that has one.
  - When `game.adjacent_structure` finds a structure that doesn't need a program, `handle_upgrade_direction_key` calls `game.upgrade_structure(entity, None)` directly instead of opening `Mode::BuildProgram`. It mirrors the deploy path's `structure_needs_program` check.

- [ ] **Step 1: Write the failing app-core tests** (`crates/app-core/src/app/` test module, using an existing template fixture):
  - `hovering_a_shield_returns_its_circle`: `set_map_hover(Some(anchor))` gives one `Hover` circle with radius 4. Hovering a tile beside it gives none.
  - `deploying_a_shield_previews_four_circles`: in `BuildDirection` with `pending_structure = Some("shield")`, there are four `Preview` circles at the party's ±x/±y neighbours, each radius 4.
  - `upgrading_previews_the_next_radius`: in `UpgradeDirection` beside a Mk1 Shield, there is one `Preview` circle of radius 6.
  - `a_shield_upgrade_skips_the_program_picker`: pressing the direction toward a Shield in `UpgradeDirection` leaves the mode not `BuildProgram` and files an upgrade site.
  - `board_hover_circle_is_in_board_cells`: during a siege, hovering the Shield's board cell gives a circle whose anchor equals the field anchor − `siege_origin`.

- [ ] **Step 2: Write the failing engine test:** `outline_flag_marks_only_covered_structures`. One Shield, one structure inside its circle and one outside: `EntityView::shielded` is true, false. With the Shield unarmed, both are false.

- [ ] **Step 3:** Run `cargo test -p feral-processes-app-core shield` and `cargo test -p feral-processes-engine outline_flag`. Expected: FAIL.

- [ ] **Step 4: Implement the engine and app-core parts.**

- [ ] **Step 5: Wire up the gui.**
  - `playing.rs` collects `app.shield_circles()` before the `game` borrow (the `tactical_hover_path` pattern) and passes them to both map draws.
  - Each map converts an anchor with `tile_origin_px(..) + tile_px / 2` and draws at `radius * tile_px`, clipped to the pane (`Painter::clipped`).
  - Each map also draws `fx.shield_flashes(in_battle)` as `CircleStyle::Flash(alpha)`.
  - `surface_map.rs` replaces the base-wide `shielded = true` with the view's `EntityView::shielded`.

- [ ] **Step 6: Write a gui test** in the `render/tactical/tests/` style: `a_hovered_shield_draws_a_circle_on_the_surface_map`. A one-Shield view with a hover circle paints one stroke and one fill centred on the Shield's tile centre.

- [ ] **Step 7:** Run the tests and expect PASS. Then fmt and clippy.

- [ ] **Step 8: Check visually.** Run `cargo run -- --template stack --keys "<keys that reach a base with a Shield>" --screenshot out.png`, or use a template whose base has a Shield (check `dev-saves/README.md`), and Read the PNG. A hover is pointer-only, so screenshot the deploy preview (`BuildDirection` with a Shield pending) instead. If no template reaches that, say so in the handback rather than claiming it was seen.

- [ ] **Step 9: Commit.** Message: "Shield circles on hover, deploy and upgrade; outline only under a circle".

### Task 5: Siege body hits are softened under a circle

**Files:**
- Modify: `crates/engine/src/game/combat_damage.rs` (`mitigate_with_resist`, `apply_damage_typed`)
- Modify: `crates/engine/src/game/base/defences.rs` (`Game::siege_shields_over`)
- Modify: `crates/engine/src/tactical/mod.rs` (`TacticalBattle::shields_softened`)
- Test: `crates/engine/src/tests/siege/` (new file `shields.rs`, registered in that directory's `mod.rs`)

**Interfaces:**
- Consumes: Task 1's coverage functions and Task 2's `cue_shield_flash`.
- Produces:
  - `pub(crate) fn Game::siege_shields_over(&self, target: Entity) -> Vec<ShieldReach>`. It is empty unless a `TacticalBattle` with `siege_pack > 0` exists and `target` is not `Hostile`. Otherwise it lists the armed Shields covering the target's field cells: `battle.cells_of(target)` + `siege_origin`, falling back to `structure_cells` for an unseated entity.
  - `TacticalBattle::shields_softened: BTreeSet<(i32, i32)>`, the field anchors of Shields that softened a hit this siege.
  - In `mitigate_with_resist`'s non-`ignores_deflection` branch, deflection becomes `gear deflection + Σ raid_defense of siege_shields_over(target)`.
  - In `apply_damage_typed`, when the Shield list is non-empty and the incoming `dmg > 0`, record each anchor in `shields_softened` and `cue_shield_flash` each one.

"Enemy hit" is implemented as "damage to a non-hostile body during a siege". During a siege battle every source of that damage is the besiegers' (swings, routines, statuses they applied). Tantrums never run inside a battle. Note this in the code comment where the gate lives.

- [ ] **Step 1: Write the failing tests** in `tests/siege/shields.rs`. Open a siege with the existing siege fixtures (`tests/support/`, `open_siege`), and place an armed Shield so a staff member stands inside its circle and another stands outside.
  - `a_staff_hit_under_a_shield_loses_two`: `apply_damage_typed(inside, 10, None)` takes 2 more off than the same call on `outside`. Stats must be equal; set mitigation to 0 on both.
  - `a_shield_cut_never_drops_a_hit_below_one`: a 2-damage hit under two Shields deals 1.
  - `shield_cut_shares_the_deflection_cap`: gear deflection 6 plus a Shield deals the same damage as deflection 6 alone.
  - `a_besieger_under_the_circle_is_not_shielded`: a `Hostile` body inside the circle takes the full hit.
  - `an_unarmed_shield_softens_nothing_in_a_siege`
  - `a_softened_hit_flashes_and_is_recorded`: `take_tactical_fx()` contains `TacticalFxKind::Shielded { radius: 4 }` at the board anchor, and `shields_softened` contains the field anchor.
  - `no_shield_cut_outside_a_siege`: in an ordinary tactical battle, the same placement gives no cut.
  - `a_shield_destroyed_mid_siege_stops_protecting`: after `damage_structure` destroys the Shield, the next hit on `inside` takes the full damage and nothing panics.

- [ ] **Step 2:** Run `cargo test -p feral-processes-engine siege::shields`. Expected: FAIL.

- [ ] **Step 3: Implement.** `resisted` at `resolve_and_apply_attack_among` stays consistent because both mitigate functions go through `mitigate_with_resist`. Add an assertion in a test that `resisted` is unchanged for a typed hit under a Shield.

- [ ] **Step 4:** Run `cargo test -p feral-processes-engine` and expect PASS. Then run `cargo test -p feral-processes-engine balance_sim` and expect it unchanged, since balance_sim fights no sieges.

- [ ] **Step 5: Commit.** Message: "Shields soften enemy hits on bodies under their circle in a siege".

### Task 6: Siege structure hits, the coil at the siege's end, and persistence

**Files:**
- Modify: `crates/engine/src/tactical/turn.rs` (the structure branch of `tactical_attack`)
- Modify: `crates/engine/src/game/siege/raiders.rs` (`besieger_wreck`)
- Modify: `crates/engine/src/game/base/defences.rs` (`Game::shielded_structure_damage`)
- Modify: `crates/engine/src/game/combat_teardown.rs` (`finish_fight`)
- Modify: `crates/engine/src/save/mod.rs` (`SiegeSave::shields_softened`), `crates/engine/src/game/siege/persist.rs` (`assemble`/`restore`)
- Test: `crates/engine/src/tests/siege/shields.rs`

**Interfaces:**
- Consumes: `siege_shields_over`, `TacticalBattle::shields_softened` and `cue_shield_flash`.
- Produces:
  - `pub(crate) fn Game::shielded_structure_damage(&mut self, structure: Entity, dmg: u32) -> u32`. It returns `dmg - Σ cut`, floored at 1 when `dmg > 0`, and records and cues each covering Shield as in Task 5.
  - Both board paths call it before `damage_structure`. `tactical_attack` calls it only when `actor` is `Hostile`; `besieger_wreck` always calls it.
  - `SiegeSave::shields_softened: Vec<(i32, i32)>`, `#[serde(default)]`.
  - In `finish_fight`, while the `TacticalBattle` is still present and `siege_pack > 0` (beside the besieger sweep), each Shield whose anchor is in `shields_softened` and still stands spends one supply through `stock::spend_supply(self, e, ConsumeSource::Defence)`.

- [ ] **Step 1: Write the failing tests:**
  - `a_structure_hit_under_a_shield_loses_two`: a besieger wrecks a structure inside the circle and one outside with equal swing damage. The durability lost differs by 2.
  - `a_structure_hit_is_never_cut_to_zero`
  - `the_player_hitting_a_structure_is_not_shielded`: a non-hostile actor's `tactical_attack` on a covered structure deals full damage. This is the case of the player breaking their own wall.
  - `a_shield_that_softened_spends_one_coil_at_the_end`: after several softened hits, the coil count drops by exactly 1 when the siege ends. Check it for a win (hostiles gone) and for a morale break.
  - `a_shield_that_softened_nothing_spends_nothing`
  - `softened_shields_survive_a_mid_siege_reload`: soften a hit, then `save` → `load` (a real save→load round trip, not RON alone), end the siege, and the coil drops by exactly 1.
  - `a_softening_shield_destroyed_before_the_end_spends_nothing_and_does_not_panic`

- [ ] **Step 2:** Run `cargo test -p feral-processes-engine siege::shields`. Expected: the new tests FAIL.

- [ ] **Step 3: Implement.** `restore` repopulates `TacticalBattle::shields_softened` from the save.

- [ ] **Step 4:** Run `cargo test -p feral-processes-engine` and expect PASS. Then fmt and clippy.

- [ ] **Step 5: Commit.** Message: "Shields soften siege hits on structures and spend a coil per siege".

### Task 7: Rules, seams and changelog

**Files:**
- Modify:
  - `.claude/rules/seams-sieges.md` (the off-screen open balance item stays; add one line saying a fought siege's damage to the defending side is cut by `siege_shields_over`)
  - `.claude/rules/seams-base.md` (the sweep reads `sweep_defense_for`, and only covering Shields spend)
  - the `seams` skill and the `seam:` graph entries for both lines, in the order the skill documents
  - `docs/structures.md` (the Shield row: "-2 sweep damage inside its circle, radius 4 + 2 a tier")
  - `assets/help/66-sieges.md`, if its Shield sentence now misleads
  - `CHANGELOG.md`, through the release step, not now
- Do not touch `docs/manual.md`, `README.md` or `TODO.md`.

- [ ] **Step 1: Update the docs.** Mark the spec's and the ADR's status in `docs/superpowers/INDEX.md` the way the other shipped entries are marked.
- [ ] **Step 2: Run the full gate:** `unset FERAL_DEV_NO_SIEGES; cargo test --workspace` and `cargo clippy --workspace --all-targets`. Expected: all green.
- [ ] **Step 3: Commit.** Message: "Shield radius: rules, seams and docs".

The CHANGELOG entry and version bump happen at deploy, as a patch release (no save-format bump). The entry must say that existing Shields now cover only their circle.
