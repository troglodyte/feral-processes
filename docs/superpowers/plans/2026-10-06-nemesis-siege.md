# Nemesis siege — plan

Spec: `docs/superpowers/specs/2026-10-06-nemesis-siege-design.md` (the
argument and all the rules; this file is only the order of work).
Branch: `nemesis-siege`. Engine-only (`crates/engine`) plus a dev-save.

**Every phase:**
- Use TDD: write a failing test first, then make it pass.
- Commit at each green step.
- Run `cargo fmt` and `cargo clippy --workspace --all-targets`, and leave
  them clean.
- Run the phase gate: `cargo test -p feral-processes-engine nemesis` plus
  the named suites.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.
- Never push.
- Never run `git checkout`/`stash`/`reset` over uncommitted work.
- Stage explicit paths only.

Before Phase 2, read `.claude/rules/seams-sieges.md` and
`.claude/rules/seams-ground.md`.

## Phase 1 — The band: gathering, following, cull exemption, saves

**Files:**
- `components.rs`
- `tuning.rs`
- a new `game/nemesis_muster.rs`, registered next to the other per-tick
  game hooks in `game/turn.rs`
- `systems.rs` (wander filter)
- `game/spawning.rs`
- `game/combat_teardown.rs` (follower marked → its own leader)
- `save.rs`
- `game/lifecycle.rs` (save write/read)
- a new `tests/nemesis_siege.rs`

**Interfaces:**
- Components:
  - `NemesisMuster { ticks: u32 }`
  - `NemesisFollower(pub Entity)`
  - `NemesisHome(pub Position)`: added now for the save field, used in
    Phase 2.
- Tuning constants: `NEMESIS_RECRUIT_INTERVAL` (600),
  `NEMESIS_RECRUIT_RADIUS` (12), `NEMESIS_BAND_MAX` (3),
  `NEMESIS_MARCH_DELAY` (= `SIEGE_WARN_FLOOR_TICKS`). Each gets a
  minutes-of-play doc, written the way `SIEGE_PRESSURE_THRESHOLD`'s is.
- `Game::nemesis_muster(&mut self)` runs per tick. It increments
  `NemesisMuster.ticks` (inserting it on a nemesis that lacks one), and
  `muster.ticks` resets on each recruit. When the band is short and
  `ticks >= NEMESIS_RECRUIT_INTERVAL`, it recruits:
  - the nearest eligible `Hostile` in radius (the spec's exclusion list);
  - otherwise one `spawn_group(species, 1, …)` body from
    `habitat_pools` at the leader's cell.
  When the band becomes full, it posts the "gathered a band" alert once
  (a new `AlertKind` if the alert code requires one).
- **Following:** `wander_ai_system` gets `Without<NemesisFollower>`. A
  small `nemesis_follow` step moves each follower one walkable step toward
  its leader when Chebyshev distance > 2.
- **Leaving the band:** a follower whose leader is missing or no longer a
  `Nemesis` loses its tag (swept in `nemesis_muster`).
  `mark_nemeses` removes `NemesisFollower` from any body it marks.
- **Cull exemption:** `cull_to_cap`'s filter gains
  `Without<Nemesis>, Without<NemesisFollower>`. Audit the other despawning
  sweeps in `spawning.rs` the same way. `clear_local_wild` also keeps
  followers.
- **Saves:** new `CreatureSave` fields, all `#[serde(default)]`:
  - `nemesis_muster_ticks: u32`
  - `nemesis_home: Option<(i32, i32)>`
  - `nemesis_band: Option<u32>`

  `nemesis_band` follows the `sortie_index` precedent: entity ids are not
  stable. At save, each leader is assigned a band number and the leader
  and its followers all carry it. At load, followers are linked to the
  leader with the same number (a body with `Nemesis` plus a band number is
  the leader).

**Test intent:** the spec's §6 bullets for these areas:
- gathering: recruit, refusals, spawn fallback, cap, following;
- cull;
- follower → leader on a jack-out;
- band save→load through the real save path, not RON alone.

Mutation-check the cull exemption.

**Gate:** the `nemesis`, `siege`, `save_roundtrip` and `spawning` test
suites are green. Then run `cargo test -p feral-processes-engine
balance_sim`: it should not move, but this phase adds a tick system.

## Phase 2 — March, siege, aftermath

**Files:**
- `game/siege/{mod,offscreen,clock,persist}.rs`
- `game/combat_teardown.rs`
- `game/nemesis_muster.rs`
- `tests/nemesis_siege.rs`

**Interfaces:**
- **Refactor first, behaviour unchanged:**
  - `open_siege(&mut self)` becomes `open_siege_with(&mut self, raiders:
    Vec<Entity>) -> bool`. The existing `open_siege` becomes a thin
    wrapper that calls `spawn_siege_pack(site, zone)`. The empty-pack and
    nobody-seated rules stay inside, unchanged.
  - `resolve_siege_offscreen()` becomes `resolve_siege_offscreen_with(
    strength: u32)`, and the old name passes `pack_size(zone)`.
  - All 87 `siege` tests must stay green with no edits.
  - Commit this on its own.
- **March:** `Game::nemesis_march_check()` runs from the same `turn.rs`
  site, after `siege_check`.
  - It picks the first nemesis (ordered by save-stable sort key, not query
    order — see the memory on query iteration order) whose band is full
    and whose `ticks >= NEMESIS_MARCH_DELAY`.
  - It holds under the siege holds. Factor `siege_check`'s hold
    predicate into one function both call; do not copy it. It also holds
    while any `Besieger` exists or a battle is running, and when
    `sieges_enabled()` is false.
  - On firing:
    - insert `NemesisHome(current pos)`;
    - tag the leader and followers `Besieger`;
    - if the player is at the base, call `open_siege_with(band)` and log
      the taunt using `combat.rs`'s taunt helper (extract it if it is
      inline);
    - otherwise call `resolve_siege_offscreen_with(band.len())` and run
      the aftermath immediately.
- **Aftermath:** a new `Game::nemesis_return_home(leader)`.
  - In `end_battle`'s besieger sweep, a leader carrying `NemesisHome` is
    spared. Remove its `Besieger` tag, set its `Position` to the home
    cell, then escalate the grudge.
  - Extract from `mark_nemeses` the per-body grudge +1, `promote_rarity`
    and heal into `escalate_nemesis(e)`, and call it from both places.
  - Reset the muster and remove `NemesisHome`.
  - A downed leader despawns as usual. Post the "trouble you no more"
    alert when a `NemesisHome` leader is absent at teardown.
  - The off-screen path despawns the followers and calls
    `nemesis_return_home` directly.
- **Siege save:** `SiegeSave` needs nothing new if the leader's
  `NemesisHome` is on its `CreatureSave`. Verify this with the
  mid-siege round-trip test rather than assuming it.

**Test intent:** the spec's §6 bullets for these areas:
- trigger and holds, including `FERAL_DEV_NO_SIEGES`;
- siege at home and off-screen;
- both aftermath branches;
- mid-siege save;
- a map fight that pulls in the band.

Mutation-check the leader's exemption from the besieger sweep.

**Gate:** run `cargo test --workspace`, then `balance_sim`.

## Phase 3 — Dev save, docs, review

- Create `dev-saves/nemesis.ron`:
  1. Build it from `dev-saves/siege.ron`: load it, use `savetool dump` to
     give one wild creature `nemesis_grudges: 1` plus a band of 1, then
     `pack`.
  2. Confirm it loads with `cargo run -- --template nemesis` (start it, no
     need to play).
  3. Add it to `dev-saves/README.md` (not the root README).
- `CHANGELOG.md`: add an unreleased entry.
- Add the `seams-sieges.md` line from the spec's Docs section, with the
  matching `seam:` graph entry and `seams` skill section, written in the
  order the skill documents.
- Final whole-branch review by opus, with the diff passed as a file. Fix
  what it finds, and have the fixes reviewed too.

## Execution notes

- Phases 1 and 2 are sequential: Phase 2 reads Phase 1's components.
  Each one fits a single subagent (sonnet).
- Phase 3's docs can run in parallel with the review only after Phase 2
  is green.
- Per-task review gates are off unless you want them. The final review
  is mandatory.
