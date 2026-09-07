# Diffs each docs/*-gen.py's hand-transcribed table against the source it
# claims to transcribe. Run from the repo root:
#     python3 docs/audit-gen.py
#
# Exits non-zero if any table has drifted, so it can gate a docs change.
#
# WHY THIS EXISTS. Every `*-gen.py` carries a hand-copied table at the top and
# derives its page from it; nothing parses the assets, and the `-gen` suffix
# reads as though something does. So an asset change does not merely make a
# page stale -- the page keeps regenerating confidently from a table nobody
# updated, and every count on it is derived from the stale table and therefore
# self-consistent. There is no way to see that by reading the page.
#
# It has happened at least five times: research short one node for a release
# (2026-08-13), abilities short six (2026-08-25) and then short seven again
# (2026-09-07), items short nine, structures short five *and* carrying a row
# for an asset that no longer exists.
#
# TWO FAILURE MODES, AND THE SECOND IS THE QUIET ONE. A missing row is at
# least a count. A stale *column* is invisible: on 2026-09-07 abilities' `cost`
# column read 0 for 50 of 78 rows, and nine rows still named `BuffKind::Def`, a
# variant deleted when mitigation became percentage points -- so the page
# printed 4 where the asset said 12, fluently, with four paragraphs of prose
# built on top explaining the numbers.
#
# SO A ROW CENSUS IS NOT A CLEAN BILL OF HEALTH, and this script refuses to
# call it one. A kind with no `fields` entry below reports ROWS ONLY, which
# means the ids line up and the columns were never looked at. Three kinds have
# been diffed cell by cell -- abilities, research, structures -- and the other
# four have not. Adding a `fields` extractor is how that list grows, and every
# one written so far found real rot the row census could not see: a `cost`
# column of zeros, a lost `harness_puller`, a Zone Portal priced at 10 when it
# costs 24 and three crafted items.
#
# EXPECT FALSE POSITIVES WHEN YOU WRITE ONE, and encode them rather than
# living with them, or the script's silence stops meaning anything. Four kinds
# have turned up so far, all deliberate on the doc side:
#
#   - a column rendered for a human rather than transcribed raw (abilities'
#     `status` is "Stun 30% 1r" against the RON's `chance: 0.3, duration: 1`);
#   - a column belonging to a nested part of a structure rather than its top
#     (abilities' `dur` is 0 on a Damage row whose *rider* carries one);
#   - a column that is not in the assets at all (structures' `feeder` is a
#     property of the items, and that script's header says so), or that is
#     data for some rows and hand-written prose for others (structures' `does`
#     names an item for a producer and describes the thing for a utility);
#   - a trailing column a row may omit, which `zip` then truncates away --
#     `research` does this deliberately and backfills with `setdefault`, so
#     the config carries `optional_tail` and `defaults` to say so.
#
# And write the extractor against the file, not against the table: a regex
# that agrees with the doc is not evidence. Two of the three here were wrong
# on their first run and the *table* was right -- `requires_structure` fell
# outside a non-greedy match that stopped at the first `)]`, which sits inside
# the recipe's own cost list.
#
# AFTER FIXING A TABLE, RE-READ THE PROSE. A correct regeneration leaves false
# sentences standing, because the f-string's prose is hand-written while the
# numbers around it are computed. Adding abilities' seven missing rows left
# "six exclusives ship" (seven do) and "Packet Shred is the one family that
# doesn't pay" for reach (the newly-added Segfault tiers pay the same way).
# Hunt the superlatives: "the only", "leads", "nothing else comes near".

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def ron_dir(name):
    """Ids of `assets/<name>/*.ron`, read from each file's own `id:` field.

    **Not the file stem**, which is what this did first and got wrong:
    `assets/structures/black_market.ron` declares `id: "market"`. The file was
    renamed and the id was not, which is the right way round -- the id is what
    code and saved games reference, and the filename is for humans. Keyed on
    the stem, the audit reported the doc table's correct `market` row as a
    phantom and demanded a `black_market` row that would have been wrong. One
    file in the whole asset tree does this, which is exactly the number it
    takes to make an assumption feel safe.

    Returns the directory too, so the one place a directory is named is also
    the place a field extractor reads from.
    """
    directory = ROOT / "assets" / name

    def source():
        ids = {}
        for path in directory.glob("*.ron"):
            m = re.search(r'^\s*id:\s*"([^"]*)"', path.read_text(), re.M)
            ids[m.group(1) if m else path.stem] = path
        return ids

    source.directory = directory
    return source


def rust_enum(relpath, enum_name):
    """Variant names of a Rust enum -- the source of truth for the one page
    whose subject is code rather than data. Reads the braced body so a doc
    comment between variants cannot truncate the list, which is what a bare
    line-wise grep does."""

    def source():
        text = (ROOT / relpath).read_text()
        start = text.index(f"pub enum {enum_name} {{") + len(f"pub enum {enum_name} {{")
        depth, i = 1, start
        while depth:
            depth += {"{": 1, "}": -1}.get(text[i], 0)
            i += 1
        body = text[start : i - 1]
        return {
            m.group(1): None
            for m in re.finditer(r"^\s*([A-Z][A-Za-z0-9]*)\s*,", body, re.M)
        }

    source.directory = None
    return source


def abilities_fields(path):
    """One ability's columns as `abilities-gen.py`'s table spells them.

    The two renderings this has to reproduce rather than report are called out
    in the header: `status` is written for a human, and `dur` belongs to the
    effect and not to a Damage rider that happens to carry one.
    """
    text = "\n".join(
        line
        for line in path.read_text().split("\n")
        if not line.strip().startswith("//")
    )

    def num(key, where=None, default=0):
        m = re.search(rf"\b{key}:\s*(-?[0-9.]+)", where if where is not None else text)
        if not m:
            return default
        value = float(m.group(1))
        return int(value) if value == int(value) else value

    effect = re.search(r"\beffect:\s*([A-Za-z]+)", text).group(1)
    inner = re.search(r"\beffect:\s*[A-Za-z]+\((.*?)\)\s*,\s*\n", text, re.S)
    inner = inner.group(1) if inner else ""

    kind = re.search(r"\bkind:\s*([A-Za-z]+)", inner)
    sub = kind.group(1) if kind and effect in ("FieldBuff", "Buff", "Debuff") else ""

    rider = re.search(
        r"status:\s*Some\(\(\s*kind:\s*([A-Za-z]+),\s*chance:\s*([0-9.]+),"
        r"\s*duration:\s*([0-9]+)",
        text,
        re.S,
    )
    status = (
        f"{rider.group(1)} {round(float(rider.group(2)) * 100)}% {rider.group(3)}r"
        if rider
        else ""
    )

    return {
        "name": re.search(r'\bname:\s*"([^"]*)"', text).group(1),
        "target": re.search(r"\btarget:\s*([A-Za-z]+)", text).group(1),
        "effect": effect,
        "sub": sub,
        "power": num("power", inner),
        "spread": num("spread", inner),
        "dur": 0 if rider else num("duration", inner),
        "status": status,
        "cd": num("cooldown"),
        "cost": num("power_cost"),
    }


def structures_fields(path):
    """One structure's columns as `structures-gen.py`'s table spells them.

    Two columns are deliberately not checked. **`feeder`** is not in the file
    at all -- a machine runs its product's own `craftable.cost`, so which
    machine feeds which is a property of the *items*, and that script's header
    says so. **`does`** is the produced item's id for a producer or assembler
    and a hand-written phrase for a utility ("anchors the base, radius 4 and
    up"), so it is checked only where it is data.
    """
    text = "\n".join(
        line
        for line in path.read_text().split("\n")
        if not line.strip().startswith("//")
    )

    def block(key):
        m = re.search(rf"\b{key}:\s*Some\(\(", text)
        if not m:
            return ""
        depth, i = 1, m.end()
        while depth:
            depth += {"(": 1, ")": -1}.get(text[i], 0)
            i += 1
        return text[m.end() : i - 1]

    def num(key, where=None, default=None):
        m = re.search(rf"\b{key}:\s*(-?[0-9.]+)", text if where is None else where)
        if not m:
            return default
        value = float(m.group(1))
        return int(value) if value == int(value) else value

    work, assembles, upgrade = block("work"), block("assembles"), block("upgrade")
    if work:
        kind, does, ticks = "producer", re.search(r'produces:\s*"([^"]*)"', work).group(1), num("ticks_per_unit", work)
    elif assembles:
        kind, does, ticks = "assembler", re.search(r'item:\s*"([^"]*)"', assembles).group(1), num("ticks_per_unit", assembles)
    else:
        kind, does, ticks = "utility", None, None

    cost = re.search(r"build_cost:\s*\[(.*?)\]", text, re.S).group(1)
    fields = {
        "name": re.search(r'\bname:\s*"([^"]*)"', text).group(1),
        "glyph": re.search(r"\bglyph:\s*'(.)'", text).group(1),
        "color": re.search(r"\bcolor:\s*([A-Za-z]+)", text).group(1),
        "cost": [
            (m.group(1), int(m.group(2)))
            for m in re.finditer(r'\("([a-z_]+)",\s*([0-9]+)\)', cost)
        ],
        "kind": kind,
        "ticks": ticks,
        "cap": num("capacity"),
        # The table records the core_fragment half of an upgrade's cost, which
        # is the only part that differs between structures.
        "upgrade": next(
            (
                int(m.group(1))
                for m in re.finditer(r'\("core_fragment",\s*([0-9]+)\)', upgrade)
            ),
            None,
        ),
        "draw": num("power_draw", default=0),
        "supply": num("power_supply", default=0),
    }
    if does is not None:
        fields["does"] = does
    return fields


def research_fields(path):
    """One research node's columns as `research-gen.py`'s table spells them.

    The table's `unlocks` collapses three mutually-exclusive asset fields into
    one `(kind, [...])` pair, and the third of them is flattened rather than
    listed: a `recipe` reads `[result, bench, portal_fragment cost]`, not a
    list of ids. `unlocks_tools` stays a column of its own, because a node may
    grant tools *as well as* its main payload -- `deep_analysis` does, and it
    is the row where that column had silently lost `harness_puller`.
    """
    text = "\n".join(
        line
        for line in path.read_text().split("\n")
        if not line.strip().startswith("//")
    )

    def id_list(key):
        m = re.search(rf"\b{key}:\s*\[(.*?)\]", text, re.S)
        return [x.group(1) for x in re.finditer(r'"([^"]*)"', m.group(1))] if m else []

    # Scanned to the matching paren rather than matched with `.*?`, which
    # stops at the first `)]` -- and that lands inside the recipe's own `cost`
    # list, cutting `requires_structure` off the end of every one of them.
    recipes = re.search(r"unlocks_recipes:\s*\[\(", text)
    if recipes:
        depth, i = 1, recipes.end()
        while depth:
            depth += {"(": 1, ")": -1}.get(text[i], 0)
            i += 1
        body = text[recipes.end() : i - 1]
        bench = re.search(r'requires_structure:\s*Some\("([^"]*)"\)', body)
        fragment = re.search(r'\("portal_fragment",\s*([0-9]+)\)', body)
        unlocks = (
            "recipe",
            [
                re.search(r'result:\s*"([^"]*)"', body).group(1),
                bench.group(1) if bench else None,
                # A string in the table, not an int -- it is rendered into a
                # sentence rather than summed.
                fragment.group(1) if fragment else None,
            ],
        )
    elif id_list("unlocks_abilities"):
        unlocks = ("abilities", id_list("unlocks_abilities"))
    else:
        unlocks = ("structures", id_list("unlocks_structures"))

    zone = re.search(r"\bmin_zone:\s*([0-9]+)", text)
    return {
        "name": re.search(r'\bname:\s*"([^"]*)"', text).group(1),
        # No `min_zone` means available from turn one, which the table spells 0.
        "zone": int(zone.group(1)) if zone else 0,
        "cost": int(re.search(r"^\s*cost:\s*([0-9]+)", text, re.M).group(1)),
        "req": id_list("requires"),
        "unlocks": unlocks,
        "tools": id_list("unlocks_tools"),
    }


# `table` is the name of the tuple list at the top of the script -- they are
# not consistently named, and guessing wrong reads as a missing table rather
# than a typo. `fields` is optional: absent means row-census only, and the
# report says so rather than calling it clean.
KINDS = {
    "abilities": dict(table="A", source=ron_dir("abilities"), fields=abilities_fields),
    "achievements": dict(table="A", source=ron_dir("achievements")),
    "items": dict(table="I", source=ron_dir("items")),
    # `optional_tail`: how many trailing cells a row may legitimately omit.
    # `research-gen.py` says so in a comment above its own table -- a node
    # granting no tool simply stops one short, and a `setdefault` fills it back
    # in. Without this the ragged check below reports 20 rows that are correct.
    "research": dict(
        table="N", source=ron_dir("research"), optional_tail=1,
        # What the gen script's own `setdefault` puts in an omitted trailing
        # cell. Stated here rather than assumed, because "absent" and "empty"
        # are only the same thing if the script says so -- and this one does,
        # right below its table.
        defaults={"tools": []},
        fields=research_fields,
    ),
    "roster": dict(table="S", source=ron_dir("species")),
    "structures": dict(
        table="S", source=ron_dir("structures"), fields=structures_fields
    ),
    "perks": dict(
        table="P", source=rust_enum("crates/engine/src/perks.rs", "Perk")
    ),
}


def table_of(script, var):
    """The `<var> = [...]` literal out of a gen script, without running the
    rest of it -- importing would rewrite the .md as a side effect."""
    src = (ROOT / "docs" / f"{script}-gen.py").read_text()
    start = src.index(f"\n{var} = [")
    end = src.index("\nK = ", start)
    namespace = {}
    exec(src[start:end], namespace)  # noqa: S102 -- our own file, not input
    keys = re.search(r'\nK = "([^"]*)"', src).group(1).split()
    rows = namespace[var]
    # `R = [dict(zip(K, r))]` in every one of these scripts, and `zip` stops at
    # the shorter side -- a row a field short is silently truncated rather than
    # raising, so a column added to K without every tuple gaining a cell loses
    # that tuple's last value with no error anywhere. Checked here because the
    # gen script never will.
    return keys, rows


def audit(script, spec):
    keys, rows = table_of(script, spec["table"])
    table = {}
    for row in rows:
        cells = dict(zip(keys, row))
        for key, default in spec.get("defaults", {}).items():
            cells.setdefault(key, default)
        table[row[0]] = cells
    real = spec["source"]()  # id -> the file it came from (None for an enum)
    shortest = len(keys) - spec.get("optional_tail", 0)
    ragged = [r[0] for r in rows if not shortest <= len(r) <= len(keys)]
    missing = sorted(set(real) - set(table))
    phantom = sorted(set(table) - set(real))

    problems = []
    if ragged:
        problems.append(
            f"rows outside {shortest}..{len(keys)} cells: {', '.join(ragged)}"
        )
    if missing:
        problems.append(f"absent from the table ({len(missing)}): {', '.join(missing)}")
    if phantom:
        problems.append(
            f"in the table, absent from source ({len(phantom)}): {', '.join(phantom)}"
        )

    checked_columns = False
    if spec.get("fields"):
        checked_columns = True
        for ident in sorted(set(table) & set(real)):
            for key, value in spec["fields"](real[ident]).items():
                if str(table[ident][key]) != str(value):
                    problems.append(
                        f"{ident}.{key}: table={table[ident][key]!r} "
                        f"source={value!r}"
                    )

    if problems:
        print(f"!! {script:14} {len(table):3} rows")
        for line in problems:
            print(f"     {line}")
        return False
    print(
        f"OK {script:14} {len(table):3} rows"
        + ("  rows and columns" if checked_columns else "  ROWS ONLY - columns unchecked")
    )
    return True


if __name__ == "__main__":
    print("docs/*-gen.py tables against the sources they transcribe\n")
    clean = all([audit(script, spec) for script, spec in sorted(KINDS.items())])
    print(
        "\nA kind marked ROWS ONLY has had its ids checked and its cell values "
        "never looked at.\nSee this file's header before trusting one."
    )
    sys.exit(0 if clean else 1)
