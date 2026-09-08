# Regenerates docs/abilities.md. Run from the repo root:
#     python3 docs/abilities-gen.py
#
# Transcribed from assets/abilities/*.ron by hand rather than parsed from it,
# for the same reason docs/roster-gen.py is. Update the table when an ability
# moves, then rerun.
#
# `cost` is the file's `power_cost`, which is what the engine charges
# whatever the routine is and wherever it runs -- `Game::spend_power`, priced
# through `abilities::routine_power_cost`, off the invoker's own reserve.
# This column said 0 for every battle routine until 2026-09-07, describing a
# 2026-08-08 state of the world that the 2026-08-17 rename out of
# `fatigue_cost` had already ended: a Special charges Power *and* arms a
# cooldown, and 50 of these rows were carrying an inert 0 that read as "free".
# The passives really do show 0 -- they fire on a trigger rather than being
# run, and `Game::fire_passives` charges nothing. `status` is the rider on a
# Damage effect, flattened to "kind chance duration".
#
# Before trusting a regenerated page, run `python3 docs/audit-gen.py` — it
# diffs this table against the source it claims to transcribe. Nothing else
# can see a table that has gone stale.
A = [
 # id                   name                       target                effect       sub              pow  spr  dur  status             cd  cost
 ("ablative_layer",    "Ablative Layer Single",   "OneAlly",            "FieldBuff", "Mitigation",     10,   0,    0, "",                 0, 20),
 ("acid_wash",         "Etch Single",             "OneEnemyGroupFront", "Buff",      "Mitigation",    -15,   0,    3, "",                 2, 8),
 ("bastion",           "Bastion Party",           "WholeParty",         "Buff",      "Mitigation",     12,   0,    3, "",                 3, 11),
 ("bastion_shield_v2", "Bastion Single v2.0",     "OneAlly",            "Buff",      "Mitigation",     15,   0,    3, "",                 2, 7),
 ("bastion_shield_v3", "Bastion Single v3.0",     "OneAlly",            "Buff",      "Mitigation",     20,   0,    4, "",                 2, 9),
 ("bit_rot",           "Bit Rot Everyone",        "AllEnemies",         "Debuff",    "Bleed",           2,   0,    4, "",                 5, 16),
 ("bit_rot_v2",        "Bit Rot Single v2.0",     "OneEnemyGroupFront", "Debuff",    "Bleed",           4,   0,    3, "",                 2, 7),
 ("bit_rot_v3",        "Bit Rot Single v3.0",     "OneEnemyGroupFront", "Debuff",    "Bleed",           6,   0,    4, "",                 3, 9),
 ("branch_hazard",     "Pipeline Stall Group",    "WholeEnemyGroup",    "Damage",    "",                6,   2,    0, "Stun 30% 1r",      4, 13),
 ("broadcast_storm",   "Packet Shred Everyone",   "AllEnemies",         "Damage",    "",               25,   6,    0, "",                 4, 15),
 ("brownout",          "Throttle Everyone",       "AllEnemies",         "Buff",      "Atk",            -3,   0,    3, "",                 5, 16),
 ("buffer_overrun",    "Buffer Overrun Party",    "WholeParty",         "Phase",     "",                0,   0,    0, "",                 0, 12),
 ("bus_fault",         "Pipeline Stall Everyone", "AllEnemies",         "Damage",    "",                6,   2,    0, "Stun 25% 1r",      5, 18),
 ("bus_snoop",         "Snoop Everyone",          "AllEnemies",         "Drain",     "",                9,   2,    0, "",                 5, 20),
 ("cascade_overflow",  "Packet Shred Group v1.0", "WholeEnemyGroup",    "Damage",    "",                6,   2,    0, "",                 2, 8),
 ("checksum_repair",   "Patch Single v2.0",       "OneAlly",            "Heal",      "",               25,   6,    0, "",                 3, 9),
 ("clock_gate",        "Throttle Single",         "OneEnemyGroupFront", "Buff",      "Atk",            -5,   0,    3, "",                 2, 8),
 ("clock_skew",       "Clock Skew Single",       "OneEnemyGroupFront", "Debuff",    "Bleed",           2,   0,    2, "",                 4, 0),
 ("cold_boot",         "Patch Single v3.0",       "OneAlly",            "Heal",      "",               50,  12,    0, "",                 5, 15),
 ("core_dump",        "Core Dump Single",        "OneEnemyGroupFront", "Damage",    "",                9,   2,    0, "",                 3, 0),
 ("cycle_harvest",     "Leech Everyone",          "AllEnemies",         "Drain",     "",                4,   1,    0, "",                 5, 17),
 ("deadlock",          "Hard Lock Single v1.0",   "OneEnemyGroupFront", "Debuff",    "Stun",            0,   0,    1, "",                 2, 6),
 ("deadman",          "Deadman Everyone",        "AllEnemies",         "Damage",    "",               14,   4,    0, "",                 4, 0),
 ("decompile",         "Decompile Single",        "OneEnemyGroupFront", "Decompile", "",                0,   0,    0, "",                 0, 1),
 ("deep_scan",         "Deep Scan Party",         "WholeParty",         "FieldBuff", "CaptureBoost",   20,   0,    0, "",                 0, 18),
 ("etch",              "Etch Group",              "WholeEnemyGroup",    "Buff",      "Mitigation",    -12,   0,    3, "",                 3, 10),
 ("flush_cache",       "Flush Cache Party",       "WholeParty",         "Cleanse",   "",                0,   0,    0, "",                 3, 7),
 ("fork_bomb",         "Fork Bomb Group",         "WholeEnemyGroup",    "Damage",    "",               15,   4,    0, "Bleed 35% 2r",     3, 12),
 ("hard_fault",       "Hard Fault Everyone",     "AllEnemies",         "Debuff",    "Stun",            0,   0,    2, "",                 5, 20),
 ("hard_lock",         "Hard Lock Single v2.0",   "OneEnemyGroupFront", "Debuff",    "Stun",            0,   0,    2, "",                 4, 10),
 ("hardened_shell",    "Hardened Shell Single",   "OneAlly",            "FieldBuff", "Mitigation",     12,   0,    0, "",                 0, 14),
 ("hardened_shell_party", "Hardened Shell Party", "WholeParty",         "FieldBuff", "Mitigation",     12,   0,    0, "",                 0, 32),
 ("heap_corruption",   "Bit Rot Group",           "WholeEnemyGroup",    "Debuff",    "Bleed",           3,   0,    3, "",                 3, 11),
 ("hot_patch",         "Patch Single v1.0",       "OneAlly",            "Heal",      "",                8,   2,    0, "",                 1, 5),
 ("hot_spare",        "Hot Spare Single",        "OneAlly",            "Heal",      "",                8,   2,    0, "",                 3, 0),
 ("hyperthread",       "Hyperthread Single v2.0", "OneAlly",            "Buff",      "Atk",             6,   0,    4, "",                 3, 8),
 ("interrupt_request","Interrupt Single",        "OneEnemyGroupFront", "Damage",    "",                5,   1,    0, "",                 4, 0),
 ("invalidate_line",   "Flush Cache Single",      "OneAlly",            "Cleanse",   "",                0,   0,    0, "",                 2, 4),
 ("kernel_panic",      "Packet Shred Single",     "OneEnemyGroupFront", "Damage",    "",               16,   4,    0, "",                 3, 10),
 ("kernel_shear",     "Kernel Shear Group",      "WholeEnemyGroup",    "Damage",    "",               22,   6,    0, "Bleed 75% 4r",     4, 16),
 ("leech_array",       "Leech Group",             "WholeEnemyGroup",    "Drain",     "",                6,   2,    0, "",                 4, 13),
 ("long_winter",      "Long Winter Party",       "WholeParty",         "FieldBuff", "Mitigation",     25,   0,    0, "",                 0, 40),
 ("memory_leak",       "Bit Rot Single v1.0",     "OneEnemyGroupFront", "Debuff",    "Bleed",           2,   0,    3, "",                 1, 5),
 ("mirror_restore",    "Patch Party v1.0",        "WholeParty",         "Heal",      "",                8,   2,    0, "",                 2, 10),
 ("null_cache",       "Null Cache Group",        "WholeEnemyGroup",    "Drain",     "",               12,   3,    0, "",                 3, 18),
 ("null_route",        "Hard Lock Everyone",      "AllEnemies",         "Debuff",    "Stun",            0,   0,    1, "",                 5, 15),
 ("overclock",         "Overclock Single",        "OneAlly",            "FieldBuff", "Atk",             4,   0,    0, "",                 0, 14),
 ("overclock_array",   "Hyperthread Party",       "WholeParty",         "Buff",      "Atk",             3,   0,    3, "",                 3, 10),
 ("oxide_strip",       "Etch Everyone",           "AllEnemies",         "Buff",      "Mitigation",     -9,   0,    3, "",                 5, 16),
 ("packet_shred",      "Packet Shred Group v2.0", "WholeEnemyGroup",    "Damage",    "",               10,   2,    0, "",                 3, 11),
 ("parity_guard",     "Parity Single",           "OneAlly",            "Buff",      "Mitigation",      9,   0,    3, "",                 4, 0),
 ("pid_exhaustion",    "Fork Bomb Everyone",      "AllEnemies",         "Damage",    "",                8,   2,    0, "Bleed 20% 2r",     5, 18),
 ("pipeline_stall",    "Pipeline Stall Single",   "OneEnemyGroupFront", "Damage",    "",                7,   2,    0, "Stun 40% 1r",      3, 9),
 ("priority_boost",    "Hyperthread Single v1.0", "OneAlly",            "Buff",      "Atk",             3,   0,    3, "",                 1, 5),
 ("quarantine",       "Quarantine Single",       "OneAlly",            "Cleanse",   "",                0,   0,    0, "",                 4, 0),
 ("race_condition",    "Hard Lock Group",         "WholeEnemyGroup",    "Debuff",    "Stun",            0,   0,    1, "",                 4, 13),
 ("redundancy_sync",   "Patch Party v1.1",        "WholeParty",         "Heal",      "",               10,   2,    0, "",                 3, 12),
 ("repair_loop",       "Repair Loop Single",      "OneAlly",            "FieldBuff", "Regen",           2,   0,  300, "",                 0, 18),
 ("rollback_v1",       "Rollback Single v1.0",    "OneAlly",            "Heal",      "",               10,   2,    0, "",                 2, 6),
 ("rollback_v2",       "Rollback Single v2.0",    "OneAlly",            "Heal",      "",               20,   5,    0, "",                 3, 8),
 ("rollback_v3",       "Rollback Single v3.0",    "OneAlly",            "Heal",      "",               35,   9,    0, "",                 4, 10),
 ("row_hammer_everyone","Row Hammer Everyone",    "AllEnemies",         "Damage",    "",                5,   1,    0, "",                 4, 15),
 ("row_hammer_group",  "Row Hammer Group",        "WholeEnemyGroup",    "Damage",    "",                6,   1,    0, "",                 3, 10),
 ("row_hammer_single", "Row Hammer Single",       "OneEnemyGroupFront", "Damage",    "",                7,   1,    0, "",                 2, 7),
 ("salvage_routine",   "Salvage Routine Party",   "WholeParty",         "FieldBuff", "DropBoost",      20,   0,    0, "",                 0, 18),
 ("sandbox",           "Bastion Single v1.0",     "OneAlly",            "Buff",      "Mitigation",      9,   0,    3, "",                 1, 5),
 ("segfault_everyone", "Segfault Everyone",       "AllEnemies",         "Damage",    "",               28,  12,    0, "",                 5, 18),
 ("segfault_group",    "Segfault Group",          "WholeEnemyGroup",    "Damage",    "",               14,   8,    0, "",                 3, 11),
 ("segfault_v1",       "Segfault Single v1.0",    "OneEnemyGroupFront", "Damage",    "",                6,   2,    0, "",                 2, 6),
 ("segfault_v2",       "Segfault Single v2.0",    "OneEnemyGroupFront", "Damage",    "",               11,   3,    0, "",                 3, 8),
 ("segfault_v3",       "Segfault Single v3.0",    "OneEnemyGroupFront", "Damage",    "",               17,   4,    0, "",                 4, 10),
 ("siphon_cycles",     "Leech Single",            "OneEnemyGroupFront", "Drain",     "",               10,   2,    0, "",                 2, 9),
 ("skim_everyone",     "Skim Everyone",           "AllEnemies",         "Drain",     "",                3,   1,    0, "",                 4, 15),
 ("skim_group",        "Skim Group",              "WholeEnemyGroup",    "Drain",     "",                4,   1,    0, "",                 3, 8),
 ("skim_v1",           "Skim Single v1.0",        "OneEnemyGroupFront", "Drain",     "",                5,   1,    0, "",                 2, 6),
 ("skim_v2",           "Skim Single v2.0",        "OneEnemyGroupFront", "Drain",     "",                9,   2,    0, "",                 3, 8),
 ("skim_v3",           "Skim Single v3.0",        "OneEnemyGroupFront", "Drain",     "",               14,   4,    0, "",                 4, 10),
 ("stack_smash",       "Fork Bomb Single",        "OneEnemyGroupFront", "Damage",    "",                9,   2,    0, "Bleed 60% 3r",     2, 8),
 ("stealth_protocol",  "Stealth Protocol Party",  "WholeParty",         "FieldBuff", "EncounterDamp",  20,   0,    0, "",                 0, 18),
 ("symlink",           "Symlink Party",           "WholeParty",         "Symlink",   "",                0,   0,    0, "",                 0, 25),
 ("throttle",          "Throttle Group",          "WholeEnemyGroup",    "Buff",      "Atk",            -4,   0,    3, "",                 3, 10),
 ("trace_analysis",    "Trace Analysis Party",    "WholeParty",         "FieldBuff", "XpBoost",        20,   0,    0, "",                 0, 18),
 ("trickle_charge",    "Trickle Charge Party",    "WholeParty",         "FieldBuff", "Trickle",         1,   0,   60, "",                 0, 25),
 ("watchdog",         "Watchdog Party",          "WholeParty",         "Cleanse",   "",                0,   0,    0, "",                 4, 0),
 ("wild_jump",         "Wild Jump Party",         "WholeParty",         "Jump",      "",                0,   0,    0, "",                 0, 20),
]
K = "id name target effect sub power spread dur status cd cost".split()
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
           "Phase", "Jump", "Symlink"]
FIELD = [r for r in R if r["effect"] == "FieldBuff"]
MOVE = [r for r in R if r["effect"] in ("Phase", "Jump")]


def band(r):
    """`power` is the centre of a band and `spread` its half-width, so the
    table shows what the routine actually rolls. A spread of 0 is a
    degenerate band and prints as the single number it is -- which is every
    effect that moves no Integrity."""
    if not r["spread"]:
        return r["power"]
    return f'{max(0, r["power"] - r["spread"])}\u2013{r["power"] + r["spread"]}'


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

    One of a routine's two prices, not the whole of it: the other is the
    Power in the PWR column, charged to the invoker's own reserve. This
    chart holds the cooldown axis on its own because that is the one a
    player is spending *inside* a fight, where the reserve is fixed and
    only rest refills it.
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
`assets/abilities/*.ron` on 2026-08-25 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

A species grants abilities by naming their ids with a level to unlock each at;
`priority_boost` must exist, because it is the fallback for a companion whose
species grants nothing. The [research tree](research.md) teaches the rest.

| | |
|---|---|
| abilities | {len(R)} |
| effect shapes | {len({r["effect"] for r in R})} |
| target shapes | {len({r["target"] for r in R})} |
| routines that never run in battle | {len(FIELD) + len(MOVE)} |
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

{table(["Ability", "Name", "Target", "Effect", "Pow", "Dur", "Rider", "CD", "PWR"],
       [[f'`{r["id"]}`', r["name"], r["target"], r["effect"] + (f' {r["sub"]}' if r["sub"] else ""),
         band(r), r["dur"] or "-", r["status"] or "-", r["cd"] or "-", r["cost"] or "-"]
        for r in sorted(R, key=lambda r: (EFFECTS.index(r["effect"]), -r["power"]))],
       ["l", "l", "l", "l", "r", "r", "l", "r", "r"])}

A routine costs two things at once, and both columns above are real. **CD** is
rounds before the same combatant can run it again, cleared when the battle
ends. **PWR** comes off the invoker's own reserve — a companion's Special
spends the companion's Power, not yours — and only rest refills one. The
picker greys a row that fails either test, with the reason on it.

The exceptions are worth knowing because they are the whole of the pattern.
A **passive** shows no PWR: it fires on a trigger rather than being run, and
`cooldown` is its entire price. A **hostile** carrier is never charged, since
nothing on the wild side holds a reserve at all. And the wielded program's
proc is free by design — its 25% rate is what it pays instead.

## What a hit costs

{cost_chart()}

Read this one carefully, because it measures power per round and **not** total
damage dealt: a routine at the top of the chart that reaches one program is
worth far less per run than one halfway down that reaches five. Packet Shred
Everyone leads on both counts at once, which is why no research node teaches
it and no wild carrier rolls it — it is the two bosses' innate, and the only
way to meet it is to be on the wrong end of it.

Segfault Everyone is the one directly behind it on both axes and *is*
learnable, which is what the family is for: 16-40 against Packet Shred
Everyone's 19-31, bought with a fifth round of cooldown, three more Power and
all but one point of aim. A better ceiling and a worse floor at the same
reach — the widening band is Segfault's whole identity, and it is what keeps
the family from being a second copy of a ladder that already exists.

Within a family the rate is where reaching wider gets paid for, and it falls
as the scope grows: Pipeline Stall runs 2.33, 1.50, 1.20 across its three
tiers, and Fork Bomb drops from 5.00 at Group to 1.60 at Everyone. You buy
reach with efficiency. Two families don't, and they buy their way out of it
differently. Packet Shred rises from 3.00 at Group v1.0 to 6.25 at Everyone —
better per round as well as wider — and what holds those tiers back is what it
takes to learn them rather than what they cost to run, the top one being a
boss's innate and nothing else's. Segfault climbs the same way, 3.00 to 5.60
across five tiers, and is learnable at every rung; what it pays instead is the
band. Its floor gets worse as its ceiling gets better, so the rate this chart
measures is a mean the routine will often miss on the low side.

A routine is bought twice over: once in the rounds it spends locked away,
and once out of the reserve of whoever ran it. Until 2026-08-08 the second
half came off *the player's* meter even when a companion was the one acting,
which rationed the party's own kit against a pool only the player had; it
comes off the invoker now, which is what makes a levelled companion's Special
its own to spend. What marks out the first thing a species grants is the
bottom of the cooldown ladder: the routines that
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
and cost **Power**. Most of them have no duration at all: they run until the
party rests, so they are bought at base as a loadout for a trip rather than
timed against a fight. The two that restore a pool over time keep a turn
count, because an unbounded one is unbounded healing or unbounded Power.

They are no longer the only things the map's routine list offers. A **Heal**
that charges Power runs out there too, on top of being a Special — all eight
Patch and Rollback routines, `hot_patch` included since it was priced. A heal
is priced in Power and nothing else out there, because a cooldown counts
battle rounds and the map has no round to count; a heal costing nothing would
therefore have no throttle at all, and would stay a Special. Everything else
about it is the battle invocation's: the same band, scaled by the invoker's
own level and Heal affinity, restoring what fits under the target's ceiling.

{table(["Routine", "Effect", "Power", "Duration", "Costs"],
       [[r["name"], r["sub"], r["power"],
         f'{r["dur"]} turns' if r["dur"] else "until rest", f'{r["cost"]:.0f}']
        for r in sorted(FIELD, key=lambda r: (-r["dur"], r["name"]))],
       ["l", "l", "r", "r", "r"])}

{len([r for r in FIELD if r["sub"] in ("CaptureBoost", "XpBoost", "DropBoost", "EncounterDamp")])} of them are not buffs in any combat sense — CaptureBoost, XpBoost,
DropBoost and EncounterDamp change the odds of a whole run rather than the
outcome of a fight, which is what Deep Analysis is buying at the far end of
the research tree. The other {len(FIELD) - len([r for r in FIELD if r["sub"] in ("CaptureBoost", "XpBoost", "DropBoost", "EncounterDamp")])} are ordinary stat and regeneration work.

Getting one into a slot is where a known routine meets an item, and it takes
two steps. **Etching** burns a blank Routine Disk with a routine you know and
produces an etched disk; **installing** spends that etched disk on a slot.
Both spend last, after every refusal has cleared — there is no way to lose a
disk to a failed attempt. Uninstalling returns nothing, which is the whole
point: a slot is a commitment.

That split is also what makes the exclusive pool possible. An **exclusive**
routine is one nobody can learn and therefore nobody can etch — its disk
only ever arrives already written, off a boss's drop table or a Stack
trader's rare shelf row. Seven ship: Kernel Shear, Null Cache and Deadman off
Wintermute; Hard Fault, Long Winter, Watchdog and Snoop off the Overseer. The
two bosses' tables roll independently, so the Overseer holding a fourth costs
the other three nothing. Long Winter is the field routine among them, which is
why it sits at the top of the table above with a Power cost nothing else comes
near.

**Eight of them are passives.** They occupy a slot, appear in no menu, and
fire on an event instead of a turn. Their cooldowns really are their whole
price — the PWR column reads 0 because `Game::fire_passives` charges nothing,
and a passive is the only kind of routine that is genuinely free to run.

| Passive | Fires on | And then |
|:---|:---|:---|
| Clock Skew Single | the round opening | the nearest hostile starts bleeding |
| Interrupt Single | the round opening | the nearest hostile takes a small hit |
| Parity Single | the round opening | the wearer's own mitigation goes up |
| Core Dump Single | its holder driven low | the nearest hostile takes a large hit |
| Hot Spare Single | its holder driven low | the holder patches itself |
| Deadman Everyone | one of yours going down | everything hostile takes the fallout |
| Quarantine Single | a condition landing | the wearer sheds it |
| Watchdog Party | a condition landing | the whole party is cleared |

Read the table by trigger rather than by effect. `RoundStart` fires every
round there is, which is why all three of those are priced slow as well as
low. The two `AllyWounded` rungs are the crossing worth noticing: `core_dump`
answers the crisis by hitting back and `hot_spare` by patching, and the heal
is the smaller number on purpose — a heal on the way down buys the round the
crisis is supposed to be survivable in. Neither is `AllyDropped`, which only
`deadman` uses: a dropped companion is gone for good at every difficulty, so
a routine paying out there pays a player who has already lost more than the
payout is worth.

## Movement routines

The other {len(MOVE)} run outside battle too. They were the last routines still
priced in the retired Fatigue meter, and are denominated in the same Power as
everything else now — which is the only reason their numbers can be compared
with the tables above at all. Both are Stack-only: they read and write the party's frame coordinates, so they grey
out with a reason on the open grid.

{table(["Routine", "Effect", "Power", "What it does"],
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
