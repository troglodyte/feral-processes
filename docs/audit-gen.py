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
# call it one. A kind with no `fields` entry below reports ROWS OK, which means
# the ids line up and the columns were never looked at. Only `abilities` has
# been diffed cell by cell. Adding a `fields` extractor for another kind is how
# that list grows -- and expect to find two kinds of *false* positive when you
# do, both deliberate on the doc side: a column rendered for a human rather
# than transcribed raw (abilities' `status` is "Stun 30% 1r" against the RON's
# `chance: 0.3, duration: 1`), and a column that belongs to a nested part of
# the effect rather than the effect itself (abilities' `dur` is 0 on a Damage
# row whose *rider* carries the duration). Encode those in the extractor, so
# the script's silence keeps meaning something.
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
    """Ids of `assets/<name>/*.ron`, which is the file stem for every kind that
    ships one file per entry. Returns the directory too, so the one place the
    directory is named is also the place a field extractor reads from."""
    directory = ROOT / "assets" / name

    def source():
        return {p.stem for p in directory.glob("*.ron")}

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
            m.group(1)
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
    "research": dict(table="N", source=ron_dir("research"), optional_tail=1),
    "roster": dict(table="S", source=ron_dir("species")),
    "structures": dict(table="S", source=ron_dir("structures")),
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
    table = {r[0]: dict(zip(keys, r)) for r in rows}
    real = spec["source"]()
    shortest = len(keys) - spec.get("optional_tail", 0)
    ragged = [r[0] for r in rows if not shortest <= len(r) <= len(keys)]
    missing = sorted(real - set(table))
    phantom = sorted(set(table) - real)

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
        directory = spec["source"].directory
        for ident in sorted(set(table) & real):
            for key, value in spec["fields"](directory / f"{ident}.ron").items():
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
