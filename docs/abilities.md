# Ability catalogue

Every shipped ability in feral-processes, charted from its own file in
`assets/abilities/`. Forty-one of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/abilities/*.ron` on 2026-08-05 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

A species grants abilities by naming their ids with a level to unlock each at;
`priority_boost` must exist, because it is the fallback for a companion whose
species grants nothing. The [research tree](research.md) teaches the rest.

| | |
|---|---|
| abilities | 41 |
| effect shapes | 8 |
| target shapes | 5 |
| field routines (run outside battle) | 10 |
| cost nothing | 5 |

## The naming scheme

An ability's **id** is flavour and its **name** is a spec. `kernel_panic`,
`cascade_overflow` and `broadcast_storm` sound like three unrelated things;
their names say Packet Shred Single, Packet Shred Group v1.0 and Packet Shred
Everyone, which is one effect at three scopes. A player reading a menu is
being told what the routine does and how wide it reaches, every time, in the
same word order.

```
ABILITY FAMILIES            (display name = effect + scope + tier)

Patch           Single v1.0 (8), Single v2.0 (25), Single v3.0 (50), Party v1.0 (8), Party v1.1 (10)
Packet Shred    Single (16), Group v1.0 (6), Group v2.0 (10), Everyone (25)
Hard Lock       Single v1.0 (0), Single v2.0 (0), Group (0), Everyone (0)
Bit Rot         Single (2), Group (3), Everyone (2)
Hyperthread     Single v1.0 (3), Single v2.0 (6), Party (3)
Bastion         Single (3), Party (4)
Pipeline Stall  Single (7), Everyone (6)
Fork Bomb       Single (9), Group (15)
Leech           Single (10), Group (6)

one of a kind: Ablative Layer, Coolant Flush, Decompile, Deep Scan, Etch, Flush Cache, Hardened Shell, Overclock, Repair Loop, Salvage Routine, Stealth Protocol, Throttle, Trace Analysis, Trickle Charge
```

The number in brackets is the effect's power. Read across a row and the
scaling rule is visible: reaching wider costs magnitude, and a `v2.0` at the
same scope is the straight upgrade. Nothing in the game names a routine after
what it is *called* rather than what it *does* — which is why the id column
exists at all, and why renaming an id never changes what a player reads.

## Who it hits against what it does

```
TARGET AGAINST EFFECT

                      Damag  Debuf   Buff   Heal  Drain  Field  Clean  Decom

OneAlly                   .      .      3      3      .      4      .      .
WholeParty                .      .      2      2      .      6      1      .
OneEnemyGroupFront        3      3      .      .      1      .      .      1
WholeEnemyGroup           3      2      2      .      1      .      .      .
AllEnemies                2      2      .      .      .      .      .      .

                          8      7      7      5      2     10      1      1
```

The grid is sparse on purpose. Heals and buffs point at allies, damage and
debuffs point at enemies. The one crossing is `Buff` aimed at an enemy group —
Etch and Throttle are buffs with **negative** power, so a sap is not a separate
effect shape but the same one run backwards. `Decompile` and `Cleanse` are one
of a kind apiece: taming is an ability rather than a separate verb, and
cleansing is the only routine that removes rather than adds.

## Everything

| Ability | Name | Target | Effect | Pow | Dur | Rider | CD | Cost |
|:---|:---|:---|:---|---:|---:|:---|---:|---:|
| `broadcast_storm` | Packet Shred Everyone | AllEnemies | Damage | 25 | - | - | 4 | 15 |
| `kernel_panic` | Packet Shred Single | OneEnemyGroupFront | Damage | 16 | - | - | 3 | 10 |
| `fork_bomb` | Fork Bomb Group | WholeEnemyGroup | Damage | 15 | - | Bleed 35% 2r | 3 | 12 |
| `packet_shred` | Packet Shred Group v2.0 | WholeEnemyGroup | Damage | 10 | - | - | 3 | 11 |
| `stack_smash` | Fork Bomb Single | OneEnemyGroupFront | Damage | 9 | - | Bleed 60% 3r | 2 | 8 |
| `pipeline_stall` | Pipeline Stall Single | OneEnemyGroupFront | Damage | 7 | - | Stun 40% 1r | 3 | 9 |
| `bus_fault` | Pipeline Stall Everyone | AllEnemies | Damage | 6 | - | Stun 25% 1r | 5 | 18 |
| `cascade_overflow` | Packet Shred Group v1.0 | WholeEnemyGroup | Damage | 6 | - | - | 2 | 8 |
| `heap_corruption` | Bit Rot Group | WholeEnemyGroup | Debuff Bleed | 3 | 3 | - | 3 | 11 |
| `bit_rot` | Bit Rot Everyone | AllEnemies | Debuff Bleed | 2 | 4 | - | 5 | 16 |
| `memory_leak` | Bit Rot Single | OneEnemyGroupFront | Debuff Bleed | 2 | 3 | - | 1 | - |
| `deadlock` | Hard Lock Single v1.0 | OneEnemyGroupFront | Debuff Stun | 0 | 1 | - | 2 | - |
| `hard_lock` | Hard Lock Single v2.0 | OneEnemyGroupFront | Debuff Stun | 0 | 2 | - | 4 | 10 |
| `null_route` | Hard Lock Everyone | AllEnemies | Debuff Stun | 0 | 1 | - | 5 | 15 |
| `race_condition` | Hard Lock Group | WholeEnemyGroup | Debuff Stun | 0 | 1 | - | 4 | 13 |
| `hyperthread` | Hyperthread Single v2.0 | OneAlly | Buff Atk | 6 | 4 | - | 3 | 8 |
| `bastion` | Bastion Party | WholeParty | Buff Def | 4 | 3 | - | 3 | 11 |
| `overclock_array` | Hyperthread Party | WholeParty | Buff Atk | 3 | 3 | - | 3 | 10 |
| `priority_boost` | Hyperthread Single v1.0 | OneAlly | Buff Atk | 3 | 3 | - | 1 | - |
| `sandbox` | Bastion Single | OneAlly | Buff Def | 3 | 3 | - | 1 | - |
| `etch` | Etch Group | WholeEnemyGroup | Buff Def | -4 | 3 | - | 3 | 10 |
| `throttle` | Throttle Group | WholeEnemyGroup | Buff Atk | -4 | 3 | - | 3 | 10 |
| `cold_boot` | Patch Single v3.0 | OneAlly | Heal | 50 | - | - | 5 | 15 |
| `checksum_repair` | Patch Single v2.0 | OneAlly | Heal | 25 | - | - | 3 | 9 |
| `redundancy_sync` | Patch Party v1.1 | WholeParty | Heal | 10 | - | - | 3 | 12 |
| `hot_patch` | Patch Single v1.0 | OneAlly | Heal | 8 | - | - | 1 | - |
| `mirror_restore` | Patch Party v1.0 | WholeParty | Heal | 8 | - | - | 2 | 10 |
| `siphon_cycles` | Leech Single | OneEnemyGroupFront | Drain | 10 | - | - | 2 | 9 |
| `leech_array` | Leech Group | WholeEnemyGroup | Drain | 6 | - | - | 4 | 13 |
| `deep_scan` | Deep Scan Party | WholeParty | FieldBuff CaptureBoost | 20 | 100 | - | - | 18 |
| `salvage_routine` | Salvage Routine Party | WholeParty | FieldBuff DropBoost | 20 | 100 | - | - | 18 |
| `stealth_protocol` | Stealth Protocol Party | WholeParty | FieldBuff EncounterDamp | 20 | 90 | - | - | 18 |
| `trace_analysis` | Trace Analysis Party | WholeParty | FieldBuff XpBoost | 20 | 100 | - | - | 18 |
| `ablative_layer` | Ablative Layer Single | OneAlly | FieldBuff Mitigation | 10 | 80 | - | - | 20 |
| `hardened_shell` | Hardened Shell Single | OneAlly | FieldBuff Def | 4 | 90 | - | - | 14 |
| `overclock` | Overclock Single | OneAlly | FieldBuff Atk | 4 | 90 | - | - | 14 |
| `repair_loop` | Repair Loop Single | OneAlly | FieldBuff Regen | 2 | 100 | - | - | 18 |
| `coolant_flush` | Coolant Flush Party | WholeParty | FieldBuff Coolant | 1 | 90 | - | - | 15 |
| `trickle_charge` | Trickle Charge Party | WholeParty | FieldBuff Trickle | 1 | 80 | - | - | 20 |
| `flush_cache` | Flush Cache Party | WholeParty | Cleanse | 0 | - | - | 3 | 7 |
| `decompile` | Decompile Single | OneEnemyGroupFront | Decompile | 0 | - | - | - | - |

## What a hit costs

```
DAMAGE PER POINT OF FATIGUE

Packet Shred Everyone      25 / 15   ############################## 1.67
Packet Shred Single        16 / 10   #############################. 1.60
Fork Bomb Group            15 / 12   ######################........ 1.25
Fork Bomb Single            9 / 8    ####################.......... 1.12
Packet Shred Group v2.0    10 / 11   ################.............. 0.91
Pipeline Stall Single       7 / 9    ##############................ 0.78
Packet Shred Group v1.0     6 / 8    #############................. 0.75
Pipeline Stall Everyone     6 / 18   ######........................ 0.33
```

Read this one carefully, because it measures power per point and **not** total
damage dealt: a routine at the top of the chart that reaches one program is
worth far less per cast than one halfway down that reaches five. Packet Shred
Everyone leads on both counts at once, which is exactly why it is a boss
routine and not something a player is ever taught.

Within a family the rate is the honest signal. Packet Shred runs from 0.75 at
Group v1.0 to 1.67 at Everyone, so the tiers are not merely wider — they are
better per point as well, and the thing holding them back is what it takes to
learn them rather than what they cost to cast.

Note the 5 routines that cost **nothing** — `deadlock`, `hot_patch`, `memory_leak`, `priority_boost`, `sandbox`.
Those are the starters, the ones a species grants at level 1 and the fallback
every companion has. A free routine is deliberately the weakest tier of its
family, so the opening move of a fight is always available and never the best
one.

## Field routines

These 10 do not run in battle at all. They are written onto Routine Disks
and cost **Power** rather than Fatigue, and their durations are measured in
turns of walking around rather than rounds of combat.

| Routine | Effect | Power | Duration | Costs |
|:---|:---|---:|---:|---:|
| Deep Scan Party | CaptureBoost | 20 | 100 turns | 18 |
| Repair Loop Single | Regen | 2 | 100 turns | 18 |
| Salvage Routine Party | DropBoost | 20 | 100 turns | 18 |
| Trace Analysis Party | XpBoost | 20 | 100 turns | 18 |
| Coolant Flush Party | Coolant | 1 | 90 turns | 15 |
| Hardened Shell Single | Def | 4 | 90 turns | 14 |
| Overclock Single | Atk | 4 | 90 turns | 14 |
| Stealth Protocol Party | EncounterDamp | 20 | 90 turns | 18 |
| Ablative Layer Single | Mitigation | 10 | 80 turns | 20 |
| Trickle Charge Party | Trickle | 1 | 80 turns | 20 |

4 of them are not buffs in any combat sense — CaptureBoost, XpBoost,
DropBoost and EncounterDamp change the odds of a whole run rather than the
outcome of a fight, which is what Deep Analysis is buying at the far end of
the research tree. The other five are ordinary stat and regeneration work,
just measured in turns.

Installing one is the one place a known routine meets an item, and the item is
spent **last**: the game checks battle, ownership, knowledge and a free slot
before it looks for the disk. Uninstalling returns nothing, which is the whole
point — a slot is a commitment.

---

Source of truth is `assets/abilities/`. A mod that drops a `.ron` file in that
directory becomes grantable without a recompile, and will not appear above
until this page is regenerated -- edit the table at the top of
[`docs/abilities-gen.py`](abilities-gen.py) and run
`python3 docs/abilities-gen.py` from the repo root. The schema is documented
in [`assets/abilities/README.md`](../assets/abilities/README.md).
