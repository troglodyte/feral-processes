# Tactical Props Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Authored derelict high-tech set pieces on tactical battle maps — walls, chokepoints, decoration — with destructible cover, volatile explosions and new art.

**Architecture:** A prop overlay on `tactical::map::Board` (like `screens`), filled at generation from `assets/battle-props/` prefabs. Every terrain read goes through `Board` methods, so reach/sight/cover pick props up without touching their callers. Destruction is cued through the existing `TacticalFxQueue`.

**Tech Stack:** Rust, standalone `bevy_ecs` (engine), bevy + bevy_egui (gui), RON assets, Python+Pillow art script.

**Spec:** `docs/superpowers/specs/2026-10-10-tactical-props-design.md` — read it first; this plan does not restate its rationale.

## Global Constraints

- Renderer never touches `World`; everything crosses through `Game` / `TacticalView`. No `world()` accessor.
- Only `crates/gui/src/paint.rs` names the graphics backend; `render/` draws through `Painter`. Sprite art: 16x16 near-white PNG, tinted by multiplication; a sprite substitutes for a glyph, never drawn beside it.
- Hue rule (`cell_color`, ground-textures spec): props spend brightness and a muted cast, never a wash hue.
- Every new `.ron` field `#[serde(default)]`; malformed file → logged warning + skip (`*Db::load_dir` pattern); update `assets/battle-props/README.md` in the same change.
- No save-format change: `Board`/props are never serialized; siege boards get no props; `BattleCell` gains no variant.
- Generation stays pure: no `GameRng` draw, hash-fold only (same fold as the noise in `map.rs`).
- Named constants in `tuning.rs`. Actions UPPERCASE; lowercase letters are row selectors. No player-facing tick vocabulary.
- Read `.claude/rules/seams-tactical.md` before Phase 2; obey: swings need LOS via `reach::swing_reaches`; routine effect via `Game::run_tactical_routine`; AI: one draw per turn, candidates sorted before scoring, read `Game::decision_temperature`.
- `cargo fmt` + `cargo clippy --workspace --all-targets` clean after each task; commit per green step. Unset `FERAL_DEV_NO_SIEGES` before tests.
- Subagents: never push, never `git checkout`/`stash` others' work, never `git add -A` — stage explicit paths.

## Review Focus

1. **A prefab that would wall one side in** — a 14-side board with a long indestructible wall run. Expect: `carve_to_connect` still yields one walkable region; deploy cells never land on props. (Task 3 test over seeds × biomes.)
2. **A volatile next to a volatile next to the actor** — chain + self-damage. Expect: chain stops at `TACTICAL_PROP_CHAIN_MAX`; actor killed by own blast ends turn via `hand_on_turn` without panic. (Task 5 test.)
3. **Last hostile killed by a prop blast** — fight must end through `finish_fight` and pay normally. (Task 5 test.)
4. **Swing aimed at an empty cell vs a prop cell vs a decoration cell** — empty and decoration refuse (no action spent); prop lands. (Task 4 + Task 7 tests.)
5. **Modded prefab legend naming a missing piece / unknown biome** — prefab skipped with warning, others load, no panic. (Task 1 test.)

---

## Phase 1 — Engine: data and board (no combat yet)

### Task 1: `PropDb` and the asset schema

**Files:**
- Create: `crates/engine/src/tactical/props.rs` (defs + loader), `assets/battle-props/README.md`, `assets/battle-props/pieces/*.ron`, `assets/battle-props/prefabs/*.ron`
- Modify: `crates/engine/src/tactical/mod.rs` (mod decl), wherever `Game` loads its other `*Db`s (follow `AbilityDb::load_dir`, `abilities.rs:1576`, and find its call site) to hold a `PropDb`; `tuning.rs` constants below.

**Interfaces — Produces:**
- `PieceDef { id: String, blocks_move: bool, blocks_sight: bool, cover: bool, hp: Option<u32>, armour: u32, volatile: Option<Blast>, leaves: Leaves, sprite: String, decoration: bool, move_cost: Option<u32> }`, all `#[serde(default)]` except `id`.
- `Blast { radius: u32, damage: u32 }`; `enum Leaves { Rubble, #[default] Floor }`.
- `PrefabDef { id: String, biomes: Vec<Biome>, weight: u32, rows: Vec<String>, legend: BTreeMap<char, String> }` (empty `biomes` = all).
- `PropDb { pieces: BTreeMap<String, PieceDef>, prefabs: Vec<PrefabDef> }`; `PropDb::load_dir(dir: &Path) -> io::Result<(PropDb, Vec<String>)>` (warnings vec, same shape as other dbs); `PropDb::prefabs_for(biome) -> impl Iterator<Item=&PrefabDef>`; `PropDb::piece(id) -> Option<&PieceDef>`.
- A `"rubble"` piece is required in data (`blocks_move:false`, no cover, `hp:None`, `move_cost: Some(2)` = `TACTICAL_ROUGH_COST`); loader warns if absent.
- Constants: `TACTICAL_PROP_CHAIN_MAX: u32 = 3`.

**Content (this task):** pieces `server_rack`, `hull_plate`, `pylon`, `drone_chassis`, `conduit_wall` (indestructible), `conduit_wall_broken`, `power_cell` (volatile 1/12), `coolant_tank` (volatile 2/8), `rubble`, decorations `cable_run`, `scorch`, `glass_shards`, `dead_screen`. Prefabs (6): `wall_gap` (conduit run with one-cell gap), `hull_wreck_l`, `rack_row`, `crashed_drone` (+power cell), `fallen_pylon`, `coolant_cluster`. Sprite keys `prop_<piece>`.

- [ ] Tests (in `props.rs`): loads the shipped dir with zero warnings; every prefab legend resolves; malformed `.ron` and unknown-piece legend are skipped with a warning while siblings load; a piece file omitting optional fields parses with defaults.
- [ ] Run `cargo test -p feral-processes-engine props` → FAIL, implement, → PASS.
- [ ] Write README (schema of both file kinds, legend `.` semantics, sprite key convention).
- [ ] fmt/clippy, commit.

### Task 2: The prop layer on `Board`

**Files:** Modify `crates/engine/src/tactical/map.rs`, `crates/engine/src/tactical/reach.rs`, any other reader found by the census below.

**Interfaces — Produces:**
- `PropCell { piece: String, hp: Option<u32>, max_hp: Option<u32>, armour: u32, blocks_move: bool, blocks_sight: bool, cover: bool, volatile: Option<Blast>, decoration: bool, sprite: String }` — copied from the def at placement, so reach queries never consult `PropDb`. `PropCell::from_def(&PieceDef)`.
- On `Board`: `props: BTreeMap<(i32,i32), PropCell>` (private); `prop_at(x,y) -> Option<&PropCell>`; `props() -> impl Iterator<Item=(&(i32,i32), &PropCell)>`; `place_prop(cell, PropCell)`; `remove_prop(cell) -> Option<PropCell>`; `damage_prop(cell, amount: u32) -> PropHit` where `enum PropHit { Missing, Indestructible, Damaged, Destroyed(PropCell) }` (Destroyed already removed it).
- `Board::walkable`, `Board::blocks_sight`, and a new `Board::move_cost(x,y) -> Option<u32>` fold props in: `blocks_move` → `None`; otherwise `max(cell cost, prop.move_cost.unwrap_or(1))`. `PropCell` therefore also carries `move_cost: Option<u32>`.
- `Board::is_cover(x,y) -> bool` = `cell(x,y)==Cover || prop.cover`. `reach::cover_between` reads `is_cover` instead of `BattleCell::Cover`.
- `Board::from_rows` (test-only) gains `P` = indestructible sight+move-blocking cover prop, `D` = destructible (hp 10) one, `V` = volatile (radius 1, damage 12, hp 5).

- [ ] **Census first:** `rg -n "\.cell\(|BattleCell::" crates/engine/src/tactical crates/engine/src/game/siege` — list every terrain read; each one that means "walkable / sight / cost / cover" converts to the `Board` method. Record the list in the commit message.
- [ ] Tests: `P` blocks `line_of_sight` and `movement_field`; `D` damaged to 0 → `Destroyed`, then LOS true; `Indestructible` never changes; rubble `move_cost == TACTICAL_ROUGH_COST`, no cover; `cover_between` true behind a `P` the same as behind `#`; decoration blocks nothing.
- [ ] FAIL → implement → PASS; run `cargo test -p feral-processes-engine tactical` (existing suite must stay green — no board has props yet).
- [ ] fmt/clippy, commit.

### Task 3: Placement in `map::generate`

**Files:** Modify `crates/engine/src/tactical/map.rs` (`generate` :416, `carve_to_connect` :362), its caller `tactical/turn.rs:~131` (`open_tactical_battle_at`), `tuning.rs`.

**Interfaces:**
- Consumes: `PropDb`, `PropCell::from_def`, `Board::place_prop/remove_prop`.
- Produces: `generate(spec: BattleSpec, props: &PropDb) -> Board` (every caller updated; siege boards untouched). Constants `TACTICAL_PREFABS_BY_SIDE: [(i32, u32, u32); 3]` (side → min,max), `TACTICAL_DECOR_PER_MILLE: u32`.
- Order: noise → stamp prefabs (hash-seeded pick by weight, position, rotation 0/90/180/270; reject overlap and deploy edge bands — reuse whatever `deploy::plan` treats as edge) → decoration on `Open` cells → `carve_to_connect` (blocking props count as unwalkable; it may remove a *destructible* prop, never an indestructible one when noise would do) → `deploy::plan`.

- [ ] Tests: same spec + same db → identical board including props (determinism); across ≥200 seeds × every `Biome`, walkable region is connected and no deploy cell holds a blocking prop; census: every biome a fight opens on gets ≥1 prefab within N seeds (pattern: `cover_is_reachable_on_every_biome_a_fight_opens_on`); an empty `PropDb` reproduces today's boards exactly (guards old tests).
- [ ] FAIL → implement → PASS; `cargo test -p feral-processes-engine tactical`.
- [ ] fmt/clippy, commit. **Phase gate:** `cargo test --workspace` green; `cargo run -- --template stack` into a fight shows nothing new yet visually except blocked/glyph cells — acceptable.

## Phase 2 — Engine: fighting the terrain

Read `.claude/rules/seams-tactical.md` and the `seams` skill's tactical section first.

### Task 4: Swing at a prop + destruction cue

**Files:** Modify `crates/engine/src/tactical/turn.rs` (beside `tactical_attack` :534), `crates/engine/src/resources.rs` (`TacticalFxKind` :975), `game/combat.rs` only if `swing_damage` visibility needs it.

**Interfaces — Produces:**
- `TacticalFxKind::PropDestroyed { volatile: bool }`.
- `Game::tactical_attack_prop(cell: (i32,i32)) -> bool`: refuses (no action spent) if no battle, no actions left, charging, no prop, decoration prop, indestructible prop, or `!reach::swing_reaches(board, actor_cells, &[cell], self.swing_range(actor))`. Damage `swing_damage(actor).saturating_sub(armour).max(1)`. Spends one action; hands on via `hand_on_turn` like `tactical_attack`. On `Destroyed`: place `leaves` (rubble → `PropCell` of the `rubble` def), cue `PropDestroyed`, the volatile blast is Task 5's — Task 4's tests use non-volatile props only.
- `Game::tactical_prop_at(cell) -> Option<PropView>` for app-core (see Task 7 for `PropView`; define it here in `tactical/view.rs`).
- Log a line on destruction (plain-language, security vocabulary, e.g. "The server rack collapses.").

- [ ] Tests: in-range swing damages and spends one action; out of LOS / out of range / decoration / indestructible / empty refuse with actions unchanged; destroy → rubble placed, LOS restored, cue queued; no XP and no morale change (assert party XP unchanged).
- [ ] FAIL → implement → PASS → fmt/clippy → commit.

### Task 5: Routines hit props; volatile blasts

**Files:** Modify `crates/engine/src/tactical/turn.rs` (`run_tactical_routine` path from `tactical_use_routine` :1094), `crates/engine/src/tactical/reach.rs` (`shape_cells` :381).

**Interfaces — Produces:**
- `shape_cells` returns the sight-blocking cell that truncates a `Line`/`Cone` **when it holds a destructible prop** (append it; bodies' recipients logic unchanged — that cell holds no body).
- In the routine's damage application: for each affected cell with a destructible prop, `damage_prop` with the routine's damage figure (the same pre-mitigation number it rolls for a body, minus `armour`, min 1). Heals/buffs ignore props.
- `Game::detonate(cell, blast: Blast, depth: u32)`: if `depth >= TACTICAL_PROP_CHAIN_MAX` return; damage every body whose cells intersect the radius (use the radius-splash cell set `shape_cells` computes for `AbilityShape::Radius`, sight-clipped from `cell`) via `Game::apply_damage` (cues `Hit` itself); damage props in radius, recursing on `Destroyed` volatile ones with `depth+1`. Friendly fire is full (seam: `reach::recipients` never reads `Hostile`). Wire it into Task 4's destroy path.
- Deaths from blasts go through the normal reap; the fight ends via `finish_fight`.

- [ ] Tests: beam stopped by a `D` damages it; radius routine over two `D` damages both; `V` swung to 0 → adjacent bodies take 12 and a neighbouring `V` chains; a line of 6 `V`s stops after `TACTICAL_PROP_CHAIN_MAX`; actor adjacent to its own detonation dies → turn handed on, no panic; last hostile killed by a blast → battle closes and rewards pay.
- [ ] FAIL → implement → PASS → `cargo test -p feral-processes-engine tactical` → fmt/clippy → commit.

### Task 6: AI candidates — break cover, detonate

**Files:** Modify `crates/engine/src/tactical/ai.rs` (`tactical_ai_turn` :372 and the intent/target choosers named in seams: `tactical_intent`, `chosen_target`, `argmax_scored`).

**Interfaces — Produces:**
- Detonate candidate: for each volatile destructible prop the actor can swing at, score = expected blast damage to hostile-to-actor bodies minus to its own side; competes with the best body swing on the same scale (`battle::expected_damage`). Swing only if one hit destroys it (hp ≤ damage) — otherwise skip (keep it simple, no multi-turn planning).
- Break-cover candidate: only when no hostile-to-actor body is in sight from any reachable cell (`cell_merit`'s field) and the nearest target's sight line from the actor's cell is blocked by exactly one destructible prop; then swing at it.
- Both are candidates in the existing argmax/sample, sorted before scoring; no extra `GameRng` draw; temperature zero stays deterministic. Profiled-hostile forecast must call the same planner (seam).

- [ ] Tests (via `tactical_ai_turn_at` test hook, `from_rows` boards, temperature 0): hostile walled off by a `D` with no path to sight swings at the `D`; hostile with a `V` next to two party bodies detonates it over a weaker direct swing; with a clear shot available it does not hit props; same seed → same choice.
- [ ] FAIL → implement → PASS → fmt/clippy → commit. **Phase gate:** `cargo test --workspace`; `cargo test -p feral-processes-engine balance_sim` unchanged.

## Phase 3 — Frontend and art

### Task 7: View + app-core aim

**Files:** Modify `crates/engine/src/tactical/view.rs` (`TacticalView` :220ff), `crates/app-core/src/app/tactical.rs` (`commit_tactical_aim` ~:483), app-core tests dir.

**Interfaces — Produces:**
- `PropView { cell: (i32,i32), sprite: String, hp: Option<u32>, max_hp: Option<u32>, volatile: bool, decoration: bool }`; `TacticalView.props: Vec<PropView>` (sorted by cell).
- `TacticalIntent::Swing`: if `tactical_occupant(aim)` is `None` and `tactical_prop_at(aim)` is a destructible non-decoration prop → `game.tactical_attack_prop(aim)`.
- Aim readout row for a prop cell: name from piece id, `hp/max`, `explosive` tag when volatile. Fits the existing readout width (`popup-row-width-is-testable-headlessly`).

- [ ] Tests (app-core): Swing aim on a prop cell spends an action and reduces hp; on decoration or empty cell refuses with the status line; readout string for a volatile prop contains "explosive" and fits width.
- [ ] FAIL → implement → PASS → fmt/clippy → commit.

### Task 8: Draw props and explosions

**Files:** Modify `crates/gui/src/render/tactical.rs` (after `draw_ground` :97, before bodies), `crates/gui/src/fx.rs` (`TacticalFxKind` match ~:995), create `assets/effects/explosion_large.ron`, update `assets/effects/README.md` effect list.

**Interfaces:**
- Consumes `TacticalView.props`, `TacticalFxKind::PropDestroyed`.
- Draw `prop_<piece>` (or `_cracked` when `hp*2 < max_hp` and the sprite exists) through `Painter::sprite`, tinted brightness × `palette::ground_cast(biome)`; decoration at reduced brightness (named constant). Missing sprite → glyph fallback through `painter.map` (one glyph per class: `#` wall, `%` debris, `*` volatile, `,` decor), never both.
- `PropDestroyed { volatile:false }` plays effect `explosion`; `true` plays `explosion_large` at the cell (look up how `BoltCue`'s `fx` id resolves to a library effect and reuse that path).
- `explosion_large.ron`: `travel: Streak, impact: [Explosion(radius: 2.0), Smoke], shake: 2.0`.

- [ ] Tests (gui, headless where existing tests do): the draw order test pattern near `tactical.rs:2792` extended to include props; effect library loads `explosion_large`.
- [ ] fmt/clippy, commit.

### Task 9: Art, Sprite Forge, screenshot

**Files:** Create `scripts/battle-props.py` (model on `scripts/ground-tiles.py`), `assets/sprites/prop_*.png` (+ `_cracked`); modify `crates/app-core/src/app/sprite_forge.rs` (`load_static_sprite_subjects` :633) to list `prop_*` sprites; `CHANGELOG.md` unreleased notes only if the repo keeps such a section (otherwise leave to deploy).

- [ ] Script writes near-white 16x16 PNGs for every piece in Task 1 plus `_cracked` for each destructible one; derelict high-tech look (panel seams, broken LEDs as darker pixels, cracked glass). Run it; commit PNGs + script.
- [ ] Forge test: subject list contains `prop_server_rack`.
- [ ] Visual check: `FERAL_DEV_ARENA=1`/arena path (memory: battle screenshot goes through the arena) → `--screenshot`, Read the PNG on two biomes; adjust brightness constants if props read as washes or vanish.
- [ ] fmt/clippy, commit. **Final gate:** `cargo test --workspace`, `balance_sim`, clippy clean; then whole-branch review (opus) before deploy.
