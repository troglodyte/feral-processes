# Bounty Contracts — Plan

Spec: `docs/superpowers/specs/2026-11-06-bounty-contracts-design.md`. ADR:
`docs/adr/0044-a-bounty-is-a-rust-rolled-contract-that-owns-its-quarry.md`.
Branch `worktree-bounty-contracts`. Paths below are relative to `crates/engine/src/` unless they
say otherwise.

**Precondition:** ADR 0044 must be **Agreed** (in its Status and in the README row) before Phase 1
writes any code. Never build on a Proposed record.

**Rules for every phase:**
- TDD: write the failing test first, then make it pass. Commit at each green step, staging explicit
  paths.
- After each phase: `cargo fmt`, then `cargo clippy --workspace --all-targets` with no warnings. Run
  the phase's targeted tests, then `cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES`) as the
  phase gate.
- Before touching combat, spawning, contracts or compass code, read the matching
  `.claude/rules/seams-*.md` (`seams-ground`, `seams-combat`, `seams-tactical`, `content-schema`).
- RNG: every new draw happens over input sorted by a stable id. No existing draw order changes, so
  `balance_sim` must stay unmoved (checked at the end of Phase 3).
- No push. Never run `git checkout`, `git stash` or `git add -A`.

Phases are sized to one context each and run in order. Each phase depends on the one before it.

---

## Phase 1 — Pure core: types, roll, objective variant

**Files**
- new `bounty/mod.rs`, `bounty/roll.rs`, `bounty/names.rs`, `bounty/tests.rs`
- new `tuning/bounty.rs`, re-exported from `tuning/mod.rs` (one `pub use` line only)
- `contracts.rs`: `Objective::Bounty(BountyQuarry)`, appended last. Add an arm to every exhaustive
  match, in `contracts.rs` and `game/contracts.rs` (`objective_line`, `objective_hint`,
  `Objective::target`, and whichever others the compiler finds).
- `contracts.rs` loader: an authored file or template whose objective is `Bounty` is skipped with a
  warning (the `template_complaint` / `load_dir` pattern).
- `lib.rs`: `pub mod bounty;`

**Interfaces**
```rust
// bounty/mod.rs: all plain data, Serialize/Deserialize, Clone, PartialEq
pub struct BountyQuarry {
    pub name: String,            // "Vesk the Long Wire"
    pub species: String,
    pub grade: f32,              // stat multiplier on top of the boss path
    pub rarity: Rarity,
    pub escort: Vec<(String, u32)>, // species, count; sorted by species
    pub anchor: (i32, i32),
    pub tether: i32,
}
pub struct BountyInput {         // gathered by Game in Phase 2
    pub zone: u32,
    pub habitat: Vec<String>,    // eligible non-boss species, sorted
    pub anchor_candidates: Vec<(i32, i32)>, // walkable, filtered, sorted
}
pub fn roll(input: &BountyInput, rng: &mut impl Rng) -> Option<RolledBounty>;
pub struct RolledBounty { pub def: ContractDef } // Objective::Bounty, reward, min_zone, description
```
- `names.rs`: two plain word banks, `<Name> the <Epithet>`. Follow the naming rule: no occult words,
  no slogans.
- `tuning/bounty.rs`: `BOUNTY_MIN_ZONE = 2`, `BOUNTY_ANCHOR_BAND = (25, 60)`, `BOUNTY_TETHER = 6`,
  the grade curve, escort count per zone, reward credits/XP per zone, and the board odds. The builder
  picks the numbers. The reward must clearly exceed a Hunt at the same zone; test that against
  `hunt.ron`'s per-unit reward times its maximum count.

**Test intent (call `roll` directly, no `Game`)**
- The same input and seed give the same bounty, and a different seed gives a different name or
  species.
- Grade, escort size and reward don't fall as the zone rises, and each is strictly higher at zone 6
  than at zone 2.
- Below `BOUNTY_MIN_ZONE`, with an empty habitat, or with no anchor candidates, the roll returns
  `None`.
- The description names the target and its species.
- A `.ron` file with `objective: Bounty(...)` in a temporary contracts directory is skipped with a
  warning, and the other files still load.
- The `BountyQuarry` RON round-trip works inside an `ActiveContract`.

**Gate:** `cargo test -p feral-processes-engine bounty contracts`, then the workspace.

---

## Phase 2 — On the map: board, accept, quarry, compass, save

**Files:** `game/contracts.rs`, new `game/bounty.rs` (the `impl Game` shell), `components/world.rs`
(marker), `systems.rs` (tether), `game/spawning.rs` (cull/clear), `settlements/mod.rs`,
`game/inspection.rs`, `views/programs.rs` if `CompassRow` needs it, `game/lifecycle/load.rs`, and
tests in `tests/bounty.rs` (new).

**Work**
1. **Board.** `rolled_contracts` may add one bounty per refresh, off `board_seed`, through
   `bounty::roll` with a gathered `BountyInput`. Habitat comes from the existing pools. Anchor
   candidates are walkable surface tiles inside the band, outside settlement footprints and off Stack
   links, sorted. `offerable` hides a bounty while one is held, and hides a rolled id that is in
   `ActiveContracts.failed`. That is a new `#[serde(default)] failed: Vec<ContractId>`, cleared of any
   id that is no longer on the current epoch's board.
2. **Accept.** The normal `accept_contract` path, with no new refusal (the cap still applies).
   Afterwards, `ensure_bounty_quarry()` spawns it.
3. **Quarry entity.** `components::BountyMark { contract: ContractId }`. It is spawned through
   `spawn_wild_creature_pinned(boss: true, …)` and then its `Stats` are scaled by `grade`, with
   `Rarity` set. Check whether a pin or a post-multiply is the existing idiom, and follow it. Add a
   `Hostile` and `WanderAi` like any wanderer.
4. **Tether.** `wander_ai_system` keeps a `BountyMark` creature within `tether` of `anchor`, the same
   way the `NestGuardian` branch does. If possible, reuse that branch's step-rejection through a
   shared pure function, not a copy.
5. **Survival.** `cull_to_cap` and `clear_local_wild` spare `BountyMark`, as they spare `Nemesis`.
   `ensure_bounty_quarry()` runs after load, after `enter_next_zone`, and from the contract tick. It is
   one query for the single marker, and it respawns at `anchor` if the quarry is missing. It is
   idempotent: never two quarries.
6. **Abandon.** `abandon_contract` on a bounty despawns its quarry and clears a compass selection
   that names it.
7. **Compass.** Add `CompassTarget::Bounty(ContractId)`, appended last. `compass_targets` adds a row
   group after outposts, built from the live quarry `Position`. The label is `Bounty: <name>`, with an
   exact bearing and distance. When the contract ends, a selection naming it is cleared to `None`.
8. **Save.** Only the additive fields (`failed`, the variants). The quarry is not saved: it respawns at
   its anchor on load.

**Test intent (dev-saves templates where they fit, not a fresh `Game::new`)**
- The board at zone ≥ 2 can offer a bounty, and is stable across save → load within an epoch. Zone 1
  never offers one.
- Accepting spawns exactly one marked quarry at the anchor, and the compass lists it with the
  quarry's live position.
- Over many ticks, the quarry stays within the tether.
- A forced cull, a `clear_local_wild` call and an `enter_next_zone` breach each leave a quarry
  standing (respawned or spared), never zero and never two. After the breach its grade is unchanged.
- **Save → load with a live bounty** gives the same contract, a quarry at its anchor, and a compass
  row that still resolves. This must be the real save path, not a RON round-trip.
- Abandoning removes the quarry and the compass row, and the bearing falls back to Home.
- A held bounty hides any second bounty on the board.

**Gate:** `cargo test -p feral-processes-engine bounty`, then the workspace.

---

## Phase 3 — The fight: bump, escort, verdict, fail path

**Files:** `game/turn.rs` (bump), `game/combat.rs` (`gather_pack` exclusion and the bounty start
route), `tactical/turn.rs` and `tactical/` squad planning (only as far as needed),
`resources.rs` (`BattleState` tag), `game/combat_rewards.rs` (quarry kill), `game/combat_teardown.rs`
(verdict), `game/contracts.rs` (fail door), `game/bounty.rs`, and `tests/bounty.rs`.

**Work**
1. **Exclusion.** `gather_pack` drops `BountyMark` creatures from any other pack, as it drops bosses.
   A pack anchored *on* the quarry never goes through `gather_pack` at all.
2. **Bump.** In `move_player_inner`, when `find_wild_creature_at` returns a `BountyMark` entity, call
   `start_bounty_battle(quarry)`. It spawns the escort roster next to the quarry (`scatter_open_tile`),
   builds the pack with the quarry first, then the escort, and opens whichever model `fights_tactically`
   picks. Abstract fights go to `begin_battle(groups)`, grouped by species, with **no** zone group-count
   or group-size ceilings. For the tactical model, find where caps apply in `open_tactical_battle_at` /
   `squads::plan`, and pass the full roster past them with the smallest change that does it. If that
   needs more than a parameter, stop and report back rather than reshaping the planner.
3. **Tag.** Add `BattleState.bounty: Option<BountyFight { contract, quarry: Entity, quarry_down: bool }>`.
   It isn't serialized, which is the same argument as `lair`.
4. **Quarry kill.** On the kill path where kills are recorded (`award_loot`), the death of
   `BountyFight.quarry` sets `quarry_down` and marks the contract met. It completes through
   `settle_contracts` / `complete_contract` (the normal door and completion summary) at teardown.
   It is never paid twice.
5. **Verdict.** A pure function, `bounty::verdict(quarry_down: bool) -> BountyOutcome { Complete,
   Fail }`. In `finish_fight`, it reads only the tag, never `verdict.won`. On `Fail`, call the new
   `Game::fail_contract(id)`: remove the contract, push its id to `failed`, despawn the quarry,
   clear the compass selection, and announce "<name> got away. Bounty failed." exactly once through
   the existing notification/log path. Read the notifications seam rules first. Surviving escort
   members are despawned in either case.
6. A Forgiving-mode reboot after a loss doesn't change the verdict.

**Test intent**
- Bumping the quarry opens a fight holding the quarry plus the exact roster. At zone 2 there is more
  than one group whenever the roster has two species, so the caps don't apply. Cover both models: a
  surface the tactical model picks, and an abstract one.
- Bumping an ordinary wanderer next to the quarry gives a fight without the quarry.
- Kill the quarry and then flee: the contract completes, pays once and is gone.
- Flee with the quarry alive: the contract fails, the id is in `failed`, the quarry is despawned, the
  compass row is gone, one notification fires, and the board doesn't re-offer that id this epoch.
- Lose (on a Forgiving template) with the quarry alive: the contract fails.
- Win by clearing everything: it completes.
- Escort kills advance a held `Terminate` contract for that species.
- Unit tests on `bounty::verdict`.
- Mutation-check the fail door and the `quarry_down` read. Each test must fail with its line removed.

**Gate:** targeted tests, then `cargo test -p feral-processes-engine balance_sim` (unmoved), then the
workspace.

---

## Phase 4 — Screens, arena check, docs

**Files:** `crates/gui/src/render/compass.rs` (`target_glyph` for `Bounty`, with a hue added to
`hud::palette` if needed), the zone map's drawing of the quarry if it isn't already drawn as a boss,
the battle header or group label showing the quarry's name (find where the group name comes from,
and take the name from the engine view, not from the gui), app-core contracts screen if a row needs a
bounty-specific hint, `crates/app-core` tests. Also `dev-arenas/bounty-z2.ron`, `dev-arenas/bounty-z6.ron`,
`assets/contracts/README.md`, `CHANGELOG.md` (an unversioned section is fine; versioning happens at
deploy), and a `docs/superpowers/INDEX.md` row.

**Work and test intent**
- The engine exposes the quarry name on the battle view (`views/`), and the gui only reads it. An
  app-core or gui test asserts the name appears in the battle header text.
- The compass picker row has the bounty glyph and colour, and a headless row test passes.
- Screenshot check: `cargo run -- --template <zone≥2 template> --keys ... --screenshot` on the compass
  picker with a bounty held. The battle screen goes through the arena (`FERAL_DEV_ARENA=1`), because
  `--keys` can't walk into a fight. Read the PNGs. If a dev-save with a live bounty would help, `capture`
  one into `dev-saves/`.
- Arena: run both scenarios against a mid-grade party. Before running, read `docs/measurements/`
  and record the result only if it meets that README's bar. If the zone-2 bounty is trivial or a wipe,
  retune `tuning/bounty.rs` and rerun `balance_sim`.
- Docs: in `contracts/README.md`, `Bounty` is rolled by code and can't be authored. Write the CHANGELOG
  entry in plain player language, and add the INDEX row (spec, plan, ADR 0044, key symbols).

**Gate:** `cargo clippy --workspace --all-targets`, `cargo test --workspace`, then the final
whole-branch review (opus) with the diff passed as a file. After that, deploy is a separate ask.
