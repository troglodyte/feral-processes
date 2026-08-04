# Achievements — design

Date: 2026-08-04

A meta-progression layer that survives a run. Depth milestones earned in one
run stamp a permanent profile; the profile pays out at the start of the next
run. Gives permadeath a reason to exist and gives breaching a reason to
continue — the two TODO lines this came from.

## Storage

`profile.ron` at the repo root, beside `run_history.log`. It is deliberately
**not** part of `SaveData`:

- No `SAVE_FORMAT_VERSION` bump, and none for any later field either —
  `#[serde(default)]` keeps an old profile loading, the same contract the
  `.ron` assets have and the bincode save deliberately does not.
- A profile is run-spanning state, like the history log. A save is one run.
- It cannot collide with the save picker: `App::list_saves` filters on `.bin`.

A malformed or unreadable profile is skipped with a logged warning and treated
as empty. Never a panic — the `PerkDb::load_dir` pattern.

```ron
Profile(earned: [
    Earned(id: "breach_zone_4", first_tick: 812, permadeath: true, rolled_stat: None),
    Earned(id: "breach_zone_2", first_tick: 190, permadeath: false, rolled_stat: Some(Atk)),
])
```

The profile is written **the moment an achievement is earned**, not at run end.
A permadeath run that ends badly must not lose what it proved.

## Achievements are data

`assets/achievements/*.ron`, one file per achievement, loaded by
`AchievementDb::load_dir` following the existing db pattern. Adding a rung is a
file drop; no Rust.

```ron
AchievementDef(
    id: "breach_zone_4",
    name: "Deep Cut",
    description: "Breach into zone 4.",
    trigger: ZoneReached(4),
    reward: PerkPoints(1),
)
```

`reward` is exactly one of:

- `RandomMainStat(n)` — n points into one of ATK / DEF / Integrity (`max_hp`) /
  Decompiler, chosen by a roll made once at earn time and then stored.
- `PerkPoints(n)` — added to the player's Perk Point pool at run start, spent
  through the existing `perks.rs` machinery.
- `StartingProgram(species_id)` — the next run begins with that program tamed
  and in the party.

One reward per achievement is what makes the total power bounded by a finite
authored list and therefore assertable.

## Trigger evaluation — one polling system

All three shipping triggers are **high-water marks on a monotone counter**:

| Trigger | Read from |
|---|---|
| `ZoneReached(u32)` | the zone resource |
| `StackDepthReached(u32)` | `Locale::Stack { depth }` |
| `CyclesSurvived(u64)` | `GameClock.tick` |

So one `achievement_system` in the tick evaluates the whole authored list with
`>=` comparisons and needs no call sites at all.

That is the point. Hooking `enter_next_zone` and `enter_frame` separately would
create a third of this repo's two-paths-that-must-agree pairs
(`dissolve_tamed_program`/`fuse_companions`,
`damage_structure`/`remove_structure`). One system, one place, and a new
threshold trigger costs a `.ron` file and nothing else.

The system is guarded the way `nest_aggro_tick` is: it reads the player's
locale, so it must not confuse the surface-pinned `Position` with Stack
coordinates. It reads `Locale` directly and never `Position`.

### The boss rung is not in v1

`BossDefeated` is the first **event**-shaped trigger, and no boss exists.
Shipping a rung that can never fire is a half-finished implementation. The
extension point is stated instead: a `BossDefeated(id)` variant, a set of fired
flags on the run resource, and one call site at the kill. Its stat point is
pre-budgeted in the ceiling below, so adding it later does not move the test.

## Rewards pay at the start of the next run

Earning logs a line and stamps the profile. The reward is applied in
`Game::new`, which gains the profile as a parameter.

**Never on load.** A save already has its bonus baked into `Stats` and
`perk_points`; re-applying on load would double it on every reload. This is the
one real trap in the feature and gets a dedicated test.

Paying at new-game is also the only timing all three reward types share — a
starting tamed program has no mid-run meaning — so all three behave alike
rather than two being immediate and one deferred.

### The random roll

`RandomMainStat` rolls from a local `StdRng` salted off the achievement id,
never from `resources::GameRng`, for the reason `stack::generate`,
`Game::spawn_surface_links` and `Game::orphan_species` all avoid it: a draw off
the shared stream is not reproducible across a save/load and shifts every later
roll in the run. The rolled stat is written into the profile entry, so it is
decided once and never rerolls.

## Difficulty

Either mode earns an achievement. The entry records `permadeath: bool` — the
mode it was **first** earned in. A later re-earn on permadeath upgrades the flag
to `true`; it never downgrades. The reward pays once regardless of mode or
re-earns.

## The ladder

Ten rungs across three axes. Names are placeholders for the flavour pass.

| id | Trigger | Reward |
|---|---|---|
| `breach_zone_2` — First Breach | `ZoneReached(2)` | `RandomMainStat(1)` |
| `breach_zone_4` — Deep Cut | `ZoneReached(4)` | `PerkPoints(1)` |
| `breach_zone_6` — Sector Runner | `ZoneReached(6)` | `RandomMainStat(1)` |
| `breach_zone_8` — Far Sector | `ZoneReached(8)` | `PerkPoints(1)` |
| `stack_depth_3` — Down the Stack | `StackDepthReached(3)` | `RandomMainStat(1)` |
| `stack_depth_5` — Frame Diver | `StackDepthReached(5)` | `PerkPoints(1)` |
| `stack_depth_8` — Bottom Frame | `StackDepthReached(8)` | `StartingProgram("scrapper")` |
| `uptime_500` — Uptime | `CyclesSurvived(500)` | `RandomMainStat(1)` |
| `uptime_2000` — Long Uptime | `CyclesSurvived(2000)` | `RandomMainStat(1)` |
| `uptime_5000` — Persistent Process | `CyclesSurvived(5000)` | `PerkPoints(1)` |

Totals: 5 stat points, 4 Perk Points, 1 starting program.

Scrapper is the starting program because `balance_sim`'s sweep already models a
mid-grade party as three of them — it is the "better than nothing, not a head
start" tier. The species is authored in the `.ron` like everything else, so it
is a one-word change and not an engine decision.

**The three `CyclesSurvived` thresholds are uncalibrated.** Nothing in the repo
tells us how long a run actually lasts, and no test can. They are arithmetic
guesses to be checked against a real run, in the same position as the four
bounded-income knobs and the six Stack tuning numbers.

## Bounding the power

`RandomMainStat` is the unbounded-permanent-buff shape this design has closed
off twice already (the scan action, the Market fragment listing). The authored
ladder therefore carries a hard ceiling asserted over the real `.ron` files, the
way item values already are by
`no_craftable_item_is_worth_more_than_its_ingredients`:

- at most **6** total stat points (5 used; the sixth is the boss rung's budget)
- at most **4** total Perk Points
- at most **1** `StartingProgram`

A fully-cleared profile is worth roughly one extra level's worth of stats spread
across a whole run — small enough that it flavours a new run rather than
skipping its opening.

`balance_sim` does not model the profile and will not. It simulates a run's own
curve; the profile sits outside that curve, which is exactly why the ceiling
test exists instead.

## UI

- **Achievements screen on the main menu.** `crates/gui/src/render/meta.rs` is
  already "the screens outside a run", which is what the profile is. Lists every
  authored achievement, earned or not, with its reward and — where earned — the
  cycle, the mode, and the rolled stat.
- **In-run**, earning pushes a log line. It needs a `MessageKind` that survives
  `retain_outcomes_since_battle`, since a rung can be crossed mid-fight.
- **Nothing added to the group menus.** Menu consolidation is already unplayed;
  this does not add to it.

## Files

| File | Change |
|---|---|
| `crates/engine/src/achievements.rs` | new — defs, db, triggers, rewards, profile, profile IO |
| `crates/engine/src/resources.rs` | new run resource: earned-this-run set + pending drain queue |
| `crates/engine/src/game/achievements.rs` | new — `achievement_system`, reward application |
| `crates/engine/src/game/lifecycle.rs` | `Game::new` takes the profile and applies rewards; `Game::load` does not |
| `crates/engine/src/lib.rs` | wire the db, the resource, the system into the tick |
| `crates/app-core/src/app/lifecycle.rs` | `App::new` takes a profile path; loads on start; drains and writes after tick |
| `crates/app-core/src/app/menus.rs` | main-menu row + key for the achievements screen |
| `crates/gui/src/render/meta.rs` | draw the achievements screen |
| `crates/launcher/src/main.rs` | supply `profile.ron`'s path |
| `assets/achievements/*.ron` | the ten rungs |
| `assets/achievements/README.md` | new — schema reference, per the moddability rules |
| `README.md`, `CHANGELOG.md` | the doc obligation |

## Testing

Engine:

- `AchievementDb::load_dir` loads the shipped set; a malformed file is skipped
  with a warning, not a panic.
- Each trigger fires at its threshold and not before.
- An achievement is earned once — crossing the threshold again does not re-pay.
- The random stat is a deterministic function of the achievement id.
- `Game::new` applies a profile's rewards; **`Game::load` does not**.
- The permadeath flag upgrades on re-earn and never downgrades.
- The ceiling test over the real `.ron` assets.

app-core:

- Earning writes the profile immediately, and the written file reloads equal.
- An absent profile file is an empty profile, not an error.
- The achievements screen's row count matches what the renderer draws (the
  read-only-screen rule: app-core owns the count, gui draws the rows).

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`, and
`cargo test -p feral-processes-engine balance_sim` to confirm the curve did not
move.

## Not in scope

- The boss rung and the `BossDefeated` trigger (the other TODO line).
- Non-depth achievement axes — collection completeness, challenge feats,
  economy landmarks. All three were considered and cut from v1.
- Any in-run achievements screen.
- Profile reset/wipe from inside the game; delete the file.
