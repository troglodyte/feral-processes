# Spend preview — plan

Spec: `docs/superpowers/archive/specs/2026-09-29-spend-preview-design.md`. Branch:
`worktree-level-up-summary`. TDD, a commit per green step, every new test
mutation-checked with the fix committed. No save-format change. This is a
**minor** release (`CHANGELOG.md` preamble).

## Shape

The plan has three phases. Phases 1 and 2 are each one sonnet dispatch, run
serially because phase 2 consumes phase 1's API. Every dispatch **forbids
push** and stages explicit paths. Phase 3 is done inline by the controller.

Each phase ends with `cargo check --workspace`, `cargo test -p <crates
touched>`, `cargo clippy --workspace --all-targets` and `cargo fmt`, all green.
Per-task review gates are off. The final review is opus, over the whole branch.
`balance_sim` is expected untouched; phase 3 runs it once as a check.

## Anchors (verified 2026-09-29; paths under `crates/`)

| What | Where |
|---|---|
| `swings_to`, `snapshot_player`, `typical_foe` (returns no zone), `take_level_up_report` | `engine/src/game/level_up.rs:28`, `:52`, `~:81`, `:104` |
| `LevelUpReport` (derives `Clone, Debug, PartialEq`) | `engine/src/views.rs:3598` |
| `LevelSnapshot` (`Copy`, carries `mitigation`) | `engine/src/resources.rs:1023` |
| `spend_stat_points`; the `banked - total` write; `recompute_derived` | `engine/src/game/derived.rs:19`, `~:63`, `:132` |
| `unlock_perk` (callers: `engine/src/game/creation.rs:202`, `app-core/src/app/progression.rs:40`) | `engine/src/game/unlocks.rs:171` |
| `apply_equipment_delta`; `emulated_base` | `engine/src/game/crafting.rs:692`; `engine/src/game/kit.rs:97` |
| `Game::perk_defs` → `Vec<PerkDef>` (perk is `def.id`) | `engine/src/game/catalog.rs:1083` |
| `StatRow` | `engine/src/progression.rs:99` |
| Components `Derived`, `StatPoints`, `BoughtStats`, `Perks`, `Decompiler`, `PowerReserve` | `engine/src/components.rs:1949`, `:1970`, `:3110`, `:3082`, `:395`, `:217` |
| `Game::save(&mut self, &Path)` (the only save route); `PlayerSave` | `engine/src/game/lifecycle.rs:2942`; `engine/src/save.rs:18` |
| Tests reading the moved duel fields | `engine/src/tests/level_up.rs:914-932` |
| `App` struct; `pending_level_up`; `menu_selected`; `Mode` | `app-core/src/lib.rs:2497`, `:2608`, `:3003`, `:1442` |
| `Mode::is_battle` exhaustive match (`=> false` arm) | `app-core/src/lib.rs:2211-2381` |
| Key dispatch | `app-core/src/app/input.rs:265-312` |
| `open_stat_allocation`; `handle_allocate_stats_key`; `leave_allocation` | `app-core/src/app/stat_allocation.rs:259`, `~:295`, `:341` |
| `handle_perks_key`; respec confirm (`Mode::Perks` at `:53`, `:55`) | `app-core/src/app/progression.rs:9`, `:47-56` |
| `LevelUp` `P` key (`Mode::Perks`) | `app-core/src/app/level_up.rs:32-33` |
| Group menu, generic `self.mode = rows[idx].target` | `app-core/src/app/group_menu.rs:472` (Perks entry `:396`) |
| `duel_lines` (private); heading strings; `widest_report`; `the_widest_report_fits_its_screen` | `gui/src/render/level_up.rs:43`; `:90`, `:139`; `:187`; `:215` |
| `attribute_line`, `footer`, `draw_allocate_stats`, `allocate_stats_rows` | `gui/src/render/points.rs:34`, `:57`, `:73`, `:87` |
| `perks_menu_rows` (instruction line `:29-31`); `draw_perks_menu` (inserts the stat-points row at `:111`); width census | `gui/src/render/progression.rs:18`, `:92`, `~:830-870` |
| `popup_layout` / `popup_scrolls` (pinned header, scrolling body, pinned footer) | `gui/src/render/popup.rs:~540-590` |
| `needs_status_banner`; `LevelUp` draw; `draw_mode_overlay`; `ALL_MODES: [Mode; 121]`; `NEEDS_PENDING_STATE: [Mode; 23]` | `gui/src/render/mod.rs:572`, `:659-667`, `:880`, `:1561`, `:1758` |

## Where the spec is wrong or leaves a gap (decided here)

1. **The save-identity test cannot see most of the trial's list.**
   `PlayerSave` keeps `hp`, `power`, `stat_points`, `attributes`, perks and
   `bought_stats`. It rebuilds `Stats.max_hp/atk/mitigation`, `Decompiler` and
   `Derived`, so a leak of those would not show in the file. **Decision:** keep
   the save-file comparison, which covers future persisted components. Also add
   a component-equality check over every component on the trial list, with
   each component compared before and after the preview. Mutation-check: drop
   each component from the trial list in turn. At least one of the two
   assertions must fail for every component.
2. **No Mitigation row exists today.** `take_level_up_report` builds only Max
   HP and ATK rows. **Decision:** extract `stat_rows(before, after) ->
   Vec<StatRow>`, with Max HP, ATK and Mitigation filtered to rows that moved.
   The level-up page and `PerkPreview` both call it. The level-up page gains a
   Mitigation row when that stat moves.
3. **The level-up tests are edited, not left as they are.** The duel fields
   move to `report.duel.*`, so `tests/level_up.rs:914-932` and gui
   `level_up.rs` change mechanically. No assertion value changes.
4. **The heading says ZONE, not SECTOR.** The shipped heading is "AGAINST A
   TYPICAL ZONE {n} PROGRAM", and a gui test asserts that string.
   **Decision:** keep "ZONE" everywhere, including the Perks line: `… against
   a typical zone N program`. Renaming it is out of scope.
5. **The Perks popup does scroll.** The comment at `progression.rs:25-27` is
   stale. The preview line sits in the pinned header, so it costs one
   body row. Fix the stale comment. Census: the Perks menu at full perk count
   with the preview line, asserted through `popup_layout`/`popup_scrolls`
   (no vertical-fit test exists today). The Points duel block sits in the
   pinned footer, so the census checks its height from scratch.
6. **An unaffordable spend underflows the core.** `apply_stat_spend` computes
   `banked.saturating_sub(total)`. The public method validates first, so its
   behaviour is unchanged.
7. **The trial must restore absence.** For each listed component, record
   `Option<T>`. Restoring re-inserts `Some` and removes a component that was
   `None`. `Derived` is the realistic case.
8. **`zone` is not in `typical_foe`.** The caller of `duel_comparison` passes
   `ZoneLevel.0`.
9. **Buffer heals to full inside the core.** The trial restores `Stats`,
   which covers it. The preview therefore never heals.
10. **The group menu has no per-mode hook.** `group_menu.rs:472` gets
    `if target == Mode::Perks { self.open_perks() } else { self.mode = target }`
    (or equivalent). `open_perks` sets the mode itself.
11. **A perk is bought with its row key, not Enter.** The spec's app-core test
    ("Enter on the Perks menu opens `PerkBought`") means: a row key opens
    `Mode::PerkBought`, and `Enter`/`Esc` there returns to `Mode::Perks` with
    `menu_selected` kept.
12. **`preview_perk` and game over.** Return `None` when `is_game_over()`,
    matching `unlock_perk`'s guard.
13. **Exports.** `DuelComparison`, `PerkPreview` and `PerkReport` are
    `pub use`d from the engine root beside `LevelUpReport`, and derive `Clone,
    Debug, PartialEq`.

## Phase 1: engine (sonnet)

1.1 **`DuelComparison` + `duel_comparison` + `stat_rows`** in
`game/level_up.rs`, following the spec §One comparison and gaps 2, 3, 8, 13.
`LevelUpReport` loses the four fields and gains `duel`. Update the tests
mechanically. Update gui `level_up.rs` only enough to compile
(`report.duel.*`). The gui redraw is phase 2. Test: a Mitigation-moving
snapshot pair yields a Mitigation row.

1.2 **Split the cores.** `apply_stat_spend(entity, spend)` (gap 6) and
`apply_perk_level(player, perk)`. The log line, `note_deed`, the cost check,
the `PerkDb` check and the game-over check stay outside the cores. Existing
tests must stay green with no edits: that is the refactor's test.

1.3 **`trial`** (gap 7) and **`preview_stat_spend`, `preview_perk`,
`buy_perk`** (spec §Trial, gaps 9 and 12).

Tests, all in `engine/src/tests/`:
- **Preview equals outcome.** A spend (under `preview_stat_spend` followed by
  `spend_stat_points`, the duel then compared to a fresh `snapshot_player`),
  and each stat perk (`preview_perk` followed by `buy_perk`'s
  `report.preview`).
- **Emulation.** Under `Kit::Emulated`, a Parity/Analysis spend previews no
  ATK change. Scope this test to attribute spends only.
- **No leak** (gap 1), with the per-component mutation check.
- **Refusals.** `buy_perk` without Perk Points returns `Err`. `preview_perk`
  without points still returns `Some`.

Start from a `dev-saves/` template where one fits.

## Phase 2: app-core + gui (sonnet)

2.1 **app-core.** Follow the spec §App-core, with gaps 10 and 11:
- Add `allocation_duel`, written only by `refresh_allocation_duel`. It is
  called from `open_stat_allocation` and after each spend key in
  `handle_allocate_stats_key`, and never on the creation Points step.
- Add `perk_previews`, written only by `refresh_perk_previews`, via
  `open_perks`. Route all five `Mode::Perks` sets through `open_perks`:
  `level_up.rs:33`, `progression.rs:53` and `:55`, `leave_allocation`'s
  arms, and the group menu. Add the purchase-page return as a sixth route.
  After that, `rg 'mode = Mode::Perks'` outside `open_perks` returns nothing.
- Add `Mode::PerkBought` and `pending_perk_report`. `handle_perks_key` calls
  `buy_perk`. Add the mode to the `is_battle` `=> false` arm and to input
  dispatch.

Tests:
- The Points duel changes on a spend key.
- The duel is `None` on the creation Points step.
- `perk_previews` is filled on every route in (one test per route, or one
  table-driven test).
- A buy opens `PerkBought`; `Enter` returns with `menu_selected` kept.

2.2 **gui.**
- `duel_lines(&DuelComparison)` becomes `pub(super)` and keeps its "ZONE"
  heading (gap 4).
- On the Points screen, the block goes between the attribute rows and the
  footer. `allocate_stats_rows` reads `app.allocation_duel`.
- The Perks header line sits under the instruction line (spec wording, gap 4)
  and reads `perk_previews[menu_selected]`. Fix the stale "no scroll"
  comment.
- Add a new `render/perk_bought.rs`, with the overlay shape from
  `level_up.rs`. Register it in `mod.rs`: the module, `draw_mode_overlay` or
  the `LevelUp`-style draw arm, `needs_status_banner`, `ALL_MODES`
  (121 → 122) and, if its census app has no report, `NEEDS_PENDING_STATE`
  (23 → 24) with the same fallback `LevelUp` uses.

Tests: the three censuses (gap 5), plus a test that the purchase page draws
the description when no fight figure moved. All drawing goes through
`Painter`.

## Phase 3: controller, inline

1. Run `cargo test --workspace` once. Then run `cargo test -p
   feral-processes-engine balance_sim`; it must be green and unmoved.
2. Take `--screenshot`s and Read the PNGs, via `--template stack --keys …`:
   - the Points screen, reached through Perks → `S`, after one spend key;
   - the Perks menu with a perk highlighted;
   - the purchase page after buying one.
3. Docs: none of the `assets/*/README.md` files changes. The CHANGELOG entry
   is written at release.
4. Final opus review over the whole branch, with the diff given as a file.
   Ask it to re-derive, independently:
   - the trial's component list against every writer that
     `apply_stat_spend`, `apply_perk_level` and `recompute_derived` reach;
   - that each `Mode::Perks` entrance refreshes the previews.
