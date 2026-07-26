# Player Ability Unlocks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the player their own battle abilities, granted permanently by research nodes.

**Architecture:** `ResearchDef` gains an `unlocks_abilities: Vec<AbilityId>` field. The player's ability list is *derived* on demand from the already-saved `Research` set rather than stored, so no component, resource or save field is added. A single `Game::actor_abilities(entity)` dispatch sends the player down the research path and companions down the existing species path, leaving the whole battle resolution chain untouched.

**Tech Stack:** Rust, `bevy_ecs` (standalone), `serde` + `ron` for data files. Workspace crates: `feral-processes-engine` (all logic here), `feral-processes-app-core` (one regression test).

**Spec:** `docs/superpowers/specs/2026-07-26-player-ability-unlocks-design.md`

## Global Constraints

- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings rather than silencing them.
- `cargo test --workspace` is the final gate. Baseline before this work: **484 tests passing, ~3s**.
- New schema fields are `#[serde(default)]` so existing and modded `.ron` files keep parsing untouched.
- A malformed or partially-invalid `.ron` file is skipped or trimmed with a **logged warning, never a panic**.
- Update the matching `assets/*/README.md` in the same change as any schema change.
- No flaky tests: no `sleep()`, no wall-clock dependence, no unseeded RNG. `Game::new` takes an explicit seed.
- Comments explain *why*, never *what*.
- Tests needing more than one party slot go in the engine, not app-core: app-core battles are always one group and one slot.
- Work happens on branch `feat/player-ability-unlocks` (already created; the design doc is committed there as `e37298a`).

---

### Task 1: `unlocks_abilities` on `ResearchDef`

Adds the schema field and its validation. Nothing consumes it yet — this task is done when a research file can name abilities and a bad name is survivable.

**Files:**
- Modify: `crates/engine/src/research.rs` (the `ResearchDef` struct at :28-41, `load_dir` at :48-56, the `load` test helper at :135-149, the shipped-tree test at :248)
- Modify: `crates/engine/src/game/lifecycle.rs:578-580` (the one production call site)

**Interfaces:**
- Consumes: `abilities::AbilityDb` (already loaded at `lifecycle.rs:573`, before research at `:579` — no reordering needed), `AbilityDb::get(&str) -> Option<&AbilityDef>`
- Produces: `ResearchDef::unlocks_abilities: Vec<AbilityId>`; `ResearchDb::load_dir(dir: &Path, structures: &StructureDb, abilities: &AbilityDb) -> std::io::Result<(Self, Vec<String>)>`

- [ ] **Step 1: Extend the test helper to build an `AbilityDb`**

In `crates/engine/src/research.rs`, replace the body of the `load` helper in `mod tests` (currently at :135-149) so it also loads the real abilities:

```rust
    /// Writes `files` as `.ron` into a fresh temp dir and loads a `ResearchDb`
    /// from it against a `StructureDb` and `AbilityDb` built from the real
    /// assets — so `unlocks_structures` and `unlocks_abilities` validation
    /// both run against real ids.
    fn load(tag: &str, files: &[(&str, &str)]) -> (ResearchDb, Vec<String>) {
        let dir = std::env::temp_dir().join(format!("feral_research_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
        }
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (structures, _) = StructureDb::load_dir(&assets.join("structures")).unwrap();
        let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
        let result = ResearchDb::load_dir(&dir, &structures, &abilities).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }
```

- [ ] **Step 2: Write the failing tests**

Add to `mod tests` in `crates/engine/src/research.rs`:

```rust
    #[test]
    fn a_node_may_unlock_abilities() {
        let node = r#"(
            id: "self_exec",
            name: "Self-Execution",
            description: "Run a routine yourself.",
            cost: 12,
            unlocks_abilities: ["priority_boost"],
        )"#;
        let (db, warnings) = load("grants_ability", &[("self_exec", node)]);
        let def = db.get("self_exec").expect("valid node should load");
        assert_eq!(def.unlocks_abilities, vec!["priority_boost".to_string()]);
        assert!(warnings.is_empty(), "a valid node warns about nothing");
    }

    /// A node can also unlock structures and recipes, so one bad ability id
    /// must not take the whole node — and everything else it grants — with
    /// it. Mirrors how `SpeciesDb::load_dir` treats an unknown ability.
    #[test]
    fn an_unknown_ability_id_is_dropped_but_the_node_survives() {
        let node = r#"(
            id: "automation",
            name: "Automation",
            description: "Self-running compile jobs.",
            cost: 8,
            unlocks_structures: ["compiler"],
            unlocks_abilities: ["priority_boost", "no_such_ability"],
        )"#;
        let (db, warnings) = load("unknown_ability", &[("automation", node)]);
        let def = db.get("automation").expect("the node itself must survive");
        assert_eq!(
            def.unlocks_abilities,
            vec!["priority_boost".to_string()],
            "the unknown id is dropped and the known one kept"
        );
        assert_eq!(
            def.unlocks_structures,
            vec!["compiler".to_string()],
            "the node's other unlocks are untouched"
        );
        assert_eq!(warnings.len(), 1, "the dropped id explains itself");
    }
```

Also add one assertion to the existing `a_valid_node_loads_with_defaulted_optional_fields` test, next to the other default checks:

```rust
        assert!(
            def.unlocks_abilities.is_empty(),
            "unlocks_abilities defaults to empty"
        );
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine research::tests`
Expected: FAIL to compile — `no field 'unlocks_abilities' on type 'ResearchDef'`, and `load_dir` takes 2 arguments but 3 were supplied.

- [ ] **Step 4: Add the field**

In `crates/engine/src/research.rs`, add to `ResearchDef` after `unlocks_recipes`:

```rust
    /// Abilities the player may use in battle once this node is researched
    /// (see `Game::player_abilities`). The abilities themselves are data in
    /// `assets/abilities/`; naming one here is the only way the player gets
    /// it, since unlike a companion the player has no species to grant one.
    /// `#[serde(default)]` so existing research files — including mods —
    /// keep parsing.
    #[serde(default)]
    pub unlocks_abilities: Vec<crate::abilities::AbilityId>,
```

- [ ] **Step 5: Validate on load**

In `crates/engine/src/research.rs`, change the `load_dir` signature and its parse arm. The doc comment gains the ability clause:

```rust
    /// Loads every `*.ron` research node in `dir`, then drops any node that
    /// can never be reached or acted on: one naming an unknown prereq, or an
    /// unknown structure in `unlocks_structures`. Both would otherwise sit in
    /// the menu forever with no explanation. Dropping cascades — a node whose
    /// prereq was itself dropped is equally unreachable — so validation runs
    /// to a fixpoint. Malformed files are skipped with a warning rather than
    /// aborting the load, so one bad mod file can't crash startup.
    ///
    /// An unknown id in `unlocks_abilities` is treated more gently than an
    /// unknown structure: the id is dropped and the node kept, because a node
    /// also unlocks structures and recipes and killing it over one bad
    /// ability id would silently remove content the modder never touched.
    pub fn load_dir(
        dir: &Path,
        structures: &StructureDb,
        abilities: &crate::abilities::AbilityDb,
    ) -> std::io::Result<(Self, Vec<String>)> {
```

Replace the `Ok(def)` arm of the `ron::from_str` match with:

```rust
                Ok(mut def) => {
                    // `id` is cloned out first because `retain`'s closure
                    // borrows it while `unlocks_abilities` is borrowed
                    // mutably — same shape as `SpeciesDb::load_dir`.
                    let id = def.id.clone();
                    def.unlocks_abilities.retain(|ability| {
                        let known = abilities.get(ability).is_some();
                        if !known {
                            warnings.push(format!(
                                "research {id:?}: unknown ability {ability:?} — dropped"
                            ));
                        }
                        known
                    });
                    db.nodes.insert(def.id.clone(), def);
                }
```

- [ ] **Step 6: Update the other two call sites**

In `crates/engine/src/research.rs`, in the `the_shipped_tree_loads_clean` test (:248):

```rust
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (structures, _) = StructureDb::load_dir(&assets.join("structures")).unwrap();
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
        let (db, warnings) =
            ResearchDb::load_dir(&assets.join("research"), &structures, &abilities).unwrap();
```

In `crates/engine/src/game/lifecycle.rs:578-580`:

```rust
    let (research, research_warnings) =
        ResearchDb::load_dir(&assets_dir.join("research"), &structures, &abilities)?;
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine research`
Expected: PASS, including `a_node_may_unlock_abilities`, `an_unknown_ability_id_is_dropped_but_the_node_survives`, and the pre-existing `the_shipped_tree_loads_clean`.

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/research.rs crates/engine/src/game/lifecycle.rs
git commit -m "feat: let a research node unlock abilities

An unknown ability id trims the list and warns rather than dropping the
whole node, since a node's structures and recipes are innocent of it."
```

---

### Task 2: Ship the three research nodes

Pure data plus its schema doc. Nothing reads `unlocks_abilities` yet, so the deliverable is: the shipped tree still loads clean with three more nodes in it.

**Files:**
- Create: `assets/research/self_exec.ron`, `assets/research/runtime_patching.ron`, `assets/research/kernel_privileges.ron`
- Modify: `crates/engine/src/research.rs:253` (the shipped-node count)
- Modify: `assets/research/README.md`

**Interfaces:**
- Consumes: `ResearchDef::unlocks_abilities` from Task 1
- Produces: research ids `self_exec`, `runtime_patching`, `kernel_privileges`, granting `priority_boost`, `hot_patch`, `null_route` respectively — Task 3's tests research these by id

- [ ] **Step 1: Update the count assertion to the failing value**

In `crates/engine/src/research.rs`, in `the_shipped_tree_loads_clean`:

```rust
        assert_eq!(db.all().count(), 15, "15 nodes ship with the game");
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p feral-processes-engine the_shipped_tree_loads_clean`
Expected: FAIL — `assertion \`left == right\` failed: 15 nodes ship with the game / left: 12 / right: 15`

- [ ] **Step 3: Write the three node files**

`assets/research/self_exec.ron`:

```ron
(
    id: "self_exec",
    name: "Self-Execution",
    description: "Run a routine yourself in battle. Unlocks Priority Boost.",
    cost: 12,
    unlocks_abilities: ["priority_boost"],
)
```

`assets/research/runtime_patching.ron`:

```ron
(
    id: "runtime_patching",
    name: "Runtime Patching",
    description: "Patch an ally back together mid-fight. Unlocks Hot Patch.",
    cost: 28,
    requires: ["self_exec"],
    unlocks_abilities: ["hot_patch"],
)
```

`assets/research/kernel_privileges.ron`:

```ron
(
    id: "kernel_privileges",
    name: "Kernel Privileges",
    description: "Halt every hostile process at once. Unlocks Null Route.",
    cost: 48,
    requires: ["runtime_patching"],
    unlocks_abilities: ["null_route"],
)
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p feral-processes-engine research`
Expected: PASS. `the_shipped_tree_loads_clean` also asserts `warnings.is_empty()`, which proves all three ability ids resolve against `assets/abilities/`.

- [ ] **Step 5: Document the field**

In `assets/research/README.md`, add `unlocks_abilities` to the schema block and its explanation alongside `unlocks_structures` / `unlocks_recipes`. Read the file first and match its existing voice; the content to convey:

- `unlocks_abilities: ["ability_id", ...]`, optional, defaults to empty.
- Every id must name a file in `assets/abilities/`. An unknown id is dropped with a logged warning and the rest of the node still loads.
- This is how the **player** gets abilities. Companions get theirs from their species file instead; a node listed here does not change any companion's kit.
- Naming the same ability from two nodes is allowed — the player's list shows it once, whichever node is researched first.

- [ ] **Step 6: Commit**

```bash
cargo fmt
git add assets/research crates/engine/src/research.rs
git commit -m "feat: ship the three player-ability research nodes

Self-Execution (12) -> Priority Boost, Runtime Patching (28) -> Hot Patch,
Kernel Privileges (48) -> Null Route. Costs calibrated against the existing
tree; unplayed, like every balance number here."
```

---

### Task 3: Derive and dispatch the player's abilities

The engine half of the feature: the player's list exists and resolution honours it. The battle menu still hides the row, so this task is verified by tests rather than by playing.

**Files:**
- Modify: `crates/engine/src/game/combat.rs` (add both methods near `companion_abilities` at :387; switch `battle_special_options` at :469)
- Modify: `crates/engine/src/game/party.rs:126` (`companion_ability_label`)
- Modify: `crates/engine/src/game/combat_round.rs:114` and `:242`
- Modify: `crates/engine/src/tests/support.rs:153-186` (add an `extra_research` parameter to `modded_assets_dir`)
- Modify: `crates/engine/src/tests/support.rs:193`, `:546`, `crates/engine/src/tests/taming.rs:66`, `:129`, `:163`, `crates/engine/src/tests/combat_abilities.rs:153` (pass `&[]` for the new parameter)
- Test: `crates/engine/src/tests/combat_abilities.rs`

**Interfaces:**
- Consumes: research ids from Task 2; `Game::is_researched(&str) -> bool` (`unlocks.rs:107`); `ResearchDb::all()` (sorted by cost then id); `AbilityDb::get`; test helpers `test_assets_dir()`, `unlock_research_chain(&mut Game, &str)`, `insert_battle`, `resolve_round_with` from `crates/engine/src/tests/support.rs`
- Produces: `Game::player_abilities(&self) -> Vec<AbilityDef>` (public); `Game::actor_abilities(&self, entity: Entity) -> Vec<AbilityDef>` (`pub(crate)`) — Task 4's menu calls both

- [ ] **Step 1: Add an `extra_research` parameter to the modded-assets helper**

In `crates/engine/src/tests/support.rs`, `modded_assets_dir` currently takes `(tag, omit_items, extra_items, extra_species)`. Add a fifth parameter mirroring `extra_species` exactly:

```rust
pub(super) fn modded_assets_dir(
    tag: &str,
    omit_items: &[&str],
    extra_items: &[(&str, &str)],
    extra_species: &[(&str, &str)],
    extra_research: &[(&str, &str)],
) -> std::path::PathBuf {
```

and, beside the existing `extra_species` write loop at the end of the body:

```rust
    for (name, body) in extra_research {
        std::fs::write(dir.join("research").join(name), body).unwrap();
    }
```

Then add `&[]` as the final argument at all six existing call sites: `support.rs:193`, `support.rs:546`, `taming.rs:66`, `taming.rs:129`, `taming.rs:163`, `combat_abilities.rs:153`.

- [ ] **Step 2: Write the failing tests**

Add to `crates/engine/src/tests/combat_abilities.rs`:

```rust
#[test]
fn the_player_has_no_abilities_until_they_research_one() {
    let game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.player_abilities().is_empty(),
        "the player starts with nothing to spend a Special on — that's what \
         the research is selling"
    );
}

#[test]
fn researching_self_execution_grants_the_player_priority_boost() {
    let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "self_exec");

    let ids: Vec<String> = game
        .player_abilities()
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(ids, vec!["priority_boost".to_string()]);
}

/// Two nodes may legitimately name the same ability — a mod branching the
/// tree, say. The picker must not then show it twice, because the duplicate
/// rows would be indistinguishable and one of them a lie.
#[test]
fn an_ability_granted_by_two_nodes_appears_once() {
    const ALSO_BOOST: &str = r#"(
        id: "also_boost",
        name: "Redundant Routine",
        description: "Grants what self_exec already grants.",
        cost: 12,
        unlocks_abilities: ["priority_boost"],
    )"#;
    let dir = modded_assets_dir("dup_ability", &[], &[], &[], &[("also_boost.ron", ALSO_BOOST)]);
    let mut game = Game::new(33, DifficultyMode::Forgiving, &dir).unwrap();
    unlock_research_chain(&mut game, "self_exec");
    unlock_research_chain(&mut game, "also_boost");

    let ids: Vec<String> = game
        .player_abilities()
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(ids, vec!["priority_boost".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `HashMap` iteration is randomized per instance, so a derived list has to
/// be sorted somewhere. It's sorted in `ResearchDb::all` — cheapest node
/// first — and this pins that the derivation preserves it.
#[test]
fn the_players_abilities_are_ordered_cheapest_node_first() {
    let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "kernel_privileges");

    let ids: Vec<String> = game
        .player_abilities()
        .into_iter()
        .map(|a| a.id)
        .collect();
    assert_eq!(
        ids,
        vec![
            "priority_boost".to_string(),
            "hot_patch".to_string(),
            "null_route".to_string(),
        ],
        "self_exec (12), runtime_patching (28), kernel_privileges (48)"
    );
}

/// The player's Special goes through the same resolution path a companion's
/// does — so the cooldown must arm on the player's own entity, and the
/// effect must land.
#[test]
fn a_player_special_applies_its_effect_and_arms_the_players_cooldown() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "runtime_patching");
    let player = game.player_entity();
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    // Hot Patch is the second node's grant, so index 1 in research order.
    let hot_patch = game
        .player_abilities()
        .iter()
        .position(|a| a.id == "hot_patch")
        .expect("runtime_patching grants hot_patch");
    game.world.get_mut::<Stats>(player).unwrap().hp = 1;

    resolve_round_with(
        &mut game,
        BattleAction::Special {
            ability: hot_patch,
            target: battle::SpecialTarget::Ally { slot: 0 },
        },
    );

    assert!(
        game.world.get::<Stats>(player).unwrap().hp > 1,
        "the player patched themselves, so their Integrity must have gone up"
    );
    assert_eq!(
        game.world
            .get::<AbilityCooldowns>(player)
            .and_then(|c| c.0.get("hot_patch").copied()),
        Some(2),
        "cooldown 1 is armed as 1 + 1 so this round's tick doesn't eat it"
    );
}

/// Commanding an ability spends the *player's* Fatigue, which is what keeps
/// a top-tier routine a budget decision rather than a free extra action.
/// Measured as a delta around the round, the same way
/// `fatigue_spent_commanding_companion` does it.
#[test]
fn a_player_special_spends_the_players_fatigue_once() {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "kernel_privileges");
    let player = game.player_entity();
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    let abilities = game.player_abilities();
    let index = abilities
        .iter()
        .position(|a| a.id == "null_route")
        .expect("kernel_privileges grants null_route");
    let cost = abilities[index].fatigue_cost;
    assert!(
        cost > 0.0,
        "null_route is the first researched routine that costs Fatigue"
    );
    let before = game.world.get::<Needs>(player).unwrap().fatigue;

    resolve_round_with(
        &mut game,
        BattleAction::Special {
            ability: index,
            target: battle::SpecialTarget::AllEnemies,
        },
    );

    let after = game.world.get::<Needs>(player).unwrap().fatigue;
    assert_eq!(before - after, cost, "charged exactly once");
}

/// The player's routines are derived from `Research`, which the save
/// already carries. This proves the no-new-save-field claim rather than
/// asserting it.
#[test]
fn a_save_round_trip_preserves_the_players_abilities() {
    let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "runtime_patching");
    let before: Vec<String> = game.player_abilities().into_iter().map(|a| a.id).collect();
    assert_eq!(before.len(), 2, "priority_boost and hot_patch");

    let path = std::env::temp_dir().join(format!(
        "feral_player_abilities_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let after: Vec<String> = loaded.player_abilities().into_iter().map(|a| a.id).collect();
    assert_eq!(after, before);
}

/// The fallback exists so a companion's menu is never empty. It must not
/// leak onto the player, or the first research node would sell something
/// already owned.
#[test]
fn the_companion_fallback_does_not_leak_onto_the_player() {
    let mut game = Game::new(36, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 20, 5);

    assert!(game.player_abilities().is_empty());
    assert!(
        !game.actor_abilities(companion).is_empty(),
        "a companion always resolves at least the fallback"
    );
    assert!(
        game.actor_abilities(game.player_entity()).is_empty(),
        "the player gets no fallback"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine combat_abilities`
Expected: FAIL to compile — `no method named 'player_abilities'` / `no method named 'actor_abilities'` found for struct `Game`.

- [ ] **Step 4: Implement the derivation and dispatch**

In `crates/engine/src/game/combat.rs`, immediately after `companion_abilities` (which ends at :409), add:

```rust
    /// The abilities the player has unlocked through research, in research
    /// order (see `ResearchDb::all`), each appearing once however many nodes
    /// grant it.
    ///
    /// Unlike `companion_abilities` this may be empty, and deliberately so:
    /// before any node is researched the player has no routines at all,
    /// which is exactly what the research is selling. Nothing is stored —
    /// the set is derived from `Research`, which the save already carries,
    /// the same way structure and recipe unlocks are.
    pub fn player_abilities(&self) -> Vec<AbilityDef> {
        let abilities = self.world.resource::<AbilityDb>();
        let mut seen = std::collections::HashSet::new();
        self.world
            .resource::<ResearchDb>()
            .all()
            .filter(|def| self.is_researched(&def.id))
            .flat_map(|def| def.unlocks_abilities.iter())
            .filter(|id| seen.insert((*id).clone()))
            .filter_map(|id| abilities.get(id).cloned())
            .collect()
    }

    /// Every ability the combatant at `entity` can be commanded to use: the
    /// player's researched routines, or a companion's species list. Menu and
    /// resolution both go through this, so the two cannot disagree about
    /// what a slot knows.
    pub(crate) fn actor_abilities(&self, entity: Entity) -> Vec<AbilityDef> {
        if entity == self.player_entity() {
            self.player_abilities()
        } else {
            self.companion_abilities(entity)
        }
    }
```

If `ResearchDb` isn't already in scope in `combat.rs` (the file opens with `use crate::*;`), add whatever import `cargo build` asks for rather than guessing.

- [ ] **Step 5: Switch the four call sites**

Replace `self.companion_abilities(...)` with `self.actor_abilities(...)` at exactly these four places, leaving everything around them alone:

- `crates/engine/src/game/combat.rs:469` — in `battle_special_options`
- `crates/engine/src/game/party.rs:126` — in `companion_ability_label`
- `crates/engine/src/game/combat_round.rs:114` — in the `BattleAction::Special` resolution arm
- `crates/engine/src/game/combat_round.rs:242` — in `action_label`

`companion_abilities` itself stays exactly as it is: it remains the companion path, fallback included.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine combat_abilities`
Expected: PASS, all eight new tests.

Then run the neighbours most likely to be disturbed by the dispatch swap:

Run: `cargo test -p feral-processes-engine combat_specials`
Expected: PASS — companion Specials are unchanged.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src
git commit -m "feat: derive the player's abilities from researched nodes

actor_abilities dispatches the player to research and companions to their
species, so the whole resolution chain is untouched. The player's list is
allowed to be empty — the companion fallback must not leak onto them."
```

---

### Task 4: Offer the player a Special row

The player-facing half. After this task the feature is playable.

**Files:**
- Modify: `crates/engine/src/game/combat.rs:537-546` (the `if !is_player` guard)
- Modify: `crates/engine/src/game/party.rs:120-132` (rename `companion_ability_label` and handle the empty case) and `party.rs:118` (its call in `companion_info`)
- Test: `crates/engine/src/tests/combat.rs`, `crates/app-core/src/tests/battle.rs`
- Modify: `docs/manual.md:138`, `assets/abilities/README.md`

**Interfaces:**
- Consumes: `Game::actor_abilities` from Task 3
- Produces: an `ActionOption { kind: ActionKind::Special, key: 's' }` in `battle_action_options(0)`, carrying `unavailable: Some("no routines researched")` while the player's list is empty

- [ ] **Step 1: Write the failing engine tests**

Add to `crates/engine/src/tests/combat.rs`:

```rust
/// A hidden row teaches nobody the feature exists; a greyed one with a
/// reason points at the research tree.
#[test]
fn the_player_is_offered_a_greyed_special_before_researching_one() {
    let mut game = Game::new(37, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    let special = game
        .battle_action_options(0)
        .into_iter()
        .find(|o| o.kind == ActionKind::Special)
        .expect("the player's Special row is shown, not hidden");
    assert_eq!(
        special.unavailable.as_deref(),
        Some("no routines researched")
    );
}

#[test]
fn researching_a_routine_makes_the_players_special_available() {
    let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "self_exec");
    let player = game.player_entity();
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    let special = game
        .battle_action_options(0)
        .into_iter()
        .find(|o| o.kind == ActionKind::Special)
        .expect("the Special row is still there");
    assert_eq!(special.unavailable, None);
    assert_eq!(
        special.detail, "Priority Boost",
        "one ability reads as its own name"
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine tests::combat::the_player_is_offered_a_greyed_special tests::combat::researching_a_routine`
Expected: FAIL — `the player's Special row is shown, not hidden` (the option isn't built for slot 0 yet).

- [ ] **Step 3: Make the ability label handle an empty list**

In `crates/engine/src/game/party.rs`, rename `companion_ability_label` to `ability_label` (it now describes the player too) and add the empty arm:

```rust
    /// Terse label for what commanding `entity` in battle would do right
    /// now. A member with several abilities reads as a count, since no one
    /// of them is *the* answer until the player picks in
    /// `Mode::BattleSpecial`.
    pub(crate) fn ability_label(&self, entity: Entity) -> String {
        match self.actor_abilities(entity).as_slice() {
            // Only the player can be empty: `companion_abilities` resolves
            // the fallback rather than returning nothing.
            [] => "No routines researched".to_string(),
            [only] => only.name.clone(),
            many => format!("{} abilities", many.len()),
        }
    }
```

Update its call in `companion_info` (`party.rs:118`) to `ability: self.ability_label(entity),`.

- [ ] **Step 4: Offer the row to every slot**

In `crates/engine/src/game/combat.rs`, delete this block (:537-546):

```rust
        if !is_player {
            options.push(ActionOption {
                kind: ActionKind::Special,
                key: 's',
                label: "[s]pecial".to_string(),
                detail: self.companion_ability_label(entity),
                target: TargetSpec::SpecialAbility,
                unavailable: None,
            });
        }
```

and replace it with an unconditional push:

```rust
        options.push(ActionOption {
            kind: ActionKind::Special,
            key: 's',
            label: "[s]pecial".to_string(),
            detail: self.ability_label(entity),
            target: TargetSpec::SpecialAbility,
            // Only the player can be empty here, and only until they
            // research their first routine.
            unavailable: self
                .actor_abilities(entity)
                .is_empty()
                .then(|| "no routines researched".to_string()),
        });
```

This leaves the player's menu ordered `a`, `d`, `s`, `c`, `u`, and a companion's unchanged.

- [ ] **Step 5: Run the engine tests to verify they pass**

Run: `cargo test -p feral-processes-engine tests::combat`
Expected: PASS — including the pre-existing
`battle_action_keys_are_lowercase_with_defend_on_d_and_decompile_on_c`
(`tests/combat.rs:182`), which pins `a`/`d`/`c`/`u` and asserts every
option's label brackets its own key. It neither counts the options nor
asserts Special is absent, so the new row satisfies it as written — the
`[s]pecial` label brackets `s`. If it fails, the new row's label is wrong;
fix the label, not the test.

- [ ] **Step 6: Add the app-core regression test**

The guard that makes the greyed row real already exists at `crates/app-core/src/app/battle.rs:68` — an `unavailable` option sets `status_line` and returns without planning. Pin it against this feature, in `crates/app-core/src/tests/battle.rs`:

```rust
/// The player's Special row is offered before any routine is researched,
/// greyed with a reason. If the guard on `unavailable` ever went away, the
/// keypress would be planned, resolve against an empty ability list, and
/// silently cost the player their round.
#[test]
fn pressing_special_with_no_routines_researched_explains_itself_and_costs_nothing() {
    let mut app = battling_app();
    let offered = app
        .game
        .as_ref()
        .unwrap()
        .battle_action_options(0)
        .into_iter()
        .find(|o| o.kind == ActionKind::Special)
        .expect("the player is offered a Special row");
    assert!(
        offered.unavailable.is_some(),
        "a fresh game has researched nothing"
    );

    app.handle_key(GameKey::Char('s'));

    assert_eq!(app.mode, Mode::Battle, "no picker should have opened");
    assert!(app.pending_battle_action.is_none());
    assert!(
        app.status_line.is_some(),
        "the player must be told why nothing happened"
    );
}
```

Run: `cargo test -p feral-processes-app-core battle`
Expected: PASS. Two pre-existing tests in this file also cover the new row and should stay green without edits — `battle_action_keys_come_from_the_engine_with_only_the_party_pair_case_sensitive` (an unavailable `s` still sets `status_line`, so the key counts as routed) and `every_offered_action_reaches_a_state_that_can_complete_it` (which skips `unavailable` options). If either fails, fix the code rather than the test.

- [ ] **Step 7: Update the two docs the change falsifies**

`docs/manual.md:138` currently reads:

```
| `s` | Special (party members only) — picks one of that member's abilities, then a target if it needs one. A rally (ATK boost) by default, or the species' own abilities if it defines any. Costs you Fatigue — how much the ability decides — and may sit out a few rounds afterwards |
```

Replace that row with:

```
| `s` | Special — picks one of that member's abilities, then a target if it needs one. A companion's come from its species and unlock as it levels; yours come from research — Self-Execution first, then Runtime Patching and Kernel Privileges — and the row sits greyed until you have researched one. Costs you Fatigue — how much the ability decides — and may sit out a few rounds afterwards |
```

In `assets/abilities/README.md`, the opening prose says an ability is "what a companion spends its round on when commanded with Special in battle" and that "Which abilities a companion has comes from its species file". Extend both: the player can spend a round on one too, and the player's come from `unlocks_abilities` on a research node (cross-reference `../research/README.md`). Leave the schema section alone — `AbilityDef` is unchanged.

- [ ] **Step 8: Full suite, then commit**

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

Expected: PASS. Baseline was 484 tests; this plan adds 13 (2 in Task 1, 8 in Task 3, 2 engine + 1 app-core in Task 4), so expect **497 passing**.

```bash
git add crates/engine/src crates/app-core/src docs/manual.md assets/abilities/README.md
git commit -m "feat: give the player a Special row of researched routines

Greyed with a reason until the first node is researched, rather than
hidden — a hidden row teaches nobody the feature exists."
```

---

## Verification

The feature is complete when, from a fresh game:

1. `[s]pecial` appears in the player's battle menu, greyed, reading "no routines researched".
2. Researching Self-Execution (12 Research Data) makes it available, offering Priority Boost.
3. Using it buffs the chosen ally's ATK for 3 rounds and spends the player's round.
4. Runtime Patching and Kernel Privileges each add a row to the picker, in that order.
5. `cargo test --workspace` passes.

Balance is unplayed, as with every number in this repo. The thing most likely to want a tuning pass is the first two nodes being free of Fatigue cost — see the spec's "Known balance property".
