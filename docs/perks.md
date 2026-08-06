# Perk catalogue

Every perk a player can buy, charted from `assets/perks/` and
`crates/engine/src/tuning.rs`. Twelve of them, and there will be twelve until
someone writes Rust.

**These numbers are a transcription, not a read.** They were copied out on
2026-08-05 and will drift the moment either source is edited; regenerate the
page rather than trusting it blind.

This is the one page in `docs/` that has to reach across the moddable seam,
because a perk is deliberately split in half. What it is **called**, how it
**reads** and what it **costs** live in `assets/perks/*.ron` and can be edited
without a compiler. How much it **gives per level** lives in `tuning.rs` and
cannot. That split is the game's standing rule — content is moddable, how hard
the game is, is not — and it means a `.ron` edit can make a perk cheaper or
dearer but never stronger.

| | |
|---|---|
| perks | 12 |
| prices | 2 and 3 Perk Points |
| one level of everything | 27 points |
| points earned | 1 per player level, plus up to 5 from the [achievement ladder](achievements.md) |
| affinity perks | 5 of 12, sharing two rates |

## The catalogue

Listed in `Perk::all()` order, which is the order the picker shows and — for
the enum behind it — the save format. Saves are bincode, which encodes an enum
positionally, so this order is load-bearing: append, never reorder.

|  | Perk | Cost | Per level | Hooks into |
|:---|:---|---:|:---|:---|
| `KeenScavenger` | Keen Scavenger | 2 | +1pp mining success | systems::mining_success_chance |
| `LowPowerMode` | Low Power Mode | 2 | -1pp Power drain, floor 0 | the hunger-decay multiplier |
| `ExploitFocus` | Exploit Focus | 3 | -3pp of the target's HP penalty | taming::capture_chance |
| `LeanCompiler` | Lean Compiler | 3 | -1 of each ingredient, floor 1 | Game::craft_recipes' costs |
| `Attacker` | Attacker | 2 | +3 ATK, permanent | a direct Stats write at purchase |
| `Defender` | Defender | 2 | +3 DEF, permanent | a direct Stats write at purchase |
| `Buffer` | Buffer | 3 | +1% max Integrity, at least +10 | a direct Stats write, plus a full heal |
| `DamageAffinity` | Payload Tuning | 2 | +15% Damage magnitude | the player's own casts only |
| `HealAffinity` | Field Medic | 2 | +5% Heal magnitude | the player's own casts only |
| `BuffAffinity` | Overclocker | 2 | +5% Buff magnitude | the player's own casts only |
| `DebuffAffinity` | Corruption Vector | 2 | +5% Debuff magnitude | the player's own casts only |
| `DrainAffinity` | Siphon Protocol | 2 | +15% Drain damage | the player's own casts only |

## Price

```
PERK POINT PRICE

3  Exploit Focus, Lean Compiler, Buffer
2  Keen Scavenger, Low Power Mode, Attacker, Defender, Payload Tuning, Field Medic, Overclocker, Corruption Vector, Siphon Protocol

one level of all 12: 27 points
```

Only three perks cost 3, and what they have in common is that they change a
*rate* rather than a number: Buffer scales with the Integrity you already
have, Lean Compiler pays out on every craft for the rest of the run, and
Exploit Focus is worth more the healthier the program you are trying to take.
The nine at 2 are flat.

Note what 27 points means against how they arrive. A Perk Point is
1 per player level and at most 5 more from a fully cleared
profile, so buying one level of all twelve is most of the first thirty levels
of a run. Perks are not a shopping list to complete; they are a shape to
commit to.

## Where the magnitudes live

| Perk | Constant in `tuning.rs` |
|:---|:---|
| Keen Scavenger | `KEEN_SCAVENGER_BONUS_PER_LEVEL = 0.01` |
| Low Power Mode | `LOW_POWER_MODE_REDUCTION_PER_LEVEL = 0.01` |
| Exploit Focus | `EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL = 0.03` |
| Lean Compiler | `LEAN_COMPILER_DISCOUNT_PER_LEVEL = 1` |
| Attacker | `ATTACKER_BONUS_PER_LEVEL = 3` |
| Defender | `DEFENDER_BONUS_PER_LEVEL = 3` |
| Buffer | `BUFFER_BONUS_PERCENT_PER_LEVEL = 0.01` |
| Payload Tuning | `AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED` |
| Field Medic | `AFFINITY_PERK_BONUS_PER_LEVEL` |
| Overclocker | `AFFINITY_PERK_BONUS_PER_LEVEL` |
| Corruption Vector | `AFFINITY_PERK_BONUS_PER_LEVEL` |
| Siphon Protocol | `AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED` |

Every one of those is a hook into a different formula — a mining roll, a
hunger multiplier, a capture chance's HP term, a recipe cost, a direct `Stats`
write. There is no shared shape between them, which is exactly why `PerkDef`
has no `effect:` field and why a thirteenth perk is a new `Perk` variant plus
a hook wherever its effect belongs, rather than a new file.

## The five affinity perks

These are the one place the twelve *do* share a shape: each multiplies one
`AffinityKind` category — Damage, Heal, Buff, Debuff, Drain — for the player's
own ability casts. Never a companion's: a companion's affinity is its species'
business, and a party-wide perk would multiply against it.

They cost the same and they do not pay the same.

```
AFFINITY BY PERK LEVEL                 (clamped at 2.00)

Payload Tuning     +15%/lvl  ##############..........................   7 lvl / 14 pts
Siphon Protocol    +15%/lvl  ##############..........................   7 lvl / 14 pts
Field Medic        +5%/lvl   ########################################  20 lvl / 40 pts
Overclocker        +5%/lvl   ########################################  20 lvl / 40 pts
Corruption Vector  +5%/lvl   ########################################  20 lvl / 40 pts
```

Payload Tuning and Siphon Protocol run at three times the rate of the other
three, which looks like a mistake and is not. At the shared rate, 0.05 of an
authored `power` 10 is +0.5 damage per perk level, against the flat +3 that
`Attacker` buys for the same two points on *every* attack — strictly worse,
and only on one category. The higher rate is what makes them worth buying, and
the cost is reaching the clamp sooner.

That comparison is not fixed, either, which is the other half of why the rates
differ. An affinity multiplies a magnitude that already grows with player
level, so 0.15 starts behind `Attacker`'s flat +3, passes it around player
level 3 and keeps widening; +3 is +3 for the rest of the run. A flat perk that
never stops paying a little, against a scaling one that pays little at first
and then hits a ceiling.

The clamp is `AFFINITY_MAX`, the same ceiling a species file is held to at
load. Perk *levels* are uncapped, so without it a long enough run would let
the player's own casts exceed the bound every affinity in the game is bound
by. `AffinityKind::perk_bonus_per_level` is what picks the right rate, so a
perk's category decides it rather than a match repeated at five call sites.

---

Source of truth is `assets/perks/` for names and costs, and
`crates/engine/src/tuning.rs` for magnitudes. Unlike species, structures,
items and abilities, **you cannot add a perk with a file** — the catalogue
keys off the `Perk` enum, so a `.ron` naming anything else fails to parse and
is skipped. To update this page, edit the table at the top of
[`docs/perks-gen.py`](perks-gen.py) and run `python3 docs/perks-gen.py` from
the repo root. The schema is documented in
[`assets/perks/README.md`](../assets/perks/README.md).
