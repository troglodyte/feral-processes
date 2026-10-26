# 0031 Player gear is built from designs over a slot-generic frame

- Status: Accepted
- Decided by: the user, 2026-10-26
- Date: 2026-10-26
- Pattern: Builder (parts compiled into a value), as 0029; the slot split is Strategy (one `compile` per slot behind one recipe type)
- Depends on: [0029](0029-player-routines-are-compiled-from-a-rust-part-catalogue.md), [0019](0019-content-defaults-to-rust.md), [0023](0023-saves-are-field-named-ron.md), [0017](0017-game-is-the-entire-api.md)

## Context

Every weapon is an authored file in `assets/items/` (15 of them), crafted from a fixed `craftable` bill or found as a drop.
What a weapon *is* (band, accuracy, `range`, `reach`, `damage_type`) is fixed per file. The only per-copy variation is `GearCopy` (rarity, fusion tier, quality, wear, and `affixes`, which are stats only).
No weapon field applies a status on hit; three weapons do it only through a `grants` routine.
Routine crafting (0029) showed the shape that works here: a saved recipe, one pure `compile`, and one registration door into a content db so every lookup site keeps working.
The design in `docs/superpowers/specs/2026-10-26-weapon-crafting-design.md` wants the same for weapons now, and for armour and modules later.

## Decision

**A new subsystem `crates/engine/src/gear_craft/`, slot-generic at the edges and per-slot in the middle.**

- Generic: `GearDesign { id, name, slot, parts: Vec<GearPart> }`, the saved value; `GearPart` is an enum with one variant per part family (`Weapon(WeaponPart)` now, `Affix(AffixId)` for every slot); the registration door; the research-tree synthesis; the design screens.
- Per slot: the part catalogue enum and a pure `compile`. Only `gear_craft::weapon` is written now. `gear_craft::compile(&GearDesign, &AffixDb) -> ItemDef` matches on `slot` and calls it. Armour and modules get no code until they are built.

**A design is a recipe, saved; its `ItemDef` is derived.**
Compiled defs register in `ItemDb` under `crafted_gear/<n>` through one door, `ItemDb::set_crafted_gear`, called whenever the design set changes and on load, before any inventory or equipment is restored.
`ItemDb::get` answers for authored and crafted ids, so a built weapon is an ordinary item from then on: `GearCopy`, equip, wear, traders and `copy_bonus` all work unchanged. Fusion is the exception: it refuses a crafted copy (the user's call — fusion is for drops).

**Runtime-only weapon behaviour.** The on-hit rider and the exploit payoff are new `ItemDef` fields marked `#[serde(skip)]`, set only by `compile`. No authored file can set them, and no `.ron` field is added (the same move 0029 made for heal-over-time).

**A fourth research tree**, `ResearchTree::Weapons`, synthesised from `WeaponPart::all()` like `routine_tree::synthesise_part_nodes`, and opened in Rust when the existing `weapon_bench` node is researched (no new `opens_*` field).

## Alternatives considered

- **Reuse `Subroutine` for weapons.** The user ruled this out: weapons don't use routine parts. The two only share the game states they act on (statuses, damage types).
- **A weapon-only subsystem now, generalise later.** Cheaper today, but the user asked for armour and modules next. The recipe, door, tree and screens are what all three would repeat. The per-slot numbers stay per slot either way.
- **Generic all the way down (one part enum, one compile).** Armour parts and weapon parts don't mean the same thing. A shared `compile` would grow a branch per slot inside every rule.
- **Bake chosen affixes onto the minted `GearCopy`** instead of into the design. That would count them against `affix_slots` (1 on a new copy) and make every build of a design differ. Design affixes are folded into the compiled `EquipmentStats` instead. A copy keeps its own Mod Bench slots.
- **Parts as `assets/weapon_parts/`.** Fails 0019 for the same reasons as 0029.

## Consequences

Retuning a part moves every weapon built from it on the next load.
`ItemDb` gains a mutable tier beside `AbilityDb`'s. 0004, if agreed, moves both.
Two builds of one design share an item id; fusion refuses them with a typed reason.
The save gains `gear_designs`, `next_gear_design_id` and `known_affixes`, additive and `#[serde(default)]`, so no `SAVE_FORMAT_VERSION` bump. A save→load test is required.
`balance_sim` keeps its hard-picked authored weapon. A census test holds every compiled weapon under a per-frame ceiling instead.

## Evidence

- `crates/engine/src/routine_craft/` — `compile`, `decompose`, `plan_build`; `AbilityDb::set_crafted`.
- `crates/engine/src/items_db.rs` — `ItemDef`, `ItemDb::synthesise_parts`, `worn_stats`.
- `crates/engine/src/items.rs` — `EquipmentStats`, `WeaponReach`, `GearCopy`.
- `crates/engine/src/game/combat.rs`, `game/combat_damage.rs` — `attack_profile`, `swing_range`, `swing_reach`, `type_side_effect`.
- `crates/engine/src/research.rs` — `ResearchTree`; `routine_tree.rs`, `affix_tree.rs`.
- `crates/engine/src/game/affix_mods.rs` — `apply_affix`, `affix_slots`.
