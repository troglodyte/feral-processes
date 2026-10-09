# Perception and Surface Fog — plan

Spec: `docs/superpowers/specs/2026-10-09-perception-fog-design.md`. The spec
holds the rules and the argument. This file gives the order of work, the
files, the corrections the source forced, and the gates. Branch:
`perception-fog`.

**Every phase:**
- Use TDD and commit at each green step.
- Leave `cargo fmt` and `cargo clippy --workspace --all-targets` clean.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.
- Never push. Never `git checkout`, `stash` or `reset` over uncommitted
  work. Stage explicit paths only.
- Before editing, read `.claude/rules/seams-ground.md` and
  `seams-screens.md`. Phase 1 also reads `content-schema.md` and
  `seams-combat.md`. Phase 3 also reads `drawing-seam.md` and `seams-hud.md`.
- The phases run in order, because each one consumes the previous phase's
  API. Each phase fits one fresh subagent: hand it this file and the spec,
  not the history.

## Corrections and decisions (verified against source, 2026-10-09)

Where this list and the spec disagree, this list wins.

1. **`DerivedBase` is saved, so it needs a serde default.** `DerivedBase`,
   `DerivedStats` and `derive` are in `progression.rs` (:273, :321, :442),
   not in `attributes.rs`. `DerivedBase` is written to
   `CreatureSave.base` (`save.rs:1021`) and appears literally in the
   `dev-saves/*.ron` templates.
   - Give the new `perception` field
     `#[serde(default = "default_perception")]`, a function returning
     `PERCEPTION_BASE_RADIUS`. A `0.0` default would clamp every program
     to the minimum.
   - Still no `SAVE_FORMAT_VERSION` bump.
   - Test: every `dev-saves/` template still loads. Also do a real save
     and load of a seated program whose `base` has no `perception`.
2. **Remembered landmarks.** The user chose: settlements, the base anchor,
   nests and Stack links stay drawn dim once seen. Caravans, patrols,
   traps and creatures show only in sight.
   - Settlements are `components::Settlement` / `SettlementCentre`, not
     `Structure`.
   - Add `EntityView.is_landmark: bool`, set in view construction from
     those components, `is_anchor`, the nest component, and the Stack-link
     component (find the last two by name; they may not be entities, and
     if not, skip them).
   - `shown_at` reads `is_landmark` in place of `is_structure`.
3. **Trade and build scans are not fogged.** The 40-tile
   `MENU_SCAN_RADIUS` scans (`traders_in_range`, `trade.rs`,
   `inventory.rs`, `building.rs`, gui `render/trade.rs`) stay as they are.
   The user chose this.
4. **The fogged readers** are:
   - `gui render/base.rs::draw_surface_map`, at the entity filter
     (:704-724). The Threat count (:489) and the chase lookup (:1673)
     follow, because they read the same filtered Vec.
   - `app-core app/hover.rs:22` and `app/travel.rs:56-60`, beside their
     existing `drawn_on_surface_map` filter.
   - `Game::find_target_in_direction` (`inspection.rs:496`), which runs
     its own queries: creatures around :556 and settlements around
     :602-611. Apply `sight_at` to each. A remembered settlement stays
     examinable; a hidden creature does not.

   Gate: `rg 'view_entities(_at)?\(' crates --glob '!**/tests/**'`. Every
   production hit is either one of the readers above or one of the scans in
   item 3. The `view_entities(12, 12)` calls in tactical and battle code are
   `#[cfg(test)]`.
5. **Helpers to widen.** `chunk_of` (`world_map.rs:19`) and `dev_reveal`
   (`stack_view.rs:49`) are private. Make them `pub(crate)`.
6. **Locale.** `Locale` is `Surface | Stack | Base`; there is no zone
   locale. "Off the surface" means `!matches!(locale, Locale::Surface)`.
7. **Mark at start.** `Game::new` doesn't call `mark_explored_chunks`
   today. Call `mark_seen_tiles` at the end of `new_with`
   (`lifecycle.rs:~717`) and of `load` (`:~2039`).
   - Save writer: `Game::save` (`lifecycle.rs:~3376`, beside
     `explored_chunks`).
   - Load read: beside `:1894`.
   - Default inserts: beside `:604` and `:1567`.
   - The `SaveData {` literals are `lifecycle.rs:3226`, `save.rs:2296` and
     `tests/spawning.rs:1655`.
8. **Baseline test uses base attributes.** Every class moves Entropy off
   40, so a real character starts between 4.67 and 6.12 tiles.
   - Test with class `None` or `attrs_at_base`.
   - The spec's "exactly 5" holds only there.
9. **Setting attributes in tests.** There is no setter on `Game`.
   - Engine: the `bank` helper in `tests/derived.rs:172`, then
     `spend_stat_points`.
   - app-core and gui: edit `PlayerSave.attributes` in the save, then
     `Game::load` (precedent: `app-core/src/tests/level_up.rs:405`).
   - +7 tiles takes 14 Analysis.
10. **Party companions are never drawn on the surface**
    (`drawn_on_surface_map`). The "party always shown" row matters only for
    the player and walking tamed programs.
11. **The fog dim constant** `FOG_REMEMBERED_DIM` goes beside
    `VIGNETTE_FLOOR_*` in `gui/src/render/terrain.rs:26`. The gui has no
    tuning file.
12. **Screenshot template:** `settlements` or `world-map` (both surface).
    `stack` is underground.

## Phase 1 — the stat (engine + assets)

- `attributes.rs`: `DerivedStat::Perception` and its `label()` arm.
- `tuning.rs`: `PERCEPTION_BASE_RADIUS = 5.0` (doc names
  `MAX_BUILD_DISTANCE_FROM_HOME`) and `PERCEPTION_MIN_RADIUS = 2.0`.
- `progression.rs`:
  - add the field to `DerivedBase` with the serde default from item 1;
  - set it in `player()`; `program()` inherits it through `..player()`;
  - add it to `DerivedStats`, the `get()` arm, and the
    `attribute_contribution` literal;
  - in `derive`, clamp it to `>= PERCEPTION_MIN_RADIUS`.
- `components.rs:2072`: the `Derived` field, with a `Default` of
  `PERCEPTION_BASE_RADIUS`. Also the `recompute_derived` literal
  (`game/derived.rs:354`).
- `Game::perception_radius()`, which reads the player's `Derived`.
- Assets:
  - `analysis.ron` gets `(Perception, 0.5)` and `entropy.ron` gets
    `(Perception, 0.033)`, each with its `does:` line updated;
  - `assets/attributes/README.md:65` lists `Perception`;
  - update `attributes.rs:404` `the_shipped_effects_are_the_specs_table`.
- Tests in a new `tests/perception.rs`: the four derivation cases (item 8),
  the dev-saves load, and the old-`base` load (item 1).
- **Gate:** `cargo test -p feral-processes-engine`, and `balance_sim`
  unchanged.

## Phase 2 — sight and memory (engine)

- `resources::SeenTiles` as specified (`BTreeMap<(i32,i32), ChunkSeen>`,
  `[u64; 16]`).
- `mark_seen_tiles`: call it in `turn.rs` beside `mark_explored_chunks`
  (:233), surface only, plus at the item 7 points.
- Save field `seen_tiles` with `#[serde(default)]`, at every item 7 site.
- The new API:
  - `Sight`, `sight_at` and `sight_view_at(center, hw, hh)`, which mirrors
    `view_tiles_at`'s shape;
  - `dev_reveal` and off-surface both answer `InSight`.
- `EntityView.is_landmark` (item 2).
- `views::shown_at(sight, &EntityView)` implementing the table, with
  landmarks in place of structures.
- `find_target_in_direction` filtering (item 4).
- Tests: every spec engine case.
  - The save test is a real save and load, plus an old save with no field.
  - `shown_at` gets one case per row, including anchor and caravan.
  - Examine skips a hostile at 8 tiles but still names a remembered
    settlement.
- **Gate:** `cargo test -p feral-processes-engine`.

## Phase 3 — readers, rendering, full gate (app-core + gui)

- app-core: filter `hover.rs` and `travel.rs` by `shown_at(sight_at(pos))`.
  - Tests: a hostile at 8 tiles gets no hover and no click-travel goal.
  - Fix `menus.rs:290/305` (`EXAMINE_RANGE_TILES` + 1 / 12) by raising
    Analysis 14 points (item 9). Other app-core placements are ≤ 4 tiles
    and need no change.
- gui `draw_surface_map`:
  - one `sight_view_at` call;
  - filter entities with `shown_at`;
  - Unseen tiles are filled black;
  - Remembered tiles fold `FOG_REMEMBERED_DIM` into `vig` (:1004), with no
    cloud shade or jitter;
  - the one-tile edge fade uses `perception_radius()`.
- gui `points.rs::effect_text`: a `Perception` arm reading
  `Perception 5.0 → 5.5 tiles`.
- gui tests:
  - new tests per the spec's gui list;
  - fix the far-hostile tests by moving the creature within 5:
    `:3040`, and the `a_wild_creature`/`an_unseen_wild_creature` helpers
    (:3573, :3584) with their users (:3643, :3666, :5785);
  - other breaks get the same fix, never a fog bypass.
- **Gates:**
  - `cargo test --workspace` and clippy;
  - `balance_sim` unchanged;
  - the item 4 `rg` audit;
  - `cargo run -- --template settlements --keys "Right Right Right"
    --screenshot fog.png`, then Read the PNG. Judge fog against the vignette
    at low Power, and retune `VIGNETTE_FLOOR_*` only if it reads as mud.

## After Phase 3

- A final whole-branch review on opus.
- Landing is the `deploy` skill: 0.18.7, a patch, with the `CHANGELOG.md`
  section.
