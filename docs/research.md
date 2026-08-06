# Research tree

Every shipped research node in feral-processes, charted from its own file in
`assets/research/`. Nineteen of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/research/*.ron` on 2026-08-05 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

Research Data is the currency, and it comes from one place: a Research Node
structure worked by an assigned tamed program, the same way a Mining Node
produces Core Fragments. So the whole tree below is priced in *base uptime* —
it is the one progression track you cannot fight your way along.

| | |
|---|---|
| nodes | 19 |
| roots (need nothing) | 3 — Automation, Isometric Commerce, Power Grid |
| deepest chain | 5 nodes |
| total Research Data | 477 |
| cheapest / dearest node | 8 / 48 |
| unlocks | 11 structures, 13 routines, 6 gear recipes |

## The tree

```
RESEARCH TREE            (Research Data to unlock each node)

Automation (8)
|-- Reactive Armor (18)
|   `-- Firewall Plating (22)
|       `-- Ablative Lattice (40)
|-- Weapon Fabrication (18)
|   |-- Neural Interfacing (25)
|   |   `-- Cortex Hacking (45)
|   `-- Overclock Cores (22)
|       `-- Monofilament Edge (40)
`-- Routine Fabrication (20)
    `-- Self-Execution (12)
        |-- Field Operations (16)
        |   |-- Adaptive Plating (32)
        |   `-- Deep Analysis (46)
        `-- Runtime Patching (28)
            `-- Kernel Privileges (48)

Isometric Commerce (12)

Power Grid (10)
`-- Fortification (15)
```

Three roots, and they are three different games. **Automation** is the trunk:
everything that makes a base do work hangs off it, and it is also the cheapest
node in the tree at 8, so the opening move is barely a
decision. **Power Grid** is a two-node stub that ends in defence.
**Isometric Commerce** is a leaf — 12 Research Data buys
the iso Market and leads nowhere, which makes it the one node you take purely
because you want the thing rather than the branch.

Under Automation the tree splits three ways and never rejoins: benches
(Reactive Armor, Weapon Fabrication) lead to **gear recipes**, and Routine
Fabrication leads to **routines**. Nothing in the tree requires two parents —
every `requires` is a single id — so this is a tree in the strict sense, and
there is no node you can reach two ways.

## What each node unlocks

| Node | Cost | Needs | Unlocks |
|:---|---:|:---|:---|
| Automation | 8 | - | `compiler` |
| Isometric Commerce | 12 | - | `market` |
| Power Grid | 10 | - | `power_conduit` |
| Fortification | 15 | `power_grid` | `shield`, `patch_node` |
| Reactive Armor | 18 | `automation` | `armory` |
| Routine Fabrication | 20 | `automation` | `log_scraper`, `lathe`, `transcriber`, `disk_press` |
| Weapon Fabrication | 18 | `automation` | `fabricator` |
| Firewall Plating | 22 | `armor_bench` | recipe `firewall_plating` at the armory — 6 `portal_fragment` |
| Neural Interfacing | 25 | `weapon_bench` | recipe `neural_amplifier` at the fabricator — 6 `portal_fragment` |
| Overclock Cores | 22 | `weapon_bench` | recipe `overclock_core` at the fabricator — 6 `portal_fragment` |
| Self-Execution | 12 | `routine_fabrication` | `priority_boost` |
| Ablative Lattice | 40 | `firewall` | recipe `ablative_plating` at the armory — 12 `portal_fragment` |
| Cortex Hacking | 45 | `neural_amp` | recipe `cortex_hack` at the fabricator — 12 `portal_fragment` |
| Field Operations | 16 | `self_exec` | `repair_loop`, `coolant_flush`, `trickle_charge` |
| Monofilament Edge | 40 | `overclock` | recipe `monofilament_whip` at the fabricator — 12 `portal_fragment` |
| Runtime Patching | 28 | `self_exec` | `hot_patch` |
| Adaptive Plating | 32 | `field_ops` | `hardened_shell`, `overclock`, `ablative_layer` |
| Deep Analysis | 46 | `field_ops` | `deep_scan`, `trace_analysis`, `stealth_protocol`, `salvage_routine` |
| Kernel Privileges | 48 | `runtime_patching` | `null_route` |

A structure named by **no** research file is buildable from turn one — the
tree gates the machines that automate a base, not the base itself.

## What it costs to get there

A node's own `cost` is not what it costs you. Everything above it has to be
unlocked first, so the real price of Monofilament Edge is its own 40 plus the
whole chain behind it.

```
CUMULATIVE COST FROM A STANDING START

Automation              8  ###.....................................
Power Grid             10  ###.....................................
Isometric Commerce     12  ####....................................
Fortification          25  #########...............................
Reactive Armor         26  #########...............................
Weapon Fabrication     26  #########...............................
Routine Fabrication    28  ##########..............................
Self-Execution         40  ##############..........................
Firewall Plating       48  #################.......................
Overclock Cores        48  #################.......................
Neural Interfacing     51  ##################......................
Field Operations       56  ###################.....................
Runtime Patching       68  #######################.................
Ablative Lattice       88  ##############################..........
Adaptive Plating       88  ##############################..........
Monofilament Edge      88  ##############################..........
Cortex Hacking         96  #################################.......
Deep Analysis         102  ###################################.....
Kernel Privileges     116  ########################################
```

The shape to notice is the 6 end-of-branch nodes: Kernel Privileges, Deep Analysis, Cortex Hacking, Adaptive Plating, Ablative Lattice, Monofilament Edge.
Each carries 48-68 Research Data of prerequisites behind it,
which is at least what the dearest single node in the whole tree costs on its
own (48). The tree is not steep at the top; it is long.

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

