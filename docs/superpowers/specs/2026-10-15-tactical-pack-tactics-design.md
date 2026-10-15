# Tactical pack tactics

On the tactical battle map every hostile decides alone: it picks an action
by its species personality (`assets/behaviours/`), then a cell by
`scored_cells`, with no notion of a packmate. Nothing stays together,
nothing holds a line, nothing waits in cover for the player to come to it.
This change gives the hostile side of a fight one **pack tactic**, rolled
when the fight opens, that adds pack-aware terms on top of each body's
personality.

The 2026-10-12 personalities spec listed "teamwork" and "fall back to
cover" as later sub-projects. This spec covers the group-movement and
focus-fire part of those; healing and buffing packmates, guarding and
disengaging stay open.

## Decisions

| Question | Decision |
|---|---|
| Mode | Tactical map only. Group battles and `combat_policy.rs` are untouched. |
| Scope | One tactic per fight for the whole hostile side; per-body personality still applies under it. |
| Tactics | Phase 1: `Rush`, `Together`, `Hold`. Phase 2: `Formation`, `Flank`. |
| Who picks | Rolled per encounter, with odds keyed by the pack's personality. |
| Data | **Internal.** A Rust enum and `tuning.rs` constants — no new asset kind, no schema field. Requested by the user; the moddability rule does not apply to this feature. |
| Randomness | The roll is derived from the battle's `BattleSpec`, never from `GameRng`. The one-draw-per-AI-turn rule is unchanged. |
| Mid-fight change | Two transitions only: `Hold` → `Rush` when triggered, and `Formation` → `Rush` when the front rank is gone. |
| Visibility | One battle log line at the start, one on each transition. |
| Sieges | Always `Rush`; the besieger hook keeps its own logic. |
| Save format | Unchanged. Ordinary battles are never saved, and sieges do not roll. |
| Rejected | A commander layer that issues per-body orders (overrides personalities, a second decision layer); pack terms inside the personality profiles (no per-encounter variety, no clean hold trigger); an `assets/tactics/` data kind (user: keep it internal). |

## The tactic — `tactical/pack.rs` (new)

```rust
pub enum PackTactic { Rush, Together, Hold, Formation, Flank }
```

`TacticalBattle` gains `pack: PackState`, holding the current tactic plus
what `Hold` needs to judge its trigger (below). It is not serialised;
`TacticalBattle` never is.

**The pack** is every body `tactical_sides` puts on the hostile side at
that moment. A tampered body that changed sides leaves it, a hallucinating
one stays in. A squad is one body. Every pack term reads positions through
`TacticalBattle::cells_of` and gaps through `reach::gap`, so footprints
count.

### The roll

`pack::roll(spec, pack) -> PackTactic`, called once where the tactical
battle is seated (`start_battle`'s tactical arm and `arena::stage`).

- **Seed:** a hash of `BattleSpec`'s `world_seed`, `site` and `tick`,
  salted so it is independent of the board drawn from the same spec. The
  roll makes no `GameRng` draw, so the game's random stream does not move
  and no seeded test shifts because of it.
- **Odds:** `tuning::PACK_TACTIC_ODDS`, keyed by behaviour id. One row per
  shipped profile (`default`, `brute`, `hunter`, `skirmisher`, `cautious`)
  gives integer weights per tactic. A pack's profile is
  `Game::behaviour_of` its toughest body (highest max HP; ties go to the
  first in initiative order). An unlisted id uses the `default` row.
  Starting leanings:
  - brute → Rush, Together
  - hunter → Flank, Rush
  - skirmisher → Hold, Flank
  - cautious → Hold, Formation
  - default → an even spread
- **Eligibility** is checked before the draw, so an ineligible tactic gets
  weight zero:
  - `Together`, `Formation` and `Flank` need at least two bodies.
  - `Formation` also needs at least one front-rank body and one back-rank
    body (see Formation).
  - A siege pack is `Rush` without a roll.
- **Arena override:** the scenario field `tactic: Option<PackTactic>` forces
  the result, the way `behaviour` forces a profile. An unknown name must
  fail to parse, never fall back silently (the
  `arena-scenario-fields-fail-silently` trap).

### How a tactic enters the decision

Everything goes through the planner the turn already runs, so
`tactical_forecast` follows without its own copy (seam: *a profiled
hostile's forecast is a call into the planner its turn runs*).

- **Cell terms:** `scored_cells` gets the tactic's extra per-cell terms
  from one `pack::cell_terms(game, body, cell) -> f32`. Each tactic's
  weights are `tuning.rs` constants, separate from the profile's
  `CellWeights`.
- **Cell merit:** the tactic's terms join `cell_merit`. A body still stays
  put unless some cell beats its own; the tactic changes what "beats"
  means, not the rule.
- **Closing scale:** a per-tactic multiplier on the closing term (1.0 for
  `Rush`). `Hold` and the `Formation` back rank turn it down.
- **Intent bonus:** an additive bonus in `choose_hostile_intent`, outside
  the profile weights, so no `assets/behaviours/*.ron` changes. Only
  `Flank` uses it (focus).

`Rush` has no terms and a closing scale of 1.0, so a `Rush` fight decides
exactly as today. Any test that asserts the current behaviour holds under
`Rush`. Fixtures that build a battle by hand and do not go through the
roll default to `Rush`.

## Phase 1

### Together

A slack-rope cohesion term. For a body at a candidate cell, `d` is the
step distance to the centroid of its packmates (excluding itself). The term
is `-TACTICAL_COHESION_WEIGHT × max(0, d − TACTICAL_COHESION_SLACK)`.
Start with a slack of 2.

Closing is unchanged. The pull comes from the cohesion term: a fast body
that would outrun the pack finds its forward cells cost more than they
gain, so it waits. The pack arrives roughly together.

### Hold

The pack takes cover and waits.

- **While holding:** the closing scale is 0, and the cover term gets a
  bonus (`TACTICAL_HOLD_COVER_BONUS`). Each body moves to cover within its
  move allowance if there is some, then stays put, since staying put is the
  default once it has the best cell.
- **Triggers.** These are checked at the start of every hostile turn,
  before the intent is chosen. The first one to fire ends the hold for the
  whole pack:
  1. **Sighted:** a non-pack body is within `TACTICAL_HOLD_ALERT_RANGE` of
     a pack member and in its `line_of_sight`. Start the range at 6, the
     thrown range.
  2. **Hit:** a pack member's HP is below what it was when the hold
     started, or a member is gone. `PackState` keeps a snapshot of the
     members' HP at the start. This needs no hook in `apply_damage`, which
     stays the only damage path.
  3. **Timeout:** `TacticalBattle::round` passes the start round plus
     `TACTICAL_HOLD_ROUNDS` (start at 6). Without this, a player who also
     waits stalls the fight.
- **On trigger:** the tactic becomes `Rush` and one log line is written.
  The body whose turn it is plans under `Rush` that same turn.

## Phase 2

### Formation

- **Ranks** are derived each turn and never stored:
  - **Back rank:** a body that can attack beyond melee. Its
    `Game::swing_range` max is above `TACTICAL_MELEE_RANGE`, or it has a
    ready routine whose range is.
  - **Front rank:** every other body.
- **Facing:** from the pack's centroid toward the nearest non-pack body,
  snapped to one of the 8 compass directions.
- **Slots:**
  - The front rank stands in a line across the facing, centred on the
    front rank's centroid. Bodies are ordered by max HP, toughest in the
    middle and alternating outward. A squad takes its footprint's width.
  - The back rank's slots mirror the front's, `TACTICAL_FORMATION_DEPTH`
    (start at 2) cells behind the line.
  - Ties go to initiative order. The whole assignment is a pure function
    of positions and kit.
- **Terms:**
  - Every body gets `-TACTICAL_FORMATION_WEIGHT × distance to its slot`.
  - A front-rank body keeps closing at 1.0, so the line advances.
  - A back-rank body's closing scale is `TACTICAL_FORMATION_BACK_CLOSING`
    (below 1). It stays behind the line and fires once its target is
    within its range band.
- **Breaking:**
  - A front-rank body next to a non-pack body (by `reach::gap`) drops its
    slot term and fights as `Rush`.
  - When no front-rank body is left, the tactic becomes `Rush` and one log
    line is written.

### Flank (with focus)

- **Focus (intent):** for a swing or routine candidate, add
  `TACTICAL_FOCUS_BONUS × number of other pack bodies within melee gap of
  the target`, capped at `TACTICAL_FOCUS_CAP`. Argmax and sorted
  candidates are unchanged.
- **Flank (cell):** for a body that has chosen a target, its candidate
  cell has a bearing from the target, snapped to one of 8 sectors.
  - Subtract `TACTICAL_FLANK_CROWD` for each packmate already in that
    sector within melee gap.
  - Add `TACTICAL_FLANK_PINCER` if a packmate holds the opposite sector.
  - The existing `walk_risk` provocation price still applies, so a flanker
    does not walk through the player's reach to get around them.

## Messages

All lines go to the battle log through the existing logging path
(`MessageKind::Info`), in the setting's vocabulary: no fantasy words, and
"programs", never "monsters". The fight-start line names the pack by its
toughest body's species. Draft lines:

- Rush: no line. It is today's behaviour, so it needs no announcement.
- Together: "The pack advances together."
- Hold: "The pack spreads out behind cover, waiting."
- Formation: "The pack forms a line, ranged programs behind."
- Flank: "The pack fans out to surround you."
- Hold → Rush: "The pack breaks cover!"
- Formation → Rush: "The line breaks!"

## Testing

Engine tests go in `tests/tactical_pack.rs` (new). Each one is a small
hand-built board using `tests/support.rs` fixtures, with the tactic set
directly.

- **Roll:**
  - The same `BattleSpec` and pack give the same tactic.
  - Rolling makes no `GameRng` draw: the stream position is the same
    before and after.
  - Ineligible tactics are never rolled (one body never gets `Together`).
  - A siege is always `Rush`.
  - An arena `tactic:` override wins.
- **Rush:** an existing behaviour test, such as
  `a_brute_walks_past_cover_and_a_skirmisher_walks_to_it`, passes under an
  explicit `Rush` unchanged.
- **Together:** a fast body that would outrun a slow packmate stops within
  the slack.
- **Hold:**
  - A holding body walks to cover and then does not move while the player
    is out of sight.
  - Hold breaks when the player comes into sight.
  - Hold breaks when a member is hit, and when a member is gone.
  - Hold breaks on the timeout round.
  - The body that triggers it acts under `Rush` that turn.
- **Formation:**
  - A ranged body stays behind a melee packmate while both advance.
  - A front-rank body next to the player leaves its slot.
  - The pack switches to `Rush` when the front rank is gone.
- **Flank:**
  - A second attacker takes the sector opposite the first.
  - Focus picks the target a packmate is already on over an otherwise
    preferred one.
- **Forecast:** under each tactic,
  `every_shipped_profile_forecasts_the_action_its_turn_takes` still holds.
  Extend it to loop over tactics.
- **Mutation check:** each tactic test fails with its term removed.

## Measurement

These are arena runs per tactic, compared against `Rush` on the same build
(`arena-numbers-compare-within-one-build-only`):

- `dev-arenas/tactical-full-group.ron`, plus a new mixed melee-and-ranged
  scenario, so `Formation` is eligible.
- Arena `party_turns: AutoPlay`.

The bar: each tactic's party win rate is within 10 points of `Rush`'s, and
its party HP lost is within 20%. A tactic outside the bar gets its weights
adjusted before landing. The result goes in one note in
`docs/measurements/`, per that directory's README.

`balance_sim` exercises the group model only, so it should not move. Check
it once.

## Phasing

The two phases are landed and released separately:

1. `pack.rs`, `PackState`, the roll, the arena override, the messages,
   `Together` and `Hold`, with their tests and measurement.
2. `Formation` and `Flank` with focus, their odds rows, tests and
   measurement.

Phase 2 changes no interface that phase 1 introduces; it adds enum arms,
terms and rows.

## Files

- **engine:**
  - `tactical/pack.rs` (new)
  - `tactical/mod.rs` (`PackState` on `TacticalBattle`)
  - `tactical/ai.rs` (`scored_cells`, `cell_merit`, hold triggers at the
    start of a turn)
  - `tactical/policy.rs` (intent bonus)
  - `tuning.rs` (odds table and constants)
  - the tactical seat in `start_battle`
  - `arena/scenario.rs` and `arena/setup.rs` (the `tactic` override)
  - `tests/tactical_pack.rs`
- **launcher:** none expected. The arena bin reads the scenario through
  the engine.
- **docs:**
  - `CHANGELOG.md`
  - a measurement note
  - `seam:` entries, a skill line and a rules line for the new rule, *the
    pack tactic is rolled from the `BattleSpec` and never draws `GameRng`*,
    via the `seams` skill.
