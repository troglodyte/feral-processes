# Regenerates docs/research.md. Run from the repo root:
#     python3 docs/research-gen.py
#
# Transcribed from assets/research/*.ron by hand rather than parsed from it,
# for the same reason docs/roster-gen.py is. Update the table when a node
# moves, then rerun -- the tree, the depths and the running totals below are
# all derived from `req`, so a re-parented node redraws the page correctly
# without anything else being edited.
#
# `zone` is the node's `min_zone`: 0 for ungated, otherwise the zone the
# player must have reached before it can be bought at all. The bands below
# must stay monotone in `req` -- a node gated below its own prerequisite is a
# gate that can never fire. See assets/research/README.md.
N = [
 # id                    name                  zone cost requires             unlocks (kind, [ids])
 ("automation",          "Automation",           0,   8, [],                  ("structures", ["compiler"])),
 ("commerce",            "Isometric Commerce",   0,  14, [],                  ("structures", ["market"])),
 ("power_grid",          "Power Grid",           0,  10, [],                  ("structures", ["power_conduit"])),
 ("armor_bench",         "Reactive Armor",       0,  24, ["automation"],      ("structures", ["armory"])),
 ("weapon_bench",        "Weapon Fabrication",   0,  24, ["automation"],      ("structures", ["fabricator"])),
 ("routine_fabrication", "Routine Fabrication",  0,  26, ["automation"],      ("structures", ["log_scraper", "lathe", "transcriber", "disk_press"])),
 ("program_refactoring", "Program Refactoring",  2,  75, ["automation"],      ("structures", ["annealing_node", "refactor_bench"])),
 ("fortification",       "Fortification",        0,  18, ["power_grid"],      ("structures", ["shield", "patch_node"])),
 ("heap_allocation",     "Heap Allocation",      0,  30, ["power_grid"],      ("structures", ["heap_pillar"])),
 ("self_exec",           "Self-Execution",       0,  14, ["routine_fabrication"], ("abilities", ["priority_boost"])),
 ("field_ops",           "Field Operations",     0,  20, ["self_exec"],       ("abilities", ["repair_loop", "trickle_charge"])),
 ("runtime_patching",    "Runtime Patching",     2,  60, ["self_exec"],       ("abilities", ["hot_patch"])),
 ("adaptive_plating",    "Adaptive Plating",     2,  70, ["field_ops"],       ("abilities", ["hardened_shell", "overclock", "ablative_layer"])),
 ("mesh_plating",        "Mesh Plating",         3, 120, ["adaptive_plating"], ("abilities", ["hardened_shell_party"])),
 ("deep_analysis",       "Deep Analysis",        3, 130, ["field_ops"],       ("abilities", ["deep_scan", "trace_analysis", "stealth_protocol", "salvage_routine"])),
 ("address_translation", "Address Translation",  3, 140, ["deep_analysis"],   ("abilities", ["buffer_overrun", "wild_jump"])),
 ("kernel_privileges",   "Kernel Privileges",    3, 135, ["runtime_patching"], ("abilities", ["null_route"])),
 ("firewall",            "Firewall Plating",     2,  45, ["armor_bench"],     ("recipe", ["firewall_plating", "armory", "6"])),
 ("ablative",            "Ablative Lattice",     3, 110, ["firewall"],        ("recipe", ["ablative_plating", "armory", "12"])),
 ("neural_amp",          "Neural Interfacing",   2,  55, ["weapon_bench"],    ("recipe", ["neural_amplifier", "fabricator", "6"])),
 ("cortex",              "Cortex Hacking",       3, 125, ["neural_amp"],      ("recipe", ["cortex_hack", "fabricator", "12"])),
 ("overclock",           "Overclock Cores",      2,  45, ["weapon_bench"],    ("recipe", ["overclock_core", "fabricator", "6"])),
 ("monofilament",        "Monofilament Edge",    3, 110, ["overclock"],       ("recipe", ["monofilament_whip", "fabricator", "12"])),
]
K = "id name zone cost req unlocks".split()
R = [dict(zip(K, r)) for r in N]
BY = {r["id"]: r for r in R}

# Every recipe node in the tree is priced in this one item, which is what
# makes the "researched gear costs breach currency" claim below checkable
# rather than asserted.
RECIPE_CURRENCY = "portal_fragment"

roots = [r for r in R if not r["req"]]
kids = {r["id"]: [c for c in R if r["id"] in c["req"]] for r in R}

# Guards the transcription against itself rather than restating the game's
# rule -- the engine census `no_research_node_is_gated_below_its_own_
# prerequisite` is what holds the assets. This catches a row copied out with
# the wrong band, which would otherwise draw a plausible-looking page.
for r in R:
    for p in r["req"]:
        assert BY[p]["zone"] <= r["zone"], f'{r["id"]} is gated below {p}'


def depth(r):
    return 0 if not r["req"] else 1 + max(depth(BY[p]) for p in r["req"])


def to_root(r):
    """Total Research Data to take this node from a standing start."""
    return r["cost"] + sum(to_root(BY[p]) for p in r["req"])


for r in R:
    r["depth"] = depth(r)
    r["total"] = to_root(r)

deep_leaves = sorted(
    (r for r in R if not kids[r["id"]] and r["depth"] >= 3),
    key=lambda r: -r["total"],
)
DEEP_NAMES = ", ".join(r["name"] for r in deep_leaves)
DEEP_LO = min(r["total"] - r["cost"] for r in deep_leaves)
DEEP_HI = max(r["total"] - r["cost"] for r in deep_leaves)
DEEP_TOTAL_LO = min(r["total"] for r in deep_leaves)
DEEP_TOTAL_HI = max(r["total"] for r in deep_leaves)
DEAREST = max(r["cost"] for r in R)
total_cost = sum(r["cost"] for r in R)
max_depth = max(r["depth"] for r in R)


def tree():
    out = []

    def walk(r, prefix, last):
        elbow = "" if prefix == "" and last is None else ("`-- " if last else "|-- ")
        out.append(f'{prefix}{elbow}{r["name"]} ({r["cost"]})')
        cs = kids[r["id"]]
        for i, c in enumerate(cs):
            tail = i == len(cs) - 1
            nxt = prefix + ("" if last is None else ("    " if last else "|   "))
            walk(c, nxt, tail)

    for root in roots:
        walk(root, "", None)
        out.append("")
    return "```\nRESEARCH TREE            (Research Data to unlock each node)\n\n" + "\n".join(out).rstrip() + "\n```"


def cost_ladder(width=40):
    hi = max(r["total"] for r in R)
    out = ["CUMULATIVE COST FROM A STANDING START", ""]
    for r in sorted(R, key=lambda r: (r["total"], r["name"])):
        n = round(r["total"] / hi * width)
        out.append(f'{r["name"]:<21} {r["total"]:>3}  {"#" * n}{"." * (width - n)}')
    return "```\n" + "\n".join(out) + "\n```"


def table(header, rows, align):
    sep = "|" + "|".join(("---:" if a == "r" else ":---") for a in align) + "|"
    body = "\n".join("| " + " | ".join(str(c) for c in r) + " |" for r in rows)
    return "| " + " | ".join(header) + " |\n" + sep + "\n" + body


def unlock_text(r):
    kind, ids = r["unlocks"]
    if kind == "recipe":
        item, bench, price = ids
        return f'recipe `{item}` at the {bench} — {price} `{RECIPE_CURRENCY}`'
    return ", ".join(f"`{i}`" for i in ids)


counts = {k: sum(1 for r in R if r["unlocks"][0] == k) for k in ("structures", "abilities", "recipe")}
structures_unlocked = sum(len(r["unlocks"][1]) for r in R if r["unlocks"][0] == "structures")
abilities_unlocked = sum(len(r["unlocks"][1]) for r in R if r["unlocks"][0] == "abilities")

BANDS = sorted({r["zone"] for r in R})


def band_label(z):
    return "from turn one" if z == 0 else f"zone {z}"


def band_rows():
    return [
        [
            band_label(z),
            sum(1 for r in R if r["zone"] == z),
            sum(r["cost"] for r in R if r["zone"] == z),
            ", ".join(r["name"] for r in sorted(R, key=lambda r: r["cost"]) if r["zone"] == z),
        ]
        for z in BANDS
    ]

doc = f"""# Research tree

Every shipped research node in feral-processes, charted from its own file in
`assets/research/`. {len(R)} of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/research/*.ron` on 2026-08-17 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

Research Data is the currency, and it comes from one place: a Research Node
structure worked by an assigned tamed program, the same way a Mining Node
produces Core Fragments. So the whole tree below is priced in *base uptime* —
it is the one progression track you cannot fight your way along.

| | |
|---|---|
| nodes | {len(R)} |
| roots (need nothing) | {len(roots)} — {", ".join(r["name"] for r in roots)} |
| deepest chain | {max_depth + 1} nodes |
| total Research Data | {total_cost} |
| cheapest / dearest node | {min(r["cost"] for r in R)} / {max(r["cost"] for r in R)} |
| zone bands | {", ".join(f'{band_label(z)} ({sum(1 for r in R if r["zone"] == z)})' for z in BANDS)} |
| unlocks | {structures_unlocked} structures, {abilities_unlocked} routines, {counts["recipe"]} gear recipes |

## What the zone gates

Price is not the only thing pacing the tree. A node may declare a `min_zone`,
and below it the node is listed, priced and explained but unbuyable at any
balance — the visible tier *is* the reason to go breach. Research Data
survives a breach, so without this the whole tree could be finished without
ever opening a portal.

{table(["Available", "Nodes", "Research Data", "Which"],
       band_rows(),
       ["l", "r", "r", "l"])}

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

{tree()}

Three roots, and they are three different games. **Automation** is the trunk:
everything that makes a base do work hangs off it, and it is also the cheapest
node in the tree at {BY["automation"]["cost"]}, so the opening move is barely a
decision. **Power Grid** is a three-node stub: the two things that
keep a base standing, and the one that lets it grow.
**Isometric Commerce** is a leaf — {BY["commerce"]["cost"]} Research Data buys
the iso Market and leads nowhere, which makes it the one node you take purely
because you want the thing rather than the branch.

Under Automation the tree splits three ways and never rejoins: benches
(Reactive Armor, Weapon Fabrication) lead to **gear recipes**, and Routine
Fabrication leads to **routines**. Nothing in the tree requires two parents —
every `requires` is a single id — so this is a tree in the strict sense, and
there is no node you can reach two ways.

## What each node unlocks

{table(["Node", "Zone", "Cost", "Needs", "Unlocks"],
       [[r["name"], "-" if r["zone"] == 0 else r["zone"], r["cost"],
         f'`{r["req"][0]}`' if r["req"] else "-", unlock_text(r)]
        for r in sorted(R, key=lambda r: (r["depth"], r["name"]))],
       ["l", "r", "r", "l", "l"])}

A structure named by **no** research file is buildable from turn one — the
tree gates the machines that automate a base, not the base itself.

## What it costs to get there

A node's own `cost` is not what it costs you. Everything above it has to be
unlocked first, so the real price of Monofilament Edge is its own
{BY["monofilament"]["cost"]} plus the whole chain behind it.

{cost_ladder()}

The shape to notice is the {len(deep_leaves)} end-of-branch nodes: {DEEP_NAMES}.
Each carries {DEEP_LO}-{DEEP_HI} Research Data of prerequisites behind it before
its own price is counted, and lands at {DEEP_TOTAL_LO}-{DEEP_TOTAL_HI} from a
standing start — {DEEP_TOTAL_HI // DEAREST}x the dearest single node in the
tree ({DEAREST}) at the top end. The tree is not steep; it is long, and the
zone bands are what stop that length being paid off in one sitting.

## Routines against recipes

The two halves of the tree pay in different currencies, and that is the
sharper divide than depth.

A **routine** node hands you the knowledge outright: unlock it and the
routines are yours to install, no materials involved. A **recipe** node hands
you the right to *build* something, and every one of the six is priced in
`{RECIPE_CURRENCY}` — the item a Stack lair guardian drops and nothing else
in the game does, and the same one that pays for a breach. So the recipe half
of the tree is priced in descents: every node on it competes directly with
the portal you are saving for.

{table(["Recipe node", "Builds", "At", f"`{RECIPE_CURRENCY}`"],
       [[r["name"], f'`{r["unlocks"][1][0]}`', r["unlocks"][1][1], r["unlocks"][1][2]]
        for r in R if r["unlocks"][0] == "recipe"],
       ["l", "l", "l", "r"])}

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
"""

import pathlib
pathlib.Path("docs/research.md").write_text(doc)
print(doc)
