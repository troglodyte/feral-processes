# Abilities: data-driven multi-target combat actions

**Date:** 2026-07-25
**Branch:** `abilities`
**Status:** Phase 1 design, approved

## Problem

Companions have exactly one interesting battle action — Special — and it can
only ever do four things, all of them single-target: buff one ally's ATK,
buff one ally's DEF, heal one ally, or inflict one status on one enemy. The
set is a Rust enum (`species::SpecialAbility`), so it cannot be extended by
dropping in a file. The player has no Special at all. Nothing about a
companion's kit changes over its life; a species' abilities are fixed at
authoring time and never unlock.

We want the party-support fantasy other games get from clerics and mages —
sweep several enemies at once, cripple a whole pack, patch the entire party —
expressed in this game's science-fiction register, and reachable through
progression rather than handed out at spawn.

## Scope

This is Phase 1 of two.

**Phase 1 (this spec)** builds the abilities *system*: abilities become
`.ron` data behind an `AbilityDb`, the new targeting shapes land, back-rank
enemies become damageable, cooldowns and Fatigue costs work, and species files
gain level-gated ability lists. It requires **no save-format change** and is
playable on its own — companions get real multi-target kits from their
species files, tunable by editing data.

**Phase 2 (separate spec)** builds the unlock layer: routine slots on the
player, one install slot on companions, ability modules as compilable items,
research nodes unlocking their recipes, and a perk that widens slot capacity.
The player gains abilities in Phase 2, not Phase 1.

## Existing architecture this builds on

Verified by reading, not assumed:

- `species::SpecialAbility` is a Rust enum with four variants. **No shipped
  species `.ron` declares `special_abilities`** — every companion in the game
  currently falls back to the generic rally in `Game::companion_abilities`.
  There is therefore no migration burden for shipped content.
- The renderer never sees `SpecialAbility`. app-core touches only
  `TargetSpec::SpecialAbility` (a menu-shape enum) and draws `SpecialOption` /
  `AllyOption` rows verbatim. The engine can restructure the ability model
  without the GUI knowing.
- Enemies are up to `MAX_ENEMY_GROUPS` (4) groups; only `members[0]` of each
  group can be targeted or damaged today.
- `ENGAGED_GROUPS` (2) constrains what **enemies** can do — a back group may
  only use ranged moves. It does not constrain the party;
  `party_member_attacks` already strikes any group. AoE therefore needs no
  reach special case.
- Companions cap at `CREATURE_MAX_LEVEL` (12) and carry `Experience { level }`.
- `CombatBuff` and `StatusEffects` are battle-scoped: armed in a fight, ticked
  in `tick_round_status_effects`, cleared in `clear_battle_status_effects`,
  never persisted.
- Reference damage numbers: `PLAYER_STRIKE_POWER` is 5, species move powers
  run 7–9, `DEFEND_DEF_BONUS` is 6, `COMPANION_COMMAND_FATIGUE_COST` is 5.0.

## Design

### 1. Abilities become data

New module `crates/engine/src/abilities.rs` and asset directory
`assets/abilities/`, following the `ItemDb` / `SpeciesDb` / `ResearchDb`
pattern: `AbilityId` is a string type alias like `SpeciesId`, and a malformed
`.ron` file is skipped with a logged warning rather than panicking at startup.

```rust
pub type AbilityId = String;

pub struct AbilityDef {
    pub id: AbilityId,
    pub name: String,             // "Cascade Overflow"
    pub description: String,
    pub target: AbilityTarget,
    pub effect: AbilityEffect,
    #[serde(default)] pub cooldown: u32,
    #[serde(default = "default_fatigue_cost")] pub fatigue_cost: f32,
}

pub enum AbilityEffect {
    Damage { power: i32, #[serde(default)] status: Option<MoveEffect> },
    Heal   { power: i32 },
    Buff   { kind: BuffKind,   power: i32, duration: u32 },
    Debuff { kind: StatusKind, power: i32, duration: u32 },
}
```

`Damage` reuses the existing `species::MoveEffect` rider, so an attack can
also inflict Bleed exactly as `MoveDef` already can. `Buff { kind: Atk }` and
`Buff { kind: Def }` collapse today's separate `Rally` and `Shield` variants
into one data-driven effect. The `SpecialAbility` enum is deleted.

**Powers are flat numbers, not stat-scaled.** `Damage` runs through
`battle::compute_damage(atk, def, power)`, so it already scales with the
user's ATK the same way a move does. Because that formula is additive
(`power + atk - def`), a flat +3 ATK buff is worth exactly 3 damage per hit at
every level — so the `atk / 3` scaling on today's fallback rally buys nothing
and goes away.

`AbilityDb` is loaded in `Game::new` alongside the other databases and lives
as a `Resource`, same as `SpeciesDb`.

### 2. Targeting shapes

```rust
pub enum AbilityTarget {
    OneAlly,             // today's Rally / Shield / Heal
    WholeParty,          // NEW
    OneEnemyGroupFront,  // today's Debuff
    WholeEnemyGroup,     // NEW — every member of one group
    AllEnemies,          // NEW — every living enemy on the field
}
```

`battle::SpecialTarget` gains `WholeParty` and `AllEnemies` variants.
`species::SpecialTargeting` gains a `None` case meaning "resolve immediately,
no second picker". That is the entire UI-contract change: app-core's existing
two-step flow either opens a picker or skips straight to the action.

Resolution happens at round-resolve time, not plan time — matching what
`resolve_one_action` already does, so a group that dies before the acting
member's turn retargets and an ally knocked out in the meantime is skipped
rather than being healed as a corpse.

- `WholeParty` collects every living party slot at resolve time. Downed
  members are skipped.
- `AllEnemies` uses the existing `Game::all_living_enemies`, which already
  walks every member of every group and filters on alive.
- `WholeEnemyGroup` collects every living member of the chosen group,
  retargeting to a surviving group if the planned one is already gone.

### 3. Back-rank enemies become killable

This is the one genuinely new engine mechanic, and it is what both new
enemy-side AoE shapes depend on.

Today `finish_group_member(group, player)` and `pop_group_member(group)` are
hardcoded to index 0, and `reap_dead_fronts` only inspects each group's front.
A back-rank member reduced to 0 HP would sit in the group as a corpse, be
promoted to front later, and then be attacked as though alive.

The fix is a generalization, not a new subsystem:

- `pop_group_member(group)` → `remove_member(group, index)`.
- `finish_group_member(group, player)` → `finish_member(group, index, player)`,
  awarding loot and XP through the same path a direct kill already uses, and
  removing the group entirely when it empties.
- Existing front-only call sites pass `index = 0`.
- `reap_dead_fronts` → `reap_dead_members`, walking every index back-to-front
  so a removal cannot shift a later index out from under the loop.

The "Another rogue program from the pack engages!" log line stays front-only —
it describes a promotion, which is meaningless for a back-rank kill.

### 4. Cooldowns and Fatigue costs

A new `AbilityCooldowns` component holding remaining rounds per `AbilityId`,
with **the exact lifecycle `CombatBuff` already has**: armed during a fight,
ticked down in `tick_round_status_effects`, cleared in
`clear_battle_status_effects` when the battle ends. Battle-scoped means never
persisted, which is why Phase 1 needs no save-format change.

Per-ability `fatigue_cost` replaces the flat `COMPANION_COMMAND_FATIGUE_COST`,
which remains as the serde default so an ability that declares no cost behaves
exactly as commanding a companion does today. As today, the cost comes off the
**player's** `Needs.fatigue` — displayed as "Fatigue" — even when a companion
is the one acting, since it models the player issuing the command. Note this is
a different resource from `Needs.hunger`, which the UI labels "Power".

`battle::SpecialOption` gains `unavailable: Option<String>`, matching the field
`ActionOption` already carries, so an ability on cooldown or unaffordable
renders greyed with its reason. `battle_set_action` refuses such an action
rather than silently burning the member's round — the same fail-fast validation
it already applies to out-of-range slots and ability indices.

### 5. The shipped ability set

Ten files in `assets/abilities/`. The first five reproduce today's behavior as
pure data; the last five are the new content.

| id | name | target | effect | cooldown | fatigue_cost |
|---|---|---|---|---|---|
| `priority_boost` | Priority Boost | OneAlly | Buff Atk +3, 3 rounds | 0 | 5 |
| `sandbox` | Sandbox | OneAlly | Buff Def +3, 3 rounds | 0 | 5 |
| `hot_patch` | Hot Patch | OneAlly | Heal 8 | 1 | 5 |
| `memory_leak` | Memory Leak | OneEnemyGroupFront | Debuff Bleed 2, 3 rounds | 1 | 5 |
| `deadlock` | Deadlock | OneEnemyGroupFront | Debuff Stun, 1 round | 2 | 5 |
| `cascade_overflow` | Cascade Overflow | WholeEnemyGroup | Damage 6 | 2 | 8 |
| `broadcast_storm` | Broadcast Storm | AllEnemies | Damage 4 | 4 | 15 |
| `null_route` | Null Route | AllEnemies | Debuff Stun, 1 round | 5 | 15 |
| `redundancy_sync` | Redundancy Sync | WholeParty | Heal 10 | 3 | 12 |
| `overclock_array` | Overclock Array | WholeParty | Buff Atk +3, 3 rounds | 3 | 10 |

Naming is networking and operating-system vocabulary — no magic, no religion —
sitting in the same register as the existing Firewall Plating, Cortex Hack and
Decompile language.

**These numbers are unplayed.** They are arithmetic-plausible starting points:
`broadcast_storm` at power 4 is deliberately weak per-hit against
`PLAYER_STRIKE_POWER` 5 and species moves at 7–9, because it can land on up to
twelve targets. Being `.ron`, they are tunable without a rebuild.

### 6. Species wiring

```rust
pub struct SpeciesAbility {
    pub id: AbilityId,
    #[serde(default = "default_learn_level")] pub level: u32,   // 1
}
```

`SpeciesDef.special_abilities: Vec<SpecialAbility>` becomes
`abilities: Vec<SpeciesAbility>`, still `#[serde(default)]` so a species file
that declares none keeps parsing.

`SpeciesDef.legacy_special_ability` is **deleted**. It exists solely to migrate
a format no shipped file uses, and the type it names is being removed; keeping
it would be exactly the backwards-compat cruft CLAUDE.md rules out.

`Game::companion_abilities` filters the species list to entries whose `level`
is at or below the companion's `Experience.level`. A species declaring nothing
falls back to `priority_boost` — a real shipped id rather than a synthesized
value, so the fallback stops being a special case in the resolver.

`SpeciesDb` validates ability ids against `AbilityDb` at load time, dropping
unknown entries with a warning, the way `ResearchDb` drops nodes with unknown
prerequisites. This requires `AbilityDb` to load before `SpeciesDb`.

Kits follow species character. Sentinel, the tank, gets `sandbox` early and
`redundancy_sync` late; Cipher leans on debuffs; the bosses Overseer and
Wintermute are where `broadcast_storm` lives.

### 7. Testing

New `crates/engine/src/tests/combat_abilities.rs`, matching the themed test
split already in that directory.

Loader tests mirror `research.rs`'s: a valid def loads with defaults applied, a
malformed file is skipped with a warning while the rest still load, and the
shipped set loads clean with the expected count.

Behavior tests:

- `WholeEnemyGroup` damage lands on every member, not just the front.
- A back-rank member killed by AoE leaves the group and awards its XP and loot.
- AoE emptying every group ends the battle in a win.
- `WholeParty` heal raises every living member and skips downed ones.
- A cooldown blocks a second use and expires after its declared rounds.
- Cooldowns are cleared when the battle ends.
- `fatigue_cost` is deducted from the player, and an ability is unavailable
  when Fatigue is short.
- A species ability above the companion's level is not offered.

**These must live in the engine crate.** app-core battles are always one group
and one slot, so multi-group AoE is untestable there. Seeded RNG, no sleeps, no
wall-clock dependence — background systems can and will interfere with a naive
assertion.

`cargo test --workspace` is the gate before this is called done.

### 8. Documentation

- New `assets/abilities/README.md`, the schema reference for the new asset
  type, matching the species / structures / items READMEs in depth and form.
- `assets/species/README.md` updated for `special_abilities` becoming
  `abilities` and for the level-gating field.
- Root `README.md` and `CHANGELOG.md` swept for claims this change falsifies.
- CLAUDE.md's moddability list should gain abilities as a fourth data-driven
  content type — but CLAUDE.md is gitignored, so that edit does not ship with
  this branch.

## Out of scope

Deferred to Phase 2: player routine slots, the companion install slot, ability
modules as items, research wiring, the slot-capacity perk, and the save-format
changes those require.

Considered and rejected for Phase 1:

- **A `OneEnemyGroupFront`-per-group "sweep the fronts" shape.** Free to add
  once the targeting enum exists, but not asked for; leaving it out until
  there is a reason.
- **Giving companions full equipment.** Reverses a documented decision that
  companions carry no gear, and touches loot, stats, fusion, trade and saves.
- **Perks unlocking individual abilities.** `Perk` is a Rust enum by design;
  routing content through it would drag abilities back out of data.
