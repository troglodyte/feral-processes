# Wielding a program as your weapon

**Date:** 2026-08-06
**Status:** built, 2026-08-06

A hidden command on the companion screen equips one of your tamed programs
into the weapon slot. Wielding it lends you a passive ATK/DEF bonus and gives
every attack you make a chance to fire one of that program's installed
routines. The program is unharmed and gains nothing; it is a weapon, not a
combatant.

This is an easter egg. Nothing in the game's text advertises the key.

## Why it is shaped this way

The obvious implementation — put the program in `Equipment::weapon` and bake
its bonus into `Stats` through `apply_equipment_delta` — is wrong twice.

`Equipment::weapon` is `Option<EquippedItem>`: an `ItemId` plus the level and
fusion tier captured at equip time. An entity is none of those things, and
widening that type would push the `Option<Entity>` case through
`equip_swap_rows`, `slot_occupant_with_mods` and the fusion ledger, all of
which are about items.

Worse, a *baked* bonus from a program is the `EquippedItem::fusion_tier` trap
again. A program can be sold, extracted, fused away or killed, and each of
those despawns it. A bonus welded into the player's `Stats` by an equip that
can never be matched by a corresponding unequip is permanent free stats with
no record of where they came from.

So the bonus is computed **live**, exactly as `Game::party_stat_bonus` is and
for the reason that function's doc already gives: computing from the
companion's current `Stats` on demand keeps it correct automatically as the
companion levels, is fused, or dies, with no bookkeeping to keep in sync.

## Components

### 1. State

```rust
/// The tamed program currently equipped as the player's weapon, if any.
/// Not a field on `Equipment`: that slot holds items, and this bonus is
/// computed live rather than baked into `Stats`.
pub struct WieldedProgram(pub Option<Entity>);
```

A resource, in `resources.rs`.

Every reader goes through one accessor:

```rust
/// The wielded program if it still exists. A program sold, extracted,
/// fused away or killed stops being a weapon by omission rather than by
/// anyone remembering to clear the resource.
pub(crate) fn wielded_program(&self) -> Option<Entity>
```

which returns `None` for an entity with no `Stats` — this repo's idiom for
"this entity is gone" (`tests/trade.rs`). Bevy's generation counter means a
recycled entity index cannot resurrect a stale reference.

**This is what keeps the feature out of CLAUDE.md's "two paths, not one"
trap.** Destroying a tamed program goes through either
`dissolve_tamed_program` (sale, extraction, death) or `fuse_companions`
(which does its own `Party::retain` and `despawn` inline). Neither needs to
know this feature exists. A third destruction path added later inherits the
same immunity for free. Do not "tidy this up" into an explicit clear in both
paths — the omission is the design.

### 2. The passive bonus

```rust
/// Standing `(atk, def)` bonus for wielding a program. Deliberately a
/// second, independent knob from `party_stat_bonus` and not a call into
/// it: the party buff may be removed, and this must survive that.
pub(crate) fn wielded_stat_bonus(&self) -> (i32, i32)
```

Lives in `game/combat_round.rs` beside `party_stat_bonus`. Reads the wielded
program's current `Stats` and yields
`((atk / WIELDED_PROGRAM_STAT_DIVISOR).max(1), (def / … ).max(1))`.

Summed into `effective_atk` and `effective_def` at the same player-only
point `party_stat_bonus` is added — so the ATK total still passes through
`battle::power_attack_multiplier` and a hungry player is weakened on the
whole sum, not on part of it.

New in `tuning.rs`, in the same section as `PARTY_PASSIVE_STAT_DIVISOR`:

```rust
/// Divisor on the wielded program's ATK and DEF when totalling the bonus
/// it lends the player (see `Game::wielded_stat_bonus`), floored at 1.
///
/// Deliberately independent of `PARTY_PASSIVE_STAT_DIVISOR` and starting
/// equal to it. Do not re-express this in terms of that constant: the
/// party buff is a candidate for removal and this must not move with it.
pub const WIELDED_PROGRAM_STAT_DIVISOR: i32 = 10;
```

### 3. The proc

In `party_member_attacks`, **slot 0 only**, after the strike has fully
resolved and only if the battle is still running.

Ordering matters and is the least obvious part of the change.
`party_member_attacks` currently ends by checking `creature_alive(front)`
and returning `finish_group_member(group, player)` — an early return whose
`true` means the battle ended. The proc must land *after* that check, so a
routine never resolves against a corpse, and must be skipped entirely when
the battle is over. The attacked group is re-resolved through `retarget`
before the proc, since the strike may have just emptied it.

- **Chance:** `WIELDED_ROUTINE_PROC_CHANCE: f64 = 0.25`, drawn from
  `GameRng`. This is a battle roll; the world-generation prohibition on
  `GameRng` does not apply.
- **Cost:** none. No fatigue drain, no cooldown armed. The wielded program is
  not in the battle line and has no `AbilityCooldowns` tick of its own to
  hang off, and inventing one for a non-combatant is new bookkeeping for no
  gain at a 25% rate.
- **Nothing happens to the program.** No damage, no XP, no `Task` change, no
  status. It is not a party member and does not become one.

**The actor is the program, not the player.** `use_ability` reads
`ability_user_level(actor)` and `ability_affinity(actor, effect)`, and its
`Damage` and `Drain` arms read `effective_atk(actor)`. Passing the program
means a proc scales by *that program's* level, species affinity and ATK —
so which program you wield is what the proc is worth, which is the whole
point of the feature. `is_hostile` is `world.get::<Hostile>(entity).is_some()`
and a tamed program has no `Hostile`, so `ability_recipients` takes the
friendly branch correctly.

**Targeting is synthesized from the attack**, since a proc has no picker:

| `AbilityTarget`      | Synthesized `SpecialTarget`                 |
| -------------------- | ------------------------------------------- |
| `OneEnemyGroupFront` | `EnemyGroup { group }` — the attacked group |
| `WholeEnemyGroup`    | `EnemyGroup { group }`                      |
| `AllEnemies`         | ignored, no picker                          |
| `OneAlly`            | `Ally { slot: 0 }` — the player             |
| `WholeParty`         | ignored, no picker                          |

`OneAlly` resolving to the player rather than a companion is deliberate: it
is your weapon, and slot 0 is the one ally guaranteed to exist.

### 4. Eligibility

One predicate, so the roll and any screen that wants to preview it cannot
disagree:

```rust
/// The routines a wielded program could actually fire — what the proc
/// rolls from.
pub(crate) fn wieldable_routines(&self, entity: Entity) -> Vec<AbilityDef>
```

`actor_abilities(entity)` minus two exclusions:

- **`effect.field_only()`** — `FieldBuff`, `Phase` and `Jump` have nothing to
  resolve against a battle recipient. This is the existing predicate, shared
  with `field_routines`, `battle_special_options`, `wild_routine_ready` and
  `use_ability`'s `unreachable!` arm. Reuse it; do not spell out the three
  variants again.
- **`AbilityEffect::Decompile`** — a free capture roll on every attack,
  spending an ICE Breaker the player did not authorise, undercuts taming as
  something earned by fighting. It is also special-cased in
  `resolve_one_action` because it needs a group index rather than recipient
  entities, so it would not survive the `use_ability` path anyway.

Rolled uniformly from what survives. A program whose whole kit is excluded
simply never procs, and that is a legitimate outcome, not an error — every
tamed program has innate routines installed by `install_innate_routines`, but
nothing guarantees any of them are battle-legal.

### 5. Wielding and unwielding

```rust
pub fn wield_program(&mut self, entity: Entity) -> Result<(), String>
pub fn unwield_program(&mut self) -> Result<(), String>
```

Wielding is mutually exclusive with party membership and with an item in the
weapon slot. The ordering follows the `use_symlink` rule — **every refusal
resolves before any state moves**, so a rejected wield can neither strand a
program between roles nor destroy a weapon:

1. Refuse if the game is over or a battle is active (same guard as
   `Game::equip`).
2. Refuse if `entity` is not a tamed program the player owns.
3. Stand it down from `Party` if it is a member.
4. `unequip(EquipmentSlot::Weapon)` if an item is worn, so its stat delta
   comes off `Stats` and the item returns to inventory.
5. Set `WieldedProgram`.

Wielding costs one turn, like `equip` and `unequip` do. Since step 4 calls
`unequip`, which ticks on its own, the wield path must not tick again after
it — one player action is one tick whether or not a weapon was displaced.

`add_companion` enforces the other door: adding a wielded program to the
party unwields it first, by the same ordering.

### 6. UI

- **Companion menu:** `GameKey::Char('W')` toggles wielding on the
  highlighted row, in `App::handle_companion_key`. Uppercase reaches app-core
  as a distinct key and is already used that way (`Char('S')` in inventory,
  `Char('L')` in playing), so it never collides with `menu_shortcut`'s
  digits-then-lowercase scheme however large the roster grows.
  **The two help lines at the top of that screen do not mention it.** That
  omission is the easter egg and is asserted by a test.
- **`PetInfo` gains `wielded: bool`.** The companion row renders ` (WEP)`,
  following the existing `fusion_tag` / `activity_tag` pattern. It does not
  get its own row colour — `draw_companion_menu` already resolves CRITICAL
  over `fusion_color`, and a third meaning on that axis makes all three
  unreadable.
- **`program_activity` returns `"equipped as weapon"`** for the wielded
  program, ahead of the `Party` check. This gets the manifest, fuse, extract
  and cronjob screens right for free, since they all read that one function.
- **`sale_detachments` gains `"stops being your weapon"`**, so selling the
  program you are wielding warns you the way selling a party member does.
- **`PlayerStatus` gains a `wielded` view** (name, level, the `(atk, def)`
  bonus) so the inventory equipped panel shows the program on the weapon
  line. `weapon` is always `None` when `wielded` is `Some`, since the two are
  mutually exclusive, so no screen has to render both.

### 7. Save

`CreatureSave` gains `wielded: bool`, written and restored the same way
`party_slot` is. Bincode has no field-level compatibility — `CreatureSave`'s
own `custom_name` doc records this — so this is a shape change requiring
`SAVE_FORMAT_VERSION` to go from 22 to 23.

Consequences to plan for: existing saves are invalidated, and every
`dev-saves/` template needs recapturing.

## Testing

Engine:

- `wielded_stat_bonus` lands in both `effective_atk` and `effective_def`, and
  the ATK path still passes through the low-Power multiplier.
- The bonus vanishes when the wielded program is sold, and when it is fused
  away — one test per destruction path, asserting the live-compute safety net
  rather than that anything was cleared.
- Wielding a party member stands it down; adding a wielded program to the
  party unwields it.
- Wielding while a weapon item is worn returns the item to inventory and
  removes its stat delta from `Stats`.
- A refused wield (in battle) leaves the party, the weapon slot and
  `WieldedProgram` all untouched.
- A proc'd `Damage` routine scales off the *program's* ATK and level, not the
  player's — set them apart and assert which one the damage followed.
- `wieldable_routines` never yields `Decompile` or a `field_only` effect,
  asserted over the real `assets/abilities/` set so a new ability file is
  covered.
- The proc does not fire when the strike ended the battle.
- Save/load round trip preserves the wielded program.

app-core:

- `Char('W')` on the companion menu wields the highlighted program, and again
  unwields it.
- The companion screen's help text does not contain the key — the easter-egg
  census, so a later "helpful" edit fails rather than quietly spoiling it.

## What is not gated

`balance_sim` models no abilities at all. It cannot see the proc rate, the
routines that fire, or their magnitudes; the only thing it would notice is
the passive ATK/DEF bonus, and only if a sweep were taught to wield. So this
feature ships with the balance regression suite effectively blind to it, and
the real check is playing it.

That is a statement of fact, not a blocker. But it means the proc chance and
the stat divisor are both unguarded numbers, and a
retune of either will not fail a test — the same position
`assets/abilities/` magnitudes are already in.

## Open

`0.25` for the proc chance is a guess. It wants playing, not deriving.
