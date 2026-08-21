# Entity memories — Phase 3: Triggers

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The four shipped memory defs are written by four real triggers, so
an owned program's store fills from play rather than from a test fixture.
`Game::remember` gains its first non-test callers and the phase-2
`dead_code` attribute goes with them.

**Architecture:** Four hooks, each at the one door its event already goes
through — `resolve_and_apply_attack` for a maul, `end_battle` for the two
victory memories, and a `&mut Game` pass in `tick_inner` for a stranding.
No new system, no new resource, no data-driven trigger vocabulary.

**Tech Stack:** Rust, `bevy_ecs` 0.19.

**Spec:** `docs/superpowers/specs/2026-08-21-entity-memories-design.md` —
read sections 4, 5 and 9 before starting. This plan implements section 5 and
the one tuning constant section 9 leaves to it, and nothing else.

**Phases 1 and 2 are landed** (`6a1cd1c3`, `f336c89b`, `787155dd`,
`c80d2416`, `3aeed62f`, `7fd985b4`, `54c85fff`): `ProgramId`, `MemoryDb`,
`assets/memories/` with all four defs already authored, `components::
Memories`/`Memory`/`MemorySubject`, derived intensity, and `Game::remember`
/ `morale` / `opinion_of` in `crates/engine/src/game/memories.rs`. **Read
that file before starting** — every task in this plan is a caller of it.

## Global Constraints

- **No new `.ron` files and no schema change.** All four defs already ship
  under `assets/memories/`. If you find yourself editing `MemoryDef`, stop:
  this phase writes memories, it does not describe new ones.
- **No `SAVE_FORMAT_VERSION` bump.** Nothing here is saved. `Stranded` is
  not saved and must not become saved; `BattleState` is never serialised.
- **No RNG, anywhere.** `remember` draws nothing and neither may any hook.
  A trigger that spends a draw shifts every seeded test and every
  `dev-arenas/` report.
- **Nothing here may reach `Stats`, damage, accuracy, or any figure
  `balance_sim` models.** The hooks *read* `Stats` (a maul fraction, an
  opening-round power sum); none of them writes one. A moved balance curve
  means you changed something you shouldn't have.
- **Formation logs nothing.** No `MessageLog` write in any hook. The screen
  is phase 4.
- **An empty database stays inert.** With `assets/memories/` deleted every
  hook still runs and every `remember` answers `UnknownDef`, writing
  nothing. Do not gate a hook on the database being non-empty — that would
  make the property hold by accident at one site and lapse at another.
- Comments explain *why*, never *what*.
- Gates while iterating: `cargo test -p feral-processes-engine <name>`.
  Before the phase is called done: `cargo fmt`, `cargo clippy --workspace`,
  `cargo test --workspace`.

**Evidence standard.** Every test in this plan must be mutation-proved:
delete the fix, run the test, watch it fail, restore the fix. Record the
mutation applied and the failure seen. A test that passes with its fix
removed is coverage-shaped and worse than nothing.

**Known trap — a single-crate run is a different build.** `-p
feral-processes-engine` and `--workspace` compile different crate sets and
so shift the RNG stream. A seeded test can fail in one and pass in the
other. Confirm any surprise under `--workspace` before treating it as real.

**Known trap — a new field on `BattleState` fails to compile at every
construction site.** That is the point of Task 2, not an obstacle to route
around with `..Default::default()`.

---

## Decisions this plan takes, and why

The spec's section 5 names four triggers in one line each. Five things it
leaves open are settled here so they are not re-argued mid-task.

1. **The maul hook is `resolve_and_apply_attack`, not `apply_damage`.**
   `apply_damage` is the one door that *damages* a creature, and it is the
   obvious place — except that it does not know who is swinging, and
   `mauled_by`'s subject is the attacker's species. Every
   creature-versus-creature attack in the game already funnels through
   `resolve_and_apply_attack` (four call sites: the basic-attack path, two
   ability paths in `combat_round.rs`, and the wild swing), and that
   function holds both entities and the figure that actually landed. The
   other five `apply_damage` callers are terrain, a thrown item, a raid, a
   status tick and a fumble rung — none has an attacker whose *species* a
   program could hold a grudge against, and a fumble's recoil damages the
   swinger, which through `apply_damage` would read as being mauled by
   one's own kind.

2. **"At the opening round" is a snapshot taken in `begin_battle` and
   parked on `BattleState`.** `hard_won` asks whether the hostiles
   outweighed the party *at the opening round*, and by the time
   `end_battle` runs the hostiles are dead by definition — the question is
   unanswerable from the end of the fight. `BattleState` is the right home
   for the same reason it holds `decompile_attempts`, `rewards` and `lair`:
   the fight's lifetime is exactly the value's lifetime, removing the
   resource takes the snapshot with it, and battles are never serialised so
   it costs no save field. It stores the **verdict**, a `bool`, not the two
   sums — nothing else has a use for the figures, and a stored pair invites
   a second reader deriving a different threshold from them.

3. **The party side of that comparison includes the player.** The spec says
   "the party's", and `kill_xp` deliberately reads the player *alone* so
   recruiting cannot dock XP. That argument does not transfer: this figure
   is "were we outmatched", and a party of four that walks into a pack is
   not outmatched merely because the player is one body. Nothing pays out
   per head here, so there is no incentive to invert.

4. **The stranding trigger is edge-triggered, and `Stranded` grows a
   `since` tick to say so.** `haul_step_system` sets and clears that marker
   *every tick*, so a per-tick `remember` would saturate `strike_cap` in
   three ticks and hold the grudge at full intensity for as long as the
   route stayed broken — which makes `strikes` mean nothing and a three-tick
   route loss worth the same as an hour of it. `set_machine_status`'s rule
   is the one to follow: entering a state is news, staying in it is not. So
   the marker becomes `Stranded { since: u64 }`, written only when the
   worker did not already carry one, and the `&mut Game` pass reads
   `since == now` to find this tick's *new* strandings. That keeps the edge
   in the one place that can see it, needs no change-detection query and no
   new resource, and re-stranding after a repair correctly earns a second
   strike.

5. **The remembered tile is the worker's own, not its machine's.** "Left
   stranded here" is where the body is standing, which is the only tile it
   has any claim about — it is stranded precisely because it is *not* at
   its post. It is also the only reading under which phase 5's hook can
   ever fire: `park_idle_staff` already rejects a tile a `Structure` stands
   on, so a memory keyed to the machine's tile would make that hook inert
   by construction.

**Deferred on purpose, do not build:** `MEMORY_AVOIDANCE_THRESHOLD` and the
`park_idle_staff` rejection (phase 5); the screen and `views::MemoryRow`
(phase 4); any log line announcing a memory; fusion inheriting a parent's
memories; the `CLAUDE.md`/`AGENTS.md`/`docs/seams.md` entries (phase 5, when
the feature is whole — a seam doc describing a half-built feature is a
recorded trap in this repo).

---

## File structure

| File | Responsibility in this phase |
|---|---|
| `crates/engine/src/tuning.rs` | `MEMORY_MAUL_FRACTION`, in the existing memory section |
| `crates/engine/src/game/memories.rs` | the module `dead_code` attribute narrowed to the two unread readers; `note_maul`, `form_victory_memories`, `note_strandings` |
| `crates/engine/src/game/combat_damage.rs` | the maul hook, at `resolve_and_apply_attack`'s landed figure |
| `crates/engine/src/game/combat.rs` | `begin_battle` computes the opening-round verdict |
| `crates/engine/src/resources.rs` | `BattleState::outmatched` |
| `crates/engine/src/game/combat_teardown.rs` | the victory hook, beside `mark_nemeses` |
| `crates/engine/src/components.rs` | `Stranded` gains `since` |
| `crates/engine/src/game/base/hauling.rs` | insert the marker on entry only |
| `crates/engine/src/game/turn.rs` | `note_strandings` in `tick_inner` |
| `crates/engine/src/tests/memories.rs` | every test below |
| `crates/engine/src/tests/assets.rs` | the trigger-reachability census |

---

## Task 1 — A hit that nearly ends you is remembered by species

**Files:** `tuning.rs`, `game/memories.rs`, `game/combat_damage.rs`,
`tests/memories.rs`

- [ ] Add `MEMORY_MAUL_FRACTION: f32 = 0.35` to the memory section of
      `tuning.rs`, documented as what fraction of a program's *maximum* HP a
      single landed hit has to take for the program to remember what hit it.
      Phase 2 deliberately left this constant to this phase.
- [ ] Write `Game::note_maul(&mut self, attacker, defender, landed)` in
      `game/memories.rs`. It resolves the attacker's `Creature::species`,
      compares `landed` against `MEMORY_MAUL_FRACTION * max_hp`, and calls
      `remember(defender, "mauled_by", MemorySubject::Species(..))`. An
      attacker with no `Creature` (the player) and a defender with no
      `Stats` are both no-ops. Strictly greater than, matching the spec's
      "above".
- [ ] Call it from `resolve_and_apply_attack`, immediately after `let landed
      = self.apply_damage(defender, rolled);` and before the outcome is
      rebuilt. Do **not** call it on the `rolled <= 0` early return — a miss
      and a fumble are not mauls.
- [ ] Delete the module-level `#![cfg_attr(not(test), expect(dead_code, ..))]`
      from `game/memories.rs` — `remember` now has a real caller, which is
      what that attribute's own comment says to do — and put a narrower
      `expect` on `morale` and `opinion_of` alone, naming phases 4 and 5.
      They are still unread outside tests and the build is warning-clean.

**Tests** (`tests/memories.rs`):

- [ ] A companion taking a hit above the fraction holds one `mauled_by`
      memory naming the attacker's species; the same companion taking a hit
      *below* it holds none. Both halves in one test — the "forms" half
      alone passes against a hook with no threshold at all.
- [ ] The subject is the attacker's species and not the defender's. A
      fixture where the two differ is the only one that can tell.
- [ ] A wild program mauled by another wild program stores nothing (no
      `Memories` component, so `remember` answers `NoStore`), and the hook
      does not panic reaching for a store that isn't there.
- [ ] The player being mauled stores nothing, for the same reason.
- [ ] Mitigation counts: the figure compared is what *landed*, not what was
      rolled. Give the defender enough mitigation that a rolled hit above
      the fraction lands below it, and assert nothing is remembered. This is
      what pins the hook to `landed` rather than to `rolled`.

**Gate:** `cargo test -p feral-processes-engine memories`

---

## Task 2 — The fight records whether it was uphill

**Files:** `resources.rs`, `game/combat.rs`, `tests/memories.rs`

- [ ] Add `pub(crate) outmatched: bool` to `BattleState`, documented with
      decision 2 above: why the verdict is taken at the bell rather than
      derived at teardown, and why it needs no save field.
- [ ] `begin_battle` computes it before inserting the resource: the summed
      `Stats::power()` of every living member of every group, against the
      summed `Stats::power()` of the player plus every living `Party`
      member. Strictly greater than.
- [ ] Every other construction site of `BattleState` — the compiler will
      list them — sets it explicitly. Do not reach for a `Default`.

**Tests** (`tests/memories.rs`):

- [ ] A fight opened against a pack that outweighs the party sets the flag;
      one against a single weak hostile does not. Read the flag off
      `BattleState` directly.
- [ ] The party side counts the player *and* the companions: a party whose
      companions alone are outweighed but which is not outweighed once the
      player is counted reads `false`. This is the half that pins decision 3
      and it is the only test that can distinguish the two readings.

**Gate:** `cargo test -p feral-processes-engine memories`

---

## Task 3 — What a won fight leaves behind

**Files:** `game/memories.rs`, `game/combat_teardown.rs`,
`tests/memories.rs`

- [ ] Write `Game::form_victory_memories(&mut self)` in `game/memories.rs`.
      It returns immediately unless `BattleState::groups` is empty — the
      same definition of a win `settle_rewards` and the `FightEnd` record
      already read, so the three cannot disagree. Then, over the **living**
      members of `Party`:
      - each one that carries a `ProgramId` forms `bonded_in_battle` about
        every *other* living party member that carries one. The player
        carries none and so is neither a holder nor a subject, which falls
        out of `ProgramId`'s absence rather than needing a `Player` check.
      - if `BattleState::outmatched`, each one also forms `hard_won` with
        `MemorySubject::Nothing`.
- [ ] Call it from `end_battle`, immediately after `mark_nemeses()`. Both
      sit inside the window where `BattleState` is still present, and the
      pairing is worth a comment: `mark_nemeses` is what a fight the party
      *lost* leaves behind, this is what a fight it won does.
- [ ] Confirm by reading that the dead have already been reaped by that
      point (`dissolve_tamed_program` runs above it) and that this is
      harmless: the hook wants survivors only, and a reaped companion is
      neither a holder nor a subject.

**Tests** (`tests/memories.rs`):

- [ ] Two companions that win a fight together each hold one
      `bonded_in_battle` about the *other*, and neither holds one about
      itself.
- [ ] A companion that died in the fight is neither remembered by the
      survivors nor left holding anything. (Assert on the survivor's store —
      the dead one is gone.)
- [ ] A lone companion with no other party member forms no
      `bonded_in_battle` at all, and does not panic.
- [ ] The player is never a subject: no memory names the player's
      `ProgramId`, because the player has none. A survivor's store after a
      solo-plus-one fight is the fixture.
- [ ] Winning an `outmatched` fight forms `hard_won`; winning an even one
      does not. Both halves, one test.
- [ ] **A fight the party jacks out of forms neither memory.** Fleeing
      leaves `groups` non-empty, which is the whole gate — a hook without it
      passes every "won" test above.
- [ ] Winning twice reinforces rather than forking: two wins beside the same
      companion leave one `bonded_in_battle` entry at two strikes.

**Gate:** `cargo test -p feral-processes-engine memories`

---

## Task 4 — A grudge against one corner of the base

**Files:** `components.rs`, `game/base/hauling.rs`, `game/memories.rs`,
`game/turn.rs`, `tests/memories.rs`

- [ ] `Stranded` becomes `pub struct Stranded { pub since: u64 }` — the tick
      the worker *entered* the state. Rewrite its doc comment: it is no
      longer "set and cleared every tick" but set on entry and cleared as
      before, and the reason is decision 4 above. It stays unsaved.
- [ ] `haul_step_system`'s worker query gains `Option<&'static Stranded>`
      and its lookups gain `Res<GameClock>` (bundle it into `HaulLookups`
      rather than adding a bare system parameter — the argument count is
      already at clippy's threshold and that bundle is exactly this kind of
      read-only reference data). The no-route branch inserts only when the
      worker did not already carry the marker, so `since` survives a
      continuing stranding. Both clear sites are unchanged.
- [ ] Write `Game::note_strandings(&mut self)` in `game/memories.rs`: every
      entity carrying `Stranded` whose `since` equals the current
      `GameClock` tick remembers `stranded_at` about its own `Position`, as
      `MemorySubject::BaseTile`. Collect the entities before writing —
      `remember` takes `&mut self`.
- [ ] Call it from `tick_inner` immediately after `self.schedule.run(&mut
      self.world)`, where `haul_step_system` has just run and its commands
      have flushed, and before the clock increments at the bottom of the
      tick. Comment why the position in the tick is load-bearing: one tick
      later the edge is gone.

**Tests** (`tests/memories.rs`):

- [ ] A worker whose route is cut remembers `stranded_at` about the tile it
      is standing on, and the coordinates are the worker's own rather than
      its machine's. Assert both — the "a memory formed" half alone passes
      against either reading.
- [ ] **Ticking again while still stranded adds no second strike.** This is
      the edge rule and the whole reason `since` exists. Run several ticks
      and assert one entry at one strike.
- [ ] A stranding repaired and then broken again earns a second strike, so
      the edge rule has not simply frozen the memory after one episode.
- [ ] A worker walking normally to its post remembers nothing.
- [ ] `Stranded`'s `since` is not written into the save: a save→load leaves
      no marker, and the load does not mint a memory.

**Gate:** `cargo test -p feral-processes-engine memories hauling`

---

## Task 5 — The census, and the whole gate

**Files:** `tests/assets.rs`, `tests/memories.rs`

- [ ] Extend `tests/assets.rs` with the spec's remaining census clause:
      every shipped def's declared `subject` kind is reachable from a real
      `remember` call site. Spell the four triggers as a table in the test —
      id against the `MemorySubjectKind` the trigger actually writes — and
      assert that every def in the shipped catalogue appears in it and that
      its declared subject matches. A def no trigger can satisfy is dead
      content whose every write would be refused, and nothing else in the
      build would say so.
- [ ] The census must fail if a fifth `.ron` file is added without a
      trigger. Prove it by adding one temporarily and watching it redden.
- [ ] The empty-database property, end to end and at the trigger level: with
      an empty `MemoryDb` installed, a maul, a won fight and a stranding all
      run and leave every store empty. Phase 2 asserted this of `remember`;
      this asserts it of the four hooks, which is where it can now lapse.
- [ ] Full gate: `cargo fmt`, `cargo clippy --workspace`, `cargo test
      --workspace`. Then `cargo test -p feral-processes-engine balance_sim`
      and state plainly that the curves did not move — this feature is meant
      to be invisible to that gate.
- [ ] Record the mutation table for every test in tasks 1–5: the mutation
      applied, the failure seen, the restore confirmed.

---

## What "done" looks like

`Game::remember` has four callers in play. An owned program that has fought
beside another, been nearly killed, won something it should not have, or
been left in a corner nothing reaches, is carrying a store that says so —
and nothing anywhere reads it yet except a test. The screen is phase 4.
