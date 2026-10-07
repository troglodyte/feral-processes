# Phase Keys and the Basin Exit — plan

Spec: `docs/superpowers/specs/2026-10-07-phase-keys-endgame-design.md`. The spec
holds the rules and the argument; this file gives only the order of work,
the files, and the gates. Branch: `phase-keys-endgame`.

**Every phase:**
- Use TDD and commit at each green step.
- Leave `cargo fmt` and `cargo clippy --workspace --all-targets` clean.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.
- Never push. Never `git checkout`/`stash`/`reset` over uncommitted work.
  Stage explicit paths only.
- Phase gate: the crate tests named in the phase. Phase 6 runs the full
  `cargo test --workspace`.

The phases run in order. Phase 1 has no dependencies and could run alongside
Phase 2 in a separate worktree, but serial is simpler and the phase is small.

## Phase 1 — The Grid rename (assets only, own commit)

- Make the spec §6 replacements across `assets/**` and the ~5 strings and
  comments in the crates (`rg -n "the Grid"`). Leave every term the spec
  lists as unchanged alone.
- Fix the `assets/items/core_fragment.ron` breach text.
- Test (engine `tests/assets.rs`): no setting-sense `the Grid` in `assets/`,
  and `Power Grid` is still present.
- Gate: `cargo test -p feral-processes-engine assets`.

## Phase 2 — Keys as data, state, effects, save

Read `.claude/rules/` for saves and items/stats first.

**Files:**
- new `engine/src/phase_keys.rs`: `PhaseKeyDef { zone, name, flavour,
  effect: KeyEffect }`, `KeyEffect { stat_pct, stats: ImplantStats, hooks:
  Vec<ImplantHook>, signature: Option<ImplantSignature> }`, and
  `PhaseKeyDb::load_dir`. `load_dir` follows `ImplantDb::load_dir` and
  fills all ten slots (fallbacks per spec §1).
- `StatPct { max_hp, atk, mitigation, max_power, decompiler }`, all fields
  `#[serde(default)]`.
- `tuning.rs`: `PHASE_KEY_COUNT`, `PHASE_KEY_DROP_CHANCE`,
  `PHASE_KEY_GUARANTEE_KILLS`.
- `components.rs`: `PhaseKeys { held: u16, misses: u32, story_complete:
  bool }`.
- `save.rs` and `game/lifecycle.rs`: `SAVE_FORMAT_VERSION` 35 → 36.
- Asset loading at the place where `ImplantDb` is inserted.
- new `assets/phase_keys/*.ron` (ten files, spec §7) and
  `assets/phase_keys/README.md`.

**Effects.** Fold held keys into the existing implant readers in
`game/implants.rs` rather than writing parallel ones:
- `implant_stats` adds the flat stats, which also covers `hit_bonus`,
  `derived.rs` and `kit.rs`.
- `implant_hook_total` adds the hooks.
- the `DeadMansSwitch` check becomes `installed || key-held`, so it still
  fires once.

A new pure `apply_key_pct(value, pct) -> i32` (minimum +1 when pct > 0)
applies the summed percents. Call it at the point where the final stat is
formed: in `recompute_derived` after implants, and wherever gear joins. Read
`derived.rs`/`kit.rs` to find every such site, and call the same function at
each one; never copy the formula. Call `recompute_derived` whenever `held`
changes.

**Renderer API:** `Game::phase_keys() -> PhaseKeysView` in `views.rs`. The
effect line comes from the formatter implants already use for their effect
text. If no formatter exists, extract one from the implant UI path and call
it from both.

**Tests** (new `engine/src/tests/phase_keys.rs`):
- the db fallback cases;
- a census test: shipped assets fill all ten slots with no fallback;
- percent summed and minimum +1;
- flat stats, hooks, and a signature that does not double;
- derived, not baked: drop the key and the stats return;
- v36 save→load keeps `held`, `misses` and `story_complete`.

Gate: `cargo test -p feral-processes-engine phase_key implants save`.

## Phase 3 — Drops, gate, warp, achievements (engine)

**Files:**
- `game/combat_rewards.rs` (`award_loot`, on the `pay_stack_boss_fragments`
  branch)
- `game/zone.rs` (`enter_next_zone`, `warp_to_zone`)
- `game/base/building.rs` (`place_structure`)
- `game/base_space.rs` (the portal step at ~L1091)
- `achievements.rs` + `game/achievements.rs`
- `assets/achievements/`
- the alert/popup path the game already uses for modal alerts

**Work:**
- `fn phase_key_roll(seed, zone, kill_index) -> bool`: a pure hash, so the
  shared RNG is never touched. On a drop: set the bit, fire the modal alert,
  the notification and the log line, then call `recompute_derived`. On no
  drop: `misses += 1`. Find the save-seed resource by reading the code.
- `enter_next_zone` clears `misses`.
- `pub(crate) fn phase_key_gate(&self, zone) -> Result<(), String>`.
  Call it in `place_structure` for any `zone_portal` def, and again at the
  portal step. If the step is refused, the portal is **not** despawned and a
  message is posted.
- `warp_to_zone` sets the key of every zone it leaves before calling
  `enter_next_zone`. That covers savetool for free.
- Triggers `PhaseKeyFound(u32)`, `AllPhaseKeys` and `StoryComplete`. Add
  the 12 achievement files.
- Recapture the `dev-saves/` templates that sit past zone 1, using
  `savetool warp`/`capture`. Check the README for each template's zone.

**Tests:**
- eligibility, covering guardian-only, not-held and zone ≤ 10;
- the third kill always drops;
- `misses` clears on breach;
- the same seed gives the same outcome;
- the RNG stream is unchanged, shown by comparing a draw after a guardian
  kill with and without the feature;
- portal build and portal step are both refused without the key, and open
  in zone 11+;
- warp grants keys;
- the achievements fire.

Gate: `cargo test -p feral-processes-engine phase_key zone achievements stack`.
Also run `cargo test -p feral-processes` for the launcher and dev-saves.

## Phase 4 — Basin Exit and ending (engine + app-core + gui)

**Engine:**
- `assets/structures/basin_exit.ron`, plus a built-in fallback def inserted
  when the file is missing or malformed. Mirror how `PhaseKeyDb` fills its
  fallbacks.
- The build gate requires all ten keys, zone ≥ 10, and `!story_complete`.
- The step onto the exit returns a pending confirmation rather than acting.
- `Game::escape_basin()`, called after the confirmation, sets the flag,
  unlocks the achievement and despawns the structure.
- The ending text is a new asset, `assets/story/ending.ron`: a list of
  screens, with a README and a built-in fallback.
- Expose it through `Game::ending_screens()`.

**app-core:**
- A confirmation popup using the existing yes/no popup flow.
- A new `Mode::Ending { page }` that pages through the screens and then
  returns to `Playing`.
- Watch the `all-modes` traps (memory: `all-modes-length-is-a-semantic-merge-conflict`).

**gui:**
- `render/ending.rs` draws through `Painter`.
- The "Escaped" marker on the character sheet.

**Tests:**
- confirmation is required;
- the flag, achievement and structure removal;
- a rebuild is refused;
- play continues: ticks run and you can move;
- the fallback def and the fallback text;
- app-core mode transitions.

Gate: `cargo test -p feral-processes-engine basin` plus
`-p feral-processes-app-core`.

## Phase 5 — Inventory Phase Keys tab (app-core + gui)

- app-core `app/inventory.rs`: an inventory tab enum, `Items | PhaseKeys`,
  switched by `GameKey::Tab`. That follows `playing.rs:371` and the
  `InfoTab` convention. On the keys tab, row selection shows the flavour
  and no item actions resolve.
- gui `render/inventory.rs`: a tab header, ten rows and the footer
  (spec §4). Size rows from the longest name and effect in the data.
- **Tests:**
  - tab switching;
  - ten rows always;
  - sell, erase and equip are inert on the keys tab;
  - a headless row-width test (memory: `popup-row-width-is-testable-headlessly`).
- Gate: `cargo test -p feral-processes-app-core inventory` plus
  `-p feral-processes-gui`.
- Take a screenshot with `--template` on a recaptured save, then `--keys`
  to open the inventory and Tab. Read the PNG.

## Phase 6 — Balance check, docs, final gate

- Check the spec §7 numbers against `docs/measurements/` and the per-zone
  level curve. Report outliers to the user rather than retuning on our own.
- Add a `balance_sim` case with keys 1..=N held at zone N's cap. A moved
  curve should show up as the key delta.
- Update `CHANGELOG.md` under an unreleased heading, with the breaking
  save-format note. The version bump happens at deploy.
- Update `docs/superpowers/INDEX.md`.
- The `seams` triple, if a new seam emerged. The likely candidate is "key
  effects ride the implant readers".
- `cargo test --workspace`, then a whole-branch review on opus, run from a
  diff file.
