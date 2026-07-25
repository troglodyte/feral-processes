# Abilities Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make combat abilities data-driven `.ron` content with multi-target shapes (whole enemy group, all enemies, whole party), cooldowns, and per-ability Fatigue costs.

**Architecture:** A new `AbilityDb` asset database mirrors the existing `ItemDb`/`SpeciesDb`/`ResearchDb` pattern. The four-variant `species::SpecialAbility` enum is deleted; species files reference abilities by string id with a learn level. Enemy back-rank members become damageable and killable by generalizing the front-only group reaper. Cooldowns are a battle-scoped component with the exact lifecycle `CombatBuff` already has, so no save-format change is needed.

**Tech Stack:** Rust, `bevy_ecs` (standalone), `ron` for assets, `serde`. 4-crate Cargo workspace.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-25-abilities-design.md`. Read it before starting.
- Branch: `abilities` (already created, off `main`).
- New fields on `SpeciesDef`/`ItemDef`/`StructureDef`/`AbilityDef` MUST be `#[serde(default)]` so existing `.ron` files — including mods — keep parsing.
- A malformed `.ron` file MUST be skipped with a logged warning, never a panic. Follow `SpeciesDb::load_dir` / `ItemDb::load_dir`.
- Comments explain *why*, never *what*. No `// removed` markers, no backwards-compat shims.
- No flaky tests: no `sleep()`, no wall-clock dependence, no unseeded RNG. Background systems (habitat spawning, nests) will interfere with naive assertions.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings rather than silencing them.
- `cargo test --workspace` is the gate at every task boundary. Measured baseline: **469 workspace tests (engine 392, app-core 37, gui 38, font 2)**. CLAUDE.md's "433 tests" is stale — ignore it.
- All new tests land in the engine crate. Expected engine counts per task: **396 → 399 → 398 → 401 → 406 → 407** (workspace 484 at the end). Treat these as a sanity check, not a hard gate — an off-by-one from a test reasonably split or merged is fine, a drop of several is not.
- Reference values, already verified in the tree: `PLAYER_STRIKE_POWER` 5, `DEFEND_DEF_BONUS` 6, `RALLY_DURATION` 3, `COMPANION_COMMAND_FATIGUE_COST` 5.0, `MAX_ENEMY_GROUPS` 4, `ENGAGED_GROUPS` 2, `FRONT_SLOTS` 3, `MAX_PARTY_SIZE` 5, `CREATURE_MAX_LEVEL` 12.
- `Needs.fatigue` is displayed as **"Fatigue"**; `Needs.hunger` is displayed as **"Power"**. Ability costs come off `fatigue`.

### Known merge hazard

`crates/app-core/src/lib.rs` is a single 3,282-line file on this branch. Branch `fix/popup-hides-rows-that-fit` has already split it into `crates/app-core/src/app/*.rs`. Task 4 modifies ~20 lines of it. Expect a conflict when these branches meet; the resolution is to re-apply Task 4's `SpecialTargeting::None` arm and `action_from` changes into `crates/app-core/src/app/battle.rs`.

---

## File Structure

**Created:**
- `crates/engine/src/abilities.rs` — `AbilityId`, `AbilityTarget`, `AbilityEffect`, `AbilityDef`, `AbilityDb`, plus loader unit tests.
- `assets/abilities/*.ron` — 10 shipped ability definitions.
- `assets/abilities/README.md` — schema reference for modders.
- `crates/engine/src/tests/combat_abilities.rs` — behavior tests for the new shapes, cooldowns and costs.

**Modified:**
- `crates/engine/src/lib.rs` — `pub mod abilities;`, imports.
- `crates/engine/src/species.rs` — `SpeciesAbility`; `special_abilities` → `abilities`; delete `SpecialAbility`, `SpecialTargeting`, `legacy_special_ability`.
- `crates/engine/src/battle.rs` — `SpecialTargeting` moves here; `SpecialTarget` gains `WholeParty`/`AllEnemies`; `SpecialOption` gains `unavailable`.
- `crates/engine/src/components.rs` — `AbilityCooldowns`.
- `crates/engine/src/game/lifecycle.rs` — load and insert `AbilityDb`.
- `crates/engine/src/game/combat.rs` — `companion_abilities`, `battle_special_options`, `battle_set_action`.
- `crates/engine/src/game/combat_round.rs` — `use_ability`, recipient resolution, `finish_member`, `remove_member`.
- `crates/engine/src/game/combat_status.rs` — `reap_dead_members`, cooldown tick and clear.
- `crates/app-core/src/lib.rs` — `SpecialTargeting::None` arm, `action_from`.
- `crates/engine/src/tests/support.rs` — `modded_assets_dir` copies `abilities`; `TWO_ABILITY_SPECIES` migrated.
- `crates/engine/src/tests/combat_specials.rs` — migrated off `SpecialAbility::`.
- `crates/engine/src/tests/mod.rs` — register `combat_abilities`.
- `assets/species/README.md`, `README.md`, `CHANGELOG.md`.

---

### Task 1: `AbilityDb` and the shipped ability set

Purely additive — nothing consumes the database yet, so the suite stays green throughout.

**Files:**
- Create: `crates/engine/src/abilities.rs`
- Create: `assets/abilities/` — 10 `.ron` files
- Create: `assets/abilities/README.md`
- Modify: `crates/engine/src/lib.rs` (module declaration)
- Modify: `crates/engine/src/game/lifecycle.rs:551-592` (`AssetDbs`, `load_asset_dbs`, resource insertion)
- Modify: `crates/engine/src/tests/support.rs` (`modded_assets_dir` copy loop)

**Interfaces:**
- Produces:
  - `abilities::AbilityId` = `String`
  - `abilities::AbilityTarget` — `OneAlly | WholeParty | OneEnemyGroupFront | WholeEnemyGroup | AllEnemies`
  - `abilities::AbilityEffect` — `Damage { power: i32, status: Option<MoveEffect> } | Heal { power: i32 } | Buff { kind: BuffKind, power: i32, duration: u32 } | Debuff { kind: StatusKind, power: i32, duration: u32 }`
  - `abilities::AbilityDef { id, name, description, target, effect, cooldown: u32, fatigue_cost: f32 }`
  - `abilities::AbilityDb::load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)>`
  - `abilities::AbilityDb::get(&self, id: &str) -> Option<&AbilityDef>`
  - `abilities::AbilityDb::all(&self) -> impl Iterator<Item = &AbilityDef>`
  - `abilities::FALLBACK_ABILITY_ID: &str = "priority_boost"`

**Not in this task:** `AbilityTarget::targeting()` is added in Task 3, not here. It returns `SpecialTargeting::None`, a variant that does not exist until Task 3 moves and extends that enum — writing it here does not compile. Nothing in Task 1 needs it.

- [ ] **Step 1: Write the failing loader tests**

Create `crates/engine/src/abilities.rs`. Put the tests at the bottom; they mirror `research.rs`'s test module exactly.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `files` as `.ron` into a fresh temp dir and loads an
    /// `AbilityDb` from it.
    fn load(tag: &str, files: &[(&str, &str)]) -> (AbilityDb, Vec<String>) {
        let dir = std::env::temp_dir().join(format!("feral_abilities_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
        }
        let result = AbilityDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    const VALID: &str = r#"(
        id: "test_sweep",
        name: "Test Sweep",
        description: "Damage 6 to one group.",
        target: WholeEnemyGroup,
        effect: Damage(power: 6),
    )"#;

    #[test]
    fn a_valid_def_loads_with_defaulted_optional_fields() {
        let (db, warnings) = load("valid", &[("test_sweep", VALID)]);
        let def = db.get("test_sweep").expect("valid ability should load");
        assert_eq!(def.name, "Test Sweep");
        assert_eq!(def.target, AbilityTarget::WholeEnemyGroup);
        assert_eq!(def.cooldown, 0, "cooldown defaults to none");
        assert_eq!(
            def.fatigue_cost,
            crate::COMPANION_COMMAND_FATIGUE_COST,
            "an ability declaring no cost charges what commanding always did"
        );
        assert!(warnings.is_empty(), "a valid def warns about nothing");
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_the_rest_still_load() {
        let (db, warnings) = load(
            "malformed",
            &[("test_sweep", VALID), ("broken", "(this is not ron")],
        );
        assert!(
            db.get("test_sweep").is_some(),
            "one bad mod file must not take the others down"
        );
        assert_eq!(warnings.len(), 1, "exactly the bad file should warn");
        assert!(warnings[0].contains("broken"));
    }

    #[test]
    fn all_is_ordered_by_id() {
        let b = r#"(id: "b", name: "B", description: "d", target: OneAlly, effect: Heal(power: 1))"#;
        let a = r#"(id: "a", name: "A", description: "d", target: OneAlly, effect: Heal(power: 1))"#;
        let (db, _) = load("order", &[("b", b), ("a", a)]);
        let ids: Vec<&str> = db.all().map(|d| d.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["a", "b"],
            "HashMap order is randomized per instance; the menu must not be"
        );
    }

    #[test]
    fn the_shipped_set_loads_clean() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join("abilities");
        let (db, warnings) = AbilityDb::load_dir(&dir).unwrap();
        assert!(
            warnings.is_empty(),
            "the shipped set must not warn: {warnings:?}"
        );
        assert_eq!(db.all().count(), 10, "10 abilities ship with the game");
        assert!(
            db.get(FALLBACK_ABILITY_ID).is_some(),
            "the fallback ability must ship, or every companion loses its Special"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine abilities`
Expected: FAIL — `abilities` module not declared, `AbilityDb` not found.

- [ ] **Step 3: Write the module**

At the top of `crates/engine/src/abilities.rs`:

```rust
use std::collections::HashMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::components::{BuffKind, StatusKind};
use crate::species::MoveEffect;

pub type AbilityId = String;

/// The ability every companion falls back to when its species declares
/// none. Validated at startup (see `Game::new`) rather than defended at
/// every call site, the same way a missing economy role aborts the load.
pub const FALLBACK_ABILITY_ID: &str = "priority_boost";

/// Who an ability lands on. Which picker the UI opens for it — if any — is
/// `AbilityTarget::targeting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbilityTarget {
    /// One party member the player picks.
    OneAlly,
    /// Every living party member, no picker.
    WholeParty,
    /// The front member of one enemy group the player picks.
    OneEnemyGroupFront,
    /// Every living member of one enemy group the player picks.
    WholeEnemyGroup,
    /// Every living enemy in every group, no picker.
    AllEnemies,
}

/// What an ability does to each of its recipients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AbilityEffect {
    /// Direct damage through `battle::compute_damage`, so it scales with the
    /// user's ATK exactly as a `MoveDef` does, plus an optional status rider
    /// — the same shape a move already has.
    Damage {
        power: i32,
        #[serde(default)]
        status: Option<MoveEffect>,
    },
    Heal {
        power: i32,
    },
    Buff {
        kind: BuffKind,
        power: i32,
        duration: u32,
    },
    Debuff {
        kind: StatusKind,
        power: i32,
        duration: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbilityDef {
    pub id: AbilityId,
    pub name: String,
    /// The one-line detail the ability picker shows. Authored rather than
    /// computed from `effect`, so a modder controls exactly how their
    /// ability reads.
    pub description: String,
    pub target: AbilityTarget,
    pub effect: AbilityEffect,
    /// Battle rounds before this ability can be used again by the same
    /// combatant. `#[serde(default)]` — 0 means usable every round.
    #[serde(default)]
    pub cooldown: u32,
    /// Player Fatigue spent commanding this ability.
    /// `#[serde(default)]` to the flat cost commanding a companion has
    /// always charged, so an ability omitting it behaves as before.
    #[serde(default = "default_fatigue_cost")]
    pub fatigue_cost: f32,
}

fn default_fatigue_cost() -> f32 {
    crate::COMPANION_COMMAND_FATIGUE_COST
}

impl AbilityDef {
    /// Names the first field holding a NaN or infinity, if any. RON accepts
    /// bare `NaN`/`inf` literals and they survive every clamp downstream —
    /// cheaper to refuse the file at load than to defend every read. Same
    /// rationale as `ItemDef::non_finite_field`.
    fn non_finite_field(&self) -> Option<&'static str> {
        if !self.fatigue_cost.is_finite() {
            return Some("fatigue_cost");
        }
        if let AbilityEffect::Damage {
            status: Some(status),
            ..
        } = &self.effect
            && !status.chance.is_finite()
        {
            return Some("effect.status.chance");
        }
        None
    }
}

impl AbilityTarget {
    /// Which picker the UI opens after this ability is chosen. `None` means
    /// it resolves immediately — there is nothing left for the player to
    /// choose.
    pub fn targeting(self) -> crate::battle::SpecialTargeting {
        use crate::battle::SpecialTargeting;
        match self {
            AbilityTarget::OneAlly => SpecialTargeting::Ally,
            AbilityTarget::OneEnemyGroupFront | AbilityTarget::WholeEnemyGroup => {
                SpecialTargeting::Enemy
            }
            AbilityTarget::WholeParty | AbilityTarget::AllEnemies => SpecialTargeting::None,
        }
    }
}

#[derive(Resource, Default)]
pub struct AbilityDb {
    abilities: HashMap<AbilityId, AbilityDef>,
}

impl AbilityDb {
    /// Loads every `*.ron` ability in `dir`. A malformed file is skipped
    /// with a returned warning rather than aborting the load, same as
    /// `ItemDb::load_dir`.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = AbilityDb::default();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<AbilityDef>(&text) {
                Ok(def) => {
                    if let Some(field) = def.non_finite_field() {
                        warnings.push(format!(
                            "skipped invalid ability file {path:?}: {field} is not a finite number"
                        ));
                        continue;
                    }
                    db.abilities.insert(def.id.clone(), def);
                }
                Err(e) => warnings.push(format!("skipped invalid ability file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    pub fn get(&self, id: &str) -> Option<&AbilityDef> {
        self.abilities.get(id)
    }

    /// Every loaded ability, by id. `HashMap` iteration order is randomized
    /// per instance, so without this the picker's numbering would shuffle
    /// between sessions even though nothing about the files changed.
    pub fn all(&self) -> impl Iterator<Item = &AbilityDef> {
        let mut defs: Vec<&AbilityDef> = self.abilities.values().collect();
        defs.sort_by(|a, b| a.id.cmp(&b.id));
        defs.into_iter()
    }
}
```

Add to `crates/engine/src/lib.rs`, keeping the module list alphabetical (it goes first, before `pub mod balance;`):

```rust
pub mod abilities;
```

- [ ] **Step 4: Write the 10 shipped `.ron` files**

Create `assets/abilities/` and write each file. Filenames match ids.

`priority_boost.ron`:
```ron
(
    id: "priority_boost",
    name: "Priority Boost",
    description: "+3 ATK to one ally for 3 rounds",
    target: OneAlly,
    effect: Buff(kind: Atk, power: 3, duration: 3),
)
```

`sandbox.ron`:
```ron
(
    id: "sandbox",
    name: "Sandbox",
    description: "+3 DEF to one ally for 3 rounds",
    target: OneAlly,
    effect: Buff(kind: Def, power: 3, duration: 3),
)
```

`hot_patch.ron`:
```ron
(
    id: "hot_patch",
    name: "Hot Patch",
    description: "Restore 8 Integrity to one ally",
    target: OneAlly,
    effect: Heal(power: 8),
    cooldown: 1,
)
```

`memory_leak.ron`:
```ron
(
    id: "memory_leak",
    name: "Memory Leak",
    description: "Bleed 2 per round for 3 rounds on one target",
    target: OneEnemyGroupFront,
    effect: Debuff(kind: Bleed, power: 2, duration: 3),
    cooldown: 1,
)
```

`deadlock.ron`:
```ron
(
    id: "deadlock",
    name: "Deadlock",
    description: "Stun one target for 1 round",
    target: OneEnemyGroupFront,
    effect: Debuff(kind: Stun, power: 0, duration: 1),
    cooldown: 2,
)
```

`cascade_overflow.ron`:
```ron
(
    id: "cascade_overflow",
    name: "Cascade Overflow",
    description: "Damage 6 to every member of one group",
    target: WholeEnemyGroup,
    effect: Damage(power: 6),
    cooldown: 2,
    fatigue_cost: 8.0,
)
```

`broadcast_storm.ron`:
```ron
(
    id: "broadcast_storm",
    name: "Broadcast Storm",
    description: "Damage 4 to every hostile program on the field",
    target: AllEnemies,
    effect: Damage(power: 4),
    cooldown: 4,
    fatigue_cost: 15.0,
)
```

`null_route.ron`:
```ron
(
    id: "null_route",
    name: "Null Route",
    description: "Stun every hostile program for 1 round",
    target: AllEnemies,
    effect: Debuff(kind: Stun, power: 0, duration: 1),
    cooldown: 5,
    fatigue_cost: 15.0,
)
```

`redundancy_sync.ron`:
```ron
(
    id: "redundancy_sync",
    name: "Redundancy Sync",
    description: "Restore 10 Integrity to the whole party",
    target: WholeParty,
    effect: Heal(power: 10),
    cooldown: 3,
    fatigue_cost: 12.0,
)
```

`overclock_array.ron`:
```ron
(
    id: "overclock_array",
    name: "Overclock Array",
    description: "+3 ATK to the whole party for 3 rounds",
    target: WholeParty,
    effect: Buff(kind: Atk, power: 3, duration: 3),
    cooldown: 3,
    fatigue_cost: 10.0,
)
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine abilities`
Expected: PASS — 4 tests.

- [ ] **Step 6: Wire `AbilityDb` into game startup**

In `crates/engine/src/game/lifecycle.rs`, add `abilities: AbilityDb` to the `AssetDbs` struct (around line 551). In `load_asset_dbs`, load it **first**, because Task 3 will pass it to `SpeciesDb::load_dir`:

```rust
let (abilities, mut warnings) = AbilityDb::load_dir(&assets_dir.join("abilities"))?;
let (species, species_warnings) = SpeciesDb::load_dir(&assets_dir.join("species"))?;
warnings.extend(species_warnings);
```

(The remaining loads are unchanged; note `species` no longer seeds `warnings`.)

Then, after the existing `missing_roles` check and before `Ok(AssetDbs { .. })`, add the fallback-ability check — same fail-fast rationale as the economy roles:

```rust
if abilities.get(abilities::FALLBACK_ABILITY_ID).is_none() {
    return Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "ability set is missing the fallback ability {:?}, which every \
             companion without a declared kit relies on",
            abilities::FALLBACK_ABILITY_ID
        ),
    ));
}
```

Add `abilities` to the returned struct literal, and insert it as a resource at **both** call sites — `Game::new` (near line 24-28) and `Game::load` (near line 116-118):

```rust
world.insert_resource(ability_db);
```

Add the import to `crates/engine/src/lib.rs`'s use block. Only `AbilityDb` is used yet; Task 3 widens this line:

```rust
use abilities::AbilityDb;
```

- [ ] **Step 7: Write the schema README**

Create `assets/abilities/README.md` documenting every field: `id`, `name`, `description`, `target` (all five variants with what each hits), `effect` (all four variants with their fields), `cooldown`, `fatigue_cost`. Match the depth and tone of `assets/items/README.md`. State that unknown ids referenced by a species are dropped with a warning, that a malformed file is skipped rather than fatal, and that `priority_boost` must exist because it is the fallback.

- [ ] **Step 8: Verify and commit**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`
Expected: PASS, 437 tests (433 baseline + 4 new).

```bash
git add crates/engine/src/abilities.rs crates/engine/src/lib.rs \
        crates/engine/src/game/lifecycle.rs assets/abilities/
git commit -m "feat: load abilities as .ron data behind an AbilityDb"
```

---

### Task 2: Back-rank enemies become damageable and killable

Independent of abilities — a generalization of the front-only group reaper. Do it before the new targeting shapes, which depend on it.

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs:361-402` (`pop_group_member`, `finish_group_member`)
- Modify: `crates/engine/src/game/combat_status.rs:338-352` (`reap_dead_fronts`)
- Test: `crates/engine/src/tests/combat_abilities.rs` (create)
- Modify: `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  - `Game::remove_member(&mut self, group: usize, index: usize) -> bool` — removes `groups[group].members[index]`, dropping the group if that emptied it; returns whether all groups are now gone.
  - `Game::finish_member(&mut self, group: usize, index: usize, player: Entity) -> bool` — logs the kill, awards XP and loot, despawns, queues nest respawn, removes; returns whether the battle ended.
  - `Game::finish_group_member(&mut self, group: usize, player: Entity) -> bool` — retained, now `finish_member(group, 0, player)`.
  - `Game::reap_dead_members(&mut self, player: Entity) -> bool` — replaces `reap_dead_fronts`.

- [ ] **Step 1: Write the failing test**

Create `crates/engine/src/tests/combat_abilities.rs`:

```rust
//! Data-driven abilities: multi-target shapes, cooldowns, and the
//! back-rank kill handling the enemy-side shapes depend on.

use crate::components::*;
use crate::resources::*;
use crate::*;

use super::support::*;

/// Spawns `count` hostile members of one species into a single group and
/// starts a battle against them, so back-rank indices actually exist.
/// Stats are set by hand rather than rolled, because these tests assert on
/// exact HP.
fn battle_with_a_pack_of(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let members: Vec<Entity> = (0..count)
        .map(|i| {
            game.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Hostile,
                    Position {
                        x: 5 + i as i32,
                        y: 5,
                    },
                    Stats {
                        hp,
                        max_hp: hp,
                        atk: 0,
                        def: 0,
                    },
                    StatusEffects::default(),
                ))
                .id()
        })
        .collect();
    insert_battle(game, player, members.clone());
    members
}

#[test]
fn a_back_rank_member_killed_outright_leaves_the_group_and_awards_its_xp() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pack = battle_with_a_pack_of(&mut game, 3, 20);
    let back = pack[2];

    let xp_before = game.world.get::<Experience>(player).unwrap().xp;
    game.apply_damage(back, 20);
    assert!(!game.creature_alive(back), "the back member should be down");

    let ended = game.reap_dead_members(player);
    assert!(!ended, "two members are still standing");

    let members = &game.world.resource::<BattleState>().groups[0].members;
    assert_eq!(members.len(), 2, "the dead back member must leave the group");
    assert!(
        !members.contains(&back),
        "a corpse must not stay in the group where it can be promoted to front"
    );
    assert!(
        game.world.get::<Experience>(player).unwrap().xp > xp_before,
        "a back-rank kill awards XP exactly as a front kill does"
    );
}

#[test]
fn killing_every_member_of_the_only_group_ends_the_battle() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pack = battle_with_a_pack_of(&mut game, 3, 20);

    for member in &pack {
        game.apply_damage(*member, 20);
    }
    let ended = game.reap_dead_members(player);

    assert!(ended, "clearing every group ends the encounter");
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "a won battle removes BattleState"
    );
}

#[test]
fn reaping_walks_every_index_so_two_deaths_in_one_group_both_resolve() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pack = battle_with_a_pack_of(&mut game, 4, 20);

    // Front and a middle member, so removal has to survive the indices
    // shifting underneath it.
    game.apply_damage(pack[0], 20);
    game.apply_damage(pack[2], 20);
    game.reap_dead_members(player);

    let members = &game.world.resource::<BattleState>().groups[0].members;
    assert_eq!(members.len(), 2, "both corpses must be cleared in one pass");
    assert_eq!(
        members,
        &vec![pack[1], pack[3]],
        "the survivors keep their relative order"
    );
}
```

Register it in `crates/engine/src/tests/mod.rs`, alphabetically after `mod combat;`:

```rust
mod combat_abilities;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine combat_abilities`
Expected: FAIL — `reap_dead_members` not found.

- [ ] **Step 3: Generalize the group removal helpers**

In `crates/engine/src/game/combat_round.rs`, replace `pop_group_member` with:

```rust
    /// Drops `group`'s member at `index` (the caller is responsible for
    /// whatever happened to it — a kill or a successful tame), removing the
    /// group entirely if that emptied it. Returns whether the whole pack is
    /// gone.
    pub(crate) fn remove_member(&mut self, group: usize, index: usize) -> bool {
        let mut battle = self.world.resource_mut::<BattleState>();
        let Some(g) = battle.groups.get_mut(group) else {
            return battle.groups.is_empty();
        };
        if index < g.members.len() {
            g.members.remove(index);
        }
        if g.members.is_empty() {
            battle.groups.remove(group);
        }
        battle.groups.is_empty()
    }
```

Replace `finish_group_member` with a generalized `finish_member` plus a thin front-only wrapper:

```rust
    /// Handles `group`'s member at `index` dying (from a direct hit, an AoE,
    /// or a status tick): logs the kill, awards its loot/XP, despawns it, and
    /// drops it from the group. If that emptied the last standing group, the
    /// whole encounter ends in a win (`BattleState` removed) and this returns
    /// `true`; otherwise the fight continues, returning `false`.
    pub(crate) fn finish_member(&mut self, group: usize, index: usize, player: Entity) -> bool {
        let Some(&victim) = self
            .world
            .get_resource::<BattleState>()
            .and_then(|b| b.groups.get(group))
            .and_then(|g| g.members.get(index))
        else {
            return self.living_group_count() == 0;
        };
        self.log("The rogue program crashes and deletes itself!");
        let wild_max_hp = self.world.get::<Stats>(victim).unwrap().max_hp;
        self.award_player_xp(player, wild_max_hp as u32);
        self.award_loot(victim);
        let nest = self.world.get::<NestGuardian>(victim).map(|g| g.nest);
        self.world.despawn(victim);
        if let Some(nest) = nest
            && let Some(mut n) = self.world.get_mut::<Nest>(nest)
        {
            n.pending_respawns.push(NEST_RESPAWN_TICKS);
        }
        if self.remove_member(group, index) {
            self.end_battle(player, Some(victim));
            true
        } else {
            // Only a front kill promotes someone into the line of fire; a
            // back-rank death changes nothing the player can see.
            if index == 0 {
                self.log("Another rogue program from the pack engages!");
            }
            false
        }
    }

    pub(crate) fn finish_group_member(&mut self, group: usize, player: Entity) -> bool {
        self.finish_member(group, 0, player)
    }
```

- [ ] **Step 4: Generalize the reaper**

In `crates/engine/src/game/combat_status.rs`, replace `reap_dead_fronts` with:

```rust
    /// Clears out every member that died this round — to a status tick or an
    /// area effect — awarding loot and XP exactly as a direct kill would.
    /// Walks groups and indices back to front so a removal can't shift a
    /// later one out from under the loop. Returns whether that ended the
    /// battle.
    pub(crate) fn reap_dead_members(&mut self, player: Entity) -> bool {
        let mut group = self.living_group_count();
        while group > 0 {
            group -= 1;
            let mut index = self
                .world
                .get_resource::<BattleState>()
                .and_then(|b| b.groups.get(group))
                .map(|g| g.members.len())
                .unwrap_or(0);
            while index > 0 {
                index -= 1;
                let alive = self
                    .world
                    .get_resource::<BattleState>()
                    .and_then(|b| b.groups.get(group))
                    .and_then(|g| g.members.get(index))
                    .is_some_and(|&e| self.creature_alive(e));
                if alive {
                    continue;
                }
                if self.finish_member(group, index, player) {
                    return true;
                }
            }
        }
        false
    }
```

Update its call site in `tick_round_status_effects` from `self.reap_dead_fronts(player)` to `self.reap_dead_members(player)`. Grep for any other caller: `rg 'reap_dead_fronts|pop_group_member' crates/` must come back empty.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine combat_abilities`
Expected: PASS — 3 tests.

- [ ] **Step 6: Verify the whole suite and commit**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`
Expected: PASS, 440 tests.

```bash
git add crates/engine/src/game/combat_round.rs crates/engine/src/game/combat_status.rs \
        crates/engine/src/tests/combat_abilities.rs crates/engine/src/tests/mod.rs
git commit -m "feat: enemies can now die from any rank, not just the front"
```

---

### Task 3: Species reference abilities by id; delete `SpecialAbility`

Behavior-preserving swap. The suite must end green with the same abilities doing the same things — only the plumbing changes.

**Files:**
- Modify: `crates/engine/src/species.rs:13-102` (delete `SpecialAbility`, move `SpecialTargeting`), `:138-260` (`SpeciesDef`, `load_dir`)
- Modify: `crates/engine/src/battle.rs` (host `SpecialTargeting`)
- Modify: `crates/engine/src/game/combat.rs:305-368` (`companion_abilities`, `battle_special_options`)
- Modify: `crates/engine/src/game/combat_round.rs:112-143, 415-476` (`resolve_one_action`, `use_special_ability` → `use_ability`)
- Modify: `crates/engine/src/lib.rs` (imports)
- Modify: `crates/engine/src/tests/support.rs:106-140` (`modded_assets_dir`), `:501-533` (`TWO_ABILITY_SPECIES`)
- Modify: `crates/engine/src/tests/combat_specials.rs`

**Interfaces:**
- Consumes: `abilities::{AbilityDb, AbilityDef, AbilityId, AbilityTarget, FALLBACK_ABILITY_ID}` from Task 1.
- Produces:
  - `species::SpeciesAbility { id: AbilityId, level: u32 }`
  - `SpeciesDef.abilities: Vec<SpeciesAbility>` (replaces `special_abilities`)
  - `SpeciesDb::load_dir(dir: &Path, abilities: &AbilityDb) -> std::io::Result<(Self, Vec<String>)>`
  - `battle::SpecialTargeting` — `Ally | Enemy | None` (moved from `species`)
  - `Game::companion_abilities(&self, entity: Entity) -> Vec<AbilityDef>`
  - `Game::use_ability(&mut self, ability: &AbilityDef, actor: Entity, name: &str, recipients: &[Entity])`
  - `Game::ability_recipients(&self, target: AbilityTarget, chosen: &battle::SpecialTarget) -> Vec<Entity>`

- [ ] **Step 1: Migrate the test fixtures first**

These are the callers that tell you whether the swap is behavior-preserving, so change them before the implementation.

(`modded_assets_dir`'s copy loop already gained `"abilities"` in Task 1 — it had to, since `load_asset_dbs` reads that directory from Task 1 onward and every modded-install test fails without it. No signature change is needed: no test writes a custom ability file, so there is no `extra_abilities` parameter.)

Replace `TWO_ABILITY_SPECIES` with the id-referencing form. It keeps naming a heal and a shield so `combat_specials.rs`'s expectations still hold, and adds a level gate so Task 3's filtering is exercised:

```rust
/// A species declaring two abilities, so the multi-ability paths can be
/// exercised without waiting on shipped content to grow any. The second is
/// level-gated above a fresh companion's level 1, which is what pins down
/// `Game::companion_abilities`' filtering.
pub(super) const TWO_ABILITY_SPECIES: &str = r#"(
    id: "test_medic",
    name: "Test Medic",
    glyph: 'm',
    color: Cyan,
    base_hp: 10,
    base_atk: 4,
    base_def: 2,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    base_speed: 10,
    moves: [(name: "Poke", power: 3)],
    abilities: [
        (id: "hot_patch"),
        (id: "sandbox", level: 5),
    ],
)"#;
```

- [ ] **Step 2: Run the suite to verify it fails**

Run: `cargo test -p feral-processes-engine`
Expected: FAIL — `abilities` is an unknown field on `SpeciesDef`; serde ignores it, so the medic loads with no abilities and the `combat_specials` tests break.

- [ ] **Step 3: Move `SpecialTargeting` and extend it**

Delete `SpecialTargeting` from `crates/engine/src/species.rs` and add it to `crates/engine/src/battle.rs`, next to `SpecialTarget`:

```rust
/// Which picker the UI opens after an ability is chosen — see
/// `abilities::AbilityTarget::targeting`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialTargeting {
    /// Lands on a party member: the player or any companion.
    Ally,
    /// Lands on an enemy group.
    Enemy,
    /// Needs no choice at all — it resolves the moment it is picked.
    None,
}
```

Delete the entire `SpecialAbility` enum and its `impl` block (`targeting`, `display_label`, `short_name`) from `species.rs`.

- [ ] **Step 4: Rework `SpeciesDef`**

In `crates/engine/src/species.rs`, add:

```rust
/// One ability a species grants a tamed member, and the level it unlocks
/// at.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeciesAbility {
    pub id: crate::abilities::AbilityId,
    /// Companion level at which this becomes usable. `#[serde(default)]` to
    /// 1 — available as soon as the program is tamed.
    #[serde(default = "default_learn_level")]
    pub level: u32,
}

fn default_learn_level() -> u32 {
    1
}
```

Replace the `special_abilities` field and delete `legacy_special_ability` entirely:

```rust
    /// The abilities a tamed member of this species can be commanded to use,
    /// in menu order, each gated on the companion's level. Left empty, the
    /// companion falls back to `abilities::FALLBACK_ABILITY_ID` — see
    /// `Game::companion_abilities`, which resolves that fallback so no caller
    /// has to special-case an empty list. `#[serde(default)]` so existing
    /// species files (including mods) without this field keep parsing.
    #[serde(default)]
    pub abilities: Vec<SpeciesAbility>,
```

Change `SpeciesDb::load_dir` to take the ability database and validate ids, mirroring how `ResearchDb::load_dir` validates against `StructureDb`:

```rust
    pub fn load_dir(dir: &Path, abilities: &crate::abilities::AbilityDb) -> std::io::Result<(Self, Vec<String>)> {
```

Inside the per-file `Ok(def)` arm, after parsing and before inserting, drop unknown ability ids with a warning rather than dropping the whole species — a species is still playable without one ability:

```rust
                    def.abilities.retain(|a| {
                        let known = abilities.get(&a.id).is_some();
                        if !known {
                            warnings.push(format!(
                                "species {:?}: unknown ability {:?} — dropped",
                                def.id, a.id
                            ));
                        }
                        known
                    });
```

Delete the two legacy-migration tests in `species.rs`'s test module that exercise `special_ability` (singular) folding, and fix the remaining tests in that module to construct `SpeciesDb::load_dir` with an `AbilityDb` loaded from the shipped `assets/abilities` dir.

- [ ] **Step 5: Rework the resolver**

In `crates/engine/src/game/combat.rs`, replace `companion_abilities`:

```rust
    /// Every ability `entity` can be commanded to use right now, in menu
    /// order — its species' declared list, filtered to what its level has
    /// unlocked.
    ///
    /// Never empty: a species that declares none, or whose whole list is
    /// still level-gated, yields `FALLBACK_ABILITY_ID`. Resolving the
    /// fallback here rather than at each call site is what lets
    /// `BattleAction::Special` carry a plain index and the menu list one row
    /// instead of zero.
    pub(crate) fn companion_abilities(&self, entity: Entity) -> Vec<AbilityDef> {
        let db = self.world.resource::<AbilityDb>();
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        let declared: Vec<AbilityDef> = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.abilities.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.level <= level)
            .filter_map(|a| db.get(&a.id).cloned())
            .collect();
        if !declared.is_empty() {
            return declared;
        }
        db.get(abilities::FALLBACK_ABILITY_ID)
            .cloned()
            .into_iter()
            .collect()
    }
```

Update `battle_special_options` to read names off the def (add the `unavailable` field in Task 5, not here):

```rust
            .map(|(index, ability)| SpecialOption {
                index,
                name: ability.name.clone(),
                detail: ability.description.clone(),
                targeting: ability.target.targeting(),
            })
```

In `crates/engine/src/game/combat_round.rs`, replace `use_special_ability` with `use_ability`, taking a slice so single- and multi-target land on one path. It also takes the acting entity, which the `Damage` arm needs for `effective_atk` and which `name` cannot supply:

```rust
    /// Executes `ability` (one of `Game::companion_abilities`) on every
    /// entity in `recipients` — party members for a buff or heal, enemies
    /// for damage or a debuff. See `Game::ability_recipients`, which
    /// resolves which entities those are.
    pub(crate) fn use_ability(
        &mut self,
        ability: &AbilityDef,
        actor: Entity,
        name: &str,
        recipients: &[Entity],
    ) {
        for &recipient in recipients {
            let on = self.target_label(recipient);
            match &ability.effect {
                AbilityEffect::Buff {
                    kind,
                    power,
                    duration,
                } => {
                    self.arm_buff(
                        recipient,
                        ActiveBuff {
                            kind: *kind,
                            remaining: *duration,
                            power: *power,
                        },
                    );
                    let stat = match kind {
                        BuffKind::Atk => "attack",
                        BuffKind::Def => "defense",
                    };
                    self.log(format!("{name} runs {} on {on}, boosting {stat}!", ability.name));
                }
                AbilityEffect::Heal { power } => {
                    if let Some(mut stats) = self.world.get_mut::<Stats>(recipient) {
                        stats.hp = (stats.hp + power).min(stats.max_hp);
                    }
                    self.log(format!("{name} patches {on} for {power} HP."));
                }
                AbilityEffect::Debuff {
                    kind,
                    power,
                    duration,
                } => {
                    if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(recipient) {
                        statuses.active = Some(ActiveStatus {
                            kind: *kind,
                            remaining: *duration,
                            power: *power,
                        });
                    }
                    match kind {
                        StatusKind::Bleed => self.log(format!("{name} corrupts {on}'s data!")),
                        StatusKind::Stun => self.log(format!("{name} locks up {on}!")),
                    }
                }
                AbilityEffect::Damage { power, status } => {
                    let def = self.world.get::<Stats>(recipient).map(|s| s.def).unwrap_or(0);
                    let dmg = battle::compute_damage(self.effective_atk(actor), def, *power);
                    self.apply_damage(recipient, dmg);
                    self.log(format!("{name} hits {on} for {dmg} damage."));
                    if let Some(effect) = status {
                        let label = self.target_label(recipient);
                        self.apply_status_effect(recipient, effect, &label);
                    }
                }
            }
        }
    }
```

In `resolve_one_action`, replace the `BattleAction::Special` arm's body so it resolves recipients through a new helper and calls `use_ability`. Add the helper to `combat_round.rs`:

```rust
    /// Which entities an ability actually lands on this round, resolved at
    /// resolve time rather than plan time — so a group that died before the
    /// acting member's turn retargets, and an ally knocked out in the
    /// meantime is skipped instead of being healed as a corpse.
    pub(crate) fn ability_recipients(
        &self,
        target: AbilityTarget,
        chosen: &battle::SpecialTarget,
    ) -> Vec<Entity> {
        match target {
            AbilityTarget::OneAlly => match chosen {
                battle::SpecialTarget::Ally { slot } => self
                    .actor_entity(battle::Actor::Party(*slot))
                    .filter(|&e| self.creature_alive(e))
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            AbilityTarget::WholeParty => (0..self
                .world
                .get_resource::<BattleState>()
                .map(|b| b.planned.len())
                .unwrap_or(0))
                .filter_map(|slot| self.actor_entity(battle::Actor::Party(slot)))
                .filter(|&e| self.creature_alive(e))
                .collect(),
            AbilityTarget::OneEnemyGroupFront => match chosen {
                battle::SpecialTarget::EnemyGroup { group } => self
                    .retarget(*group)
                    .and_then(|g| self.front_of_group(g))
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            AbilityTarget::WholeEnemyGroup => match chosen {
                battle::SpecialTarget::EnemyGroup { group } => self
                    .retarget(*group)
                    .and_then(|g| {
                        self.world
                            .get_resource::<BattleState>()
                            .and_then(|b| b.groups.get(g))
                            .map(|grp| grp.members.clone())
                    })
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|&e| self.creature_alive(e))
                    .collect(),
                _ => Vec::new(),
            },
            AbilityTarget::AllEnemies => self.all_living_enemies(),
        }
    }
```

The `Special` arm becomes:

```rust
            BattleAction::Special { ability, target } => {
                let name = self.creature_label(entity);
                let abilities = self.companion_abilities(entity);
                // Falls back to the first rather than skipping the turn: the
                // index was valid when planned, and a party edited mid-round
                // shouldn't silently cost a member its action.
                let chosen = abilities
                    .get(ability)
                    .or_else(|| abilities.first())
                    .cloned();
                if let Some(ability) = chosen {
                    let recipients = self.ability_recipients(ability.target, &target);
                    self.use_ability(&ability, entity, &name, &recipients);
                    // An area effect can drop members from any rank, and a
                    // corpse left in a group would be promoted to front and
                    // then attacked as though alive.
                    self.reap_dead_members(player);
                }
                if let Some(mut needs) = self.world.get_mut::<Needs>(player) {
                    needs.fatigue = (needs.fatigue - COMPANION_COMMAND_FATIGUE_COST).max(0.0);
                }
            }
```

Also update `action_label`'s Special arm to use `.name` instead of `.short_name()`.

- [ ] **Step 6: Fix imports and the remaining call sites**

In `crates/engine/src/lib.rs`, add to the `use abilities::{...}` line: `AbilityDef, AbilityEffect, AbilityTarget`. Remove `SpecialAbility` from the `use species::{...}` line. Add `SpecialTargeting` to the `use battle::{...}` line if the crate root references it.

Update `crates/app-core/src/lib.rs:21` from `use feral_processes_engine::species::SpecialTargeting;` to `use feral_processes_engine::battle::SpecialTargeting;`, and add a `SpecialTargeting::None => Mode::Battle` arm at line ~1256 as a placeholder — Task 4 replaces it with the real immediate-resolve behavior.

Rewrite `crates/engine/src/tests/combat_specials.rs`'s six `SpecialAbility::` construction sites to look the ability up from `AbilityDb` instead, e.g.:

```rust
    let heal = game
        .world
        .resource::<AbilityDb>()
        .get("hot_patch")
        .cloned()
        .expect("hot_patch ships");
    game.use_ability(&heal, companion, "TestBot", &[player]);
```

The test at line 177 (`companion_ability_label_shows_special_ability_or_a_computed_attack_rally`) and line 183's `.find(|s| s.special_abilities.is_empty())` become `.find(|s| s.abilities.is_empty())`. Line 246's `buffs_and_heals_aim_at_the_party_while_debuffs_aim_at_the_enemy` now asserts on `AbilityTarget::targeting()` rather than `SpecialAbility::targeting()`.

`a_species_with_several_abilities_offers_each_one_in_menu_order` (line 214) needs care: the migrated `TWO_ABILITY_SPECIES` gates its second ability at level 5, so a freshly spawned level-1 medic now offers **one** row, not two. Level the companion up before asserting, so the test keeps covering menu order rather than silently becoming a one-row test:

```rust
    let (mut game, medic) = game_with_two_ability_companion();
    game.world.get_mut::<Experience>(medic).unwrap().level = 5;
    let options = game.battle_special_options(1);
    assert_eq!(options.len(), 2, "both abilities are unlocked at level 5");
    assert_eq!(options[0].name, "Hot Patch");
    assert_eq!(options[1].name, "Sandbox");
```

Then add the level-gating test the spec calls for — put it in `combat_specials.rs` beside the one above, since it is about the same fixture:

```rust
#[test]
fn an_ability_above_the_companions_level_is_not_offered_yet() {
    let (game, medic) = game_with_two_ability_companion();
    assert_eq!(
        game.world.get::<Experience>(medic).unwrap().level,
        1,
        "a freshly tamed program starts at level 1"
    );

    let options = game.battle_special_options(1);
    assert_eq!(
        options.len(),
        1,
        "the level-5 ability must stay hidden until it is earned"
    );
    assert_eq!(options[0].name, "Hot Patch");
}
```

- [ ] **Step 7: Run the suite to verify it passes**

Run: `cargo test --workspace`
Expected: PASS. Net −1 from the baseline of 440: two legacy-migration tests deleted, one level-gating test added — **439**.

- [ ] **Step 8: Verify and commit**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

```bash
git add -A
git commit -m "refactor: species reference abilities by id instead of an inline enum"
```

---

### Task 4: The new targeting shapes reach the UI

The engine can already resolve all five shapes after Task 3. This task makes the two no-picker shapes reachable from the action menu.

**Files:**
- Modify: `crates/engine/src/battle.rs` (`SpecialTarget` variants)
- Modify: `crates/engine/src/game/combat.rs:231-271` (`battle_set_action` validation)
- Modify: `crates/app-core/src/lib.rs:1077-1090, 1180-1200, 1250-1262`
- Test: `crates/engine/src/tests/combat_abilities.rs`

**Interfaces:**
- Consumes: `AbilityTarget`, `Game::ability_recipients`, `Game::use_ability` from Task 3.
- Produces:
  - `battle::SpecialTarget::{WholeParty, AllEnemies}`
  - `battle::SpecialOption.sweeps_party: bool`

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/combat_abilities.rs`:

```rust
/// A game whose species ships the three new multi-target abilities, so the
/// shapes can be exercised without depending on shipped kit assignments.
fn game_with_a_sweeper() -> (Game, Entity) {
    const SWEEPER: &str = r#"(
        id: "test_sweeper",
        name: "Test Sweeper",
        glyph: 's',
        color: Red,
        base_hp: 30,
        base_atk: 10,
        base_def: 2,
        taming_difficulty: 0.5,
        habitats: [OpenGrid],
        base_speed: 10,
        moves: [(name: "Poke", power: 3)],
        abilities: [
            (id: "cascade_overflow"),
            (id: "broadcast_storm"),
            (id: "redundancy_sync"),
        ],
    )"#;
    let dir = modded_assets_dir("sweeper", &[], &[], &[("test_sweeper.ron", SWEEPER)]);
    let mut game = Game::new(31, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let sweeper = game
        .world
        .spawn((
            Creature {
                species: "test_sweeper".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 30,
                max_hp: 30,
                atk: 10,
                def: 2,
            },
            Tamed { owner: player },
            Experience::default(),
        ))
        .id();
    game.add_companion(sweeper).unwrap();
    (game, sweeper)
}

#[test]
fn a_whole_group_ability_damages_every_member_not_just_the_front() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let pack = battle_with_a_pack_of(&mut game, 3, 50);

    companion_uses_special(
        &mut game,
        sweeper,
        0, // cascade_overflow
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );

    for (i, member) in pack.iter().enumerate() {
        let hp = game.world.get::<Stats>(*member).unwrap().hp;
        assert!(
            hp < 50,
            "member {i} at rank {i} should have taken damage, still at {hp}"
        );
    }
}

#[test]
fn an_all_enemies_ability_reaches_every_group_including_past_engagement_range() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    // Four distinct species so `group_pack` yields four groups — more than
    // ENGAGED_GROUPS, which is the point.
    let species: Vec<String> = game
        .species_defs()
        .into_iter()
        .take(4)
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(species.len(), 4, "the shipped set must supply four species");
    let enemies: Vec<Entity> = species
        .iter()
        .enumerate()
        .map(|(i, id)| {
            game.world
                .spawn((
                    Creature { species: id.clone() },
                    Hostile,
                    Position { x: 5 + i as i32, y: 5 },
                    Stats { hp: 50, max_hp: 50, atk: 0, def: 0 },
                    StatusEffects::default(),
                ))
                .id()
        })
        .collect();
    insert_battle(&mut game, player, enemies.clone());
    assert_eq!(game.living_group_count(), 4, "four species, four groups");

    companion_uses_special(
        &mut game,
        sweeper,
        1, // broadcast_storm
        battle::SpecialTarget::AllEnemies,
    );

    for (i, enemy) in enemies.iter().enumerate() {
        let hp = game.world.get::<Stats>(*enemy).unwrap().hp;
        assert!(hp < 50, "group {i} should have been hit, still at {hp}");
    }
}

#[test]
fn a_whole_party_heal_raises_every_living_member_and_skips_the_downed() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    let downed = spawn_tamed(&mut game, 20, 5);
    game.add_companion(downed).unwrap();
    battle_with_a_pack_of(&mut game, 1, 200);

    for (entity, hp) in [(player, 10), (sweeper, 10), (downed, 0)] {
        game.world.get_mut::<Stats>(entity).unwrap().hp = hp;
    }

    companion_uses_special(&mut game, sweeper, 2, battle::SpecialTarget::WholeParty);

    assert!(
        game.world.get::<Stats>(player).unwrap().hp > 10,
        "the player is part of the party"
    );
    assert!(
        game.world.get::<Stats>(sweeper).unwrap().hp > 10,
        "the caster heals itself too"
    );
    assert_eq!(
        game.world.get::<Stats>(downed).unwrap().hp,
        0,
        "a heal spent on a downed member would be wasted"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p feral-processes-engine combat_abilities`
Expected: FAIL — `SpecialTarget::WholeParty` and `::AllEnemies` do not exist.

- [ ] **Step 3: Add the variants**

In `crates/engine/src/battle.rs`, extend `SpecialTarget`:

```rust
pub enum SpecialTarget {
    /// A party slot, indexed as `battle::Actor::Party` — slot 0 is the
    /// player.
    Ally { slot: usize },
    EnemyGroup { group: usize },
    /// Every living party member. Carries no index because the player makes
    /// no choice — see `SpecialTargeting::None`.
    WholeParty,
    /// Every living enemy in every group. Same no-choice rationale.
    AllEnemies,
}
```

In `crates/engine/src/game/combat.rs`, `battle_set_action`'s `target_group` match already falls through to `None` for the new variants via its `_ =>` arm — verify that, and confirm the ally-slot validation block's `if let battle::SpecialTarget::Ally { slot: ally } = target` still compiles unchanged.

- [ ] **Step 4: Run to verify the engine tests pass**

Run: `cargo test -p feral-processes-engine combat_abilities`
Expected: PASS — 6 tests in this file.

- [ ] **Step 5: Wire the no-picker shapes through app-core**

`SpecialTargeting::None` alone can't say whether the ability sweeps the party or the enemies, and app-core must not pattern-match on ability content. Carry that fact on the engine's menu row instead.

First, in `crates/engine/src/battle.rs`, extend `SpecialOption`:

```rust
    /// For a `SpecialTargeting::None` ability, which side it sweeps —
    /// carried here so neither renderer has to know what any ability does.
    /// Meaningless (and always `false`) for abilities that open a picker.
    pub sweeps_party: bool,
```

Set it in `battle_special_options` in `crates/engine/src/game/combat.rs`:

```rust
                sweeps_party: ability.target == AbilityTarget::WholeParty,
```

Then in `crates/app-core/src/lib.rs`, replace the placeholder `SpecialTargeting::None` arm added in Task 3 (around line 1256) so a no-choice ability commits immediately rather than opening a picker:

```rust
        self.pending_special_ability = Some(chosen.index);
        self.menu_selected = 0;
        match chosen.targeting {
            SpecialTargeting::Ally => self.mode = Mode::BattleAlly,
            SpecialTargeting::Enemy => self.mode = Mode::BattleTarget,
            // Nothing left to choose — commit the action now rather than
            // opening a picker with one meaningless row.
            SpecialTargeting::None => {
                let target = if chosen.sweeps_party {
                    SpecialTarget::WholeParty
                } else {
                    SpecialTarget::AllEnemies
                };
                let action = BattleAction::Special {
                    ability: chosen.index,
                    target,
                };
                self.pending_battle_action = None;
                self.pending_special_ability = None;
                self.commit_battle_action(slot, action);
            }
        }
```

- [ ] **Step 6: Verify and commit**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`
Expected: PASS, 442 tests.

```bash
git add -A
git commit -m "feat: whole-group, whole-field and whole-party ability targeting"
```

---

### Task 5: Cooldowns and per-ability Fatigue costs

**Files:**
- Modify: `crates/engine/src/components.rs` (`AbilityCooldowns`)
- Modify: `crates/engine/src/game/combat.rs` (`battle_special_options`, `battle_set_action`)
- Modify: `crates/engine/src/game/combat_round.rs` (arm the cooldown, charge `fatigue_cost`)
- Modify: `crates/engine/src/game/combat_status.rs` (tick and clear)
- Modify: `crates/engine/src/battle.rs` (`SpecialOption.unavailable`)
- Test: `crates/engine/src/tests/combat_abilities.rs`

**Interfaces:**
- Consumes: everything from Tasks 1, 3 and 4.
- Produces: `components::AbilityCooldowns(pub HashMap<AbilityId, u32>)`, `battle::SpecialOption.unavailable: Option<String>`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/combat_abilities.rs`:

```rust
#[test]
fn an_ability_on_cooldown_is_offered_but_refused() {
    let (mut game, sweeper) = game_with_a_sweeper();
    battle_with_a_pack_of(&mut game, 2, 200);
    let slot = 1;

    companion_uses_special(
        &mut game,
        sweeper,
        0, // cascade_overflow, cooldown 2
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );

    let options = game.battle_special_options(slot);
    assert!(
        options[0].unavailable.is_some(),
        "an ability just spent must render greyed, not silently fail"
    );
    assert!(
        game.battle_set_action(
            slot,
            BattleAction::Special {
                ability: 0,
                target: battle::SpecialTarget::EnemyGroup { group: 0 },
            }
        )
        .is_err(),
        "planning a cooling ability must be refused, not burn the round"
    );
}

#[test]
fn a_cooldown_expires_after_its_declared_rounds() {
    let (mut game, sweeper) = game_with_a_sweeper();
    battle_with_a_pack_of(&mut game, 2, 500);

    companion_uses_special(
        &mut game,
        sweeper,
        0, // cooldown 2
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    assert!(game.battle_special_options(1)[0].unavailable.is_some());

    for _ in 0..2 {
        resolve_round_with(&mut game, BattleAction::Defend);
    }

    assert!(
        game.battle_special_options(1)[0].unavailable.is_none(),
        "a 2-round cooldown must be clear two rounds later"
    );
}

#[test]
fn cooldowns_do_not_survive_the_battle_that_set_them() {
    let (mut game, sweeper) = game_with_a_sweeper();
    battle_with_a_pack_of(&mut game, 1, 1);

    companion_uses_special(
        &mut game,
        sweeper,
        0,
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );

    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "the one 1-HP enemy should have died, ending the fight"
    );
    let cooldowns = game.world.get::<AbilityCooldowns>(sweeper);
    assert!(
        cooldowns.is_none_or(|c| c.0.values().all(|&r| r == 0)),
        "cooldowns are scoped to one intrusion, like every other combat status"
    );
}

#[test]
fn a_costly_ability_charges_its_own_fatigue_not_the_flat_command_cost() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);

    let before = game.world.get::<Needs>(player).unwrap().fatigue;
    companion_uses_special(
        &mut game,
        sweeper,
        0, // cascade_overflow declares fatigue_cost 8.0
        battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    let spent = before - game.world.get::<Needs>(player).unwrap().fatigue;

    assert!(
        (spent - 8.0).abs() < f32::EPSILON,
        "expected the ability's own 8.0 cost, spent {spent}"
    );
}

#[test]
fn an_ability_costing_more_fatigue_than_you_have_is_unavailable() {
    let (mut game, sweeper) = game_with_a_sweeper();
    let player = game.player_entity();
    battle_with_a_pack_of(&mut game, 2, 500);
    game.world.get_mut::<Needs>(player).unwrap().fatigue = 1.0;

    let options = game.battle_special_options(1);
    assert!(
        options[1].unavailable.is_some(),
        "broadcast_storm costs 15.0 Fatigue and must be refused at 1.0"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p feral-processes-engine combat_abilities`
Expected: FAIL — `unavailable` is not a field on `SpecialOption`; `AbilityCooldowns` not found.

- [ ] **Step 3: Add the component**

In `crates/engine/src/components.rs`, next to `CombatBuff`:

```rust
/// Rounds remaining before each ability this combatant has spent can be
/// used again. Battle-scoped exactly like `CombatBuff` and `StatusEffects`
/// — armed during a fight, ticked at end of round, cleared when the
/// intrusion ends — so nothing here is ever persisted.
#[derive(Component, Default)]
pub struct AbilityCooldowns(pub std::collections::HashMap<crate::abilities::AbilityId, u32>);
```

- [ ] **Step 4: Add the field, gate the menu, charge the cost**

In `crates/engine/src/battle.rs`, add to `SpecialOption`:

```rust
    /// `Some(reason)` means render it greyed with the reason shown — same
    /// contract as `ActionOption::unavailable`.
    pub unavailable: Option<String>,
```

In `crates/engine/src/game/combat.rs`, add a helper and use it from `battle_special_options`:

```rust
    /// Why `entity` can't spend `ability` right now, or `None` if it can.
    pub(crate) fn ability_unavailable(&self, entity: Entity, ability: &AbilityDef) -> Option<String> {
        let remaining = self
            .world
            .get::<AbilityCooldowns>(entity)
            .and_then(|c| c.0.get(&ability.id).copied())
            .unwrap_or(0);
        if remaining > 0 {
            return Some(format!("{remaining} more rounds"));
        }
        let fatigue = self
            .world
            .get::<Needs>(self.player_entity())
            .map(|n| n.fatigue)
            .unwrap_or(0.0);
        if fatigue < ability.fatigue_cost {
            return Some("not enough Fatigue".to_string());
        }
        None
    }
```

```rust
                unavailable: self.ability_unavailable(entity, &ability),
```

In `battle_set_action`, extend the existing `BattleAction::Special` validation block:

```rust
            let options = self.battle_special_options(slot);
            if *ability >= options.len() {
                return Err("That party member has no such ability.".to_string());
            }
            if let Some(reason) = &options[*ability].unavailable {
                return Err(format!("That ability isn't ready: {reason}."));
            }
```

In `crates/engine/src/game/combat_round.rs`'s `Special` arm, replace the flat fatigue deduction and arm the cooldown:

```rust
                if let Some(ability) = chosen {
                    let recipients = self.ability_recipients(ability.target, &target);
                    self.use_ability(&ability, entity, &name, &recipients);
                    self.reap_dead_members(player);
                    if ability.cooldown > 0 {
                        let mut cooldowns = self
                            .world
                            .get_mut::<AbilityCooldowns>(entity)
                            .map(|c| c.0.clone())
                            .unwrap_or_default();
                        // +1 so the tick at the end of this same round
                        // doesn't eat a round the player never got.
                        cooldowns.insert(ability.id.clone(), ability.cooldown + 1);
                        self.world
                            .entity_mut(entity)
                            .insert(AbilityCooldowns(cooldowns));
                    }
                    if let Some(mut needs) = self.world.get_mut::<Needs>(player) {
                        needs.fatigue = (needs.fatigue - ability.fatigue_cost).max(0.0);
                    }
                }
```

(Delete the old unconditional `COMPANION_COMMAND_FATIGUE_COST` deduction that followed the `if let`.)

- [ ] **Step 5: Tick and clear**

In `crates/engine/src/game/combat_status.rs`, add:

```rust
    /// Counts one round off every cooldown `entity` is carrying, dropping
    /// the entries that reach zero so the map doesn't grow across a long
    /// fight.
    pub(crate) fn tick_ability_cooldowns(&mut self, entity: Entity) {
        let Some(mut cooldowns) = self.world.get_mut::<AbilityCooldowns>(entity) else {
            return;
        };
        cooldowns.0.retain(|_, remaining| {
            *remaining = remaining.saturating_sub(1);
            *remaining > 0
        });
    }
```

Call it in `tick_round_status_effects` alongside `tick_combat_buff` — for the player and for each companion:

```rust
        self.tick_combat_buff(player);
        self.tick_ability_cooldowns(player);
```

```rust
            self.tick_combat_buff(companion);
            self.tick_ability_cooldowns(companion);
```

In `clear_battle_status_effects`, clear them for the player and each companion, alongside the existing `StatusEffects`/`CombatBuff` clears:

```rust
        if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(player) {
            c.0.clear();
        }
```

(and the equivalent inside the existing companion loop).

- [ ] **Step 6: Run to verify the tests pass**

Run: `cargo test -p feral-processes-engine combat_abilities`
Expected: PASS — 11 tests in this file.

- [ ] **Step 7: Verify and commit**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`
Expected: PASS, 447 tests.

```bash
git add -A
git commit -m "feat: ability cooldowns and per-ability Fatigue costs"
```

---

### Task 6: Assign species kits and sweep the docs

**Files:**
- Modify: `assets/species/*.ron` (kit assignment)
- Modify: `assets/species/README.md`
- Modify: `README.md`, `CHANGELOG.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: everything above. Produces no new API.

- [ ] **Step 1: Write the failing assertion**

In `crates/engine/src/tests/assets.rs`, add:

```rust
#[test]
fn the_shipped_species_kits_reference_only_real_abilities() {
    let game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let db = game.world.resource::<AbilityDb>();
    let mut declared = 0;
    for species in game.species_defs() {
        for ability in &species.abilities {
            assert!(
                db.get(&ability.id).is_some(),
                "species {:?} names unknown ability {:?}",
                species.id,
                ability.id
            );
            assert!(
                ability.level >= 1 && ability.level <= crate::progression::CREATURE_MAX_LEVEL,
                "species {:?}: ability {:?} unlocks at level {}, outside 1..={}",
                species.id,
                ability.id,
                ability.level,
                crate::progression::CREATURE_MAX_LEVEL
            );
            declared += 1;
        }
    }
    assert!(
        declared >= 10,
        "the shipped roster should actually use the ability system, found {declared}"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p feral-processes-engine the_shipped_species_kits`
Expected: FAIL — `declared` is 0; no species declares abilities yet.

- [ ] **Step 3: Assign the kits**

Add an `abilities:` field to these species `.ron` files. Levels must stay within `1..=12`.

- `sentinel.ron` — the tank: `[(id: "sandbox"), (id: "redundancy_sync", level: 6)]`
- `cipher.ron` — the debuffer: `[(id: "memory_leak"), (id: "null_route", level: 8)]`
- `sub_process.ron` — the medic: `[(id: "hot_patch"), (id: "redundancy_sync", level: 7)]`
- `scrapper.ron` — the bruiser: `[(id: "cascade_overflow", level: 3)]`
- `rootkit.ron` — the disabler: `[(id: "deadlock"), (id: "memory_leak", level: 4)]`
- `overseer.ron` — boss: `[(id: "broadcast_storm"), (id: "overclock_array", level: 5)]`
- `wintermute.ron` — boss: `[(id: "broadcast_storm"), (id: "null_route", level: 4)]`

Leave the remaining species without the field so the `FALLBACK_ABILITY_ID` path stays exercised by shipped content.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p feral-processes-engine the_shipped_species_kits`
Expected: PASS.

- [ ] **Step 5: Update the schema docs**

In `assets/species/README.md`, replace the `special_abilities` documentation with `abilities`: a list of `(id: "<ability id>", level: <1-12>)` entries, `level` defaulting to 1, ids validated against `assets/abilities/` at load with unknown ones dropped and warned. Cross-reference `assets/abilities/README.md`. Remove any mention of the singular `special_ability` field, which no longer exists.

- [ ] **Step 6: Sweep the root docs**

```bash
rg -n 'special|abilit|Rally|Shield' README.md CHANGELOG.md
```

Fix any claim this change falsifies. Add a CHANGELOG entry describing: abilities are now moddable `.ron` data; multi-target attacks, debuffs and party heals; enemies can be defeated from any rank; abilities have cooldowns and Fatigue costs.

- [ ] **Step 7: Final verification and commit**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`
Expected: PASS, 448 tests.

```bash
git add -A
git commit -m "feat: assign ability kits to the shipped roster, update schema docs"
```

---

## Post-Plan Notes

**Not built here, by design.** The player still has no Special action — `battle_action_options` keeps its `if !is_player` gate. Player routine slots, the companion install slot, ability modules as items, research wiring, and the slot-capacity perk are all Phase 2, and they are what give the player abilities. `use_ability` and `ability_recipients` are entity-generic, so slot 0 works the moment Phase 2 wires a kit to it.

**CLAUDE.md** should gain abilities as a fourth data-driven content type alongside species, structures and items — but it is gitignored, so that edit will not ship with this branch and must be made locally.

**Balance is unplayed.** Every number in `assets/abilities/*.ron` is arithmetic-plausible and nothing more. They are data; retune without a rebuild.
