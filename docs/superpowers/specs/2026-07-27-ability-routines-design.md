# Ability routines: extractable, slot-limited abilities

**Date:** 2026-07-27
**Branch:** `feat/ability-routines`
**Status:** Design, approved

## Problem

Abilities are welded to whoever was born with them. A companion's kit is
whatever its species file declares, level-gated and always fully available;
the player's kit is whatever research has unlocked, also always fully
available. Nothing can be moved, traded, chosen between, or given up. Two
consequences:

- A program with a kit you don't want is simply a worse program. There is no
  way to build away from it, so a rare capture with a poor kit is dead weight.
- Nothing is ever a decision. Every ability you own is usable every battle, so
  more abilities is strictly better and progression is pure accumulation.

Phase 2 of the abilities work (`2026-07-26-player-ability-unlocks-design.md`)
explicitly deferred "teaching or installing abilities onto companions,
ability modules as craftable items, capacity limits on how many abilities
anyone can hold." This spec builds all three.

## Scope

Abilities become **routines**: installable objects that occupy a limited
number of slots on any party member, including the player. Routines are
extracted from programs you own — destroying the program — and can then be
installed anywhere.

Also in scope, because the menus this feature adds need it: item and
structure descriptions move out of Rust and into the `.ron` files.

Not in scope: crafting routines from raw materials, routine rarity or
quality tiers, routine-specific gear, and any new ability content beyond the
one new `decompile` ability the unification requires.

### Naming

The thing is called a **routine**, not a module. `EquipmentSlot::Module`
already exists in `crates/engine/src/items.rs:56` and means equippable MOD
gear granting flat ATK/DEF/DECOMP. "Routine" is already the word this
codebase's own docs use for abilities (`combat.rs:419`: "the player has no
routines at all"), fits the program fiction, and collides with nothing.

## Existing architecture this builds on

Verified by reading, not assumed:

- **`Game::actor_abilities` already unifies player and companion** at
  `crates/engine/src/game/combat.rs:440`. Everything downstream of it —
  cooldowns, recipient expansion, effect application — is entity-generic.
  Only the two functions feeding it care where an ability came from.
- **`Game::has_structure(kind)`** (`crates/engine/src/game/crafting.rs:56`)
  is already "built anywhere, no proximity required" and already gates
  researched recipes. The extraction bench needs exactly this, unchanged.
- **`Game::sell_companion`** (`crates/engine/src/game/trade.rs:158-188`)
  already consumes a tamed program: ownership check, battle guard, despawn.
  Extraction is the same shape with a different payout.
- **Saves reject other versions outright** (`crates/engine/src/save.rs:195`),
  a documented deliberate choice. There is no migration to write, only a
  version bump.
- **Shipped species declare at most 2 abilities**, latest unlock level 8
  (`cipher`: `null_route` at 8). No shipped species can outgrow its slots.
  Eight of seventeen declare none at all and rely on the fallback.
- **`ItemDef` and `StructureDef` have no description field.** Item text does
  not exist; structure text is derived in Rust by `Game::structure_description`
  and guarded by `tests/building.rs:364`, a test that exists because the
  derivation went stale when `pet_slot_bonus` was added.

## Design

### Routines and slots

A routine is an installed `AbilityId` occupying a slot. New component on the
player and every companion:

```rust
Routines(Vec<AbilityId>)   // len <= slots(level); position is menu order
```

Slot count comes from one pure function with two constant sets, all in
`crates/engine/src/tuning.rs`:

```
slots(level) = clamp(BASE + level / PER_LEVEL, 1, CAP)

COMPANION_ROUTINE_SLOT_BASE      = 0
COMPANION_ROUTINE_SLOT_PER_LEVEL = 2
COMPANION_ROUTINE_SLOT_CAP       = 6

PLAYER_ROUTINE_SLOT_BASE      = 1
PLAYER_ROUTINE_SLOT_PER_LEVEL = 10
PLAYER_ROUTINE_SLOT_CAP       = 6
```

| Companion level | 1–3 | 4–5 | 6–7 | 8–9 | 10–11 | 12 |
|---|---|---|---|---|---|---|
| Slots | 1 | 2 | 3 | 4 | 5 | 6 |

| Player level | 1–9 | 10–19 | 20–29 | 30–39 | 40–49 | 50+ |
|---|---|---|---|---|---|---|
| Slots | 1 | 2 | 3 | 4 | 5 | 6 |

The caps are separate constants deliberately: the two sides are tuned
independently, and the player's slower rate means their first free slot —
the one not holding `decompile` — arrives at level 10.

The player has no level cap (`level_cap: None` in `progression.rs`), so the
`CAP` clamp is what stops their slots growing forever.

### Where a companion's kit comes from

Species abilities become **pre-installed routines**, not a separate
always-on list. Concretely:

- At tame time, every species ability whose unlock level is 1 is installed,
  in declared order.
- Each later unlock installs on the level-up that reaches it, into the first
  free slot.
- If no slot is free at that moment the ability is logged and skipped,
  permanently. No shipped species can reach this state; it is mod-safety
  only, and failing loudly beats carrying a pending-installs list nothing
  ships to exercise.
- A species declaring no abilities implicitly installs
  `abilities::FALLBACK_ABILITY_ID` (`priority_boost`) at level 1. This keeps
  the eight fallback species behaving exactly as they do today, and keeps
  that ability obtainable by extraction — nothing else grants it.

`FALLBACK_ABILITY_ID` stops being an invisible backstop resolved inside
`companion_abilities` and becomes an ordinary pre-installed routine. It stays
mandatory at startup for the same reason as before.

**A member with no installed routines has no Special.** The command is
hidden, not greyed — app-core builds the battle command list, so the row is
dropped there. `party.rs:127`'s comment that "only the player can be empty"
becomes true of everyone and is updated.

### Routines as items

A loose (uninstalled) routine is a real `ItemDef` in the player's existing
`Inventory`, so it stores, stacks, and sells with no new machinery.

Those item defs are **synthesized from `AbilityDb` at load**, not
hand-written. `ItemDef` gains `routine: Option<AbilityId>`; the loader mints
one item per loaded ability:

- id: `routine_<ability_id>`
- name: `"<Ability Name> Routine"`
- description: the ability's own authored `description`, read at synthesis
  rather than copied

A modder drops one `.ron` into `assets/abilities/` and its routine item
exists automatically. There is no second file to forget, and no way for the
item's text to drift from the ability's.

### Extraction

Extraction consumes a program you own and yields exactly one routine, which
you pick. Everything else installed on it is lost with it.

It is gated on owning a bench, built anywhere — no proximity requirement —
checked with the existing `has_structure`. `StructureDef` gains
`#[serde(default)] extracts_routines: bool`, and
`assets/structures/compiler.ron` sets it. No new structure, no new build cost
to balance.

```rust
Game::can_extract_routines() -> bool
Game::extractable_routines(creature: Entity) -> Vec<AbilityDef>
Game::extract_routine(creature: Entity, index: usize) -> Result<(), String>
```

`extract_routine` refuses when: no bench is built, the creature isn't tamed
by the player, a battle is active, or the index is out of range. On success
the chosen routine's item lands in `Inventory` and the creature is despawned
through the same path `sell_companion` uses, so roster bookkeeping is
unchanged.

### Installing

Free and unrestricted outside battle, both directions:

```rust
Game::install_routine(entity: Entity, item: &ItemId) -> Result<(), String>
Game::uninstall_routine(entity: Entity, slot: usize) -> Result<(), String>
```

`install_routine` spends the item and fills the first free slot; it refuses
when the entity has no free slot, doesn't hold the item, or a battle is
active. `uninstall_routine` frees the slot and returns the item to inventory.

Uninstalling an innate species routine is allowed and permanent — it becomes
an ordinary item, and a Sentinel's `sandbox` can be plugged into a Scrapper.
That is the point of the feature.

### Research

Research stops conferring abilities directly. A node's `unlocks_abilities`
now grants the matching routine *items* into inventory the moment it is
researched — one-shot, carried by the save as any item is. Researching a
routine and installing it become two separate acts.

`Game::player_abilities()` — today derived from `Research` on every call —
is deleted; `actor_abilities` reads `Routines` for the player exactly as it
does for a companion.

### Decompile becomes an ability

New `AbilityEffect::Decompile` variant in the `.ron` schema, plus
`assets/abilities/decompile.ron` with `target: OneEnemyGroupFront`. It is
validated as mandatory at startup the same way `priority_boost` is, because
a new game pre-installs it into the player's slot 1.

`BattleAction::Decompile` and `ActionKind::Decompile`
(`crates/app-core/src/lib.rs:346`) are both deleted. Decompiling resolves as
a Special like everything else. `attempt_decompile`'s catalyst spend and
capture roll are untouched — only how it is reached changes.

Its two "refused before anything was spent" cases — roster full, no taming
catalyst — move into `Game::ability_unavailable`, so the row greys with the
reason instead of silently refunding the round. That removes the
`Option<bool>` return contract at `combat_rewards.rs:195`.

### Descriptions move into `.ron`

`ItemDef` and `StructureDef` each gain a `description`, `#[serde(default)]`
so existing mod files keep parsing.

- All 11 shipped item files get authored text. Nothing exists to drift from;
  this is pure addition, and the install/extract menus need it.
- All 13 shipped structure files get authored text.
  `Game::structure_description` and its two derivation tests
  (`tests/building.rs:364` and `:381`) are deleted.
- Routine items author nothing — their description is the ability's, read at
  synthesis.
- `assets/items/README.md` and `assets/structures/README.md` document the new
  field in the same change.

The tradeoff accepted here: authored structure text can contradict a
structure's numbers after a retune, which derivation could not. The
replacement guard is an assets test asserting every shipped item and
structure has non-empty description text — it catches an omission, not a
lie.

### UI

New app-core Modes, rendered by gui:

- **Routines panel** on the roster/party screen: one row per slot, filled or
  empty, showing the routine's name and description; install from inventory,
  uninstall back to it.
- **Extract dialog**: pick which of the program's routines to salvage, then
  confirm — it destroys the program and everything else installed on it. Same
  confirmation shape the sell-to-trader flow uses.
- **Battle**: the Special menu is unchanged in shape and hidden when the
  actor has no routines. Decompile appears inside it rather than as its own
  command row.

### Save format

`Routines` is serialized per entity and `SAVE_FORMAT_VERSION` is bumped. Old
saves are rejected with the existing clear message; there is no migration,
per the documented choice at `save.rs:168`.

## Testing

- Slot count at every level 1–12 (companion) and across 1–50 (player),
  asserted against the tuning constants rather than hardcoded twice.
- Install/uninstall round trip returns the same item; install refused with no
  free slot, without the item, and during battle.
- Extraction yields exactly the picked routine, despawns the program, loses
  the rest, and is refused with no bench built.
- A level-up that reaches a species unlock auto-installs it into a free slot.
- An actor with no routines offers no Special.
- Decompile greys with a reason on roster-full and on no-catalyst, and never
  refunds a round.
- Save round trip preserves every member's installed routines.
- Every shipped item and structure has non-empty description text.
- `cargo test -p feral-processes-engine balance_sim` after the change.

## Balance consequences

No shipped species is squeezed: the most any declares is 2 abilities, the
latest unlock is level 8, and 4 slots exist by then. Companion curves in
`balance_sim.rs` should not move. If they do, that is the signal, not a
broken test.

The player changes materially. One slot until level 10, and `decompile`
occupies it. Until you either pop decompile out or reach level 10, researched
routines sit in inventory unusable. This is the design working as specified,
and it is the thing to watch in play.
