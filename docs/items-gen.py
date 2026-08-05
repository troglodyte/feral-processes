# Regenerates docs/items.md. Run from the repo root: python3 docs/items-gen.py
#
# Transcribed from assets/items/*.ron by hand rather than parsed from it, for
# the same reason docs/roster-gen.py is. Update the table when an item moves,
# then rerun.
#
# `bench` is `craftable.requires_structure`, kept beside `recipe` rather than
# folded into it because the two gate separately: an item can be craftable
# with no bench (hand-made anywhere) but never benched with no recipe.
# `recipe` is empty for the six items whose recipe is granted by a research
# node rather than declared on the item -- see docs/research.md.
I = [
 # id                    name                        val  role        slot      stats                    recipe                                               bench          drops                              cache pot
 ("ablative_plating",    "Ablative Plating",          25, "",         "Armor",  "def+4",                 "",                                                  "",            "rootkit 30%",                     0, 0),
 ("access_shard",        "Access Shard",              12, "",         "",       "",                      "",                                                  "",            "",                                0.35, 0),
 ("arc_lance",           "Arc Lance",                 12, "",         "Weapon", "atk+3",                 "12 core_fragment",                                  "fabricator",  "scrapper 10%;worm 8%",            0, 0),
 ("bastion_lattice",     "Bastion Lattice",           80, "",         "Armor",  "def+4",                 "12 portal_fragment+4 bytecode_block+2 charge_coil", "armory",      "sentinel 8%;rootkit 6%",          0, 0),
 ("black_ice_pick",      "Black ICE Pick",            90, "",         "Weapon", "atk+3 decompiler+2",    "18 portal_fragment",                                "fabricator",  "cipher 7%;rootkit 6%",            0, 0),
 ("blank_substrate",     "Blank Substrate",            3, "",         "",       "",                      "4 core_fragment",                                   "lathe",       "",                                0, 0),
 ("bytecode_block",      "Bytecode Block",             4, "",         "",       "",                      "4 core_fragment",                                   "refinery",    "",                                0, 0),
 ("charge_coil",         "Charge Coil",                3, "",         "",       "",                      "3 power_cell",                                      "winding_node", "",                                0, 0),
 ("core_fragment",       "Core Fragment",              1, "Currency", "",       "",                      "",                                                  "",            "",                                0, 0),
 ("cortex_hack",         "Cortex Hack",               25, "",         "Module", "decompiler+3",          "",                                                  "",            "cipher 35%",                      0, 0),
 ("credits",             "Credits",                    0, "TradeCurrency", "",       "",                      "",                                                  "",            "",                                0, 0),
 ("entropy_damper",      "Entropy Damper",            15, "",         "Module", "decompiler+2 def+1",    "2 logic_wafer+3 charge_coil",                       "fabricator",  "crawler 8%;proxy 7%",             0, 0),
 ("firewall_plating",    "Firewall Plating",          20, "",         "Armor",  "def+3",                 "",                                                  "",            "sentinel 35%;crawler 20%",        0, 0),
 ("handshake_forge",     "Handshake Forge",            8, "",         "Module", "decompiler+2",          "8 core_fragment",                                   "",            "drone 9%;sub_process 7%",         0, 0),
 ("hardened_shell",      "Hardened Shell",            12, "",         "Armor",  "def+3",                 "3 bytecode_block",                                  "armory",      "crawler 10%;sentinel 8%",         0, 0),
 ("ice_breaker",         "ICE Breaker",                1, "",         "",       "",                      "3 core_fragment",                                   "",            "",                                0.3, 0.4),
 ("kernel_key",          "Kernel Key",                80, "",         "Module", "decompiler+4",          "12 portal_fragment+5 logic_wafer+2 charge_coil",    "fabricator",  "cipher 8%;virus 6%",              0.02, 0),
 ("kinetic_edge",        "Kinetic Edge",               7, "",         "Weapon", "atk+2",                 "7 core_fragment",                                   "",            "sub_process 10%;glitch 8%",       0, 0),
 ("logic_probe",         "Logic Probe",               15, "",         "Module", "decompiler+2 atk+1",    "3 logic_wafer+1 charge_coil+1 bytecode_block",      "fabricator",  "trojan 8%;scrapper 7%",           0.08, 0),
 ("logic_wafer",         "Logic Wafer",                3, "",         "",       "",                      "4 raw_trace",                                       "transcriber", "",                                0, 0),
 ("monofilament_whip",   "Monofilament Whip",         60, "",         "Weapon", "atk+4",                 "",                                                  "",            "wintermute 60%",                  0, 0),
 ("neural_amplifier",    "Neural Amplifier",          20, "",         "Module", "decompiler+2",          "",                                                  "",            "zero_day 30%;overseer 50%;proxy 30%;virus 25%", 0, 0),
 ("null_weave",          "Null Weave",                14, "",         "Armor",  "def+2 atk+1",           "3 bytecode_block+1 charge_coil",                    "armory",      "proxy 9%;trojan 7%",              0, 0),
 ("nullsteel_plate",     "Nullsteel Plate",           90, "",         "Armor",  "def+3 decompiler+2",    "14 portal_fragment+3 bytecode_block+1 charge_coil+2 logic_wafer", "armory",      "zero_day 6%;cipher 6%",           0, 0),
 ("oracle_core",         "Oracle Core",               90, "",         "Module", "decompiler+3 atk+2",    "14 portal_fragment+4 logic_wafer+1 charge_coil+2 bytecode_block", "fabricator",  "overseer 12%;rootkit 5%",         0, 0),
 ("outlet",              "Power Outlet",               5, "",         "",       "",                      "5 core_fragment",                                   "",            "",                                0, 0),
 ("overclock_core",      "Overclock Core",            22, "",         "Weapon", "atk+3",                 "",                                                  "",            "construct 30%;scrapper 15%;trojan 20%", 0, 0),
 ("packet_buffer",       "Packet Buffer",              7, "",         "Armor",  "def+2",                 "7 core_fragment",                                   "",            "drone 10%;sub_process 8%",        0, 0),
 ("patch_routine",       "Patch Routine",              7, "",         "",       "",                      "3 charge_coil",                                     "assembly_bay", "",                                0, 0),
 ("phase_carapace",      "Phase Carapace",            90, "",         "Armor",  "def+3 atk+2",           "14 portal_fragment+5 bytecode_block+1 charge_coil", "armory",      "zero_day 7%;virus 6%",            0, 0),
 ("plasma_router",       "Plasma Router",             80, "",         "Weapon", "atk+4",                 "16 portal_fragment",                                "fabricator",  "construct 8%;virus 6%",           0, 0),
 ("portal_fragment",     "Portal Fragment",            5, "CraftCurrency", "",       "",                      "",                                                  "",            "",                                0, 0),
 ("power_cell",          "Power Cell",                 1, "",         "",       "",                      "2 core_fragment",                                   "",            "",                                0.15, 0),
 ("probe_service",       "Probe Service",              5, "",         "Module", "decompiler+1",          "5 core_fragment",                                   "",            "sprite 9%;glitch 7%",             0.12, 0),
 ("raw_trace",           "Raw Trace",                  1, "",         "",       "",                      "",                                                  "",            "",                                0, 0),
 ("recursion_blade",     "Recursion Blade",           14, "",         "Weapon", "atk+2 def+1",           "14 core_fragment",                                  "fabricator",  "trojan 10%;proxy 7%",             0, 0),
 ("research_data",       "Research Data",              1, "ResearchCurrency", "",       "",                      "",                                                  "",            "",                                0, 0),
 ("routine_disk",        "Routine Disk",               5, "",         "",       "",                      "2 blank_substrate",                                 "disk_press",  "",                                0, 0),
 ("scrap_ward",          "Scrap Ward",                 4, "",         "Armor",  "def+1",                 "4 core_fragment",                                   "",            "glitch 10%;sprite 8%",            0.12, 0),
 ("shim_blade",          "Shim Blade",                14, "",         "Weapon", "atk+2 decompiler+1",    "14 core_fragment",                                  "fabricator",  "worm 9%;scrapper 7%",             0, 0),
 ("shiv_routine",        "Shiv Routine",               4, "",         "Weapon", "atk+1",                 "4 core_fragment",                                   "",            "sprite 10%;drone 8%",             0, 0),
 ("siege_compiler",      "Siege Compiler",            90, "",         "Weapon", "atk+3 def+2",           "18 portal_fragment",                                "fabricator",  "construct 7%;sentinel 6%",        0, 0),
 ("singularity_matrix",  "Singularity Matrix",       120, "",         "Module", "atk+3 def+3 decompiler+3", "20 portal_fragment+3 logic_wafer+3 charge_coil+2 bytecode_block", "fabricator",  "wintermute 15%;overseer 8%",      0, 0),
 ("static_mesh",         "Static Mesh",               14, "",         "Armor",  "def+2 decompiler+1",    "2 bytecode_block+1 charge_coil+1 logic_wafer",      "armory",      "crawler 8%;worm 7%",              0.08, 0),
 ("sync_governor",       "Sync Governor",             16, "",         "Module", "atk+1 def+1 decompiler+1", "2 logic_wafer+2 charge_coil+1 bytecode_block",      "fabricator",  "worm 7%;trojan 6%",               0, 0),
 ("trace_sniffer",       "Trace Sniffer",             13, "",         "Module", "decompiler+3",          "5 logic_wafer",                                     "fabricator",  "proxy 10%;zero_day 7%",           0.08, 0),
]
K = "id name value role slot stats recipe bench drops cache potency".split()
R = [dict(zip(K, r)) for r in I]

# The value ladder the economy is designed around. Bands are named here rather
# than derived, because the bands are the design statement and the prices are
# what has to keep agreeing with them.
BANDS = [
    (1, 2, "printable", "a base can make it out of nothing"),
    (3, 8, "scavenged", "salvage and intermediates"),
    (12, 16, "standard", "the craftable working set"),
    (20, 60, "researched", "needs a node and a bench"),
    (80, 120, "premium", "portal_fragment gear"),
]

SLOTS = ["Weapon", "Armor", "Module"]
gear = [r for r in R if r["slot"]]
craftable = [r for r in R if r["recipe"]]
benched = [r for r in R if r["bench"]]
droppers = [r for r in R if r["drops"]]


def table(header, rows, align):
    sep = "|" + "|".join(("---:" if a == "r" else ":---") for a in align) + "|"
    body = "\n".join("| " + " | ".join(str(c) for c in r) + " |" for r in rows)
    return "| " + " | ".join(header) + " |\n" + sep + "\n" + body


def band_of(r):
    for lo, hi, label, _ in BANDS:
        if lo <= r["value"] <= hi:
            return label
    return "unbanded"


def ladder():
    out = ["THE VALUE LADDER", ""]
    for lo, hi, label, gloss in BANDS:
        members = sorted((r for r in R if lo <= r["value"] <= hi), key=lambda r: r["value"])
        out.append(f'{label:<11} {lo:>3}-{hi:<4} {len(members):>2} items   {gloss}')
    strays = [r for r in R if band_of(r) == "unbanded"]
    out.append("")
    out.append(
        "every item sits in a band"
        if not strays
        else "unpriced: " + ", ".join(f'{r["name"]} ({r["role"] or "no role"})' for r in strays)
    )
    return "```\n" + "\n".join(out) + "\n```"


def slot_chart(width=26):
    """What each slot is worth at the top of its range."""
    out = ["EQUIPMENT BY SLOT", ""]
    hi = max(r["value"] for r in gear)
    for slot in SLOTS:
        members = sorted((r for r in gear if r["slot"] == slot), key=lambda r: -r["value"])
        out.append(f"{slot} ({len(members)})")
        for r in members:
            n = round(r["value"] / hi * width)
            out.append(f'  {r["name"]:<22} {r["value"]:>3}  {"#" * n}{"." * (width - n)}  {r["stats"]}')
        out.append("")
    return "```\n" + "\n".join(out).rstrip() + "\n```"


def drop_table():
    """Which species pay out, and how often, across the whole item set."""
    tally = {}
    for r in droppers:
        for entry in r["drops"].split(";"):
            species, pct = entry.rsplit(" ", 1)
            tally.setdefault(species, []).append((r["name"], pct))
    out = ["WHO DROPS WHAT", ""]
    w = max(len(s) for s in tally)
    for species in sorted(tally, key=lambda s: -len(tally[s])):
        items = ", ".join(f"{n} {p}" for n, p in sorted(tally[species], key=lambda t: -float(t[1][:-1])))
        out.append(f"{species:<{w}}  {items}")
    return "```\n" + "\n".join(out) + "\n```", tally


DROPS_CHART, TALLY = drop_table()

doc = f"""# Item catalogue

Every shipped item in feral-processes, charted from its own file in
`assets/items/`. Forty-six of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/items/*.ron` on 2026-08-05 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

`ItemId` is a string newtype rather than an enum, so a new item never requires
touching Rust — shipped ones stay reachable from code through the `ids` module
for test setup and data-defined recipes, and nothing else needs them.

| | |
|---|---|
| items | {len(R)} |
| equipment | {len(gear)} across {len(SLOTS)} slots |
| craftable | {len(craftable)} |
| need a bench | {len(benched)} |
| drop from a kill | {len(droppers)} |
| species that drop anything | {len(TALLY)} |

## The value ladder

An item's `value` is the one place a price is decided — selling, buying back
and the trade screen all read it, and a trader's `sell_rate` is a multiplier
on it rather than a price of its own. So the ladder below is the economy.

{ladder()}

One item sits outside the ladder entirely, and correctly: **Credits** carry no
`value` at all, because they are what a price is denominated in rather than
something with a price. Core Fragments are the opposite case — a currency that
*is* priced, at the floor, because a base prints them.

The bands are a design statement, not an observation: worth comes from what a
base **cannot** manufacture. That is why the printable end is worth 1-2 and
the premium end is gated behind `portal_fragment`, the currency you only get
by fighting.

Two bounds hold it in place, and the second is the one that is not obvious. A
craftable worth more than its ingredients is an infinite Credit loop — that
much is guessable, and build salvage is deliberately sellable while a Mining
Node produces it forever. But a `work.produces` structure makes its item out
of *nothing* on a timer, so **that item's value is really a Credit-per-tick
rate** the recipe ceiling cannot see. Both are asserted over the real assets
by `no_craftable_item_is_worth_more_than_its_ingredients` and
`every_base_produced_item_sits_at_the_floor_price`, so a retune that breaks
one fails rather than quietly minting money.

## Equipment

{slot_chart()}

A worn item and a candidate to replace it are measured at two **different**
levels, and that is the point rather than a bug: gear locks in the zone level
it was equipped at and doubles per level, so the equip screen measures the
worn copy at its recorded level and every candidate at the current zone's.
Collapsing those to one level would hide the case the screen exists for — a
spare copy of the weapon already on your back is a real upgrade after a
breach.

## Recipes

{table(["Item", "Value", "Recipe", "Bench"],
       [[r["name"], r["value"], ", ".join(r["recipe"].split("+")), f'`{r["bench"]}`' if r["bench"] else "hand"]
        for r in sorted(craftable, key=lambda r: (r["bench"], r["value"]))],
       ["l", "r", "l", "l"])}

{len(benched)} of the {len(R)} items name a bench, and the nine assembler
recipes in the game are exactly the products of the nine machines — because a
machine runs its product's own `craftable.cost`, there is no second recipe on
the structure that could drift from the bench recipe, and every craftable a
mod adds is automatable for free.

## Drops

{DROPS_CHART}

A drop table is declared on the **item**, naming the species that pay it out,
rather than on the species naming its loot. That inversion is what lets a mod
add an item that drops from creatures it did not write, without editing
anyone else's species file.

---

Source of truth is `assets/items/`. A mod that drops a `.ron` file in that
directory joins the catalogue without a recompile, and will not appear above
until this page is regenerated -- edit the table at the top of
[`docs/items-gen.py`](items-gen.py) and run `python3 docs/items-gen.py` from
the repo root. The schema is documented in
[`assets/items/README.md`](../assets/items/README.md).
"""

import pathlib
pathlib.Path("docs/items.md").write_text(doc)
print(doc)
