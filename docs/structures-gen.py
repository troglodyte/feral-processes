# Regenerates docs/structures.md. Run from the repo root:
#     python3 docs/structures-gen.py
#
# Transcribed from assets/structures/*.ron by hand rather than parsed from it,
# for the same reason docs/roster-gen.py is. Update the table when a structure
# moves, then rerun.
#
# `feeder` is the one column not read straight off the file: a machine runs
# its product's own `craftable.cost` (there is deliberately no recipe on
# AssembleDef), so which machine feeds which is a property of the *items*.
# It is recorded here so the chain diagram can be drawn from data rather than
# hand-arranged, and it is what `every_shipped_assembler_recipe_is_a_single_
# ingredient` holds to one input apiece.
S = [
 # id               name             glyph color    build cost                                 kind        makes / does                    ticks cap  feeder          upgrade
 ("home",           "Home",           "H", "Green",  [("core_fragment", 5)],                    "utility",  "anchors the base, radius 4 and up", None, None, None,          None),
 ("mining_node",    "Mining Node",    "$", "Brown",  [("core_fragment", 12)],                   "producer", "core_fragment",                    10, None, None,          10),
 ("log_scraper",    "Log Scraper",    "T", "Cyan",   [("core_fragment", 14)],                   "producer", "raw_trace",                        10, None, None,          10),
 ("research_node",  "Research Node",  "R", "Cyan",   [("core_fragment", 10)],                   "producer", "research_data",                    14, None, None,          10),
 ("power_conduit",  "Power Conduit",  "+", "Yellow", [("core_fragment", 14)],                   "producer", "power_cell",                        6, None, None,          None),
 ("compiler",       "Compiler",       "&", "Green",  [("core_fragment", 16)],                   "assembler","ice_breaker",                       8,   20, "mining_node", 12),
 ("lathe",          "Lathe",          "L", "Brown",  [("core_fragment", 18)],                   "assembler","blank_substrate",                  12,   20, "mining_node", None),
 ("refinery",       "Refinery",       "B", "Orange", [("core_fragment", 18)],                   "assembler","bytecode_block",                   12,   20, "mining_node", None),
 ("transcriber",    "Transcriber",    "S", "Cyan",   [("core_fragment", 18)],                   "assembler","logic_wafer",                      12,   20, "mining_node", None),
 ("winding_node",   "Winding Node",   "W", "Blue",   [("core_fragment", 18)],                   "assembler","charge_coil",                      12,   20, "mining_node", None),
 ("disk_press",     "Disk Press",     "P", "Magenta",[("core_fragment", 20), ("blank_substrate", 4)], "assembler","routine_disk",              20,   10, "lathe",       None),
 ("armory",         "Armory",         "%", "Blue",   [("core_fragment", 18)],                   "assembler","hardened_shell",                   30,   15, "refinery",    None),
 ("fabricator",     "Fabricator",     "*", "White",  [("core_fragment", 18)],                   "assembler","trace_sniffer",                    30,   15, "transcriber", None),
 ("assembly_bay",   "Assembly Bay",   "Y", "Magenta",[("core_fragment", 20), ("charge_coil", 4)],"assembler","patch_routine",                   20,   10, "winding_node",None),
 ("annealing_node", "Annealing Node", "A", "Cyan",   [("core_fragment", 16)],                   "assembler","annealed_core",                    12,   20, "mining_node", None),
 ("refactor_bench", "Refactor Bench", "X", "Orange", [("core_fragment", 22), ("annealed_core", 4)],"assembler","recompile_kernel",              20,   10, "annealing_node",None),
 ("market",         "iso Market",     "$", "Yellow", [("core_fragment", 16)],                   "utility",  "buys and sells, 1 Credit a unit", None, None, None,          None),
 ("contract_broker","Contract Broker","!", "Yellow", [("core_fragment", 5)] ,                   "utility",  "posts contracts, takes deliveries",None, None, None,         None),
 ("data_cache",     "Data Cache",     "=", "Gray",   [("core_fragment", 10)],                   "utility",  "+5 roster slots while standing",  None, None, None,          None),
 ("depot",          "Depot",          "D", "Cyan",   [("core_fragment", 12)],                   "utility",  "programs stock it and fetch from it",None, 100, None,        None),
 ("shield",         "Shield",         "^", "Blue",   [("core_fragment", 16)],                   "utility",  "-2 sweep damage, base-wide",      None, None, None,          None),
 ("patch_node",     "Patch Node",     "/", "Green",  [("core_fragment", 18), ("power_cell", 4)],"utility",  "+1 Durability per tier / 20 ticks",None, None, None,         12),
 ("recharger_node", "Recharger Node", "z", "Orange", [("core_fragment", 10)],                   "utility",  "+1 Power a tick, base-wide",     None, None, None,          None),
 ("heap_pillar",    "Heap Pillar",    "I", "Cyan",   [("core_fragment", 14)],                   "utility",  "+1 tile of base radius, max 5",   None, None, None,          None),
 ("portal",         "Zone Portal",    "O", "Magenta",[("portal_fragment", 10)],                 "utility",  "breaches to the next sector",     None, None, None,          None),
]
K = "id name glyph color cost kind does ticks cap feeder upgrade".split()
R = [dict(zip(K, r)) for r in S]
BY = {r["id"]: r for r in R}

MAX_TIER = 5  # every `upgrade` in the directory sets max_tier: 5

SALVAGE = "core_fragment"
# A bench that names its own feeder's product in `build_cost` — computed from
# the relationship rather than listed, because the interesting fact is that
# only two of the four end benches do it, and a hand-kept list would not
# notice a third being added.
END_BENCHES = [
    r for r in R
    if r["kind"] == "assembler" and r["feeder"] and BY[r["feeder"]]["kind"] == "assembler"
]
SELF_FED = [
    r for r in END_BENCHES
    if any(i == BY[r["feeder"]]["does"] for i, _ in r["cost"])
]
BEYOND_SALVAGE = [r for r in R if any(i != SALVAGE for i, _ in r["cost"])]

kinds = {k: [r for r in R if r["kind"] == k] for k in ("producer", "assembler", "utility")}
upgradeable = [r for r in R if r["upgrade"]]


def cost_text(cost):
    return ", ".join(f"{n} `{i}`" for i, n in cost)


def table(header, rows, align):
    sep = "|" + "|".join(("---:" if a == "r" else ":---") for a in align) + "|"
    body = "\n".join("| " + " | ".join(str(c) for c in r) + " |" for r in rows)
    return "| " + " | ".join(header) + " |\n" + sep + "\n" + body


def chains():
    """Each production line, drawn from `feeder` rather than hand-arranged."""
    out = ["PRODUCTION LINES", ""]
    taps = [r for r in kinds["producer"] if any(c["feeder"] == r["id"] for c in R)]
    for tap in taps:
        firsts = [c for c in R if c["feeder"] == tap["id"]]
        for first in firsts:
            seconds = [c for c in R if c["feeder"] == first["id"]]
            line = f'{tap["glyph"]} {tap["name"]:<13} -> {first["glyph"]} {first["name"]:<14}'
            if seconds:
                s = seconds[0]
                line += f' -> {s["glyph"]} {s["name"]:<13} -> {s["does"]}'
            else:
                line += f' -> {first["does"]}'
            out.append(line)
    out.append("")
    out.append("standalone taps (no machine downstream):")
    for r in kinds["producer"]:
        if not any(c["feeder"] == r["id"] for c in R):
            out.append(f'  {r["glyph"]} {r["name"]:<13} -> {r["does"]} every {r["ticks"]} ticks')
    return "```\n" + "\n".join(out) + "\n```"


def rate_chart(width=34):
    """Ticks per unit, so the slow benches read as slow."""
    machines = kinds["producer"] + kinds["assembler"]
    hi = max(r["ticks"] for r in machines)
    out = ["TICKS PER UNIT                     (slower is longer)", ""]
    for r in sorted(machines, key=lambda r: r["ticks"]):
        n = round(r["ticks"] / hi * width)
        out.append(f'{r["name"]:<15} {r["ticks"]:>2}  {"#" * n}{"." * (width - n)}')
    return "```\n" + "\n".join(out) + "\n```"


doc = f"""# Structures

Every shipped structure in feral-processes, charted from its own file in
`assets/structures/`. {len(R)} of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/structures/*.ron` on 2026-08-14 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

Everything below must be deployed on the base slab — within 4 tiles of a Home,
minus the chamfer taken off each of the slab's four corners — and demolishing
the Home cascades to the rest. Each Heap Pillar pushes that radius out by one,
and five is all a grid will hold, so a base tops out at 19x19. A structure named by no research file is
buildable from turn one; the [research tree](research.md) gates the machines
that automate a base, not the base itself.

| | |
|---|---|
| structures | {len(R)} |
| producers (make something from nothing, on a timer) | {len(kinds["producer"])} |
| assemblers (consume a neighbour's output) | {len(kinds["assembler"])} |
| utility | {len(kinds["utility"])} |
| upgradeable | {len(upgradeable)}, all to Mk{MAX_TIER} |
| built from something other than salvage | {len(BEYOND_SALVAGE)} — {", ".join(r["name"] for r in BEYOND_SALVAGE)} |

## Everything that can be built

{table(["", "Structure", "Build cost", "Cap", "Makes / does"],
       [[f'`{r["glyph"]}`', r["name"], cost_text(r["cost"]), r["cap"] or "-",
         (f'`{r["does"]}`' if r["kind"] != "utility" else r["does"])] for r in R],
       ["l", "l", "l", "r", "l"])}

Two glyph collisions are worth knowing before you read a map: the Mining Node
and the iso Market both draw as `$`, and the Recharger Node draws as `z`, the
same as a ZeroDay. Colour is what separates them — and in the last case,
position: wild programs never spawn on your base platform.

## The production lines

A chain flows one way without belts existing, and the reason is an asymmetry
in one struct: a machine's `output` is public and its `input` is private.
Neighbours pull from what a machine has **finished**; nothing ever reaches
into what it is still working on. Adjacency is the whole of the wiring.

{chains()}

Every one of the eleven assembler recipes is a **single ingredient**, and that
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

{table(["Bench", "Runs on", "Build cost"],
       [[r["name"], f'`{BY[r["feeder"]]["does"]}`', cost_text(r["cost"])]
        for r in kinds["assembler"] if r["feeder"] and BY[r["feeder"]]["kind"] == "assembler"],
       ["l", "l", "l"])}

{len(SELF_FED)} of those {len(END_BENCHES)} are built out of the very thing their own feeder makes.
The Disk Press costs Blank Substrate and the Assembly Bay costs Charge Coils, so
the line that runs the bench is also the line that pays for it. The Armory and
the Fabricator are not — both cost plain salvage, so they can be put up before
their feeder is producing. `each_bench_is_built_out_of_what_its_own_feeder_
makes` asserts the first pair and deliberately not the second.

## Rates

{rate_chart()}

The two 30-tick benches are the gear benches, and the gap between them and
their 12-tick feeders is the point: a Refinery outruns an Armory better than
two to one, so a single feeder keeps a bench saturated and the queue backs up
at the bench rather than starving it.

Rate starts as a property of the machine — its own `ticks_per_unit` above is
the baseline — but the posted program scales it: a species' `base_speed`
paces the cycle the same way `base_int` paces a mining roll, faster above
`DEFAULT_BASE_SPEED` and slower below it (see
[`assets/species/README.md`](../assets/species/README.md)). The scaled
length is computed once, at assignment (`Game::work_ticks_for`), and baked
into the `Task`, so re-reading a machine's `ticks_per_unit` mid-cycle would
not show a swapped-in program taking effect early. A stall is announced only
on the transition into it, so a base with four stalled machines does not put
four lines in the pane every tick.

## Upgrades

{table(["Structure", "Per tier", "Ceiling"],
       [[r["name"], cost_text([("core_fragment", r["upgrade"])]), f"Mk{MAX_TIER}"]
        for r in upgradeable],
       ["l", "l", "l"])}

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
"""

import pathlib
pathlib.Path("docs/structures.md").write_text(doc)
print(doc)
