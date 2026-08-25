# Perk catalogue

Every perk a player can buy, charted from `assets/perks/` and
`crates/engine/src/tuning.rs`. 17 of them, and there will be 17
until someone writes Rust.

**These numbers are a transcription, not a read.** They were copied out on
2026-08-21 and will drift the moment either source is edited; regenerate the
page rather than trusting it blind. The Attacker and Defender rows spent a
release saying +3 against a `tuning.rs` that said 2, which is what this
warning is about.

This is the one page in `docs/` that has to reach across the moddable seam,
because a perk is deliberately split in half. What it is **called**, how it
**reads** and what it **costs** live in `assets/perks/*.ron` and can be edited
without a compiler. How much it **gives per level** lives in `tuning.rs` and
cannot. That split is the game's standing rule — content is moddable, how hard
the game is, is not — and it means a `.ron` edit can make a perk cheaper or
dearer but never stronger.

| | |
|---|---|
| perks | 17 |
| prices | 2, 3 and 4 Perk Points |
| one level of everything | 42 points |
| points earned | 1 per player level, plus up to 5 from the [achievement ladder](achievements.md) |
| affinity perks | 5 of 17, sharing two rates |

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
| `Attacker` | Attacker | 2 | +2 ATK, permanent | a direct Stats write at purchase |
| `Defender` | Defender | 2 | +2 DEF, permanent | a direct Stats write at purchase |
| `Buffer` | Buffer | 3 | +1% max Integrity, at least +10 | a direct Stats write, plus a full heal |
| `DamageAffinity` | Payload Tuning | 2 | +15% Damage magnitude | the player's own invocations only |
| `HealAffinity` | Field Medic | 2 | +5% Heal magnitude | the player's own invocations only |
| `BuffAffinity` | Overclocker | 2 | +5% Buff magnitude | the player's own invocations only |
| `DebuffAffinity` | Corruption Vector | 2 | +5% Debuff magnitude | the player's own invocations only |
| `DrainAffinity` | Siphon Protocol | 2 | +15% Drain damage | the player's own invocations only |
| `Obfuscation` | Obfuscation | 3 | -10% to every Trace rise, floor 1 | Game::raise_trace |
| `ProcessPool` | Process Pool | 3 | +1 tamed program you may own | Game::pet_capacity |
| `Teardown` | Teardown | 4 | +1 work resource per kill | Game::award_loot |
| `Failover` | Failover | 2 | +1 Durability per repair interval | Game::total_repair_rate |
| `TightenTolerances` | Tighten Tolerances | 3 | +5pp on a compiled copy's quality floor | Game::craft_quality_floor |

## Price

```
PERK POINT PRICE

4  Teardown
3  Exploit Focus, Lean Compiler, Buffer, Obfuscation, Process Pool, Tighten Tolerances
2  Keen Scavenger, Low Power Mode, Attacker, Defender, Payload Tuning, Field Medic, Overclocker, Corruption Vector, Siphon Protocol, Failover

one level of all 17: 42 points
```

What the perks at 3 have in common is that they change a *rate* rather than a
number: Buffer scales with the Integrity you already have, Lean Compiler pays
out on every craft for the rest of the run, Exploit Focus is worth more the
healthier the program you are trying to take, Obfuscation is a proportion of
whatever you were about to spend, Process Pool raises a ceiling every later
program is measured against, and Tighten Tolerances moves the band every
piece of gear you compile from here on rolls inside. The ones at 2 are flat.

Teardown is alone at 4 because it is the steepest thing in the catalogue
relative to what it modifies: a kill drops 2-4 work resources, so a single
level is worth between a third and a half again of every fight in the run.

Note what 42 points means against how they arrive. A Perk Point is
1 per player level and at most 5 more from a fully cleared
profile, so buying one level of each is most of the first forty levels of a
run. Perks are not a shopping list to complete; they are a shape to commit
to.

## Where the magnitudes live

| Perk | Constant in `tuning.rs` |
|:---|:---|
| Keen Scavenger | `KEEN_SCAVENGER_BONUS_PER_LEVEL = 0.01` |
| Low Power Mode | `LOW_POWER_MODE_REDUCTION_PER_LEVEL = 0.01` |
| Exploit Focus | `EXPLOIT_FOCUS_HP_PENALTY_REDUCTION_PER_LEVEL = 0.03` |
| Lean Compiler | `LEAN_COMPILER_DISCOUNT_PER_LEVEL = 1` |
| Attacker | `ATTACKER_BONUS_PER_LEVEL = 2` |
| Defender | `DEFENDER_BONUS_PER_LEVEL = 2` |
| Buffer | `BUFFER_BONUS_PERCENT_PER_LEVEL = 0.01` |
| Payload Tuning | `AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED` |
| Field Medic | `AFFINITY_PERK_BONUS_PER_LEVEL` |
| Overclocker | `AFFINITY_PERK_BONUS_PER_LEVEL` |
| Corruption Vector | `AFFINITY_PERK_BONUS_PER_LEVEL` |
| Siphon Protocol | `AFFINITY_PERK_BONUS_PER_LEVEL_UNSCALED` |
| Obfuscation | `OBFUSCATION_REDUCTION_PER_LEVEL = 0.10` |
| Process Pool | `PROCESS_POOL_SLOTS_PER_LEVEL = 1` |
| Teardown | `TEARDOWN_SALVAGE_PER_LEVEL = 1` |
| Failover | `FAILOVER_REPAIR_PER_LEVEL = 1` |
| Tighten Tolerances | `QUALITY_PERK_PER_LEVEL = 5` |

Every one of those is a hook into a different formula — a mining roll, a
hunger multiplier, a capture chance's HP term, a recipe cost, a direct `Stats`
write. There is no shared shape between them, which is exactly why `PerkDef`
has no `effect:` field and why an eighteenth perk is a new `Perk` variant plus
a hook wherever its effect belongs, rather than a new file.

## The five affinity perks

These are the one place the 17 *do* share a shape: each multiplies one
`AffinityKind` category — Damage, Heal, Buff, Debuff, Drain — for the player's
own ability invocations. Never a companion's: a companion's affinity is its species'
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
the player's own invocations exceed the bound every affinity in the game is bound
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
