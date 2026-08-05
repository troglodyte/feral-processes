# Roster stat sheet

Every shipped species in feral-processes, charted from its own file in
`assets/species/`. Seventeen of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/species/*.ron` on 2026-08-05 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind. Where a
file omits a field, the engine default from `crates/engine/src/tuning.rs` is
shown: `base_speed 10`, `growth_multiplier 1.0`, all five affinities `1.0`.

POW is the engine's own scalar, `Stats::power` — `max_hp + atk + def`,
unweighted. It is what `difficulty_color` reads to decide whether a program
shows up green or red on your map. Every table below is in POW order.

| | |
|---|---|
| species | 17 |
| bosses | 2 (Wintermute, Overseer) |
| nest builders | 4 (Scrapper, Trojan, Worm, Wraith) |
| work yields | 8 — 7 `core_fragment`, 1 `power_cell` |
| HP span | 36 (Glitch) to 200 (Wintermute) |
| speed span | 6 (Construct) to 14 (Sprite); the player rolls from 11 |

## Core stats

|  | Species | HP | ATK | DEF | SPD | POW | Tame | Growth |
|:---|:---|---:|---:|---:|---:|---:|---:|---:|
| `W` | Wintermute **·boss** | 200 | 19 | 17 | 13 | 236 | 0.95 | x2.00 |
| `B` | Overseer **·boss** | 180 | 17 | 15 | 12 | 212 | 0.90 | x2.00 |
| `S` | Sentinel | 150 | 9 | 12 | 7 | 171 | 0.65 | x1.50 |
| `C` | Construct | 128 | 11 | 9 | 6 | 148 | 0.70 | x1.50 |
| `k` | Rootkit | 120 | 11 | 10 | 9 | 141 | 0.75 | x1.50 |
| `c` | Cipher | 112 | 10 | 8 | 11 | 130 | 0.80 | x1.50 |
| `v` | Virus | 112 | 10 | 6 | 10 | 128 | 0.60 | x1.50 |
| `h` | Ghost | 98 | 14 | 4 | 10 | 116 | 0.65 | x1.50 |
| `m` | Worm | 105 | 8 | 2 | 9 | 115 | 0.40 | x1.25 |
| `x` | Scrapper | 98 | 9 | 5 | 9 | 112 | 0.45 | x1.25 |
| `t` | Trojan | 90 | 10 | 4 | 10 | 104 | 0.50 | x1.25 |
| `p` | Phantom | 82 | 12 | 2 | 12 | 96 | 0.55 | x1.25 |
| `w` | Wraith | 75 | 8 | 4 | 11 | 87 | 0.50 | x1.25 |
| `d` | SubProcess | 54 | 5 | 3 | 12 | 62 | 0.30 | x1.00 |
| `s` | Sprite | 48 | 4 | 2 | 14 | 54 | 0.20 | x1.00 |
| `o` | Drone | 42 | 3 | 2 | 13 | 47 | 0.15 | x1.00 |
| `g` | Glitch | 36 | 3 | 1 | 13 | 40 | 0.15 | x1.00 |

## Attack against defense

Each species sits at its own `(base_atk, base_def)`, drawn with its map glyph.
The top-right corner belongs to the two bosses alone, and the bottom-left to
the four programs the opening ring draws from. Between them the roster splits
into a defended column around ATK 9-11 and a glass column out at ATK 12-14.

```
BASE DEF
18 |+ - - - + - - - + - - - + - - - + - - - +
17 ||       |       |       |       |     W |
16 ||       |       |       |       |       |
15 |+ - - - + - - - + - - - + - - - + B - - +
14 ||       |       |       |       |       |
13 ||       |       |       |       |       |
12 |+ - - - + - - - + S - - + - - - + - - - +
11 ||       |       |       |       |       |
10 ||       |       |     k |       |       |
 9 |+ - - - + - - - + - - C + - - - + - - - +
 8 ||       |       |   c   |       |       |
 7 ||       |       |       |       |       |
 6 |+ - - - + - - - + - v - + - - - + - - - +
 5 ||       |       | x     |       |       |
 4 ||       |       w   t   |   h   |       |
 3 |+ - - - + d - - + - - - + - - - + - - - +
 2 ||     o s       m       p       |       |
 1 ||     g |       |       |       |       |
 0 |+ - - - + - - - + - - - + - - - + - - - +
   +------------------------------------------
    0       4       8       12      16      20
    BASE ATK
```

Five species pile up at ATK 10-11 — Cipher, Construct, Rootkit, Virus and
Trojan. What separates them is entirely HP and DEF, which is what the profiles
below are for. Ghost is the roster's outlier: ATK 14 on DEF 4, the highest
attack outside a boss carried on almost nothing.

## Stat profiles

One row order throughout, so a species' shape across the four charts is its
character. Each chart is scaled against the roster maximum for that stat, not
against the others — the HP bars are not comparable to the ATK bars.

```
BASE HP                             (max 200)

Wintermute 200  ████████████████████████████████████████████
Overseer   180  ████████████████████████████████████████····
Sentinel   150  █████████████████████████████████···········
Construct  128  ████████████████████████████················
Rootkit    120  ██████████████████████████··················
Cipher     112  █████████████████████████···················
Virus      112  █████████████████████████···················
Ghost       98  ██████████████████████······················
Worm       105  ███████████████████████·····················
Scrapper    98  ██████████████████████······················
Trojan      90  ████████████████████························
Phantom     82  ██████████████████··························
Wraith      75  ████████████████····························
SubProcess  54  ████████████································
Sprite      48  ███████████·································
Drone       42  █████████···································
Glitch      36  ████████····································
```

```
BASE ATK                             (max 19)

Wintermute  19  ████████████████████████████████████████████
Overseer    17  ███████████████████████████████████████·····
Sentinel     9  █████████████████████·······················
Construct   11  █████████████████████████···················
Rootkit     11  █████████████████████████···················
Cipher      10  ███████████████████████·····················
Virus       10  ███████████████████████·····················
Ghost       14  ████████████████████████████████············
Worm         8  ███████████████████·························
Scrapper     9  █████████████████████·······················
Trojan      10  ███████████████████████·····················
Phantom     12  ████████████████████████████················
Wraith       8  ███████████████████·························
SubProcess   5  ████████████································
Sprite       4  █████████···································
Drone        3  ███████·····································
Glitch       3  ███████·····································
```

```
BASE DEF                             (max 17)

Wintermute  17  ████████████████████████████████████████████
Overseer    15  ███████████████████████████████████████·····
Sentinel    12  ███████████████████████████████·············
Construct    9  ███████████████████████·····················
Rootkit     10  ██████████████████████████··················
Cipher       8  █████████████████████·······················
Virus        6  ████████████████····························
Ghost        4  ██████████··································
Worm         2  █████·······································
Scrapper     5  █████████████·······························
Trojan       4  ██████████··································
Phantom      2  █████·······································
Wraith       4  ██████████··································
SubProcess   3  ████████····································
Sprite       2  █████·······································
Drone        2  █████·······································
Glitch       1  ███·········································
```

```
BASE SPEED

14  Sprite
13  Wintermute, Drone, Glitch
12  Overseer, Phantom, SubProcess
11  Cipher, Wraith   <- the player rolls from here
10  Virus, Ghost, Trojan
 9  Rootkit, Worm, Scrapper
 8  -
 7  Sentinel
 6  Construct
```

Speed inverts almost everything else: the top of the ladder is Sprite, Drone
and Glitch, three of the four weakest programs in the game, and the bottom is
Construct and Sentinel, the two heaviest non-bosses. Both bosses sit near the
top anyway, which is the one place the roster does not trade power for pace.
Speed is an initiative baseline rather than a turn order, though — every
combatant rolls `base_speed + d10` each round, so a 4-point gap still loses
sometimes.

## Taming cost against growth tier

`taming_difficulty` along the axis, `growth_multiplier` as the lane. A tie
shifts one glyph right, so read the lanes for grouping rather than for an
exact position.

```
GROWTH
x2.00  |                                      B  W
x1.50  |                        v Sh C k c
x1.25  |              m  x tw p
x1.00  |  og s    d
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

|  | Species | OpenGrid | Mainframe | NullSector | StaticField | Yield |
|:---|:---|:---|:---|:---|:---|:---|
| `W` | Wintermute | # | # | # | # | - |
| `B` | Overseer | # | # | # | # | - |
| `S` | Sentinel | . | . | . | # | - |
| `C` | Construct | . | # | . | . | `core_fragment` |
| `k` | Rootkit | . | # | # | . | - |
| `c` | Cipher | . | # | . | # | - |
| `v` | Virus | . | # | # | . | `core_fragment` |
| `h` | Ghost | . | . | # | # | - |
| `m` | Worm | # | . | # | . | `core_fragment` |
| `x` | Scrapper | # | . | # | . | `core_fragment` |
| `t` | Trojan | # | # | . | . | - |
| `p` | Phantom | . | # | . | # | - |
| `w` | Wraith | . | . | . | # | - |
| `d` | SubProcess | # | . | # | . | `core_fragment` |
| `s` | Sprite | # | # | . | . | `core_fragment` |
| `o` | Drone | # | # | . | . | `core_fragment` |
| `g` | Glitch | # | . | # | . | `power_cell` |

Both bosses list all four biomes; seven species are single-biome. Glitch is the
only source of `power_cell` in the roster.

## Traits

|  | Species | Flags | Abilities | Affinities |
|:---|:---|:---|:---|:---|
| `W` | Wintermute | BOSS | `broadcast_storm`, `null_route L4` | baseline |
| `B` | Overseer | BOSS | `broadcast_storm`, `overclock_array L5` | baseline |
| `S` | Sentinel | - | `sandbox`, `redundancy_sync L6` | buff 1.3 / damage 0.85 |
| `C` | Construct | - | - | baseline |
| `k` | Rootkit | - | `deadlock`, `memory_leak L4` | drain 1.3 / buff 0.85 |
| `c` | Cipher | - | `memory_leak`, `null_route L8` | debuff 1.35 / heal 0.85 |
| `v` | Virus | - | - | baseline |
| `h` | Ghost | - | - | damage 1.25 / buff 0.85 |
| `m` | Worm | NEST | - | baseline |
| `x` | Scrapper | NEST | `cascade_overflow L3` | damage 1.2 / heal 0.85 |
| `t` | Trojan | NEST | - | baseline |
| `p` | Phantom | - | - | baseline |
| `w` | Wraith | NEST | - | baseline |
| `d` | SubProcess | - | `hot_patch`, `redundancy_sync L7` | heal 1.4 / damage 0.8 |
| `s` | Sprite | - | - | baseline |
| `o` | Drone | - | - | baseline |
| `g` | Glitch | - | - | baseline |

Six species override an affinity, and each override is paid for with a
matching weakness — SubProcess heals at 1.4 and hits at 0.8, Sentinel buffs at
1.3 and hits at 0.85. Seven grant abilities; a species that grants none falls
back to `priority_boost`.

---

Source of truth is `assets/species/`. A mod that drops a `.ron` file in that
directory joins the roster without a recompile, and will not appear above until
this page is regenerated -- edit the table at the top of
[`docs/roster-gen.py`](roster-gen.py) and run `python3 docs/roster-gen.py` from
the repo root. The schema is documented in
[`assets/species/README.md`](../assets/species/README.md).
