# Roster stat sheet

Every shipped species in feral-processes, charted from its own file in
`assets/species/`. Seventeen of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/species/*.ron` on 2026-08-11 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind. Where a
file omits a field, the engine default from `crates/engine/src/tuning.rs` is
shown: `base_speed 10`, `base_int 10`, `growth_multiplier 1.0`, all five
affinities `1.0`.

POW is the engine's own scalar, `Stats::power` — `max_hp + atk + def`,
unweighted. It is what `difficulty_color` reads to decide whether a program
shows up green or red on your map. Every table below is in POW order.

| | |
|---|---|
| species | 17 |
| bosses | 2 (Wintermute, Overseer) |
| nest builders | 4 (Scrapper, Trojan, Worm, Crawler) |
| work yields | 8 — 7 `core_fragment`, 1 `power_cell` |
| HP span | 38 (Glitch) to 200 (Wintermute) |
| speed span | 6 (Construct) to 14 (Sprite); the player rolls from 11 |
| extraction span | 5 (Construct, Glitch) to 15 (SubProcess), non-boss; the player works at 10 |

## Core stats

|  | Species | HP | ATK | DEF | SPD | INT | POW | Tame | Growth |
|:---|:---|---:|---:|---:|---:|---:|---:|---:|---:|
| `W` | Wintermute **·boss** | 200 | 19 | 17 | 13 | 18 | 236 | 0.95 | x2.00 |
| `B` | Overseer **·boss** | 180 | 17 | 15 | 12 | 16 | 212 | 0.90 | x2.00 |
| `S` | Sentinel | 136 | 6 | 12 | 7 | 8 | 154 | 0.65 | x1.50 |
| `k` | Rootkit | 132 | 12 | 3 | 9 | 13 | 147 | 0.75 | x1.50 |
| `v` | Virus | 122 | 10 | 8 | 12 | 12 | 140 | 0.60 | x1.50 |
| `c` | Cipher | 113 | 15 | 5 | 13 | 14 | 133 | 0.80 | x1.50 |
| `z` | ZeroDay | 106 | 16 | 4 | 10 | 12 | 126 | 0.65 | x1.50 |
| `r` | Crawler | 102 | 5 | 9 | 7 | 8 | 116 | 0.50 | x1.25 |
| `m` | Worm | 99 | 9 | 2 | 9 | 11 | 110 | 0.40 | x1.25 |
| `p` | Proxy | 92 | 7 | 6 | 12 | 13 | 105 | 0.55 | x1.25 |
| `t` | Trojan | 85 | 11 | 4 | 13 | 13 | 100 | 0.50 | x1.25 |
| `x` | Scrapper | 80 | 12 | 3 | 10 | 7 | 95 | 0.45 | x1.25 |
| `C` | Construct | 49 | 2 | 4 | 6 | 5 | 55 | 0.35 | x1.00 |
| `o` | Drone | 48 | 4 | 1 | 8 | 7 | 53 | 0.15 | x1.00 |
| `d` | SubProcess | 43 | 4 | 3 | 12 | 15 | 50 | 0.30 | x1.00 |
| `s` | Sprite | 41 | 5 | 2 | 14 | 11 | 48 | 0.20 | x1.00 |
| `g` | Glitch | 38 | 6 | 1 | 11 | 5 | 45 | 0.15 | x1.00 |

## Attack against defense

Each species sits at its own `(base_atk, base_def)`, drawn with its map glyph.
The top-right corner belongs to the two bosses alone, and the bottom-left to
the five programs the opening ring draws from. Between them the roster is laid
out by **class**: a species' position here is its role, and its distance from
the origin is its tier.

```
BASE DEF
18 |+ - - - + - - - + - - - + - - - + - - - +
17 ||       |       |       |       |     W |
16 ||       |       |       |       |       |
15 |+ - - - + - - - + - - - + - - - + B - - +
14 ||       |       |       |       |       |
13 ||       |       |       |       |       |
12 |+ - - - + - S - + - - - + - - - + - - - +
11 ||       |       |       |       |       |
10 ||       |       |       |       |       |
 9 |+ - - - + r - - + - - - + - - - + - - - +
 8 ||       |       |   v   |       |       |
 7 ||       |       |       |       |       |
 6 |+ - - - + - - p + - - - + - - - + - - - +
 5 ||       |       |       |     c |       |
 4 ||   C   |       |     t |       z       |
 3 |+ - - - d - - - + - - - x - - - + - - - +
 2 ||       | s     | m     |       |       |
 1 ||       o   g   |       |       |       |
 0 |+ - - - + - - - + - - - + - - - + - - - +
   +------------------------------------------
    0       4       8       12      16      20
    BASE ATK
```

Each species spends its growth band's stat budget on a class share, so the
five programs of any one tier all cost the same and differ only in shape. The
extremes are ZeroDay (ATK 16 on DEF 4, the highest attack outside a boss
carried on almost nothing) and Sentinel (DEF 12 on ATK 6, which is the same
trade read backwards). Because the shares are constant across tiers, that
contrast repeats at every rung: Scrapper against Crawler, and Glitch against
Construct, are the same two shapes at a third of the size.

## Stat profiles

One row order throughout, so a species' shape across the four charts is its
character. Each chart is scaled against the roster maximum for that stat, not
against the others — the HP bars are not comparable to the ATK bars.

```
BASE HP                             (max 200)

Wintermute 200  ████████████████████████████████████████████
Overseer   180  ████████████████████████████████████████····
Sentinel   136  ██████████████████████████████··············
Rootkit    132  █████████████████████████████···············
Virus      122  ███████████████████████████·················
Cipher     113  █████████████████████████···················
ZeroDay    106  ███████████████████████·····················
Crawler    102  ██████████████████████······················
Worm        99  ██████████████████████······················
Proxy       92  ████████████████████························
Trojan      85  ███████████████████·························
Scrapper    80  ██████████████████··························
Construct   49  ███████████·································
Drone       48  ███████████·································
SubProcess  43  █████████···································
Sprite      41  █████████···································
Glitch      38  ████████····································
```

```
BASE ATK                             (max 19)

Wintermute  19  ████████████████████████████████████████████
Overseer    17  ███████████████████████████████████████·····
Sentinel     6  ██████████████······························
Rootkit     12  ████████████████████████████················
Virus       10  ███████████████████████·····················
Cipher      15  ███████████████████████████████████·········
ZeroDay     16  █████████████████████████████████████·······
Crawler      5  ████████████································
Worm         9  █████████████████████·······················
Proxy        7  ████████████████····························
Trojan      11  █████████████████████████···················
Scrapper    12  ████████████████████████████················
Construct    2  █████·······································
Drone        4  █████████···································
SubProcess   4  █████████···································
Sprite       5  ████████████································
Glitch       6  ██████████████······························
```

```
BASE DEF                             (max 17)

Wintermute  17  ████████████████████████████████████████████
Overseer    15  ███████████████████████████████████████·····
Sentinel    12  ███████████████████████████████·············
Rootkit      3  ████████····································
Virus        8  █████████████████████·······················
Cipher       5  █████████████·······························
ZeroDay      4  ██████████··································
Crawler      9  ███████████████████████·····················
Worm         2  █████·······································
Proxy        6  ████████████████····························
Trojan       4  ██████████··································
Scrapper     3  ████████····································
Construct    4  ██████████··································
Drone        1  ███·········································
SubProcess   3  ████████····································
Sprite       2  █████·······································
Glitch       1  ███·········································
```

```
BASE SPEED

14  Sprite
13  Wintermute, Cipher, Trojan
12  Overseer, Virus, Proxy, SubProcess
11  Glitch   <- the player rolls from here
10  ZeroDay, Scrapper
 9  Rootkit, Worm
 8  Drone
 7  Sentinel, Crawler
 6  Construct
```

Speed is the fourth axis of a species' class rather than a fifth stat: the
three Saboteurs sit at the top, the three Bastions at the bottom, and the
ladder repeats itself once per tier. Both bosses sit near the top regardless,
which is the one place the roster does not trade power for pace.
Speed is an initiative baseline rather than a turn order, though — every
combatant rolls `base_speed + d10` each round, so a 4-point gap still loses
sometimes. The same number sets a posted program's pace at a machine too: a
cycle scales faster above `DEFAULT_BASE_SPEED` and slower below it, so a
Sprite mining a Mk1 node finishes cycles a fifth quicker than the roster
baseline while a Construct takes a fifth longer than that same baseline (see [`assets/species/README.md`](../assets/species/README.md)).

## Taming cost against growth tier

`taming_difficulty` along the axis, `growth_multiplier` as the lane. A tie
shifts one glyph right, so read the lanes for grouping rather than for an
exact position.

```
GROWTH
x2.00  |                                      B  W
x1.50  |                        v Sz   k c
x1.25  |              m  x rt p
x1.00  |  og s    d C
       +--------------------------------------------
        0.10            0.55               0.95
        TAMING DIFFICULTY
```

The two move together across the whole roster. Nothing is cheap to compile and
steep to level, and nothing is expensive and flat — so taming difficulty is a
straight read on long-term value, with no bargains and no traps.

## Habitats and yield

Four walkable biomes host spawns. `DataVoid` and `BlackIce` are barrier terrain
and `Platform` is base floor — no shipped species lists any of the three, which
is exactly what keeps a player's base free of wild spawns.

|  | Species | OpenGrid | Mainframe | NullSector | Deadlock | Yield |
|:---|:---|:---|:---|:---|:---|:---|
| `W` | Wintermute | # | # | # | # | - |
| `B` | Overseer | # | # | # | # | - |
| `S` | Sentinel | . | . | . | # | - |
| `k` | Rootkit | . | # | # | . | - |
| `v` | Virus | . | # | # | . | `core_fragment` |
| `c` | Cipher | . | # | . | # | - |
| `z` | ZeroDay | . | . | # | # | - |
| `r` | Crawler | . | . | . | # | - |
| `m` | Worm | # | . | # | . | `core_fragment` |
| `p` | Proxy | . | # | . | # | - |
| `t` | Trojan | # | # | . | . | - |
| `x` | Scrapper | # | . | # | . | `core_fragment` |
| `C` | Construct | . | # | . | . | `core_fragment` |
| `o` | Drone | # | # | . | . | `core_fragment` |
| `d` | SubProcess | # | . | # | . | `core_fragment` |
| `s` | Sprite | # | # | . | . | `core_fragment` |
| `g` | Glitch | # | . | # | . | `power_cell` |

Both bosses list all four biomes; seven species are single-biome. Glitch is the
only source of `power_cell` in the roster.

## Traits

|  | Species | Flags | Abilities | Affinities |
|:---|:---|:---|:---|:---|
| `W` | Wintermute | BOSS | `broadcast_storm`, `null_route L4` | baseline |
| `B` | Overseer | BOSS | `broadcast_storm`, `overclock_array L5` | baseline |
| `S` | Sentinel | - | `overclock_array L2`, `bastion_shield_v3 L6` | buff 1.3 / damage 0.85 |
| `k` | Rootkit | - | `skim_group L2`, `skim_v3 L6` | drain 1.3 / buff 0.85 |
| `v` | Virus | - | `redundancy_sync L2`, `rollback_v3 L6` | heal 1.3 / damage 0.85 |
| `c` | Cipher | - | `deadlock L2`, `bit_rot_v3 L6` | debuff 1.3 / heal 0.85 |
| `z` | ZeroDay | - | `cascade_overflow L2`, `segfault_v3 L6` | damage 1.3 / heal 0.85 |
| `r` | Crawler | NEST | `overclock_array L2`, `bastion_shield_v2 L6` | buff 1.3 / damage 0.85 |
| `m` | Worm | NEST | `skim_group L2`, `skim_v2 L6` | drain 1.3 / buff 0.85 |
| `p` | Proxy | - | `redundancy_sync L2`, `rollback_v2 L6` | heal 1.3 / damage 0.85 |
| `t` | Trojan | NEST | `deadlock L2`, `bit_rot_v2 L6` | debuff 1.3 / heal 0.85 |
| `x` | Scrapper | NEST | `cascade_overflow L2`, `segfault_v2 L6` | damage 1.3 / heal 0.85 |
| `C` | Construct | - | `overclock_array L2`, `sandbox L6` | buff 1.3 / damage 0.85 |
| `o` | Drone | - | `skim_group L2`, `skim_v1 L6` | drain 1.3 / buff 0.85 |
| `d` | SubProcess | - | `redundancy_sync L2`, `rollback_v1 L6` | heal 1.3 / damage 0.85 |
| `s` | Sprite | - | `deadlock L2`, `memory_leak L6` | debuff 1.3 / heal 0.85 |
| `g` | Glitch | - | `cascade_overflow L2`, `segfault_v1 L6` | damage 1.3 / heal 0.85 |

Every non-boss species raises exactly one affinity axis to 1.3 and damps
exactly one to 0.85, and **the raised axis is what names its class** — damage
is a Striker, buff a Bastion, heal a Medic, debuff a Saboteur, drain a Leech.
Three species share each class, one per growth band, so the axis says what a
program does and the tier says how much of it. The stats and the speed above
are checked against that axis by
`every_ordinary_species_stat_shape_agrees_with_its_affinity_class`, and the
kit is checked against it by `every_ordinary_species_kit_agrees_with_its_
affinity_class`, which together stop a retune quietly changing a species'
role. Bosses declare no affinity at all: they are outside the class system.

The class also decides what a program does when it is **posted to a
structure**: a Leech draws an extra unit from every successful gather cycle, a
Bastion's Defense counts twice against a sweep on the structure it guards, and
a Medic mends that structure by 2 Durability every 20 ticks. A Striker and a
Saboteur do nothing at a post, which is the asymmetry the three pet slots make
expensive — every program at a machine is one absent from the party.

Every ordinary species grants two abilities: a class utility at level 2 that
all three members of its class share, and a tier rung at level 6 that it holds
alone. Nothing unlocks at level 1, which is what keeps `priority_boost` — the
fallback for a species that has taught a companion nothing yet — reachable at
all, since extracting it from such a companion is the only way to get it.

---

Source of truth is `assets/species/`. A mod that drops a `.ron` file in that
directory joins the roster without a recompile, and will not appear above until
this page is regenerated -- edit the table at the top of
[`docs/roster-gen.py`](roster-gen.py) and run `python3 docs/roster-gen.py` from
the repo root. The schema is documented in
[`assets/species/README.md`](../assets/species/README.md).
