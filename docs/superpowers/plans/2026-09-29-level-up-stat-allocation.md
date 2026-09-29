# Level-up stat allocation — plan

Spec: `docs/superpowers/specs/2026-09-29-level-up-stat-allocation-design.md`.
Branch: `level-up-stat-allocation`. TDD, a commit per green step, and every
new test is mutation-checked. There is no `SAVE_FORMAT_VERSION` bump: the
one new save field is `#[serde(default)]`.

There are three phases. Phases 1 and 2 are one sonnet dispatch each, and
dispatches must forbid push. Phase 3 is done inline by the controller. Each
phase ends green on `cargo test -p <crate(s) touched>`, `cargo clippy
--workspace --all-targets` and `cargo fmt`. After phase 2, run `cargo test
--workspace` once and `cargo test -p feral-processes-engine balance_sim`.
The final review is opus, over the whole branch.

## Anchors (verified 2026-09-29; engine paths are under `crates/engine/src`)

| What | Where |
|---|---|
| `HP_PER_LEVEL` 24, `ATK_PER_LEVEL` 2, `BASELINE_GROWTH_MULTIPLIER` | `tuning.rs:196`, `:197`, `:203` |
| `CREATION_STAT_POINTS` 20, `CREATION_COST_*` (all 1), `CREATION_GAIN_INTEGRITY` 6 | `tuning.rs:64`, `:68-93`, `:98` |
| `DECOMPILER_SKILL_PER_LEVEL`; `PLAYER_BASE_STATS` | `tuning.rs:445`; `:35` |
| `MainStat {Atk, Def, Integrity, Decompiler}` + `all()` | `achievements.rs:92` |
| `LevelGain`; `scaled_growth`; `stats_after_levels`; `add_xp` | `progression.rs:27`, `:14`, `:213`, `:255` |
| `add_xp` callers: player, companion, workers, arena | `game/combat_rewards.rs:1015`, `:1157`; `systems.rs:1506`; `arena/mod.rs:98` |
| `award_player_xp` (tally, `PendingLevelUp`, Perk Points, Decompiler) | `game/combat_rewards.rs:997` |
| `XpTally {xp, gain, perk_points, decompiler}` + `absorb` | `resources.rs:1484` |
| `apply_creation_stats` (writes `Stats`/`Decompiler` directly) | `game/creation.rs:223` |
| `BoughtStats` (perk/talent receipt, refunded by respec) | `components.rs:3045`; `game/respec.rs:170`; `game/kit.rs:88` |
| `attention`; its Perk Points row (copy this one) | `game/inspection.rs:1733`, `:1841` |
| `PlayerSave`; the player's load in `lifecycle.rs` near `BoughtStats` | `save.rs:18`; `game/lifecycle.rs:417`, `:2756` |
| `LevelUpReport`; `take_level_up_report` | `views.rs:3587`; `game/level_up.rs:101` |
| balance_sim player growth sites (the one at `:285` is level 1 and does not change) | `balance_sim.rs:472`, `:761` |
| Power-reference derivation test (keep it, it still holds) | `tuning.rs:5894` |
| app-core `stat_cost`, `stat_value` (the copy), roll spread, `spend_on_row` | `app-core/src/app/creation.rs:114`, `:127`, `:172`, `:889` |
| Points rows, Points leave refusal, Points key handler | `app/creation.rs:345`, `:569`, `:532` |
| `CreationRow::Stat`; `Mode` (**`Copy`, unit variants only**) | `app-core/src/lib.rs:1396`, `:1436` |
| `Mode::LevelUp` handler; where it opens | `app/level_up.rs:16`; `app/lifecycle.rs:520` |
| `handle_perks_key` (`X` = respec; lowercase letters are rows) | `app/progression.rs:9` |
| gui creation Stat row (bar width from `CREATION_STAT_POINTS`); level-up page | `gui/src/render/creation.rs:212`; `gui/src/render/level_up.rs` |

## Decisions the spec left open, or got wrong against the code

1. **Level-up spends do not write `BoughtStats`. This contradicts the spec
   and needs the user's OK.** `BoughtStats` is the perk/talent receipt.
   `respec_perks` subtracts it, so a perk respec would take level-up stats
   away without giving their points back. `emulated_base` adds it on top of
   an emulation, so emulated players would suddenly keep level ATK/Def they
   don't keep today. Spends go straight into `Stats`/`Decompiler`, as
   `apply_creation_stats` does. Those stats are baked, but a stat respec is
   out of scope, and `kit.rs:88` already argues the same for creation.
2. **Target type:** `pub enum StatOwner { Player }`, placed in
   `progression.rs` and re-exported. `Game` resolves it to an `Entity`.
   Companions get a `Companion(…)` variant later.
3. **`stat_units` returns `StatDelta { max_hp, atk, mitigation, decompiler:
   i32 }`**, which is new in `progression.rs`. Integrity raises `hp` by the
   same `max_hp` amount, and the caller applies that. Add `const _: () =
   assert!(STAT_POINTS_PER_LEVEL as i32 == HP_PER_LEVEL /
   CREATION_GAIN_INTEGRITY as i32 + ATK_PER_LEVEL);` next to the constant.
4. **`Growth::Points` gives nothing, and `stat_points` counts at 6 per
   level.** `LevelGain.max_hp`/`atk` are 0, and `stat_points =
   STAT_POINTS_PER_LEVEL × levels`. `absorb` sums it. `XpTally` needs no new
   field, because it reads `gain.stat_points`. Every log line that prints
   the tally says "+N stat points".
5. **The canonical spend:** `progression::canonical_spend(levels) ->
   StatDelta` = `stat_units(Integrity, 4×levels) + stat_units(Atk,
   2×levels)`. It derives the 4 and the 2 from the constants rather than
   typing them in. balance_sim sites `:472` and `:761` use
   `PLAYER_BASE_STATS + canonical_spend(level-1)`. The test checks that this
   equals `stats_after_levels(PLAYER_BASE_STATS, n, BASELINE_…)` for n in
   0..=cap.
6. **Mode shape:** `Mode` is `Copy` and has unit variants only, so the spec's
   `Mode::AllocateStats(StatAllocation)` can't be written. It becomes
   `Mode::AllocateStats` plus `App.stat_allocation:
   Option<StatAllocation>`. Watch the all-modes list traps
   (`all-modes-does-not-fail-to-compile`, `all-modes-length-is-a-semantic-merge-conflict`).
7. **One owner for the spend state:** creation keeps
   `CharacterChoice.stats` as its store, because it is engine API and
   `[R]`'s roll writes it. The shared part is `StatAllocation { owner:
   AllocationFor, pool: u32, base: StatLine }` (`AllocationFor { Creation,
   Owned(StatOwner) }`). Its methods take `spent: &[u32; 4]` or `&mut
   [u32; 4]`: `rows`, `spend_on_row`, `remaining`. Creation passes
   `&mut creation_choice.stats`, and `AllocateStats` passes its own
   `spent` field. `stat_value` becomes `base + stat_units(…)`, a call and
   not a copy. Run the `design-patterns` dialog before coding this.
8. **Menu entry:** the only way in is uppercase `S` on the Perks screen,
   because no character sheet mode exists. When no points are banked it is
   refused with `App::refuse`. The attention row uses the Perk Points
   row's key `'p'`, which reaches Perks, and the row text names `S`.
9. **Returning:** `AllocateStats` opened from the level-up flow goes to
   `Perks` if Perk Points are unspent and to `Playing` otherwise. Opened
   from Perks, it goes back to `Perks`.

## Phase 1 — engine (sonnet)

Files: `tuning.rs`, `progression.rs`, `components.rs`,
`game/combat_rewards.rs`, `systems.rs`, `arena/mod.rs`, `game/creation.rs`,
new `game/stat_points.rs` (or an existing `game/` file if one fits better),
`save.rs`, `game/lifecycle.rs`, `game/inspection.rs`, `views.rs`
(`AttentionKind::StatPoints`, `LevelUpReport.stat_points`),
`game/level_up.rs`, `balance_sim.rs`, `lib.rs` re-exports. Tests go in
`progression.rs`, `tests/{level_up,combat_rewards,save…}.rs`.

1. `STAT_POINTS_PER_LEVEL` + the assert, then `StatDelta`, `stat_units` and
   `canonical_spend`. Test that each stat's unit worth matches the creation
   constants.
2. `apply_creation_stats` goes through `stat_units`. The existing creation
   tests pass without edits.
3. `Growth` replaces the `growth_multiplier` parameter. All four callers
   pass `Auto`, and the progression tests are updated mechanically. Tests:
   `Points` grows nothing, heals, and reports `stat_points`. `Auto` is
   unchanged, and the existing tests show it.
4. `StatPoints(u32)` component, spawned on the player at 0.
   `award_player_xp` passes `Points` and banks the points. Test: one level
   banks 6 and leaves `max_hp`/`atk` alone. Decompiler still +2.
5. `Game::spend_stat_points(StatOwner, &[(MainStat, u32)]) ->
   Result<StatDelta, SpendError>`. Validate first, then write
   `Stats`/`Decompiler`; `BoughtStats` is not touched (decision 1). Tests:
   the write applies, an overspend writes nothing, Integrity raises `hp`,
   and `BoughtStats` stays unchanged.
6. `PlayerSave.stat_points` with `#[serde(default)]`, saved and loaded.
   Test with a real save→load, not only a RON round-trip.
7. `attention` gets a `StatPoints` row. `LevelUpReport` gains
   `stat_points` (from the tally or `PendingLevelUp`, whichever holds the
   perk figure the report shows today).
8. balance_sim uses `canonical_spend` (decision 5), plus the equality test.
   `balance_sim` must stay green with no curve edits.

## Phase 2 — app-core + gui (sonnet)

Files: `app-core/src/lib.rs`, `app/creation.rs`, new
`app/stat_allocation.rs`, `app/level_up.rs`, `app/progression.rs`,
`app/input.rs`, the all-modes list, `gui/src/render/creation.rs`,
`gui/src/render/level_up.rs`, `gui/src/render/mod.rs` dispatch. Tests go in
app-core's tests.

1. `stat_value` becomes a call to `stat_units` (decision 7). Test that
   creation still commits the same character from a fixed spread.
2. Extract `StatAllocation` (decision 7). Creation's rows, refusal and roll
   behave exactly as before, and the existing creation tests pass without
   edits.
3. `Mode::AllocateStats` + `stat_allocation`. `Enter` commits through
   `spend_stat_points`, and `Esc` leaves without spending. Leaving is always
   allowed. Tests: LevelUp `Enter` → AllocateStats → Perks (when Perk
   Points are unspent) → Playing; `Esc` keeps the points; creation's leave
   refusal still fires and the Owned leave refusal doesn't.
4. Perks screen `S` (decision 8), refused at 0 points. Test both cases.
5. gui: the Points drawing takes a `StatAllocation` view. The bar width
   uses `pool` and not `CREATION_STAT_POINTS`. The footer depends on
   `AllocationFor`. The level-up page's after-column shows stat points and
   Perk Points. Draw only through `Painter`.

## Phase 3 — measure, see, ship (inline)

1. **Def gate:** read `docs/measurements/README.md` first. Run an arena
   batch at levels 10, 15 and 20, pure Def against the canonical spend.
   Check how `arena/scenario.rs` `CharacterSpec` sets player stats before
   writing the scenario. Record the result in `docs/measurements/`. **If
   pure Def dominates, stop and bring the numbers to the user.**
2. **Screenshot:** start from a `dev-saves/` template near a level-up (or
   `savetool capture` one), then `--keys` into AllocateStats and take a
   `--screenshot`. Read the PNG. Needs `DISPLAY`. Without one, say so.
3. Update the `CHANGELOG.md` unreleased section. The minor bump happens at
   landing, not on the branch.
