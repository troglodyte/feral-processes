# Regenerates docs/roster.md. Run from the repo root: python3 docs/roster-gen.py
#
# The roster below is transcribed from assets/species/*.ron by hand rather than
# parsed from it -- a RON reader in Python would be a second parser to keep in
# step with the engine's, for a page nobody's build depends on. Update the table
# when a species file moves, then rerun.
S = [
 # id            name         glyph hp  atk def spd int tame  grow  biomes                                          yield           boss nest abilities                                      affinities
 ("wintermute",  "Wintermute", "W", 200, 19, 17, 13, 18, .95, 2.0,  ["OpenGrid","Mainframe","NullSector","StaticField"], None,            1, 0, ["broadcast_storm","null_route L4"],      None),
 ("overseer",    "Overseer",   "B", 180, 17, 15, 12, 16, .90, 2.0,  ["OpenGrid","Mainframe","NullSector","StaticField"], None,            1, 0, ["broadcast_storm","overclock_array L5"], None),
 ("sentinel",    "Sentinel",   "S", 136,  6, 12,  7,  8, .65, 1.5,  ["StaticField"],                                    None,            0, 0, ["overclock_array L2","bastion_shield_v3 L6"], "buff 1.3 / damage 0.85"),
 ("construct",   "Construct",  "C",  49,  2,  4,  6,  5, .35, 1.0,  ["Mainframe"],                                      "core_fragment", 0, 0, ["overclock_array L2","sandbox L6"],           "buff 1.3 / damage 0.85"),
 ("rootkit",     "Rootkit",    "k", 132, 12,  3,  9, 13, .75, 1.5,  ["Mainframe","NullSector"],                         None,            0, 0, ["skim_group L2","skim_v3 L6"],                "drain 1.3 / buff 0.85"),
 ("cipher",      "Cipher",     "c", 113, 15,  5, 13, 14, .80, 1.5,  ["Mainframe","StaticField"],                        None,            0, 0, ["deadlock L2","bit_rot_v3 L6"],               "debuff 1.3 / heal 0.85"),
 ("virus",       "Virus",      "v", 122, 10,  8, 12, 12, .60, 1.5,  ["NullSector","Mainframe"],                         "core_fragment", 0, 0, ["redundancy_sync L2","rollback_v3 L6"],       "heal 1.3 / damage 0.85"),
 ("zero_day",      "ZeroDay",    "z",  106, 16,  4, 10, 12, .65, 1.5,  ["StaticField","NullSector"],                       None,            0, 0, ["cascade_overflow L2","segfault_v3 L6"],      "damage 1.3 / heal 0.85"),
 ("worm",        "Worm",       "m",  99,  9,  2,  9, 11, .40, 1.25, ["NullSector","OpenGrid"],                          "core_fragment", 0, 1, ["skim_group L2","skim_v2 L6"],                "drain 1.3 / buff 0.85"),
 ("scrapper",    "Scrapper",   "x",   80, 12,  3, 10,  7, .45, 1.25, ["OpenGrid","NullSector"],                          "core_fragment", 0, 1, ["cascade_overflow L2","segfault_v2 L6"],      "damage 1.3 / heal 0.85"),
 ("trojan",      "Trojan",     "t",   85, 11,  4, 13, 13, .50, 1.25, ["Mainframe","OpenGrid"],                           None,            0, 1, ["deadlock L2","bit_rot_v2 L6"],               "debuff 1.3 / heal 0.85"),
 ("proxy",       "Proxy",      "p",   92,  7,  6, 12, 13, .55, 1.25, ["Mainframe","StaticField"],                        None,            0, 0, ["redundancy_sync L2","rollback_v2 L6"],       "heal 1.3 / damage 0.85"),
 ("crawler",     "Crawler",    "r",  102,  5,  9,  7,  8, .50, 1.25, ["StaticField"],                                    None,            0, 1, ["overclock_array L2","bastion_shield_v2 L6"], "buff 1.3 / damage 0.85"),
 ("sub_process", "SubProcess", "d",   43,  4,  3, 12, 15, .30, 1.0,  ["OpenGrid","NullSector"],                          "core_fragment", 0, 0, ["redundancy_sync L2","rollback_v1 L6"],       "heal 1.3 / damage 0.85"),
 ("sprite",      "Sprite",     "s",   41,  5,  2, 14, 11, .20, 1.0,  ["OpenGrid","Mainframe"],                           "core_fragment", 0, 0, ["deadlock L2","memory_leak L6"],              "debuff 1.3 / heal 0.85"),
 ("drone",       "Drone",      "o",   48,  4,  1,  8,  7, .15, 1.0,  ["OpenGrid","Mainframe"],                           "core_fragment", 0, 0, ["skim_group L2","skim_v1 L6"],                "drain 1.3 / buff 0.85"),
 ("glitch",      "Glitch",     "g",   38,  6,  1, 11,  5, .15, 1.0,  ["OpenGrid","NullSector"],                          "power_cell",    0, 0, ["cascade_overflow L2","segfault_v1 L6"],      "damage 1.3 / heal 0.85"),
]
K = "id name g hp atk def spd int tame grow bio yield boss nest ab aff".split()
R = [dict(zip(K, r)) for r in S]
for r in R:
    r["pow"] = r["hp"] + r["atk"] + r["def"]
R.sort(key=lambda r: -r["pow"])

def bars(key, title, width=44):
    hi = max(r[key] for r in R)
    w = max(len(r["name"]) for r in R)
    out = [title, ""]
    for r in R:
        n = round(r[key] / hi * width)
        out.append(f'{r["name"]:<{w}} {r[key]:>3}  {"█" * n}{"·" * (width - n)}')
    return "```\n" + "\n".join(out) + "\n```"

def scatter():
    xs, ys = 20, 18
    grid = {}
    for r in R:
        grid[(r["atk"], r["def"])] = r["g"]
    lines = []
    for y in range(ys, -1, -1):
        row = ""
        for x in range(xs + 1):
            if (x, y) in grid:
                row += grid[(x, y)] + " "
            elif y % 3 == 0 and x % 4 == 0:
                row += "+ "
            elif y % 3 == 0:
                row += "- "
            elif x % 4 == 0:
                row += "| "
            else:
                row += "  "
        lines.append(f"{y:>2} |{row.rstrip()}")
    lines.append("   +" + "-" * (2 * xs + 2))
    lines.append("    " + "".join(f"{x:<2}" if x % 4 == 0 else "  " for x in range(xs + 1)))
    lines.append("    " + "BASE ATK".rjust(0))
    return "```\nBASE DEF\n" + "\n".join(lines) + "\n```"

def speed_ladder():
    out = ["BASE SPEED", ""]
    for v in range(14, 5, -1):
        names = [r["name"] for r in R if r["spd"] == v]
        note = "   <- the player rolls from here" if v == 11 else ""
        out.append(f'{v:>2}  {", ".join(names) if names else "-"}{note}')
    return "```\n" + "\n".join(out) + "\n```"

def lanes():
    out = []
    for tier in (2.0, 1.5, 1.25, 1.0):
        members = [r for r in R if r["grow"] == tier]
        row = [" "] * 44
        for r in members:
            col = round((r["tame"] - 0.10) / 0.90 * 43)
            while col < 44 and row[col] != " ":
                col += 1
            if col < 44:
                row[col] = r["g"]
        out.append(f'x{tier:<5.2f} |{"".join(row).rstrip()}')
    out.append("       +" + "-" * 44)
    out.append("        0.10" + " " * 12 + "0.55" + " " * 15 + "0.95")
    out.append("        TAMING DIFFICULTY")
    return "```\nGROWTH\n" + "\n".join(out) + "\n```"

def table(header, rows, align):
    sep = "|" + "|".join(("---:" if a == "r" else ":---") for a in align) + "|"
    body = "\n".join("| " + " | ".join(str(c) for c in r) + " |" for r in rows)
    return "| " + " | ".join(header) + " |\n" + sep + "\n" + body

BIOMES = ["OpenGrid", "Mainframe", "NullSector", "StaticField"]

doc = f"""# Roster stat sheet

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

{table(["", "Species", "HP", "ATK", "DEF", "SPD", "INT", "POW", "Tame", "Growth"],
       [[f'`{r["g"]}`', r["name"] + (" **·boss**" if r["boss"] else ""), r["hp"], r["atk"],
         r["def"], r["spd"], r["int"], r["pow"], f'{r["tame"]:.2f}', f'x{r["grow"]:.2f}']
        for r in R],
       ["l", "l", "r", "r", "r", "r", "r", "r", "r", "r"])}

## Attack against defense

Each species sits at its own `(base_atk, base_def)`, drawn with its map glyph.
The top-right corner belongs to the two bosses alone, and the bottom-left to
the five programs the opening ring draws from. Between them the roster is laid
out by **class**: a species' position here is its role, and its distance from
the origin is its tier.

{scatter()}

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

{bars("hp", "BASE HP                             (max 200)")}

{bars("atk", "BASE ATK                             (max 19)")}

{bars("def", "BASE DEF                             (max 17)")}

{speed_ladder()}

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

`taming_difficulty` along the axis, `growth_multiplier` as the lane. A tie\nshifts one glyph right, so read the lanes for grouping rather than for an\nexact position.

{lanes()}

The two move together across the whole roster. Nothing is cheap to compile and
steep to level, and nothing is expensive and flat — so taming difficulty is a
straight read on long-term value, with no bargains and no traps.

## Habitats and yield

Four walkable biomes host spawns. `DataVoid` and `BlackIce` are barrier terrain
and `Platform` is base floor — no shipped species lists any of the three, which
is exactly what keeps a player's base free of wild spawns.

{table([""] + ["Species"] + BIOMES + ["Yield"],
       [[f'`{r["g"]}`', r["name"]] + ["#" if b in r["bio"] else "." for b in BIOMES]
        + [f'`{r["yield"]}`' if r["yield"] else "-"] for r in R],
       ["l", "l", "l", "l", "l", "l", "l"])}

Both bosses list all four biomes; seven species are single-biome. Glitch is the
only source of `power_cell` in the roster.

## Traits

{table(["", "Species", "Flags", "Abilities", "Affinities"],
       [[f'`{r["g"]}`', r["name"],
         " ".join(x for x in (["BOSS"] if r["boss"] else []) + (["NEST"] if r["nest"] else [])) or "-",
         ", ".join(f"`{a}`" for a in r["ab"]) or "-",
         r["aff"] or "baseline"] for r in R],
       ["l", "l", "l", "l", "l"])}

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
"""

import pathlib
pathlib.Path("docs/roster.md").write_text(doc)
print(doc[:1200])
