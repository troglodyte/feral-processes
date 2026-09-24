# Level-up Summary Implementation Plan

> **For agentic workers:** use superpowers:executing-plans or superpowers:subagent-driven-development. Steps use `- [ ]`.

**Spec:** `docs/superpowers/specs/2026-09-24-level-up-summary-design.md`. Read it first. Where it disagrees with the corrections below, the corrections win, because they were checked against the source.

**Shape:** three phases, one per crate. Each ends green and committed, so a session can `/clear` between phases and restart from this file. The phases run in order, because each one consumes the previous one's interface.

## Corrections and locations (checked against the source)

1. **The player's swing is `duel_damage`'s** (`game/inspection.rs:~2440`). The attacker is `combatant_profile(player, Swing { range: natural_range_of(player), ..Default::default() })` and the defender is `combatant_profile(player, Swing::default())`. The two differ only in `range`, so **the snapshot stores one `Combatant`**, the attacker profile. Its `evasion` field serves the defender side. "Swings" counts single swings, not rounds, so `attacks_for` (a Striker's second swing) is deliberately not applied. Say so in the doc comment.
2. **Mitigation is `Game::effective_mitigation(player)`** (`game/combat_round.rs:1916`), not `Stats::mitigation`. That door is the seam's.
3. **The foe's range must be a call.** `natural_range_of`'s `Kit::Innate` arm (`game/combat_damage.rs:336`) is `def.moves.first().map_or(PLAYER_UNARMED_DAMAGE, |mv| mv.range())`. Extract it as `SpeciesDef::natural_range(&self) -> DamageRange` in `species.rs`. Both that arm and the foe build call it.
4. **Foe `Combatant`:** `accuracy_of(median.base_speed, zone, 0)`, `evasion_of(median.base_speed, zone, 0)`, `atk: wild_stats_at_zone(median, zone).atk` and `range: median.natural_range()`. Foe EHP is `effective_hp(wild.max_hp, wild.mitigation)`. `balance_sim::wild_stats_at_zone` (`balance_sim.rs:72`) is private, so make it `pub(crate)`. `worn_detail` (`game/catalog.rs:554`) builds `NominalHostile` from the same median, so build one `fn typical_foe(&self) -> (Combatant, f64 /*ehp*/)` in the new module and leave `worn_detail` alone.
5. **The snapshot is taken in `award_player_xp`** (`game/combat_rewards.rs:997`). Take it *before* the `add_xp` block, and **insert it only if `gain.levels > 0` and the resource is `None`**. Overflow at the cap converts to Perk Points without a level, and that shows no page. Both XP callers (`combat_round.rs:1157`, `combat_rewards.rs:1285`) and contracts reach this one function, so a level earned outside a fight is covered with no new code.
6. **The Perk Point and Decompiler deltas are `current − snapshot`** (`Perks::points`, `Decompiler::skill`). They are not recomputed from `PERK_POINTS_PER_LEVEL`. Overflow points earned in the same fight are then counted, which is correct.
7. **The stat labels are `StatRow`'s own ("Max HP", "ATK")** (`progression.rs:65`). The spec's mock-up says "Max Integrity", but the spec also says the rows are the log's format, so keep `StatRow`'s labels. Build the rows with `StatRow::new(label, before, after)`, then drop any row where `before == after`.
8. **App-core hook:** put the check **inside `show_next_notification`** (`app/lifecycle.rs:497`), ahead of `take_notification`. It then inherits the `Mode::Playing` gate and the arena guard, and stays the one writer of its pending field. The field is `App::pending_level_up: Option<LevelUpReport>`, next to `pending_notification` (`lib.rs:2459`), and it is initialised in `lifecycle.rs:71`. On `Esc`, set `Playing`, then call `show_next_notification()`. That lets a queued `LevelCapReached` through, which is `handle_notification_key`'s pattern (`lifecycle.rs:515`). `handle_key`'s tail already calls `after_tick` (`app/input.rs:345`), so leaving the results screen reaches the hook.
9. **`P`** closes the page, then sets `Mode::Perks`. `handle_perks_key`'s `Esc` is `close_screen()` (`app/progression.rs:10`). With `menu_origin` `None`, confirm that it lands in `Playing`, and assert this.
10. **Every place `Mode::Notification` is listed gets `Mode::LevelUp` beside it:** `is_battle`'s `=> false` arm (`app-core/src/lib.rs:2236`), `input.rs:258` dispatch, `needs_status_banner` (`gui/src/render/mod.rs:566`), the draw match (`mod.rs:638`, drawn over `draw_playing_base`), and `ALL_MODES` (`mod.rs:1530`, which goes **118 → 119**). Run `rg -n 'Mode::Notification' crates/*/src` and treat the result as the census.

## Global constraints

- `PendingLevelUp` is a `Resource`, **not saved**, and is inserted at both `Game::new` and `Game::load`. Adding a resource can shift query iteration order (memory: `new-resource-shifts-query-iteration-order`). If seeded tests move, fix each one so it no longer depends on luck. Never change the seed.
- Every figure is a call to `battle::hit_chance`, `expected_damage` or `effective_hp`, and nothing is inlined.
- Gates before every commit: `cargo fmt`, then `cargo clippy --workspace --all-targets`, then the phase's tests. Run `cargo test --workspace` at the end of each phase.
- Deleted-fix check: remove the `is_none()` guard and the `levels > 0` guard, and watch their tests fail.
- No version bump and no CHANGELOG section on the branch. Never push.

## File map

| File | Phase | Change |
|---|---|---|
| `engine/src/species.rs` | 1 | `SpeciesDef::natural_range` |
| `engine/src/game/combat_damage.rs` | 1 | `natural_range_of` calls it |
| `engine/src/balance_sim.rs` | 1 | `wild_stats_at_zone` becomes `pub(crate)` |
| `engine/src/resources.rs` | 1 | `LevelSnapshot`, `PendingLevelUp` |
| `engine/src/views.rs` | 1 | `LevelUpReport`, `DuelFigures { before, after }` |
| `engine/src/game/level_up.rs` (new) | 1 | `snapshot_player`, `typical_foe`, `take_level_up_report` |
| `engine/src/game/combat_rewards.rs` | 1 | snapshot write in `award_player_xp` |
| `engine/src/game/lifecycle.rs` | 1 | insert the resource at new and load |
| `engine/src/tests/level_up.rs` (new) | 1 | engine tests |
| `app-core/src/lib.rs`, `app/lifecycle.rs`, `app/input.rs`, `app/level_up.rs` (new) | 2 | mode, field, hook, keys |
| `app-core/src/tests/level_up.rs` (new) | 2 | app-core tests |
| `gui/src/render/level_up.rs` (new), `render/mod.rs` | 3 | page, dispatch, banner, `ALL_MODES` |

---

## Phase 1 — Engine

- [x] **1.1** Extract `SpeciesDef::natural_range` (correction 3). The existing suite is the regression gate. Commit.
- [x] **1.2** Types. `LevelSnapshot { level, max_hp, mitigation, perk_points, decompiler, combatant: battle::Combatant }` and `PendingLevelUp(Option<LevelSnapshot>)` (`#[derive(Resource, Default)]`). `LevelUpReport { from_level, to_level, zone, stats: Vec<StatRow>, hit_chance: (f64, f64), per_swing: (f64, f64), swings_to_win: (u32, u32), swings_to_down_you: (u32, u32), perk_points_gained, perk_points_unspent, decompiler_gained }`. Pairs are `(before, after)`. Keep them as tuples unless the gui wants a struct.
- [x] **1.3** Tests first, in `tests/level_up.rs` (fixtures in `tests/support.rs`). There are the spec's five engine tests, plus: (a) an overflow award at the cap leaves no report; (b) a level from a contract reward, outside a fight, produces a report. For the figure test, compute the expected values from `battle::*` calls on the report's own `Combatant`s. Expose them with `#[cfg(test)]` accessors if they are needed, and do not copy a formula. `swings_*` use `f64::ceil`, cast to `u32`. Guard against a zero `expected_damage`: `u32::MAX` would render badly, so clamp and document it, or show `—`. Decide in 1.4 and test whichever you pick.
- [x] **1.4** Implement `game/level_up.rs` and the write site (corrections 1, 2, 4, 5, 6). Run the targeted tests, then the whole suite. Commit.

## Phase 2 — App-core

- [x] **2.1** Tests first, in `tests/level_up.rs`: the spec's three tests, plus `Esc` → `Playing` from Perks after `P` (correction 9), plus an arena session never opens the page. Drive the level with a real fight or with a contract award. Remember `app-core-battles-are-always-one-group-one-slot`.
- [x] **2.2** Add `Mode::LevelUp`, the field, the hook (correction 8), `handle_level_up_key` (only `Esc` and uppercase `P` act; everything else is ignored), and the correction 10 app-core sites. Commit.

## Phase 3 — Gui and docs

- [ ] **3.1** `render/level_up.rs`, through `Painter` only, with fixed rows and no scroll. The layout is the spec's mock-up: three section headers and one row per figure. Format percentages as whole numbers and per-swing as one decimal. Mirror `notify.rs`'s panel and `draw_playing_base` underlay.
- [ ] **3.2** The census test, following `the_tallest_shipped_notification_fits_its_screen` (`notify.rs:256`). At `ui_metrics(720.0)`, the widest report fits the panel in both height and width: two stat rows, 4-digit figures, level `99 → 99`, the largest zone number and `u32`-scale swing counts. Build the report by hand, since `LevelUpReport` is a plain view. Verify the test by mutation: shrink the panel and watch it fail.
- [ ] **3.3** Add the correction 10 gui sites and bump `ALL_MODES` to 119. Run `rg -n 'Mode::Notification' crates/gui/src` and check that every hit has a `LevelUp` twin.
- [ ] **3.4** Docs. Draft a CHANGELOG entry under the existing convention, without a section header. If the `assets/help/` page on levelling names the level-up log lines, add a sentence about the page there. `docs/manual.md` and the root README are carved out, so leave them alone. Add a seam if one emerged. The likely candidate is "the level-up page's before column is a stored `Combatant`, never a re-derivation at the old level", written three places per the `seams` skill.
- [ ] **3.5** Run `cargo test --workspace`. Commit. Tell the user it is unplaytested; agents have no display.
