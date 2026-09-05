# Program Extraction — Phase 5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** A fifth extraction tool category that pulls the worn gear off a
downed program, rolling the species' own authored drop chances scaled by
tool and bench tier — additive to every gear door the game already has.

**Architecture:** One new derivation (`Game::gear_chances`) beside
`extraction_yield`, reading the bench the same way and scaled by the same
shipped `tier_scale` curve, so a tier-1 tool on an un-upgraded bench is
*exactly* the authored chance and no new constant enters the economy. One
new `ToolCategory` variant, pool-less like `Routines`, taking a third arm in
`extract_program` that grants through `grant_gear_drop` — the single
rare-tier door. One new `ExtractionPreview` variant so the screen quotes the
same numbers the pull uses.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, serde.

**Spec:** `docs/superpowers/specs/2026-09-04-program-extraction-design.md` —
**read §9 first**, it is the authority on this phase and it contradicts §8's
phase list on purpose. §3's 2026-09-05 amendment (no `structure_tier`
parameter; the bench enters as `tier - 1`) is load-bearing for Task 1.

**Branch:** cut from `main` at v0.13.105.

## Global Constraints

- **No new tuning constant for the chance scale.** §9.5: the baseline is
  `tier_scale(1) = 1.0` falling out of `tuning::TOOL_TIER_SCALE_STEP`. If a
  task finds itself introducing `GEAR_TOOL_CHANCE_SCALE`, it has broken the
  neutrality identity Task 1 asserts — stop and re-read §9.5.
- **The bench is never a parameter.** `gear_chances` calls
  `Game::extraction_bench_tier()` itself, for §3's amended reason: the
  preview and the pull must not be able to differ.
- **Every existing gear door stays open.** `equipment_drops_for` keeps all
  four of its callers. No task deletes a drop roll. (§9.2)
- **`grant_gear_drop` is the only door above `Ordinary`.** Nothing in this
  phase mints a rarity by any other route. (§9's act)
- **No occult naming in content.** Standing repo rule.
- **Uppercase for screen actions; lowercase letters are row selectors.**
  This phase adds no new key, but the rule binds any that gets proposed.
- Every task ends green on `cargo test --workspace`, not a single-package
  run — a single-crate run shifts the RNG stream and is a different build.

---

## Decisions this plan makes, that the spec left open

1. **The tool is `harness_puller`, "Harness Puller".** §9 left the name
   open. "Pulls the worn harness off a downed process" — plain, no occult
   vocabulary, and it does not collide with `component_stripper`'s subject
   (loose parts) or `core_tap`'s (the core under the shell).
2. **It is taught by `deep_analysis`, not a new node.** §9 named `field_ops`
   as the candidate and `runtime_patching` as the alternative; the census
   this plan owes says neither fits. `field_ops` is cost 20 with no
   `min_zone` — far too early for the strongest tool in the set — and
   `runtime_patching` is about patching an ally mid-fight. `deep_analysis`
   is zone 3, already teaches `core_tap`, and its subject *is* taking a
   downed program apart far enough to get at what is under the shell.
   **A new node was rejected on doc cost:** `docs/research.md` is a hand
   transcription (`docs/research-gen.py` is not a parser), and a new node
   moves its zone table, its tree, its cost histogram and its
   end-of-branch count. Attaching to an existing node is a one-line doc
   edit. The real gate on the tool is `forge_cost`, not the node.
3. **The asset ships in Task 2, not later.** `ToolDb` exposes no
   insertion API, so no test can install a tool that no file declares —
   the branch and its tool are one deliverable.
4. **`forge_cost` and `ticks` are guessed, and say so in the asset.** Every
   other tool's numbers are guesses too; the asset comment must not imply
   otherwise. `ticks: 40` matches the Routine Reader's band — stripping a
   program's kit off it is slow work.
5. **`gear_chances` is `pub`, matching `extraction_yield`.** Both are read
   by the view layer through `Game`.

---

### Task 1: `gear_chances`, and the identity that proves it neutral

**Files:**
- Modify: `crates/engine/src/game/extraction.rs` (add beside `extraction_yield`, which ends at line 198)
- Test: `crates/engine/src/tests/extraction.rs` (append; this file is where every phase appends)

**Interfaces:**
- Consumes: `Game::extraction_bench_tier() -> u32` (0 when no bench stands),
  `tier_scale(u32) -> f32` (private, same module),
  `Game::equipment_drops_for(&SpeciesDef) -> Vec<(ItemId, f32)>`
  (`pub(crate)`, in `game/combat_rewards.rs`).
- Produces: `Game::gear_chances(&DownedProgram, &ToolDef) -> Vec<(ItemId, f32)>`,
  clamped to `0.0..=1.0`, sorted by item id (inherited from its source).
  Tasks 2 and 3 both call it.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/extraction.rs`:

```rust
/// A `Gear` tool is defined in-test rather than off an asset: the shipped
/// one arrives in Task 4, and the derivation must be provable without it.
fn gear_tool(tier: u32) -> ToolDef {
    ToolDef {
        id: ToolId("test_gear_tool".to_string()),
        name: "Test Gear Tool".to_string(),
        description: "Pulls worn gear off a downed process.".to_string(),
        category: ToolCategory::Gear,
        yields: Vec::new(),
        tier,
        ticks: 40,
        forge_cost: Vec::new(),
    }
}

/// Picks a species that actually has gear to drop — `equipment_drops_for`
/// merges both schema directions, so "has drops" is a question only it can
/// answer. Mirrors `a_running_drop_boost_field_buff_scales_every_
/// equipment_drop_chance`'s idiom in `tests/combat_rewards.rs`.
fn species_with_gear(game: &Game) -> SpeciesDef {
    game.species_defs()
        .into_iter()
        .find(|s| !game.equipment_drops_for(s).is_empty())
        .expect("at least one shipped species should drop gear")
}

/// **The neutrality identity.** A tier-1 `Gear` tool with no bench standing
/// quotes the authored chances exactly — not approximately, not scaled by
/// grade. Written as an identity against `equipment_drops_for` rather than
/// against a literal, so it fails the day anyone inserts a constant between
/// the table and the roll (spec §9.5, and the Global Constraint above).
#[test]
fn a_tier_one_gear_tool_with_no_bench_quotes_the_authored_chances() {
    let game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = species_with_gear(&game);
    let authored = game.equipment_drops_for(&species);

    let mut downed = program(50, Rarity::Ordinary, 3);
    downed.species = species.id.clone();

    assert_eq!(game.extraction_bench_tier(), 0, "no bench should stand here");
    let quoted = game.gear_chances(&downed, &gear_tool(1));

    assert_eq!(quoted.len(), authored.len());
    for ((q_item, q_chance), (a_item, a_chance)) in quoted.iter().zip(authored.iter()) {
        assert_eq!(q_item, a_item);
        assert_eq!(
            *q_chance,
            a_chance.clamp(0.0, 1.0),
            "a tier-1 tool on no bench must quote the authored chance for {q_item:?}"
        );
    }
}

/// Grade must not enter the chance. Two programs of the same species at
/// opposite ends of every grade axis quote identically — what grade sells
/// is materials, in `extraction_yield`.
#[test]
fn program_grade_does_not_move_a_gear_chance() {
    let game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = species_with_gear(&game);

    let mut poor = program(1, Rarity::Ordinary, 1);
    poor.species = species.id.clone();
    let mut rich = program(100, Rarity::Gold, 30);
    rich.species = species.id.clone();

    assert_eq!(
        game.gear_chances(&poor, &gear_tool(1)),
        game.gear_chances(&rich, &gear_tool(1)),
        "grade sells materials, never gear odds"
    );
}

/// Tier scales the chance, and the curve is the shipped one — a tier-2 tool
/// is `1.0 + TOOL_TIER_SCALE_STEP` times the authored figure, clamped.
#[test]
fn a_higher_tier_gear_tool_scales_the_authored_chance() {
    let game = Game::new(4203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = species_with_gear(&game);
    let authored = game.equipment_drops_for(&species);

    let mut downed = program(50, Rarity::Ordinary, 3);
    downed.species = species.id.clone();

    let scaled = game.gear_chances(&downed, &gear_tool(2));
    let step = crate::tuning::TOOL_TIER_SCALE_STEP;

    for ((s_item, s_chance), (a_item, a_chance)) in scaled.iter().zip(authored.iter()) {
        assert_eq!(s_item, a_item);
        let expected = (a_chance * (1.0 + step)).clamp(0.0, 1.0);
        assert!(
            (s_chance - expected).abs() < 1e-6,
            "tier 2 should quote {expected} for {s_item:?}, got {s_chance}"
        );
    }
}

/// Clamped inside, unlike its source. `equipment_drops_for` returns chances
/// unclamped on purpose (its one caller clamps before rolling); this one has
/// two callers — the preview and the pull — and a value clamped twice in two
/// places is a crack they could differ through (spec §9.5).
#[test]
fn a_gear_chance_never_exceeds_one() {
    let game = Game::new(4204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = species_with_gear(&game);

    let mut downed = program(50, Rarity::Ordinary, 3);
    downed.species = species.id.clone();

    // A tier far past anything shippable, so every authored chance is
    // pushed over 1.0 before the clamp.
    for (item, chance) in game.gear_chances(&downed, &gear_tool(99)) {
        assert!(
            (0.0..=1.0).contains(&chance),
            "chance for {item:?} escaped the clamp: {chance}"
        );
    }
}

/// A species the run has no def for quotes nothing rather than panicking —
/// a mod species removed between save and load is the real case.
#[test]
fn an_unknown_species_quotes_no_gear() {
    let game = Game::new(4205, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut downed = program(50, Rarity::Ordinary, 3);
    downed.species = "no_such_species".to_string();
    assert!(game.gear_chances(&downed, &gear_tool(1)).is_empty());
}
```

Add `SpeciesDef` to the file's imports if `use crate::*;` does not already
bring it in — check before adding, the glob may cover it.

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p feral-processes-engine gear_chances 2>&1 | tail -20
```

Expected: compile failure — `no method named gear_chances`, and
`no variant named Gear`. Both are real: this task adds the method, Task 2
adds the variant. **Add the `Gear` variant now** (in
`crates/engine/src/tools.rs`, after `Routines`, plus its `as_str` arm
returning `"Gear"`), because the test tool cannot be constructed without
it; Task 2 owns the *dispatch*, not the variant.

Adding the variant will break `every_non_routines_tool_has_a_non_empty_yield_pool`'s
exhaustive match in `crates/engine/src/tests/assets.rs:3327`. Give it a
`ToolCategory::Gear` arm asserting `def.yields.is_empty()` with the message
`"tool {:?} is category Gear and must not declare a yields pool — it rolls the species' own drop table instead"`,
and count it into `checked` the way the `Routines` arm does.

- [ ] **Step 3: Write the implementation**

In `crates/engine/src/game/extraction.rs`, directly after `extraction_yield`:

```rust
    /// The chance of each gear item a `Gear` tool could pull off `program`,
    /// scaled by the tool's tier and the bench's.
    ///
    /// The pool is `equipment_drops_for`'s — both schema directions merged,
    /// sorted by item id, with a running `DropBoost` already folded in.
    /// Extraction needs no bench to stand, so a player may arm a buff and
    /// then strip; that is a recorded interaction, not an oversight (spec
    /// §9's "Recorded interactions").
    ///
    /// **The baseline needs no constant.** `tier_scale(1)` is `1.0`, and the
    /// bench's term is `tier - 1` like `extraction_yield`'s, so a tier-1
    /// tool on a never-upgraded bench — or no bench at all — quotes the
    /// authored chance untouched. `a_tier_one_gear_tool_with_no_bench_
    /// quotes_the_authored_chances` asserts that as an identity rather than
    /// as a number, so inserting a scale constant here fails loudly.
    ///
    /// **Grade does not enter.** `program.grade()` already sells materials
    /// in `extraction_yield`; leaving it out is what makes the baseline
    /// literally the authored chance rather than approximately it.
    ///
    /// **Clamped here, unlike its source.** `equipment_drops_for` returns
    /// chances unclamped because its one caller clamps before rolling. This
    /// has two callers — the screen's preview and the pull — whose whole
    /// reason for sharing a derivation is that a quoted figure and a rolled
    /// one cannot differ, so the clamp lands once, inside.
    pub fn gear_chances(&self, program: &DownedProgram, tool: &ToolDef) -> Vec<(ItemId, f32)> {
        let Some(species) = self
            .world
            .resource::<SpeciesDb>()
            .get(&program.species)
            .cloned()
        else {
            return Vec::new();
        };
        let bench = self.extraction_bench_tier().saturating_sub(1);
        let scale = tier_scale(tool.tier + bench);
        let mut chances = self.equipment_drops_for(&species);
        for (_, chance) in &mut chances {
            *chance = (*chance * scale).clamp(0.0, 1.0);
        }
        chances
    }
```

`SpeciesDb` and `ItemId` should already be in this file's imports for
`extraction_yield`'s neighbours; add whichever is missing.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test --workspace 2>&1 | tail -15
```

Expected: all pass. A failure in `tests/assets.rs` means Step 2's new match
arm was skipped.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/extraction.rs crates/engine/src/tests/extraction.rs crates/engine/src/tools.rs crates/engine/src/tests/assets.rs
git commit -m "feat(extraction): what a Gear tool would pull off a downed program"
```

---

### Task 2: The third arm, and the tool that takes it

The shipped asset lands **here**, not in a later task: `ToolDb` has no
insertion API (only `load_dir`, `get`, `all`), so a test cannot install a
tool that no asset file declares. The branch and its tool are therefore one
deliverable. Research, docs and the censuses follow in Task 4.

**Files:**
- Create: `assets/tools/harness_puller.ron`
- Modify: `crates/engine/src/game/extraction.rs:436-508` (`extract_program`)
- Modify: `assets/tools/README.md`
- Test: `crates/engine/src/tests/extraction.rs` (append)

**Interfaces:**
- Consumes: `Game::gear_chances` (Task 1);
  `Game::grant_gear_drop(ItemId, Rarity) -> GearCopy` (`pub(crate)`,
  `game/combat_rewards.rs:138`) — which routes an equippable item through
  `add_copies` into the player's `GearCopies` ledger, and a non-equippable
  one through `grant_loot` as a plain row;
  `Game::drop_label(&GearCopy) -> String`; `Game::record_drop(GearCopy, u32)`.
- Existing test helpers in `tests/extraction.rs`, all real — use these
  rather than writing new ones: `build_program_bench(&mut Game, Option<u32>)`,
  `give_downed_program(&mut Game, DownedProgram)`, `held(&Game, &ItemId)`,
  `program(condition, rarity, level)`.
- Produces: no new signature — `extract_program`'s behaviour for a `Gear`
  tool.

- [ ] **Step 1: Write the asset**

Create `assets/tools/harness_puller.ron`:

```ron
(
    id: "harness_puller",
    name: "Harness Puller",
    description: "Pulls the worn harness off a downed process, gear and all.",
    // The fifth category, and the second one with no `yields`: what comes
    // out is the species' own drop table rolled at extraction, not a pool
    // drawn from. See `Game::gear_chances`.
    category: Gear,
    // Tier is not decoration here — it multiplies every authored drop
    // chance through the shared `tier_scale` curve, so a tier-2 Gear tool
    // would be a straight 1.5x on the game's whole gear rate. Raise this
    // only with a play session behind it.
    tier: 1,
    // The Routine Reader's band: stripping a program's kit off it is the
    // slow kind of work. Guessed, like every other figure in this
    // directory — no instrument in the repo checks it.
    ticks: 40,
    // The real gate on this tool, since `deep_analysis` (Task 4) may
    // already be researched when it ships. Priced against the Core Tap's
    // band, above the starter clamp's replacement price. Also guessed.
    forge_cost: [("annealed_core", 2), ("logic_wafer", 4)],
)
```

Verify both ingredient ids resolve before moving on — a `forge_cost`
naming a missing item is a load warning, not a compile error, so it fails
silently:

```bash
rg -l 'id: "annealed_core"|id: "logic_wafer"' assets/items/
```

If either does not resolve, substitute a shipped item of the same band and
note the substitution in the asset comment.

- [ ] **Step 2: Write the failing tests**

Append to `crates/engine/src/tests/extraction.rs`:

```rust
/// The shipped Gear tool, installed into the player's one slot. `Tools` is
/// a bare `Vec<ItemId>`-shaped newtype over `ToolId`, so this bypasses
/// research and forging deliberately — what is under test is the branch,
/// not the door to it.
fn install_harness_puller(game: &mut Game) -> ToolId {
    let id = ToolId("harness_puller".to_string());
    assert!(
        game.world.resource::<ToolDb>().get(id.as_str()).is_some(),
        "harness_puller.ron must load before this test can install it"
    );
    let player = game.player_entity();
    game.world.get_mut::<Tools>(player).unwrap().0 = vec![id.clone()];
    id
}

/// A pull that is *certain* to land, without hunting for a seed: a bench at
/// a high tier pushes every authored chance past 1.0, where `gear_chances`
/// clamps it. The certainty comes from the shipped mechanism rather than
/// from a fixture, so this test also proves the bench term reaches the
/// chance at all.
#[test]
fn a_gear_pull_that_is_certain_grants_every_candidate_and_spends_the_program() {
    let mut game = Game::new(4206, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = species_with_gear(&game);
    build_program_bench(&mut game, Some(20));

    let tool_id = install_harness_puller(&mut game);
    let tool = game
        .world
        .resource::<ToolDb>()
        .get(tool_id.as_str())
        .unwrap()
        .clone();

    let mut downed = program(50, Rarity::Ordinary, 3);
    downed.species = species.id.clone();

    let certain: Vec<ItemId> = game
        .gear_chances(&downed, &tool)
        .into_iter()
        .filter(|(_, chance)| *chance >= 1.0)
        .map(|(item, _)| item)
        .collect();
    assert!(
        !certain.is_empty(),
        "test premise: a tier-20 bench should make at least one candidate certain"
    );

    let before: Vec<u32> = certain.iter().map(|item| held(&game, item)).collect();
    give_downed_program(&mut game, downed);
    let store_before = game.world
        .get::<DownedPrograms>(game.player_entity())
        .unwrap()
        .0
        .len();

    game.extract_program(0, &tool_id).expect("the pull should succeed");

    assert_eq!(
        game.world
            .get::<DownedPrograms>(game.player_entity())
            .unwrap()
            .0
            .len(),
        store_before - 1,
        "the program is spent"
    );
    for (item, before) in certain.iter().zip(before.iter()) {
        assert!(
            held(&game, item) > *before,
            "a certain pull should have granted {item:?}"
        );
    }
}

/// The gear branch grants a **real copy** through `grant_gear_drop` rather
/// than a plain inventory row — the copy lands in the player's `GearCopies`
/// ledger, which is what proves it went through the one rare-tier door
/// (spec §9's act). A branch that granted plain loot instead would satisfy
/// the test above and fail this one.
#[test]
fn a_gear_pull_grants_a_copy_through_the_rare_tier_door() {
    let mut game = Game::new(4211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = species_with_gear(&game);
    build_program_bench(&mut game, Some(20));
    let tool_id = install_harness_puller(&mut game);

    let mut downed = program(50, Rarity::Ordinary, 3);
    downed.species = species.id.clone();
    give_downed_program(&mut game, downed);

    let player = game.player_entity();
    let before = game
        .world
        .get::<GearCopies>(player)
        .map(|ledger| ledger.copies.len())
        .unwrap_or(0);

    game.extract_program(0, &tool_id).expect("the pull should succeed");

    let after = game
        .world
        .get::<GearCopies>(player)
        .map(|ledger| ledger.copies.len())
        .unwrap_or(0);
    assert!(
        after > before,
        "an equippable pull must enter the GearCopies ledger, not the plain inventory"
    );
}

/// A miss pays nothing at all — no consolation scrap, no pool — and still
/// spends the program and the ticks (spec §9.4). The miss is constructed
/// rather than waited for: a species the run has no def for quotes an empty
/// chance list, so the pull cannot land.
#[test]
fn a_gear_pull_that_finds_nothing_pays_nothing_and_still_spends_the_program() {
    let mut game = Game::new(4207, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tool_id = install_harness_puller(&mut game);

    let mut downed = program(50, Rarity::Ordinary, 3);
    downed.species = "no_such_species".to_string();
    give_downed_program(&mut game, downed);

    let player = game.player_entity();
    let before_items = game.world.get::<Inventory>(player).unwrap().items.clone();
    let before_copies = game
        .world
        .get::<GearCopies>(player)
        .map(|ledger| ledger.copies.len())
        .unwrap_or(0);
    let store_before = game.world.get::<DownedPrograms>(player).unwrap().0.len();

    game.extract_program(0, &tool_id).expect("the pull should succeed");

    assert_eq!(
        game.world.get::<DownedPrograms>(player).unwrap().0.len(),
        store_before - 1,
        "the program is spent on a miss too"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before_items,
        "a Gear tool has no yields pool and pays nothing on a miss"
    );
    assert_eq!(
        game.world
            .get::<GearCopies>(player)
            .map(|ledger| ledger.copies.len())
            .unwrap_or(0),
        before_copies
    );
}

/// The Global Constraint, asserted rather than assumed: phase 5 deleted no
/// gear door. `equipment_drops_for` still answers for `award_loot`.
#[test]
fn the_kill_still_rolls_its_own_gear_after_phase_five() {
    let game = Game::new(4208, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = species_with_gear(&game);
    assert!(
        !game.equipment_drops_for(&species).is_empty(),
        "equipment_drops_for must keep answering for award_loot — phase 5 is additive"
    );
}
```

Add `GearCopies` and `Inventory` to the file's imports if `use crate::*;`
does not already cover them.

- [ ] **Step 3: Run the tests to verify they fail**

```bash
cargo test -p feral-processes-engine gear_pull 2>&1 | tail -20
```

Expected: FAIL — the `Gear` tool currently falls through to the materials
branch, reads its empty `yields` pool and grants nothing, so both the
certain-pull test and the ledger test fail on a count that did not move.

- [ ] **Step 4: Write the implementation**

In `extract_program`, directly below the `Routines` early return at
lines 456-458:

```rust
        // The `Gear` category takes a third branch: no `yields` pool, and
        // the outcome is rolled rather than apportioned. It sits beside the
        // `Routines` return rather than inside the materials path because
        // the two share nothing but the program's removal.
        if tool_def.category == ToolCategory::Gear {
            return self.extract_gear_from_program(index, &program, &tool_def);
        }
```

And a new method beside `extract_routine_from_program`:

```rust
    /// The `Gear` branch of `extract_program`. Rolls each chance from
    /// `gear_chances` and grants every hit through `grant_gear_drop` — the
    /// one door a copy above `Ordinary` enters the game through, so
    /// found-gear-beats-crafted-gear still binds and
    /// `crafted_gear_is_never_rare` is untouched.
    ///
    /// `Rarity::Ordinary` is the floor, deliberately: the boss's own door
    /// is still open and still paying `SURFACE_BOSS_LOOT_RARITY_FLOOR` at
    /// the kill, so `DownedProgram::boss` does not carry a second floor
    /// here (spec §9's act).
    ///
    /// A miss pays nothing — no pool, no consolation — and the program and
    /// the ticks are spent regardless (spec §9.4).
    fn extract_gear_from_program(
        &mut self,
        index: usize,
        program: &DownedProgram,
        tool_def: &ToolDef,
    ) -> Result<(), String> {
        let chances = self.gear_chances(program, tool_def);

        let player = self.player_entity();
        self.world
            .get_mut::<DownedPrograms>(player)
            .unwrap()
            .0
            .remove(index);

        let mut taken: Vec<String> = Vec::new();
        for (item, chance) in chances {
            let hit = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(chance as f64)
            };
            if hit {
                let copy = self.grant_gear_drop(item, Rarity::Ordinary);
                taken.push(self.drop_label(&copy));
                self.record_drop(copy, 1);
            }
        }

        let label = self.downed_program_label(program);
        if taken.is_empty() {
            self.log_kind(
                MessageKind::Loot,
                format!(
                    "You work {label} over with the {} and find nothing worth wearing.",
                    tool_def.name
                ),
            );
        } else {
            self.log_kind(
                MessageKind::Loot,
                format!(
                    "You work {label} over with the {}: {}.",
                    tool_def.name,
                    taken.join(", ")
                ),
            );
        }

        // Quoted once, before the loop — a bench demolished mid-extraction
        // must not change what this use was already priced at.
        let ticks = self.extraction_ticks(tool_def);
        for _ in 0..ticks {
            if self.is_game_over().is_some() || self.has_active_battle() {
                break;
            }
            self.tick();
        }

        Ok(())
    }
```

`random_bool` needs no second clamp — `gear_chances` already clamped, which
is the whole point of clamping inside it.

- [ ] **Step 5: Update the schema doc**

In `assets/tools/README.md`: add `Gear` to the category list at lines 30-37
in the same style as the others, and add a section after "The `Routines`
category" — "The `Gear` category" — saying it takes no `yields` either,
that its output is the species' authored drop table rolled at extraction,
and that `tier` multiplies every chance through the shared curve, so a tier
bump is a game-wide gear-rate change rather than a per-tool one.

- [ ] **Step 6: Run the full suite**

```bash
cargo test --workspace 2>&1 | tail -15
```

- [ ] **Step 7: Commit**

```bash
git add assets/tools/harness_puller.ron assets/tools/README.md crates/engine/src/game/extraction.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): the Harness Puller pulls worn kit off a downed program"
```

---

### Task 3: The screen quotes the same numbers the pull uses

**Files:**
- Modify: `crates/engine/src/views.rs:2598-2614` (`ExtractionPreview`)
- Modify: `crates/engine/src/game/extraction.rs:372-407` (`extraction_options`)
- Modify: `crates/gui/src/render/extraction.rs:120-129` (the `outcome` match)
- Test: `crates/engine/src/tests/extraction.rs`, plus a row test beside the existing ones in `crates/gui/src/render/extraction.rs`

**Interfaces:**
- Consumes: `Game::gear_chances` (Task 1), the shipped `harness_puller` and
  `install_harness_puller` (Task 2).
- Produces: `ExtractionPreview::Chances(Vec<(String, f32)>)` — display name
  and clamped chance, in `gear_chances`' order.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/extraction.rs`:

```rust
/// The §3 invariant applied to the new derivation: what the screen quotes
/// is what the pull rolls, because both call one function.
#[test]
fn the_preview_quotes_gear_chances_verbatim() {
    let mut game = Game::new(4209, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = species_with_gear(&game);
    let tool_id = install_harness_puller(&mut game);
    let tool = game
        .world
        .resource::<ToolDb>()
        .get(tool_id.as_str())
        .unwrap()
        .clone();

    let mut downed = program(50, Rarity::Ordinary, 3);
    downed.species = species.id.clone();
    let expected = game.gear_chances(&downed, &tool);
    give_downed_program(&mut game, downed);

    let options = game.extraction_options(0);
    let option = options
        .iter()
        .find(|o| o.tool == tool_id)
        .expect("the installed Gear tool should have a row");

    match &option.preview {
        crate::views::ExtractionPreview::Chances(rows) => {
            assert_eq!(rows.len(), expected.len());
            for ((name, chance), (item, expected_chance)) in rows.iter().zip(expected.iter()) {
                assert_eq!(name, &game.item_name(item));
                assert_eq!(chance, expected_chance);
            }
        }
        other => panic!("a Gear tool should preview chances, got {other:?}"),
    }
}
```

And beside the existing row tests in `crates/gui/src/render/extraction.rs`,
one that builds a `Chances` preview and asserts the rendered row text
contains both the item's display name and a `%`. Follow whatever harness
the neighbouring row tests in that file already use — do not introduce a
second one.

- [ ] **Step 2: Run it to verify it fails**

```bash
cargo test --workspace preview_quotes_gear 2>&1 | tail -20
```

Expected: compile failure — `no variant named Chances`.

- [ ] **Step 3: Write the implementation**

In `views.rs`, add to `ExtractionPreview` — and amend the enum's own doc
comment, which says "the two categories" and is now three:

```rust
    /// A `Gear` tool: each candidate item and its live chance, from
    /// `Game::gear_chances` — the same call the pull makes, so the quoted
    /// figure and the rolled one are one value rather than two. Tier
    /// scaling and any running `DropBoost` are already in these numbers.
    ///
    /// **The first raw percentages on any screen in the game** (spec §9.6),
    /// chosen over a name-only list because an upgraded Compiler must not
    /// read identically to a fresh one.
    Chances(Vec<(String, f32)>),
```

In `extraction_options`, replace the two-way `if` with an exhaustive match,
so a sixth category cannot fall through a bare `else`:

```rust
                let preview = match tool.category {
                    ToolCategory::Routines => {
                        let pool = self.routine_candidates(&program);
                        if pool.is_empty() {
                            crate::views::ExtractionPreview::NothingToLearn
                        } else {
                            crate::views::ExtractionPreview::Routine(
                                pool.iter()
                                    .map(|id| self.ability_display_name(id))
                                    .collect(),
                            )
                        }
                    }
                    ToolCategory::Gear => crate::views::ExtractionPreview::Chances(
                        self.gear_chances(&program, &tool)
                            .into_iter()
                            .map(|(item, chance)| (self.item_name(&item), chance))
                            .collect(),
                    ),
                    ToolCategory::Materials | ToolCategory::Parts | ToolCategory::Cores => {
                        crate::views::ExtractionPreview::Items(
                            self.extraction_yield(&program, &tool),
                        )
                    }
                };
```

In `crates/gui/src/render/extraction.rs`, add two arms to the `outcome`
match at line 120:

```rust
            ExtractionPreview::Chances(rows) if rows.is_empty() => "no gear to strip".to_string(),
            ExtractionPreview::Chances(rows) => rows
                .iter()
                .map(|(name, chance)| format!("{name} {}%", (chance * 100.0).round() as u32))
                .collect::<Vec<_>>()
                .join(", "),
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test --workspace 2>&1 | tail -15
```

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/views.rs crates/engine/src/game/extraction.rs crates/gui/src/render/extraction.rs crates/engine/src/tests/extraction.rs
git commit -m "feat(extraction): the block quotes a Gear tool's odds"
```

---

### Task 4: The research grant, and the censuses

**Files:**
- Modify: `assets/research/deep_analysis.ron`
- Modify: `docs/research.md` (the Deep Analysis row only)
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: the shipped `harness_puller` (Task 2).
- Produces: `harness_puller` reachable through research rather than only
  through a test that writes `Tools` directly.

- [ ] **Step 1: Write the failing census**

In `crates/engine/src/tests/assets.rs`, beside `a_shipped_tool_reads_routines`
(line 3528):

```rust
/// A `Gear` category with no tool in it would ship the whole third branch
/// as unreachable content — `every_non_routines_tool_has_a_non_empty_yield_pool`
/// only says what a `Gear` tool is *exempt* from, never that one exists.
#[test]
fn a_shipped_tool_pulls_gear() {
    let game = Game::new(4210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tools = game.world.resource::<ToolDb>();
    assert!(
        tools.all().any(|def| def.category == ToolCategory::Gear),
        "no shipped tool takes the gear branch"
    );
}

/// And that some research node teaches it — a tool no node unlocks is
/// reachable only by a test that writes `Tools` directly, which is exactly
/// how Task 2's tests reach it and is not a shipping route.
///
/// Reads the files rather than a loaded `Game`, matching
/// `every_researched_tool_unlock_resolves_to_a_shipped_tool` above: there
/// is no `Game::research_defs()` accessor, and this census is about what
/// the directory declares.
#[test]
fn a_research_node_teaches_the_gear_tool() {
    // `ResearchDef::unlocks_tools` is `Vec<ToolId>`, not `Vec<String>`.
    let mut taught: Vec<ToolId> = Vec::new();
    for entry in std::fs::read_dir(test_assets_dir().join("research")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let def: crate::research::ResearchDef = ron::from_str(&text).unwrap();
        taught.extend(def.unlocks_tools.iter().cloned());
    }
    assert!(
        taught.iter().any(|id| id.as_str() == "harness_puller"),
        "no research node unlocks harness_puller; taught tools are {taught:?}"
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

```bash
cargo test --workspace gear_tool 2>&1 | tail -12
```

Expected: `a_shipped_tool_pulls_gear` PASSES already (Task 2 shipped the
asset); `a_research_node_teaches_the_gear_tool` FAILS — no node unlocks it.
That split is expected, and the first test still earns its place: it is the
census that fails the day someone deletes the asset.

- [ ] **Step 3: Grant it from Deep Analysis**

In `assets/research/deep_analysis.ron`, extend the existing line to:

```ron
    unlocks_tools: ["core_tap", "harness_puller"],
```

and extend the node's `description` to name the Harness Puller, the way
`cortex.ron` names the Routine Reader in its own description.

- [ ] **Step 4: Update the research doc**

In `docs/research.md`, extend the **Deep Analysis row only** to name
`tool harness_puller` alongside `core_tap`, matching the Cortex Hacking
row's format at line 118. **Change nothing else in that file** — it is a
hand transcription rather than generated output, and its zone tables, tree,
cost histogram and end-of-branch count are all unaffected precisely because
no node was added.

- [ ] **Step 5: Run the full suite**

```bash
cargo test --workspace 2>&1 | tail -15
```

Expected: all pass, including
`every_researched_tool_unlock_resolves_to_a_shipped_tool`
(`tests/assets.rs:3434`), which now has a second unlock on that node to
resolve.

- [ ] **Step 6: Commit**

```bash
git add assets/research/deep_analysis.ron docs/research.md crates/engine/src/tests/assets.rs
git commit -m "feat(extraction): Deep Analysis teaches the Harness Puller"
```

---

### Task 5: Documentation and the release

**Files:**
- Modify: `docs/seams.md`
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml` version fields (workspace + any crate pinning it)

**Interfaces:**
- Consumes: everything above.
- Produces: v0.13.106.

- [ ] **Step 1: Grep for claims this phase falsifies**

```bash
rg -n "equipment_drops_for|gear drop|drops at the kill" docs/ CLAUDE.md
```

Any line saying gear drops only at the kill, or that extraction pays
materials only, is now false and must be corrected. **`docs/manual.md` and
the root `README.md` are carved out** of the doc obligation and are left
stale deliberately.

- [ ] **Step 2: Add the seam**

In `docs/seams.md`, beside the phase-3 extraction entries: `gear_chances`
reads the bench itself and clamps inside; the preview and the pull share it;
the baseline is `tier_scale(1) = 1.0` and **no scale constant exists** —
adding one silently breaks the neutrality identity.

- [ ] **Step 3: Write the changelog section**

Add a `## 0.13.106` section at the top of `CHANGELOG.md`, in the file's
established voice — what a player would notice, not what changed in the
code. It must say plainly: the Harness Puller is a *second* way to get
gear rather than a replacement, so gear turns up more often overall for
anything hauled home; a miss pays nothing; and a better Compiler raises
the odds. And, following 0.13.105's own last line: **none of this has been
played.**

- [ ] **Step 4: Bump the version**

Patch bump to `0.13.106` — a patch bump does not move the internal
dependency requirements, so no `Cargo.toml` dep line changes.

```bash
cargo build --workspace 2>&1 | tail -5
```

- [ ] **Step 5: Full suite, then commit**

```bash
cargo test --workspace 2>&1 | tail -15
```

```bash
git add CHANGELOG.md docs/seams.md Cargo.toml Cargo.lock
git commit -m "release: 0.13.106"
```

- [ ] **Step 6: Stop.** Do not merge, tag or push — landing is the
  `deploy` skill's sequence and the user's call.

---

## Not in this phase

Closing any existing gear door (spec §9.2 — that is the tuning lever and it
wants a play session behind it), the boss floor travelling with a downed
program, turning `FIGHT_CONDITION_WEIGHT` on, replacing
`MAX_DOWNED_PROGRAMS`' flat count with a carried-weight metric, and a help
page for extraction. Phase 4 (the bulk work-order path) is untouched.
