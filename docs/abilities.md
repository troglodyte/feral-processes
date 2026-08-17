# Ability catalogue

Every shipped ability in feral-processes, charted from its own file in
`assets/abilities/`. 70 of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/abilities/*.ron` on 2026-08-17 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

A species grants abilities by naming their ids with a level to unlock each at;
`priority_boost` must exist, because it is the fallback for a companion whose
species grants nothing. The [research tree](research.md) teaches the rest.

| | |
|---|---|
| abilities | 70 |
| effect shapes | 10 |
| target shapes | 5 |
| field routines (run outside battle) | 12 |
| of those, Stack-only movement | 2 |

## The naming scheme

An ability's **id** is flavour and its **name** is a spec. `kernel_panic`,
`cascade_overflow` and `broadcast_storm` sound like three unrelated things;
their names say Packet Shred Single, Packet Shred Group v1.0 and Packet Shred
Everyone, which is one effect at three scopes. A player reading a menu is
being told what the routine does and how wide it reaches, every time, in the
same word order.

```
ABILITY FAMILIES            (display name = effect + scope + tier)

Bit Rot         Single v1.0 (2), Single v2.0 (4), Single v3.0 (6), Group (3), Everyone (2)
Patch           Single v1.0 (8), Single v2.0 (25), Single v3.0 (50), Party v1.0 (8), Party v1.1 (10)
Bastion         Single v1.0 (3), Single v2.0 (5), Single v3.0 (7), Party (4)
Packet Shred    Single (16), Group v1.0 (6), Group v2.0 (10), Everyone (25)
Hard Lock       Single v1.0 (0), Single v2.0 (0), Group (0), Everyone (0)
Skim            Single v1.0 (5), Single v2.0 (9), Single v3.0 (14), Group (4)
Etch            Single (-5), Group (-4), Everyone (-3)
Pipeline Stall  Single (7), Group (6), Everyone (6)
Throttle        Single (-5), Group (-4), Everyone (-3)
Leech           Single (10), Group (6), Everyone (4)
Fork Bomb       Single (9), Group (15), Everyone (8)
Hyperthread     Single v1.0 (3), Single v2.0 (6), Party (3)
Rollback        Single v1.0 (10), Single v2.0 (20), Single v3.0 (35)
Segfault        Single v1.0 (6), Single v2.0 (11), Single v3.0 (17)
Flush Cache     Single (0), Party (0)

one of a kind: Ablative Layer, Buffer Overrun, Deadman, Decompile, Deep Scan, Hard Fault, Hardened Shell, Kernel Shear, Long Winter, Null Cache, Overclock, Repair Loop, Salvage Routine, Stealth Protocol, Trace Analysis, Trickle Charge, Watchdog, Wild Jump
```

The number in brackets is the effect's power, and a `v2.0` at the same scope
is the straight upgrade over its `v1.0`. Read across a row and reaching wider
usually costs magnitude — Leech runs 10, 6, 4 — but read the whole block and
two families break that on purpose: Packet Shred and Fork Bomb both peak away
from Single, which is what marks them as the prizes of the set rather than
ladders you climb. The honest comparison is the cost chart below, not this
one. Nothing in the game names a routine after what it is *called* rather
than what it *does* — which is why the id column exists at all, and why
renaming an id never changes what a player reads.

## Who it hits against what it does

```
TARGET AGAINST EFFECT

                      Damag  Debuf   Buff   Heal  Drain  Field  Clean  Decom  Phase   Jump

OneAlly                   .      .      5      6      .      4      1      .      .      .
WholeParty                .      .      2      2      .      6      2      .      1      1
OneEnemyGroupFront        6      5      2      .      4      .      .      1      .      .
WholeEnemyGroup           5      2      2      .      3      .      .      .      .      .
AllEnemies                4      3      2      .      1      .      .      .      .      .

                         15     10     13      8      8     10      3      1      1      1
```

The grid is sparse on purpose. Heals and buffs point at allies, damage and
debuffs point at enemies. The one crossing is `Buff` aimed at enemies — Etch
and Throttle are buffs with **negative** power, so a sap is not a separate
effect shape but the same one run backwards. `Decompile` is the one effect
with a single ability to its name, because taming is an ability rather than
a separate verb. `Cleanse` is the one that *removes* rather than adds, which
is why it needs no power column and why it is the only ally-facing effect
with nothing to scale.

## Everything

| Ability | Name | Target | Effect | Pow | Dur | Rider | CD |
|:---|:---|:---|:---|---:|---:|:---|---:|
| `broadcast_storm` | Packet Shred Everyone | AllEnemies | Damage | 25 | - | - | 4 |
| `kernel_shear` | Kernel Shear Group | WholeEnemyGroup | Damage | 22 | - | Bleed 75% 4r | 4 |
| `segfault_v3` | Segfault Single v3.0 | OneEnemyGroupFront | Damage | 17 | - | - | 4 |
| `kernel_panic` | Packet Shred Single | OneEnemyGroupFront | Damage | 16 | - | - | 3 |
| `fork_bomb` | Fork Bomb Group | WholeEnemyGroup | Damage | 15 | - | Bleed 35% 2r | 3 |
| `deadman` | Deadman Everyone | AllEnemies | Damage | 14 | - | - | 4 |
| `segfault_v2` | Segfault Single v2.0 | OneEnemyGroupFront | Damage | 11 | - | - | 3 |
| `packet_shred` | Packet Shred Group v2.0 | WholeEnemyGroup | Damage | 10 | - | - | 3 |
| `stack_smash` | Fork Bomb Single | OneEnemyGroupFront | Damage | 9 | - | Bleed 60% 3r | 2 |
| `pid_exhaustion` | Fork Bomb Everyone | AllEnemies | Damage | 8 | - | Bleed 20% 2r | 5 |
| `pipeline_stall` | Pipeline Stall Single | OneEnemyGroupFront | Damage | 7 | - | Stun 40% 1r | 3 |
| `branch_hazard` | Pipeline Stall Group | WholeEnemyGroup | Damage | 6 | - | Stun 30% 1r | 4 |
| `bus_fault` | Pipeline Stall Everyone | AllEnemies | Damage | 6 | - | Stun 25% 1r | 5 |
| `cascade_overflow` | Packet Shred Group v1.0 | WholeEnemyGroup | Damage | 6 | - | - | 2 |
| `segfault_v1` | Segfault Single v1.0 | OneEnemyGroupFront | Damage | 6 | - | - | 2 |
| `bit_rot_v3` | Bit Rot Single v3.0 | OneEnemyGroupFront | Debuff Bleed | 6 | 4 | - | 3 |
| `bit_rot_v2` | Bit Rot Single v2.0 | OneEnemyGroupFront | Debuff Bleed | 4 | 3 | - | 2 |
| `heap_corruption` | Bit Rot Group | WholeEnemyGroup | Debuff Bleed | 3 | 3 | - | 3 |
| `bit_rot` | Bit Rot Everyone | AllEnemies | Debuff Bleed | 2 | 4 | - | 5 |
| `memory_leak` | Bit Rot Single v1.0 | OneEnemyGroupFront | Debuff Bleed | 2 | 3 | - | 1 |
| `deadlock` | Hard Lock Single v1.0 | OneEnemyGroupFront | Debuff Stun | 0 | 1 | - | 2 |
| `hard_fault` | Hard Fault Everyone | AllEnemies | Debuff Stun | 0 | 2 | - | 5 |
| `hard_lock` | Hard Lock Single v2.0 | OneEnemyGroupFront | Debuff Stun | 0 | 2 | - | 4 |
| `null_route` | Hard Lock Everyone | AllEnemies | Debuff Stun | 0 | 1 | - | 5 |
| `race_condition` | Hard Lock Group | WholeEnemyGroup | Debuff Stun | 0 | 1 | - | 4 |
| `bastion_shield_v3` | Bastion Single v3.0 | OneAlly | Buff Def | 7 | 4 | - | 2 |
| `hyperthread` | Hyperthread Single v2.0 | OneAlly | Buff Atk | 6 | 4 | - | 3 |
| `bastion_shield_v2` | Bastion Single v2.0 | OneAlly | Buff Def | 5 | 3 | - | 2 |
| `bastion` | Bastion Party | WholeParty | Buff Def | 4 | 3 | - | 3 |
| `overclock_array` | Hyperthread Party | WholeParty | Buff Atk | 3 | 3 | - | 3 |
| `priority_boost` | Hyperthread Single v1.0 | OneAlly | Buff Atk | 3 | 3 | - | 1 |
| `sandbox` | Bastion Single v1.0 | OneAlly | Buff Def | 3 | 3 | - | 1 |
| `brownout` | Throttle Everyone | AllEnemies | Buff Atk | -3 | 3 | - | 5 |
| `oxide_strip` | Etch Everyone | AllEnemies | Buff Def | -3 | 3 | - | 5 |
| `etch` | Etch Group | WholeEnemyGroup | Buff Def | -4 | 3 | - | 3 |
| `throttle` | Throttle Group | WholeEnemyGroup | Buff Atk | -4 | 3 | - | 3 |
| `acid_wash` | Etch Single | OneEnemyGroupFront | Buff Def | -5 | 3 | - | 2 |
| `clock_gate` | Throttle Single | OneEnemyGroupFront | Buff Atk | -5 | 3 | - | 2 |
| `cold_boot` | Patch Single v3.0 | OneAlly | Heal | 50 | - | - | 5 |
| `rollback_v3` | Rollback Single v3.0 | OneAlly | Heal | 35 | - | - | 4 |
| `checksum_repair` | Patch Single v2.0 | OneAlly | Heal | 25 | - | - | 3 |
| `rollback_v2` | Rollback Single v2.0 | OneAlly | Heal | 20 | - | - | 3 |
| `redundancy_sync` | Patch Party v1.1 | WholeParty | Heal | 10 | - | - | 3 |
| `rollback_v1` | Rollback Single v1.0 | OneAlly | Heal | 10 | - | - | 2 |
| `hot_patch` | Patch Single v1.0 | OneAlly | Heal | 8 | - | - | 1 |
| `mirror_restore` | Patch Party v1.0 | WholeParty | Heal | 8 | - | - | 2 |
| `skim_v3` | Skim Single v3.0 | OneEnemyGroupFront | Drain | 14 | - | - | 4 |
| `null_cache` | Null Cache Group | WholeEnemyGroup | Drain | 12 | - | - | 3 |
| `siphon_cycles` | Leech Single | OneEnemyGroupFront | Drain | 10 | - | - | 2 |
| `skim_v2` | Skim Single v2.0 | OneEnemyGroupFront | Drain | 9 | - | - | 3 |
| `leech_array` | Leech Group | WholeEnemyGroup | Drain | 6 | - | - | 4 |
| `skim_v1` | Skim Single v1.0 | OneEnemyGroupFront | Drain | 5 | - | - | 2 |
| `cycle_harvest` | Leech Everyone | AllEnemies | Drain | 4 | - | - | 5 |
| `skim_group` | Skim Group | WholeEnemyGroup | Drain | 4 | - | - | 3 |
| `long_winter` | Long Winter Party | WholeParty | FieldBuff Mitigation | 25 | - | - | - |
| `deep_scan` | Deep Scan Party | WholeParty | FieldBuff CaptureBoost | 20 | - | - | - |
| `salvage_routine` | Salvage Routine Party | WholeParty | FieldBuff DropBoost | 20 | - | - | - |
| `stealth_protocol` | Stealth Protocol Party | WholeParty | FieldBuff EncounterDamp | 20 | - | - | - |
| `trace_analysis` | Trace Analysis Party | WholeParty | FieldBuff XpBoost | 20 | - | - | - |
| `ablative_layer` | Ablative Layer Single | OneAlly | FieldBuff Mitigation | 10 | - | - | - |
| `hardened_shell` | Hardened Shell Single | OneAlly | FieldBuff Def | 4 | - | - | - |
| `overclock` | Overclock Single | OneAlly | FieldBuff Atk | 4 | - | - | - |
| `repair_loop` | Repair Loop Single | OneAlly | FieldBuff Regen | 2 | 300 | - | - |
| `trickle_charge` | Trickle Charge Party | WholeParty | FieldBuff Trickle | 1 | 60 | - | - |
| `flush_cache` | Flush Cache Party | WholeParty | Cleanse | 0 | - | - | 3 |
| `invalidate_line` | Flush Cache Single | OneAlly | Cleanse | 0 | - | - | 2 |
| `watchdog` | Watchdog Party | WholeParty | Cleanse | 0 | - | - | 4 |
| `decompile` | Decompile Single | OneEnemyGroupFront | Decompile | 0 | - | - | - |
| `buffer_overrun` | Buffer Overrun Party | WholeParty | Phase | 0 | - | - | - |
| `wild_jump` | Wild Jump Party | WholeParty | Jump | 0 | - | - | - |

There is no cost column, because for everything above the CD *is* the cost:
a battle routine charges no need at all, from the player, a companion or a
wild carrier. The routines that do spend something are the field ones, in
their own two tables further down.

## What a hit costs

```
DAMAGE PER ROUND OF COOLDOWN

Packet Shred Everyone      25 / 4    ############################## 6.25
Kernel Shear Group         22 / 4    ##########################.... 5.50
Packet Shred Single        16 / 3    ##########################.... 5.33
Fork Bomb Group            15 / 3    ########################...... 5.00
Fork Bomb Single            9 / 2    ######################........ 4.50
Segfault Single v3.0       17 / 4    ####################.......... 4.25
Segfault Single v2.0       11 / 3    ##################............ 3.67
Deadman Everyone           14 / 4    #################............. 3.50
Packet Shred Group v2.0    10 / 3    ################.............. 3.33
Packet Shred Group v1.0     6 / 2    ##############................ 3.00
Segfault Single v1.0        6 / 2    ##############................ 3.00
Pipeline Stall Single       7 / 3    ###########................... 2.33
Fork Bomb Everyone          8 / 5    ########...................... 1.60
Pipeline Stall Group        6 / 4    #######....................... 1.50
Pipeline Stall Everyone     6 / 5    ######........................ 1.20
```

Read this one carefully, because it measures power per round and **not** total
damage dealt: a routine at the top of the chart that reaches one program is
worth far less per cast than one halfway down that reaches five. Packet Shred
Everyone leads on both counts at once, which is exactly why it is a boss
routine and not something a player is ever taught.

Within a family the rate is where reaching wider gets paid for, and it falls
as the scope grows: Pipeline Stall runs 2.33, 1.50, 1.20 across its three
tiers, and Fork Bomb drops from 5.00 at Group to 1.60 at Everyone. You buy
reach with efficiency. Packet Shred is the one family that doesn't pay,
rising from 3.00 at Group v1.0 to 6.25 at Everyone — better per round as well
as wider — and the thing holding those tiers back is what it takes to learn
them rather than what they cost to cast.

Nothing here is *cheap*, because nothing here is bought. Every one of these
was priced in the player's Fatigue as well until 2026-08-08, including the
ones a companion ran; a routine now costs only the rounds it spends locked
away, so the question a player is answering has changed from "can I afford
this" to "is this the round to spend it". What marks out the first thing a
species grants is the bottom of the cooldown ladder: the routines that
recharge in a single round — `memory_leak`, `priority_boost`, `sandbox`,
`hot_patch` — are the weakest tier of their families, and three of the five
class utilities are one or two rounds behind them. So the opening move of a
fight is always available and never the best one.

**Nothing is granted at level 1**, and that is deliberate rather than an
accident of tuning. `priority_boost` is the fallback a companion falls back
on when its species has taught it nothing *yet*, and it is obtainable no
other way than by extracting it from one — so every species holding its
first entry back to level 2 is what keeps it reachable. It also means a
program you have just tamed reads as generic before it reads as its class.

## Field routines

These 10 do not run in battle at all. They are written onto Routine Disks
and cost **Power**. Most of them have no duration at all: they run until the
party rests, so they are bought at base as a loadout for a trip rather than
timed against a fight. The two that restore a pool over time keep a turn
count, because an unbounded one is unbounded healing or unbounded Power.

| Routine | Effect | Power | Duration | Costs |
|:---|:---|---:|---:|---:|
| Repair Loop Single | Regen | 2 | 300 turns | 18 |
| Trickle Charge Party | Trickle | 1 | 60 turns | 25 |
| Ablative Layer Single | Mitigation | 10 | until rest | 20 |
| Deep Scan Party | CaptureBoost | 20 | until rest | 18 |
| Hardened Shell Single | Def | 4 | until rest | 14 |
| Long Winter Party | Mitigation | 25 | until rest | 40 |
| Overclock Single | Atk | 4 | until rest | 14 |
| Salvage Routine Party | DropBoost | 20 | until rest | 18 |
| Stealth Protocol Party | EncounterDamp | 20 | until rest | 18 |
| Trace Analysis Party | XpBoost | 20 | until rest | 18 |

4 of them are not buffs in any combat sense — CaptureBoost, XpBoost,
DropBoost and EncounterDamp change the odds of a whole run rather than the
outcome of a fight, which is what Deep Analysis is buying at the far end of
the research tree. The other 6 are ordinary stat and regeneration work.

Getting one into a slot is where a known routine meets an item, and it takes
two steps. **Etching** burns a blank Routine Disk with a routine you know and
produces an etched disk; **installing** spends that etched disk on a slot.
Both spend last, after every refusal has cleared — there is no way to lose a
disk to a failed attempt. Uninstalling returns nothing, which is the whole
point: a slot is a commitment.

That split is also what makes the exclusive pool possible. An **exclusive**
routine is one nobody can learn and therefore nobody can etch — its disk
only ever arrives already written, off a boss's drop table or a Stack
trader's rare shelf row. Six ship: Kernel Shear, Null Cache and Deadman off
Wintermute; Hard Fault, Long Winter and Watchdog off the Overseer. Long
Winter is the field routine among them, which is why it sits at the top of
the table above with a Power cost nothing else comes near.

**Deadman and Watchdog are passives.** They occupy a slot, appear in no
menu, and fire on an event instead of a turn — Deadman when one of your own
goes down, Watchdog the moment a status condition lands on its holder. Their
cooldowns are their whole price; the Fatigue column reads 0 because a
passive is never cast.

## Movement routines

The other 2 run outside battle too, and are the only routines in the game
that still spend **Fatigue** — every other use of that meter went away when
Specials moved onto cooldowns. Both are Stack-only: they read and write the
party's frame coordinates, so they grey out with a reason on the open grid.

| Routine | Effect | Fatigue | What it does |
|:---|:---|---:|:---|
| Buffer Overrun Party | Phase | 12 | steps the party through one solid cell they are facing |
| Wild Jump Party | Jump | 20 | moves the party to any cell of the frame, and kills them if it is solid |

Wild Jump is the more expensive of the two because the landing is unvalidated
— that is the whole mechanic, not a missing check. Buffer Overrun refuses and
spends nothing when the rock runs deeper than one cell, when the far side is
off the frame, or when there is nothing solid ahead at all.

---

Source of truth is `assets/abilities/`. A mod that drops a `.ron` file in that
directory becomes grantable without a recompile, and will not appear above
until this page is regenerated -- edit the table at the top of
[`docs/abilities-gen.py`](abilities-gen.py) and run
`python3 docs/abilities-gen.py` from the repo root. The schema is documented
in [`assets/abilities/README.md`](../assets/abilities/README.md).
