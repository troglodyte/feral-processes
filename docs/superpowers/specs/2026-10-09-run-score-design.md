# Run score: an epitaph for a run

**Date:** 2026-10-09

## Goal

When a run ends, by a Permadeath game over or by escaping the Basin, the
player sees a **score card**: a breakdown of what the run achieved, line by
line, with a difficulty multiplier and a total. The same card can be read
mid-run on its own tab of the player's manifest.

Every death or escape adds the run's score to a **lifetime score** in
`profile.ron`. The next run's character creation reads that lifetime score
and grants a small bonus of stat points, perk points and creation credits,
each capped.

## What the player said (decisions)

- **Purpose: an epitaph that also pays forward.** The card summarises a
  run. It is not a live goal or a leaderboard, and there is no high-score
  table. Its total also feeds the next run's creation (see Carryover).
- **Shown in three places:** the game-over screen, a final screen after the
  escape ending, and a **Score tab** on the player's manifest with the full
  card. `run_history.log` also gets the **total only**.
- **Four pillars:** fighting, depth/progress, building, collection.
- **Difficulty is a multiplier** on the total. Run length is ignored.
- **Mixed source:** what is still live at scoring time is read live. What
  disappears (kills, bosses, programs compiled then lost, the deepest tier
  reached) is kept in a new saved `RunTally`.
- **Weights are named constants in `tuning.rs`**, not a data file.
- **A kill is worth the foe's level**, so deep or strong fights outscore
  farming the surface.
- **Sortie kills count**, also at foe level.
- **Carryover is a cumulative bank.** The lifetime score only grows.
- **It buys a little of each:** stat points, perk points and creation
  credits, each on its own threshold-and-cap ladder in `tuning.rs`.
- **Only a death or an escape banks.** Abandoning a run banks nothing.
- **Old saves start the tally at zero.** `#[serde(default)]`, no
  `SAVE_FORMAT_VERSION` bump, no backfill.

## Non-goals

- A high-score table. `profile.ron` keeps one lifetime number, not a list
  of runs.
- A HUD score or any score shown outside the three places above.
- Score as an input to anything except the creation bonus: no unlocks,
  achievements or in-run balance read it.
- Banking on abandon, at any rate.
- A time or speed component.
- Penalties. Dying is not a deduction; the card records what was done.

## Design invariants it must respect

- **Progression is earned by fighting.** A fighting run must outscore a
  building run under the shipped constants. A test pins this (see Testing).
- **No player-facing tick vocabulary.** Labels say "programs defeated", never
  ticks.
- **GUI text never clips or wraps.** The widest card is checked headlessly.

## The card

Each line is a label, a count and the points it contributes. A line whose
count is zero is still shown, so the card always has the same shape and
reads as a record of what was *not* done too.

| Pillar | Line | Points | Source |
|---|---|---|---|
| Fighting | Programs defeated | sum of foe levels × `SCORE_PER_FOE_LEVEL` | `RunTally.foe_levels` |
| | Bosses defeated | count × `SCORE_PER_BOSS` | `RunTally.bosses` |
| Depth | Deepest Stack tier | tier × `SCORE_PER_TIER` | `RunTally.deepest_tier` |
| | Phase keys held | count × `SCORE_PER_KEY` | live, `components::PhaseKeys` |
| | Escaped the Basin | `SCORE_ESCAPE` or 0 | live |
| Building | Structures standing | count × `SCORE_PER_STRUCTURE` | live |
| Collection | Programs compiled | count × `SCORE_PER_PROGRAM` | `RunTally.compiled` |
| | Achievements earned this run | count × `SCORE_PER_ACHIEVEMENT` | `RunTally.achievements` |

**Multiplier:** `1.0`, times `SCORE_PERMADEATH_MULT` under
`DifficultyMode::Permadeath`, plus `SCORE_PER_BAND` for each
`EnemyStrength` band above `Standard`. The card shows the multiplier as its
own line. **Total** = sum of line points × multiplier, rounded down.

"Programs compiled" is tallied, not counted live, so a program that was
compiled and later fell or was sold still counts. Achievements are tallied
for the same reason: the profile records what has ever been earned, not what
this run earned.

## Carryover

**Banking.** `RunTally` carries `banked: u64`, the part of this run's total
already added to the profile. A death (`Mode::GameOver`) or an escape
(`Game::escape_basin`) adds `card.total - banked` to
`Profile::lifetime_score` and sets `banked = card.total`. A run continues
after escape, so a Permadeath run can bank twice: once at the exit, and the
difference at a later death. Nothing is counted twice, and play after the
escape still counts if the run later ends in death. A Forgiving run never
flatlines, so it banks only at escape.

**Known limitation, shared with achievements:** the profile is written the
moment it changes, so reloading an older save and escaping again banks
again. Achievements accept the same gap today; this feature does not add an
autosave to close it.

**The bonus** is derived from `lifetime_score` each time creation opens.
It is never spent, so it grows over runs until each ladder caps:

| Bonus | Rule | Added to |
|---|---|---|
| Stat points | `min(lifetime / SCORE_PER_BONUS_STAT_POINT, BONUS_STAT_POINT_CAP)` | the Points step's budget, beside `CREATION_STAT_POINTS` and the banked `RandomMainStat` rewards |
| Perk points | `min(lifetime / SCORE_PER_BONUS_PERK_POINT, BONUS_PERK_POINT_CAP)` | the perk picker's allowance |
| Credits | `min(lifetime / SCORE_PER_BONUS_CREDIT, BONUS_CREDIT_CAP)` credits | `CREATION_CREDITS` |

One pure function, `score::creation_bonus(lifetime) -> CreationBonus`, holds
all three rules. Creation and its tests call it; nothing recomputes it.

**Shown at creation.** The Points step names the bonus's source next to its
budget ("+N from past runs"), and the creation summary of the profile shows
the lifetime score. The arena's scripted creation does not apply the bonus,
so arena numbers and `balance_sim` stay independent of whoever's
`profile.ron` is on disk.

## Engine

**`crates/engine/src/score.rs`** (new module) holds the whole formula as
pure code with no `World`:

- `ScoreInputs`: a plain struct of every count above, plus `DifficultyMode`,
  the `EnemyStrength` band and `escaped: bool`.
- `ScoreLine { label: &'static str, count: u64, points: u64 }`.
- `ScoreCard { lines: Vec<ScoreLine>, multiplier: f32, total: u64 }`.
- `pub fn card(inputs: &ScoreInputs) -> ScoreCard`.

**`RunTally`** (new saved resource in `resources.rs`):

```rust
pub struct RunTally {
    pub foe_levels: u64,
    pub bosses: u32,
    pub compiled: u32,
    pub achievements: u32,
    pub deepest_tier: u32,
    pub banked: u64,
}
```

`Profile` gains `lifetime_score: u64` behind `#[serde(default)]`, so an
existing `profile.ron` loads at zero.

It goes into `SaveData` behind `#[serde(default)]`. Each field is written
directly at its one site, not through a queue, so `RunFeats`' one-drainer
rule is untouched:

- `foe_levels`, `bosses`: `combat_rewards::award_loot`, the single kill site
  for present fights.
- `foe_levels`: the sortie battle resolution in `game/sortie.rs`, summing the
  hostiles' levels before the unconditional despawn.
- `compiled`: the compile sites in `combat_rewards.rs` that already note
  `Deed::Tamed`. Both go through one helper, so the two sites cannot drift.
- `achievements`: where a rung is earned and the profile is dirtied.
- `deepest_tier`: where the player enters a Stack tier, as a high-water mark.

The plan confirms each site against the source before relying on it.

**`Game::score_card() -> ScoreCard`** builds `ScoreInputs` from `RunTally`
and live state, then calls `score::card`. This is the one API all three
displays read. `history_summary` appends ` Score: N.` to its existing line.

## App-core and GUI

- **Game over:** `render/meta.rs::draw_game_over` draws the card lines
  under the existing summary, then the multiplier and total.
- **Escape:** after the last `Mode::Ending` screen, a final screen titled
  "Escaped" draws the same card before returning to the menu.
- **Player manifest:** a `Score` face on the player's manifest, reached by
  `Tab` like the existing faces, showing the full card. Program manifests
  do not get it.
- One shared row builder turns a `ScoreCard` into popup rows, so the three
  screens cannot lay the card out differently.

## Testing

- **`score::card` unit tests:** each line's arithmetic, the multiplier for
  each difficulty and for a raised band, and an all-zero card that still
  has every line.
- **Spine test:** a scripted fighter input (kills and depth, small base)
  must outscore a scripted turtle input (large base, few kills) under the
  live `tuning.rs` constants. If someone retunes a weight so building wins,
  this test fails.
- **Tally sites:** a present kill moves `foe_levels` by the foe's level; a
  boss kill moves `bosses`; a sortie battle moves `foe_levels`; a compile
  moves `compiled`; entering a deeper tier raises `deepest_tier` and a
  shallower one does not lower it. Each is mutation-checked: the test must
  fail with the write removed.
- **Save→load round trip** of a non-zero `RunTally` through
  `save_to_file`/load, not only a RON round trip.
- **Old save:** a save without the field loads with a zero tally.
- **Banking:** a death banks the card total; an escape followed by a death
  banks exactly the card total overall, never more; an abandon banks
  nothing; a Forgiving escape banks once.
- **`creation_bonus`:** each ladder's threshold, each cap, and zero.
- **Creation:** a profile with a lifetime score raises the Points budget,
  the perk allowance and the credits by `creation_bonus`'s amounts; a
  profile without the field gives no bonus. Mutation-checked.
- **Profile round trip:** `lifetime_score` survives write and reload, and
  an old `profile.ron` without it still loads with its achievements intact.
- **Layout:** the widest possible card fits its popup headlessly.
- **`run_history.log`:** the summary line ends with the score.

Expected `balance_sim` impact: none. Only `score.rs` and creation read the
scoring constants. The plan confirms that `balance_sim` does not build its
parties through creation, and runs it as a gate either way.

## Docs

- `CHANGELOG.md` entry with the release.
- No asset schema changes. `assets/achievements/README.md` says achievements
  are "the game's only cross-run progression"; that sentence changes to
  name the score bonus too.
