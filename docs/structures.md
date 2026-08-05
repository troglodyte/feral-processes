# Structures

Every shipped structure in feral-processes, charted from its own file in
`assets/structures/`. Twenty of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/structures/*.ron` on 2026-08-05 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

Everything below must be deployed within 7 tiles of a Home, and demolishing
the Home cascades to the rest. A structure named by no research file is
buildable from turn one; the [research tree](research.md) gates the machines
that automate a base, not the base itself.

| | |
|---|---|
| structures | 20 |
| producers (make something from nothing, on a timer) | 4 |
| assemblers (consume a neighbour's output) | 9 |
| utility | 7 |
| upgradeable | 5, all to Mk5 |
| built from something other than salvage | 4 — Disk Press, Assembly Bay, Patch Node, Zone Portal |

## Everything that can be built

|  | Structure | Build cost | Cap | Makes / does |
|:---|:---|:---|---:|:---|
| `H` | Home | 5 `core_fragment` | - | anchors the base, radius 7 |
| `$` | Mining Node | 12 `core_fragment` | - | `core_fragment` |
| `T` | Log Scraper | 14 `core_fragment` | - | `raw_trace` |
| `R` | Research Node | 10 `core_fragment` | - | `research_data` |
| `+` | Power Conduit | 14 `core_fragment` | - | `power_cell` |
| `&` | Compiler | 16 `core_fragment` | 20 | `ice_breaker` |
| `L` | Lathe | 18 `core_fragment` | 20 | `blank_substrate` |
| `B` | Refinery | 18 `core_fragment` | 20 | `bytecode_block` |
| `S` | Transcriber | 18 `core_fragment` | 20 | `logic_wafer` |
| `W` | Winding Node | 18 `core_fragment` | 20 | `charge_coil` |
| `P` | Disk Press | 20 `core_fragment`, 4 `blank_substrate` | 10 | `routine_disk` |
| `%` | Armory | 18 `core_fragment` | 15 | `hardened_shell` |
| `*` | Fabricator | 18 `core_fragment` | 15 | `trace_sniffer` |
| `Y` | Assembly Bay | 20 `core_fragment`, 4 `charge_coil` | 10 | `patch_routine` |
| `$` | iso Market | 16 `core_fragment` | - | buys and sells, 1 Credit a unit |
| `=` | Data Cache | 10 `core_fragment` | - | +5 roster slots while standing |
| `^` | Shield | 16 `core_fragment` | - | -2 sweep damage, base-wide |
| `/` | Patch Node | 18 `core_fragment`, 4 `power_cell` | - | +1 Durability per tier / 20 ticks |
| `z` | Recharger Node | 10 `core_fragment` | - | +1 Power a tick within 7 tiles |
| `O` | Zone Portal | 10 `portal_fragment` | - | breaches to the next sector |

Two glyph collisions are worth knowing before you read a map: the Mining Node
and the iso Market both draw as `$`, and the Recharger Node draws as `z`, the
same as a ZeroDay. Colour is what separates them — and in the last case,
position: wild programs never spawn on your base platform.

## The production lines

A chain flows one way without belts existing, and the reason is an asymmetry
in one struct: a machine's `output` is public and its `input` is private.
Neighbours pull from what a machine has **finished**; nothing ever reaches
into what it is still working on. Adjacency is the whole of the wiring.

```
PRODUCTION LINES

$ Mining Node   -> & Compiler       -> ice_breaker
$ Mining Node   -> L Lathe          -> P Disk Press    -> routine_disk
$ Mining Node   -> B Refinery       -> % Armory        -> hardened_shell
$ Mining Node   -> S Transcriber    -> * Fabricator    -> trace_sniffer
$ Mining Node   -> W Winding Node   -> Y Assembly Bay  -> patch_routine

standalone taps (no machine downstream):
  T Log Scraper   -> raw_trace every 10 ticks
  R Research Node -> research_data every 14 ticks
  + Power Conduit -> power_cell every 6 ticks
```

Every one of the nine assembler recipes is a **single ingredient**, and that
is a property of the items rather than the machines — a machine runs its
product's own `craftable.cost`, so there is no separate recipe on the
structure that could drift from the bench recipe. A second ingredient added to
any of the four intermediates would silently turn its bench back into a corner
puzzle needing two lines stood up before a single unit comes out.

The four intermediates match the four end benches one to one, so a line is a
straight line: raw tap, refiner, bench. The Compiler is the exception that
proves it — it hangs straight off the Mining Node with nothing in between,
which is why ICE Breakers are the one manufactured good available before any
of the four lines exist.

## What a line costs to stand up

| Bench | Runs on | Build cost |
|:---|:---|:---|
| Disk Press | `blank_substrate` | 20 `core_fragment`, 4 `blank_substrate` |
| Armory | `bytecode_block` | 18 `core_fragment` |
| Fabricator | `logic_wafer` | 18 `core_fragment` |
| Assembly Bay | `charge_coil` | 20 `core_fragment`, 4 `charge_coil` |

2 of those 4 are built out of the very thing their own feeder makes.
The Disk Press costs Blank Substrate and the Assembly Bay costs Charge Coils, so
the line that runs the bench is also the line that pays for it. The Armory and
the Fabricator are not — both cost plain salvage, so they can be put up before
their feeder is producing. `each_bench_is_built_out_of_what_its_own_feeder_
makes` asserts the first pair and deliberately not the second.

## Rates

```
TICKS PER UNIT                     (slower is longer)

Power Conduit    6  #######...........................
Compiler         8  #########.........................
Mining Node     10  ###########.......................
Log Scraper     10  ###########.......................
Lathe           12  ##############....................
Refinery        12  ##############....................
Transcriber     12  ##############....................
Winding Node    12  ##############....................
Research Node   14  ################..................
Disk Press      20  #######################...........
Assembly Bay    20  #######################...........
Armory          30  ##################################
Fabricator      30  ##################################
```

The two 30-tick benches are the gear benches, and the gap between them and
their 12-tick feeders is the point: a Refinery outruns an Armory better than
two to one, so a single feeder keeps a bench saturated and the queue backs up
at the bench rather than starving it.

Rate is a property of the machine, not of the worker — an assembler's speed
comes from its own `ticks_per_unit` and never from how the program was
assigned. A stall is announced only on the transition into it, so a base with
four stalled machines does not put four lines in the pane every tick.

## Upgrades

| Structure | Per tier | Ceiling |
|:---|:---|:---|
| Mining Node | 10 `core_fragment` | Mk5 |
| Log Scraper | 10 `core_fragment` | Mk5 |
| Research Node | 10 `core_fragment` | Mk5 |
| Compiler | 12 `core_fragment` | Mk5 |
| Patch Node | 12 `core_fragment` | Mk5 |

A tier is bounded twice and the two bounds mean different things. The def's
`max_tier` is permanent; the zone is not, and reaching sector *N* is what
unlocks Mk*N* — so **nothing upgrades at all before the first breach**. A
structure sitting at its zone ceiling still lists in the upgrade menu rather
than being filtered out, because a player who has never breached would
otherwise never learn upgrading exists.

Note which five they are. Three of the four producers upgrade and the fourth,
the Power Conduit, does not; one assembler upgrades out of nine, and it is the
Compiler. Upgrading is a lever on the taps, not on the lines.

---

Source of truth is `assets/structures/`. A mod that drops a `.ron` file in
that directory becomes buildable without a recompile, and will not appear
above until this page is regenerated -- edit the table at the top of
[`docs/structures-gen.py`](structures-gen.py) and run
`python3 docs/structures-gen.py` from the repo root. The schema is documented
in [`assets/structures/README.md`](../assets/structures/README.md).
