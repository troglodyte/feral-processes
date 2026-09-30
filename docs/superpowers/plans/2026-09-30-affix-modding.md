# Affix modding — plan

Spec: `docs/superpowers/specs/2026-09-30-affix-modding-design.md`. Branch:
`affix-modding` (ask before moving to a worktree; the user plays from the
primary checkout). TDD, a commit per green step, every new test
mutation-checked with the fix committed. No `SAVE_FORMAT_VERSION` bump
(33 stays) → **minor** release.

## Shape

Four phases. Phases 1–3 are one sonnet dispatch each, serial (each consumes
the previous one's API). Every dispatch **forbids push** and stages explicit
paths. Phase 4 is inline. Each phase ends green on `cargo check --workspace`,
`cargo test -p <crates touched>`, `cargo clippy --workspace --all-targets`,
`cargo fmt`. Per-task review gates off; final review opus, whole branch.

## Anchors (verified 2026-09-30; paths under `crates/engine/src/` unless noted)

| What | Where |
|---|---|
| `AffixId`, `AffixDef` (+ `fault` :110), `AffixDb` (`pool_for`, `all`), `load_dir(&Path)`; `def()` test helper lists every field | `affixes.rs:28`, `:44`, `:141`, `:150`, `:206` |
| `load_asset_dbs`: structures → **research :3410** → items :3417 → … → **affixes :3444** | `game/lifecycle.rs:3408-3445` |
| `roll_affix` → `pick_affix` → `weighted_affix`; caravan's second roll site | `game/combat_rewards.rs:175`, `:31`, `:50`; `game/caravan.rs:536/552` |
| `affixes_of` (skips unknown ids) | `game/combat_rewards.rs:196` |
| `GearCopy` (`tier`, sorted `affixes`, `quality`), `with_affixes(item, rarity, tier, affixes, quality)` (sorts, no dedupe), `is_plain` | `items.rs:232`, `:303`, `:328` |
| `fuse_item` / `fuse_copy` (union at ~:1156; worn-copy delta swap :1187-1210); `count_copies`/`take_copies`/`add_copies` (route by `is_plain`) | `game/crafting.rs:1089`, `:1119`; `:641`, `:658`, `:674` |
| `EquipmentStats` (`atk, mitigation, decompiler, damage, accuracy, evasion`); `is_empty` :605, `has_upside` :628 (destructure); `scaled_for_level` :644, `fused_for_tier` :682, `for_rarity` :733, `for_quality` :770 (full literals) | `items.rs:565` |
| `apply_equipment_delta`; `copy_bonus` (affix fold :795); `worn_bonus`; `gear_bonus` (fold :856); `has_structure(&str)` | `game/crafting.rs:692`, `:780`, `:836`, `:848`, `:126` |
| `take_hand_craft_unit` (pay from pack `Inventory` + `note_consumed(.., ConsumeSource::Craft)`) | `game/crafting.rs:508` |
| `mitigate_incoming_damage` (early return on `percent <= 0`); sole caller `apply_damage` (13 callers: terrain, status, fumble, turret, …); `effective_mitigation` | `game/combat_damage.rs:473`, `:358`; `game/combat_round.rs:1952` |
| `MAX_FUSIONS = 3`; `MAX_MITIGATION_PERCENT` (combat-resolution block) | `tuning.rs:2631`, `:4338` |
| Calibration test; `axes() -> [(&str,i32,i32); 6]` | `tests/affixes.rs:140`, `:41` |
| `ResearchTree {Base, Routines}`; `ResearchDef` (`tree`/`teaches` are `serde(skip)`; `opens_routine_tree` :112; `unlocks_fusion` :148) | `research.rs:18`, `:41` |
| `ResearchDb::load_dir(dir, &StructureDb, &AbilityDb, &ToolDb)`: routine-node merge, then fixpoint cascade + Kahn; callers lifecycle `:3410`, `tests/routine_tree.rs:196`, `tests/extraction.rs:1085`, `research.rs:541` | `research.rs:169` |
| `routine_tree::synthesise_nodes` (+ the only `ResearchDef` literal :191, `node_id`) | `routine_tree.rs:178` |
| `node_researched` (no change); `capability_unlocked` :332, `routine_tree_open` :343, `fusion_unlocked` :350; `listed_research` :456 (`Base` else Routines); `has_research_tree` :891; `select_research` :929/:946; `eligible_discoveries` :1188 (`tree == Base` at :1195) | `game/unlocks.rs:264` |
| Save→load models | `tests/equipment.rs:1813`, `tests/research.rs:2839` |
| Base-node census `checked == 30` | `tests/assets.rs:~4877` |
| `stat_summary` (`MIT` row :624); `ManifestEquipSlot` (built `game/inspection.rs:2280`, drawn `gui/src/render/manifest.rs:641`) | `game/catalog.rs:617`; `views.rs:2690` |
| `Mode::Research`/`RoutineResearch`; group-menu rows | `app-core/src/lib.rs:2012/2018`; `app-core/src/app/group_menu.rs:163-203` |
| `handle_research_key(key, tree)`; key dispatch (exhaustive) | `app-core/src/app/progression.rs:98`; `app-core/src/app/input.rs:~270-346` |
| `inventory_item_actions(game,&ItemId)` (`'u'` Fuse :193; lowercase rule :203); `handle_inventory_item_action_key` (`'u'` :322) | `app-core/src/lib.rs:188`; `app-core/src/app/inventory.rs:~262` |
| `App::refuse` / `report` | `app-core/src/app/input.rs:442`, `:457` |
| `Mode::is_battle` (exhaustive) | `app-core/src/lib.rs:2217` |
| `draw_research_menu` (only exhaustive `match tree`); `draw_mode_overlay` (`_ =>` at :1509, **not** exhaustive; research arms :1466-1487); `ALL_MODES [Mode;122]` :1577; `NEEDS_PENDING_STATE [Mode;23]` :1775; `draw_inventory_item_action` | `gui/src/render/progression.rs:445`; `gui/src/render/mod.rs:890`; `gui/src/render/inventory.rs:716` |

## Where the spec is wrong or leaves a gap (decided here)

1. **Two roll sites.** The caravan also rolls from `pool_for`. **Filter in
   `AffixDb::pool_for`**, not `roll_affix`; test both drops and caravan shelf.
2. **Load order.** Move the `ResearchDb::load_dir` call below items and
   affixes (items don't read research). `ResearchDb::load_dir` gains
   `&AffixDb` and merges `affix_tree::synthesise_nodes` beside the routine
   merge, before the cascade. Update the three other callers.
3. **Item-id validation** of `apply_cost`/`materials`: `AffixDb::load_dir`
   gains `&ItemDb`. Refuse (skip + warn) an empty `apply_cost` or an unknown
   item id there. A `requires` naming an unknown or non-research affix is
   refused in `AffixDb::load_dir` too, iterated to a fixpoint (cascade).
4. **`weight` on a research-only affix** can't be told from its default.
   Warn when `weight != 1`.
5. **`is_empty`/`has_upside` must count `deflection`**, or DEFL-only
   affixes are refused at load. The four scalers + `copy_bonus` fold carry
   `deflection` through unscaled by hand; `gear_bonus` sums it.
   `apply_equipment_delta` untouched (live-read).
6. **`mitigate_incoming_damage` is restructured**: `if dmg <= 0 { return dmg }`;
   percentage (if > 0); subtract `min(gear_bonus.deflection, DEFLECTION_MAX)`
   clamped at ≥ 0; `.max(1)`. It reaches every `apply_damage` source — as the
   spec says ("every source that mitigates"); state it in the doc comment.
7. **Worn copies.** Apply/remove follow `fuse_copy`'s worn path
   (delta −1/+1, `Equipment.slot_mut`). Re-key only via
   `take_copies`/`add_copies` (removing the last affix can make a copy plain).
8. **The new copy key goes back to the caller.** `apply_affix`/`remove_affix`
   return `Result<(GearCopy, String), String>` — the mod screen must follow
   the re-keyed copy.
9. **Payment** is from the pack `Inventory`, like hand-crafting: check every
   line, then take all, then `note_consumed(.., ConsumeSource::Craft)`. Small
   private helper `pay_items(&[(ItemId,u32)]) -> Result<(), String>`.
10. **Duplicates:** remove deletes the first matching position only.
11. **Key case.** The item-action page uses lowercase action keys (rule at
    `lib.rs:203`); the spec's "UPPERCASE" is wrong there. **`'m'` "[M] Modify
    affixes…"** beside `'u'`, listed only for Weapon/Armor items. Inside the
    mod screen: lowercase selects a slot row; on a filled row `R` removes, on
    an empty row `Enter` opens the picker; picker rows lowercase apply.
12. **Hidden affix nodes.** `listed_research(Affixes)` is an explicit branch:
    empty while the tree is shut, else only discovered-or-researched nodes.
    `select_research` refuses a hidden Affixes node as it does a Base one.
13. **Mod Bench** node is `discoverable: true`, like `weapon_bench`/
    `armor_bench`; base census 30 → 31; `requires_subject: true` keeps it out
    of the ungated five.
14. **DEFL readouts:** `stat_summary` gains `(deflection, "DEFL")`;
    `ManifestEquipSlot` gains `deflection`, drawn beside MIT when non-zero.
    No wearer-total readout (YAGNI).
15. **Costs** (untuned until played):

| Asset | cost | materials | zone | apply_cost / notes |
|---|---|---|---|---|
| research `mod_bench` | 40 | bytecode_block 16, logic_wafer 8 | 0 | requires `weapon_bench`, `armor_bench` |
| structure `mod_bench` | — | build core_fragment 18, power_draw 3 | — | no `work`, no `assembles`; glyph `M` (unused) |
| `deflecting` | 30 | logic_wafer 6 | 2 | core_fragment 4 |
| `of_introspection` | 30 | trace_sniffer 3, logic_wafer 6 | 2 | trace_sniffer 2, core_fragment 4 |
| `of_deflection` | 90 | hardened_shell 8, bytecode_block 20 | 4 | hardened_shell 3, core_fragment 6; requires `deflecting` |

`DEFLECTION_MAX = 6` (both DEFL affixes together exactly reach it).

## Phase 1: engine data + Deflection (sonnet)

1.1 `EquipmentStats::deflection` (gap 5), `DEFLECTION_MAX`, restructured
`mitigate_incoming_damage` (gap 6), `stat_summary` DEFL, `ManifestEquipSlot`
field (gui render in phase 3). Tests: percentage→flat→floor 1; zero
mitigation still deflects; cap; unscaled by tier, rarity, quality, level
(one `copy_bonus` test per scaler); DEFL-only affix passes `fault`.
1.2 `AffixDef::research: Option<AffixResearch>` (spec §1 fields), `pool_for`
filter (gap 1), load validation (gaps 3, 4). Tests: research-only never
offered by `pool_for`; shipped pool identical (ids); each refusal warns and
skips; cascade.
1.3 Calibration `axes()` → 7 entries (DEFL 4).

## Phase 2: tree, bench, apply/remove, content (sonnet)

2.1 `ResearchTree::Affixes`; `ResearchDef::opens_affix_tree` (serde default);
`affix_tree.rs` with `node_id` (`affix:<id>`) and `synthesise_nodes`; load
order + merge (gap 2); `affix_tree_open()`, `affix_researched(&AffixId)`;
`listed_research`/`select_research` branch (gap 12); `eligible_discoveries`
admits Affixes while open. Tests: node fields; requires mapping; lenient rule
with the flag stripped; discovery pool byte-identical before the gate, admits
after.
2.2 Assets: `research/mod_bench.ron`, `structures/mod_bench.ron`, the three
affixes (gap 15). Asset censuses (descriptions, base-node count gap 13).
2.3 `affix_slots`, `appliable_affixes`, `apply_affix`, `remove_affix` (spec §5,
gaps 7–10), game-over/in-battle guards as `fuse_item`, log line + `tick()`.
Tests: each refusal (sentence asserted); pays exactly `apply_cost`, nothing
on refusal; slot cap; over-cap copy keeps all, refuses apply; duplicate
removal; worn copy's `Stats` right after apply/remove; last-affix removal
makes the copy plain and findable.
2.4 Save→load (real `Game::save`/`Game::load`): a modded copy (worn and
carried), and a discovered + researched affix node with the tree open.

## Phase 3: app-core + gui (sonnet)

3.1 app-core: `'m'` action (gap 11); `Mode::ModCopy`, `Mode::ModPickAffix`,
`App::mod_copy: Option<GearCopy>` updated from the returned key; refusals via
`App::refuse`. `Mode::AffixResearch` + group-menu row (shown via
`has_research_tree(Affixes)`, as Routines), dispatch to
`handle_research_key(key, Affixes)`. Add every new mode to `is_battle` and
input dispatch. Tests: key flow item menu → mod screen → apply → remove;
refusal line; research row opens the view.
3.2 gui: `render/mod_copy.rs` (slots, picker with stats + apply_cost, DECOMP
note "does nothing on a companion" for `decompiler` on a Weapon); Affixes arm
in `draw_research_menu` with its closed-tree line; arms in `draw_mode_overlay`
(non-exhaustive — don't forget), `ALL_MODES` 122 → 125,
`NEEDS_PENDING_STATE` 23 → 25 for the two mod modes; manifest DEFL. Draw via
`Painter` only. Width census for the picker row.

## Phase 4: controller, inline

1. `cargo test --workspace`; `balance_sim` green and unmoved.
2. Screenshots (`--template`, `--keys`), Read the PNGs: item menu with `[M]`,
   mod screen with a filled + empty slot, picker, Affixes research view.
3. Docs: `assets/affixes/README.md` (research block, research-only, DEFL,
   unscaled), `assets/research/README.md` (`opens_affix_tree`, Affixes tree),
   `assets/structures/README.md` if needed. Add `**/affix_tree*` to
   `seams-research.md`'s paths. CHANGELOG at release.
4. Final opus review, diff as a file. Ask it to re-derive independently:
   every `EquipmentStats` site carries `deflection` unscaled; every affix roll
   site excludes research-only; the discovery pool before the gate.
