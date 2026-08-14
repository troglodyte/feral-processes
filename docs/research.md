# Research tree

Every shipped research node in feral-processes, charted from its own file in
`assets/research/`. 23 of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/research/*.ron` on 2026-08-14 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

Research Data is the currency, and it comes from one place: a Research Node
structure worked by an assigned tamed program, the same way a Mining Node
produces Core Fragments. So the whole tree below is priced in *base uptime* —
it is the one progression track you cannot fight your way along.

| | |
|---|---|
| nodes | 23 |
| roots (need nothing) | 4 — Automation, Isometric Commerce, Contract Brokerage, Power Grid |
| deepest chain | 6 nodes |
| total Research Data | 1298 |
| cheapest / dearest node | 8 / 140 |
| zone bands | from turn one (11), zone 2 (6), zone 3 (6) |
| unlocks | 15 structures, 15 routines, 6 gear recipes |

## What the zone gates

Price is not the only thing pacing the tree. A node may declare a `min_zone`,
and below it the node is listed, priced and explained but unbuyable at any
balance — the visible tier *is* the reason to go breach. Research Data
survives a breach, so without this the whole tree could be finished without
ever opening a portal.

| Available | Nodes | Research Data | Which |
|:---|---:|---:|:---|
| from turn one | 11 | 198 | Automation, Contract Brokerage, Power Grid, Isometric Commerce, Self-Execution, Fortification, Field Operations, Reactive Armor, Weapon Fabrication, Routine Fabrication, Heap Allocation |
| zone 2 | 6 | 350 | Firewall Plating, Overclock Cores, Neural Interfacing, Runtime Patching, Adaptive Plating, Program Refactoring |
| zone 3 | 6 | 750 | Ablative Lattice, Monofilament Edge, Cortex Hacking, Deep Analysis, Kernel Privileges, Address Translation |

The gate and the tap compound without either knowing about the other.
`Game::upgrade_ceiling` caps a Research Node at Mk1 in zone 1, Mk2 in zone 2,
Mk3 in zone 3 — and its cycle succeeds 50% of the time at Mk1 against 90% at
Mk5. So the band you can buy earliest is also the band you earn slowest, and
each breach speeds the bank up at the same moment it releases more to spend
it on.

The bands are monotone in `requires`: a node is never gated below something
it depends on, or the prerequisite lock would always outlive the zone lock
and the gate could never be the reason the node was unbuyable. The one thing
that must never be gated is anything unlocking the Zone Portal — that is the
structure you reach the next zone *with*, so gating it behind the zone it
opens softlocks the run. No shipped node touches the portal at all; both
rules are asserted against the loaded tree in the engine's test suite.

## The tree

```
RESEARCH TREE            (Research Data to unlock each node)

Automation (8)
|-- Reactive Armor (24)
|   `-- Firewall Plating (45)
|       `-- Ablative Lattice (110)
|-- Weapon Fabrication (24)
|   |-- Neural Interfacing (55)
|   |   `-- Cortex Hacking (125)
|   `-- Overclock Cores (45)
|       `-- Monofilament Edge (110)
|-- Routine Fabrication (26)
|   `-- Self-Execution (14)
|       |-- Field Operations (20)
|       |   |-- Adaptive Plating (70)
|       |   `-- Deep Analysis (130)
|       |       `-- Address Translation (140)
|       `-- Runtime Patching (60)
|           `-- Kernel Privileges (135)
`-- Program Refactoring (75)

Isometric Commerce (14)

Contract Brokerage (10)

Power Grid (10)
|-- Fortification (18)
`-- Heap Allocation (30)
```

Three roots, and they are three different games. **Automation** is the trunk:
everything that makes a base do work hangs off it, and it is also the cheapest
node in the tree at 8, so the opening move is barely a
decision. **Power Grid** is a three-node stub: the two things that
keep a base standing, and the one that lets it grow.
**Isometric Commerce** is a leaf — 14 Research Data buys
the iso Market and leads nowhere, which makes it the one node you take purely
because you want the thing rather than the branch.

Under Automation the tree splits three ways and never rejoins: benches
(Reactive Armor, Weapon Fabrication) lead to **gear recipes**, and Routine
Fabrication leads to **routines**. Nothing in the tree requires two parents —
every `requires` is a single id — so this is a tree in the strict sense, and
there is no node you can reach two ways.

## What each node unlocks

| Node | Zone | Cost | Needs | Unlocks |
|:---|---:|---:|:---|:---|
| Automation | - | 8 | - | `compiler` |
| Contract Brokerage | - | 10 | - | `contract_broker` |
| Isometric Commerce | - | 14 | - | `market` |
| Power Grid | - | 10 | - | `power_conduit` |
| Fortification | - | 18 | `power_grid` | `shield`, `patch_node` |
| Heap Allocation | - | 30 | `power_grid` | `heap_pillar` |
| Program Refactoring | 2 | 75 | `automation` | `annealing_node`, `refactor_bench` |
| Reactive Armor | - | 24 | `automation` | `armory` |
| Routine Fabrication | - | 26 | `automation` | `log_scraper`, `lathe`, `transcriber`, `disk_press` |
| Weapon Fabrication | - | 24 | `automation` | `fabricator` |
| Firewall Plating | 2 | 45 | `armor_bench` | recipe `firewall_plating` at the armory — 6 `portal_fragment` |
| Neural Interfacing | 2 | 55 | `weapon_bench` | recipe `neural_amplifier` at the fabricator — 6 `portal_fragment` |
| Overclock Cores | 2 | 45 | `weapon_bench` | recipe `overclock_core` at the fabricator — 6 `portal_fragment` |
| Self-Execution | - | 14 | `routine_fabrication` | `priority_boost` |
| Ablative Lattice | 3 | 110 | `firewall` | recipe `ablative_plating` at the armory — 12 `portal_fragment` |
| Cortex Hacking | 3 | 125 | `neural_amp` | recipe `cortex_hack` at the fabricator — 12 `portal_fragment` |
| Field Operations | - | 20 | `self_exec` | `repair_loop`, `coolant_flush`, `trickle_charge` |
| Monofilament Edge | 3 | 110 | `overclock` | recipe `monofilament_whip` at the fabricator — 12 `portal_fragment` |
| Runtime Patching | 2 | 60 | `self_exec` | `hot_patch` |
| Adaptive Plating | 2 | 70 | `field_ops` | `hardened_shell`, `overclock`, `ablative_layer` |
| Deep Analysis | 3 | 130 | `field_ops` | `deep_scan`, `trace_analysis`, `stealth_protocol`, `salvage_routine` |
| Kernel Privileges | 3 | 135 | `runtime_patching` | `null_route` |
| Address Translation | 3 | 140 | `deep_analysis` | `buffer_overrun`, `wild_jump` |

A structure named by **no** research file is buildable from turn one — the
tree gates the machines that automate a base, not the base itself.

## What it costs to get there

A node's own `cost` is not what it costs you. Everything above it has to be
unlocked first, so the real price of Monofilament Edge is its own
110 plus the whole chain behind it.

```
CUMULATIVE COST FROM A STANDING START

Automation              8  #.......................................
Contract Brokerage     10  #.......................................
Power Grid             10  #.......................................
Isometric Commerce     14  ##......................................
Fortification          28  ###.....................................
Reactive Armor         32  ####....................................
Weapon Fabrication     32  ####....................................
Routine Fabrication    34  ####....................................
Heap Allocation        40  #####...................................
Self-Execution         48  ######..................................
Field Operations       68  ########................................
Firewall Plating       77  #########...............................
Overclock Cores        77  #########...............................
Program Refactoring    83  ##########..............................
Neural Interfacing     87  ##########..............................
Runtime Patching      108  #############...........................
Adaptive Plating      138  ################........................
Ablative Lattice      187  ######################..................
Monofilament Edge     187  ######################..................
Deep Analysis         198  #######################.................
Cortex Hacking        212  #########################...............
Kernel Privileges     243  #############################...........
Address Translation   338  ########################################
```

The shape to notice is the 6 end-of-branch nodes: Address Translation, Kernel Privileges, Cortex Hacking, Ablative Lattice, Monofilament Edge, Adaptive Plating.
Each carries 68-198 Research Data of prerequisites behind it before
its own price is counted, and lands at 138-338 from a
standing start — 2x the dearest single node in the
tree (140) at the top end. The tree is not steep; it is long, and the
zone bands are what stop that length being paid off in one sitting.

## Routines against recipes

The two halves of the tree pay in different currencies, and that is the
sharper divide than depth.

A **routine** node hands you the knowledge outright: unlock it and the
routines are yours to install, no materials involved. A **recipe** node hands
you the right to *build* something, and every one of the six is priced in
`portal_fragment` — the item a Stack lair guardian drops and nothing else
in the game does, and the same one that pays for a breach. So the recipe half
of the tree is priced in descents: every node on it competes directly with
the portal you are saving for.

| Recipe node | Builds | At | `portal_fragment` |
|:---|:---|:---|---:|
| Firewall Plating | `firewall_plating` | armory | 6 |
| Ablative Lattice | `ablative_plating` | armory | 12 |
| Neural Interfacing | `neural_amplifier` | fabricator | 6 |
| Cortex Hacking | `cortex_hack` | fabricator | 12 |
| Overclock Cores | `overclock_core` | fabricator | 6 |
| Monofilament Edge | `monofilament_whip` | fabricator | 12 |

So researched gear is deliberately expensive twice: once in base uptime to
learn it, and again in the currency you would otherwise have spent moving to
the next sector. Every one of the six also names a bench it must be built at,
which is a third gate — the research alone never puts the item in reach.

---

Source of truth is `assets/research/`. A mod that drops a `.ron` file in that
directory joins the tree without a recompile, and will not appear above until
this page is regenerated -- edit the table at the top of
[`docs/research-gen.py`](research-gen.py) and run
`python3 docs/research-gen.py` from the repo root. The schema is documented in
[`assets/research/README.md`](../assets/research/README.md).
