# Player ability unlocks: research-granted routines

**Date:** 2026-07-26
**Branch:** `feat/player-ability-unlocks`
**Status:** Design, approved

## Problem

The player is the only combatant in the party who cannot run an ability. Slot
0 is a full member of the initiative order with Attack, Defend, Decompile and
Use Item, but the `[s]pecial` row is withheld from it explicitly —
`if !is_player` in `Game::battle_action_options`
(`crates/engine/src/game/combat.rs:537`). Every companion has a kit that
grows with its level; the player's kit is fixed from turn one to the end of
the run.

The abilities Phase 1 spec (`2026-07-25-abilities-design.md`) built the
system and deferred exactly this: "The player gains abilities in Phase 2, not
Phase 1."

## Scope

The player gains their own ability list, sourced from the research tree.
Researching a node grants its abilities permanently; the abilities themselves
stay `.ron` data in `assets/abilities/`.

Not in scope: teaching or installing abilities onto companions, ability
modules as craftable items, capacity limits on how many abilities anyone can
hold, and any new player-only ability content. See **Divergence from the
Phase 1 sketch** below — some of those were named in Phase 1's forward look
and are being dropped, not postponed by accident.

## Existing architecture this builds on

Verified by reading, not assumed:

- **The Special resolution path is already entity-generic.** Cooldowns
  (`AbilityCooldowns`), recipient expansion (`ability_recipients`), effect
  application (`use_ability`) and death reaping in
  `crates/engine/src/game/combat_round.rs:112-152` never ask whether the
  actor is a `Creature`. The only species-specific link in the chain is
  `Game::companion_abilities` reading `SpeciesDb`.
- **The Fatigue economy is already player-owned.** Both the availability gate
  (`combat.rs:448`) and the spend (`combat_round.rs:145`) charge
  `fatigue_cost` to `self.player_entity()`, not to the acting companion —
  commanding a program has always cost *your* Fatigue.
- **`Research` is already persisted.** `resources::Research` is a
  `HashSet<ResearchId>` written to and read from `save.rs:147`. Unlock state
  derived from it needs no save-format change.
- **Research listing is already deterministic.** `ResearchDb::all()` sorts by
  cost then id (`research.rs:123`), with a test asserting it
  (`all_is_ordered_by_cost_then_id`). `AbilityDb::all()` sorts by id.
- **Buffs replace rather than stack.** `Game::arm_buff`
  (`combat_status.rs:298`) overwrites the single `CombatBuff` slot, so a
  zero-cooldown buff can be refreshed but never compounded.
- **Asset load order already works.** `AbilityDb` loads at
  `lifecycle.rs:573`, `ResearchDb` at `:579`. Passing abilities into research
  validation needs no reordering.
- **`ResearchDef` already carries two unlock vectors**, `unlocks_structures`
  and `unlocks_recipes`, both consumed lazily by querying `ResearchDb`
  against `is_researched` (`catalog.rs:130`, `crafting.rs:34`) rather than by
  applying anything at purchase time.
- **`companion_abilities` never returns empty**, resolving
  `FALLBACK_ABILITY_ID` ("priority_boost") when a species declares nothing or
  everything is still level-gated.

## Design

### Schema

`ResearchDef` gains one field:

```rust
#[serde(default)]
pub unlocks_abilities: Vec<AbilityId>,
```

`#[serde(default)]` per the moddability rules in `CLAUDE.md`: every existing
research file, shipped or modded, keeps parsing untouched.

`ResearchDb::load_dir` takes an `&AbilityDb` alongside its existing
`&StructureDb`. Three call sites update (`lifecycle.rs:579` and two tests in
`research.rs`).

### Validation: retain the node, drop the id

`load_dir` currently drops an entire node that names an unknown structure,
because such a node is unreachable. An unknown *ability* id is treated
differently: the node is kept, the unknown id is dropped from
`unlocks_abilities`, and a warning is logged — the same
warn-and-retain shape `SpeciesDb::load_dir` uses for a species naming an
unknown ability (`species.rs:174`).

Rationale: a node can also unlock structures and recipes. Dropping the whole
node over one bad ability id would silently remove working content the
modder never touched. A node whose *only* unlock was a bad ability id
survives as a node that grants nothing, which is inert rather than
destructive.

### Derivation

```rust
/// Abilities the player has unlocked through research, in research order.
/// Unlike `companion_abilities` this may be empty — before any node is
/// researched the player has no routines at all, which is the point.
pub fn player_abilities(&self) -> Vec<AbilityDef>
```

Walks `ResearchDb::all()`, filters by `is_researched`, flat-maps
`unlocks_abilities` through `AbilityDb::get`, and dedupes on id preserving
first occurrence (two nodes may legitimately name the same ability).

No new component, no new resource, no new save field. The unlock set is a
pure function of `Research` and `ResearchDb`, exactly as structure and recipe
unlocks already are.

Order is stable across sessions because `ResearchDb::all()` is sorted; the
picker's numbering will not shuffle between saves.

### Dispatch

One new private method:

```rust
fn actor_abilities(&self, entity: Entity) -> Vec<AbilityDef>
```

Returns `player_abilities()` when `entity == self.player_entity()`, otherwise
`companion_abilities(entity)`. The four existing call sites —
`battle_special_options`, `companion_ability_label`, and
`combat_round.rs:114` and `:242` — switch to it and are otherwise unchanged.
They all already pass an `Entity`, so nothing threads a slot index around.

**The fallback must not leak onto the player.** `companion_abilities` keeps
resolving `priority_boost` for companions; `actor_abilities` does not apply
it on the player branch. A player who has researched nothing has nothing —
otherwise the first node sells something already owned.

### Battle menu

The `if !is_player` guard at `combat.rs:537` is deleted. The Special row is
built for every slot. For the player with an empty ability list it carries
`unavailable: Some("no routines installed".into())`, using the greyed-row
mechanism `ActionOption` already has for Decompile with no catalyst.

Deliberate: a hidden row teaches nobody the feature exists; a greyed row with
a reason points at the research tree.

**To verify during implementation, before relying on it:** that app-core
refuses to plan an action whose `unavailable` is `Some`. If it does not, the
greyed row is cosmetic and an empty-list Special reaches
`combat_round.rs:118`, where `chosen` is `None` and the round is silently
spent. If that gap exists, the fix is a guard at the plan site, not a
different menu shape.

## Content

A three-node branch, rooted with no prerequisite so the greyed Special row
resolves early rather than sitting unexplained. Costs are calibrated against
the existing tree (roots 8-12, mid 18-25, tips 40-45):

| Node | Cost | Requires | Grants |
|---|---|---|---|
| `self_exec` "Self-Execution" | 12 | — | `priority_boost` |
| `runtime_patching` "Runtime Patching" | 28 | `self_exec` | `hot_patch` |
| `kernel_privileges` "Kernel Privileges" | 48 | `runtime_patching` | `null_route` |

- `priority_boost` (+3 ATK on one ally, cooldown 0, fatigue 0) is the
  humblest thing in the pool and non-stacking, so its only real price is the
  round spent on it instead of attacking. It is also the routine every
  companion falls back to, which reads well as the first thing you learn to
  run yourself.
- `hot_patch` (heal 8, cooldown 1) is the first genuinely kit-changing
  unlock: the player currently cannot heal without spending an item.
- `null_route` (stun every hostile for a round, cooldown 5, **fatigue 15**)
  is the marquee unlock and the first that costs Fatigue at all.

The ten shipped abilities are reused rather than authoring player-only ones.
They are already data and already balanced against each other; a
player-exclusive set is separate content work this feature does not need.
Modders get player abilities for free either way — any `.ron` in
`assets/abilities/` becomes player-grantable the moment a research node names
it.

**Known balance property, stated rather than fixed:** the Fatigue budget only
tightens at the top tier. `priority_boost` and `hot_patch` both omit
`fatigue_cost` and so default to 0.0; the first two nodes are gated purely by
cooldown and by the round they consume. If the tension should arrive earlier,
the lever is authoring `fatigue_cost` onto those abilities — which would also
change them for the companions that already have them. Not done here.

Like every balance number in this repo, these costs are arithmetic-plausible
and unplayed.

## Documentation

- `assets/research/README.md` gains the `unlocks_abilities` field, per the
  rule that schema docs update in the same change.
- `assets/abilities/README.md` currently says an ability is "what a companion
  spends its round on" and that "which abilities a companion has comes from
  its species file". Both become wrong; they gain the research path.
- `docs/manual.md:138` describes `s` as "Special (**party members only**)"
  and its detail line as "the species' own abilities" — the exact claim this
  falsifies. It needs the player's research-granted case.
- `README.md:83` says "You can Attack, Defend, spend a Special, Decompile
  ..." in a sentence addressed to the player, which is wrong *today* and
  becomes true only once this ships. No edit needed, but it is worth knowing
  the README was already ahead of the code here.

## Testing

Engine unit tests, deterministic, no wall-clock or unseeded RNG.

Schema:
- a node omitting `unlocks_abilities` parses
- a node naming an unknown ability id is retained with that id dropped, and
  warns
- a node whose only unlock is a bad ability id survives with its structures
  and recipes intact

Derivation:
- `player_abilities()` is empty on a fresh game
- returns exactly the granted set after the node is researched
- is ordered by cost-then-id across multiple nodes
- dedupes an ability named by two nodes

Menu:
- the player's Special row is present and `unavailable` at zero abilities
- it becomes available once a node is researched
- companion rows are unaffected

Resolution:
- a player Special applies its effect, arms the cooldown on the *player*
  entity, and deducts `fatigue_cost` exactly once
- a companion whose species declares nothing still resolves `priority_boost`,
  and that fallback does not leak onto the player

Persistence:
- a save/load round-trip leaves researched abilities intact, proving the
  no-new-save-field claim rather than asserting it

Anything involving more than one party slot is tested in the engine, not
app-core: app-core battles are always one group and one slot.

`cargo test --workspace` is the gate, not the subset touched here.

## Divergence from the Phase 1 sketch

`2026-07-25-abilities-design.md` sketched Phase 2 as "routine slots on the
player, one install slot on companions, ability modules as compilable items,
research nodes unlocking their recipes, and a perk that widens slot capacity."

This design keeps the research gate and drops the rest:

- **Ability modules as items, unlocked as recipes.** Dropped. Research would
  gate a recipe, which crafts a module, which is installed into a slot, to
  arrive at an ability — three layers of indirection to reach a permanent
  unlock. Research granting the ability directly is one layer and reuses the
  exact pattern `unlocks_structures` already establishes.
- **Routine slots and a slot-capacity perk.** Dropped. A cap is only
  interesting when there is more to hold than room to hold it; with ten
  shipped abilities and three granting them, the cap would be a menu that
  never binds. Adding it later costs a component and a save field, and can be
  added when the ability pool is large enough to make choosing hurt.
- **A companion install slot.** Dropped as a design decision, not an
  oversight: this feature is about the player's kit, and breaking the
  species-fixed ability list is a separate change with its own consequences
  for fusion, taming value and species identity.

Phase 1's rejected list already ruled out **perks unlocking individual
abilities**, on the grounds that `Perk` is a Rust enum by design and routing
content through it would drag abilities back out of data. That reasoning is
what points at the research tree here.

## Out of scope

- Player-only ability content
- Any change to how companions acquire abilities
- Rebalancing the ten shipped abilities' costs or cooldowns
- A UI for browsing unlocked abilities outside of battle
