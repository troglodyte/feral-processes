# Achievements (mods)

Drop a `.ron` file in this directory and it's picked up automatically the next
time a game session starts — no recompiling required. A malformed file is
skipped with a warning logged in-game rather than crashing startup.

Unlike `assets/perks/`, this **is** a content directory: a new achievement is a
new file, not a new enum variant. The `trigger` and `reward`
shapes below are the whole vocabulary, and every combination of them already
works.

## What an achievement is

Achievements and the run score bonus (`crates/engine/src/score.rs`) are the
game's only cross-run progression. Achievements are earned inside
a run, recorded the moment they're earned into `profile.ron` at the repo root,
and paid out at the **start of the next run** — never mid-run, and never when
you load a save (a save already has its bonus baked into your stats).

That means a run you lose still counts for something. It's also why a reward
that only makes sense at run start — a free program — sits alongside the stat
rewards rather than being a special case.

`profile.ron` is a plain readable RON file. Delete it to wipe your profile;
there is no in-game reset.

## Schema

```ron
(
    // Unique. Also the key in profile.ron, so renaming an id makes an
    // already-earned achievement look unearned. Treat it as permanent once
    // anyone has played with it.
    id: "breach_zone_4",

    // Shown on the achievements screen, reachable from the main menu.
    name: "Deep Cut",

    // The line under the name. Say what has to happen to earn it — this is
    // the only place a player is told. Player-facing prose, not a trigger
    // description; the engine never derives it from the trigger, so a
    // retuned threshold leaves the wording stale until you edit it.
    description: "Reach sector 4. Everything out here was compiled against a harder spec than you were.",

    // What earns it. Exactly one of the shapes below.
    trigger: ZoneReached(4),

    // What it pays, once, at the start of the next run. Exactly one of the
    // four below.
    reward: PerkPoints(1),
)
```

`id`, `name`, `description` and `trigger` are required; `reward` defaults to `None`.

### `trigger`

| Written as | Earned when |
|---|---|
| `ZoneReached(4)` | the run has breached to sector 4 or deeper |
| `StackDepthReached(3)` | the party stands in a Stack frame 3 or more levels down |
| `CyclesSurvived(500)` | the run's clock reaches 500 cycles |
| `BossDefeated(None)` | any boss program dies |
| `BossDefeated(Some("overseer"))` | that species' boss dies |
| `PhaseKeyFound(3)` | the player holds the Zone 3 Phase Key (`assets/phase_keys/`) |
| `AllPhaseKeys` | the player holds all ten Phase Keys |
| `StoryComplete` | the player has left the Basin through the Basin Exit |

The first three are high-water marks: the game checks them every cycle, so
they fire the moment the number is crossed and never again. A threshold of 0
or 1 for `ZoneReached` is earned on the first cycle of a new game, which is
probably not what you want.

`BossDefeated` names a species id from `assets/species/`, and that species
needs `is_boss: true` to be reachable at all. **Fleeing does not count** — the
kill is recorded at the one point in combat that knows the boss actually died.
There is deliberately no "kill N bosses in one run" trigger — but not, any
longer, because it would be expensive. It used to say counting within a run
needs saved run state the game doesn't keep; contracts added exactly that
state (`resources::ActiveContracts`), so a counting trigger is now cheap. It
is simply a separate feature, and belongs in its own change rather than
riding in on this note.

### `reward`

| Written as | Pays |
|---|---|
| `RandomMainStat(1)` | 1 stat point, banked at the start of the next run and spent on the Points screen |
| `PerkPoints(1)` | 1 Perk Point, spent in the perk picker like any other |
| `StartingProgram("scrapper")` | that species, tamed and owned, at the start of the next run |
| `None` | nothing; recognition only. This is also what a missing `reward` means |

The name is a leftover from when the reward rolled one of four stats; it now
banks plain stat points, and the name stays because it is what the shipped
files say. A `profile.ron` written under the old rule still loads.

`StartingProgram` hands the program over **owned but not deployed** — it goes
in your roster, and you add it to the party yourself, like every other
acquisition. An id naming a species that doesn't exist logs a warning and pays
nothing.

A reward of `0` (`RandomMainStat(0)`, `PerkPoints(0)`) is skipped with a
warning, the same way a `cost: 0` perk is: a rung that pays nothing is a
mistake that reads as a working file.

## The ceiling — why your eighth stat point is rejected

The whole authored ladder is capped, and the cap is asserted over these files
by `the_full_ladder_stays_under_its_ceiling` in
`crates/engine/src/achievements.rs`:

- at most **8** total `RandomMainStat` stat points
- at most **5** total `PerkPoints`
- at most **1** `StartingProgram`

The shipped rungs that pay anything spend 7 / 5 / 1. Add another rung paying a stat point
and you're at the line; add a second and the engine's test suite fails.

This exists because `balance_sim.rs` — the game's balance regression gate —
simulates a *single run's* curve and does not model the profile at all. The
profile sits outside everything that gate can see, so a bound asserted here is
the only thing standing between a cross-run buff and unbounded permanent power.
If you're modding for yourself and want a bigger profile, raise
`MAX_PROFILE_STAT_POINTS` and friends in `crates/engine/src/tuning.rs` — they
live there with every other difficulty knob, deliberately on the code side of
the moddability line.

## Deleting a file

An achievement with no file here stops being listed and can't be earned. It is
not an error, and it is not retroactive: an entry already in `profile.ron`
stays there, and is simply ignored — restore the file and it counts again.
