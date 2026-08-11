# Regenerates docs/abilities.md. Run from the repo root:
#     python3 docs/abilities-gen.py
#
# Transcribed from assets/abilities/*.ron by hand rather than parsed from it,
# for the same reason docs/roster-gen.py is. Update the table when an ability
# moves, then rerun.
#
# `cost` is what the engine actually charges: power_cost for a FieldBuff,
# fatigue_cost for the two movement routines, and nothing at all for a battle
# routine, which is priced in its cooldown alone. Most battle files still
# carry a fatigue_cost the engine stopped reading on 2026-08-08 -- it is left
# out of this table deliberately, because transcribing an inert number would
# put it in front of a reader as though it meant something. `status` is the
# rider on a Damage effect, flattened to "kind chance duration".
A = [
 # id                   name                       target                effect       sub              pow  dur  status             cd  cost
 ("ablative_layer",    "Ablative Layer Single",   "OneAlly",            "FieldBuff", "Mitigation",     10,   80, "",                 0, 20.0),
 ("acid_wash",         "Etch Single",             "OneEnemyGroupFront", "Buff",      "Def",            -5,    3, "",                 2, 0),
 ("bastion",           "Bastion Party",           "WholeParty",         "Buff",      "Def",             4,    3, "",                 3, 0),
 ("bastion_shield_v2", "Bastion Single v2.0",     "OneAlly",            "Buff",      "Def",             5,    3, "",                 2, 0),
 ("bastion_shield_v3", "Bastion Single v3.0",     "OneAlly",            "Buff",      "Def",             7,    4, "",                 2, 0),
 ("bit_rot",           "Bit Rot Everyone",        "AllEnemies",         "Debuff",    "Bleed",           2,    4, "",                 5, 0),
 ("bit_rot_v2",        "Bit Rot Single v2.0",     "OneEnemyGroupFront", "Debuff",    "Bleed",           4,    3, "",                 2, 0),
 ("bit_rot_v3",        "Bit Rot Single v3.0",     "OneEnemyGroupFront", "Debuff",    "Bleed",           6,    4, "",                 3, 0),
 ("branch_hazard",     "Pipeline Stall Group",    "WholeEnemyGroup",    "Damage",    "",                6,    0, "Stun 30% 1r",      4, 0),
 ("broadcast_storm",   "Packet Shred Everyone",   "AllEnemies",         "Damage",    "",               25,    0, "",                 4, 0),
 ("brownout",          "Throttle Everyone",       "AllEnemies",         "Buff",      "Atk",            -3,    3, "",                 5, 0),
 ("buffer_overrun",    "Buffer Overrun Party",    "WholeParty",         "Phase",     "",                0,    0, "",                 0, 12.0),
 ("bus_fault",         "Pipeline Stall Everyone", "AllEnemies",         "Damage",    "",                6,    0, "Stun 25% 1r",      5, 0),
 ("cascade_overflow",  "Packet Shred Group v1.0", "WholeEnemyGroup",    "Damage",    "",                6,    0, "",                 2, 0),
 ("checksum_repair",   "Patch Single v2.0",       "OneAlly",            "Heal",      "",               25,    0, "",                 3, 0),
 ("clock_gate",        "Throttle Single",         "OneEnemyGroupFront", "Buff",      "Atk",            -5,    3, "",                 2, 0),
 ("cold_boot",         "Patch Single v3.0",       "OneAlly",            "Heal",      "",               50,    0, "",                 5, 0),
 ("coolant_flush",     "Coolant Flush Party",     "WholeParty",         "FieldBuff", "Coolant",         1,   90, "",                 0, 15.0),
 ("cycle_harvest",     "Leech Everyone",          "AllEnemies",         "Drain",     "",                4,    0, "",                 5, 0),
 ("deadlock",          "Hard Lock Single v1.0",   "OneEnemyGroupFront", "Debuff",    "Stun",            0,    1, "",                 2, 0),
 ("decompile",         "Decompile Single",        "OneEnemyGroupFront", "Decompile", "",                0,    0, "",                 0, 0),
 ("deep_scan",         "Deep Scan Party",         "WholeParty",         "FieldBuff", "CaptureBoost",   20,  100, "",                 0, 18.0),
 ("etch",              "Etch Group",              "WholeEnemyGroup",    "Buff",      "Def",            -4,    3, "",                 3, 0),
 ("flush_cache",       "Flush Cache Party",       "WholeParty",         "Cleanse",   "",                0,    0, "",                 3, 0),
 ("fork_bomb",         "Fork Bomb Group",         "WholeEnemyGroup",    "Damage",    "",               15,    0, "Bleed 35% 2r",     3, 0),
 ("hard_lock",         "Hard Lock Single v2.0",   "OneEnemyGroupFront", "Debuff",    "Stun",            0,    2, "",                 4, 0),
 ("hardened_shell",    "Hardened Shell Single",   "OneAlly",            "FieldBuff", "Def",             4,   90, "",                 0, 14.0),
 ("heap_corruption",   "Bit Rot Group",           "WholeEnemyGroup",    "Debuff",    "Bleed",           3,    3, "",                 3, 0),
 ("hot_patch",         "Patch Single v1.0",       "OneAlly",            "Heal",      "",                8,    0, "",                 1, 0),
 ("hyperthread",       "Hyperthread Single v2.0", "OneAlly",            "Buff",      "Atk",             6,    4, "",                 3, 0),
 ("invalidate_line",   "Flush Cache Single",      "OneAlly",            "Cleanse",   "",                0,    0, "",                 2, 0),
 ("kernel_panic",      "Packet Shred Single",     "OneEnemyGroupFront", "Damage",    "",               16,    0, "",                 3, 0),
 ("leech_array",       "Leech Group",             "WholeEnemyGroup",    "Drain",     "",                6,    0, "",                 4, 0),
 ("memory_leak",       "Bit Rot Single v1.0",     "OneEnemyGroupFront", "Debuff",    "Bleed",           2,    3, "",                 1, 0),
 ("mirror_restore",    "Patch Party v1.0",        "WholeParty",         "Heal",      "",                8,    0, "",                 2, 0),
 ("null_route",        "Hard Lock Everyone",      "AllEnemies",         "Debuff",    "Stun",            0,    1, "",                 5, 0),
 ("overclock",         "Overclock Single",        "OneAlly",            "FieldBuff", "Atk",             4,   90, "",                 0, 14.0),
 ("overclock_array",   "Hyperthread Party",       "WholeParty",         "Buff",      "Atk",             3,    3, "",                 3, 0),
 ("oxide_strip",       "Etch Everyone",           "AllEnemies",         "Buff",      "Def",            -3,    3, "",                 5, 0),
 ("packet_shred",      "Packet Shred Group v2.0", "WholeEnemyGroup",    "Damage",    "",               10,    0, "",                 3, 0),
 ("pid_exhaustion",    "Fork Bomb Everyone",      "AllEnemies",         "Damage",    "",                8,    0, "Bleed 20% 2r",     5, 0),
 ("pipeline_stall",    "Pipeline Stall Single",   "OneEnemyGroupFront", "Damage",    "",                7,    0, "Stun 40% 1r",      3, 0),
 ("priority_boost",    "Hyperthread Single v1.0", "OneAlly",            "Buff",      "Atk",             3,    3, "",                 1, 0),
 ("race_condition",    "Hard Lock Group",         "WholeEnemyGroup",    "Debuff",    "Stun",            0,    1, "",                 4, 0),
 ("redundancy_sync",   "Patch Party v1.1",        "WholeParty",         "Heal",      "",               10,    0, "",                 3, 0),
 ("repair_loop",       "Repair Loop Single",      "OneAlly",            "FieldBuff", "Regen",           2,  100, "",                 0, 18.0),
 ("rollback_v1",       "Rollback Single v1.0",    "OneAlly",            "Heal",      "",               10,    0, "",                 2, 0),
 ("rollback_v2",       "Rollback Single v2.0",    "OneAlly",            "Heal",      "",               20,    0, "",                 3, 0),
 ("rollback_v3",       "Rollback Single v3.0",    "OneAlly",            "Heal",      "",               35,    0, "",                 4, 0),
 ("salvage_routine",   "Salvage Routine Party",   "WholeParty",         "FieldBuff", "DropBoost",      20,  100, "",                 0, 18.0),
 ("sandbox",           "Bastion Single v1.0",     "OneAlly",            "Buff",      "Def",             3,    3, "",                 1, 0),
 ("segfault_v1",       "Segfault Single v1.0",    "OneEnemyGroupFront", "Damage",    "",                6,    0, "",                 2, 0),
 ("segfault_v2",       "Segfault Single v2.0",    "OneEnemyGroupFront", "Damage",    "",               11,    0, "",                 3, 0),
 ("segfault_v3",       "Segfault Single v3.0",    "OneEnemyGroupFront", "Damage",    "",               17,    0, "",                 4, 0),
 ("siphon_cycles",     "Leech Single",            "OneEnemyGroupFront", "Drain",     "",               10,    0, "",                 2, 0),
 ("skim_group",        "Skim Group",              "WholeEnemyGroup",    "Drain",     "",                4,    0, "",                 3, 0),
 ("skim_v1",           "Skim Single v1.0",        "OneEnemyGroupFront", "Drain",     "",                5,    0, "",                 2, 0),
 ("skim_v2",           "Skim Single v2.0",        "OneEnemyGroupFront", "Drain",     "",                9,    0, "",                 3, 0),
 ("skim_v3",           "Skim Single v3.0",        "OneEnemyGroupFront", "Drain",     "",               14,    0, "",                 4, 0),
 ("stack_smash",       "Fork Bomb Single",        "OneEnemyGroupFront", "Damage",    "",                9,    0, "Bleed 60% 3r",     2, 0),
 ("stealth_protocol",  "Stealth Protocol Party",  "WholeParty",         "FieldBuff", "EncounterDamp",  20,   90, "",                 0, 18.0),
 ("throttle",          "Throttle Group",          "WholeEnemyGroup",    "Buff",      "Atk",            -4,    3, "",                 3, 0),
 ("trace_analysis",    "Trace Analysis Party",    "WholeParty",         "FieldBuff", "XpBoost",        20,  100, "",                 0, 18.0),
 ("trickle_charge",    "Trickle Charge Party",    "WholeParty",         "FieldBuff", "Trickle",         1,   80, "",                 0, 20.0),
 ("wild_jump",         "Wild Jump Party",         "WholeParty",         "Jump",      "",                0,    0, "",                 0, 20.0),
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
EFFECTS = ["Damage", "Debuff", "Buff", "Heal", "Drain", "FieldBuff", "Cleanse", "Decompile",
           "Phase", "Jump"]
FIELD = [r for r in R if r["effect"] == "FieldBuff"]
MOVE = [r for r in R if r["effect"] in ("Phase", "Jump")]


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
    """What a round of cooldown buys, for the routines that deal damage.

    A cooldown is the only price a battle routine has, so this is the whole
    of what one costs -- the chart used to divide by fatigue_cost, back when
    a Special was charged to the player's Fatigue as well.
    """
    dmg = sorted((r for r in R if r["effect"] == "Damage"), key=lambda r: -r["power"] / r["cd"])
    out = ["DAMAGE PER ROUND OF COOLDOWN", ""]
    hi = max(r["power"] / r["cd"] for r in dmg)
    for r in dmg:
        ratio = r["power"] / r["cd"]
        n = round(ratio / hi * width)
        out.append(
            f'{r["name"]:<26} {r["power"]:>2} / {r["cd"]:<4} {"#" * n}{"." * (width - n)} {ratio:.2f}'
        )
    return "```\n" + "\n".join(out) + "\n```"


doc = f"""# Ability catalogue

Every shipped ability in feral-processes, charted from its own file in
`assets/abilities/`. {len(R)} of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/abilities/*.ron` on 2026-08-11 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

A species grants abilities by naming their ids with a level to unlock each at;
`priority_boost` must exist, because it is the fallback for a companion whose
species grants nothing. The [research tree](research.md) teaches the rest.

| | |
|---|---|
| abilities | {len(R)} |
| effect shapes | {len({r["effect"] for r in R})} |
| target shapes | {len({r["target"] for r in R})} |
| field routines (run outside battle) | {len(FIELD) + len(MOVE)} |
| of those, Stack-only movement | {len(MOVE)} |

## The naming scheme

An ability's **id** is flavour and its **name** is a spec. `kernel_panic`,
`cascade_overflow` and `broadcast_storm` sound like three unrelated things;
their names say Packet Shred Single, Packet Shred Group v1.0 and Packet Shred
Everyone, which is one effect at three scopes. A player reading a menu is
being told what the routine does and how wide it reaches, every time, in the
same word order.

{families()}

The number in brackets is the effect's power, and a `v2.0` at the same scope
is the straight upgrade over its `v1.0`. Read across a row and reaching wider
usually costs magnitude — Leech runs 10, 6, 4 — but read the whole block and
two families break that on purpose: Packet Shred and Fork Bomb both peak away
from Single, which is what marks them as the prizes of the set rather than
ladders you climb. The honest comparison is the cost chart below, not this
one. Nothing in the game names a routine after what it is *called* rather
than what it *does* — which is why the id column exists at all, and why
renaming an id never changes what a player reads.

## Who it hits against what it does

{matrix()}

The grid is sparse on purpose. Heals and buffs point at allies, damage and
debuffs point at enemies. The one crossing is `Buff` aimed at enemies — Etch
and Throttle are buffs with **negative** power, so a sap is not a separate
effect shape but the same one run backwards. `Decompile` is the one effect
with a single ability to its name, because taming is an ability rather than
a separate verb. `Cleanse` is the one that *removes* rather than adds, which
is why it needs no power column and why it is the only ally-facing effect
with nothing to scale.

## Everything

{table(["Ability", "Name", "Target", "Effect", "Pow", "Dur", "Rider", "CD"],
       [[f'`{r["id"]}`', r["name"], r["target"], r["effect"] + (f' {r["sub"]}' if r["sub"] else ""),
         r["power"], r["dur"] or "-", r["status"] or "-", r["cd"] or "-"]
        for r in sorted(R, key=lambda r: (EFFECTS.index(r["effect"]), -r["power"]))],
       ["l", "l", "l", "l", "r", "r", "l", "r"])}

There is no cost column, because for everything above the CD *is* the cost:
a battle routine charges no need at all, from the player, a companion or a
wild carrier. The routines that do spend something are the field ones, in
their own two tables further down.

## What a hit costs

{cost_chart()}

Read this one carefully, because it measures power per round and **not** total
damage dealt: a routine at the top of the chart that reaches one program is
worth far less per cast than one halfway down that reaches five. Packet Shred
Everyone leads on both counts at once, which is exactly why it is a boss
routine and not something a player is ever taught.

Within a family the rate is where reaching wider gets paid for, and it falls
as the scope grows: Pipeline Stall runs 2.33, 1.50, 1.20 across its three
tiers, and Fork Bomb drops from 5.00 at Group to 1.60 at Everyone. You buy
reach with efficiency. Packet Shred is the one family that doesn't pay,
rising from 3.00 at Group v1.0 to 6.25 at Everyone — better per round as well
as wider — and the thing holding those tiers back is what it takes to learn
them rather than what they cost to cast.

Nothing here is *cheap*, because nothing here is bought. Every one of these
was priced in the player's Fatigue as well until 2026-08-08, including the
ones a companion ran; a routine now costs only the rounds it spends locked
away, so the question a player is answering has changed from "can I afford
this" to "is this the round to spend it". What marks out the first thing a
species grants is the bottom of the cooldown ladder: the routines that
recharge in a single round — `memory_leak`, `priority_boost`, `sandbox`,
`hot_patch` — are the weakest tier of their families, and three of the five
class utilities are one or two rounds behind them. So the opening move of a
fight is always available and never the best one.

**Nothing is granted at level 1**, and that is deliberate rather than an
accident of tuning. `priority_boost` is the fallback a companion falls back
on when its species has taught it nothing *yet*, and it is obtainable no
other way than by extracting it from one — so every species holding its
first entry back to level 2 is what keeps it reachable. It also means a
program you have just tamed reads as generic before it reads as its class.

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
the research tree. The other {len(FIELD) - len([r for r in FIELD if r["sub"] in ("CaptureBoost", "XpBoost", "DropBoost", "EncounterDamp")])} are ordinary stat and regeneration work, just
measured in turns.

Installing one is the one place a known routine meets an item, and the item is
spent **last**: the game checks battle, ownership, knowledge and a free slot
before it looks for the disk. Uninstalling returns nothing, which is the whole
point — a slot is a commitment.

## Movement routines

The other {len(MOVE)} run outside battle too, and are the only routines in the game
that still spend **Fatigue** — every other use of that meter went away when
Specials moved onto cooldowns. Both are Stack-only: they read and write the
party's frame coordinates, so they grey out with a reason on the open grid.

{table(["Routine", "Effect", "Fatigue", "What it does"],
       [[r["name"], r["effect"], f'{r["cost"]:.0f}', d]
        for r, d in zip(sorted(MOVE, key=lambda r: r["cost"]),
                        ["steps the party through one solid cell they are facing",
                         "moves the party to any cell of the frame, and kills them if it is solid"])],
       ["l", "l", "r", "l"])}

Wild Jump is the more expensive of the two because the landing is unvalidated
— that is the whole mechanic, not a missing check. Buffer Overrun refuses and
spends nothing when the rock runs deeper than one cell, when the far side is
off the frame, or when there is nothing solid ahead at all.

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
