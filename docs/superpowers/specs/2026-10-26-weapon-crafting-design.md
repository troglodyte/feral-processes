# Weapon crafting

**ADR:** [0031](../../adr/0031-player-gear-is-built-from-designs-over-a-slot-generic-frame.md) (Proposed — no code until agreed)
**Model:** [routine crafting](2026-10-24-routine-crafting-design.md), ADR 0029

## Intent

The player should feel they *made* their build. They choose what their weapon does (how far it hits, how many it hits, what it does on contact, what it cashes in on), and those choices line up with their routines and the way they like to fight. A routine that stuns and a weapon that punishes stunned targets is a plan the player put together. That's the point of this feature.

Weapons are their own system. They don't use routine parts. They have their own parts, their own research tree, and effects the player discovers. The two systems meet only through the game states both act on: statuses and damage types.

The code is written so armour and modules can follow with the same machinery. Only weapons are built now.

## Vocabulary

- **Weapon part**: one component in the catalogue (`WeaponPart`, Rust).
- **Design**: a named recipe of a slot plus parts (`GearDesign`). Saved. Its weapon is derived from it.
- **Build**: spending parts to make one copy of a design.
- **Frame**: the one required part. It sets the weapon's family (melee, lance, launcher) and its base numbers.
- **Setter / payoff**: a part that applies a state on hit, and a part that does more against a target already carrying a state.

## Goals

- A player can design a weapon from a frame plus modifiers, name it, and build copies of it.
- Modifiers cover: range, reach (group or everyone), damage type, an on-hit status rider, an exploit payoff, a trade-off tilt (accuracy vs band), capture (decompiler), a granted proc routine, and affixes. No armour (mitigation) on crafted weapons, by the user's decision.
- Setters and payoffs use the same six statuses routines apply (`routine_craft::StatusKind`), so weapon and routine builds can feed each other.
- Weapon parts are unlocked through a Weapons research tree, and some are found rather than bought.
- Every built weapon is an ordinary item afterwards: equip, wear, fusion, Mod Bench affixes, selling.
- The generic layer (design value, registration door, tree synthesis, screens, affix category) is shaped so armour and modules add a part enum and a `compile`, and nothing else.

## Non-goals

- Armour and module crafting (designed for, not built).
- Changing routine crafting, or moving it onto the new layer.
- Changing how affixes roll on drops, or the Mod Bench.
- Wild programs' and enemies' weapons (they keep authored items).
- `balance_sim` modelling crafted weapons.

## Anti-goals

- **A strictly best design.** Every modifier costs something on the weapon's own axes (band, accuracy, recharge). If one combination beats all others everywhere, the feature has failed even if it works.
- **A weapon that is a free routine.** Group/everyone reach keeps a recharge, and riders are chance-based and weaker than a routine's Status part.
- **Authored weapons made pointless overnight.** A compiled weapon stays under a per-frame ceiling (see Testing).
- New materials, new `.ron` fields, a new `assets/` kind.

## The parts

All numbers live in `tuning/gear_craft.rs`. The figures below are shape, not tuning.

### Frames (exactly one per design, versioned v1–v3 like routine Cores)

| Frame | Range | Base character | Lifted from |
|---|---|---|---|
| Edge | 1 (melee) | high accuracy, narrow band | `kinetic_edge`, `shiv_routine` |
| Lance | 2 | middle | `arc_lance` |
| Whip | 2 | wide band, no accuracy bonus | `monofilament_whip` |
| Launcher | 3 | heavy band, low accuracy | `plasma_router`, `siege_compiler` |

Versions scale the band like routine versions (`CRAFT_VERSION_MAGNITUDE` shape), gated by zone.

### Modifiers (each fills one category slot; the cap counts them)

| Category | Options | What it costs |
|---|---|---|
| Range | Extend (+1, max 3) | accuracy |
| Reach | Group, Everyone | band and a recharge (the existing `WeaponReach` rule) |
| Damage type | Thermal, Magnetic, Noise, Voltage, Radiation | band (types bring their own side effects) |
| Rider (setter) | one of the six statuses, chance per landed hit | band |
| Exploit (payoff) | one of the six statuses: +% damage against a target carrying it | nothing extra when the status is absent; a small accuracy cost |
| Tilt | Precise (+acc, −band) or Heavy (+band, −acc) | itself |
| Capture | +decompiler (helps capture programs, as `shim_blade` and `black_ice_pick` do) | band |
| Proc | grants one routine the weapon fires on its own, from the set authored weapons grant today (`clock_skew`, `core_dump`, `interrupt_request`), via the existing `ItemDef.grants` | band and accuracy |
| Affix | any affix that fits `Weapon` and the player knows | counts against the cap |

**Fits:** at most one part per category, except Affix (up to the cap). Rider and Exploit may name the same status (a self-feeding weapon), but that design costs the most.

**The cap:** a design holds the frame plus `GEAR_PART_CAP_START` modifiers, raised by `weapon_capacity/<n>` research up to `GEAR_PART_CAP_MAX`, mirroring `Game::part_cap`.

### Synergy, concretely

- Routine applies Exposed → weapon with Exploit(Exposed) hits harder.
- Weapon Rider(Bleed) → a routine Drain or a companion's bleed payoff has a target ready.
- Damage type stacks with the type side effects already in `damage/effects.rs` (Voltage arcs, Thermal heat, Noise jams).

## Life of a design and a build

1. Researched or held weapon parts appear at the Workbench's **Forge** pages.
2. The player starts a design, picks a frame, adds modifiers by category, sees a live preview (the same numbers `gear_detail` will show), and names it (1–24 chars).
3. **Build** pays the design's parts from reach (`pay_from_reach`) plus a material bill from the frame (core_fragment / portal_fragment as authored weapons cost today), and mints one Ordinary, tier-0 `GearCopy` of `crafted_gear/<n>`.
4. The design stays. Building again needs a fresh set of parts. Two copies of one design share an item id, so they fuse.
5. A design can be deleted only when no copy of it exists anywhere (inventory, worn, companions' gear, a trader's buyback shelf — `BuybackLedger`); otherwise the delete is refused with a typed reason (ADR 0027).

## How parts become numbers

One pure `gear_craft::weapon::compile(&[GearPart], &AffixDb) -> ItemDef`:

- Frame sets `EquipmentStats` (atk, band, accuracy) and `range`.
- Modifiers adjust those and set `reach`, `damage_type` and the runtime-only rider/exploit fields.
- Design affixes are summed into `EquipmentStats` before any copy scaling, the same point an affix enters today, so they grow with level, tier and rarity.
- The output must pass every validator `ItemDb::load_dir` runs on an authored weapon (band present, range 1..=3, reach plural and enemy-facing).

Combat reads the rider in `resolve_and_apply_attack`, beside its `type_side_effect` call, through the existing status-arming code. The exploit's +% is applied in the swing's damage calculation *before* `Game::apply_damage` / `apply_damage_typed`, which see no attacker or weapon and stay the only damage path.

**Rider figures:** each status has its own tuned rider (chance, rounds, power) in `tuning/gear_craft.rs`, set below the routine Status part's `CRAFT_RIDER` for the same status. The "weaker than a routine" anti-goal is a test comparing the two tables. **Stun is not offered as a rider** (my call: a stun on a battle map costs no turn, see `stun-costs-no-turn-on-a-battle-map`); it stays available as an Exploit.

## Where weapon parts come from

- **Craft** at the Workbench once the part's research node (`weapon_part/<key>`) is researched, for a material bill (mirrors `craft_subroutine`).
- **Salvage** an authored weapon at the Workbench: `gear_craft::weapon::decompose(&ItemDef)` hands out the parts it maps to and *discovers* their research nodes. The copy is consumed. Decompiler maps to Capture, `grants` maps to Proc(that routine), and mitigation maps to nothing. Proc parts come **only** from salvage, which makes them the clearest "found it" moment.
- **Study discovery:** weapon-tree nodes are `discoverable`, and `eligible_discoveries` gains an explicit `Weapons`-when-open branch beside its `Affixes` one (`discoverable` alone does not admit a new tree).
- **Shelves:** Stack market and caravans may sell frames and modifiers (one `shelf()` door, as `Subroutine::shelf`). Payoff parts stay off shelves; they are found or researched.

**Affixes in a design:** the player may pick an affix whose `affix:<id>` node is researched, or one they have salvaged off a copy that carried it (salvage records it in `KnownAffixes`, saved — see Save).

## Research

- `ResearchTree::Weapons`, a fourth tree on the existing research screen (new Mode, `listed_research` branch, gui title and empty state).
- Opened when `weapon_bench` is researched. Until then the player has no crafted weapons: early weapons come from drops and traders only (the user's decision).
- Nodes are synthesised from `WeaponPart::all()`: frames chain v1→v2→v3 with `min_zone = version`, and modifiers have no chain except Exploit, which requires the matching Rider's node. `weapon_capacity/<n>` nodes raise the cap.
- Cost from a `tuning::weapon_research_cost` shaped like `routine_research_cost`.

## The Workbench screens

New structure-hub pages beside the routine ones: **Forge** (designs list, highlight + Enter), **New design** (category rows, part picker, preview, name), **Weapon parts** (craft parts). They follow `seams-screens`: highlight + Enter, type-to-filter on long lists, and read-only row counts owned by app-core. New app-core logic goes in `app/gear_craft.rs`, never `lib.rs`. The gui goes in `render/gear_craft.rs`.

## What changes for authored weapons

They stay as drops and enemy gear. Their `craftable` blocks (8 of the 15 weapons have one, including the no-research `shiv_routine` and `kinetic_edge`) are removed: making a weapon now means designing one, and before `weapon_bench` there's no way to make one. Tests that pin the craftable gear tiers (`standard_and_premium_gear_is_made_from_intermediates` and its fragment-only sibling) are updated to drop weapons from their sets, not deleted. Check that `shiv_routine` and `kinetic_edge` can still be found early (a drop table or a starting shelf): if neither is, the early game has no weapon source, and the plan must add one. `weapon_bench` keeps its research role and now opens the Weapons tree.

## Save

`SaveData.gear_designs: Vec<GearDesign>`, `next_gear_design_id: u32` and `known_affixes: BTreeSet<AffixId>`, all `#[serde(default)]`. Weapon-part items are synthesised ids, not saved state. `set_crafted_gear` runs on load before inventory, equipment, companions and shelves are restored. No version bump.

## Testing

- `compile` unit tests per part, and every design in `all_designs_up_to_cap()` passes the authored-weapon validators.
- **Ceiling census:** no compiled design's `design_power` exceeds `GEAR_FRAME_CEILING[frame][version]`. `design_power` is `copy_power` plus a tuned flat value per rider, exploit and proc (`copy_power` cannot see those runtime-only fields, so without the term the riskiest parts would read as *weaker*). The no-dominance test uses the same measure.
- **No-dominance test:** for each pair of modifier sets of the same frame and size, neither is better on every axis.
- Rider and exploit: battle-level tests that a rider arms its status on a landed hit (seeded) and that an exploit raises damage only when the status is present.
- Save→load round-trip with a design, a worn built copy, a fused one and a salvaged known affix.
- `balance_sim` unchanged and green.

## Left to the builder

Exact figures, the order rows appear in, the part item glyph and name prefix ("Weapon part · X"), how the preview lays out, `tuning` constant names.

## My calls (check these)

- Workbench hosts the Forge, not the Mod Bench (the Mod Bench is late research).
- Proc offers only the routines authored weapons grant today, not crafted routines.
- Design affixes are folded into the weapon's base stats and count against the cap, not against `affix_slots`.
- Designs persist and can be rebuilt. Same design = same item id = fusable.
- Authored weapons lose their `craftable` blocks.
- Payoff parts stay off shelves.
- Radiation becomes available on a weapon (none ships today); the census rule that a typed weapon needs a resisting armour must be checked.

## Decided by the user

- Weapons don't use routine parts. They have their own catalogue, research tree and discoverable effects (2026-10-26).
- Affixes are a category you pick from while designing (2026-10-26).
- Make what can be shared generic, so armour and modules can follow (2026-10-26).
- No crafted weapons before `weapon_bench`: early weapons are drops and traders only (2026-10-26).
- Crafted weapons may carry decompiler (Capture) and a granted routine (Proc), not armour (2026-10-26).
