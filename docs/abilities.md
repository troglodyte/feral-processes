# Ability catalogue

Every shipped ability in feral-processes, charted from its own file in
`assets/abilities/`. 85 of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/abilities/*.ron` on 2026-08-25 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

A species grants abilities by naming their ids with a level to unlock each at;
`priority_boost` must exist, because it is the fallback for a companion whose
species grants nothing. The [research tree](research.md) teaches the rest.

| | |
|---|---|
| abilities | 85 |
| effect shapes | 11 |
| target shapes | 5 |
| routines that never run in battle | 13 |
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
Segfault        Single v1.0 (6), Single v2.0 (11), Single v3.0 (17), Group (14), Everyone (28)
Skim            Single v1.0 (5), Single v2.0 (9), Single v3.0 (14), Group (4), Everyone (3)
Bastion         Single v1.0 (9), Single v2.0 (15), Single v3.0 (20), Party (12)
Packet Shred    Single (16), Group v1.0 (6), Group v2.0 (10), Everyone (25)
Hard Lock       Single v1.0 (0), Single v2.0 (0), Group (0), Everyone (0)
Etch            Single (-15), Group (-12), Everyone (-9)
Pipeline Stall  Single (7), Group (6), Everyone (6)
Throttle        Single (-5), Group (-4), Everyone (-3)
Leech           Single (10), Group (6), Everyone (4)
Fork Bomb       Single (9), Group (15), Everyone (8)
Hyperthread     Single v1.0 (3), Single v2.0 (6), Party (3)
Rollback        Single v1.0 (10), Single v2.0 (20), Single v3.0 (35)
Row Hammer      Single (7), Group (6), Everyone (5)
Flush Cache     Single (0), Party (0)
Hardened Shell  Single (12), Party (12)

one of a kind: Ablative Layer, Buffer Overrun, Clock Skew, Core Dump, Deadman, Decompile, Deep Scan, Hard Fault, Hot Spare, Interrupt, Kernel Shear, Long Winter, Null Cache, Overclock, Parity, Quarantine, Repair Loop, Salvage Routine, Snoop, Stealth Protocol, Symlink, Trace Analysis, Trickle Charge, Watchdog, Wild Jump
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

                      Damag  Debuf   Buff   Heal  Drain  Field  Clean  Decom  Phase   Jump  Symli

OneAlly                   .      .      6      7      .      4      2      .      .      .      .
WholeParty                .      .      2      2      .      7      2      .      1      1      1
OneEnemyGroupFront        9      6      2      .      4      .      .      1      .      .      .
WholeEnemyGroup           7      2      2      .      3      .      .      .      .      .      .
AllEnemies                6      3      2      .      3      .      .      .      .      .      .

                         22     11     14      9     10     11      4      1      1      1      1
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

| Ability | Name | Target | Effect | Pow | Dur | Rider | CD | PWR |
|:---|:---|:---|:---|---:|---:|:---|---:|---:|
| `segfault_everyone` | Segfault Everyone | AllEnemies | Damage | 16–40 | - | - | 5 | 18 |
| `broadcast_storm` | Packet Shred Everyone | AllEnemies | Damage | 19–31 | - | - | 4 | 15 |
| `kernel_shear` | Kernel Shear Group | WholeEnemyGroup | Damage | 16–28 | - | Bleed 75% 4r | 4 | 16 |
| `segfault_v3` | Segfault Single v3.0 | OneEnemyGroupFront | Damage | 13–21 | - | - | 4 | 10 |
| `kernel_panic` | Packet Shred Single | OneEnemyGroupFront | Damage | 12–20 | - | - | 3 | 10 |
| `fork_bomb` | Fork Bomb Group | WholeEnemyGroup | Damage | 11–19 | - | Bleed 35% 2r | 3 | 12 |
| `deadman` | Deadman Everyone | AllEnemies | Damage | 10–18 | - | - | 4 | - |
| `segfault_group` | Segfault Group | WholeEnemyGroup | Damage | 6–22 | - | - | 3 | 11 |
| `segfault_v2` | Segfault Single v2.0 | OneEnemyGroupFront | Damage | 8–14 | - | - | 3 | 8 |
| `packet_shred` | Packet Shred Group v2.0 | WholeEnemyGroup | Damage | 8–12 | - | - | 3 | 11 |
| `core_dump` | Core Dump Single | OneEnemyGroupFront | Damage | 7–11 | - | - | 3 | - |
| `stack_smash` | Fork Bomb Single | OneEnemyGroupFront | Damage | 7–11 | - | Bleed 60% 3r | 2 | 8 |
| `pid_exhaustion` | Fork Bomb Everyone | AllEnemies | Damage | 6–10 | - | Bleed 20% 2r | 5 | 18 |
| `pipeline_stall` | Pipeline Stall Single | OneEnemyGroupFront | Damage | 5–9 | - | Stun 40% 1r | 3 | 9 |
| `row_hammer_single` | Row Hammer Single | OneEnemyGroupFront | Damage | 6–8 | - | - | 2 | 7 |
| `branch_hazard` | Pipeline Stall Group | WholeEnemyGroup | Damage | 4–8 | - | Stun 30% 1r | 4 | 13 |
| `bus_fault` | Pipeline Stall Everyone | AllEnemies | Damage | 4–8 | - | Stun 25% 1r | 5 | 18 |
| `cascade_overflow` | Packet Shred Group v1.0 | WholeEnemyGroup | Damage | 4–8 | - | - | 2 | 8 |
| `row_hammer_group` | Row Hammer Group | WholeEnemyGroup | Damage | 5–7 | - | - | 3 | 10 |
| `segfault_v1` | Segfault Single v1.0 | OneEnemyGroupFront | Damage | 4–8 | - | - | 2 | 6 |
| `interrupt_request` | Interrupt Single | OneEnemyGroupFront | Damage | 4–6 | - | - | 4 | - |
| `row_hammer_everyone` | Row Hammer Everyone | AllEnemies | Damage | 4–6 | - | - | 4 | 15 |
| `bit_rot_v3` | Bit Rot Single v3.0 | OneEnemyGroupFront | Debuff Bleed | 6 | 4 | - | 3 | 9 |
| `bit_rot_v2` | Bit Rot Single v2.0 | OneEnemyGroupFront | Debuff Bleed | 4 | 3 | - | 2 | 7 |
| `heap_corruption` | Bit Rot Group | WholeEnemyGroup | Debuff Bleed | 3 | 3 | - | 3 | 11 |
| `bit_rot` | Bit Rot Everyone | AllEnemies | Debuff Bleed | 2 | 4 | - | 5 | 16 |
| `clock_skew` | Clock Skew Single | OneEnemyGroupFront | Debuff Bleed | 2 | 2 | - | 4 | - |
| `memory_leak` | Bit Rot Single v1.0 | OneEnemyGroupFront | Debuff Bleed | 2 | 3 | - | 1 | 5 |
| `deadlock` | Hard Lock Single v1.0 | OneEnemyGroupFront | Debuff Stun | 0 | 1 | - | 2 | 6 |
| `hard_fault` | Hard Fault Everyone | AllEnemies | Debuff Stun | 0 | 2 | - | 5 | 20 |
| `hard_lock` | Hard Lock Single v2.0 | OneEnemyGroupFront | Debuff Stun | 0 | 2 | - | 4 | 10 |
| `null_route` | Hard Lock Everyone | AllEnemies | Debuff Stun | 0 | 1 | - | 5 | 15 |
| `race_condition` | Hard Lock Group | WholeEnemyGroup | Debuff Stun | 0 | 1 | - | 4 | 13 |
| `bastion_shield_v3` | Bastion Single v3.0 | OneAlly | Buff Mitigation | 20 | 4 | - | 2 | 9 |
| `bastion_shield_v2` | Bastion Single v2.0 | OneAlly | Buff Mitigation | 15 | 3 | - | 2 | 7 |
| `bastion` | Bastion Party | WholeParty | Buff Mitigation | 12 | 3 | - | 3 | 11 |
| `parity_guard` | Parity Single | OneAlly | Buff Mitigation | 9 | 3 | - | 4 | - |
| `sandbox` | Bastion Single v1.0 | OneAlly | Buff Mitigation | 9 | 3 | - | 1 | 5 |
| `hyperthread` | Hyperthread Single v2.0 | OneAlly | Buff Atk | 6 | 4 | - | 3 | 8 |
| `overclock_array` | Hyperthread Party | WholeParty | Buff Atk | 3 | 3 | - | 3 | 10 |
| `priority_boost` | Hyperthread Single v1.0 | OneAlly | Buff Atk | 3 | 3 | - | 1 | 5 |
| `brownout` | Throttle Everyone | AllEnemies | Buff Atk | -3 | 3 | - | 5 | 16 |
| `throttle` | Throttle Group | WholeEnemyGroup | Buff Atk | -4 | 3 | - | 3 | 10 |
| `clock_gate` | Throttle Single | OneEnemyGroupFront | Buff Atk | -5 | 3 | - | 2 | 8 |
| `oxide_strip` | Etch Everyone | AllEnemies | Buff Mitigation | -9 | 3 | - | 5 | 16 |
| `etch` | Etch Group | WholeEnemyGroup | Buff Mitigation | -12 | 3 | - | 3 | 10 |
| `acid_wash` | Etch Single | OneEnemyGroupFront | Buff Mitigation | -15 | 3 | - | 2 | 8 |
| `cold_boot` | Patch Single v3.0 | OneAlly | Heal | 38–62 | - | - | 5 | 15 |
| `rollback_v3` | Rollback Single v3.0 | OneAlly | Heal | 26–44 | - | - | 4 | 10 |
| `checksum_repair` | Patch Single v2.0 | OneAlly | Heal | 19–31 | - | - | 3 | 9 |
| `rollback_v2` | Rollback Single v2.0 | OneAlly | Heal | 15–25 | - | - | 3 | 8 |
| `redundancy_sync` | Patch Party v1.1 | WholeParty | Heal | 8–12 | - | - | 3 | 12 |
| `rollback_v1` | Rollback Single v1.0 | OneAlly | Heal | 8–12 | - | - | 2 | 6 |
| `hot_patch` | Patch Single v1.0 | OneAlly | Heal | 6–10 | - | - | 1 | 5 |
| `hot_spare` | Hot Spare Single | OneAlly | Heal | 6–10 | - | - | 3 | - |
| `mirror_restore` | Patch Party v1.0 | WholeParty | Heal | 6–10 | - | - | 2 | 10 |
| `skim_v3` | Skim Single v3.0 | OneEnemyGroupFront | Drain | 10–18 | - | - | 4 | 10 |
| `null_cache` | Null Cache Group | WholeEnemyGroup | Drain | 9–15 | - | - | 3 | 18 |
| `siphon_cycles` | Leech Single | OneEnemyGroupFront | Drain | 8–12 | - | - | 2 | 9 |
| `bus_snoop` | Snoop Everyone | AllEnemies | Drain | 7–11 | - | - | 5 | 20 |
| `skim_v2` | Skim Single v2.0 | OneEnemyGroupFront | Drain | 7–11 | - | - | 3 | 8 |
| `leech_array` | Leech Group | WholeEnemyGroup | Drain | 4–8 | - | - | 4 | 13 |
| `skim_v1` | Skim Single v1.0 | OneEnemyGroupFront | Drain | 4–6 | - | - | 2 | 6 |
| `cycle_harvest` | Leech Everyone | AllEnemies | Drain | 3–5 | - | - | 5 | 17 |
| `skim_group` | Skim Group | WholeEnemyGroup | Drain | 3–5 | - | - | 3 | 8 |
| `skim_everyone` | Skim Everyone | AllEnemies | Drain | 2–4 | - | - | 4 | 15 |
| `long_winter` | Long Winter Party | WholeParty | FieldBuff Mitigation | 25 | - | - | - | 40 |
| `deep_scan` | Deep Scan Party | WholeParty | FieldBuff CaptureBoost | 20 | - | - | - | 18 |
| `salvage_routine` | Salvage Routine Party | WholeParty | FieldBuff DropBoost | 20 | - | - | - | 18 |
| `stealth_protocol` | Stealth Protocol Party | WholeParty | FieldBuff EncounterDamp | 20 | - | - | - | 18 |
| `trace_analysis` | Trace Analysis Party | WholeParty | FieldBuff XpBoost | 20 | - | - | - | 18 |
| `hardened_shell` | Hardened Shell Single | OneAlly | FieldBuff Mitigation | 12 | - | - | - | 14 |
| `hardened_shell_party` | Hardened Shell Party | WholeParty | FieldBuff Mitigation | 12 | - | - | - | 32 |
| `ablative_layer` | Ablative Layer Single | OneAlly | FieldBuff Mitigation | 10 | - | - | - | 20 |
| `overclock` | Overclock Single | OneAlly | FieldBuff Atk | 4 | - | - | - | 14 |
| `repair_loop` | Repair Loop Single | OneAlly | FieldBuff Regen | 2 | 300 | - | - | 18 |
| `trickle_charge` | Trickle Charge Party | WholeParty | FieldBuff Trickle | 1 | 60 | - | - | 25 |
| `flush_cache` | Flush Cache Party | WholeParty | Cleanse | 0 | - | - | 3 | 7 |
| `invalidate_line` | Flush Cache Single | OneAlly | Cleanse | 0 | - | - | 2 | 4 |
| `quarantine` | Quarantine Single | OneAlly | Cleanse | 0 | - | - | 4 | - |
| `watchdog` | Watchdog Party | WholeParty | Cleanse | 0 | - | - | 4 | - |
| `decompile` | Decompile Single | OneEnemyGroupFront | Decompile | 0 | - | - | - | 1 |
| `buffer_overrun` | Buffer Overrun Party | WholeParty | Phase | 0 | - | - | - | 12 |
| `wild_jump` | Wild Jump Party | WholeParty | Jump | 0 | - | - | - | 20 |
| `symlink` | Symlink Party | WholeParty | Symlink | 0 | - | - | - | 25 |

A routine costs two things at once, and both columns above are real. **CD** is
rounds before the same combatant can run it again, cleared when the battle
ends. **PWR** comes off the invoker's own reserve — a companion's Special
spends the companion's Power, not yours — and only rest refills one. The
picker greys a row that fails either test, with the reason on it.

The exceptions are worth knowing because they are the whole of the pattern.
A **passive** shows no PWR: it fires on a trigger rather than being run, and
`cooldown` is its entire price. A **hostile** carrier is never charged, since
nothing on the wild side holds a reserve at all. And the wielded program's
proc is free by design — its 25% rate is what it pays instead.

## What a hit costs

```
DAMAGE PER ROUND OF COOLDOWN

Packet Shred Everyone      25 / 4    ############################## 6.25
Segfault Everyone          28 / 5    ###########################... 5.60
Kernel Shear Group         22 / 4    ##########################.... 5.50
Packet Shred Single        16 / 3    ##########################.... 5.33
Fork Bomb Group            15 / 3    ########################...... 5.00
Segfault Group             14 / 3    ######################........ 4.67
Fork Bomb Single            9 / 2    ######################........ 4.50
Segfault Single v3.0       17 / 4    ####################.......... 4.25
Segfault Single v2.0       11 / 3    ##################............ 3.67
Deadman Everyone           14 / 4    #################............. 3.50
Row Hammer Single           7 / 2    #################............. 3.50
Packet Shred Group v2.0    10 / 3    ################.............. 3.33
Packet Shred Group v1.0     6 / 2    ##############................ 3.00
Core Dump Single            9 / 3    ##############................ 3.00
Segfault Single v1.0        6 / 2    ##############................ 3.00
Pipeline Stall Single       7 / 3    ###########................... 2.33
Row Hammer Group            6 / 3    ##########.................... 2.00
Fork Bomb Everyone          8 / 5    ########...................... 1.60
Pipeline Stall Group        6 / 4    #######....................... 1.50
Interrupt Single            5 / 4    ######........................ 1.25
Row Hammer Everyone         5 / 4    ######........................ 1.25
Pipeline Stall Everyone     6 / 5    ######........................ 1.20
```

Read this one carefully, because it measures power per round and **not** total
damage dealt: a routine at the top of the chart that reaches one program is
worth far less per run than one halfway down that reaches five. Packet Shred
Everyone leads on both counts at once, which is why no research node teaches
it and no wild carrier rolls it — it is the two bosses' innate, and the only
way to meet it is to be on the wrong end of it.

Segfault Everyone is the one directly behind it on both axes and *is*
learnable, which is what the family is for: 16-40 against Packet Shred
Everyone's 19-31, bought with a fifth round of cooldown, three more Power and
all but one point of aim. A better ceiling and a worse floor at the same
reach — the widening band is Segfault's whole identity, and it is what keeps
the family from being a second copy of a ladder that already exists.

Within a family the rate is where reaching wider gets paid for, and it falls
as the scope grows: Pipeline Stall runs 2.33, 1.50, 1.20 across its three
tiers, and Fork Bomb drops from 5.00 at Group to 1.60 at Everyone. You buy
reach with efficiency. Two families don't, and they buy their way out of it
differently. Packet Shred rises from 3.00 at Group v1.0 to 6.25 at Everyone —
better per round as well as wider — and what holds those tiers back is what it
takes to learn them rather than what they cost to run, the top one being a
boss's innate and nothing else's. Segfault climbs the same way, 3.00 to 5.60
across five tiers, and is learnable at every rung; what it pays instead is the
band. Its floor gets worse as its ceiling gets better, so the rate this chart
measures is a mean the routine will often miss on the low side.

A routine is bought twice over: once in the rounds it spends locked away,
and once out of the reserve of whoever ran it. Until 2026-08-08 the second
half came off *the player's* meter even when a companion was the one acting,
which rationed the party's own kit against a pool only the player had; it
comes off the invoker now, which is what makes a levelled companion's Special
its own to spend. What marks out the first thing a species grants is the
bottom of the cooldown ladder: the routines that
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

These 11 do not run in battle at all. They are written onto Routine Disks
and cost **Power**. Most of them have no duration at all: they run until the
party rests, so they are bought at base as a loadout for a trip rather than
timed against a fight. The two that restore a pool over time keep a turn
count, because an unbounded one is unbounded healing or unbounded Power.

They are no longer the only things the map's routine list offers. A **Heal**
that charges Power runs out there too, on top of being a Special — all eight
Patch and Rollback routines, `hot_patch` included since it was priced. A heal
is priced in Power and nothing else out there, because a cooldown counts
battle rounds and the map has no round to count; a heal costing nothing would
therefore have no throttle at all, and would stay a Special. Everything else
about it is the battle invocation's: the same band, scaled by the invoker's
own level and Heal affinity, restoring what fits under the target's ceiling.

| Routine | Effect | Power | Duration | Costs |
|:---|:---|---:|---:|---:|
| Repair Loop Single | Regen | 2 | 300 turns | 18 |
| Trickle Charge Party | Trickle | 1 | 60 turns | 25 |
| Ablative Layer Single | Mitigation | 10 | until rest | 20 |
| Deep Scan Party | CaptureBoost | 20 | until rest | 18 |
| Hardened Shell Party | Mitigation | 12 | until rest | 32 |
| Hardened Shell Single | Mitigation | 12 | until rest | 14 |
| Long Winter Party | Mitigation | 25 | until rest | 40 |
| Overclock Single | Atk | 4 | until rest | 14 |
| Salvage Routine Party | DropBoost | 20 | until rest | 18 |
| Stealth Protocol Party | EncounterDamp | 20 | until rest | 18 |
| Trace Analysis Party | XpBoost | 20 | until rest | 18 |

4 of them are not buffs in any combat sense — CaptureBoost, XpBoost,
DropBoost and EncounterDamp change the odds of a whole run rather than the
outcome of a fight, which is what Deep Analysis is buying at the far end of
the research tree. The other 7 are ordinary stat and regeneration work.

Getting one into a slot is where a known routine meets an item, and it takes
two steps. **Etching** burns a blank Routine Disk with a routine you know and
produces an etched disk; **installing** spends that etched disk on a slot.
Both spend last, after every refusal has cleared — there is no way to lose a
disk to a failed attempt. Uninstalling returns nothing, which is the whole
point: a slot is a commitment.

That split is also what makes the exclusive pool possible. An **exclusive**
routine is one nobody can learn and therefore nobody can etch — its disk
only ever arrives already written, off a boss's drop table or a Stack
trader's rare shelf row. Seven ship: Kernel Shear, Null Cache and Deadman off
Wintermute; Hard Fault, Long Winter, Watchdog and Snoop off the Overseer. The
two bosses' tables roll independently, so the Overseer holding a fourth costs
the other three nothing. Long Winter is the field routine among them, which is
why it sits at the top of the table above with a Power cost nothing else comes
near.

**Eight of them are passives.** They occupy a slot, appear in no menu, and
fire on an event instead of a turn. Their cooldowns really are their whole
price — the PWR column reads 0 because `Game::fire_passives` charges nothing,
and a passive is the only kind of routine that is genuinely free to run.

| Passive | Fires on | And then |
|:---|:---|:---|
| Clock Skew Single | the round opening | the nearest hostile starts bleeding |
| Interrupt Single | the round opening | the nearest hostile takes a small hit |
| Parity Single | the round opening | the wearer's own mitigation goes up |
| Core Dump Single | its holder driven low | the nearest hostile takes a large hit |
| Hot Spare Single | its holder driven low | the holder patches itself |
| Deadman Everyone | one of yours going down | everything hostile takes the fallout |
| Quarantine Single | a condition landing | the wearer sheds it |
| Watchdog Party | a condition landing | the whole party is cleared |

Read the table by trigger rather than by effect. `RoundStart` fires every
round there is, which is why all three of those are priced slow as well as
low. The two `AllyWounded` rungs are the crossing worth noticing: `core_dump`
answers the crisis by hitting back and `hot_spare` by patching, and the heal
is the smaller number on purpose — a heal on the way down buys the round the
crisis is supposed to be survivable in. Neither is `AllyDropped`, which only
`deadman` uses: a dropped companion is gone for good at every difficulty, so
a routine paying out there pays a player who has already lost more than the
payout is worth.

## Movement routines

The other 2 run outside battle too. They were the last routines still
priced in the retired Fatigue meter, and are denominated in the same Power as
everything else now — which is the only reason their numbers can be compared
with the tables above at all. Both are Stack-only: they read and write the party's frame coordinates, so they grey
out with a reason on the open grid.

| Routine | Effect | Power | What it does |
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
