# The zone level cap

**Status:** approved, not implemented
**Date:** 2026-08-27

Level is currently the game's primary power axis and it is unbounded for the
player. A run that stalls can always be unstalled by grinding, so gear,
fusion, party composition and Kernel Rings are optional flourishes rather
than the way through. This caps level by zone, for the player and every
companion alike, and turns levelling into a **floor** — the thing you must
have — while gear, fusion and a full party become the **variable**, the thing
that decides whether you beat a lair guardian.

The intended loop: the cap holds you at a zone → the only way out is a
Portal Fragment → fragments come only from fighting underground → a lair
guardian at your cap is beatable only with gear and a developed party →
breaching lifts the cap. Progression stays earned by fighting, which is the
design spine, but *what* the fighting buys changes.

## Part 1 — One cap, derived from the zone

### The formula

```
level_cap = max(ZONE_LEVEL_CAP_FLOOR, 1 + ZONE_LEVEL_CAP_STEP * (zone - 1))
```

Two new `pub const`s in `tuning.rs`. **Linear, and not negotiable** —
`ZoneLevel::stat_multiplier`'s doc comment carries the whole argument, and
this curve races that one directly. A compounding cap outruns a linear
enemy curve wherever the coefficients are put, in the player's favour this
time, which is the same failure wearing the other hat.

The floor exists because zone 1 demands level 1 and a cap of 1 would freeze
a new run on its first tick. Zone 1 is where the game is learned, so it gets
headroom rather than a ceiling.

`ZONE_LEVEL_CAP_FLOOR` and `TALENT_START_LEVEL` both start at 6 and that is
a coincidence, not a relationship. They answer different questions — where a
new run's ceiling sits, and where talents begin — and either may be retuned
without the other. Do not express one in terms of the other.

### Deriving the constants

Starting point: `FLOOR = 6`, `STEP = 5`, giving 6, 6, 11, 16, 21, 26, 31,
36, 41, 46 for zones 1-10 against a measured geared requirement of 1, 5, 8,
14, 18, 24, 29, 35, 41, 46 and a gear-free one of 1, 8, 12, 16, 19, 25, 31,
37, 42, 49 — inside the band at most zones, drifting one to two levels above
gear-free at zones 5 and 6 where the integer search is lumpiest.

**Those two curves are quoted from a doc comment and must not be trusted.**
Re-derive them by calling `balance_sim::min_level_to_clear_zone` directly at
implementation time and fit the constants to what it actually returns.
A doc comment cannot hold two copies in sync, and this repo has been bitten
four times by exactly that, all in `balance_sim.rs`.

The target shape: the cap sits **below the gear-free requirement** (so a
zone cannot be cleared by levelling alone) and **at or above the geared
one** (so a fully equipped party can clear it). Where those two converge —
they narrow to one or two levels past zone 5, because gear's advantage
shrinks as group size grows — the band is thin and being within a level of
it is the best a linear form can do. Do not add a second term to chase the
lumpiness.

### One number for everyone

The player and every companion take the same cap. There is no species term,
no ratio and no per-entity ceiling.

Species identity survives this because `growth_multiplier` still scales what
each level *buys* — two companions at the same level are not the same
power — so the cap flattens the ceiling without flattening the roster.

`CREATURE_MAX_LEVEL` stops being a cap. It survives as the level at which
talents begin and **must be renamed** — `TALENT_START_LEVEL` — because a
constant whose meaning changes under a name it keeps is precisely the trap
the save-format rule warns about, and here nothing would fail to compile.

`Game::companion_level_cap` collapses into the one zone-derived answer.
Its readers — `combat_rewards.rs:825`, `refactor.rs:265`,
`inspection.rs:1375`, `views::level_cap`, `render/talents.rs:68` — all
re-point to it.

### What stays separate, deliberately

**`WORK_XP_LEVEL_CAP = 5` is untouched.** It is the cronjob payout's own
ceiling and it is what stops a developed program being ground up at a
Mining Node. It looks like a level cap and is not one. Anyone "unifying"
the two is deleting that property.

**The arena keeps an absolute ceiling.** `absolute_companion_level_cap()`
survives, renamed `arena_level_ceiling()` to say what it is now for. Five
shipped `dev-arenas/` scenarios author `level: 12` at `zone: 3`; pointing
the arena at the zone cap would silently clamp all five, which is the exact
failure CLAUDE.md already records happening once — those scenarios' old
reports stopped being comparable and nothing said so. A scenario authors its
own composition on purpose and must be able to stage a fight the live game
cannot reach.

**Depth does not lift the cap.** Underground danger is the zone step *plus*
the depth step, so a deep frame is disproportionately hard against a
zone-only cap. That is the feature: the lair guardian is the gated content,
and gear is what closes the gap.

## Part 2 — The Kernel Ring becomes a talent unlock

A Privilege Ring currently buys `LEVELS_PER_RING` levels of ceiling, and
levels above `CREATURE_MAX_LEVEL` pay talent points. With one zone cap that
first half is gone, and the second half would break: a zone-5 companion at
level 21 would earn 15 points against a tree that ships exactly
`KERNEL_RING_MAX * LEVELS_PER_RING = 6` tiers, held by a census.

So the ring stops buying levels and starts buying the right to **spend**:

```
talent_points_earned = min(level.saturating_sub(TALENT_START_LEVEL),
                           rings * LEVELS_PER_RING)
```

`saturating_sub`, not `-`: these are `u32` and a companion below the talent
start level is the common case, not an edge one. The existing derivation in
`game/talents.rs:32` already saturates for the same reason.

Both gates survive — you must be developed *and* hold rings — the tree
depth is unchanged, the 1+2+3 guardian cost is unchanged, and the six
censuses in `tests/assets.rs` need no edit. The ring becomes purely
horizontal, which is the axis this whole feature exists to force.

`open_kernel_ring`'s log line must be rewritten: it currently announces a
new level ceiling, which will no longer be what a ring does.

`Talents` remains a receipt, as `Refactors` is, and a `Stat` node still
bakes into `Stats` at purchase with load not re-applying it. None of that
changes.

## Part 3 — Overflow XP buys Perk Points

### Why it is needed

Perks are already uncapped and repeatable at a flat price
(`game/unlocks.rs:107`), and `Perk::Attacker` writes `stats.atk +=
ATTACKER_BONUS_PER_LEVEL` straight into `Stats`. A flat exchange would make
the perk track a linear, unbounded power source and the grind would return
wearing a different hat. **The sink must be sublinear** so the leak cannot
race the zone curve — the same geometric-versus-linear argument as Part 1.

### The mechanism

`add_xp` currently returns early at the cap and does not accumulate at all.
That changes to: at the cap, XP accumulates in `Experience::xp` as normal
and `add_xp` reports it as unabsorbed via a new field on `LevelGain`.
`add_xp` stays a pure function — the caller does the converting, as it
already does for logging.

Conversion price grows with how much perk power is already held:

```
xp_per_point = OVERFLOW_XP_BASE + OVERFLOW_XP_STEP * perks_held
```

where `perks_held` is `Perks::unlocked.len()` — **derived, never stored**,
matching this repo's idiom throughout. A linear cost makes points earned
grow like the square root of XP spent, comfortably sublinear.

### No save field, and banking for free

The accumulator is `Experience::xp`, which is already saved and already
idle at the cap. Conversion drains it. Whatever has not been converted when
a breach lifts the cap becomes real levels on the spot — so grinding at a
cap is never wasted, it is merely taxed, and the banking behaviour falls out
of the same accumulator rather than needing a second one.

`Experience::xp_to_next` is derived on load by both load paths and is not
read back from the save. That stays true.

### Companions

Companions have no Perk Points, so a capped companion's overflow is simply
not spent — the behaviour creatures have today. Stated here so it is not
read as an oversight later.

## Migration

Nothing needs a `SAVE_FORMAT_VERSION` bump and nothing needs a migration
pass.

- An entity loaded **above** the new cap keeps its level and its stats.
  `add_xp` stops paying it, and clawing back growth already spent is the
  `EquippedItem::fusion_tier` trap — a subtraction with no record of what
  was added. This is the rule `CREATURE_MAX_LEVEL` already applies.
- Talent points do not move. A 3-ring level-12 companion has
  `min(12 - 6, 6) = 6` under the new rule and had 6 under the old one; a
  ringless level-6 companion has 0 under both. Existing `Talents` receipts
  stay valid.
- No field is added, removed, or given a new meaning under a kept name. The
  two renames are constants and functions, not save data.

## The balance obligation

**This is the largest risk in the feature and it is not optional work.**

`balance_sim::min_level_to_clear_zone` fields companions at
`player_level / √2`. With companions taking the player's cap exactly, the
live party is stronger than the gate has ever modelled — companions go from
a ceiling of 12 to roughly 46 by zone 10. Meanwhile wild programs scale by
`ZoneLevel::stat_multiplier`, which is a zone term and not a level term, so
the surface does not scale to meet them.

Therefore:

- `balance_sim`'s curves must be **re-derived, not re-checked**. A moved
  curve here is the expected signal, not a broken test — but every hardcoded
  empirical curve in that file has to be recomputed from the live constants
  and the new party model, and `companion_level_for_player_level` has to be
  changed to the cap rather than the √2 ratio.
- The surface will probably need a retune. Expect `ZONE_STAT_STEP` or the
  spawn curve to want moving once the sim is re-derived. Do not pre-empt
  this; measure first.
- Run the shipped `dev-arenas/` scenarios before and after and record the
  deltas, never the absolutes — a moved baseline is a reshuffled RNG stream,
  not a difficulty change.
- Write the result to `docs/measurements/` with the commands that produced
  it. The data behind these runs is gitignored and a number not written down
  costs CPU-hours to recover.

## Testing

Every test carries a mutation check: delete the fix, watch it fail, restore.

**The cap**
- The cap is the same number for the player and a companion at a given zone.
- `add_xp` stops levelling at it and a breach lifts it.
- An entity loaded above the cap keeps its level and its stats.
- The cap is linear: the per-zone step is constant across a swept range, the
  test `ZONE_STAT_STEP` already has a peer for.
- The cap is bounded by both clear curves within a stated tolerance,
  asserted against `min_level_to_clear_zone` **called**, never against
  transcribed numbers. The tolerance is real and must be a named constant
  with its reason attached: the integer search is lumpy and the two curves
  converge to within a level or two past zone 5, so a linear cap cannot sit
  strictly inside the band at every zone. Fit the constants first, then set
  the tolerance to the smallest value the fit actually achieves — never the
  other way round, or the test is written to pass rather than to bound.

**The ring**
- Talent points are the saturating `min` above, including for a companion
  below the talent start level, which must yield 0 rather than underflow.
- A ringless companion at the zone cap earns none.
- A 3-ring companion earns exactly a full tree and no more.
- Opening a ring grants no level, no stats and no XP — the existing rule.
- The six `tests/assets.rs` talent censuses still pass untouched.

**Overflow**
- At the cap, XP accumulates rather than being discarded.
- It converts to Perk Points, and the price rises with perk levels held.
- Points earned against XP spent is sublinear across a swept range — the
  property, not a magic number.
- Unconverted overflow becomes levels when a breach lifts the cap.
- A capped companion's overflow is not spent and does not panic.

**Kept separate**
- `WORK_XP_LEVEL_CAP` still stops cronjob XP at its own level, unchanged.
- The five `dev-arenas/` scenarios authoring `level: 12` still stage at 12.

**Gates**
- `cargo test --workspace`
- `cargo test -p feral-processes-engine balance_sim` — expected to move; the
  curves are re-derived as part of this work, not patched to pass.
- `cargo clippy --workspace`, `cargo fmt`

## Out of scope

**Downed programs and the Repair Bay** ship separately
(`2026-08-27-downed-programs-and-the-repair-bay-design.md`). The two share a
thesis — power should be horizontal, the expedition should be committing —
but neither blocks the other, and that one has no balance obligation at all
while this one is mostly balance obligation.

**Retuning the surface** is downstream of the re-derivation above and gets
its own decision once there are numbers. Do not fold a `ZONE_STAT_STEP`
change into this work on the assumption it will be needed.
