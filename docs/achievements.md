# Achievement ladder

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
| achievements | 13 |
| trigger shapes used | 4 of 4 |
| reward shapes used | 3 of 3 |
| stat points on offer | 7 (ceiling 8) |
| perk points on offer | 5 (ceiling 5) |
| free programs | 1 (ceiling 1) |

## The ladder

| Achievement | id | Trigger | Reward |
|:---|:---|:---|:---|
| Uptime | `uptime_500` | `CyclesSurvived(500)` | `RandomMainStat(1)` |
| Long Uptime | `uptime_2000` | `CyclesSurvived(2000)` | `RandomMainStat(1)` |
| Persistent Process | `uptime_5000` | `CyclesSurvived(5000)` | `PerkPoints(1)` |
| First Breach | `breach_zone_2` | `ZoneReached(2)` | `RandomMainStat(1)` |
| Deep Cut | `breach_zone_4` | `ZoneReached(4)` | `PerkPoints(1)` |
| Sector Runner | `breach_zone_6` | `ZoneReached(6)` | `RandomMainStat(1)` |
| Far Sector | `breach_zone_8` | `ZoneReached(8)` | `PerkPoints(1)` |
| Down the Stack | `stack_depth_3` | `StackDepthReached(3)` | `RandomMainStat(1)` |
| Frame Diver | `stack_depth_5` | `StackDepthReached(5)` | `PerkPoints(1)` |
| Bottom Frame | `stack_depth_8` | `StackDepthReached(8)` | `StartingProgram("scrapper")` |
| Root Access | `boss_first` | `BossDefeated(None)` | `RandomMainStat(1)` |
| Chain of Command | `boss_overseer` | `BossDefeated("overseer")` | `RandomMainStat(1)` |
| Something in the Wire | `boss_wintermute` | `BossDefeated("wintermute")` | `PerkPoints(1)` |

## Progress tracks

Three of the four trigger shapes are high-water marks on a number, so they
read as tracks. Each is drawn on its own scale — the gaps are proportional
within a row, never between rows.

```
cycles survived  500           2000                      5000
                 *-------------*----------------------------+
sector breached  2             4              6             8
                 *-------------+--------------*-------------+
stack depth      3                5                         8
                 *----------------+-------------------------@

               * stat point    + perk point    @ free program
```

Every track pays a stat point first and a perk point second, so the cheap end
of each is the one that hands a new run a flat number and the far end is the
one that hands it a choice. The Stack is the only track that ends in
something else: eight frames down pays a tamed Scrapper, which is the single
`StartingProgram` the ceiling allows.

The three tracks are also not equally long in play. Cycles pass on their own —
500 is a slow afternoon and 5000 is only patience. Sector 8 needs seven
breaches, each one paid for in Portal Fragments — which only a Stack lair
guardian drops, so the Sector track runs through the Frame track whether or
not a player set out to climb both. Frame 8 of the Stack is the shortest
track on the page and by some distance the hardest, because depth scales what
lives down there and the Trace is counting.

## Bosses

`BossDefeated` is the one trigger that names a thing rather than a number, and
the three rungs stack: killing a Wintermute fires `boss_first` as well, so the
first boss a run brings down is always worth two rungs.

| Achievement | Trigger | Pays |
|:---|:---|:---|
| Root Access | `BossDefeated(None)` | `RandomMainStat(1)` |
| Chain of Command | `BossDefeated("overseer")` | `RandomMainStat(1)` |
| Something in the Wire | `BossDefeated("wintermute")` | `PerkPoints(1)` |

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

```
REWARD BUDGET                    spent / ceiling

stat points     7 / 8   #######.  (1 left)
perk points     5 / 5   #####  <- ladder is full
free program    1 / 1   #  <- ladder is full
```

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

