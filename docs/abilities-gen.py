# Regenerates docs/abilities.md. Run from the repo root:
#     python3 docs/abilities-gen.py
#
# Transcribed from assets/abilities/*.ron by hand rather than parsed from it,
# for the same reason docs/roster-gen.py is. Update the table when an ability
# moves, then rerun.
#
# `cost` is fatigue_cost for a battle routine and power_cost for a FieldBuff
# -- two different fields that are never both present, folded into one column
# because they are the same idea measured against different meters. `status`
# is the rider on a Damage effect, flattened to "kind chance duration".
A = [
 # id                    name                          target                 effect       sub            pow  dur  status             cd  cost
 ("ablative_layer",      "Ablative Layer Single",      "OneAlly",             "FieldBuff", "Mitigation",    10,   80, "",                0, 20.0),
 ("bastion",             "Bastion Party",              "WholeParty",          "Buff",      "Def",            4,    3, "",                3, 11.0),
 ("bit_rot",             "Bit Rot Everyone",           "AllEnemies",          "Debuff",    "Bleed",          2,    4, "",                5, 16.0),
 ("broadcast_storm",     "Packet Shred Everyone",      "AllEnemies",          "Damage",    "",              25,    0, "",                4, 15.0),
 ("bus_fault",           "Pipeline Stall Everyone",    "AllEnemies",          "Damage",    "",               6,    0, "Stun 25% 1r",     5, 18.0),
 ("cascade_overflow",    "Packet Shred Group v1.0",    "WholeEnemyGroup",     "Damage",    "",               6,    0, "",                2, 8.0),
 ("checksum_repair",     "Patch Single v2.0",          "OneAlly",             "Heal",      "",              25,    0, "",                3, 9.0),
 ("cold_boot",           "Patch Single v3.0",          "OneAlly",             "Heal",      "",              50,    0, "",                5, 15.0),
 ("coolant_flush",       "Coolant Flush Party",        "WholeParty",          "FieldBuff", "Coolant",        1,   90, "",                0, 15.0),
 ("deadlock",            "Hard Lock Single v1.0",      "OneEnemyGroupFront",  "Debuff",    "Stun",           0,    1, "",                2, 0),
 ("decompile",           "Decompile Single",           "OneEnemyGroupFront",  "Decompile", "",               0,    0, "",                0, 0.0),
 ("deep_scan",           "Deep Scan Party",            "WholeParty",          "FieldBuff", "CaptureBoost",   20,  100, "",                0, 18.0),
 ("etch",                "Etch Group",                 "WholeEnemyGroup",     "Buff",      "Def",           -4,    3, "",                3, 10.0),
 ("flush_cache",         "Flush Cache Party",          "WholeParty",          "Cleanse",   "",               0,    0, "",                3, 7.0),
 ("fork_bomb",           "Fork Bomb Group",            "WholeEnemyGroup",     "Damage",    "",              15,    0, "Bleed 35% 2r",    3, 12.0),
 ("hard_lock",           "Hard Lock Single v2.0",      "OneEnemyGroupFront",  "Debuff",    "Stun",           0,    2, "",                4, 10.0),
 ("hardened_shell",      "Hardened Shell Single",      "OneAlly",             "FieldBuff", "Def",            4,   90, "",                0, 14.0),
 ("heap_corruption",     "Bit Rot Group",              "WholeEnemyGroup",     "Debuff",    "Bleed",          3,    3, "",                3, 11.0),
 ("hot_patch",           "Patch Single v1.0",          "OneAlly",             "Heal",      "",               8,    0, "",                1, 0),
 ("hyperthread",         "Hyperthread Single v2.0",    "OneAlly",             "Buff",      "Atk",            6,    4, "",                3, 8.0),
 ("kernel_panic",        "Packet Shred Single",        "OneEnemyGroupFront",  "Damage",    "",              16,    0, "",                3, 10.0),
 ("leech_array",         "Leech Group",                "WholeEnemyGroup",     "Drain",     "",               6,    0, "",                4, 13.0),
 ("memory_leak",         "Bit Rot Single",             "OneEnemyGroupFront",  "Debuff",    "Bleed",          2,    3, "",                1, 0),
 ("mirror_restore",      "Patch Party v1.0",           "WholeParty",          "Heal",      "",               8,    0, "",                2, 10.0),
 ("null_route",          "Hard Lock Everyone",         "AllEnemies",          "Debuff",    "Stun",           0,    1, "",                5, 15.0),
 ("overclock",           "Overclock Single",           "OneAlly",             "FieldBuff", "Atk",            4,   90, "",                0, 14.0),
 ("overclock_array",     "Hyperthread Party",          "WholeParty",          "Buff",      "Atk",            3,    3, "",                3, 10.0),
 ("packet_shred",        "Packet Shred Group v2.0",    "WholeEnemyGroup",     "Damage",    "",              10,    0, "",                3, 11.0),
 ("pipeline_stall",      "Pipeline Stall Single",      "OneEnemyGroupFront",  "Damage",    "",               7,    0, "Stun 40% 1r",     3, 9.0),
 ("priority_boost",      "Hyperthread Single v1.0",    "OneAlly",             "Buff",      "Atk",            3,    3, "",                1, 0),
 ("race_condition",      "Hard Lock Group",            "WholeEnemyGroup",     "Debuff",    "Stun",           0,    1, "",                4, 13.0),
 ("redundancy_sync",     "Patch Party v1.1",           "WholeParty",          "Heal",      "",              10,    0, "",                3, 12.0),
 ("repair_loop",         "Repair Loop Single",         "OneAlly",             "FieldBuff", "Regen",          2,  100, "",                0, 18.0),
 ("salvage_routine",     "Salvage Routine Party",      "WholeParty",          "FieldBuff", "DropBoost",     20,  100, "",                0, 18.0),
 ("sandbox",             "Bastion Single",             "OneAlly",             "Buff",      "Def",            3,    3, "",                1, 0),
 ("siphon_cycles",       "Leech Single",               "OneEnemyGroupFront",  "Drain",     "",              10,    0, "",                2, 9.0),
 ("stack_smash",         "Fork Bomb Single",           "OneEnemyGroupFront",  "Damage",    "",               9,    0, "Bleed 60% 3r",    2, 8.0),
 ("stealth_protocol",    "Stealth Protocol Party",     "WholeParty",          "FieldBuff", "EncounterDamp",   20,   90, "",                0, 18.0),
 ("throttle",            "Throttle Group",             "WholeEnemyGroup",     "Buff",      "Atk",           -4,    3, "",                3, 10.0),
 ("trace_analysis",      "Trace Analysis Party",       "WholeParty",          "FieldBuff", "XpBoost",       20,  100, "",                0, 18.0),
 ("trickle_charge",      "Trickle Charge Party",       "WholeParty",          "FieldBuff", "Trickle",        1,   80, "",                0, 20.0),
]
K = "id name target effect sub power dur status cd cost".split()
R = [dict(zip(K, r)) for r in A]

# The display name is a spec, not flavour: "<effect> <scope> [vN.N]". Parsed
# back out here so the family ladder below is derived from the names rather
# than hand-grouped -- a new tier of an existing family joins its row for
# free, and a name that breaks the scheme shows up as its own family, which
# is the signal that it does.
SCOPES = ["Single", "Group", "Everyone", "Party"]


def split_name(name):
    words = name.split()
    version = words[-1] if words[-1].startswith("v") else None
    if version:
        words = words[:-1]
    scope = words[-1] if words[-1] in SCOPES else None
    if scope:
        words = words[:-1]
    return " ".join(words), scope, version


for r in R:
    r["family"], r["scope"], r["version"] = split_name(r["name"])

TARGETS = ["OneAlly", "WholeParty", "OneEnemyGroupFront", "WholeEnemyGroup", "AllEnemies"]
EFFECTS = ["Damage", "Debuff", "Buff", "Heal", "Drain", "FieldBuff", "Cleanse", "Decompile"]
FIELD = [r for r in R if r["effect"] == "FieldBuff"]
FREE = [r for r in R if r["cost"] == 0 and r["effect"] != "Decompile"]


def table(header, rows, align):
    sep = "|" + "|".join(("---:" if a == "r" else ":---") for a in align) + "|"
    body = "\n".join("| " + " | ".join(str(c) for c in r) + " |" for r in rows)
    return "| " + " | ".join(header) + " |\n" + sep + "\n" + body


def matrix():
    """Who it hits against what it does."""
    w = max(len(t) for t in TARGETS) + 2
    head = "".join(f"{e[:5]:>7}" for e in EFFECTS)
    out = [f'{"":<{w}}{head}', ""]
    for t in TARGETS:
        cells = ""
        for e in EFFECTS:
            n = sum(1 for r in R if r["target"] == t and r["effect"] == e)
            cells += f'{(str(n) if n else "."):>7}'
        out.append(f"{t:<{w}}{cells}")
    out.append("")
    out.append(f'{"":<{w}}' + "".join(f'{sum(1 for r in R if r["effect"] == e):>7}' for e in EFFECTS))
    return "```\nTARGET AGAINST EFFECT\n\n" + "\n".join(out) + "\n```"


def families():
    """Every family that ships more than one member, widest scope first."""
    order = {s: i for i, s in enumerate(SCOPES)}
    fams = {}
    for r in R:
        fams.setdefault(r["family"], []).append(r)
    out = ["ABILITY FAMILIES            (display name = effect + scope + tier)", ""]
    multi = {f: m for f, m in fams.items() if len(m) > 1}
    w = max(len(f) for f in multi)
    for fam, members in sorted(multi.items(), key=lambda kv: -len(kv[1])):
        members.sort(key=lambda r: (order.get(r["scope"], 9), r["version"] or ""))
        cells = ", ".join(
            f'{m["scope"]}{" " + m["version"] if m["version"] else ""} ({m["power"]})'
            for m in members
        )
        out.append(f"{fam:<{w}}  {cells}")
    out.append("")
    singles = sorted(f for f, m in fams.items() if len(m) == 1)
    out.append(f"one of a kind: {', '.join(singles)}")
    return "```\n" + "\n".join(out) + "\n```"


def cost_chart(width=30):
    """What a point of Fatigue buys, for the routines that deal damage."""
    dmg = sorted((r for r in R if r["effect"] == "Damage"), key=lambda r: -r["power"] / max(r["cost"], 1))
    out = ["DAMAGE PER POINT OF FATIGUE", ""]
    ratios = [r["power"] / r["cost"] for r in dmg if r["cost"]]
    hi = max(ratios)
    for r in dmg:
        if not r["cost"]:
            continue
        ratio = r["power"] / r["cost"]
        n = round(ratio / hi * width)
        out.append(
            f'{r["name"]:<26} {r["power"]:>2} / {r["cost"]:<4.0f} {"#" * n}{"." * (width - n)} {ratio:.2f}'
        )
    return "```\n" + "\n".join(out) + "\n```"


doc = f"""# Ability catalogue

Every shipped ability in feral-processes, charted from its own file in
`assets/abilities/`. Forty-one of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/abilities/*.ron` on 2026-08-05 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

A species grants abilities by naming their ids with a level to unlock each at;
`priority_boost` must exist, because it is the fallback for a companion whose
species grants nothing. The [research tree](research.md) teaches the rest.

| | |
|---|---|
| abilities | {len(R)} |
| effect shapes | {len({r["effect"] for r in R})} |
| target shapes | {len({r["target"] for r in R})} |
| field routines (run outside battle) | {len(FIELD)} |
| cost nothing | {len(FREE)} |

## The naming scheme

An ability's **id** is flavour and its **name** is a spec. `kernel_panic`,
`cascade_overflow` and `broadcast_storm` sound like three unrelated things;
their names say Packet Shred Single, Packet Shred Group v1.0 and Packet Shred
Everyone, which is one effect at three scopes. A player reading a menu is
being told what the routine does and how wide it reaches, every time, in the
same word order.

{families()}

The number in brackets is the effect's power. Read across a row and the
scaling rule is visible: reaching wider costs magnitude, and a `v2.0` at the
same scope is the straight upgrade. Nothing in the game names a routine after
what it is *called* rather than what it *does* — which is why the id column
exists at all, and why renaming an id never changes what a player reads.

## Who it hits against what it does

{matrix()}

The grid is sparse on purpose. Heals and buffs point at allies, damage and
debuffs point at enemies. The one crossing is `Buff` aimed at an enemy group —
Etch and Throttle are buffs with **negative** power, so a sap is not a separate
effect shape but the same one run backwards. `Decompile` and `Cleanse` are one
of a kind apiece: taming is an ability rather than a separate verb, and
cleansing is the only routine that removes rather than adds.

## Everything

{table(["Ability", "Name", "Target", "Effect", "Pow", "Dur", "Rider", "CD", "Cost"],
       [[f'`{r["id"]}`', r["name"], r["target"], r["effect"] + (f' {r["sub"]}' if r["sub"] else ""),
         r["power"], r["dur"] or "-", r["status"] or "-", r["cd"] or "-",
         f'{r["cost"]:.0f}' if r["cost"] else "-"]
        for r in sorted(R, key=lambda r: (EFFECTS.index(r["effect"]), -r["power"]))],
       ["l", "l", "l", "l", "r", "r", "l", "r", "r"])}

## What a hit costs

{cost_chart()}

Read this one carefully, because it measures power per point and **not** total
damage dealt: a routine at the top of the chart that reaches one program is
worth far less per cast than one halfway down that reaches five. Packet Shred
Everyone leads on both counts at once, which is exactly why it is a boss
routine and not something a player is ever taught.

Within a family the rate is the honest signal. Packet Shred runs from 0.75 at
Group v1.0 to 1.67 at Everyone, so the tiers are not merely wider — they are
better per point as well, and the thing holding them back is what it takes to
learn them rather than what they cost to cast.

Note the {len(FREE)} routines that cost **nothing** — {", ".join(f'`{r["id"]}`' for r in FREE)}.
Those are the starters, the ones a species grants at level 1 and the fallback
every companion has. A free routine is deliberately the weakest tier of its
family, so the opening move of a fight is always available and never the best
one.

## Field routines

These {len(FIELD)} do not run in battle at all. They are written onto Routine Disks
and cost **Power** rather than Fatigue, and their durations are measured in
turns of walking around rather than rounds of combat.

{table(["Routine", "Effect", "Power", "Duration", "Costs"],
       [[r["name"], r["sub"], r["power"], f'{r["dur"]} turns', f'{r["cost"]:.0f}']
        for r in sorted(FIELD, key=lambda r: -r["dur"])],
       ["l", "l", "r", "r", "r"])}

{len([r for r in FIELD if r["sub"] in ("CaptureBoost", "XpBoost", "DropBoost", "EncounterDamp")])} of them are not buffs in any combat sense — CaptureBoost, XpBoost,
DropBoost and EncounterDamp change the odds of a whole run rather than the
outcome of a fight, which is what Deep Analysis is buying at the far end of
the research tree. The other five are ordinary stat and regeneration work,
just measured in turns.

Installing one is the one place a known routine meets an item, and the item is
spent **last**: the game checks battle, ownership, knowledge and a free slot
before it looks for the disk. Uninstalling returns nothing, which is the whole
point — a slot is a commitment.

---

Source of truth is `assets/abilities/`. A mod that drops a `.ron` file in that
directory becomes grantable without a recompile, and will not appear above
until this page is regenerated -- edit the table at the top of
[`docs/abilities-gen.py`](abilities-gen.py) and run
`python3 docs/abilities-gen.py` from the repo root. The schema is documented
in [`assets/abilities/README.md`](../assets/abilities/README.md).
"""

import pathlib
pathlib.Path("docs/abilities.md").write_text(doc)
print(doc)
