# Tamed Program Build Requirement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every structure deploy and every tier upgrade consumes one tamed program, at depth `ZonePortal >= tier`, refundable only while the order is unbuilt.

**Architecture:** Deploys and upgrades already converge on one entity — a `BuildSite` request carrying a `BuildGoal`. The program attaches there, as a `CreatureSave` value snapshot, and the live entity is despawned. The two existing refund doors (`cancel_build_request`, `clear_pending_build_at`) rehydrate it; `consume_site` spends it simply by not doing so. A new `Mode::BuildProgram` picker is the last step of both flows.

**Tech Stack:** Rust, Bevy ECS (`World` used directly, no schedules), serde/RON saves, egui frontend behind a headless `app-core`.

**Spec:** `docs/superpowers/specs/2026-09-07-tamed-program-build-requirement-design.md`

## Global Constraints

- **The rule is a floor, not a match:** a program qualifies iff `ZonePortal >= tier`. A zone-5 program may raise a Mk1.
- **Required tier:** `BuildGoal::New` → `1`; `BuildGoal::Upgrade { to_tier }` → `to_tier`.
- **Home is exempt at every tier** — deploy and upgrade alike. `HOME_STRUCTURE_ID` never costs a program.
- **No `SAVE_FORMAT_VERSION` bump.** New save fields are additive behind `#[serde(default)]`, per the precedent stated on `BuildSiteSave::goal`.
- **A RON round-trip cannot catch a skipped field.** Any new save field needs a real save-to-file → load-from-file test.
- **Lowercase letters are row selectors.** The picker's rows are selectors; it needs no uppercase action. Any non-row action added anywhere must be UPPERCASE.
- **`ALL_MODES` is hand-written.** It is `[Mode; 101]` at `crates/gui/src/render/mod.rs:1370` and becomes `[Mode; 102]`. The draw match ends in `_ => {}`, so a new `Mode` that is not added to both **compiles clean and ships a blank screen**. Its length is a known semantic merge conflict — expect one.
- **Every test must fail with the change removed.** Delete the implementation and watch it go red before committing.
- **No occult naming** in any player-facing string (no daemon/ghost/wraith/phantom).
- **Do not update `docs/manual.md` or the root `README.md`.** `CHANGELOG.md` and `assets/*/README.md` schema docs still apply.
- **A build order may never take the base to zero programs.** The build crew *is* the roster: commit your last program and nothing can fetch materials, nothing can raise the site, and cancelling is the only exit. This is the deadlock `build_is_workable` exists to prevent, arriving by a new route. `place_structure` and `upgrade_structure` must refuse the last one.
- **Eligibility is more than depth.** Every owned program is in *some* role — there is no "owned but idle" state. A program that is wielded, away on a `Sortie`, `Downed`, or holding a `Carrying` load must not be offered: freeing a carrier destroys its goods, and despawning one is strictly worse than freeing it.
- **The filter is one engine-side derivation.** A read-only screen's row count is owned by app-core and drawn by gui, so the renderer must never filter the list a second time.
- **Everything returning to the roster goes through `roster_parts`** — the one barrier all four doors pass. A program that comes back around it is short components, remembers nothing, and nothing fails to compile.
- **Memory intensity is derived from `GameClock` on every read, never stored.** The snapshot must carry each entry's original timestamp; re-stamping on refund resets every decay curve and would *deepen* an old grudge.
- **`ProgramId` identity is load-bearing in both directions.** Minting a fresh id on refund orphans this program's memories *and* every other program's memories naming it as subject.
- **The program is charged at filing** — a deliberate exception to this seam's central rule, *"nothing is charged at filing"*. Say so in the code, or the next reader treats it as a bug.
- **Build/test command:** `cargo test --workspace`. Never pipe it to `tail`/`grep` — the exit code is lost through a pipe. Single-crate runs (`-p feral-processes-engine`) shift the RNG stream relative to `--workspace`; if a seeded test disagrees between them, that is why.

---

## File Structure

| File | Responsibility after this change |
|---|---|
| `crates/engine/src/game/lifecycle.rs` | Gains `creature_save_for` / `spawn_creature_from_save`; its two bulk loops become callers of them |
| `crates/engine/src/components.rs` | `BuildSite::program` — the committed snapshot |
| `crates/engine/src/save.rs` | `BuildSiteSave::program`, behind a serde default |
| `crates/engine/src/game/catalog.rs` | `program_tier_required`, `structure_needs_program` |
| `crates/engine/src/game/party.rs` | `programs_for_build`, `commit_program`, `refund_program` |
| `crates/engine/src/game/base/building.rs` | Commit at both filing doors; refund at both destruction doors |
| `crates/app-core/src/lib.rs` | `Mode::BuildProgram`, `pending_build` state |
| `crates/app-core/src/app/building.rs` | Picker key handling; both entrances rerouted through it |
| `crates/gui/src/render/building.rs` | Picker screen; menu refusal; per-row upgrade requirement |

---

## Task 1: Extract `creature_save_for` from the save loop

The commit step needs a `CreatureSave` for one live entity. Today that shape exists only inside a single large multi-component query at `lifecycle.rs:1708`. Writing a second builder would give the codebase two answers to "what is a creature", and the one nobody runs is the one that drifts.

**This is the highest-risk task in the plan.** It rewrites the most safety-critical loop in the engine. It is first so a reviewer can reject it on its own.

**Files:**
- Modify: `crates/engine/src/game/lifecycle.rs` (the creature-save query, around 1660–1795)
- Test: `crates/engine/src/tests/save_roundtrip.rs` (create if absent)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub(crate) fn creature_save_for(&mut self, e: Entity) -> Option<save::CreatureSave>` on `Game`. Returns `None` if `e` has no `Creature`.

- [ ] **Step 1: Write the characterisation test first — it must pass BEFORE the refactor**

This is a refactor, so the test is written against current behaviour and must stay green through it. It is the only thing standing between this change and a silent save regression.

```rust
// crates/engine/src/tests/save_roundtrip.rs
use crate::game::Game;

/// Saving, loading and saving again must produce byte-identical output.
///
/// This is the gate on `creature_save_for`: if the extracted per-creature
/// builder drops or reorders a field the bulk loop wrote, the second dump
/// differs from the first and this fails. Written before the extraction and
/// green on both sides of it.
#[test]
fn a_save_survives_a_load_and_saves_the_same() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = dir.path().join("a.ron");
    let second = dir.path().join("b.ron");

    let mut game = Game::new(20260907);
    // Give the roster something to lose: a tamed program with a level, a
    // name and gear is what makes this test non-vacuous.
    crate::tests::helpers::seed_rich_roster(&mut game);

    game.save_to_file(&first).expect("first save");
    let mut loaded = Game::load_from_file(&first).expect("load");
    loaded.save_to_file(&second).expect("second save");

    let a = std::fs::read_to_string(&first).expect("read a");
    let b = std::fs::read_to_string(&second).expect("read b");
    assert_eq!(a, b, "a load-then-save must reproduce the save it read");
}
```

If `crate::tests::helpers::seed_rich_roster` does not exist, write it in the same file: tame two programs via the existing test helper used by `crates/engine/src/tests/party.rs`, level one of them, equip it, and give it a `CustomName`. Do not invent an engine API for this — reuse whatever `tests/party.rs` already calls.

- [ ] **Step 2: Run it and confirm it passes on unmodified code**

Run: `cargo test -p feral-processes-engine a_save_survives_a_load_and_saves_the_same`
Expected: PASS. If it fails on unmodified code, **stop and report** — the save is already non-deterministic (bevy query order is not stable) and this plan's gate needs rethinking before any refactor lands.

- [ ] **Step 3: Add `creature_save_for`, built from per-entity component reads**

Add to the same `impl Game` block in `lifecycle.rs`. Every field comes from the existing push at `lifecycle.rs:1708` — copy them across verbatim rather than retyping from memory, including the `cronjob`, `nest_position` and `patrol_position` blocks above it and their doc comments.

```rust
/// One creature as the save format describes it, or `None` if `e` is not a
/// creature.
///
/// **The single answer to "what is a creature in a file".** Both the bulk
/// dump below and `Game::commit_program` — which snapshots one program onto
/// a build request — go through here. A second builder is the thing this
/// exists to prevent: the copy that drifts is the one nobody runs, which is
/// `BuildGoal`'s argument one subsystem over.
pub(crate) fn creature_save_for(&mut self, e: Entity) -> Option<save::CreatureSave> {
    let creature = self.world.get::<Creature>(e)?.clone();
    let pos = *self.world.get::<Position>(e)?;
    let stats = *self.world.get::<Stats>(e)?;
    // ... every remaining field, read with `self.world.get::<T>(e)`,
    // in the same order the bulk push writes them.
    Some(save::CreatureSave {
        species: creature.species,
        position: (pos.x, pos.y),
        hp: stats.hp,
        // ...
    })
}
```

- [ ] **Step 4: Replace the bulk loop body with a call to it**

The loop collects ids first, then maps — the borrow checker will not allow `&mut self` calls inside an open query iteration.

```rust
let ids: Vec<Entity> = {
    let mut q = self.world.query_filtered::<Entity, With<Creature>>();
    q.iter(&self.world).collect()
};
let creatures: Vec<save::CreatureSave> =
    ids.into_iter().filter_map(|e| self.creature_save_for(e)).collect();
```

Delete the old query and its push. Do **not** add a sort — the existing order is what the characterisation test locked in, and changing it is a separate decision.

- [ ] **Step 5: Run the full suite**

Run: `cargo test --workspace`
Expected: PASS, including `a_save_survives_a_load_and_saves_the_same`. Any save/load test that goes red here is a dropped field — find it, do not adjust the test.

- [ ] **Step 6: Prove the test is not vacuous**

Temporarily delete one field assignment from `creature_save_for` (e.g. `custom_name`), re-run the test, confirm it FAILS, then restore it.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/game/lifecycle.rs crates/engine/src/tests/
git commit -m "Extract creature_save_for from the bulk save loop"
```

---

## Task 2: Extract `spawn_creature_from_save` from the load loop

The refund needs to turn a snapshot back into a live program. The load loop already does this, threading two pieces of caller state through its iterations: `next_program_id` minting and `wielded` resolution. Both stay with the caller.

**Files:**
- Modify: `crates/engine/src/game/lifecycle.rs` (the creature-load loop, around 1060–1165)
- Test: `crates/engine/src/tests/save_roundtrip.rs`

**Interfaces:**
- Consumes: `save::CreatureSave` (Task 1).
- Produces: `pub(crate) fn spawn_creature_from_save(&mut self, c: &save::CreatureSave, next_program_id: &mut u32) -> Entity` on `Game`.

- [ ] **Step 1: Write the failing test**

```rust
/// A snapshot taken of a live program and spawned straight back must give
/// the same program — this is the refund path with the build order taken
/// out of the middle of it.
#[test]
fn a_creature_snapshot_round_trips_through_a_respawn() {
    let mut game = Game::new(20260907);
    let original = crate::tests::helpers::tame_one(&mut game);
    game.set_custom_name(original, "Bellwether").expect("name it");

    let snapshot = game.creature_save_for(original).expect("snapshot");
    game.world.despawn(original);

    let mut next_id = 99;
    let restored = game.spawn_creature_from_save(&snapshot, &mut next_id);

    assert_eq!(game.creature_label(restored), "Bellwether");
    assert_eq!(game.zone_tier(restored), snapshot.zone);
    assert_eq!(
        game.world.get::<ProgramId>(restored).map(|p| p.0),
        Some(snapshot.program_id),
        "a restored program keeps its own name, so its memories still find it",
    );
}
```

Use whatever the real rename API is called — check `Game::rename_companion`, referenced at `save.rs:371` — rather than inventing `set_custom_name`.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine a_creature_snapshot_round_trips_through_a_respawn`
Expected: FAIL — `no method named spawn_creature_from_save`.

- [ ] **Step 3: Extract the loop body**

Lift the body of the existing `for c in &save.creatures` loop into:

```rust
/// One creature from the save format back into the world.
///
/// **The single answer to "how a creature comes back".** `Game::load` runs
/// it per entry; `Game::refund_program` runs it once, to give back a program
/// a cancelled build order was holding.
///
/// `next_program_id` is the caller's, not this function's: minting is a
/// property of the whole load, not of one creature, and a refund has no
/// minting to do — the snapshot already carries the id it was known by.
pub(crate) fn spawn_creature_from_save(
    &mut self,
    c: &save::CreatureSave,
    next_program_id: &mut u32,
) -> Entity {
    // the existing body, verbatim
}
```

`wielded` stays in the caller: the loop keeps its `if c.wielded && wielded.is_none()` check against the returned entity.

**Two things this function must not skip.** It goes through `Game::roster_parts` for anything tamed — that is where `ProgramId` and the `Memories` store are minted, and a program that comes back around it silently remembers nothing (*"a `who` with no `Memories` is a no-op"*). And it restores each memory's original timestamp rather than re-stamping: intensity is derived from `GameClock` on every read, so a fresh stamp resets every decay curve and makes a refund *deepen* an old grudge.

- [ ] **Step 4: Make `Game::load` call it**

```rust
for c in &save.creatures {
    let e = self.spawn_creature_from_save(c, &mut next_program_id);
    if c.wielded && wielded.is_none() {
        wielded = Some(e);
    }
}
```

- [ ] **Step 5: Run both tests and the suite**

Run: `cargo test --workspace`
Expected: PASS, including Task 1's round-trip test.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/game/lifecycle.rs crates/engine/src/tests/
git commit -m "Extract spawn_creature_from_save from the load loop"
```

---

## Task 3: `BuildSite` carries a program, and the save carries it too

**Files:**
- Modify: `crates/engine/src/components.rs:2023` (`BuildSite`), `:2073` (`filed`)
- Modify: `crates/engine/src/save.rs:791` (`BuildSiteSave`)
- Modify: `crates/engine/src/game/lifecycle.rs:1853` (the `build_sites` push) and the matching load
- Test: `crates/engine/src/tests/save_roundtrip.rs`

**Interfaces:**
- Consumes: `creature_save_for` (Task 1).
- Produces: `BuildSite::program: Option<save::CreatureSave>`; `BuildSiteSave::program: Option<save::CreatureSave>`.

- [ ] **Step 1: Write the failing test**

```rust
/// The committed program must survive a real file round trip. A RON
/// round-trip test cannot catch a field that fails to serialise, so this
/// goes through the disk.
#[test]
fn a_build_sites_program_survives_a_save_and_load() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("s.ron");

    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    let program = crate::tests::helpers::tame_one(&mut game);
    let snapshot = game.creature_save_for(program).expect("snapshot");

    let site = game.world.spawn((
        BuildSite {
            program: Some(snapshot.clone()),
            ..BuildSite::new("fabricator".into(), vec![])
        },
        Position { x: 1, y: 1 },
    )).id();
    let _ = site;

    game.save_to_file(&path).expect("save");
    let loaded = Game::load_from_file(&path).expect("load");

    let sites: Vec<_> = loaded.build_site_programs();
    assert_eq!(sites.len(), 1, "the site came back");
    assert_eq!(
        sites[0].as_ref().map(|c| c.program_id),
        Some(snapshot.program_id),
        "and it is still holding the program it was given",
    );
}
```

Add `pub(crate) fn build_site_programs(&mut self) -> Vec<Option<save::CreatureSave>>` as a test-support accessor on `Game` beside the other view helpers if no equivalent exists.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine a_build_sites_program_survives_a_save_and_load`
Expected: FAIL — `BuildSite` has no field named `program`.

- [ ] **Step 3: Add the component field**

```rust
    /// The tamed program committed to this order, retired from the roster
    /// the moment the request was filed.
    ///
    /// A value and never an `Entity`: entity ids are not stable across a
    /// save round trip, which is why `HopperEntry` carries a
    /// `DownedProgram` rather than a reference to one. A whole
    /// `CreatureSave` rather than a narrower record because a refund owes
    /// the player back the program they had — gear, name, refactors and all
    /// — and a second description of a creature is one that drifts.
    ///
    /// `None` only for a Home, the one exempt structure, and for the
    /// hand-spawned sites in test fixtures.
    pub program: Option<crate::save::CreatureSave>,
```

Set `program: None` in `BuildSite::filed`. Leave `new` and `upgrade` signatures alone — the commit assigns the field after construction, so no call site changes.

- [ ] **Step 4: Add the save field**

```rust
    /// The tamed program this order is holding — see
    /// `components::BuildSite::program`.
    ///
    /// Additive behind `#[serde(default)]`, so it costs no
    /// `SAVE_FORMAT_VERSION` bump: a file written before builds cost
    /// programs loads every site holding nothing, which is exactly what
    /// that run had.
    #[serde(default)]
    pub program: Option<CreatureSave>,
```

- [ ] **Step 5: Wire both directions**

In the `build_sites` push (`lifecycle.rs:1853`) add `program: site.program.clone(),`. In the matching load, add `program: s.program.clone(),` to the `BuildSite` it constructs.

- [ ] **Step 6: Run the test and the suite**

Run: `cargo test --workspace`
Expected: PASS. `SAVE_FORMAT_VERSION` must still read 32 — if anything made you bump it, the field is not actually defaulted.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/components.rs crates/engine/src/save.rs crates/engine/src/game/lifecycle.rs crates/engine/src/tests/
git commit -m "A build request can hold the program committed to it"
```

---

## Task 4: The rule

**Files:**
- Modify: `crates/engine/src/game/catalog.rs`
- Modify: `crates/engine/src/game/party.rs`
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Produces:
  - `pub fn program_tier_required(goal: BuildGoal) -> u32` (free function or associated fn on `BuildGoal`)
  - `pub(crate) fn structure_needs_program(&self, id: &StructureId) -> bool` — false for `HOME_STRUCTURE_ID`, true otherwise
  - `pub fn programs_for_build(&mut self, tier: u32) -> Vec<PetInfo>`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_tier_a_goal_demands_is_its_own_tier() {
    assert_eq!(program_tier_required(BuildGoal::New), 1);
    assert_eq!(program_tier_required(BuildGoal::Upgrade { to_tier: 3 }), 3);
}

#[test]
fn a_deeper_program_still_qualifies_for_a_shallow_build() {
    let mut game = Game::new(20260907);
    let deep = crate::tests::helpers::tame_at_zone(&mut game, 5);
    let eligible = game.programs_for_build(1);
    assert!(
        eligible.iter().any(|p| p.entity == deep),
        "zone >= tier is a floor, not a match — a zone 5 program may raise a Mk1",
    );
}

#[test]
fn a_shallow_program_does_not_qualify_for_a_deep_upgrade() {
    let mut game = Game::new(20260907);
    let shallow = crate::tests::helpers::tame_at_zone(&mut game, 2);
    let eligible = game.programs_for_build(3);
    assert!(!eligible.iter().any(|p| p.entity == shallow));
}

#[test]
fn home_is_the_one_structure_that_needs_no_program() {
    let game = Game::new(20260907);
    assert!(!game.structure_needs_program(&HOME_STRUCTURE_ID.into()));
    assert!(game.structure_needs_program(&"fabricator".into()));
}
```

`tame_at_zone` sets `ZonePortal(n)` on a freshly tamed program; write it in the test helpers if absent.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine program_tier_required programs_for_build structure_needs_program`
Expected: FAIL — functions not found.

- [ ] **Step 3: Implement**

```rust
/// The `StructureTier` a build request will produce, and so the depth of
/// program it demands.
///
/// One number decides the whole rule. `upgrade_ceiling` already clamps a
/// tier to `ZoneLevel`, so a requirement this returns is always one the
/// player could have met — the two rules agree by construction, and
/// `the_upgrade_ceiling_keeps_the_program_rule_satisfiable` says so.
pub fn program_tier_required(goal: BuildGoal) -> u32 {
    match goal {
        BuildGoal::New => 1,
        BuildGoal::Upgrade { to_tier } => to_tier,
    }
}
```

```rust
/// Every owned program that could be spent on a build of `tier`, in
/// `owned_pets` order so the picker agrees with every other roster screen.
///
/// `>=` and never `==`: a run whose roster has outgrown zone 1 must still
/// be able to raise a Mk1, or deep play locks itself out of building.
///
/// **Depth is not the whole filter.** A program's role is derived and there
/// is no "owned but idle" state, so an unfiltered list offers the weapon in
/// the player's hand and a body that is halfway across a sortie. Each
/// exclusion below is for its own reason, and none is cosmetic:
/// a carrier's goods are destroyed by freeing it, let alone despawning it;
/// a sortied program is away and cannot be reached; a `Downed` one is the
/// roster slot a wipe is supposed to cost.
///
/// **The only derivation of this list.** The renderer draws what app-core
/// counts and filters nothing itself.
pub fn programs_for_build(&mut self, tier: u32) -> Vec<PetInfo> {
    let candidates = self.owned_pets();
    candidates
        .into_iter()
        .filter(|p| self.zone_tier(p.entity) >= tier)
        .filter(|p| !self.is_wielded(p.entity))
        .filter(|p| self.world.get::<Sortie>(p.entity).is_none())
        .filter(|p| self.world.get::<Downed>(p.entity).is_none())
        .filter(|p| self.world.get::<Carrying>(p.entity).is_none())
        .collect()
}
```

`owned_pets` takes `&mut self`, so it is collected before filtering. Use the real accessor names — check `Game::program_role` and `ProgramRole`, which already partition the roster, and prefer asking it over four component probes if it answers cleanly.

- [ ] **Step 3b: Test each exclusion separately**

Four assertions in one test is one failure message for four different bugs. One test each:

```rust
#[test]
fn the_wielded_program_is_not_offered_to_a_build() {
    let mut game = Game::new(20260907);
    let p = crate::tests::helpers::tame_one(&mut game);
    game.wield_program(p).expect("wielded");
    assert!(game.programs_for_build(1).is_empty(), "you cannot build with the thing in your hand");
}

#[test]
fn a_program_carrying_goods_is_not_offered_to_a_build() {
    let mut game = Game::new(20260907);
    let p = crate::tests::helpers::tame_one(&mut game);
    crate::tests::helpers::give_carrying(&mut game, p);
    assert!(game.programs_for_build(1).is_empty(), "despawning a carrier destroys its load");
}
```

Write the same shape for `Sortie` and `Downed`.

- [ ] **Step 4: Add the agreement test**

```rust
/// The program rule may never demand a depth the tier ceiling would not
/// have let the player reach. If `upgrade_ceiling` ever loosens, this is
/// what says so out loud instead of leaving an unsatisfiable upgrade.
#[test]
fn the_upgrade_ceiling_keeps_the_program_rule_satisfiable() {
    for zone in 1..=5u32 {
        let ceiling = zone; // upgrade_ceiling = min(max_tier, ZoneLevel)
        assert!(
            program_tier_required(BuildGoal::Upgrade { to_tier: ceiling }) <= zone,
            "a Mk{ceiling} upgrade offered in zone {zone} must not need a deeper program",
        );
    }
}
```

- [ ] **Step 5: Run and commit**

Run: `cargo test --workspace`

```bash
git add crates/engine/src/game/catalog.rs crates/engine/src/game/party.rs crates/engine/src/tests/
git commit -m "The rule: a build of tier N needs a program of zone N or deeper"
```

---

## Task 5: Commit and refund, as one pair

Both directions land together because a commit with no refund is a rob, and neither is testable without the other.

**Files:**
- Modify: `crates/engine/src/game/party.rs` (the two doors)
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Consumes: `creature_save_for`, `spawn_creature_from_save`, `BuildSite::program`.
- Produces:
  - `pub(crate) fn commit_program(&mut self, e: Entity) -> Option<save::CreatureSave>`
  - `pub(crate) fn refund_program(&mut self, c: &save::CreatureSave) -> Entity`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn committing_a_program_takes_it_off_the_roster() {
    let mut game = Game::new(20260907);
    let p = crate::tests::helpers::tame_one(&mut game);
    let before = game.owned_pets().len();

    let snapshot = game.commit_program(p).expect("committed");

    assert_eq!(game.owned_pets().len(), before - 1);
    assert!(game.world.get_entity(p).is_err(), "the entity is consumed");
    assert_eq!(snapshot.tamed, true);
}

#[test]
fn committing_a_party_member_clears_its_slot() {
    let mut game = Game::new(20260907);
    let p = crate::tests::helpers::tame_one(&mut game);
    game.add_to_party(p).expect("in the party");

    game.commit_program(p).expect("committed");

    assert!(!game.party_entities().contains(&p), "no slot points at a dead entity");
}

#[test]
fn committing_the_wielded_program_unwields_it() {
    let mut game = Game::new(20260907);
    let p = crate::tests::helpers::tame_one(&mut game);
    game.wield_program(p).expect("wielded");

    game.commit_program(p).expect("committed");

    assert!(game.wielded_program().is_none(), "the run does not wield a ghost slot");
}

#[test]
fn a_refunded_program_comes_back_whole() {
    let mut game = Game::new(20260907);
    let p = crate::tests::helpers::tame_one(&mut game);
    game.rename_companion(p, "Bellwether").expect("named");
    let id = game.world.get::<ProgramId>(p).unwrap().0;

    let snapshot = game.commit_program(p).expect("committed");
    let back = game.refund_program(&snapshot);

    assert_eq!(game.creature_label(back), "Bellwether");
    assert_eq!(game.world.get::<ProgramId>(back).unwrap().0, id);
    assert_eq!(game.owned_pets().len(), 1);
}
```

Use the real names for wield/party accessors — check `crates/engine/src/game/party.rs` rather than trusting `wield_program` / `party_entities` / `wielded_program` above; substitute whatever exists and keep the assertions.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine commit_program refund_program`
Expected: FAIL — methods not found.

- [ ] **Step 3: Implement, following fusion's teardown**

```rust
/// Retires `e` from the roster and hands back what it was, so a cancelled
/// order can give it home again.
///
/// Follows `fuse_companions`' teardown — retain out of `Party`, then
/// despawn — and closes the two loose ends a fusion sacrifice does not
/// normally have: a committed program may be the wielded one, and may be
/// holding a standing job.
pub(crate) fn commit_program(&mut self, e: Entity) -> Option<save::CreatureSave> {
    let snapshot = self.creature_save_for(e)?;
    self.world.resource_mut::<Party>().0.retain(|&x| x != e);
    // Clear the wield before the despawn, or the run wields a dead entity.
    if self.wielded_entity() == Some(e) {
        self.clear_wielded();
    }
    // A posted body evicted mid-tick leaves a machine pointing at a dead
    // entity, and an `OffShift` or `idled_with` marker naming it outlives
    // it into the save as a ghost. Clear the post before the despawn.
    self.release_post(e);
    self.world.despawn(e);
    Some(snapshot)
}

/// Puts a committed program back on the roster exactly as it left.
///
/// Through `spawn_creature_from_save` and so through `roster_parts`, the
/// one barrier every door into the roster passes. The snapshot's own
/// `ProgramId` is kept rather than reissued: memories are keyed to it, and
/// a program that came back under a new name would find none of its own.
pub(crate) fn refund_program(&mut self, c: &save::CreatureSave) -> Entity {
    // Nothing to mint — the snapshot carries the id it was known by.
    let mut unused = 0;
    self.spawn_creature_from_save(c, &mut unused)
}
```

If `wielded_entity` / `clear_wielded` / `release_post` do not exist under those names, find the real ones — `CreatureSave::wielded` names the wield concept, and `displace_task_holder` / `schedule_base_labour` name the posting one. `programs_for_build` already excludes wielded and carrying programs, so `commit_program`'s handling of them is a belt-and-braces second line, not the primary guard: a second frontend calling the engine directly must not be able to destroy a carrier's load.

- [ ] **Step 4: Run and prove non-vacuity**

Run: `cargo test --workspace`
Then delete the `Party::retain` line, confirm `committing_a_party_member_clears_its_slot` FAILS, and restore it.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/party.rs crates/engine/src/tests/
git commit -m "Commit a program to a build, and give it back"
```

---

## Task 6: Both filing doors take a program

**Files:**
- Modify: `crates/engine/src/game/base/building.rs:11` (`place_structure`), `:568` (`upgrade_structure`)
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Consumes: `commit_program`, `program_tier_required`, `structure_needs_program`.
- Produces: `place_structure` and `upgrade_structure` each gain a trailing `program: Option<Entity>` parameter.

Adding a parameter rather than a second method: a `place_structure_with_program` beside `place_structure` is two doors into one rule, and the one nobody updates is the one that skips the cost.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn deploying_without_a_program_is_refused_and_says_why() {
    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    let err = game.place_structure("fabricator", 1, 0, None).unwrap_err();
    assert!(err.contains("program"), "the refusal names what is missing: {err}");
}

#[test]
fn deploying_with_a_program_files_the_order_and_spends_it() {
    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    let p = crate::tests::helpers::tame_one(&mut game);

    game.place_structure("fabricator", 1, 0, Some(p)).expect("filed");

    assert_eq!(game.owned_pets().len(), 0, "the program left the roster");
    assert!(game.build_site_programs()[0].is_some(), "the order is holding it");
}

#[test]
fn founding_a_home_needs_no_program() {
    let mut game = Game::new(20260907);
    game.place_structure(HOME_STRUCTURE_ID, 0, 0, None).expect("Home is exempt");
}

#[test]
fn upgrading_a_home_needs_no_program_either() {
    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    crate::tests::helpers::breach_to_zone(&mut game, 2);
    let home = game.structure_entity(HOME_STRUCTURE_ID).expect("a Home");
    game.upgrade_structure(home, None).expect("Home is exempt at every tier");
}

#[test]
fn a_mk3_upgrade_refuses_a_zone_2_program() {
    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    crate::tests::helpers::breach_to_zone(&mut game, 3);
    let fab = crate::tests::helpers::standing_structure_at_tier(&mut game, "fabricator", 2);
    let shallow = crate::tests::helpers::tame_at_zone(&mut game, 2);

    let err = game.upgrade_structure(fab, Some(shallow)).unwrap_err();
    assert!(err.contains("zone 3"), "the refusal names the depth needed: {err}");
    assert_eq!(game.owned_pets().len(), 1, "a refused upgrade spends nothing");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine deploying_ founding_a_home upgrading_a_home a_mk3_upgrade`
Expected: FAIL — arity mismatch on `place_structure`.

- [ ] **Step 3: Implement in `place_structure`**

The check goes **after** the existing gates and **immediately before** the site spawn, so a player who names an unreachable cell is told about the cell, not about a program they were never going to spend. Home returns before reaching it.

```rust
// Resolved here and not at the picker: the picker is a frontend, and a
// rule only a frontend enforces is one a second frontend skips.
let committed = if self.structure_needs_program(&def.id) {
    let tier = program_tier_required(BuildGoal::New);
    let Some(chosen) = program else {
        return Err(format!(
            "Deploying a {} costs a tamed program. Pick one first.",
            def.name
        ));
    };
    if self.zone_tier(chosen) < tier {
        return Err(format!(
            "That program is from zone {}. A {} needs one from zone {tier} or deeper.",
            self.zone_tier(chosen),
            def.name
        ));
    }
    self.commit_program(chosen)
} else {
    None
};
```

Then set `program: committed` on the spawned `BuildSite`, and extend the log line: `"You mark out a {} here, and commit {} to it. Your crew will raise it."` — the second `{}` being the label read **before** the commit despawns the entity.

- [ ] **Step 3b: Refuse the last program**

```rust
/// A base whose whole crew is one program cannot spend it: nothing would
/// be left to fetch the materials or raise the site, and the order could
/// never finish. `build_is_workable`'s deadlock, reached from the other
/// side.
if self.programs_for_build(tier).len() <= 1 && self.owned_pets().len() <= 1 {
    return Err(format!(
        "Committing your last program would leave nobody to build the {}.",
        def.name
    ));
}
```

Verify the premise before trusting this shape: confirm in `run_build_crew` / `build_is_workable` that the player themselves cannot raise a site. If the player *can* build alone, this refusal is wrong and should be dropped — **report that finding rather than guessing**.

```rust
#[test]
fn a_one_program_base_may_not_spend_its_only_body() {
    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    let only = crate::tests::helpers::tame_one(&mut game);
    let err = game.place_structure("fabricator", 1, 0, Some(only)).unwrap_err();
    assert!(err.contains("last program"), "{err}");
    assert_eq!(game.owned_pets().len(), 1, "and it is still on the roster");
}
```

- [ ] **Step 3c: Keep the refusal order**

`place_structure` and `upgrade_structure` each answer their refusals in a fixed order, and the tier ceiling is checked before the materials. The program refusals take a defined position in that order — after the ceiling and the occupancy checks, before the site spawn — rather than landing wherever the picker happens to return. A player standing in the wrong place must be told about the place.

- [ ] **Step 4: Implement the same in `upgrade_structure`**

Same shape, with `program_tier_required(BuildGoal::Upgrade { to_tier: next })`, placed after the existing `build_site_at` refusal. Home is exempt here too — `structure_needs_program(&kind)` answers it.

- [ ] **Step 5: Update every existing call site**

Check `count_build_requests` while you are here: it counts `BuildGoal::New` only, so a pending upgrade does not eat a `max_deployed` slot. Anything that counts *committed programs* must do the opposite and count **both** goals, or a pending upgrade's program is double-spendable.

`rg -n "place_structure\(|upgrade_structure\(" --type rust` and pass `None` for Home, `Some(..)` elsewhere. Many engine tests will need a program tamed first; that is the feature, not a test defect.

- [ ] **Step 6: Run and commit**

Run: `cargo test --workspace`

```bash
git add crates/engine/src/game/base/building.rs crates/engine/src/tests/
git commit -m "A build order and an upgrade each cost a tamed program"
```

---

## Task 7: Both destruction doors give it back

`clear_pending_build_at` warns in its own doc that *"nothing fails to compile when only one is done."* That applies exactly here.

**Files:**
- Modify: `crates/engine/src/game/base/building.rs:356` (`cancel_build_request`), `:648` (`clear_pending_build_at`)
- Test: `crates/engine/src/tests/building.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn calling_off_an_order_gives_the_program_back() {
    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    let p = crate::tests::helpers::tame_one(&mut game);
    game.rename_companion(p, "Bellwether").expect("named");
    game.place_structure("fabricator", 1, 0, Some(p)).expect("filed");

    let site = game.build_site_at(1, 0).expect("a site");
    game.cancel_build_request(site).expect("cancelled");

    let back = game.owned_pets();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].name, "Bellwether");
}

#[test]
fn a_site_wiped_with_its_cell_gives_the_program_back_too() {
    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    let p = crate::tests::helpers::tame_one(&mut game);
    game.place_structure("fabricator", 1, 0, Some(p)).expect("filed");

    game.clear_pending_build_at(1, 0);

    assert_eq!(game.owned_pets().len(), 1, "the second door refunds like the first");
}

#[test]
fn a_finished_structure_never_gives_the_program_back() {
    let mut game = Game::new(20260907);
    crate::tests::helpers::found_a_base(&mut game);
    let p = crate::tests::helpers::tame_one(&mut game);
    game.place_structure("fabricator", 1, 0, Some(p)).expect("filed");
    crate::tests::helpers::finish_the_build_at(&mut game, 1, 0);
    assert_eq!(game.owned_pets().len(), 0, "spent at completion");

    let fab = game.structure_at(1, 0).expect("it stands");
    game.remove_structure(fab).expect("demolished");

    assert_eq!(game.owned_pets().len(), 0, "deconstruction returns materials, not programs");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine calling_off_an_order a_site_wiped a_finished_structure`
Expected: FAIL — the roster stays empty after a cancel.

- [ ] **Step 3: Implement one helper, called from both doors**

```rust
/// Hands back whatever a dying build request was holding — materials and,
/// if it had one, the program committed to it.
///
/// **Both destruction paths call this.** `cancel_build_request` is the
/// player calling the job off; `clear_pending_build_at` is the cell going
/// out from under it. Wired into one alone, the other quietly eats a
/// program, and nothing fails to compile.
fn return_build_holdings(&mut self, build: &BuildSite) {
    for (item, qty) in &build.delivered {
        self.return_material(item, *qty);
    }
    if let Some(program) = build.program.clone() {
        let back = self.refund_program(&program);
        let name = self.creature_label(back);
        self.log_base(format!("{name} comes back off the job."));
    }
}
```

Replace the `for (item, qty) in &build.delivered` loop in **both** functions with `self.return_build_holdings(&build);`.

`consume_site` needs no change: it despawns the site, and the snapshot goes with it. That is what "spent" means.

- [ ] **Step 4: Prove the second door is really wired**

Delete the `return_build_holdings` call from `clear_pending_build_at`, confirm `a_site_wiped_with_its_cell_gives_the_program_back_too` FAILS, restore it.

- [ ] **Step 5: Run and commit**

Run: `cargo test --workspace`

```bash
git add crates/engine/src/game/base/building.rs crates/engine/src/tests/
git commit -m "Both destruction doors give a committed program back"
```

---

## Task 8: The picker, headless

**Files:**
- Modify: `crates/app-core/src/lib.rs:1226` (`Mode`), `:1986` (pending state)
- Modify: `crates/app-core/src/app/building.rs:229` (`handle_build_key`), `:242` (direction), `:365` (upgrade)
- Modify: `crates/app-core/src/app/input.rs:217` (dispatch)
- Test: `crates/app-core/src/tests/`

**Interfaces:**
- Consumes: `programs_for_build`, `place_structure(.., Option<Entity>)`, `upgrade_structure(.., Option<Entity>)`.
- Produces: `Mode::BuildProgram`; `App::pending_build: Option<PendingBuild>` where

```rust
/// The order being assembled, held between the direction step and the
/// program picker that confirms it.
pub enum PendingBuild {
    Deploy { structure: String, dx: i32, dy: i32 },
    Upgrade { structure: Entity, to_tier: u32 },
}
```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn choosing_a_direction_asks_which_program_to_spend() {
    let mut app = crate::tests::helpers::app_in_base_with_a_program();
    app.mode = Mode::Build;
    app.handle_key(GameKey::Char('a'));      // first structure
    app.handle_key(GameKey::Right);          // direction
    assert_eq!(app.mode, Mode::BuildProgram, "the picker is the confirm step");
    assert!(app.game.as_ref().unwrap().build_site_at(1, 0).is_none(),
            "nothing is filed until a program is picked");
}

#[test]
fn picking_a_program_files_the_order() {
    let mut app = crate::tests::helpers::app_in_base_with_a_program();
    app.mode = Mode::Build;
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char('a'));      // first program
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.game.as_ref().unwrap().build_site_at(1, 0).is_some());
}

#[test]
fn escaping_the_picker_files_nothing_and_spends_nothing() {
    let mut app = crate::tests::helpers::app_in_base_with_a_program();
    app.mode = Mode::Build;
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Esc);
    assert!(app.pending_build.is_none());
    assert_eq!(app.game.as_mut().unwrap().owned_pets().len(), 1);
}

#[test]
fn the_picker_lists_only_programs_deep_enough() {
    let mut app = crate::tests::helpers::app_in_base(); // no programs yet
    let game = app.game.as_mut().unwrap();
    crate::tests::helpers::tame_at_zone(game, 1);
    crate::tests::helpers::tame_at_zone(game, 3);
    assert_eq!(game.programs_for_build(3).len(), 1, "only the deep one");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-app-core choosing_a_direction picking_a_program escaping_the_picker`
Expected: FAIL — no `Mode::BuildProgram`.

- [ ] **Step 3: Add the mode, the state, and the handler**

```rust
pub(crate) fn handle_build_program_key(&mut self, key: GameKey) {
    if key == GameKey::Esc {
        // Nothing was spent — the commit happens in the engine, on confirm.
        self.pending_build = None;
        self.close_screen();
        return;
    }
    let Some(pending) = self.pending_build.clone() else {
        self.mode = Mode::Playing;
        return;
    };
    let tier = match &pending {
        PendingBuild::Deploy { .. } => 1,
        PendingBuild::Upgrade { to_tier, .. } => *to_tier,
    };
    let Some(game) = &mut self.game else { return };
    let candidates = game.programs_for_build(tier);
    let Some(idx) = self.selected_index(key, candidates.len()) else { return };
    let chosen = candidates[idx].entity;
    let outcome = match pending {
        PendingBuild::Deploy { structure, dx, dy } => {
            game.place_structure(&structure, dx, dy, Some(chosen))
        }
        PendingBuild::Upgrade { structure, .. } => {
            game.upgrade_structure(structure, Some(chosen))
        }
    };
    self.report(outcome);
    self.pending_build = None;
    self.mode = Mode::Playing;
}
```

`handle_build_direction_key` now sets `pending_build` and moves to `Mode::BuildProgram` instead of calling `place_structure`. `handle_upgrade_key` (`building.rs:365`) does the same instead of calling `upgrade_structure`.

Home keeps the old direct path: `structure == HOME_STRUCTURE_ID` calls `place_structure(.., None)` from the direction step and never reaches the picker.

- [ ] **Step 4: Add the dispatch entry**

`crates/app-core/src/app/input.rs`: `Mode::BuildProgram => self.handle_build_program_key(key),`

- [ ] **Step 5: Run and commit**

Run: `cargo test --workspace`

```bash
git add crates/app-core/src/
git commit -m "The build flow asks which program to spend"
```

---

## Task 9: The picker on screen

**Files:**
- Modify: `crates/gui/src/render/mod.rs:1370` (`ALL_MODES`) and the draw match
- Modify: `crates/gui/src/render/building.rs:40` (`build_menu_rows`), and the upgrade rows
- Test: `crates/gui/src/tests/`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn the_build_program_picker_draws_rows() {
    let mut app = crate::tests::helpers::app_in_base_with_a_program();
    app.mode = Mode::BuildProgram;
    app.pending_build = Some(PendingBuild::Deploy {
        structure: "fabricator".into(), dx: 1, dy: 0,
    });
    let text = crate::tests::helpers::render_to_text(&mut app);
    assert!(text.contains("HP"), "the roster's own row format, not a bare name list");
    assert!(!text.trim().is_empty(), "a mode missing from ALL_MODES draws blank");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p feral-processes-gui the_build_program_picker_draws_rows`
Expected: FAIL — blank screen, because the mode is not in the draw match.

- [ ] **Step 3: Draw it**

Build rows with `companion_row_lines` (`crates/gui/src/render/party.rs:243`) over `game.programs_for_build(tier)`, and pass them to `draw_popup` with title `"Commit a program"` and a prompt naming the structure and the word **permanently**. Follow `draw_structure_menu` (`building.rs:586`) for the shape.

Refusal when the list is empty: `"Nothing on your roster is from zone {tier} or deeper."`

- [ ] **Step 3b: Draw the refusal exactly once**

A refusal is one sentence on two surfaces and `App::refuse` is the one door. The census `every_screen_draws_a_refusal_exactly_once` drives every `Mode` through `draw` and counts what was painted — the new mode joins it and must paint exactly one. The refusal line is counted by `popup_layout` and is **not** a `Row`, so the panel grows a line rather than covering one and the index a keypress maps to does not move.

If the picker takes horizontal arrows, name the mode in `App::handle_key`'s modifier fold, or its four modified arrows are plain steps — a feel bug no test of the handler alone can see.

- [ ] **Step 4: Add to `ALL_MODES` and bump its length**

`[Mode; 101]` → `[Mode; 102]`, with `Mode::BuildProgram` beside `Mode::BuildDirection`. Expect a merge conflict on that number.

- [ ] **Step 5: Grey the menus**

- `build_menu_rows`: when `game.owned_pets().is_empty()`, add one refusal line — `"Deploying costs a tamed program. You have none."` — and dim the rows together. The deploy requirement is uniform, so this is not a per-row tag.
- Upgrade rows: per-row, appending `" · needs a zone {n} program"` and dimming rows whose depth the roster cannot meet.

Grey, never hide: a structure vanishing from a menu reads as a bug.

- [ ] **Step 6: Run and commit**

Run: `cargo test --workspace`

```bash
git add crates/gui/src/
git commit -m "Draw the program picker, and say what a build will cost"
```

---

## Task 10: Docs

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `.claude/skills/seams/references/base.md`, `docs/seams.md`, `CLAUDE.md`

- [ ] **Step 1: CHANGELOG entry**

Under a new version heading: what changed, in the player's words — structures cost a tamed program, upgrades cost a deeper one, Home is free, calling off an order gives the program back.

- [ ] **Step 2: The new seam, three writes in order**

A seam is three writes and the order matters:
1. **Argument** → `docs/seams.md`, a new `###` section: why the program attaches to `BuildSite` rather than to the structure, and why both destruction doors must refund.
2. **Trap** → `.claude/skills/seams/references/base.md`, house style: bold rule sentence, then the trap.
3. **Rule** → `CLAUDE.md`, under the same heading, **exactly one sentence**.

Suggested rule sentence: *"A build request holds the program committed to it, and every door that destroys a request unfinished must give it back."*

The argument in `docs/seams.md` must also record the **exception**: this seam's central rule is *"nothing is charged at filing"*, and the program is the one thing that is. Unwritten, the next reader reads it as a bug and fixes it.

`CLAUDE.md` is gitignored, so this write does not ride the branch — do it in the primary checkout, or tell the user it is outstanding.

- [ ] **Step 2b: Correct the three claims this change falsifies**

Each is a statement in code or docs that was true before this branch:

1. **`end_battle` is the only legal removal point** for a benched program's roster slot. Committing one is now a second, and cancelling an order is a new way a slot is *taken*.
2. **`add_companion`'s refusal names the two things that free a slot** (selling, extracting a routine). There are now more; update the sentence.
3. **`views::BuildOrderRow` is the one derivation of what a request looks like.** If the committed program is to show on a request anywhere — map, examine line, `build_order_report` — it is derived there once, never stored beside the site for display.

- [ ] **Step 3: Grep for claims this falsifies**

`rg -n "build" docs/*.md` — any doc saying structures cost only materials is now wrong. Do **not** touch `docs/manual.md` or the root `README.md`.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md docs/
git commit -m "Document the program cost of building"
```

---

## Verification before completion

- [ ] `cargo test --workspace` passes, unpiped, and the count is reported.
- [ ] `SAVE_FORMAT_VERSION` still reads 32.
- [ ] A save written before this branch still loads.
- [ ] Each new test has been shown to fail with its implementation removed.
- [ ] The base still functions with one program committed and one remaining — check the work-order header, which reads from a `LabourDemand` cached *before* the cut and says nothing at zero staff.
- [ ] **This has not been played.** A green suite is not evidence of play, and the picker is a screen no agent here can see. Say so plainly to the user, once.
