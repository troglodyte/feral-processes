# Achievements — design

Date: 2026-08-04

A meta-progression layer that survives a run. Depth milestones and boss kills
earned in one run stamp a permanent profile; the profile pays out at the start
of the next run. Gives permadeath a reason to exist and gives breaching a reason to
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

## Trigger evaluation — one system, two trigger shapes

Three of the four triggers are **high-water marks on a monotone counter**:

| Trigger | Read from |
|---|---|
| `ZoneReached(u32)` | the zone resource |
| `StackDepthReached(u32)` | `Locale::Stack { depth }` |
| `CyclesSurvived(u64)` | `GameClock.tick` |

One `achievement_system` in the tick evaluates those with `>=` comparisons and
needs no call sites at all. Hooking `enter_next_zone` and `enter_frame`
separately would create a third of this repo's two-paths-that-must-agree pairs
(`dissolve_tamed_program`/`fuse_companions`,
`damage_structure`/`remove_structure`).

The system reads `Locale` directly and never `Position`, for the reason
`nest_aggro_tick` needs its underground guard: `Position` is pinned to the
surface entrance tile while the party is in the Stack.

### `BossDefeated(species_id)` is a feat, not a threshold

The fourth trigger is event-shaped, and the event already has exactly one
recognition point. `award_loot`'s `species.is_boss` branch
(`game/combat_rewards.rs`) is, per its own neighbouring comments, "the one point
that knows it actually died rather than being fled from" — the guarantee
`mark_lair_cleared` and `raise_trace` are both already spending. A boss-kill
record goes there as a third consumer of it, not anywhere else.

**The call site records; the system still decides.** The kill pushes the species
id into a `RunFeats` drain queue and does nothing else — no achievement lookup,
no reward, no profile write. `achievement_system` drains it in the same tick.
So there is still one place that decides what has been earned, and the kill site
cannot drift from it.

`RunFeats` is a per-tick queue and deliberately **not** an accumulator and
**not** saved. That is only sound because every authored boss trigger names a
single species, so it is satisfied by the kill itself and the profile — which is
written immediately — is the thing that accumulates. A future
"kill N bosses in one run" trigger would need real run state and a save-format
bump; it is not a small addition dressed up as one.

Two bosses ship: Overseer and Wintermute, both `is_boss: true`, both reachable
from zone 1 via `BOSS_SPAWN_CHANCE` (4% of ambient spawns) and preferentially
as Stack lair guardians via `pick_lair_species`.

## Rewards pay at the start of the next run

Earning logs a line and stamps the profile. The reward is applied by
`Game::grant_profile_rewards`, which app-core calls after a new game and not
after a load.

**Never on load.** A save already has its bonus baked into `Stats` and
`perk_points`; re-applying on load would double it on every reload. This is the
one real trap in the feature and gets a dedicated test.

That rule is why installing and granting are two operations rather than a
parameter on `Game::new`. `install_profile` says *what has been earned* and both
paths need it — `achievement_system` must not re-earn on a loaded save.
`grant_profile_rewards` says *pay for it* and only one path wants it. Splitting
them puts the whole rule at one call site instead of inside a shared
constructor. It also leaves `Game::new`'s signature alone, which matters more
than it looks: it has 667 call sites, essentially all engine tests.

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

Thirteen rungs across four axes. Names are placeholders for the flavour pass.

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
| `boss_first` — Root Access | `BossDefeated(any)` | `RandomMainStat(1)` |
| `boss_overseer` — Chain of Command | `BossDefeated("overseer")` | `RandomMainStat(1)` |
| `boss_wintermute` — Ghost in the Wire | `BossDefeated("wintermute")` | `PerkPoints(1)` |

Totals: 7 stat points, 5 Perk Points, 1 starting program.

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

- at most **8** total stat points (7 used; the eighth is budget for a third
  boss species, so adding one does not move the test)
- at most **5** total Perk Points
- at most **1** `StartingProgram`

A fully-cleared profile is worth a bit over one extra level's worth of stats
spread across a whole run — small enough that it flavours a new run rather than
skipping its opening.

The boss rungs are the reason this is 8/5 rather than the 6/4 the three depth
axes alone would need. A boss is a 4% ambient roll or a Stack lair guardian, and
both shipped ones hit far above the ordinary curve (Overseer is 200 HP / 22 ATK
with a `growth_multiplier` of 2.0). Three rungs for two stat points and one Perk
Point is proportionate to that; they are the hardest things on the ladder.

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
| `crates/engine/src/resources.rs` | new run resources: earned-this-run set, pending-profile-write queue, `RunFeats` |
| `crates/engine/src/game/combat_rewards.rs` | `award_loot`'s `is_boss` branch pushes the species id into `RunFeats` |
| `crates/engine/src/game/achievements.rs` | new — `achievement_system`, reward application |
| `crates/engine/src/game/lifecycle.rs` | `install_profile` / `grant_profile_rewards`; both constructors seed an empty `Profile` |
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
- Each threshold trigger fires at its threshold and not before.
- An achievement is earned once — crossing the threshold again does not re-pay.
- Killing a boss earns both `boss_first` and that species' own rung, in one
  tick, from one kill.
- **Fleeing a boss earns nothing.** The record is written where
  `mark_lair_cleared` is, so this is the same guarantee those tests already
  exercise — assert it rather than assume it.
- `RunFeats` is empty again after the system runs, so a second tick does not
  re-earn from the same kill.
- The random stat is a deterministic function of the achievement id.
- `grant_profile_rewards` pays; **`install_profile` alone pays nothing**, which
  is the load path in miniature. Granting twice pays once.
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

- Any *count* trigger evaluated within a single run ("kill N bosses in one
  run", "breach twice in N cycles"). See `RunFeats` above — that needs saved run
  state and a format bump, and is not the small addition it looks like.
- A zone-terminal boss, as distinct from the ambient and lair bosses that
  already exist. That is the other TODO line and its own design.
- Non-depth achievement axes — collection completeness, challenge feats,
  economy landmarks. All three were considered and cut from v1.
- Any in-run achievements screen.
- Profile reset/wipe from inside the game; delete the file.
