# Wild-carried routines, hostile specials, and level-scaled abilities

**Date:** 2026-07-28
**Status:** approved, not implemented

## Problem

Abilities are reachable two ways: a species grants them, or a research node
does. Both are deterministic. Once you know Cipher grants Memory Leak, there
is nothing to find — you go and tame a Cipher. There is no reason to look at
any *individual* wild program rather than at its species.

Second problem, independent of the first: `Heal(power: 8)` and
`Buff(power: 3)` are flat numbers. At level 20, against a program with 400
Integrity taking 100-point hits, an 8-point patch is noise. The abilities
README currently argues buffs are fine flat, because "a flat +3 ATK is worth
exactly 3 extra damage per hit at every level" — true in absolute terms and
irrelevant in relative ones.

## Solution

A wild program can spawn carrying a routine its species never grants. It
**uses that routine against you in battle**, which is how you find out it has
one. Decompile it and the routine comes with it, installed. Twenty new
abilities exist that are reachable no other way.

Separately, ability magnitudes scale with the user's level.

## Decisions taken

Recorded because each closed off a cheaper option:

- **Discovery is by being hit with it.** Not a roster marker, not a
  capture-time surprise. Hostiles have never spent a round on an ability, so
  this is the largest single piece of new engine work here.
- **Full mirror.** "Ally" means the user's own side and "enemy" means the
  other side, whichever side the user is on. A hostile's Redundancy Sync
  heals its own group. Rejected: offence-only, which halves the enemy-usable
  set and makes wild carriers strictly a damage problem.
- **A carrier fires whenever the routine is off cooldown.** No per-round
  roll, no situational policy. Safe only because every ability now carries a
  cooldown.
- **Every ability gets a cooldown ≥ 1, except `decompile`.** `decompile`
  stays at 0 deliberately: it is the player's capture mechanism, pre-installed
  on a new game, and a cooldown on a failed capture roll would change the core
  loop. Hostiles never use `decompile`, so the reason for the rule does not
  apply to it.
- **Rarity is authored per ability, in a new `wild_weight` field.** Rejected:
  a uniform pick over every loaded ability (a zone-1 Sprite handing you
  Broadcast Storm), and deriving rarity from `fatigue_cost` (welds two
  unrelated knobs together, so retuning a cost silently retunes drop rates).
- **Capture is the only way to get one.** A killed carrier's routine is
  destroyed. This gives Decompile a second reason to exist beyond filling the
  roster.
- **All twenty are wild-only.** No species file and no research node
  references any of them. This is the whole point: hunting, not targeting a
  species.
- **Magnitude scales by the user's level**, not by the target's stats and not
  by zone. Wild programs have no level, so a hostile carrier substitutes the
  current `ZoneLevel` — the closest thing it has, and already what its stats
  scale from.
- **Slot overflow at capture goes to cargo**, rather than being destroyed as
  it is today. See §2.

## 1. The roll

### Schema

`AbilityDef` gains one field:

```rust
/// How likely this ability is to be found installed on a wild program.
/// `#[serde(default)]` to 0 — an ability that doesn't opt in never spawns
/// wild, which is what keeps `priority_boost` and `decompile` (and every
/// existing shipped ability) out of the pool without naming them here.
#[serde(default)]
pub wild_weight: u32,
```

Weights are relative within the pool, not probabilities: an ability at weight
12 is twice as likely as one at 6, and the pool is normalised at pick time.
`wild_weight: 0` is exclusion, and it is the default, so a mod opts in rather
than out.

### Spawn

`Game::spawn_wild_creature` rolls `WILD_ROUTINE_CHANCE` once per creature. On
a hit it picks weighted from `AbilityDb` and inserts `Routines(vec![id])`; on
a miss it inserts `Routines::default()`.

Every wild program routes through this one function — nest guardians, pack
members, bosses, random-encounter spawns — so all of them can carry.

`AbilityDb` gains:

```rust
/// Every ability with a non-zero `wild_weight`, ordered by id, paired with
/// its weight. Ordered because `HashMap` iteration is randomised per
/// instance and a weighted pick walking an unordered pool would not be
/// reproducible from a seed — the same failure `all()` already avoids for
/// the picker's numbering.
pub fn wild_pool(&self) -> Vec<(&AbilityDef, u32)>
```

The pick itself is a small pure function over `(weights, roll)` so it can be
tested without a `Game`.

### Persistence

**No save-format change.** `CreatureSave.routines` already exists, the save
query already reads `Option<&Routines>` for every creature regardless of
`tamed`, and load already inserts `Routines` before the `if c.tamed` branch.
A wild carrier round-trips today. `SAVE_FORMAT_VERSION` is not bumped.

## 2. Capture

`install_innate_routines` currently *overwrites* `Routines` with the species
kit. It becomes a merge:

1. Whatever the program was already carrying keeps its place, first.
2. The species kit for its level fills the remaining slots, skipping
   duplicates.
3. `FALLBACK_ABILITY_ID` is added only if the result would otherwise be
   empty — a carrier is never given the fallback, because it already has
   something real.

A freshly tamed program has one slot (`companion_routine_slots(1) == 1`) and
six shipped species grant an ability at level 1, so a carrier of one of those
species overflows immediately. Today overflow is logged and destroyed.

**Change: overflow is minted as its `routine_<id>` item into the player's
cargo instead.** The routine item already exists for every loaded ability
(`ItemDb::synthesize_routines`), so this is a mint-and-deposit, not a new
concept. It turns the collision from a punishment into a swap decision — the
player can pop one out and install the other whenever they like.

This changes existing behaviour in both `install_innate_routines` and
`install_unlocked_routines`, whose "no free routine slot — the unlock is
lost" log becomes "went to cargo instead". Both log lines change; the
eviction-of-fallback path in `install_unlocked_routines` is untouched, since
that destroys a placeholder rather than a real routine.

`install_unlocked_routines` gains one guard: a carried routine must never be
evicted to make room for a species unlock. Today only `FALLBACK_ABILITY_ID`
is evictable, and a carried routine is not the fallback, so this holds
already — it gets a test rather than new code.

## 3. Hostiles spending a round

### Choosing

In `wild_retaliate`, before the move roll: if the creature holds a routine
that is not cooling, it runs that routine instead of a move. First installed
routine wins — wild carriers hold exactly one, so ordering is not a real
decision, and inventing a priority scheme for a one-element list would be
speculative.

A ranged/engaged check does not apply. `wild_retaliate` gates *moves* by
`ENGAGED_GROUPS` because a back-rank program has to reach; a routine is
executed, not swung, and gating it would silently disable carriers in the
back groups.

Fatigue is not charged. `fatigue_cost` models the player issuing a command;
a wild program commands itself.

### Cooldowns

Cooldown is the only brake. `cooldown` is `#[serde(default)]` 0, so a mod
ability declaring none would fire every single enemy round. The enemy side
therefore arms `max(ability.cooldown, ENEMY_ROUTINE_MIN_COOLDOWN)`. The
player side keeps the authored value untouched — that is what leaves
`decompile` spammable.

Two player-only paths widen, both of which become live bugs the moment a
hostile holds a buff:

- `tick_ability_cooldowns` is called for the player and party members only.
  Hostile carriers must tick too, or a fired routine never comes back.
- `clear_battle_status_effects(player, wild: Option<Entity>)` clears exactly
  one hostile's `StatusEffects` and no hostile's `CombatBuff`. With mirrored
  buffs that leaves a permanent free stat on every surviving hostile — they
  persist on the map after a jack-out. It must clear `StatusEffects`,
  `CombatBuff` and `AbilityCooldowns` for every hostile that was in the
  battle, not just the one passed in.

### Logging

A hostile routine logs at `MessageKind::EnemySpecial` — the kind that already
exists for a wild move that reached for its status effect. It names the
routine, so "Wraith runs Fork Bomb" is how the player learns what the carrier
has.

## 4. Full-mirror targeting

`ability_recipients` resolves every target from the player's side. It becomes
side-aware. For a hostile actor:

| Authored target | Hostile actor resolves to |
|---|---|
| `OneAlly` | one living hostile, seeded uniform pick across all groups |
| `WholeParty` | every living hostile in every group |
| `OneEnemyGroupFront` | `roll_enemy_target` — aggro-weighted, so slot order and bracing still matter |
| `WholeEnemyGroup` | every living party member |
| `AllEnemies` | every living party member |

The last two collapse to the same set. That is inherent, not a shortcut: the
player has one party where the hostiles have groups, and there is no
player-side subdivision for `WholeEnemyGroup` to select. An ability declaring
either reads identically when a hostile uses it.

`OneAlly` uses a uniform pick rather than "the most hurt ally". A carrier
fires whenever it can, so a heal landing on a full-health ally is wasted —
accepted, because the alternative is a per-effect situational policy that was
explicitly rejected.

`Decompile` is excluded from the enemy path entirely: a hostile holding it
falls through to a normal move. `wild_weight` on `decompile.ron` is 0, so
this is only reachable by a mod installing it deliberately.

## 5. Level scaling

A pure function in `abilities.rs`:

```rust
/// The multiplier an ability's authored magnitude is scaled by when a
/// combatant of `level` uses it. Level is clamped at
/// `ABILITY_POWER_SCALE_LEVEL_CAP` because the player has no level cap —
/// see `player_routine_slots`.
pub fn ability_power_scale(level: u32) -> f32

/// `power` scaled by `ability_power_scale(level)`, rounded. Negative powers
/// scale too: a sap gets stronger with level the same way a buff does.
pub fn scaled_power(power: i32, level: u32) -> i32
```

**Applied at the moment the effect is armed**, not at tick time. A `Heal`
restores the scaled figure; a `Buff` stores the scaled power in
`ActiveBuff`; a `Debuff` stores the scaled per-round bleed in
`ActiveStatus`. The status and buff ticks then need no change at all — they
read a value that is already correct.

**Not applied to `Damage` power.** `battle::compute_damage` is
`power + ATK − DEF`, so ability damage already rides the user's ATK. Scaling
the flat term as well would double-dip, and every `balance_sim` curve reads
`compute_damage`.

`duration` never scales.

### Whose level

- Player: `Experience.level`, uncapped, so the clamp is load-bearing.
- Companion: `Experience.level`, capped at 12 by progression.
- Hostile: the current `ZoneLevel`. Wild programs have no `Experience` —
  they scale by zone and distance instead — so zone is the closest analogue
  and keeps a carrier's routine in step with the fight it appears in.

One shared helper resolves this (`Game::ability_user_level(entity)`) so the
three cases cannot drift into three formulas.

### Documentation debt

`assets/abilities/README.md` currently states that buff powers do not need to
scale, with a worked justification. That paragraph is now wrong and gets
rewritten to describe `power` as an authored baseline that level multiplies.

## 6. New effect kinds

Two new `AbilityEffect` variants, plus one mechanic that needs no variant.

### `Drain { power, heal_fraction }`

Damage through `compute_damage`, then the user is healed for
`heal_fraction` of the damage actually dealt, capped at its maximum
Integrity. Self-scaling — the heal rides damage, which rides ATK — so it is
deliberately excluded from `scaled_power`.

`heal_fraction` is `f32` and must be finite; it joins `non_finite_field`'s
checks. It is clamped to `0.0..=1.0` at load rather than at use, so a
`heal_fraction: 5.0` mod is a bounded ability and not a bounded surprise
inside a formula.

### `Cleanse`

Clears each recipient's active `StatusEffects`. No fields. Silent on a
recipient that had nothing — a "nothing to clear" line per party member every
time would drown the log.

### Sap needs no variant

`Buff(kind: Atk, power: -4, duration: 3)` aimed with
`target: WholeEnemyGroup` already *is* a sap: `effective_atk` and
`effective_def` add the buff bonus unconditionally, so a negative power
subtracts today. Adding a `Sap` variant would be a second spelling of an
existing one.

One real caveat, asserted in a test rather than special-cased:
`CombatBuff` holds a single `active` slot, and `is_defending` identifies the
Defend stance by an exact `BuffKind::Def` + `DEFEND_DEF_BONUS` power match.
So a sap landing on a bracing member overwrites its stance and it stops
counting as defending. That is the documented cost of the single-slot design
("A single `active` slot, so this overwrites whatever the target was still
carrying — a real cost of the choice"), now reachable from a new direction.

## 7. The twenty

All wild-only: no species file and no research node references any of them.
All cooldown ≥ 1. Powers are authored baselines that §5 multiplies.

| id | Name | Target | Effect | cd | fatigue | weight |
|---|---|---|---|---|---|---|
| `kernel_panic` | Kernel Panic | OneEnemyGroupFront | Damage 16 | 3 | 10 | 10 |
| `stack_smash` | Stack Smash | OneEnemyGroupFront | Damage 9 + Bleed 3 (0.6, 3r) | 2 | 8 | 12 |
| `pipeline_stall` | Pipeline Stall | OneEnemyGroupFront | Damage 7 + Stun (0.4, 1r) | 3 | 9 | 8 |
| `fork_bomb` | Fork Bomb | WholeEnemyGroup | Damage 7 + Bleed 2 (0.35, 2r) | 3 | 12 | 6 |
| `packet_shred` | Packet Shred | WholeEnemyGroup | Damage 10 | 3 | 11 | 8 |
| `bus_fault` | Bus Fault | AllEnemies | Damage 6 + Stun (0.25, 1r) | 5 | 18 | 3 |
| `hard_lock` | Hard Lock | OneEnemyGroupFront | Stun 2r | 4 | 10 | 6 |
| `heap_corruption` | Heap Corruption | WholeEnemyGroup | Bleed 3, 3r | 3 | 11 | 7 |
| `race_condition` | Race Condition | WholeEnemyGroup | Stun 1r | 4 | 13 | 5 |
| `bit_rot` | Bit Rot | AllEnemies | Bleed 2, 4r | 5 | 16 | 4 |
| `hyperthread` | Hyperthread | OneAlly | Buff Atk +6, 4r | 3 | 8 | 9 |
| `bastion` | Bastion | WholeParty | Buff Def +4, 3r | 3 | 11 | 8 |
| `throttle` | Throttle | WholeEnemyGroup | Buff Atk −4, 3r | 3 | 10 | 7 |
| `etch` | Etch | WholeEnemyGroup | Buff Def −4, 3r | 3 | 10 | 7 |
| `checksum_repair` | Checksum Repair | OneAlly | Heal 18 | 3 | 9 | 9 |
| `mirror_restore` | Mirror Restore | WholeParty | Heal 8 | 2 | 10 | 8 |
| `cold_boot` | Cold Boot | OneAlly | Heal 30 | 5 | 15 | 4 |
| `siphon_cycles` | Siphon Cycles | OneEnemyGroupFront | Drain 10, heal 0.5 | 2 | 9 | 9 |
| `leech_array` | Leech Array | WholeEnemyGroup | Drain 6, heal 0.3 | 4 | 13 | 5 |
| `flush_cache` | Flush Cache | WholeParty | Cleanse | 3 | 7 | 8 |

Rarity reads off the weight column: `bus_fault` at 3 against
`stack_smash` at 12 is four times rarer. Weights total 143.

Two existing files change: `priority_boost.ron` and `sandbox.ron` gain
`cooldown: 1` (both currently default to 0). Every other shipped ability
already has one, and none of the eleven gains a `wild_weight`.

## 8. Tuning constants

All in `crates/engine/src/tuning.rs`, in a new labelled section. None of
these values has been playtested.

| Constant | Value | Meaning |
|---|---|---|
| `WILD_ROUTINE_CHANCE` | `0.06` | Chance a wild spawn carries a routine at all |
| `ENEMY_ROUTINE_MIN_COOLDOWN` | `1` | Floor on a hostile's armed cooldown, so a mod's cooldown-0 ability cannot fire every round |
| `ABILITY_POWER_SCALE_PER_LEVEL` | `0.15` | Magnitude multiplier is `1 + level × this` |
| `ABILITY_POWER_SCALE_LEVEL_CAP` | `40` | Level clamp, because the player has no level cap |

At 0.15, a level-12 companion runs a routine at 2.8×, a level-20 player at
4×, and the clamp caps everyone at 7×. So `cold_boot` restores 30 at level 1
and 120 at level 20, against the 400-Integrity, 100-damage-per-hit case that
motivated the change.

**Name collision to avoid:** `WILD_ABILITY_CHANCE` already exists and gates
whether a wild *move* reaches for its status effect. It is unrelated.
`WILD_ROUTINE_CHANCE` is deliberately a different word.

## 9. Testing

Unit tests in the engine, per the repo's usual split.

- **Load:** `wild_weight` defaults to 0; a file omitting it still parses; a
  non-finite `heal_fraction` skips the file with a warning; a
  `heal_fraction` outside 0–1 clamps; the shipped set is 31 abilities, all
  clean.
- **Pool:** `wild_pool` excludes weight-0 abilities, is ordered by id, and
  the weighted pick is reproducible from a seed and proportional to weight.
- **Spawn:** with the chance forced, a wild creature carries exactly one
  routine drawn from the pool; with it forced off, `Routines` is empty. Same
  seed, same carrier.
- **Persistence:** a wild carrier survives a save/load round trip with its
  routine intact, on the existing `SAVE_FORMAT_VERSION`.
- **Capture:** a carrier's routine survives `install_innate_routines`; a
  level-1 carrier of a species granting a level-1 ability keeps the carried
  one and finds the species ability in cargo; a later `install_unlocked_routines`
  never evicts a carried routine.
- **Hostile use:** a carrier with a damaging routine off cooldown uses it
  instead of a move; the cooldown arms, ticks down over rounds, and it fires
  again; a mod ability at cooldown 0 still cannot fire two rounds running.
- **Mirroring:** each of the five targets resolves to the table in §4 for a
  hostile actor; a hostile `WholeParty` heal touches every hostile and no
  party member; a hostile `AllEnemies` damage touches every party member and
  no hostile.
- **Cleanup:** after a jack-out, no surviving hostile carries a
  `CombatBuff`, `StatusEffects` or `AbilityCooldowns`.
- **Scaling:** `ability_power_scale` at levels 1/12/20/40/9999 hits the
  clamp; a scaled heal, buff and bleed all store the scaled figure; ability
  `Damage` is *not* scaled; a hostile scales off `ZoneLevel`.
- **Drain / Cleanse / sap:** drain heals the user for the fraction of damage
  dealt and never overheals; cleanse clears a status and is silent on a clean
  target; a negative-power buff reduces effective ATK, and landing one on a
  bracing member cancels its Defend stance (the documented single-slot cost).

`balance_sim` is untouched. It models no abilities at all, by design — so
none of this moves its curves, and it equally cannot tell us that hostile
carriers made fights harder. That risk lands on playtesting, and this spec
does not pretend the gate covers it.

## 10. Documentation

- `assets/abilities/README.md`: new `wild_weight` field; `Drain` and
  `Cleanse` effects; negative buff powers as saps; the rewritten scaling
  paragraph; and a note that these twenty are wild-only.
- Root `README.md` and `CHANGELOG.md`: hunting for routines is a new
  acquisition route and hostiles now use specials — both are player-visible
  claims the current text does not make.

## Out of scope

- Any roster or manifest marker showing that a program is a carrier. Being
  hit with the routine is the discovery mechanism, deliberately.
- A killed carrier dropping its routine as loot.
- New `StatusKind` variants. `Bleed` and `Stun` remain the whole set;
  widening it touches the save format, the status tick, the log and the GUI.
- Any species or research file gaining one of the twenty.
- Multi-routine carriers. One is the prize; two would need a slot policy at
  capture that nothing else in the design needs.
