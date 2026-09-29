# Level-up stat allocation

**Status:** spec, awaiting review. Not built.

## Intent

When the player levels up, they choose their stat growth instead of receiving
it automatically, then choose perks. The flow opens after the battle (or any
other level-up) ends. Unspent points bank until the player spends them — at
the next level-up or from a menu at any time.

Companions will get the same treatment later. Nothing here builds that, but
every new piece is keyed by *whose* stats they are, never hardwired to the
player, so the companion change is "give them the component and switch their
growth mode", not a rewrite.

## Decisions (from the brainstorm)

| Question | Decision |
|---|---|
| Replace automatic growth or add to it | **Replace** HP/ATK growth for the player. Decompiler keeps its automatic `DECOMPILER_SKILL_PER_LEVEL`. |
| Flow | Existing Level Up report → **Points** (the creation Points screen, generalised) → existing in-run **Perks** screen. `Esc` at any step leaves; unspent points stay banked. |
| Where banked points are spent | Also from a menu at any time, with a HUD hint via `Game::attention`. |
| Non-battle level-ups (contracts) | Same flow, opened the next time the game is in `Mode::Playing`, exactly as the report page is today. |
| Def (mitigation) | Offered at creation's 1:1 price, **measured before shipping** (below). |
| Perk screen | The existing in-run `Mode::Perks`, not the creation Perks step. |
| Stat respec | Not in scope. |

## Sizing: 6 points per level

At creation prices one point buys 1 ATK, `CREATION_GAIN_INTEGRITY` (6) max HP,
1% mitigation or 1 Decompiler. Today's growth is `HP_PER_LEVEL` 24 +
`ATK_PER_LEVEL` 2 = 4 Integrity + 2 ATK = **6 points**. New constant
`STAT_POINTS_PER_LEVEL = 6` in `tuning.rs`, with a compile-time assert that
it equals `HP_PER_LEVEL / CREATION_GAIN_INTEGRITY + ATK_PER_LEVEL` so the two
cannot drift.

The *canonical spend* — 4 Integrity + 2 ATK per level — reproduces today's
numbers exactly, so `balance_sim`'s curves and the fitted `zone_level_cap`
values do not move.

## Engine

### One pure definition of what a unit buys

`progression::stat_units(stat: MainStat, units: u32) -> StatDelta` (max_hp,
atk, mitigation, decompiler). It is the only place a point's worth is defined.
Callers:

- `Game::apply_creation_stats` (creation commit).
- app-core's creation `stat_value`, which today *mirrors*
  `apply_creation_stats` by copy. Per the project rule it becomes a call.
- The new level-up spend.
- `balance_sim`'s player path, via the canonical spend.

### Growth mode, not a player special case

`add_xp` takes a growth parameter instead of a bare `growth_multiplier`:

```rust
pub enum Growth { Auto { multiplier: f32 }, Points }
```

- `Auto` behaves exactly as today. Companions, workers, sorties and the arena
  keep passing it.
- `Points` grows no stats. It still full-heals on level-up and still banks
  overflow. `LevelGain` gains a `stat_points` field (`STAT_POINTS_PER_LEVEL ×
  levels`).

The caller decides the mode from the entity. For now that means only
`award_player_xp`, which passes `Points`. To move companions later,
`award_companion_xp` passes `Points` when the entity has `StatPoints`.

### Banked points are a component

`components::StatPoints(u32)` goes on the player entity. It is not a field of
`Perks` or a player resource, so a companion can carry the same component
later. `award_player_xp` adds `gain.stat_points` to it and records the amount
in `XpTally` for the log line, as Perk Points are today.

### Spending

`Game::spend_stat_points(target, spend: &[(MainStat, u32)]) -> Result<StatDelta, SpendError>`

- `target` is a unit identifier with only a `Player` variant for now. The
  plan picks the existing type (or adds a minimal enum) that a companion
  variant can join.
- It validates the whole spend against `StatPoints` before writing anything,
  so a refused spend leaves nothing half-applied.
- It applies the spend through `stat_units` and records the result in
  `BoughtStats`, the existing receipt for purchased stats (see the
  `a-baked-stat-needs-a-receipt` trap). Integrity raises `hp` together with
  `max_hp`, as it does at creation.
- It fails with `SpendError::{InsufficientPoints, NoSuchTarget}`, not a panic.

### Save

`PlayerSave.stat_points: u32` with `#[serde(default)]`, so there is no
`SAVE_FORMAT_VERSION` bump. It needs a save→load test, not only a RON
round-trip (`ron-round-trip-cannot-catch-a-skipped-field`).

The pending level-up flow itself stays transient, as `PendingLevelUp` is
today. Points survive a reload; the report page does not.

### Attention

`Game::attention` reports unspent stat points in the same way it already
reports unspent Perk Points.

## App-core

### The Points screen is generalised, not duplicated

The creation Points step's state (`spent` per stat, a pool, and a base to
display against) becomes a standalone `StatAllocation` value:

```text
StatAllocation { target, pool: u32, base: StatLine, spent: [u32; 4] }
```

- **Creation:** `pool = CREATION_STAT_POINTS`, `base = PLAYER_BASE_STATS`.
  The "cannot leave while points are affordable" refusal stays creation-only.
- **Level-up / menu:** `pool = StatPoints`, `base` = the target's current
  stats. Leaving is always allowed, and `Enter` commits through
  `spend_stat_points`.

The row building (`CreationRow::Stat`), `spend_on_row` and the cost lookup
move onto `StatAllocation`, and `Mode::CreateCharacter`'s Points step holds
one. A new `Mode::AllocateStats(StatAllocation)` reuses the same rows and
handler. Its commit and leave rules come from what the allocation is *for*, a
small enum (`Creation | Owned`), not from which mode holds it.

### Flow

- **`Mode::LevelUp`:** `Enter` → `Mode::AllocateStats`. `P` still jumps
  straight to Perks and `Esc` still returns to play.
- **`Mode::AllocateStats`:** commit, or `Esc` without spending, → `Mode::Perks`
  when Perk Points are unspent, otherwise `Mode::Playing`. When it was opened
  from the menu, it returns to where it came from.
- **Menu entry:** from the Perks screen and the character sheet, the key is
  settled in the plan against the uppercase-action rule. Refused with
  `App::refuse` when no points are banked.

### Level Up report

With growth deferred, the page's after-column shows no HP or ATK change, so
it shows the stat points and Perk Points earned instead. The duel projections
still read current stats. The Points screen already shows live before→after
values as points are spent.

## GUI

`render/creation.rs`'s Points drawing is split out so it takes a
`StatAllocation` view. Creation and `Mode::AllocateStats` both call it. The
footer text comes from the allocation's purpose: creation says "spend all to
continue", level-up says "Esc keeps the rest". Drawing goes only through
`Painter` (drawing seam).

## Balance and measurement

- **`balance_sim`:** the player path builds stats from the canonical spend via
  `stat_units`, replacing `stats_after_levels(PLAYER_BASE_STATS, …,
  BASELINE_GROWTH_MULTIPLIER)`. A test asserts it equals the old growth at
  every level to 1..=cap, which is the "curves did not move" proof.
  Companions keep `stats_after_levels`.
- **Def measurement (ship gate):** run an arena batch comparing a pure-Def
  spend with the canonical spend at levels 10, 15 and 20. Record it in
  `docs/measurements/` per its README. If pure Def dominates, stop and bring
  the numbers to the user. The price is not changed without them.

## Testing

- Engine: `add_xp` with `Points` grows no stats, heals and reports
  `stat_points`, and `Auto` is unchanged. `award_player_xp` banks 6 per
  level. `spend_stat_points` applies through `stat_units`, writes
  `BoughtStats`, refuses an overspend with nothing written, and raises `hp`
  with Integrity. Save→load keeps `StatPoints`. `attention` flags unspent
  points.
- App-core: LevelUp → AllocateStats → Perks → Playing. `Esc` keeps points.
  Creation's leave refusal still fires and level-up's does not. Menu entry is
  refused at 0 points. Creation still commits the same character.
- Mutation-check each new test (remove the fix and see it fail).
- Gates: `cargo test --workspace`, `balance_sim`, clippy with
  `--all-targets`. A green suite is not play, so a `--screenshot` of the
  Points screen at level-up is part of done.

## Release

Minor bump (a new mechanic, and saves still load), with a `CHANGELOG.md`
entry. Update `assets/`' README only if a schema changes; none is planned.
