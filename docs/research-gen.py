# Regenerates docs/research.md. Run from the repo root:
#     python3 docs/research-gen.py
#
# Transcribed from assets/research/*.ron by hand rather than parsed from it,
# for the same reason docs/roster-gen.py is. Update the table when a node
# moves, then rerun -- the tree, the depths and the running totals below are
# all derived from `req`, so a re-parented node redraws the page correctly
# without anything else being edited.
N = [
 # id                    name                   cost requires             unlocks (kind, [ids])
 ("automation",          "Automation",            8, [],                  ("structures", ["compiler"])),
 ("commerce",            "Isometric Commerce",   12, [],                  ("structures", ["market"])),
 ("power_grid",          "Power Grid",           10, [],                  ("structures", ["power_conduit"])),
 ("armor_bench",         "Reactive Armor",       18, ["automation"],      ("structures", ["armory"])),
 ("weapon_bench",        "Weapon Fabrication",   18, ["automation"],      ("structures", ["fabricator"])),
 ("routine_fabrication", "Routine Fabrication",  20, ["automation"],      ("structures", ["log_scraper", "lathe", "transcriber", "disk_press"])),
 ("program_refactoring", "Program Refactoring",  34, ["automation"],      ("structures", ["annealing_node", "refactor_bench"])),
 ("fortification",       "Fortification",        15, ["power_grid"],      ("structures", ["shield", "patch_node"])),
 ("self_exec",           "Self-Execution",       12, ["routine_fabrication"], ("abilities", ["priority_boost"])),
 ("field_ops",           "Field Operations",     16, ["self_exec"],       ("abilities", ["repair_loop", "coolant_flush", "trickle_charge"])),
 ("runtime_patching",    "Runtime Patching",     28, ["self_exec"],       ("abilities", ["hot_patch"])),
 ("adaptive_plating",    "Adaptive Plating",     32, ["field_ops"],       ("abilities", ["hardened_shell", "overclock", "ablative_layer"])),
 ("deep_analysis",       "Deep Analysis",        46, ["field_ops"],       ("abilities", ["deep_scan", "trace_analysis", "stealth_protocol", "salvage_routine"])),
 ("kernel_privileges",   "Kernel Privileges",    48, ["runtime_patching"], ("abilities", ["null_route"])),
 ("firewall",            "Firewall Plating",     22, ["armor_bench"],     ("recipe", ["firewall_plating", "armory", "6"])),
 ("ablative",            "Ablative Lattice",     40, ["firewall"],        ("recipe", ["ablative_plating", "armory", "12"])),
 ("neural_amp",          "Neural Interfacing",   25, ["weapon_bench"],    ("recipe", ["neural_amplifier", "fabricator", "6"])),
 ("cortex",              "Cortex Hacking",       45, ["neural_amp"],      ("recipe", ["cortex_hack", "fabricator", "12"])),
 ("overclock",           "Overclock Cores",      22, ["weapon_bench"],    ("recipe", ["overclock_core", "fabricator", "6"])),
 ("monofilament",        "Monofilament Edge",    40, ["overclock"],       ("recipe", ["monofilament_whip", "fabricator", "12"])),
]
K = "id name cost req unlocks".split()
R = [dict(zip(K, r)) for r in N]
BY = {r["id"]: r for r in R}

# Every recipe node in the tree is priced in this one item, which is what
# makes the "researched gear costs breach currency" claim below checkable
# rather than asserted.
RECIPE_CURRENCY = "portal_fragment"

roots = [r for r in R if not r["req"]]
kids = {r["id"]: [c for c in R if r["id"] in c["req"]] for r in R}


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

doc = f"""# Research tree

Every shipped research node in feral-processes, charted from its own file in
`assets/research/`. Twenty of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/research/*.ron` on 2026-08-05 and will drift the moment one of those
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
| unlocks | {structures_unlocked} structures, {abilities_unlocked} routines, {counts["recipe"]} gear recipes |

## The tree

{tree()}

Three roots, and they are three different games. **Automation** is the trunk:
everything that makes a base do work hangs off it, and it is also the cheapest
node in the tree at {BY["automation"]["cost"]}, so the opening move is barely a
decision. **Power Grid** is a two-node stub that ends in defence.
**Isometric Commerce** is a leaf — {BY["commerce"]["cost"]} Research Data buys
the iso Market and leads nowhere, which makes it the one node you take purely
because you want the thing rather than the branch.

Under Automation the tree splits three ways and never rejoins: benches
(Reactive Armor, Weapon Fabrication) lead to **gear recipes**, and Routine
Fabrication leads to **routines**. Nothing in the tree requires two parents —
every `requires` is a single id — so this is a tree in the strict sense, and
there is no node you can reach two ways.

## What each node unlocks

{table(["Node", "Cost", "Needs", "Unlocks"],
       [[r["name"], r["cost"], f'`{r["req"][0]}`' if r["req"] else "-", unlock_text(r)]
        for r in sorted(R, key=lambda r: (r["depth"], r["name"]))],
       ["l", "r", "l", "l"])}

A structure named by **no** research file is buildable from turn one — the
tree gates the machines that automate a base, not the base itself.

## What it costs to get there

A node's own `cost` is not what it costs you. Everything above it has to be
unlocked first, so the real price of Monofilament Edge is its own 40 plus the
whole chain behind it.

{cost_ladder()}

The shape to notice is the {len(deep_leaves)} end-of-branch nodes: {DEEP_NAMES}.
Each carries {DEEP_LO}-{DEEP_HI} Research Data of prerequisites behind it,
which is at least what the dearest single node in the whole tree costs on its
own ({DEAREST}). The tree is not steep at the top; it is long.

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
