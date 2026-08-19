# Environment Effects, Phase 1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the surface ground that has a name and, past zone 1, does
something to you when you walk on it.

**Architecture:** Effects are keyed to the biome and loaded from
`assets/environment/*.ron` by a new `EnvironmentDb`, on the same
`load_dir`-with-warnings pattern every other asset db follows. One reader,
`Game::ground_effect`, holds the zone-1 gate. One hook, in
`Game::move_player`'s `walkable` branch ahead of `maybe_ambush`, applies it.
Nothing is stored and no save field is added.

**Tech Stack:** Rust, `bevy_ecs` resources, `ron` for assets and saves,
`serde`.

**Spec:** `docs/superpowers/specs/2026-08-19-environment-effects-design.md` —
read it first. It carries the argument for every decision below; this plan
carries only the work.

## Global Constraints

Every task's requirements implicitly include these.

- **Follow CLAUDE.md.** In particular: no hardcoded content that could be
  data, `tuning.rs` for every magnitude, comments explain *why*, and no
  backwards-compat shims.
- **TDD.** Failing test first, watch it fail, minimal implementation, watch
  it pass, commit. A bug fix gets its reproducer first.
- **Gates per task:** `cargo test -p feral-processes-engine <name>` while
  iterating; `cargo fmt` and `cargo clippy --workspace` before every commit,
  fixing warnings rather than silencing them. `cargo test --workspace` is the
  gate at the end of each task, not only at the end of the plan.
- **No `SAVE_FORMAT_VERSION` bump anywhere in this phase.** If a task seems
  to need one, stop — something has diverged from the spec.
- **No version bump and no `CHANGELOG.md` section on this branch.** Commits
  on a branch stay unversioned; the bump, the changelog section and the tag
  happen once at the merge.
- **Zone 1 takes no effects but still gets biome names.**
- **Terrain never costs Power and never raises Trace.**
- **The player alone** takes environment damage — never the party.
- **Deleting `assets/environment/` must restore today's game exactly**, the
  same supported way deleting `assets/sectors/` already is.
- Do not run the game to verify; the user is remote and cannot play. Evidence
  is test output.

## File Structure

**Created**

- `crates/engine/src/environment.rs` — `EnvironmentDef`, `EnvironmentEffect`,
  `EnvironmentDb` and its `load_dir` validation. Data and loading only; knows
  nothing about a `Game`.
- `crates/engine/src/game/environment.rs` — `Game::ground_effect`, the single
  reader. Kept apart from the db so phase 2's weather draw has an obvious
  home beside it.
- `crates/engine/src/tests/environment.rs` — loading, validation and
  `ground_effect` resolution.
- `assets/environment/README.md` — schema reference, on the pattern of
  `assets/sectors/README.md`.
- `assets/environment/*.ron` — the shipped effects (Task 6).

**Modified**

- `crates/engine/src/world.rs` — the `Biome` rename, its `serde` alias,
  `Biome::name`, and the `SectorShape` threshold field rename.
- `crates/engine/src/game/turn.rs` — the hook in `move_player`.
- `crates/engine/src/game/lifecycle.rs` — `AssetDbs` field, `load_asset_dbs`
  load, two destructure sites (`Game::new` ~line 71, `Game::load` ~line 255)
  and the matching `insert_resource` sites (~line 95 and its `load` mirror).
- `crates/engine/src/lib.rs` — re-export.
- `crates/engine/src/tuning.rs` — the two ceilings.
- `crates/engine/src/tests/support.rs` — `assets_dir_with_environment`.
- `crates/engine/src/tests/mod.rs` — register the new test module.
- Rename fallout (Task 1): `crates/engine/src/species.rs`,
  `crates/engine/src/sectors.rs`, `crates/engine/src/game/spawning.rs`,
  `crates/engine/src/save.rs`, `crates/engine/src/tests/spawning.rs`,
  `crates/engine/src/tests/zone.rs`, `crates/engine/src/tests/sectors.rs`,
  `crates/app-core/src/app/arena.rs`, `crates/gui/src/render/base.rs`, seven
  `assets/species/*.ron`, `assets/species/README.md`,
  `assets/sectors/cold_storage.ron`, `assets/sectors/README.md`.

---

### Task 1: Rename `StaticField` to `Deadlock`

Mechanical and self-contained. Do it first so every later task writes the new
name. **The whole point of this task is that it changes no behaviour** — a
green suite afterwards is the deliverable.

**Files:** the rename-fallout list above. Start from
`rg -l "StaticField|static_field|static_temperature"` but do not trust it as
the completion gate — see the sweep step.

**Interfaces produced:**
- `Biome::Deadlock` — replaces `Biome::StaticField`, same position in the
  enum, carrying `#[serde(alias = "StaticField")]`.
- `SectorShape::deadlock_temperature: f32` — replaces `static_temperature`.
  The `SectorShapeDef` field that deserialises it carries
  `#[serde(alias = "static_temperature")]`.

- [ ] **Step 1: Write the failing tests**

Three, in `crates/engine/src/tests/sectors.rs` and a save test in
`crates/engine/src/save.rs`'s test module:

1. A sector file written with the **old** key `static_temperature` still
   applies its delta. Intent: prove the alias, so a third-party sector file
   does not silently lose its threshold.
2. A species file whose `habitats` list says `"StaticField"` still loads and
   still reports that habitat. Intent: prove the alias covers
   `SpeciesDef::habitats`, which is `Vec<Biome>` — this is what keeps every
   existing species mod working.
3. A save whose `tile_overrides` holds a tile written as `StaticField` loads,
   and a save written now reads `Deadlock`. Intent: prove the rename is not a
   save-format break. Build it by writing save text by hand with the old
   name, as `save.rs`'s existing version-handling tests already do.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine sectors::` and the save test by name.
Expected: compile failure or assertion failure naming `Deadlock`.

- [ ] **Step 3: Rename the enum variant and add the alias**

In `world.rs`. The alias is the load-bearing line and the reason this is not
a save break:

```rust
#[serde(alias = "StaticField")]
Deadlock,
```

Keep the variant in its current position. Reordering is what breaks a
positional encoding; this codebase's save is field-named RON, but `Perk`'s
ordering rule is a live trap elsewhere and the habit is worth keeping.

- [ ] **Step 4: Rename the sector threshold field**

`static_temperature` → `deadlock_temperature` in `world.rs`'s `SectorShape`
and `sectors.rs`'s def, with `#[serde(alias = "static_temperature")]` on the
deserialised field. Update `assets/sectors/cold_storage.ron` and
`assets/sectors/README.md`, including the `classify` table in that README
that names the biome.

- [ ] **Step 5: Sweep the rest**

The seven species `.ron` habitat lists, `assets/species/README.md`, the
remaining Rust references (32 across seven files, including the colour and
speckle arms in `crates/gui/src/render/base.rs` and the doc comments that
spell "Static Field" as prose).

While in `save.rs`: **correct the stale doc comment at ~line 258** claiming
the save is positional bincode. It is text RON — `save.rs:651` writes
`format!("{SAVE_FORMAT_VERSION}\n{text}")` and the reader is
`read_to_string`; bincode appears only in a test as a byte-identity
comparator. That comment is what makes this rename look like a save break.

- [ ] **Step 6: Gate the sweep on the new vocabulary**

Grepping the removed word is blind to what is half-converted around it, and
`--type rust` misses habitat lists and player text in `.ron`. Run both
directions across the whole tree, not just Rust:

```sh
rg -i "staticfield|static_field|static temperature|static field"
rg -i "deadlock"
```

The first must return only the two deliberate `serde(alias)` lines and any
test that exercises them. Read the second and confirm every hit is somewhere
the new name belongs.

- [ ] **Step 7: Full gate and commit**

`cargo fmt`, `cargo clippy --workspace`, `cargo test --workspace`. Every test
must pass with no behaviour change. Commit as
`refactor(world): the cold biome is Deadlock, and Static is free for weather`.

---

### Task 2: Biome names, and the log line on crossing a boundary

Delivers a feature on its own: the player is told what ground they are
standing on for the first time. No assets and no effects involved.

**Files:**
- Modify: `crates/engine/src/world.rs` (add `Biome::name`)
- Modify: `crates/engine/src/game/turn.rs` (`move_player`, ~line 388 —
  the `walkable` branch)
- Test: `crates/engine/src/tests/turn.rs`

**Interfaces produced:**
- `Biome::name(self) -> &'static str` — exhaustive match, `pub`. The seven
  names: Data Void, Deadlock, Null Sector, Mainframe, Open Grid, Black Ice,
  Platform.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/turn.rs`. Place the player deliberately either
side of a known biome boundary rather than hunting for one — write the
destination tile through `WorldMap`'s override overlay, which is what
`tile_overrides` exists for and what keeps the test off world-seed luck.

1. Stepping onto a tile of a different biome logs a line naming that biome.
2. Stepping within one biome logs nothing.
3. A step that bounces off an unwalkable tile logs nothing. Intent: the line
   belongs to travel, matching the comment already on `maybe_ambush`'s call
   site that shoving at a wall is not travel.
4. The line fires at `ZoneLevel(1)`. Intent: names are not gated by the
   zone-1 neutrality rule; only effects are. This test is what stops a later
   task moving the log line inside the gate.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine turn::`. Expected: no such method
`name`, then assertion failures on the log.

- [ ] **Step 3: Implement `Biome::name`**

Exhaustive match in `world.rs`. Not data: mods extend species, structures,
items and environments, but the biome set is a fixed enum the generator sorts
noise into.

- [ ] **Step 4: Implement the log line**

In `move_player`'s `walkable` branch, after the `Position` write. Compare the
biome of the tile stepped off against the destination's; both are in hand, so
nothing is stored and no save field appears. `MessageKind::Info`; pick the
`MessageSource` tag from the table in that enum's doc comment.

- [ ] **Step 5: Run the tests, then the suite**

`cargo test -p feral-processes-engine turn::`, then `cargo test --workspace`.
Watch for message-log tests elsewhere that count lines and now see an extra
one — if any fail, they are telling you the pane is noisier, which is the
open question the spec records. Fix the fixture, not the feature.

- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace`, commit**

`feat(world): the ground has a name, and crossing into it says so`.

---

### Task 3: The environment database and its one reader

Ships the data layer and the reader together so nothing is dead code at any
commit.

**Files:**
- Create: `crates/engine/src/environment.rs`
- Create: `crates/engine/src/game/environment.rs`
- Create: `crates/engine/src/tests/environment.rs`
- Create: `assets/environment/README.md`
- Modify: `crates/engine/src/lib.rs`, `crates/engine/src/game/mod.rs`,
  `crates/engine/src/tests/mod.rs` (module registration)
- Modify: `crates/engine/src/game/lifecycle.rs` — `AssetDbs` field,
  `load_asset_dbs`, both destructure sites, both `insert_resource` sites
- Modify: `crates/engine/src/tuning.rs` — the two ceilings
- Modify: `crates/engine/src/tests/support.rs` —
  `assets_dir_with_environment`

**Interfaces consumed:** `Biome::Deadlock` and friends (Task 1).

**Interfaces produced:**

```rust
pub enum EnvironmentEffect {
    Attrition { hp_percent: f32, min_damage: i32 },
    Drag { extra_ticks: u32 },
}

pub struct EnvironmentDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub biomes: Vec<Biome>,
    pub effect: EnvironmentEffect,
}

impl EnvironmentDb {
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)>;
    pub fn for_biome(&self, biome: Biome) -> Option<&EnvironmentDef>;
}

impl Game {
    pub fn ground_effect(&self, x: i32, y: i32) -> Option<&EnvironmentDef>;
}
```

`EnvironmentDb` is a bevy `Resource`. New `tuning.rs` consts:
`MAX_ENVIRONMENT_ATTRITION: f32` and `MAX_ENVIRONMENT_DRAG_TICKS: u32`, in a
labelled section with doc comments saying what each bound protects against.

- [ ] **Step 1: Write the failing load tests**

In `crates/engine/src/tests/environment.rs`. Build scratch directories with
`assets_dir_with_environment`, added to `support.rs` following
`assets_dir_with_sectors` — it must use the `ScratchAssets` RAII guard, since
a panic between creation and a manual cleanup leaks the directory and `Drop`
runs on an unwind.

1. A well-formed file loads and `for_biome` returns it.
2. A malformed file is skipped with a warning and the others still load.
3. A file naming `Platform` is skipped with a warning. Intent: the slab is
   the one safe ground in the game and that is not a file's decision to
   revoke.
4. An `hp_percent` above `MAX_ENVIRONMENT_ATTRITION` is skipped, and an
   `extra_ticks` above `MAX_ENVIRONMENT_DRAG_TICKS` is skipped. Intent: an
   authored `0.5` is death in two steps and an authored `10_000` is a hang.
5. Two ambient files claiming one biome: the **second** is skipped with a
   warning naming both ids, and the first still loads. Intent: fail fast on
   an authoring error rather than resolving it silently by directory order.
   Directory order is not stable across platforms, so the warning must name
   both files or a modder cannot tell which one won.
6. A file naming a hole (`DataVoid`, `BlackIce`) loads **without** a warning.
   Intent: unreachable is not the same as wrong, and a mod naming all six
   biomes for convenience must not be refused.
7. An absent directory loads silently to an empty db. Intent: the
   deleting-restores-the-game property.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine environment::`.

- [ ] **Step 3: Implement `environment.rs`**

Follow `PerkDb::load_dir` (`crates/engine/src/perks.rs:186`) exactly: iterate
the directory, skip non-`.ron`, `ron::from_str`, push a warning and `continue`
on both a parse error and a validation failure, return
`(db, warnings)`. Never panic — a malformed mod file must not crash startup.

Absent-directory silence: follow `AffixDb`/`SectorDb`, whose behaviour
`load_asset_dbs` already documents at its call sites.

- [ ] **Step 4: Wire it into `load_asset_dbs`**

`crates/engine/src/game/lifecycle.rs`: add the field to `AssetDbs` (~1337),
load it in `load_asset_dbs` (~1360) with a comment noting the
absent-is-silent rule as its neighbours do, and add it to both destructure
sites (~71 and ~255) and both `insert_resource` runs (~95 and the `load`
mirror). **Both doors must see it** — the `AssetDbs` destructure fails to
compile if you miss one, which is exactly why it is shaped that way.

- [ ] **Step 5: Write the failing `ground_effect` tests, then implement it**

Four: returns `Some` for a claimed biome past zone 1; `None` at
`ZoneLevel(1)`; `None` on `Platform`; `None` for an unclaimed biome. The
zone-1 gate lives **inside** `ground_effect` so it cannot lapse at a second
call site — a test that reads the db directly and asserts zone-1 emptiness
would be testing the wrong thing.

Implement in `crates/engine/src/game/environment.rs`.

- [ ] **Step 6: Write `assets/environment/README.md`**

Schema reference on the pattern of `assets/sectors/README.md`: the field
table, both effect shapes, the three refusals stated as what they protect,
and the deleting-restores-the-game paragraph. This directory's README is the
schema reference for anyone modding, so it ships with the schema, not after.

- [ ] **Step 7: Gate and commit**

`cargo fmt`, `cargo clippy --workspace`, `cargo test --workspace`. Commit as
`feat(world): environment effects load from assets, keyed by biome`.

---

### Task 4: Attrition

**Files:**
- Modify: `crates/engine/src/game/turn.rs` (`move_player`)
- Test: `crates/engine/src/tests/environment.rs`

**Interfaces consumed:** `Game::ground_effect` (Task 3), the log line
(Task 2).

- [ ] **Step 1: Write the failing tests**

1. A step onto attrition ground lowers the player's HP by
   `max(max_hp * hp_percent, min_damage)`, rounded as the implementation
   states.
2. A step that bounces off an unwalkable tile takes nothing.
3. The party is untouched. Intent: corrupting the party would route program
   deaths and the permadeath path through something that is not a fight.
4. A Mitigation field buff reduces the bite. Intent: this is free *only*
   because the damage goes through `Game::apply_damage`; the test is what
   stops someone "simplifying" it to a direct HP write.
5. **Attrition that kills does not then start an ambush.** Build it with
   `min_damage` above the player's current HP. Intent: the one place in this
   phase where two systems meet at a lethal edge.
6. No bite at `ZoneLevel(1)`, **and the biome name still logs**. Assert both
   halves in one test — the effect half alone passes against a bare early
   return that also swallowed the name.

- [ ] **Step 2: Run them and watch them fail**

- [ ] **Step 3: Implement the hook**

In `move_player`'s `walkable` branch, after the `Position` write. Order
inside the hook, mirroring `Game::arrive` underground — the ground's bite is
a property of arriving, and it lands ahead of the encounter roll:

```
resolve effect for destination
attrition -> apply_damage
transition log line
maybe_ambush()
self.tick()
```

Damage goes through `Game::apply_damage`, the one code path that lowers a
creature's HP. A fraction of max HP rather than a flat figure, for the reason
`bleed_corruption` records: terrain is uncorrelated with player level, so any
constant is lethal at level 1 and free by mid-run.

`maybe_ambush` is already guarded by `is_game_over()`, so test 5 should pass
without a new branch. If it does not, the guard is the thing to fix.

- [ ] **Step 4: Run the tests, then `cargo test --workspace`**

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit**

`feat(world): ground that costs you something to cross`.

---

### Task 5: Drag

**Files:**
- Modify: `crates/engine/src/game/turn.rs`
- Test: `crates/engine/src/tests/environment.rs`

- [ ] **Step 1: Write the failing tests**

1. A step onto `Drag { extra_ticks: 1 }` ground advances the clock by 2
   where an ordinary step advances it by 1. Read the tick count off
   `GameClock` rather than off a downstream consequence.
2. `Drag` takes no HP. Intent: the second effect kind exists precisely so the
   vocabulary is not all damage.

- [ ] **Step 2: Run them and watch them fail**

- [ ] **Step 3: Implement**

`move_player` already ends in `self.tick()`; `Drag` calls it `extra_ticks`
further times. A tick can start a fight — `nest_aggro_tick` is the precedent,
and it is why `rest`'s tick loop needed a battle check. Break out of the extra
ticks the moment `is_game_over()` or `has_active_battle()` is true, and add a
third test for it: a Drag step that opens a battle on its first tick must not
run the remaining ones. Anything that starts a fight from inside a tick loop
inherits that obligation.

- [ ] **Step 4: Run the tests, then `cargo test --workspace`**

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit**

`feat(world): ground that slows you down instead of hurting you`.

---

### Task 6: Shipped content, the census, and the docs

**Files:**
- Create: `assets/environment/*.ron` — three files
- Modify: `crates/engine/src/tests/assets.rs`
- Modify: `CLAUDE.md` and `docs/seams.md`

- [ ] **Step 1: Write the failing census tests**

In `crates/engine/src/tests/assets.rs`, over the **real** shipped assets, as
the existing censuses there do:

1. Every shipped file's magnitude is inside its `tuning.rs` ceiling.
2. No shipped file claims `Platform`.
3. `OpenGrid` has no ambient effect. Intent: the default ground stays
   neutral so "ground that does something" reads as an exception rather than
   a tax. This is a content decision the spec records and the test is what
   holds it.

- [ ] **Step 2: Author the three effect files**

`Deadlock`, `NullSector` and `Mainframe` — the only three standable biomes
left once the two holes, the slab and neutral Open Grid are excluded. Mix the
two effect kinds; give each a `name` and a one-sentence `description` in the
game's existing voice. Player-facing text follows the player's vocabulary,
and note the no-occult-naming rule in `CLAUDE.md`.

- [ ] **Step 3: Run the census, then `cargo test --workspace`**

- [ ] **Step 4: Write the seam entries**

One entry in **both** files, following their stated split: the rule in
`CLAUDE.md` under a title, the argument under the same title in
`docs/seams.md`. The rule to record is that `Game::ground_effect` is the one
door and the zone-1 gate lives inside it — plus the trap, which is that the
biome *name* is deliberately outside that gate.

Note that `CLAUDE.md` and `AGENTS.md` are gitignored twins with no tracking
to catch drift: edit `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`.

- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit**

`feat(world): three sectors' worth of ground that bites`.

---

## Landing obligations

Not tasks — they happen at the merge, via the `deploy` skill.

- Version bump in the root `Cargo.toml`. This is a **patch** bump: no save
  format change, so nothing here is breaking under this repo's definition.
- A `CHANGELOG.md` section. It must state that the sector schema key
  `static_temperature` became `deadlock_temperature`, since that is a
  mod-facing change even with the alias softening it.
- An annotated `vX.Y.Z` tag, pushed with `--follow-tags` — a bare
  `git push` does not send tags.
- `docs/manual.md` and the root `README.md` are carved out and stay stale.

## Notes for the executor

- **This feature ships unplayed.** The user is remote and cannot run the
  game. Say so plainly when reporting completion rather than implying it was
  exercised in play; a green suite is not evidence of play.
- **A seeded test that fails after an RNG-stream shift** is a known class of
  false alarm in this repo, and a single-crate run and a `--workspace` run
  are different builds with different streams. Probe before theorising, and
  fix the fixture's incidental coupling rather than the seed.
- **If many tests fail at once with `NotFound` on an assets path**, that is
  stale build artifacts from an old directory rename, not real failures. Fix
  with `cargo clean -p feral-processes-engine -p feral-processes-app-core` —
  not a full `cargo clean`, which discards ~4 GB.
- **Do not push.** Committing on the branch is expected; pushing needs an
  explicit ask from the user.
