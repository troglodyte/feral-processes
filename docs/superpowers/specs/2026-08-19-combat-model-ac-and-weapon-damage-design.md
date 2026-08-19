# Combat model: attack rolls, AC and weapon damage

**Status:** approved, not implemented
**Date:** 2026-08-19
**Slice:** 1 of 4 (see *Decomposition* below)
**Save format:** breaks — `SAVE_FORMAT_VERSION` bump required

## Why

The engine has no miss. `battle::compute_damage` is
`(move_power + atk - def).max(MIN_DAMAGE)` and every swing in the game
lands, on both sides. `def` is pure subtractive absorption; there is no
attack roll, no armour class, no damage range, no crit.

TODO #30 asked for an AC metric affecting hit chance. In discussion the
ask widened to moving the combat model toward standard RPG constructs
(D&D and EVE were both named): base stats plus equipment, level and
perks deciding damage output, AC and absorption; weapons carrying damage
ranges rather than a flat bonus; multiple attacks per round; and being
able to reach past a group's front member.

That is six subsystems, not one. This spec covers the first slice and
records the decomposition so the rest is not re-derived.

## Decomposition

The instinct is to build the attribute layer first, since everything
eventually hangs off it. That is the wrong order: it is a pure refactor
that ends with identical fights, and this project's own history is
features shipping green and unplayed. Change the resolution first, then
the inputs.

| Slice | Contents | State |
|---|---|---|
| **1** | Attack roll vs AC, mitigation as a percentage, weapon damage ranges, crit, fumble | **this spec** |
| 2 | Multiple attacks per round, both sides | deferred |
| 3 | Reaching past a group's front member | deferred |
| 4 | Base attributes (STR/DEX-analogues) behind `atk`/speed | deferred |

Slice 4 arrives *behind* `atk` and `base_speed` rather than replacing
them, so nothing built in slice 1 is thrown away.

Slice 3 is deliberately last and separate. Only `EnemyGroup::members[0]`
is targetable today, and that is what makes a swarm an attrition problem
rather than an alpha-strike; changing it rewrites what a swarm *is* and
must not ride along with a resolution change.

## Design

### Stat vocabulary

| Today | After | Note |
|---|---|---|
| `Stats::atk` | **Attack** — damage only | Meaning narrows; no migration needed |
| `Stats::def` | **Mitigation** — percentage points | Meaning changes under a kept name → save bump |
| `SpeciesDef::base_def` | **`base_mitigation`** — percentage points | Same |
| — | **Accuracy**, **Evasion** | Derived, never stored |
| `EquipmentStats` | gains `damage`, `mitigation`, `evasion`, `accuracy`; loses `def` | All `#[serde(default)]` |

`atk` drives **damage only**, and the to-hit roll comes from speed on
both sides. The alternative — `atk` feeding both to-hit and damage, as
STR does in D&D — compounds quadratically and is the most likely thing
to break `balance_sim`'s curves. Splitting them also pre-shapes slice 4:
a STR-analogue feeds `atk`, and speed stays the DEX-analogue.

"Mitigation" is the existing word (`FieldBuffKind::Mitigation`, and the
`patch_routine` item grants "Mitigation 10"). Reuse it rather than
introducing "Armor" as a second name for the same idea.

### Accuracy and Evasion are derived

Both derive from `SpeciesDef::base_speed`, which every one of the 17
shipped species already authors and which `species::stat_shape_faults`
already holds to a per-class band. This is the whole of "derive AC from
one or more base stats" and it costs no new authoring.

```
accuracy = base_speed * ACCURACY_PER_SPEED
         + level * ACCURACY_PER_LEVEL
         + gear.accuracy

evasion  = base_speed * EVASION_PER_SPEED
         + level * EVASION_PER_LEVEL
         + gear.evasion
```

Neither is stored on `Stats`, so neither is a save field and neither can
drift from its inputs. A fast program both hits and dodges well, which
is the EVE tracking-versus-signature relationship rather than the D&D
one, and is deliberate.

The player has no species. Their `base_speed` comes from
`Game::species_base_speed`, which already has a player arm — see
`game/combat.rs`.

### Resolution pipeline

One attack resolves through a single new seam,
`battle::resolve_attack`, returning
`AttackOutcome { Fumble(Rung), Miss, Hit { dmg }, Crit { dmg } }`.

**1. Hit chance.**

```
h = clamp(accuracy / (accuracy + evasion), HIT_CHANCE_MIN, HIT_CHANCE_MAX)
```

The ratio form is load-bearing and a difference form
(`base + k * (acc - eva)`) must not replace it. The ratio is
**scale-free**: doubling both sides keeps `h` at 0.5. A zone that scales
everything by its tier multiplier therefore changes *nothing* about hit
rates, and the "every difficulty curve must be linear" hazard cannot
reappear on this axis at all. A difference form makes hit rate depend on
absolute scale, so deep zones silently drift toward always-hit or
always-miss.

Two identical creatures get exactly 0.5 by construction, which is the
baseline every tuning number should be read against.

**2. One roll, four bands.** A single `GameRng` draw `r ∈ [0, 1)`:

| Condition | Outcome |
|---|---|
| `r < crit_chance` | Crit (crit_chance is clamped to at most `h`) |
| `r < h` | Hit |
| `r >= 1 - fumble_chance` | Fumble (fumble_chance is clamped to at most `1 - h`) |
| otherwise | Miss |

Evaluated in that order. One draw rather than three, which bounds the
RNG-stream shift and makes crit and fumble mutually exclusive by
construction rather than by a check.

**3. Damage**, on a hit, from a second draw:

```
weapon_roll = uniform(range.min ..= range.max)
dmg         = weapon_roll + atk          // hit
dmg         = weapon_roll * 2 + atk      // crit
```

A crit doubles the **rolled portion only**. Doubling the total would
scale crits with levelling and with every `atk` source in the game.

### Where the range comes from

`resolve_attack` takes one `DamageRange { min, max }`. It has two
constructors, because the two authoring shapes are genuinely different
and forcing one on both would cost an edit to every ability file:

- `DamageRange { min, max }` — authored directly by **items**, which
  never convert to abilities.
- `DamageRange::centred(power, spread)` — used by **abilities and
  moves**, which do.

`AbilityEffect::Damage` and `Drain` keep `power: i32` and gain
`#[serde(default)] spread: i32`. A default of 0 is a degenerate range,
which is exactly today's behaviour, so **none of the 77 ability files
needs editing** and mods gain damage ranges for free.
`abilities::scaled_hp_power` scales the centre; the spread scales with
it proportionally, or a high-level ability becomes deterministic.

`MoveDef` uses `power` + `spread` for the same reason: it is converted
to an `AbilityDef` by `species::basic_attack_ability`, and a
centre-and-spread pair survives that conversion losslessly where a
`(min, max)` pair would round on odd widths.

### Showing the range

A weapon's range is a headline number for the player — "Shiv, 4–9" is
the reference point that makes two weapons comparable at a glance, and
it is the most legible thing this whole slice adds. It is displayed
everywhere a gear stat already is.

A range is a **stat bonus, not an effect**, so it rides
`equip_preview_tag` beside ATK/MIT/DECOMP rather than going through the
`item_blurb` / `item_effects` / `item_grant` derivation, which is for
`grants` entries. A natural attack has a displayable range too, via
`DamageRange::centred`, so a companion's unarmed damage reads the same
way as a weapon's.

**The range must scale through `Game::copy_bonus`'s three axes** —
`scaled_for_level`, `fused_for_tier`, `for_rarity`, in that order, over
a base the affix has already been added to. Both `min` and `max` carry
the per-step floor, and a floor does not commute with a multiplier, so
the ends cannot be scaled by a shortcut that scales the midpoint and
re-derives the width.

This is the trap the existing seam already documents: sharing the
*formatter* was not enough, and four screens each rebuilt the scaling
chain by hand and all four silently dropped the affix. The three axis
methods are `pub(crate)` precisely so a fifth hand-rolled chain fails to
compile. A displayed range that disagrees with the damage actually
rolled is the same bug in a new place, so **one function builds the
range string**, the way `Game::copy_name` is the one place a copy's name
is built.

**A weapon overrides a natural attack, it does not add to it.** A
companion still rolls a species move each turn for its *name* and its
status rider, but an equipped weapon supplies the damage range. Unarmed,
the move's own range applies. The player has no species moves:
`PLAYER_STRIKE_POWER` becomes an unarmed range constant in `tuning.rs`,
overridden the same way.

**4. Mitigation**, through the existing `mitigate_incoming_damage`.

**5. `Game::apply_damage`**, unchanged — still the one path that lowers
HP.

`compute_damage`, `MIN_DAMAGE` and the subtractive floor are **deleted**.
That retires the constraint recorded in `CLAUDE.md` that every
difficulty curve must be linear, which existed only because a geometric
curve racing a linear one eventually lands every swing on the floor.

### Mitigation does not scale with level or zone

A percentage that grows per level approaches immunity. So:

- `progression::stats_after_levels` must not raise mitigation.
- `ZoneLevel::stat_multiplier` must not scale mitigation.
- Total mitigation is the sum of innate + gear + field buffs, capped at
  `MAX_MITIGATION_PERCENT`.

Levelling buys HP, `atk`, accuracy and evasion. Mitigation comes from
gear and from what a species innately is. This is the rule that keeps
the percentage form safe, and it is the one most likely to be
"corrected" by someone restoring symmetry with the other stats.

The player has no species, so their innate mitigation is the
`def`-turned-`mitigation` component of `tuning::PLAYER_BASE_STATS` —
which is an offset and not a rate, and so is not swept by the levelling
rule above.

`mitigate_incoming_damage` currently reads only
`field_buff_power(target, Mitigation)`. It gains the innate and gear
sources and the cap. Its existing behaviour — rounding once in the same
expression, and flooring a landed hit at 1 while leaving `dmg <= 0`
untouched — is correct as written and stays.

### The fumble ladder

Rungs replace rather than stack. A cumulative top rung is a run-ender.

| Rung | Effect | Machinery |
|---|---|---|
| 1 | **Exposed** — evasion cut by `EXPOSED_EVASION_PERCENT` until the fumbler's next turn | New `StatusKind::Exposed` |
| 2 | **Recoil** — `FUMBLE_RECOIL_FRACTION` of a fresh roll of the fumbler's own range | `apply_damage` |
| 3 | **Opening** — the target takes a free swing at the fumbler | `resolve_attack`, non-recursive |
| 4 | **Crash** — the fumbler loses their next action | `StatusKind::Stun`, `arm_status` |

Severity comes from *how deep into the fumble band the roll fell*, so it
needs no second draw:

```
d = (r - (1 - fumble_chance)) / fumble_chance      // d ∈ [0, 1)
```

with four thresholds in `tuning.rs` weighting the deep rungs rare.

**The Opening rung must not recurse.** A free swing that itself fumbles
resolves as a plain miss. This is a hard rule inside `resolve_attack`,
not a convention, and it needs its own test.

`StatusKind::Exposed` is a free win for content: `MoveEffect` already
lets any species move inflict a status from `.ron`, so a debuffer
species costs no Rust the day Exposed exists.

Exposed belongs in `StatusEffects` (conditions a hostile move inflicts,
always unwanted) and not in `ActiveBuff`, which holds one wanted buff at
a time.

Rate is symmetric between the player and hostiles, sitting on its own
constant so it can be split per side later without touching resolution.

### Not everything that takes damage rolls to hit

`Game::attack_nest` calls `compute_damage(atk, 0, 5)` against a
`Durability`, not a creature. A structure has no speed and cannot dodge.
Structure damage keeps a deterministic, unrolled path; only
creature-versus-creature attacks go through `resolve_attack`.

### `Stats::power()` has to be redefined

`power()` is `max_hp + atk + def` and has roughly ten callers, including
`progression::kill_xp`'s denominator, `difficulty_color`'s con colours,
trade valuation and unlock ratios. Summing a percentage into it is
meaningless.

Redefine it as effective HP plus attack:

```
power = max_hp / (1 - mitigation / 100)  +  atk
```

`mitigation` is capped strictly below 100 by `MAX_MITIGATION_PERCENT`,
so the denominator cannot reach zero. That cap is load-bearing here as
well as in the damage path.

which prices mitigation correctly rather than dropping it. **This moves
every con colour and every kill's XP in the game** and needs its own
test and its own line in the changelog — it is a consequence, not a
side effect to be discovered later.

### Call sites

Six `compute_damage` callers, and they do not all become attack rolls:

| Site | Becomes |
|---|---|
| `combat_round.rs` party member attacks | `resolve_attack` |
| `combat_round.rs` `AbilityEffect::Damage` | `resolve_attack` |
| `combat_round.rs` `AbilityEffect::Drain` | `resolve_attack`; a miss must skip the heal |
| `combat_enemy.rs` wild attack | `resolve_attack` |
| `combat_policy.rs` projected damage | expected-value form |
| `zone.rs` `attack_nest` | deterministic, no roll |

A miss cannot live in `apply_damage`: a missed Drain would still heal its
caster and a missed rider would still land its stun. The branch belongs
at each call site, on the `AttackOutcome`.

### `balance_sim`

`balance_sim` is RNG-free and models expected damage, so it takes an
`expected_damage(...)` function — the mean of the same arithmetic —
living beside `resolve_attack` and **called** by both sides.

It must be a call, not a copy. `CLAUDE.md` records four separate
occasions where a `balance_sim` doc comment promised it mirrored a real
formula while being an independent copy that drifted, the worst being a
mining-reliability curve that would have let the balance gate pass
against a game that no longer existed. Follow
`battle::attackers_in_group` and `battle::slot_aggro_weight`.

Its `TURN_CAP` stalemate detection stays meaningful: hit chance is
floored at `HIT_CHANCE_MIN` and mitigation capped, so expected damage is
always positive.

## Asset changes

| Directory | Files | Change |
|---|---|---|
| `assets/species/` | 17 | `base_def` → `base_mitigation`; 34 moves get `damage: (min, max)` in place of `power` |
| `assets/items/` | 13 weapons | gain `damage: (min, max)`; some trade damage for `accuracy` |
| `assets/items/` | 12 armour | `def` → `mitigation`; light pieces author `evasion` instead |
| `assets/items/` | 14 modules | `def` → `mitigation` where present |
| `assets/abilities/` | 13 with `BuffKind::Def` | powers re-authored as percentage points |

The `accuracy` and `evasion` fields on `EquipmentStats` must be
**actually authored** in this pass, not merely added. Heavy armour
buying mitigation while light armour buys evasion is what makes the two
defensive axes a real choice rather than one stat with a second name —
and a field nothing authors is an unused feature flag, which is why gear
crit is deferred below instead.

Move **names and flavour are untouched** — "Fray" and "Static Burst" stay
exactly as authored. Only the number becomes a range.

`assets/*/README.md` for species, items and abilities must be updated in
the same change, per the standing schema-doc rule.

## Save format

`SAVE_FORMAT_VERSION` bumps. Field-named RON retires migrations for
*additive* change, but `def` changing meaning under a name it keeps is
precisely the case it does not cover — an old save would load an
absorption number straight into a percentage slot.

`dev-saves/` templates are re-captured after the bump.

## Testing

Beyond the usual per-behaviour unit tests:

- **Scale invariance.** Doubling accuracy and evasion together leaves
  hit chance unchanged. This is the property the ratio form exists for.
- **Clamps hold at both ends.** Accuracy 1000 vs evasion 1 does not
  exceed `HIT_CHANCE_MAX`; the reverse does not fall below
  `HIT_CHANCE_MIN`.
- **Draw counts are pinned per outcome.** A miss costs one draw, a hit
  two, a recoil fumble three. Asserting the exact count is what stops
  crit or fumble silently becoming an extra draw and shifting the whole
  run's RNG stream.
- **Crit and fumble are mutually exclusive**, by construction rather
  than by assertion on a sample.
- **A missed Drain heals nothing** and a missed rider lands nothing.
- **The Opening rung does not recurse** — a fumbled free swing is a
  plain miss. Delete the guard and this test must fail.
- **Mitigation is capped**, and does not move with level or zone.
- **A structure cannot be missed.**
- `balance_sim`'s curves are re-baselined. Every hardcoded curve moves;
  that is the signal, not a break.

Per the standing rule: each new test must fail with its fix removed.

## What goes stale

- **`assets/policies/enemy_battle.ron`.** The weights were trained
  against a world where every swing lands and `target_def_rel` is pinned
  to zero. Accept the drift in slice 1 and retrain once slices 2 and 3
  are in; retraining against a model that is about to gain multi-attack
  and past-the-front targeting is wasted work.
- **`dev-arenas/` reports.** Arena numbers only compare within one
  build, and this reshuffles the RNG stream as well as the model.
  Existing reports become incomparable.
- **`docs/measurements/`.** Anything measuring damage, XP pacing or
  fight length predates this and should be read as historical.

## Deferred, deliberately

- **Gear crit chance.** Crit rate is a flat `tuning.rs` constant in
  slice 1. A `crit` field on `EquipmentStats` that nothing authors is an
  unused feature flag; it arrives when an item wants it.
- **Splitting fumble rate per side.** One constant now, symmetric.
- **`BuffKind` rework beyond the `Def` rename.** `Atk`/`Def` becomes
  `Atk`/`Mitigation`; a wider rework waits for slice 4.
