# Party Roster Battles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the one-action-per-round duel with a Bard's Tale roster battle — enemies as addressable species groups, an action chosen for every party member per round, resolved together in initiative order.

**Architecture:** `BattleState` grows a `Vec<EnemyGroup>` and a `planned: Vec<Option<BattleAction>>`. The engine owns action planning and emits the menu as data (`Vec<ActionOption>`), so both renderers dispatch off engine output instead of hardcoded keys. Resolution rolls initiative for every living actor and walks the sorted order.

**Tech Stack:** Rust, standalone `bevy_ecs`, `ron` for assets, `bincode` for saves, `ratatui` (TUI) and `macroquad` (GUI).

**Spec:** `docs/superpowers/specs/2026-07-24-party-roster-battles-design.md`

## Global Constraints

- The engine's `Game` struct is the entire public API surface both renderers use via app-core. Neither renderer touches the ECS `World`. Keep it that way.
- New `SpeciesDef` / `MoveDef` / `StructureDef` / `ItemDef` fields **must** be `#[serde(default)]` so existing `.ron` files, including third-party mods, keep parsing untouched.
- A malformed `.ron` file is skipped with a logged warning, never a panic.
- `assets/species/README.md` is updated in the *same task* as any schema change.
- Named constants live in `crates/engine/src/balance.rs`. No magic numbers.
- No flaky tests: no `sleep()`, no wall-clock dependence, no unseeded RNG. All randomness goes through the existing seeded `GameRng`.
- Comments explain *why*, never *what*.
- `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings, don't silence them.
- **Never commit unless the user explicitly asks.** Each task below ends with a commit step — run it only under standing instruction from the user for this branch. Otherwise stop at the passing test and report.
- Full gate at task boundaries: `cargo test --workspace` (~370 tests, ~1s).

**If many tests fail at once with `NotFound` on an assets path**, that is stale build artifacts from the old `/home/trog/code/petmud` path, not real breakage. Fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core` — *not* a full `cargo clean`, `target/` is ~4 GB.

---

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `crates/engine/src/battle.rs` | Grows from a damage formula into the battle domain module: `EnemyGroup`, `BattleAction`, `ActionKind`, `ActionOption`, `TargetSpec`, initiative rolling | 1, 2, 3 |
| `crates/engine/src/resources.rs` | `BattleState` restructure, `MAX_PARTY_SIZE` | 1, 5 |
| `crates/engine/src/species.rs` | `SpeciesDef::base_speed`, `MoveDef::ranged` | 2, 4 |
| `crates/engine/src/lib.rs` | Grouping, resolution, planning API, `BattleView` | 1, 3, 4, 5, 7 |
| `crates/engine/src/balance.rs` | New constants; offline sim rewrite | 2, 4, 5, 6 |
| `crates/engine/src/save.rs` | `CreatureSave::party_slot`, version bump | 5 |
| `assets/species/*.ron` + `README.md` | `base_speed` and `ranged` data pass | 2, 4 |
| `crates/app-core/src/lib.rs` | `Mode::BattleTarget` / `BattleResolve`, data-driven key dispatch | 7 |
| `crates/tui/src/ui.rs` | Two-panel roster screen | 8 |
| `crates/gui/src/render.rs`, `fx.rs` | Two-panel roster screen, keyed ghost bars | 9 |

`crates/engine/src/lib.rs` is already 13,968 lines. Every new *type* in this plan goes in `battle.rs`, which is currently 58 lines and is the natural home for the battle domain. Only `Game` methods land in `lib.rs`.

**Scaffolding note:** Tasks 3–6 keep `battle_attack` / `battle_decompile` / `battle_command_companion` as thin wrappers so app-core and both renderers keep compiling. These are branch-internal scaffolding with an explicit deletion step in Task 7 — they must not survive the branch. This is the one deliberate exception to the no-backwards-compat-cruft rule, and it exists so every task ends green.

---

### Task 1: Enemy groups

**Files:**
- Modify: `crates/engine/src/battle.rs` (add `EnemyGroup`)
- Modify: `crates/engine/src/resources.rs:119-132` (`BattleState`)
- Modify: `crates/engine/src/lib.rs:2451` (`start_battle`), `:2482` (`front_wild_creature`), `:2635` (`pop_front_pack_member`), `:2649` (`finish_front_pack_member`), `:2619` (`all_wild_retaliate`), `:2490` (`battle_view`)
- Modify: `crates/engine/src/balance.rs` (add `MAX_ENEMY_GROUPS`)

**Interfaces:**
- Consumes: nothing (first task)
- Produces: `battle::EnemyGroup { species: SpeciesId, members: Vec<Entity> }`; `BattleState { player, groups: Vec<EnemyGroup>, round: u32, log, finished, player_won }`; `Game::front_of_group(&self, group: usize) -> Option<Entity>`; `Game::finish_group_member(&mut self, group: usize, player: Entity) -> bool` (returns whether the *whole battle* ended); `Game::living_group_count(&self) -> usize`

- [ ] **Step 1: Write the failing grouping test**

Add to the `mod tests` block in `crates/engine/src/lib.rs`:

```rust
/// A pack partitions into one group per species, in first-appearance
/// order. `gather_pack` walks an ECS query, so the deterministic order
/// has to come from the partition step itself — an incidental query
/// order is exactly the kind of thing that produced this repo's
/// unsorted-habitat-lookup flake.
#[test]
fn a_mixed_pack_partitions_into_one_group_per_species_in_first_appearance_order() {
    let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    let b = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
    let c = game.spawn_wild_creature("glitch", 5, 7).unwrap();
    let d = game.spawn_wild_creature("scrapper", 6, 5).unwrap();

    game.start_battle(vec![a, b, c, d]);

    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), 2, "two species means two groups");
    assert_eq!(battle.groups[0].species, "glitch", "glitch appeared first");
    assert_eq!(battle.groups[0].members, vec![a, c]);
    assert_eq!(battle.groups[1].species, "scrapper");
    assert_eq!(battle.groups[1].members, vec![b, d]);
}

/// Only MAX_ENEMY_GROUPS species can engage at once. The overflow stays
/// on the map as ordinary hostiles rather than being despawned — the
/// player meets them on the next bump.
#[test]
fn a_pack_of_more_than_four_species_engages_the_four_largest_and_leaves_the_rest() {
    let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // glitch x3, scrapper x2, virus x2, worm x2, sprite x1 -> sprite is
    // the smallest group and the one left out.
    let mut spawned = Vec::new();
    for (species, count) in [
        ("glitch", 3),
        ("scrapper", 2),
        ("virus", 2),
        ("worm", 2),
        ("sprite", 1),
    ] {
        for i in 0..count {
            spawned.push(game.spawn_wild_creature(species, 5, 5 + i).unwrap());
        }
    }
    let sprite = *spawned.last().unwrap();

    game.start_battle(spawned.clone());

    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), MAX_ENEMY_GROUPS);
    assert!(
        battle.groups.iter().all(|g| g.species != "sprite"),
        "the smallest group should be the one left out"
    );
    assert!(
        game.world.get_entity(sprite).is_ok(),
        "an un-engaged hostile must stay on the map, never be despawned"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
```sh
cargo test -p feral-processes-engine partitions_into_one_group
```
Expected: FAIL — `no field 'groups' on type 'BattleState'`.

- [ ] **Step 3: Add `EnemyGroup` and the constant**

In `crates/engine/src/battle.rs`, add at the top (after the existing `use` lines, adding `use bevy_ecs::prelude::Entity;` and `use crate::species::SpeciesId;`):

```rust
/// One species' worth of the wild pack in an active intrusion.
/// `members[0]` is the front — the only member that takes hits and the
/// only one whose HP the roster shows. Emptying a group removes it from
/// `BattleState::groups`, which promotes whatever sat behind it.
#[derive(Debug, Clone)]
pub struct EnemyGroup {
    pub species: SpeciesId,
    pub members: Vec<Entity>,
}

impl EnemyGroup {
    pub fn front(&self) -> Option<Entity> {
        self.members.first().copied()
    }
}
```

In `crates/engine/src/balance.rs`:

```rust
/// How many distinct species groups can engage in one intrusion. A
/// cluster with more species than this engages its largest groups and
/// leaves the remainder standing on the map as ordinary hostiles.
pub const MAX_ENEMY_GROUPS: usize = 4;
```

- [ ] **Step 4: Restructure `BattleState`**

Replace `crates/engine/src/resources.rs:119-132` with:

```rust
/// Active turn-based encounter between the player's party and one or
/// more wild species groups (see `battle::EnemyGroup`). Groups 0 and 1
/// are engaged and can melee; anything further back needs a ranged move
/// to reach the party. Removing this resource ends the battle.
#[derive(Resource)]
pub struct BattleState {
    pub player: Entity,
    pub groups: Vec<EnemyGroup>,
    pub round: u32,
    pub log: Vec<String>,
    pub finished: bool,
    pub player_won: bool,
}
```

Add `use crate::battle::EnemyGroup;` to that file's imports.

- [ ] **Step 5: Rewrite `start_battle` to partition by species**

Replace the body of `start_battle` (`lib.rs:2451`) with:

```rust
fn start_battle(&mut self, pack: Vec<Entity>) {
    let player = self.player_entity();
    // A `PlayerBuff` armed on the map by a consumable (see `use_item`'s
    // `prebattle_buff`) must carry into the fight it was armed for —
    // intentionally left untouched here, unlike `clear_battle_status_effects`.
    let mut groups: Vec<EnemyGroup> = Vec::new();
    for entity in pack {
        let Some(species) = self.world.get::<Creature>(entity).map(|c| c.species.clone()) else {
            continue;
        };
        match groups.iter_mut().find(|g| g.species == species) {
            Some(group) => group.members.push(entity),
            None => groups.push(EnemyGroup {
                species,
                members: vec![entity],
            }),
        }
    }
    // Overflow species stay on the map rather than being dropped from
    // the world. `sort_by_key` is stable, so ties keep first-appearance
    // order and the result stays deterministic for seeded tests.
    if groups.len() > MAX_ENEMY_GROUPS {
        groups.sort_by_key(|g| std::cmp::Reverse(g.members.len()));
        groups.truncate(MAX_ENEMY_GROUPS);
    }

    let anchor_name = groups
        .first()
        .and_then(|g| self.world.resource::<SpeciesDb>().get(&g.species))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "program".to_string());
    let others: usize = groups.iter().map(|g| g.members.len()).sum::<usize>() - 1;

    self.world.insert_resource(BattleState {
        player,
        groups,
        round: 1,
        log: Vec::new(),
        finished: false,
        player_won: false,
    });
    if others > 0 {
        self.log(format!(
            "A pack of rogue programs intercepts your signal — a {anchor_name} takes point, {others} more behind it!"
        ));
    } else {
        self.log(format!("A rogue {anchor_name} intercepts your signal!"));
    }
}
```

Note the truncate-after-sort reorders groups by size. That is deliberate and only fires in the overflow case; the first test asserts the non-overflow path keeps first-appearance order.

- [ ] **Step 6: Port the group accessors**

Replace `front_wild_creature` (`lib.rs:2482`) and add the group helpers:

```rust
/// The front member of `group` — the only one that takes hits.
fn front_of_group(&self, group: usize) -> Option<Entity> {
    self.world
        .get_resource::<BattleState>()?
        .groups
        .get(group)?
        .front()
}

/// How many groups are still standing.
fn living_group_count(&self) -> usize {
    self.world
        .get_resource::<BattleState>()
        .map(|b| b.groups.len())
        .unwrap_or(0)
}

/// Every living enemy across every group, in group-then-slot order.
fn all_living_enemies(&self) -> Vec<Entity> {
    let Some(battle) = self.world.get_resource::<BattleState>() else {
        return Vec::new();
    };
    battle
        .groups
        .iter()
        .flat_map(|g| g.members.iter().copied())
        .filter(|&e| self.creature_alive(e))
        .collect()
}
```

Replace `pop_front_pack_member` / `finish_front_pack_member` (`lib.rs:2635-2672`) with the group-aware pair. This keeps the existing loot / XP / nest-respawn behaviour verbatim:

```rust
/// Drops `group`'s front member, removing the group entirely if that
/// emptied it. Returns whether the whole battle is now over.
fn pop_group_member(&mut self, group: usize) -> bool {
    let mut battle = self.world.resource_mut::<BattleState>();
    let Some(g) = battle.groups.get_mut(group) else {
        return battle.groups.is_empty();
    };
    if !g.members.is_empty() {
        g.members.remove(0);
    }
    if g.members.is_empty() {
        battle.groups.remove(group);
    }
    battle.groups.is_empty()
}

/// Handles `group`'s front member dying: logs the kill, awards its
/// loot/XP, despawns it, and drops it from the group. Returns `true`
/// when that ended the whole battle.
fn finish_group_member(&mut self, group: usize, player: Entity) -> bool {
    let Some(front) = self.front_of_group(group) else {
        return self.living_group_count() == 0;
    };
    self.log("The rogue program crashes and deletes itself!");
    let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
    self.award_player_xp(player, wild_max_hp as u32);
    self.award_loot(front);
    let nest = self.world.get::<NestGuardian>(front).map(|g| g.nest);
    self.world.despawn(front);
    if let Some(nest) = nest
        && let Some(mut n) = self.world.get_mut::<Nest>(nest)
    {
        n.pending_respawns.push(NEST_RESPAWN_TICKS);
    }
    if self.pop_group_member(group) {
        self.clear_battle_status_effects(player, front);
        self.world.remove_resource::<BattleState>();
        true
    } else {
        self.log("Another rogue program from the pack engages!");
        false
    }
}
```

- [ ] **Step 7: Port the remaining call sites**

`all_wild_retaliate` (`lib.rs:2619`) iterates `all_living_enemies()` instead of `battle.wild_creatures`. `battle_attack`, `battle_decompile`, and `resolve_post_action` each swap `front_wild_creature()` for `front_of_group(0)` and `finish_front_pack_member(player)` for `finish_group_member(0, player)`.

`battle_view` (`lib.rs:2490`) keeps its current shape this task — it reads group 0's front for `wild_*`, and `pack_remaining` becomes the total living enemy count minus one:

```rust
let pack_remaining = self.all_living_enemies().len().saturating_sub(1);
```

Both renderers therefore need no changes in this task.

- [ ] **Step 8: Add the promotion test**

```rust
/// Wiping the front group promotes whatever sat behind it — the central
/// tension of the reach rule (spec §4): clearing front-to-back is not
/// automatically correct, because it walks the back rank into melee.
#[test]
fn wiping_the_front_group_promotes_the_group_behind_it() {
    let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let glitch = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    let scrapper = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
    game.start_battle(vec![glitch, scrapper]);
    let player = game.player_entity();

    assert_eq!(game.world.resource::<BattleState>().groups[0].species, "glitch");

    game.world.get_mut::<Stats>(glitch).unwrap().hp = 0;
    let battle_over = game.finish_group_member(0, player);

    assert!(!battle_over, "the scrapper group is still standing");
    let battle = game.world.resource::<BattleState>();
    assert_eq!(battle.groups.len(), 1);
    assert_eq!(
        battle.groups[0].species, "scrapper",
        "the surviving group should have shifted into index 0"
    );
}
```

- [ ] **Step 9: Run the full suite**

Run:
```sh
cargo test --workspace
```
Expected: PASS. Existing battle tests that referenced `wild_creatures` need porting to `groups[0].members` — do that as part of this step; do not delete a test to make it pass.

- [ ] **Step 10: Lint and commit**

```sh
cargo fmt && cargo clippy --workspace
git add crates/engine/src/battle.rs crates/engine/src/resources.rs crates/engine/src/lib.rs crates/engine/src/balance.rs
git commit -m "feat: model the wild pack as addressable species groups"
```

---

### Task 2: Initiative and `base_speed`

**Files:**
- Modify: `crates/engine/src/species.rs:101-164` (`SpeciesDef`)
- Modify: `crates/engine/src/battle.rs` (initiative roll)
- Modify: `crates/engine/src/balance.rs` (speed constants)
- Modify: all 17 `assets/species/*.ron`
- Modify: `assets/species/README.md`

**Interfaces:**
- Consumes: `EnemyGroup` from Task 1
- Produces: `SpeciesDef::base_speed: i32`; `battle::Actor { Party(usize), Enemy { group: usize, slot: usize } }`; `Game::roll_initiative(&mut self) -> Vec<Actor>` returning every living actor in descending initiative order

- [ ] **Step 1: Write the failing schema test**

In `crates/engine/src/species.rs`'s `mod tests`:

```rust
/// `base_speed` must be `#[serde(default)]` — a mod author's existing
/// species file predating the field has to keep loading untouched. That
/// is the modding contract, not a nicety.
#[test]
fn base_speed_defaults_when_a_species_file_omits_it() {
    let def: SpeciesDef = ron::from_str(
        r#"(
            id: "testmon",
            name: "Testmon",
            glyph: 't',
            color: Green,
            base_hp: 10,
            base_atk: 1,
            base_def: 1,
            taming_difficulty: 0.5,
            habitats: [OpenGrid],
            moves: [(name: "Poke", power: 1)],
            work_resource: None,
        )"#,
    )
    .expect("a species file with no base_speed must still parse");
    assert_eq!(def.base_speed, crate::balance::DEFAULT_BASE_SPEED);
}

#[test]
fn shipped_species_speeds_span_a_meaningful_range() {
    let (db, warnings) = SpeciesDb::load_dir(&species_assets_dir()).unwrap();
    assert!(warnings.is_empty(), "species assets should load cleanly: {warnings:?}");
    // A Construct is a wall and a Sprite is a spark; if those two ever
    // collapse to the same number, initiative has stopped meaning anything.
    assert!(db.get("sprite").unwrap().base_speed > db.get("construct").unwrap().base_speed);
}
```

- [ ] **Step 2: Run to verify it fails**

Run:
```sh
cargo test -p feral-processes-engine base_speed_defaults
```
Expected: FAIL — `no field 'base_speed'`.

- [ ] **Step 3: Add the constants**

In `crates/engine/src/balance.rs`:

```rust
/// Initiative baseline for a species whose `.ron` file omits
/// `base_speed` — the midpoint of the shipped roster's range, so an
/// un-annotated mod species is neither free initiative nor dead weight.
pub const DEFAULT_BASE_SPEED: i32 = 10;

/// The player's initiative baseline. A shade above `DEFAULT_BASE_SPEED`:
/// the player acts first against an average opponent, but loses the roll
/// to anything genuinely fast.
pub const PLAYER_BASE_SPEED: i32 = 11;

/// Initiative is `base_speed + rng.random_range(0..=INITIATIVE_DIE)`.
/// Sized so a 4-point speed gap still loses the roll sometimes — order
/// should be a tendency, not a lookup table.
pub const INITIATIVE_DIE: i32 = 10;
```

- [ ] **Step 4: Add the field**

In `crates/engine/src/species.rs`, inside `SpeciesDef`:

```rust
/// This species' initiative baseline — see `Game::roll_initiative`.
/// `#[serde(default)]` so existing species files (including mods)
/// without this field keep parsing at the roster average.
#[serde(default = "default_base_speed")]
pub base_speed: i32,
```

And beside `default_growth_multiplier`:

```rust
fn default_base_speed() -> i32 {
    crate::balance::DEFAULT_BASE_SPEED
}
```

- [ ] **Step 5: Author the data pass**

Add a `base_speed:` line to each file in `assets/species/`. Light and fast reads high, armoured reads low; bosses are fast *and* strong on purpose.

| File | `base_speed` | File | `base_speed` |
|---|---|---|---|
| `sprite.ron` | 14 | `virus.ron` | 10 |
| `glitch.ron` | 13 | `trojan.ron` | 10 |
| `drone.ron` | 13 | `worm.ron` | 9 |
| `wintermute.ron` | 13 | `scrapper.ron` | 9 |
| `sub_process.ron` | 12 | `rootkit.ron` | 9 |
| `phantom.ron` | 12 | `sentinel.ron` | 7 |
| `overseer.ron` | 12 | `construct.ron` | 6 |
| `wraith.ron` | 11 | `cipher.ron` | 11 |
| `ghost.ron` | 10 | | |

- [ ] **Step 6: Document the field**

In `assets/species/README.md`, add `base_speed` to the field table: optional, defaults to 10, integer, "initiative baseline — each round every combatant rolls `base_speed + d10` and acts in descending order."

- [ ] **Step 7: Write the failing initiative test**

In `crates/engine/src/lib.rs`'s `mod tests`:

```rust
/// Initiative order must be reproducible under a fixed seed. Every roll
/// goes through the existing `GameRng`, so a seeded test can assert an
/// exact order without touching the wall clock.
#[test]
fn initiative_order_is_reproducible_under_a_fixed_seed() {
    let order_for = |seed: u32| {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        let b = game.spawn_wild_creature("construct", 5, 6).unwrap();
        game.start_battle(vec![a, b]);
        game.roll_initiative()
    };
    assert_eq!(order_for(1234), order_for(1234), "same seed, same order");
}

/// Speed has to actually bias the order, or the stat is decoration.
/// Sampled rather than asserted per-round: a d10 on top of a 7-point
/// gap still lets the Construct win occasionally, and a test that
/// forbade that would be asserting the die doesn't exist.
#[test]
fn a_faster_species_wins_initiative_far_more_often_than_a_slower_one() {
    let mut sprite_first = 0;
    for seed in 0..200u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let sprite = game.spawn_wild_creature("sprite", 5, 5).unwrap();
        let construct = game.spawn_wild_creature("construct", 5, 6).unwrap();
        game.start_battle(vec![sprite, construct]);
        let order = game.roll_initiative();
        let pos = |e: Entity| {
            order
                .iter()
                .position(|a| game.actor_entity(*a) == Some(e))
                .unwrap()
        };
        if pos(sprite) < pos(construct) {
            sprite_first += 1;
        }
    }
    assert!(
        sprite_first > 150,
        "a Sprite (14) should beat a Construct (6) far more often than not, got {sprite_first}/200"
    );
}
```

- [ ] **Step 8: Implement initiative**

In `crates/engine/src/battle.rs`:

```rust
/// One combatant in an initiative order — an index rather than an
/// `Entity`, so a resolution walk can survive members dying mid-round
/// and can address a party slot that is empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    /// Slot 0 is the player; 1.. are party members in roster order.
    Party(usize),
    Enemy { group: usize, slot: usize },
}
```

In `crates/engine/src/lib.rs`:

```rust
/// Every living combatant in descending initiative order. Ties break on
/// a stable key — party before enemies, then slot/group index — so a
/// seeded run always produces the same order.
fn roll_initiative(&mut self) -> Vec<battle::Actor> {
    let Some(battle_state) = self.world.get_resource::<BattleState>() else {
        return Vec::new();
    };
    let groups: Vec<(usize, Vec<Entity>)> = battle_state
        .groups
        .iter()
        .enumerate()
        .map(|(i, g)| (i, g.members.clone()))
        .collect();
    let party = self.world.resource::<Party>().0.clone();
    let player = self.player_entity();

    let mut actors: Vec<(i32, u8, usize, battle::Actor)> = Vec::new();
    let mut push = |this: &mut Self, entity: Entity, base: i32, tier: u8, idx: usize, actor| {
        if !this.creature_alive(entity) {
            return;
        }
        let roll = {
            let mut rng = this.world.resource_mut::<GameRng>();
            rng.0.random_range(0..=INITIATIVE_DIE)
        };
        actors.push((base + roll, tier, idx, actor));
    };

    push(self, player, PLAYER_BASE_SPEED, 0, 0, battle::Actor::Party(0));
    for (i, member) in party.iter().enumerate() {
        let base = self.species_base_speed(*member);
        push(self, *member, base, 0, i + 1, battle::Actor::Party(i + 1));
    }
    for (group_idx, members) in groups {
        for (slot, member) in members.into_iter().enumerate() {
            let base = self.species_base_speed(member);
            push(
                self,
                member,
                base,
                1,
                group_idx * 100 + slot,
                battle::Actor::Enemy { group: group_idx, slot },
            );
        }
    }

    actors.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    actors.into_iter().map(|(_, _, _, actor)| actor).collect()
}

/// `entity`'s species `base_speed`, or the roster default if it has no
/// `Creature` component (the player).
fn species_base_speed(&self, entity: Entity) -> i32 {
    self.world
        .get::<Creature>(entity)
        .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
        .map(|s| s.base_speed)
        .unwrap_or(DEFAULT_BASE_SPEED)
}

/// The entity an `Actor` currently refers to, or `None` if that slot is
/// empty (a party member stood down, an enemy already despawned).
fn actor_entity(&self, actor: battle::Actor) -> Option<Entity> {
    match actor {
        battle::Actor::Party(0) => Some(self.player_entity()),
        battle::Actor::Party(i) => self.world.resource::<Party>().0.get(i - 1).copied(),
        battle::Actor::Enemy { group, slot } => self
            .world
            .get_resource::<BattleState>()?
            .groups
            .get(group)?
            .members
            .get(slot)
            .copied(),
    }
}
```

The closure borrows `self` mutably per call rather than holding a borrow across the loop — that is the borrow-checker-friendly shape here, not a `.clone()` workaround.

- [ ] **Step 9: Run and verify**

Run:
```sh
cargo test -p feral-processes-engine initiative
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 10: Lint and commit**

```sh
cargo fmt && cargo clippy --workspace
git add crates/engine/src/species.rs crates/engine/src/battle.rs crates/engine/src/balance.rs crates/engine/src/lib.rs assets/species/
git commit -m "feat: per-round initiative driven by a data-driven base_speed"
```

---

### Task 3: The action planning API

**Files:**
- Modify: `crates/engine/src/battle.rs` (`ActionKind`, `TargetSpec`, `ActionOption`, `BattleAction`)
- Modify: `crates/engine/src/resources.rs` (`BattleState::planned`)
- Modify: `crates/engine/src/lib.rs` (planning methods, `resolve_round`)

**Interfaces:**
- Consumes: `Actor` and `roll_initiative` from Task 2, `EnemyGroup` from Task 1
- Produces: `Game::battle_action_options(&self, slot: usize) -> Vec<ActionOption>`; `Game::battle_set_action(&mut self, slot: usize, action: BattleAction) -> Result<(), String>`; `Game::battle_clear_action(&mut self, slot: usize)`; `Game::battle_round_ready(&self) -> bool`; `Game::battle_resolve_round(&mut self)`; `Game::battle_active_slot(&self) -> Option<usize>`

- [ ] **Step 1: Write the failing planning tests**

```rust
/// The planning API is the whole extensibility story (spec §3): the
/// engine emits the menu, renderers dispatch off it. A slot that does
/// not exist must be refused rather than silently ignored.
#[test]
fn battle_set_action_refuses_a_slot_that_is_not_in_the_party() {
    let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    // Slot 0 is the player and always exists; slot 1 needs a companion.
    assert!(game.battle_set_action(0, BattleAction::Attack { group: 0 }).is_ok());
    let err = game
        .battle_set_action(1, BattleAction::Attack { group: 0 })
        .unwrap_err();
    assert!(err.contains("party"), "expected a party-slot error, got {err:?}");
}

#[test]
fn battle_resolve_round_is_a_no_op_until_every_slot_is_planned() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("construct", 5, 5).unwrap();
    let pet = game.spawn_wild_creature("sprite", 6, 6).unwrap();
    game.world.entity_mut(pet).remove::<Hostile>().insert(Tamed);
    game.add_companion(pet).unwrap();
    game.start_battle(vec![wild]);

    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    game.battle_set_action(0, BattleAction::Attack { group: 0 }).unwrap();
    assert!(!game.battle_round_ready(), "the companion has no action yet");
    game.battle_resolve_round();
    assert_eq!(
        game.world.get::<Stats>(wild).unwrap().hp,
        hp_before,
        "resolving a half-planned round must do nothing at all"
    );

    game.battle_set_action(1, BattleAction::Attack { group: 0 }).unwrap();
    assert!(game.battle_round_ready());
    game.battle_resolve_round();
    assert!(game.world.get::<Stats>(wild).unwrap().hp < hp_before);
}

/// Backing up a slot is how the player corrects a misclick — the cursor
/// has to walk back, not just blank the entry.
#[test]
fn battle_clear_action_walks_the_active_slot_back() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    assert_eq!(game.battle_active_slot(), Some(0));
    game.battle_set_action(0, BattleAction::Attack { group: 0 }).unwrap();
    assert_eq!(game.battle_active_slot(), None, "solo party is fully planned");
    game.battle_clear_action(0);
    assert_eq!(game.battle_active_slot(), Some(0));
}

/// The menu is data, not renderer strings. Decompile must report *why*
/// it is unavailable so the UI can grey it with a reason instead of
/// hiding it and leaving the player guessing.
#[test]
fn decompile_is_offered_with_a_reason_when_no_catalyst_is_held() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // `Inventory` exposes no `clear` — `items` is a public
    // `Vec<(ItemId, u32)>` (components.rs:201), so empty it directly.
    game.world.get_mut::<Inventory>(player).unwrap().items.clear();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);

    let options = game.battle_action_options(0);
    let decompile = options
        .iter()
        .find(|o| o.kind == ActionKind::Decompile)
        .expect("Decompile must be listed even when unusable");
    assert!(
        decompile.unavailable.as_deref().is_some_and(|r| r.contains("catalyst")),
        "expected a catalyst reason, got {:?}",
        decompile.unavailable
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run:
```sh
cargo test -p feral-processes-engine battle_set_action_refuses
```
Expected: FAIL — `no method named 'battle_set_action'`.

- [ ] **Step 3: Add the action types**

In `crates/engine/src/battle.rs`:

```rust
/// What a party member is doing this round. Adding a variant here plus
/// an arm in `Game::resolve_one_action` and a rule in
/// `Game::battle_action_options` is the *entire* cost of a new battle
/// action — no renderer changes, by design.
#[derive(Debug, Clone, PartialEq)]
pub enum BattleAction {
    Attack { group: usize },
    Special { group: usize },
    Defend,
    Decompile { group: usize },
    UseItem { item: ItemId },
}

/// The menu-facing identity of an action, without its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Attack,
    Special,
    Defend,
    Decompile,
    UseItem,
}

/// What the UI must collect before an `ActionKind` becomes a
/// `BattleAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSpec {
    None,
    EnemyGroup,
    InventoryItem,
}

/// One row of a party member's action menu. Renderers draw this
/// verbatim and never author an action string of their own.
#[derive(Debug, Clone)]
pub struct ActionOption {
    pub kind: ActionKind,
    /// Hotkey the engine assigns, so the two renderers cannot drift.
    pub key: char,
    /// e.g. "[A]ttack"
    pub label: String,
    /// e.g. "Rally: +3 ATK for 3 rounds"
    pub detail: String,
    pub target: TargetSpec,
    /// `Some(reason)` means render it greyed with the reason shown.
    pub unavailable: Option<String>,
}
```

Add `use crate::items::ItemId;` to `battle.rs`.

- [ ] **Step 4: Add the plan to `BattleState`**

In `resources.rs`, add to `BattleState`:

```rust
/// This round's chosen action per party slot — index 0 is the player,
/// 1.. are party members in roster order. `None` means "not yet
/// chosen"; a round resolves only once every slot is `Some`.
pub planned: Vec<Option<BattleAction>>,
```

`start_battle` initialises it to `vec![None; party.len() + 1]`.

- [ ] **Step 5: Implement the planning methods**

In `lib.rs`:

```rust
/// The party slot currently awaiting an action, or `None` when the
/// round is fully planned.
pub fn battle_active_slot(&self) -> Option<usize> {
    let battle = self.world.get_resource::<BattleState>()?;
    battle.planned.iter().position(|a| a.is_none())
}

pub fn battle_round_ready(&self) -> bool {
    self.world
        .get_resource::<BattleState>()
        .is_some_and(|b| b.planned.iter().all(|a| a.is_some()))
}

pub fn battle_set_action(&mut self, slot: usize, action: BattleAction) -> Result<(), String> {
    let group_count = self.living_group_count();
    let Some(battle) = self.world.get_resource::<BattleState>() else {
        return Err("No active intrusion.".to_string());
    };
    if slot >= battle.planned.len() {
        return Err(format!("Slot {slot} isn't in your party."));
    }
    let target_group = match &action {
        BattleAction::Attack { group }
        | BattleAction::Special { group }
        | BattleAction::Decompile { group } => Some(*group),
        _ => None,
    };
    if let Some(group) = target_group
        && group >= group_count
    {
        return Err("That group is already down.".to_string());
    }
    self.world.resource_mut::<BattleState>().planned[slot] = Some(action);
    Ok(())
}

/// Clears `slot`'s plan and every slot after it, so the cursor lands
/// back on `slot` — the player is correcting a choice, and everything
/// they picked *after* it was picked in light of the mistake.
pub fn battle_clear_action(&mut self, slot: usize) {
    let Some(mut battle) = self.world.get_resource_mut::<BattleState>() else {
        return;
    };
    for entry in battle.planned.iter_mut().skip(slot) {
        *entry = None;
    }
}
```

- [ ] **Step 6: Implement `battle_action_options`**

```rust
/// The action menu for party `slot`. This is the single place the
/// action set is defined; both renderers draw whatever this returns.
pub fn battle_action_options(&self, slot: usize) -> Vec<ActionOption> {
    let Some(entity) = self.actor_entity(battle::Actor::Party(slot)) else {
        return Vec::new();
    };
    let is_player = slot == 0;
    let mut options = vec![
        ActionOption {
            kind: ActionKind::Attack,
            key: 'a',
            label: "[A]ttack".to_string(),
            detail: "Strike a hostile group".to_string(),
            target: TargetSpec::EnemyGroup,
            unavailable: None,
        },
        ActionOption {
            kind: ActionKind::Defend,
            key: 'f',
            label: "De[f]end".to_string(),
            detail: format!("+{DEFEND_DEF_BONUS} DEF this round, and draw fire"),
            target: TargetSpec::None,
            unavailable: None,
        },
    ];

    if !is_player {
        let ability = self.companion_ability(entity);
        options.push(ActionOption {
            kind: ActionKind::Special,
            key: 's',
            label: "[S]pecial".to_string(),
            detail: ability
                .as_ref()
                .map(|a| a.display_label())
                .unwrap_or_else(|| "Rally: boost your attack".to_string()),
            target: TargetSpec::EnemyGroup,
            unavailable: None,
        });
    }

    if is_player {
        options.push(ActionOption {
            kind: ActionKind::Decompile,
            key: 'd',
            label: "[D]ecompile".to_string(),
            detail: "Attempt to capture a group's front program".to_string(),
            target: TargetSpec::EnemyGroup,
            unavailable: match (self.taming_catalyst(), self.pet_count() >= self.pet_capacity()) {
                (None, _) => Some("no taming catalyst".to_string()),
                (_, true) => Some("roster is full".to_string()),
                _ => None,
            },
        });
        options.push(ActionOption {
            kind: ActionKind::UseItem,
            key: 'u',
            label: "[U]se item".to_string(),
            detail: "Spend a consumable".to_string(),
            target: TargetSpec::InventoryItem,
            unavailable: self
                .battle_usable_items()
                .is_empty()
                .then(|| "no usable items".to_string()),
        });
    }

    options
}
```

Add `companion_ability(&self, entity) -> Option<SpecialAbility>` by extracting the lookup already inlined in `companion_ability_label` (`lib.rs:4351`) — that method then calls it, so the lookup exists once. `battle_usable_items` filters the player `Inventory` to consumables; reuse whatever predicate `use_item` already applies rather than writing a second one.

- [ ] **Step 7: Implement round resolution**

```rust
/// Resolves the planned round: everyone rolls initiative, acts in
/// order, and the plan is cleared for the next round. A no-op unless
/// every slot is planned.
pub fn battle_resolve_round(&mut self) {
    if !self.battle_round_ready() || self.is_game_over().is_some() {
        return;
    }
    let player = self.world.resource::<BattleState>().player;
    let plan = self.world.resource::<BattleState>().planned.clone();

    for actor in self.roll_initiative() {
        if self.world.get_resource::<BattleState>().is_none() {
            break;
        }
        let Some(entity) = self.actor_entity(actor) else {
            continue;
        };
        // Anything that died earlier this round doesn't get its turn —
        // initiative was rolled before any damage landed.
        if !self.creature_alive(entity) {
            continue;
        }
        if self.is_stunned(entity) {
            let name = self.actor_label(actor);
            self.log(format!("{name} stalls — stunned, and loses the turn!"));
            continue;
        }
        match actor {
            battle::Actor::Party(slot) => {
                if let Some(Some(action)) = plan.get(slot) {
                    self.resolve_one_action(slot, entity, action.clone(), player);
                }
            }
            battle::Actor::Enemy { .. } => self.wild_retaliate(entity, player),
        }
    }

    if let Some(mut battle) = self.world.get_resource_mut::<BattleState>() {
        battle.round += 1;
        let slots = battle.planned.len();
        battle.planned = vec![None; slots];
    }
    self.tick_round_status_effects(player);
    self.tick();
}

/// Executes one party member's chosen action. Every `BattleAction`
/// variant is handled here — this match is the one place a new action
/// needs an arm.
fn resolve_one_action(
    &mut self,
    slot: usize,
    entity: Entity,
    action: BattleAction,
    player: Entity,
) {
    match action {
        BattleAction::Attack { group } => {
            let Some(group) = self.retarget(group) else { return };
            self.party_member_attacks(slot, entity, group);
        }
        BattleAction::Special { group } => {
            let Some(group) = self.retarget(group) else { return };
            let Some(front) = self.front_of_group(group) else { return };
            let name = self.creature_label(entity);
            match self.companion_ability(entity) {
                Some(ability) => self.use_special_ability(&ability, &name, player, front),
                None => self.rally_player(entity, &name, player),
            }
        }
        BattleAction::Defend => self.begin_defend(entity),
        BattleAction::Decompile { group } => {
            let Some(group) = self.retarget(group) else { return };
            self.attempt_decompile(group);
        }
        BattleAction::UseItem { item } => {
            let _ = self.use_item(item);
        }
    }
}

/// A planned target group may have died earlier in the round. Fall
/// back to the lowest surviving group rather than wasting the turn.
fn retarget(&self, group: usize) -> Option<usize> {
    let count = self.living_group_count();
    if count == 0 {
        None
    } else if group < count {
        Some(group)
    } else {
        Some(0)
    }
}
```

`party_member_attacks` rolls from the actor's species moveset the way `wild_retaliate` (`lib.rs:3138`) already does, and calls `finish_group_member(group, player)` when the front dies. The player has no `Creature` component, so slot 0 keeps the existing flat power-5 strike — that stays true per the spec's non-goals.

`attempt_decompile(group)` is the existing `battle_decompile` body with the front target parameterised by group. `begin_defend` and `tick_round_status_effects` land in Task 5; for now `begin_defend` is a stub that logs and `tick_round_status_effects` calls the existing `tick_all_status_effects` against group 0's front.

- [ ] **Step 8: Keep the old entry points compiling**

Replace the bodies of `battle_attack`, `battle_decompile`, and `battle_command_companion` with plan-one-slot-and-resolve wrappers, so app-core and both renderers still build:

```rust
// BRANCH SCAFFOLDING — deleted in Task 7 once app-core drives the
// planning API directly. Do not build on these.
pub fn battle_attack(&mut self) {
    self.plan_every_slot_then_resolve(BattleAction::Attack { group: 0 });
}
```

`plan_every_slot_then_resolve` fills every slot with the given action (companions get `Attack`) and calls `battle_resolve_round`.

- [ ] **Step 9: Run and verify**

Run:
```sh
cargo test --workspace
```
Expected: PASS. Existing battle tests will need their assertions adjusted for companions now dealing damage — that is the intended behaviour change, so update the assertions rather than the code.

- [ ] **Step 10: Lint and commit**

```sh
cargo fmt && cargo clippy --workspace
git add crates/engine/src/battle.rs crates/engine/src/resources.rs crates/engine/src/lib.rs
git commit -m "feat: engine-owned per-slot action planning and round resolution"
```

---

### Task 4: The reach rule

**Files:**
- Modify: `crates/engine/src/species.rs:89-99` (`MoveDef`)
- Modify: `crates/engine/src/lib.rs:3138` (`wild_retaliate`)
- Modify: `crates/engine/src/balance.rs` (`ENGAGED_GROUPS`)
- Modify: all 17 `assets/species/*.ron`, `assets/species/README.md`

**Interfaces:**
- Consumes: `EnemyGroup`, `roll_initiative`, `Actor`
- Produces: `MoveDef::ranged: bool`; `balance::ENGAGED_GROUPS`

- [ ] **Step 1: Write the failing reach tests**

```rust
/// The reach rule is the balance valve that makes a 12-enemy fight
/// survivable (spec §4). A back group with only melee moves can't
/// connect at all.
#[test]
fn a_back_group_with_only_melee_moves_cannot_reach_the_party() {
    let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Scrapper and Sentinel are authored melee-only; Construct too.
    let a = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
    let b = game.spawn_wild_creature("sentinel", 5, 6).unwrap();
    let c = game.spawn_wild_creature("construct", 5, 7).unwrap();
    game.start_battle(vec![a, b, c]);
    let player = game.player_entity();
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;

    // Group 2 (Construct) is behind the engaged pair and melee-only.
    let construct = game.front_of_group(2).unwrap();
    game.wild_retaliate(construct, player);

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        hp_before,
        "a melee-only back group must deal no damage"
    );
    assert_eq!(hp_before, game.world.get::<Stats>(player).unwrap().hp);
}

/// ...but a back group holding a ranged move connects normally. Without
/// this half, the test above passes just as well against a bug that
/// makes back groups unconditionally inert.
#[test]
fn a_back_group_with_a_ranged_move_still_connects() {
    let mut game = Game::new(85, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
    let b = game.spawn_wild_creature("sentinel", 5, 6).unwrap();
    // Glitch's "Static Burst" is authored ranged.
    let c = game.spawn_wild_creature("glitch", 5, 7).unwrap();
    game.start_battle(vec![a, b, c]);
    let player = game.player_entity();
    let hp_before = game.world.get::<Stats>(player).unwrap().hp;

    let glitch = game.front_of_group(2).unwrap();
    for _ in 0..20 {
        game.wild_retaliate(glitch, player);
    }

    assert!(
        game.world.get::<Stats>(player).unwrap().hp < hp_before,
        "a ranged back group must be able to land a hit"
    );
}

#[test]
fn ranged_defaults_to_melee_when_a_move_omits_it() {
    let def: MoveDef = ron::from_str(r#"(name: "Poke", power: 3)"#)
        .expect("a move with no `ranged` field must still parse");
    assert!(!def.ranged, "moves default to melee");
}
```

- [ ] **Step 2: Run to verify they fail**

Run:
```sh
cargo test -p feral-processes-engine back_group
```
Expected: FAIL — `no field 'ranged'`.

- [ ] **Step 3: Add the constant and field**

In `balance.rs`:

```rust
/// How many enemy groups are in melee range of the party. Groups past
/// this index can only act with a move flagged `ranged`, which is what
/// keeps a four-group pack from simply quadrupling incoming damage.
pub const ENGAGED_GROUPS: usize = 2;
```

In `species.rs`, inside `MoveDef`:

```rust
/// Whether this move reaches past the front line. A group behind
/// `balance::ENGAGED_GROUPS` can only use its ranged moves.
/// `#[serde(default)]` so existing species files (including mods)
/// without this field keep parsing as melee, exactly as before.
#[serde(default)]
pub ranged: bool,
```

- [ ] **Step 4: Gate move selection on reach**

In `wild_retaliate` (`lib.rs:3138`), replace the unconditional move pick. The caller knows the group index; take it as a parameter (`wild_retaliate(&mut self, wild: Entity, group: usize, player: Entity)`) and thread it from `battle_resolve_round`'s `Actor::Enemy { group, .. }` arm:

```rust
let engaged = group < ENGAGED_GROUPS;
let candidates: Vec<MoveDef> = species
    .moves
    .iter()
    .filter(|m| engaged || m.ranged)
    .cloned()
    .collect();
if candidates.is_empty() {
    let name = self.creature_label(wild);
    self.log(format!("{name} circles beyond reach, unable to strike."));
    return;
}
```

Then pick uniformly from `candidates` using the existing `GameRng` and proceed with the current damage/status logic unchanged.

- [ ] **Step 5: Author the ranged data pass**

Add `ranged: true` to exactly the moves below. Every species keeps at least one melee move, so front-rank behaviour is unchanged. Scrapper, Sentinel and Construct get **none** — they are deliberately melee-only bruisers, and the reach test above depends on that.

| File | Move flagged `ranged: true` |
|---|---|
| `cipher.ron` | Cross-Reference |
| `drone.ron` | Recon Ping |
| `ghost.ron` | Static Wail |
| `glitch.ron` | Static Burst |
| `overseer.ron` | Purge |
| `phantom.ron` | Spoof |
| `rootkit.ron` | Kernel Panic |
| `sprite.ron` | Ping |
| `sub_process.ron` | Signal Spam |
| `trojan.ron` | Payload Drop |
| `virus.ron` | Exploit |
| `wintermute.ron` | Cascade Logic |
| `worm.ron` | Replicate |
| `wraith.ron` | Freeze |

- [ ] **Step 6: Document the field**

In `assets/species/README.md`, add `ranged` to the move-table docs: optional, defaults to `false`, "a move flagged `ranged: true` can be used from the back rank; a group with no ranged moves idles when it isn't one of the front two."

- [ ] **Step 7: Run and verify**

Run:
```sh
cargo test -p feral-processes-engine back_group ranged_defaults
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 8: Lint and commit**

```sh
cargo fmt && cargo clippy --workspace
git add crates/engine/src/species.rs crates/engine/src/lib.rs crates/engine/src/balance.rs assets/species/
git commit -m "feat: back-rank enemy groups need a ranged move to reach the party"
```

---

### Task 5: Defend, soft ranks, sizes, and the save break

**Files:**
- Modify: `crates/engine/src/lib.rs` (`begin_defend`, targeting weights, `max_pack_size:3843`)
- Modify: `crates/engine/src/resources.rs:136` (`MAX_PARTY_SIZE`)
- Modify: `crates/engine/src/components.rs` (`ActiveBuff` reuse for the defend bonus)
- Modify: `crates/engine/src/balance.rs` (size and defend constants)
- Modify: `crates/engine/src/save.rs:48` (`CreatureSave::party_slot`), `:171` (`SAVE_FORMAT_VERSION`)

**Interfaces:**
- Consumes: `BattleAction::Defend` from Task 3
- Produces: `Game::begin_defend(&mut self, entity: Entity)`; `CreatureSave::party_slot: Option<u32>`; `SAVE_FORMAT_VERSION = 10`

- [ ] **Step 1: Write the failing tests**

```rust
/// Defend has to actually reduce incoming damage, or it's a wasted turn
/// dressed up as a choice.
#[test]
fn defending_reduces_the_damage_a_party_member_takes_this_round() {
    let damage_taken = |defend: bool| {
        let mut game = Game::new(86, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
        game.start_battle(vec![wild]);
        let player = game.player_entity();
        let before = game.world.get::<Stats>(player).unwrap().hp;
        game.battle_set_action(
            0,
            if defend { BattleAction::Defend } else { BattleAction::Attack { group: 0 } },
        )
        .unwrap();
        game.battle_resolve_round();
        before - game.world.get::<Stats>(player).unwrap().hp
    };
    assert!(
        damage_taken(true) < damage_taken(false),
        "a defended round must cost less HP than an undefended one"
    );
}

/// Party order is mechanically meaningful under soft ranks, so it has
/// to survive a save/load round trip. Before this change the party was
/// rebuilt in creature-iteration order.
#[test]
fn party_order_survives_a_save_load_round_trip() {
    let dir = std::env::temp_dir().join("feral_party_order_roundtrip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut game = Game::new(87, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut ordered = Vec::new();
    for (i, species) in ["sprite", "glitch", "scrapper"].iter().enumerate() {
        let pet = game.spawn_wild_creature(species, 6 + i as i32, 6).unwrap();
        game.world.entity_mut(pet).remove::<Hostile>().insert(Tamed);
        game.add_companion(pet).unwrap();
        ordered.push((*species).to_string());
    }
    let path = dir.join("slot.sav");
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let names: Vec<String> = loaded
        .world
        .resource::<Party>()
        .0
        .iter()
        .filter_map(|&e| loaded.world.get::<Creature>(e).map(|c| c.species.clone()))
        .collect();
    assert_eq!(names, ordered, "party order must round-trip exactly");
    let _ = std::fs::remove_dir_all(&dir);
}
```

`Game::save(&mut self, path) -> io::Result<()>` (`lib.rs:893`) and `Game::load(path, assets_dir)` (`lib.rs:632`) are the entry points; the free functions in `save.rs` are one layer down and not what a test should call.

- [ ] **Step 2: Run to verify they fail**

Run:
```sh
cargo test -p feral-processes-engine defending_reduces party_order_survives
```
Expected: FAIL.

- [ ] **Step 3: Add the constants**

In `balance.rs`:

```rust
/// DEF granted for the round by the Defend action.
pub const DEFEND_DEF_BONUS: i32 = 6;

/// Relative weight a defending member carries in the enemy target roll
/// — bracing draws fire, which is what makes it a party-level play
/// rather than a selfish one.
pub const DEFEND_AGGRO_WEIGHT: u32 = 4;

/// Target weight for a party member in a front slot (0..FRONT_SLOTS)
/// versus one behind it. Soft ranks: a back-slot member can still be
/// hit, just far less often.
pub const FRONT_SLOT_AGGRO_WEIGHT: u32 = 3;
pub const BACK_SLOT_AGGRO_WEIGHT: u32 = 1;
pub const FRONT_SLOTS: usize = 3;

/// Hard ceiling on one intrusion's wild pack, across every group.
pub const MAX_PACK_SIZE: u32 = 12;
```

- [ ] **Step 4a: Give companions a `PlayerBuff` component**

**This is a prerequisite, not an optional check.** `PlayerBuff` is inserted at exactly two sites — `lib.rs:608` (`Game::new`) and `lib.rs:720` (`Game::load`) — and both are the *player*. Companions do not have the component, so `get_mut::<PlayerBuff>` on one returns `None` and `begin_defend` would silently no-op: Defend would appear in the menu, log its message, and do nothing.

`effective_def` (`lib.rs:2808`) already reads `PlayerBuff` on any entity, so once the component exists the DEF bonus flows through with no further change.

Insert `PlayerBuff::default()` wherever a creature becomes tamed — the decompile success path, `fuse_companions`, and the tamed branch of the load path. Find them with:

```sh
rg -n "insert\(Tamed|Tamed\)" crates/engine/src/lib.rs
```

The name `PlayerBuff` is now wrong — it is a per-combatant buff slot. Rename it to `CombatBuff` in the same step, including the doc comment at `components.rs:425`. It is a `#[derive(Component)]` with no serde impl and does not appear in `SaveData`, so this rename does not touch the save format.

Add a test that fails before the fix:

```rust
/// Defend is offered to companions, so a companion must be able to
/// hold the buff it grants. Before this, `PlayerBuff` existed only on
/// the player and a companion's Defend silently did nothing.
#[test]
fn a_tamed_companion_can_hold_a_combat_buff() {
    let mut game = Game::new(88, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = game.spawn_wild_creature("sprite", 6, 6).unwrap();
    game.world.entity_mut(pet).remove::<Hostile>().insert(Tamed);
    assert!(
        game.world.get::<CombatBuff>(pet).is_some(),
        "a tamed program must carry a buff slot, or Defend no-ops on it"
    );
}
```

- [ ] **Step 4b: Implement Defend**

Reuse the existing `ActiveBuff` machinery rather than inventing a parallel one:

```rust
/// Braces `entity` for the round: a DEF bonus plus a raised share of
/// incoming fire. Duration is 1, so it expires in
/// `tick_round_status_effects` at the end of the same round.
fn begin_defend(&mut self, entity: Entity) {
    let Some(mut buff) = self.world.get_mut::<CombatBuff>(entity) else {
        // Every combatant gets a buff slot in Step 4a. Reaching here
        // means one was spawned without it, which is a bug, not a state
        // to tolerate silently.
        debug_assert!(false, "combatant {entity:?} has no CombatBuff slot");
        return;
    };
    buff.active = Some(ActiveBuff {
        kind: BuffKind::Def,
        remaining: 1,
        power: DEFEND_DEF_BONUS,
    });
    let name = self.creature_label(entity);
    self.log(format!("{name} braces against the next strike."));
}
```

Defend overwrites any existing buff, since `CombatBuff` holds a single `active` slot. That is a real design consequence — a companion that braces loses a Rally it was carrying. Acceptable, and worth a line in the action's `detail` text so the player isn't surprised.

- [ ] **Step 5: Implement soft-rank targeting**

Replace the flat `COMPANION_RETALIATION_CHANCE` roll in `wild_retaliate` with a weighted pick across the player and every party member:

```rust
/// Weighted target roll: front slots draw more fire than back ones, and
/// a bracing member draws more still. Soft ranks (spec §4) — every
/// member stays targetable, order just changes the odds.
fn roll_enemy_target(&mut self, player: Entity) -> Entity {
    let party = self.world.resource::<Party>().0.clone();
    let mut pool: Vec<(Entity, u32)> = Vec::new();
    for (slot, entity) in std::iter::once(player).chain(party).enumerate() {
        if !self.creature_alive(entity) {
            continue;
        }
        let mut weight = if slot < FRONT_SLOTS {
            FRONT_SLOT_AGGRO_WEIGHT
        } else {
            BACK_SLOT_AGGRO_WEIGHT
        };
        if self.is_defending(entity) {
            weight += DEFEND_AGGRO_WEIGHT;
        }
        pool.push((entity, weight));
    }
    let total: u32 = pool.iter().map(|(_, w)| w).sum();
    let mut roll = {
        let mut rng = self.world.resource_mut::<GameRng>();
        rng.0.random_range(0..total)
    };
    for (entity, weight) in &pool {
        if roll < *weight {
            return *entity;
        }
        roll -= weight;
    }
    player
}
```

`is_defending` checks for an active `BuffKind::Def` buff with `power == DEFEND_DEF_BONUS`.

- [ ] **Step 6: Raise the sizes**

`resources.rs:136`: `MAX_PARTY_SIZE` 3 → 5.

`lib.rs:3843`, `max_pack_size`:

```rust
fn max_pack_size(&self, x: i32, y: i32) -> u32 {
    let zone = self.world.resource::<ZoneLevel>().0;
    let cap = (zone * 3).min(MAX_PACK_SIZE).max(1);
    let dist = self.distance_from_danger_origin(x, y);
    let grown = 1 + (dist / PACK_SIZE_STEP_TILES) as u32;
    grown.min(cap)
}
```

`PACK_GATHER_RADIUS` (`lib.rs:90`) 2 → 3.

The existing `max_pack_size_grows_with_zone_and_distance_and_caps_per_zone` and `max_pack_size_also_counts_from_the_platform_edge` tests encode the old cap. Update their expected numbers — the *shape* they assert (solo at spawn, growth per step, capped per zone) still holds and must keep holding.

- [ ] **Step 7: Break the save format deliberately**

In `save.rs`, add to `CreatureSave`:

```rust
/// This program's index in the player's active party, or `None` if it
/// isn't a party member. Party order is mechanically meaningful under
/// soft ranks (front slots draw more fire), so it can't be rebuilt from
/// creature-iteration order the way it was before.
pub party_slot: Option<u32>,
```

`is_companion` becomes redundant — `party_slot.is_some()` says the same thing. Delete it and update both write and read paths. Leaving both would be exactly the backwards-compat cruft `CLAUDE.md` forbids.

Bump `SAVE_FORMAT_VERSION` 9 → 10 (`save.rs:171`). On load, sort party members by `party_slot` before inserting the `Party` resource, replacing the current push-in-iteration-order at `lib.rs:790-801`. Keep the `party.truncate(MAX_PARTY_SIZE)` guard.

- [ ] **Step 8: Run and verify**

Run:
```sh
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 9: Lint and commit**

```sh
cargo fmt && cargo clippy --workspace
git add crates/engine/src/
git commit -m "feat: Defend, soft-rank targeting, larger parties and packs

Bumps SAVE_FORMAT_VERSION to 10 — party order is now persisted, which
is a CreatureSave shape change. Existing saves are rejected."
```

---

### Task 6: Rewrite the balance projections

**Files:**
- Modify: `crates/engine/src/balance.rs` (the offline sim and its regression tests)

**Interfaces:**
- Consumes: every constant from Tasks 1–5
- Produces: updated `grind_only_zone_scaling_grows_predictably` and `geared_zone_scaling_grows_predictably_and_beats_grind_only`

**Context:** `balance.rs` has ~76 uncommitted insertions from in-flight work (the mid-grade-party sweep). Read the working tree before editing; do not revert it.

- [ ] **Step 1: Read what's there**

Run:
```sh
git diff crates/engine/src/balance.rs
```
Understand the in-flight change before touching it. The module simulates the *old* loop: one action per round with a `RALLY_CADENCE` of 4.

- [ ] **Step 2: Write the failing survivability test**

```rust
/// The party-size change compounds three ways (spec §6): +67% passive
/// ATK/DEF via `party_stat_bonus`, two more attackers, and two more
/// bodies to absorb hits. The pack-size increase is the counterweight.
/// This test is the only evidence that ratio is survivable before
/// anyone plays it.
#[test]
fn a_full_party_survives_a_full_pack_at_each_zone() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir()).unwrap();
    for zone in 1..=5u32 {
        let outcome = simulate_roster_fight(&db, zone);
        assert!(
            outcome.player_won,
            "zone {zone}: a full party lost to a full pack — pack/party ratio is off"
        );
        assert!(
            outcome.rounds > 3,
            "zone {zone}: won in {} rounds, which means the fight is trivial",
            outcome.rounds
        );
    }
}
```

- [ ] **Step 3: Rewrite the simulation**

`simulate_roster_fight` models the new loop:

- Party of `MAX_PARTY_SIZE` mid-grade companions (`median_ordinary_species`, already in the file) plus the player.
- Enemy pack of `MAX_PACK_SIZE` scaled for the zone, split into `MAX_ENEMY_GROUPS` groups.
- Each round: every party member attacks; every enemy in groups `0..ENGAGED_GROUPS` attacks; enemies behind that attack only if their species has a ranged move.
- Damage stays `battle::compute_damage`, so the sim tracks any future rebalance of the real formula.

Delete `RALLY_CADENCE` and the rally modelling — companions attack now. Keep `TURN_CAP` as the stalemate guard.

Use mean move power (`mean_move_power`, already in the file) rather than sampling. The sim is a deterministic projection, not a Monte Carlo run — no RNG belongs here.

- [ ] **Step 4: Retune until it passes**

If the test fails, the correct lever is `MAX_PACK_SIZE` or the `zone * 3` multiplier in `max_pack_size`, **not** weakening the enemies or deleting the assertion. Record whatever you land on in the commit message.

- [ ] **Step 5: Run and verify**

Run:
```sh
cargo test -p feral-processes-engine balance
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 6: Lint and commit**

```sh
cargo fmt && cargo clippy --workspace
git add crates/engine/src/balance.rs
git commit -m "test: reproject balance against the roster round loop"
```

---

### Task 7: `BattleView` and the app-core state machine

**Files:**
- Modify: `crates/engine/src/lib.rs:421-458` (`BattleView`), `:2490` (`battle_view`)
- Modify: `crates/app-core/src/lib.rs:159-217` (`Mode`), `:873-948` (battle key handlers)
- Modify: `crates/tui/src/ui.rs`, `crates/gui/src/render.rs` (minimal ports to compile)

**Interfaces:**
- Consumes: `battle_action_options`, `battle_set_action`, `battle_clear_action`, `battle_resolve_round`, `battle_active_slot`, `battle_round_ready`
- Produces: `EnemyGroupView`, `PartySlotView`, the new `BattleView`; `Mode::BattleTarget`, `Mode::BattleResolve`

- [ ] **Step 1: Write the failing app-core test**

```rust
/// The action set lives in the engine. If a renderer or app-core
/// hardcodes a key, the two frontends drift the moment an action is
/// added — which is the exact failure this indirection exists to
/// prevent.
#[test]
fn battle_keys_come_from_the_engine_action_options() {
    let mut app = app_in_battle();
    let game = app.game.as_ref().unwrap();
    let keys: Vec<char> = game.battle_action_options(0).iter().map(|o| o.key).collect();
    assert!(keys.contains(&'a') && keys.contains(&'f') && keys.contains(&'d'));

    // Every advertised key must be accepted case-insensitively — the
    // prompt shows "[A]ttack", so Shift+A is what the player presses.
    for key in keys {
        let mut app = app_in_battle();
        app.handle_key(GameKey::Char(key.to_ascii_uppercase()));
        assert_ne!(
            app.mode,
            Mode::Playing,
            "[{key}] is advertised by the engine but was swallowed"
        );
    }
}
```

Reuse the existing `app_in_battle` helper at `app-core/src/lib.rs:2105`.

- [ ] **Step 2: Run to verify it fails**

Run:
```sh
cargo test -p feral-processes-app-core battle_keys_come_from
```
Expected: FAIL.

- [ ] **Step 3: Rewrite `BattleView`**

Replace `lib.rs:421-458`:

```rust
pub struct EnemyGroupView {
    /// Display letter, 'A'.. — how the player addresses this group.
    pub letter: char,
    pub species_name: String,
    pub count: usize,
    pub front_hp: i32,
    pub front_max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub is_boss: bool,
    /// Whether this group is in melee range (spec §4).
    pub engaged: bool,
    pub status_effect: Option<String>,
}

pub struct PartySlotView {
    pub slot: usize,
    pub entity: Entity,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub status_effect: Option<String>,
    /// This round's chosen action, rendered for the roster, or `None`
    /// if the slot is still awaiting one.
    pub planned: Option<String>,
    /// Front slots draw more enemy fire — soft ranks (spec §4).
    pub front: bool,
}

pub struct BattleView {
    pub groups: Vec<EnemyGroupView>,
    pub party: Vec<PartySlotView>,
    pub active_slot: Option<usize>,
    pub options: Vec<ActionOption>,
    pub round: u32,
    pub log: Vec<String>,
    pub decompile_chance: Option<f32>,
    pub player_decompiler: i32,
}
```

- [ ] **Step 4: Add the modes**

In `app-core/src/lib.rs`, replace `Mode::BattleCompanion` with:

```rust
/// Picking which enemy group the pending action targets. Entered from
/// `Mode::Battle` when the chosen `ActionOption` has
/// `TargetSpec::EnemyGroup`.
BattleTarget,
/// Paging through the narration of a resolved round before the next
/// planning phase begins.
BattleResolve,
```

Add `pending_battle_action: Option<ActionKind>` to `App`, holding the action awaiting a target.

- [ ] **Step 5: Rewrite the key handlers**

`handle_battle_key` dispatches off engine data instead of literal keys:

```rust
fn handle_battle_key(&mut self, key: GameKey) {
    let GameKey::Char(c) = key else {
        if key == GameKey::Esc {
            self.battle_back_up();
        }
        return;
    };
    let c = c.to_ascii_lowercase();
    if c == 'j' {
        // Jack Out stays a party-level command, not a per-member action.
        self.battle_jack_out();
        return;
    }
    let Some(game) = &self.game else { return };
    let Some(slot) = game.battle_active_slot() else { return };
    let Some(option) = game
        .battle_action_options(slot)
        .into_iter()
        .find(|o| o.key == c)
    else {
        return;
    };
    if let Some(reason) = option.unavailable {
        self.status_line = Some(format!("Can't do that — {reason}."));
        return;
    }
    match option.target {
        TargetSpec::None => self.commit_battle_action(slot, BattleAction::Defend),
        TargetSpec::EnemyGroup => {
            self.pending_battle_action = Some(option.kind);
            self.mode = Mode::BattleTarget;
        }
        TargetSpec::InventoryItem => self.mode = Mode::Inventory,
    }
}
```

`commit_battle_action` calls `battle_set_action`, then `battle_resolve_round` and switches to `Mode::BattleResolve` if `battle_round_ready()`. `handle_battle_target_key` maps A/B/C/D to a group index, builds the `BattleAction` from `pending_battle_action`, and commits. `handle_battle_resolve_key` returns to `Mode::Battle`, or `Mode::Playing` when the battle ended.

`TargetSpec::None` hardcoding `Defend` is wrong the moment a second untargeted action exists. Build the action from `option.kind` via a small `fn action_from(kind: ActionKind, group: Option<usize>) -> Option<BattleAction>` helper so a new untargeted action needs one arm there, not a rewrite here.

- [ ] **Step 6: Delete the scaffolding**

Remove `battle_attack`, `battle_decompile`, `battle_command_companion`, `plan_every_slot_then_resolve`, and `Mode::BattleCompanion` along with `handle_battle_companion_key` and `render_battle_companion_menu` / `draw_battle_companion_menu`. Nothing may still reference them.

Run:
```sh
rg -n "battle_attack|battle_command_companion|BattleCompanion|plan_every_slot" crates/
```
Expected: no matches.

- [ ] **Step 7: Port the renderers minimally**

Both `render_battle` and `draw_battle` need only to compile and show something coherent — the real screens are Tasks 8 and 9. A vertical list of groups then party then the option labels is enough.

- [ ] **Step 8: Run and verify**

Run:
```sh
cargo test --workspace
```
Expected: PASS.

- [ ] **Step 9: Lint and commit**

```sh
cargo fmt && cargo clippy --workspace
git add crates/engine/src/lib.rs crates/app-core/src/lib.rs crates/tui/src/ui.rs crates/gui/src/render.rs
git commit -m "feat: data-driven battle menu and per-slot planning state machine"
```

---

### Task 8: The TUI roster screen

**Files:**
- Modify: `crates/tui/src/ui.rs:1753-1871` (`render_battle`)

**Interfaces:**
- Consumes: `BattleView`, `EnemyGroupView`, `PartySlotView`, `ActionOption`

- [ ] **Step 1: Build the layout**

Five vertical regions: enemy groups (`Length(groups.len() + 2)`), party roster (`Length(party.len() + 2)`), decompile odds (`Length(1)`), log (`Min(5)`), action bar (`Length(3)`).

Enemy rows, one per group, using the existing `Gauge` widget and `status_tag` helper:

```
A  3 Glitches   ‹engaged›  [██████░░] 18/30  Bleeding(2)
```

Back groups render with `Style::new().fg(Color::DarkGray)` and `‹back›`, so reach is legible at a glance rather than something the player has to infer from a log line.

Party rows show slot number, name, HP gauge, ATK/DEF, status, and the planned action once chosen. The active slot is highlighted with the existing `menu_line` helper's selected style.

- [ ] **Step 2: Render the action bar from engine data**

```rust
let actions: Vec<Span> = view
    .options
    .iter()
    .map(|o| match &o.unavailable {
        None => Span::styled(o.label.clone(), Style::new()),
        Some(reason) => Span::styled(
            format!("{} ({reason})", o.label),
            Style::new().fg(Color::DarkGray),
        ),
    })
    .collect();
```

Plus a literal `[J]ack Out`, which is the one party-level command and deliberately not an `ActionOption`.

- [ ] **Step 3: Render the target picker and the resolve page**

`Mode::BattleTarget` overlays a prompt listing group letters. `Mode::BattleResolve` shows the round's log lines with a `[Space] next round` footer.

- [ ] **Step 4: Verify**

Run:
```sh
cargo test --workspace && cargo run -p feral-processes
```
Walk into a pack and check: groups letter-addressed, back groups greyed, every party member gets a prompt, the round resolves in initiative order.

This is the one place in the plan where a human's eyes are the gate — layout is not unit-testable, and the standing policy is to verify drawing changes by reading code and unit tests rather than launching the GUI. The TUI is cheap to run, so run it.

- [ ] **Step 5: Lint and commit**

```sh
cargo fmt && cargo clippy --workspace
git add crates/tui/src/ui.rs
git commit -m "feat: TUI roster battle screen"
```

---

### Task 9: The GUI roster screen

**Files:**
- Modify: `crates/gui/src/render.rs:1758-1913` (`draw_battle`)
- Modify: `crates/gui/src/fx.rs:219-247` (`battle_frame`)

**Interfaces:**
- Consumes: the same `BattleView`
- Produces: `Fx::bar_ghost(&mut self, key: u64, value: i32, dt: f32) -> BarFx`

- [ ] **Step 1: Write the failing fx test**

```rust
/// The ghost-bar trail tracked exactly two HP scalars. A roster has one
/// bar per group and per party slot, and they must animate
/// independently — a shared ghost would make every bar jump whenever
/// any one of them changed.
#[test]
fn bar_ghosts_are_tracked_independently_per_key() {
    let mut fx = Fx::new(true);
    fx.bar_ghost(1, 100, 0.0);
    fx.bar_ghost(2, 100, 0.0);
    let a = fx.bar_ghost(1, 40, 0.016);
    let b = fx.bar_ghost(2, 100, 0.016);
    assert!(a.damage > 0, "key 1 took damage");
    assert_eq!(b.damage, 0, "key 2 was untouched and must not inherit it");
    assert!(b.ghost > a.ghost, "the two ghosts must not share state");
}
```

- [ ] **Step 2: Run to verify it fails**

Run:
```sh
cargo test -p feral-processes-gui bar_ghosts_are_tracked
```
Expected: FAIL — `no method named 'bar_ghost'`.

- [ ] **Step 3: Generalise the fx**

Replace the two-scalar `BattleFx` state with a `HashMap<u64, BarState>`, keyed by group index for enemies and `1000 + slot` for party members. Keep the existing `ghost_step` and the seed-on-first-frame behaviour — entering a battle must still not animate a drain from zero. Delete `battle_frame`.

- [ ] **Step 4: Mirror the TUI layout**

Same five regions, using the GUI's own `Metrics` and font helpers. Back groups desaturated rather than greyed — reuse the existing desaturation helper in `render.rs` rather than adding a second one.

Floating damage numbers (`spawn_float`) now spawn per row, positioned at that row's bar.

- [ ] **Step 5: Verify**

Run:
```sh
cargo test --workspace
cargo clippy --workspace
```

Per the standing policy, GUI drawing changes are verified by code reading and unit tests, not by launching the window — and per the `macroquad` Wayland note, the GUI has never actually been run in this environment, so a failure to launch would be a pre-existing condition, not evidence about this change. Leave the visual sign-off to the user.

- [ ] **Step 6: Commit**

```sh
cargo fmt
git add crates/gui/src/render.rs crates/gui/src/fx.rs
git commit -m "feat: GUI roster battle screen with per-row ghost bars"
```

---

### Task 10: Documentation

**Files:**
- Modify: `README.md`, `CHANGELOG.md`

- [ ] **Step 1: Update the README**

The spec's §Documentation obligations lists the verified-stale sections. Rewrite each:

| Line | What to fix |
|---|---|
| 111 | party "(max 3)" → max 5 |
| 127–133 | the intrusion key table: `[A]ttack`, `De[f]end`, `[S]pecial`, `[D]ecompile`, `[U]se item`, `[J]ack Out`, plus group targeting |
| 314 | Fatigue no longer framed around `c` / commanding a companion |
| 316 | damage formula now applies to the whole party via species movesets |
| 504–526 | the Companions section: companions fight, aren't commanded; `[C]ommand companion` is gone |

Add a short section explaining groups, engaged versus back, and initiative — this is the headline mechanic and the README is where a player looks for it.

- [ ] **Step 2: Update the CHANGELOG**

Lead with the breaking change; do not bury it in a feature bullet:

```markdown
### Breaking

- **Saves from earlier versions no longer load.** Party order is now
  persisted (front slots draw more enemy fire), which changes the save
  record's shape. `SAVE_FORMAT_VERSION` is 10; a v9 save is rejected
  with a clear message rather than loading corrupted.
```

Then the feature entry covering groups, per-member actions, initiative, the reach rule, Defend, and the party-size change.

- [ ] **Step 3: Final gate**

Run:
```sh
cargo test --workspace && cargo clippy --workspace && cargo fmt --check
```
Expected: PASS, clean, no diff.

- [ ] **Step 4: Commit**

```sh
git add README.md CHANGELOG.md
git commit -m "docs: roster battles in the README and CHANGELOG"
```

**Note:** `CLAUDE.md` is gitignored in this repo, so any change to it never ships with the branch. Nothing in this plan requires editing it.

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: §1 groups → Task 1; §2 round loop → Tasks 2–3; §3 actions → Task 3 (Defend's implementation in Task 5); §4 reach → Task 4; §5 schema → Tasks 2 and 4; §6 balance → Tasks 5–6; §7 persistence → Task 5; §8 renderers → Tasks 7–9; §Documentation obligations → Task 10.

**Type consistency.** `finish_group_member(group, player)`, `front_of_group(group)`, `retarget(group)`, `actor_entity(actor)`, and `species_base_speed(entity)` are used with the same signatures wherever they appear. `ActionKind` is the menu identity, `BattleAction` the parameterised action — the two are never conflated.

**Resolved while reviewing** (these were assumptions; all three were checked against the source):

- `Inventory` has no `clear` method — `add` / `count` / `cargo_used` / `add_capped` only. `items` is a public `Vec<(ItemId, u32)>`, so the test empties it directly.
- `Game::save(&mut self, path)` (`lib.rs:893`) and `Game::load(path, assets_dir)` (`lib.rs:632`) are the real entry points.
- **`PlayerBuff` genuinely is player-only** — inserted at `lib.rs:608` and `:720`, both player-spawn sites. Companion Defend would have silently no-opped. This became Task 5 Step 4a, including a rename to `CombatBuff`, since the component is no longer player-scoped.

**Remaining soft spots, flagged rather than papered over:**

1. **Task 6 has a retune loop with no predetermined answer.** Whether a party of 6 beats a pack of 12 at `zone * 3` is genuinely unknown until the sim runs. The step names the lever to pull and forbids weakening the assertion, but it cannot say what number lands. This is the one task that could expand significantly.
2. **Existing battle tests will need updating in Tasks 1 and 3.** Companions dealing damage is a deliberate behaviour change, so assertions move. Both steps say to update assertions — never to delete a test to make the suite green.
3. **Task 5 is the largest task** and could reasonably split (Defend + ranks / sizes + save break). It is kept whole because the save bump and the party-size change are the same user-visible event, and shipping a version bump twice would be worse.
4. **Group order changes in the overflow case.** `start_battle` sorts by size before truncating, so a >4-species pack does not preserve first-appearance order. Deliberate, tested one way, and called out at Task 1 Step 5.
