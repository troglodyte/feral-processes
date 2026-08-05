# Regenerates docs/achievements.md. Run from the repo root:
#     python3 docs/achievements-gen.py
#
# Transcribed from assets/achievements/*.ron by hand rather than parsed from it,
# for the same reason docs/roster-gen.py is: a RON reader in Python would be a
# second parser to keep in step with the engine's, for a page nobody's build
# depends on. Update the table when a rung moves, then rerun.
#
# The three ceilings are from crates/engine/src/tuning.rs and are code, not
# data -- they are what `the_full_ladder_stays_under_its_ceiling` asserts the
# table below against.
CEIL_STAT, CEIL_PERK, CEIL_PROGRAM = 8, 5, 1

A = [
 # id                name                trigger              arg      reward           n  earned-when
 ("uptime_500",      "Uptime",           "CyclesSurvived",    500,     "RandomMainStat", 1, "the run's clock reaches 500 cycles"),
 ("uptime_2000",     "Long Uptime",      "CyclesSurvived",    2000,    "RandomMainStat", 1, "the run's clock reaches 2000 cycles"),
 ("uptime_5000",     "Persistent Process","CyclesSurvived",   5000,    "PerkPoints",     1, "the run's clock reaches 5000 cycles"),
 ("breach_zone_2",   "First Breach",     "ZoneReached",       2,       "RandomMainStat", 1, "the run has breached to sector 2"),
 ("breach_zone_4",   "Deep Cut",         "ZoneReached",       4,       "PerkPoints",     1, "the run has breached to sector 4"),
 ("breach_zone_6",   "Sector Runner",    "ZoneReached",       6,       "RandomMainStat", 1, "the run has breached to sector 6"),
 ("breach_zone_8",   "Far Sector",       "ZoneReached",       8,       "PerkPoints",     1, "the run has breached to sector 8"),
 ("stack_depth_3",   "Down the Stack",   "StackDepthReached", 3,       "RandomMainStat", 1, "the party stands 3 frames down"),
 ("stack_depth_5",   "Frame Diver",      "StackDepthReached", 5,       "PerkPoints",     1, "the party stands 5 frames down"),
 ("stack_depth_8",   "Bottom Frame",     "StackDepthReached", 8,       "StartingProgram",1, "the party stands 8 frames down"),
 ("boss_first",      "Root Access",      "BossDefeated",      None,    "RandomMainStat", 1, "any boss program dies"),
 ("boss_overseer",   "Chain of Command", "BossDefeated",      "overseer",  "RandomMainStat", 1, "an Overseer dies"),
 ("boss_wintermute", "ZeroDay in the Wire","BossDefeated",      "wintermute","PerkPoints",  1, "a Wintermute dies"),
]
K = "id name trig arg reward n when".split()
R = [dict(zip(K, r)) for r in A]

REWARD_MARK = {"RandomMainStat": "*", "PerkPoints": "+", "StartingProgram": "@"}
PAYLOAD = {"StartingProgram": "scrapper"}

spent = {k: sum(r["n"] for r in R if r["reward"] == k) for k in REWARD_MARK}
spent["StartingProgram"] = sum(1 for r in R if r["reward"] == "StartingProgram")


def trigger_text(r):
    if r["trig"] == "BossDefeated":
        return f'BossDefeated({"None" if r["arg"] is None else chr(34) + r["arg"] + chr(34)})'
    return f'{r["trig"]}({r["arg"]})'


def reward_text(r):
    if r["reward"] == "StartingProgram":
        return f'StartingProgram("{PAYLOAD["StartingProgram"]}")'
    return f'{r["reward"]}({r["n"]})'


def track(title, trig, width=44):
    """A proportional axis for one high-water-mark family."""
    rows = [r for r in R if r["trig"] == trig]
    lo, hi = rows[0]["arg"], rows[-1]["arg"]
    pos = [round((r["arg"] - lo) / (hi - lo) * (width - 1)) for r in rows]
    label, axis = [" "] * (width + 8), [" "] * width
    for p, r in zip(pos, rows):
        axis[p] = REWARD_MARK[r["reward"]]
        s = str(r["arg"])
        start = min(p, width - len(s))
        for i, ch in enumerate(s):
            label[start + i] = ch
    line = "".join("-" if c == " " else c for c in axis)
    return f'{title:<17}{"".join(label).rstrip()}\n{"":<17}{line}'


def budget():
    out = ["REWARD BUDGET                    spent / ceiling", ""]
    for key, ceil, name in (
        ("RandomMainStat", CEIL_STAT, "stat points"),
        ("PerkPoints", CEIL_PERK, "perk points"),
        ("StartingProgram", CEIL_PROGRAM, "free program"),
    ):
        used = spent[key]
        bar = "#" * used + "." * (ceil - used)
        note = "  <- ladder is full" if used == ceil else f"  ({ceil - used} left)"
        out.append(f"{name:<14} {used:>2} / {ceil:<2}  {bar}{note}")
    return "```\n" + "\n".join(out) + "\n```"


def table(header, rows, align):
    sep = "|" + "|".join(("---:" if a == "r" else ":---") for a in align) + "|"
    body = "\n".join("| " + " | ".join(str(c) for c in r) + " |" for r in rows)
    return "| " + " | ".join(header) + " |\n" + sep + "\n" + body


families = [
    ("cycles survived", "CyclesSurvived"),
    ("sector breached", "ZoneReached"),
    ("stack depth", "StackDepthReached"),
]

doc = f"""# Achievement ladder

Every shipped achievement in feral-processes, charted from its own file in
`assets/achievements/`. Thirteen of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/achievements/*.ron` on 2026-08-05 and will drift the moment one of
those files is edited; regenerate the page rather than trusting it blind.

Achievements are the game's only cross-run progression. A rung is earned
inside a run and written to `profile.ron` the moment it fires — then paid at
the **start of the next run**, never mid-run and never on load. That is why a
run you lose still counts for something, and why the ladder is the one place
in the game where a permanent buff accumulates.

| | |
|---|---|
| achievements | {len(R)} |
| trigger shapes used | {len({r["trig"] for r in R})} of 4 |
| reward shapes used | {len({r["reward"] for r in R})} of 3 |
| stat points on offer | {spent["RandomMainStat"]} (ceiling {CEIL_STAT}) |
| perk points on offer | {spent["PerkPoints"]} (ceiling {CEIL_PERK}) |
| free programs | {spent["StartingProgram"]} (ceiling {CEIL_PROGRAM}) |

## The ladder

{table(["Achievement", "id", "Trigger", "Reward"],
       [[r["name"], f'`{r["id"]}`', f'`{trigger_text(r)}`', f'`{reward_text(r)}`'] for r in R],
       ["l", "l", "l", "l"])}

## Progress tracks

Three of the four trigger shapes are high-water marks on a number, so they
read as tracks. Each is drawn on its own scale — the gaps are proportional
within a row, never between rows.

```
{chr(10).join(track(t, k) for t, k in families)}

               * stat point    + perk point    @ free program
```

Every track pays a stat point first and a perk point second, so the cheap end
of each is the one that hands a new run a flat number and the far end is the
one that hands it a choice. The Stack is the only track that ends in
something else: eight frames down pays a tamed Scrapper, which is the single
`StartingProgram` the ceiling allows.

The three tracks are also not equally long in play. Cycles pass on their own —
500 is a slow afternoon and 5000 is only patience. Sector 8 needs seven
breaches, each one earned by fighting for Portal Fragments. Frame 8 of the
Stack is the shortest track on the page and by some distance the hardest,
because depth scales what lives down there and the Trace is counting.

## Bosses

`BossDefeated` is the one trigger that names a thing rather than a number, and
the three rungs stack: killing a Wintermute fires `boss_first` as well, so the
first boss a run brings down is always worth two rungs.

{table(["Achievement", "Trigger", "Pays"],
       [[r["name"], f'`{trigger_text(r)}`', f'`{reward_text(r)}`']
        for r in R if r["trig"] == "BossDefeated"],
       ["l", "l", "l"])}

Fleeing does not count — the kill is recorded at the one point in combat that
knows the boss actually died. There is deliberately no "kill N bosses in one
run" trigger: counting within a run would need saved run state the game does
not keep.

## What the profile may pay

The ladder is bounded in `crates/engine/src/tuning.rs`, not in this directory,
on the same principle that keeps every other difficulty knob out of the
assets: content is moddable, how hard the game is, is not. The three ceilings
are asserted against the real files by
`the_full_ladder_stays_under_its_ceiling`, so a fourteenth rung that overpays
fails the suite rather than quietly inflating every future run.

{budget()}

Two of the three are already spent to the last point. The eighth stat point is
deliberate headroom for a third boss species — add one and the test does not
move. A new rung paying **Perk Points or a free program has nowhere to go**:
the ladder would have to give one up, or someone would have to argue the
ceiling up in `tuning.rs` and say why the profile is worth more than it was.

That asymmetry is the ladder's actual design. `RandomMainStat` is a flat
number rolled once from the achievement's **id** — the same answer on every
machine and after every reload — so it is the cheap rung, and seven of the
thirteen pay it. A Perk Point is a *choice*, which is worth more than a point
of Attack, and only five exist across the whole profile.

---

Source of truth is `assets/achievements/`. A mod that drops a `.ron` file in
that directory joins the ladder without a recompile, and will not appear above
until this page is regenerated -- edit the table at the top of
[`docs/achievements-gen.py`](achievements-gen.py) and run
`python3 docs/achievements-gen.py` from the repo root. The schema is documented
in [`assets/achievements/README.md`](../assets/achievements/README.md).
"""

import pathlib
pathlib.Path("docs/achievements.md").write_text(doc)
print(doc)
