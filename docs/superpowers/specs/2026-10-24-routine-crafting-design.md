# Routines built from parts

**Status:** Shipped in 0.20.0 (approved by the user 2026-10-24). ADR
[0029](../../adr/0029-player-routines-are-compiled-from-a-rust-part-catalogue.md)
is Accepted. Plan: `docs/superpowers/plans/2026-10-24-routine-crafting.md`; the work
follows the spec-and-plan pipeline with one release at the end.

Builder check run; its questions are answered below, and the three that were
the user's are under "Decided by the user".

Clickable mockup of the Workbench screens:
<https://claude.ai/artifact/9CaSbxtTHm77yxx9sSNgfv> (layout and flow only; its
numbers are placeholders).

## Intent

The player stops receiving ready-made routines. Instead they craft parts at the
Workbench and combine them into a routine they name. Heal plus Trickle makes a
heal over time; another Trickle makes it last longer; an Area part makes it hit
more than one target. Why: research and loot should hand out building blocks
that combine, so a routine is something the player designed rather than a
fixed file they unlocked.

## Vocabulary

- **Subroutine**: one part. Either a **Core** (what the routine does) or a
  **modifier** (how it is delivered).
- **Routine**: the finished, named thing that sits in a routine slot. The word
  keeps its current meaning on screen.
- A **built routine** is a Routine the player made. "Authored routine" in this
  document means one of today's `assets/abilities/` files.

## Goals

1. Every routine the player owns is built at the Workbench from Subroutines:
   exactly one Core plus zero or more modifiers.
2. The Core decides which modifiers fit. Modifiers that do not fit cannot be
   added; the screen greys them with the reason.
3. Cores come in versions (v1.0, v2.0, v3.0) where they have a magnitude.
   Modifiers have no versions.
4. Crafting one Subroutine costs **1 Routine Disk + N of one existing
   material**, where N is the Core's version number (1, 2, 3) and 1 for every
   modifier. No new materials.
5. Building a routine uses up its Subroutines. The player names it.
6. A built routine installs into the player's slots **or a companion's**.
7. Uninstalling a built routine destroys it. Nothing comes back.
8. Research unlocks Subroutines (each Core version, each modifier) and raises
   the **part cap**: how many parts one routine may hold.
9. Extracting from a downed program (Routines tool) or tearing down a tamed
   program at the Workbench gives the **Subroutines of one of its routines**,
   as items, instead of discovering a routine.
10. A new game starts with Decompile plus one built routine, **Patch**: Heal
    v1.0, one ally or yourself, no modifiers.

## Non-goals

- Wild programs, species kits (`abilities:`), species `moves:`, companion
  talent trees (`Ability(id)`) and item `grants` keep their authored routines,
  untouched.
- No return of parts on uninstall (the user chose "keep it simple" for now).
- No new materials, no new `assets/` kind, no new `.ron` fields (ADR 0029).
- No migration of old saves. `SAVE_FORMAT_VERSION` is bumped; the user's own
  `fred` save is edited by hand instead (see Save).
- No combining two Cores in one routine (damage + Teleport is a new mechanic,
  not a combination).
- `balance_sim` is unchanged: it models no routines.

## Anti-goals

- A combination that is strictly best and free. Power cost and cooldown must
  rise with what a routine holds.
- A combination that loads but cannot run. Every routine `compile` can produce
  must pass the same validators `AbilityDb::load_dir` applies to authored files.
- Any path left where the player obtains an authored routine (etching, the
  creation pool, a boss-drop disk, extraction).

## The parts

### Cores

One Core per routine. "Versioned" Cores exist as v1.0, v2.0 and v3.0.

| Core | Maps to `AbilityEffect` | Versioned | Notes |
|---|---|---|---|
| Damage | `Damage` | yes | |
| Heal | `Heal` | yes | |
| Drain | `Drain` | yes | |
| Boost Attack | `Buff { kind: Atk }` positive | yes | ally-facing |
| Boost Mitigation | `Buff { kind: Mitigation }` positive | yes | ally-facing |
| Sap Attack | `Buff { kind: Atk }` negative | yes | enemy-facing |
| Sap Mitigation | `Buff { kind: Mitigation }` negative | yes | enemy-facing; base from `acid_wash` (`etch` and `oxide_strip` are its Group and Everyone forms) |
| Debuff | `Debuff` | no | needs a Status modifier |
| Cleanse | `Cleanse` | no | |
| Cloak | `Cloak` | no | ally-facing |
| Fork | `Summon` | yes | v1 = 1 body, v2 = 1–2, v3 = 2–3 (my call) |
| Respawn | `Reanimate` | yes | count 1/2/3 (my call) |
| Cold Sample / Heat Injection | `Tamper { Temperature }` | no | two Cores, low and high temperature |
| Profile | `Tamper { Profiled }` | no | |
| Inject | `Tamper { Injected }` | no | |
| Hallucinate | `Tamper { Hallucinating }` | no | |
| Teleport | `Teleport` | no | |
| Reinforce | `Reinforce` | no | |
| Emulate | `Emulate` | no | player slots only (today's rule) |
| Field Regen, Recharge, Field Attack, Field Mitigation, Capture Boost, XP Boost, Encounter Damp, Drop Boost | `FieldBuff { kind }` (8 kinds) | yes | outside battle; "Recharge" is today's Trickle kind, renamed so it does not collide with the Trickle modifier |
| Phase, Jump, Symlink | `Phase`, `Jump`, `Symlink` | no | outside battle |

Decompile is not a Core. It stays pre-installed and permanent, as today.

### Modifiers

| Modifier | Kinds | Default without it | Effect |
|---|---|---|---|
| Range | Near (3 cells), Far (6 cells) | adjacent | sets `range`; Near or Far also sets `ranged` so a back-rank group may use it |
| Area | Group, Everyone | one target | sets `target` (and the battle-map `shape`) |
| Trickle | stackable | the Core's base: instant for Heal, the shipped v1 duration for durational Cores | each one adds duration on top of the base (see the table below) |
| Damage type | Thermal, Magnetic, Noise, Voltage, Radiation | untyped | sets `damage_type` |
| Status | bleed, stun, exposed, poison, throttled, locked | none | a rider on Damage; the whole effect of Debuff |

### What fits which Core

| Core | Range | Area | Trickle | Damage type | Status |
|---|---|---|---|---|---|
| Damage | ✓ | ✓ | — | ✓ | ✓ (rider) |
| Heal | ✓ | ✓ | ✓ (heal each round, in battle) | — | — |
| Drain | ✓ | ✓ | — | ✓ | — |
| Boost / Sap (Attack, Mitigation) | ✓ | ✓ | ✓ (+rounds) | — | — |
| Debuff | ✓ | ✓ | ✓ (+rounds) | — | required |
| Cleanse | ✓ | ✓ | — | — | — |
| Cloak | — | ✓ (Group = whole party) | ✓ (+rounds) | — | — |
| Tamper Cores | ✓ | Hallucinate only | ✓ (+turns) | — | — |
| Field Regen / Recharge | — | ✓ (Group = whole party) | ✓ (+turns) | — | — |
| other field Cores, Fork, Respawn, Teleport, Reinforce, Emulate, Phase, Jump, Symlink | — | — | — | — | — |

Every durational Core (Boost, Sap, Debuff, Cloak, Tamper, field Cores) has a
base duration taken from its shipped v1 file, so each is valid with no
modifiers at all; Trickle only lengthens it. Debuff alone needs a modifier
(Status).

**Facing.** Ally-facing: Heal, Boost Attack, Boost Mitigation, Cleanse, Cloak,
the field Cores, Fork, Respawn, Reinforce, Emulate. Enemy-facing: Damage,
Drain, Sap Attack, Sap Mitigation, Debuff, the Tamper Cores. Teleport takes no target (today's
rule).

Ally-facing Cores read Area as "one ally or yourself" / "your whole party";
enemy-facing Cores read it as one enemy / one group / every enemy. Combinations
the engine already refuses at load (an enemy-facing Cloak, a single-cell
Hallucinate) are not offered.

### The part cap

A routine holds at most the cap. The Core counts as 1, every modifier counts
as 1, each Trickle counts separately. The cap starts at **2** and rises by 1
per capacity research node, to **6** (my call).

## Life of a built routine

- Ids are `crafted/<n>` from a counter that only grows; an id is never reused.
- A built routine exists in exactly one place: unslotted (the Built routines
  page) or in one slot of the player or one companion.
- It is destroyed when uninstalled, and also when its holder leaves for good:
  a companion released, sold, fused away or torn down at the Workbench takes
  its built routines with it (the user's call). A companion that
  is downed, posted to the base or sent on a sortie keeps them.
- Emulate does not touch slots (it swaps the kit through `kit_of`), so it
  destroys nothing.
- `AbilityDb::set_crafted` is rebuilt from the surviving set after every
  change and on load, so no def outlives its routine.

## How parts become numbers

`routine_craft::compile(&CraftedRoutine) -> AbilityDef` is the one place
(ADR 0029). Its rules:

- **Base figures** for each Core at v1.0 are taken from today's shipped v1
  files where one exists (Heal from `rollback_v1`: 10 ± 2, cooldown 2,
  Power 6; Damage from `segfault_v1`: 6 ± 2, cooldown 2, Power 6), and live in
  `tuning/routine_craft.rs`.
- **Versions** multiply magnitude: v2.0 ×1.8, v3.0 ×2.8. Today's families run
  about 1 : 1.8 : 2.8 (Segfault) to 1 : 2 : 3.5 (Rollback); 1.8/2.8 is my call.
  Power cost rises +2 per version (Rollback's 6 → 8 → 10).
- **Area** lowers per-target magnitude and raises Power cost, in line with what
  today's Group and Everyone routines pay relative to Single. The factors are
  tuning constants the builder derives from the shipped families.
- **Range** adds Power cost and sets only how far away the effect may be
  centred (`range`). It never resizes or replaces the Area part's shape: a
  Group area with Near keeps the Group shape size, centred up to 3 cells away.
- **Trickle** adds a fixed duration per stack (a tuning constant), and Power
  cost per stack. Stacking gives the same amount each time (no diminishing
  returns for now; the cap is the limit).
- **Cooldown** = the Core's base + 1 per Area part + 1 per Trickle.
- **Status** rider on Damage lands at a fixed chance and duration (tuning).
- The routine's `description` is generated from the parts in plain language,
  like the mockup's preview line. The `name` is the player's text (1–24
  characters).
- Level and affinity scaling apply exactly as they do to authored routines; a
  built routine's class affinity follows its Core's effect
  (`AbilityEffect::affinity_kind`).

## Where Subroutines come from

- **Crafting** at the Workbench, for any part the player has researched.
  Cost: 1 Routine Disk + N of the part's material. Materials (my call, from
  existing items): `bytecode_block` for Heal, Boost, Cleanse, Cloak, Trickle
  and Status; `core_fragment` for Damage, Drain, Sap, Debuff, Area and Damage
  type; `power_cell` for Range, Tamper, movement, Fork, Respawn, Reinforce,
  Emulate and the field Cores. The builder must check each against the
  research-bill seam rule (a part's cost may only name what its own research
  prerequisites let the player make).
- **Extraction.** A Routines tool on a downed program draws one of its
  routines as today (`routine_candidate_ids`, weighted), then hands out
  `decompose(def)` as Subroutine items. Tearing down a tamed program at the
  Workbench (`Game::extract_routine`) does the same for the picked routine.
  Extraction gives parts the player has not researched; they can still be
  used in a build (my call: research gates crafting, not use).
- Subroutines are ordinary inventory items, synthesised into `ItemDb` from the
  Rust catalogue at load, one per part (and per Core version). They take over
  every role etched disks have in the item economy today (hauling, the depot,
  trade, the Stack market, `tuning/production.rs`): whatever an etched disk
  could do, a part can do, and nothing new. No new item field.
- **Extraction gives every part of the drawn routine**, Core and all
  modifiers. The Core's version comes from the routine's `vN.N` name; the
  routine's authored magnitude is not carried over (a decomposed routine
  rebuilt from its parts uses this design's numbers).

`decompose` maps every authored routine that a wild program, species kit or
talent can carry to a Core (with its version from the `vN.N` name, v1.0 when
there is none) plus the modifiers its `target`, `range`, `damage_type`, rider
and duration imply. A census test holds that every such routine decomposes
into a valid build.

## Research

The routine research tree keeps its place (`ResearchTree::Routines`, opened by
`routine_fabrication`) but its nodes change:

- One node per **Core version** (v2.0 requires v1.0, v3.0 requires v2.0) and
  one per **modifier kind**.
- **Capacity** nodes raise the part cap, each gated to a later zone (2→3 in
  zone 1, then one per zone up to 6; my call).
- Nodes are synthesised from the Rust catalogue, not authored, the way routine
  nodes are synthesised from `AbilityDb` today. The discovery layer
  (`DiscoveredRoutines`, `family_is_discoverable`) goes away; visibility is
  zone-gated only.
- Research cost reuses `tuning::routine_research_cost`'s shape, with the
  version standing in for today's version and capacity nodes priced by zone.
- Heal v1.0 and Decompile's prerequisites are known from a new game, so Patch
  can be rebuilt.

## The Workbench screens

The mockup is the reference for layout. The Workbench's structure hub gains
three pages on its rail, gated on the structure's existing
`extracts_routines` flag (my call, so no new `.ron` field):

- **Build routine**: one row per part slot (Core, Range, Area, Timing, Damage
  type, Status). Enter on a row opens a highlight-and-Enter list of the parts
  that fit, with how many the player holds. Rows the Core does not take show
  the reason in place of a part. Below: a live preview (the generated
  description, Power cost, cooldown), the name field, the parts it will use,
  any parts missing, and Compile.
- **Subroutines**: every part with its version, slot, how many held, its
  craft cost, and "locked · research to unlock" where unresearched. Enter
  crafts one.
- **Built routines**: built routines not currently installed, with their
  description.

Lists follow the house rules: highlight + Enter, no row letters or digits,
type-to-filter on the Subroutines list.

The existing routine screens (`m`, `Mode::Routines`): Install lists unslotted
built routines instead of etched disks. The **Etch** row is removed. Extract
stays and now yields parts.

## What goes away

- Etching: `Game::etch_disk`, `ItemDb::synthesise_etched_disks`, the `etched_*`
  items and `Mode::RoutineEtch`.
- The creation Routine step, `CharacterChoice.routine`, `starter_rows`, and the
  `starter:` field on ability files.
- `KnownRoutines` and `DiscoveredRoutines` as player state, replaced by known
  Subroutines.
- Ability files referenced by nothing but the player's research tree (31
  files: the 10 field routines, the Tamper, movement, Teleport, Reinforce,
  Emulate, Cloak and a few others). Their effects survive as Cores. Arena
  scenarios naming them switch to the nearest wild-pool id; the arena format
  gains no recipe field. Tests switch to building a routine through
  `compile`. `dev-saves/` templates holding removed ids are rewritten (they are
  dev tools, not frozen). The builder greps every remaining reference before
  deleting a file.
- The `assets/abilities/README.md` sections on the research tree, etching and
  the creation pool, rewritten to say player routines are built (and its stale
  "sixteen" effects count and slot numbers corrected).

## Save

- New: the list of built routines (`CraftedRoutine { id, name, parts }`, ids
  `crafted/<n>`), which of them are unslotted, the known Subroutines, and the
  part cap. Part items ride in the ordinary inventory.
- Slots (`PlayerSave.routines`, `CreatureSave.routines`) keep holding ids; a
  `crafted/<n>` id resolves because `AbilityDb::set_crafted` runs on load
  before slots are restored.
- Removed: `known_routines`, `discovered_routines`, and research ids of the
  form `routine/<id>`.
- `SAVE_FORMAT_VERSION` 38 → 39. Old saves are refused, which makes the
  release **breaking** under `CHANGELOG.md`'s preamble.
- **The `fred` save**: after the build lands, dump it with `savetool`, clear
  the player's slots to Decompile + Patch, drop authored routines the player
  had installed on companions (species-kit routines on companions stay), drop
  etched disks and the removed fields, bump the version, and pack it. Any
  research spent on removed routine nodes stays spent.

## Testing

- `compile`: every Core under every allowed modifier combination up to the
  maximum cap produces a def that passes `AbilityDb`'s load validators.
- `decompose`: every routine reachable by a wild program, species kit or
  talent decomposes into a build `compile` accepts.
- Build → install → battle → save → load round trip for a built routine on the
  player and on a companion (the "new save field needs a save→load test"
  rule).
- Uninstalling a built routine removes it from the save and from
  `AbilityDb`'s crafted tier.
- New game: slots are exactly Decompile + Patch; no creation Routine step.
- Extraction yields Subroutine items, never a discovery or an etched disk.
- No path grants the player an authored routine (census over the remaining
  grant sites).

## Left to the builder

- Module layout inside `routine_craft/`, the `Subroutine` enum's shape, and
  the item id scheme for parts.
- Exact tuning constants not fixed above (Area factors, Trickle duration and
  cost, Range cost, Status rider chance), derived from the shipped families.
- How unslotted built routines are stored (a list, or derived from the full
  set minus everything slotted).
- The order of the work; a plan follows this document.

## My calls (for the user to check)

1. Core list and names above, including splitting Buff into Boost Attack,
   Boost Mitigation and Sap Attack, and renaming the field Trickle Core to
   Recharge.
2. Version multipliers ×1.8 / ×2.8, and Power +2 per version.
3. Part cap 2 → 6, one capacity node per zone.
4. Materials per part family.
5. Extracted parts can be used before they are researched.
6. Workbench pages gated on `extracts_routines`.
7. Delete the 31 player-only ability files.

## Decided by the user after the builder check

1. **Heal + Trickle heals each round inside battle.** This is a new in-battle
   heal-over-time mechanic: each Trickle adds rounds, and the per-round amount
   is a share of the Core's heal (tuning). It must work in both battle models
   (the group fight and the battle map) and keep its existing out-of-battle
   counterpart, the Field Regen Core, separate. How it is represented (an
   over-time form of `AbilityEffect::Heal`, or a regeneration status) is the
   builder's call within ADR 0029: Rust only, no authored file gains a field.
2. **Bosses drop the parts of their routine.** The seven exclusive routines
   (Bus Snoop, Deadman, Hard Fault, Kernel Shear, Long Winter, Null Cache,
   Watchdog) drop as their `decompose`d Subroutines instead of an etched disk,
   at today's `boss_drop` odds. Their Cores must exist in the catalogue; where
   one does not fit a Core above, the builder raises it before inventing one.
3. **A companion that leaves for good takes its built routines with it.**
   Released, sold, fused away or torn down: they are destroyed.
