# Program Permadeath Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A tamed program whose HP reaches 0 is destroyed and removed from the world for good, taking its installed routines with it and dropping nothing.

**Architecture:** Death is detected at one chokepoint — `Game::apply_damage`, the only code path that lowers a tamed program's HP — and executed at two reap sites. In battle the despawn is deferred to `end_battle`, because `BattleState::planned` indexes `Party` positionally and removing a member mid-fight would shift every member behind it into the wrong slot. Outside battle (a raid defender) the despawn is immediate. Both sites route through the existing `dissolve_tamed_program`, already the single call that destroys a tamed program. The GUI gains a shared `hp_critical` helper driving a red warning in the two screens where the player can get a program killed.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone in `crates/engine`), Bevy + `bevy_egui` in `crates/gui`. Tests are `#[test]` fns compiled into the crates themselves.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-28-program-permadeath-design.md`. Read it before starting.
- Branch is already created: `feat/program-permadeath`. Do not create another and do not merge to `main`.
- The engine's `Game` struct is the entire public API the renderer talks to. Never add a `world` accessor. `crates/gui` must not touch the ECS `World`.
- `crates/gui/src/paint.rs` is the only file allowed to name a graphics library. Nothing in `crates/gui/src/render/` may call a backend directly.
- Run `cargo fmt` and `cargo clippy --workspace` after every change; fix warnings rather than silencing them.
- Comments explain *why*, never *what*. A doc comment that claims to mirror, match, or share another module's logic must be a call, not a copy.
- Named constants over magic numbers.
- No backwards-compat cruft, no `// removed` comments, no shims. If something becomes unused, delete it.
- Difficulty tuning belongs in `crates/engine/src/tuning.rs`. Presentation constants do not — they live with the code that draws them.
- Full-suite gate is `cargo test --workspace`. Baseline before this work: **681 tests passing**.
- A cold build after a dependency change takes minutes, not seconds. Budget for it.
- If many tests fail at once with `NotFound` on an assets path, that is stale build artifacts, not real failures: run `cargo clean -p feral-processes-engine -p feral-processes-app-core` (never a full `cargo clean` — `target/` is ~4 GB).
- Pre-existing uncommitted edits to four files under `assets/abilities/` were in the working tree before this work began. They are not yours. Do not stage, revert, or commit them.

---

### Task 1: Announce a party member's death at the damage chokepoint

Detection only — nothing is despawned yet. After this task a companion brought to 0 HP writes an `Outcome` line naming itself and the routines that will die with it, and otherwise behaves exactly as it does today.

**Files:**
- Modify: `crates/engine/src/game/combat_status.rs:306-310` (`apply_damage`)
- Test: `crates/engine/src/tests/combat_status.rs` (append)

**Interfaces:**
- Consumes: `Game::creature_label(Entity) -> String` (`game/party.rs:78`), `Game::extractable_routines(&self, Entity) -> Vec<AbilityDef>` (`game/routines.rs:251`), `Game::log_kind(MessageKind, impl Into<String>)` (`game/turn.rs:24`), the `Party` resource (`resources.rs`, a tuple struct whose `.0` is a `Vec<Entity>`).
- Produces: `Game::announce_program_death(&mut self, program: Entity)`, private to the `game` module — Task 2 and Task 3 do **not** call it; it fires only from `apply_damage`.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/combat_status.rs`:

```rust
/// The death line has to be `Outcome`, not the default `Info` kind:
/// `MessageLog::retain_outcomes_since_battle` prunes everything else when
/// the battle ends, so an `Info` line would be announced mid-fight and then
/// silently vanish before the player reached the map.
#[test]
fn a_companion_brought_to_zero_announces_its_deletion_and_its_lost_routines() {
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["priority_boost".to_string()]));
    game.add_companion(companion).unwrap();
    let name = game.creature_label(companion);

    game.apply_damage(companion, 10);

    let (kind, line) = game
        .message_log(20)
        .into_iter()
        .find(|(_, l)| l.contains(&name) && l.contains("deleted"))
        .expect("a companion reaching 0 HP must announce its deletion");
    assert_eq!(
        kind,
        MessageKind::Outcome,
        "the death line must survive retain_outcomes_since_battle"
    );
    assert!(
        line.contains("Priority Boost"),
        "the line must name the routines lost with it, got: {line}"
    );
}

/// The guard is `Party` membership. A hostile reaching 0 is already handled
/// by `finish_member`, which logs its own kill line and awards loot — a
/// second announcement here would double-report every kill in the game.
#[test]
fn a_hostile_brought_to_zero_is_not_announced_by_the_companion_death_path() {
    let mut game = Game::new(4243, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 5;

    game.apply_damage(wild, 5);

    assert!(
        !game
            .message_log(20)
            .iter()
            .any(|(_, l)| l.contains("deleted for good")),
        "only party members route through the companion death announcement"
    );
}

/// The player is not a party member and must never be reaped by this path —
/// flatlining stays with `difficulty::death_handling_system`, which is what
/// `DifficultyMode` selects between. A player deleted from the world would
/// take the whole run with it.
#[test]
fn the_player_at_zero_hp_is_not_touched_by_the_program_death_path() {
    let mut game = Game::new(4245, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let hp = game.world.get::<Stats>(player).unwrap().hp;

    game.apply_damage(player, hp);

    assert!(
        game.world.get::<Stats>(player).is_some(),
        "the player entity survives 0 HP; only difficulty handling may act on it"
    );
    assert!(
        !game
            .message_log(20)
            .iter()
            .any(|(_, l)| l.contains("deleted for good")),
        "the player does not get a program death line"
    );
}

/// Damage that hurts without killing must stay silent, and a second hit on
/// an already-dead member must not announce twice — the transition is
/// `> 0` to `0`, not "is at 0".
#[test]
fn the_death_line_fires_once_on_the_transition_to_zero_and_never_above_it() {
    let mut game = Game::new(4244, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    game.apply_damage(companion, 4);
    assert!(
        !game
            .message_log(20)
            .iter()
            .any(|(_, l)| l.contains("deleted for good")),
        "a survivable hit must not announce a death"
    );

    game.apply_damage(companion, 6);
    game.apply_damage(companion, 6);
    let announcements = game
        .message_log(20)
        .iter()
        .filter(|(_, l)| l.contains("deleted for good"))
        .count();
    assert_eq!(
        announcements, 1,
        "hitting a corpse again must not re-announce its death"
    );
}
```

Check the top of `crates/engine/src/tests/combat_status.rs` for its existing imports. It should already carry `use super::support::*;` and `use crate::*;`. If `Routines` is not in scope through `crate::*`, add `use crate::components::Routines;`.

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p feral-processes-engine a_companion_brought_to_zero_announces
```

Expected: FAIL — no line containing "deleted" is found, so the `.expect(...)` panics.

- [ ] **Step 3: Write the implementation**

In `crates/engine/src/game/combat_status.rs`, replace `apply_damage` (currently lines 306-310) with:

```rust
    /// Applies `dmg` to `target`, floored at 0.
    ///
    /// Death is detected here rather than at the six call sites because this
    /// is the only path that lowers a tamed program's HP — every other write
    /// to `Stats::hp` is a heal, a full-heal, or `needs_decay_system`, which
    /// is `With<Player>`. A seventh caller added later inherits the check for
    /// free; six separate checks would not.
    ///
    /// Only party members are announced. A hostile reaching 0 is reported by
    /// `finish_member`, and the player by `difficulty::death_handling_system`.
    pub(crate) fn apply_damage(&mut self, target: Entity, dmg: i32) {
        let killed = {
            let Some(mut stats) = self.world.get_mut::<Stats>(target) else {
                return;
            };
            let was_alive = stats.hp > 0;
            stats.hp = (stats.hp - dmg).max(0);
            was_alive && stats.hp == 0
        };
        if killed && self.world.resource::<Party>().0.contains(&target) {
            self.announce_program_death(target);
        }
    }

    /// The `Outcome` line for a party member killed in battle: what died and
    /// what died with it.
    ///
    /// Emitted the moment its HP reaches 0, while the entity itself lives on
    /// until `end_battle` reaps it — see that method for why the removal has
    /// to wait.
    fn announce_program_death(&mut self, program: Entity) {
        let name = self.creature_label(program);
        let routines: Vec<String> = self
            .extractable_routines(program)
            .into_iter()
            .map(|def| def.name)
            .collect();
        let line = if routines.is_empty() {
            format!("{name} crashes and is deleted for good.")
        } else {
            format!(
                "{name} crashes and is deleted for good, taking {} with it.",
                routines.join(", ")
            )
        };
        self.log_kind(MessageKind::Outcome, line);
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p feral-processes-engine a_companion_brought_to_zero_announces
cargo test -p feral-processes-engine a_hostile_brought_to_zero_is_not
cargo test -p feral-processes-engine the_death_line_fires_once
cargo test -p feral-processes-engine the_player_at_zero_hp_is_not_touched
```

Expected: all PASS.

- [ ] **Step 5: Check nothing else regressed**

```bash
cargo test -p feral-processes-engine
```

Expected: PASS. Some existing tests call `apply_damage` on party members during fixture setup (`tests/combat_abilities.rs:25,58,77,78`) — those damage members by 20 against larger pools and should not reach 0. If one now fails on an unexpected log line, the fixture is genuinely killing a member; raise its `spawn_tamed` HP rather than weakening the new check.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/game/combat_status.rs crates/engine/src/tests/combat_status.rs
git commit -m "feat: announce a party member's deletion when it hits 0 HP"
```

---

### Task 2: Reap dead party members when the battle ends

**Files:**
- Modify: `crates/engine/src/game/combat_status.rs:606-642` (`end_battle`)
- Test: `crates/engine/src/tests/party.rs` (modify the existing test at line ~409, and append two new ones)

**Interfaces:**
- Consumes: `Game::dissolve_tamed_program(&mut self, creature: Entity) -> String` (`game/trade.rs:164`) — logs detachments, drops the program from `Party`, removes its `Task`, despawns it, and returns the label it logged. `Game::creature_alive(Entity) -> bool` (`game/combat_status.rs:312`).
- Produces: no new public surface. After `end_battle`, a party member that was at 0 HP no longer exists in the `World`.

- [ ] **Step 1: Strengthen the existing test**

In `crates/engine/src/tests/party.rs`, the test around line 409 currently asserts only that the party is empty after the fight. "Removed from the party" and "deleted" are now different claims and the weaker one no longer proves the stronger. Replace the block that runs from the `flee_until_clear(&mut game);` call through its following `assert!` with:

```rust
            flee_until_clear(&mut game);
            assert!(
                game.player_status().companions.is_empty(),
                "ending the battle should have stood the downed companion down"
            );
            assert!(
                game.world.get::<Stats>(companion).is_none(),
                "a companion that hit 0 HP is deleted, not merely stood down"
            );
            assert!(
                !game.owned_pets().iter().any(|p| p.entity == companion),
                "and it is gone from the roster, not just the party"
            );
            return;
```

`world.get::<Stats>(e).is_none()` is this repo's established way to assert an entity is gone — `tests/trade.rs:96` checks a sold program the same way. Use it rather than reaching for `World::get_entity`, so the two read alike.

- [ ] **Step 2: Write the new failing tests**

Append to `crates/engine/src/tests/party.rs`:

```rust
/// Nothing drops. A program's routines die with it — the only way to get a
/// routine back off a program is `extract_routine` at a bench, and that
/// destroys the program deliberately.
#[test]
fn a_companion_killed_in_battle_returns_none_of_its_routines_to_inventory() {
    let assets = test_assets_dir();
    let mut game = Game::new(5150, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["priority_boost".to_string()]));
    game.add_companion(companion).unwrap();

    let routine_item = crate::abilities::routine_item_id("priority_boost");
    let before = game
        .world
        .get::<Inventory>(player)
        .map(|i| i.count(&routine_item))
        .unwrap_or(0);

    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.apply_damage(companion, 10);
    flee_until_clear(&mut game);

    let after = game
        .world
        .get::<Inventory>(player)
        .map(|i| i.count(&routine_item))
        .unwrap_or(0);
    assert_eq!(
        before, after,
        "a dead program's routines are destroyed with it, not dropped"
    );
}

/// The reap has to run before `retain_outcomes_since_battle`, or the
/// `Info`-kind detachment lines `dissolve_tamed_program` writes ("leaves
/// your battle party") would survive and trail the death line onto the map.
#[test]
fn only_the_outcome_death_line_follows_the_player_out_of_the_battle() {
    let assets = test_assets_dir();
    let mut game = Game::new(5151, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    let name = game.creature_label(companion);

    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.apply_damage(companion, 10);
    flee_until_clear(&mut game);

    let log = game.message_log(40);
    assert!(
        log.iter()
            .any(|(k, l)| *k == MessageKind::Outcome && l.contains("deleted for good")),
        "the death line survives the end of the battle"
    );
    assert!(
        !log.iter()
            .any(|(_, l)| l.contains(&name) && l.contains("leaves your battle party")),
        "the dissolve's departure chatter must be pruned, not trail the death line"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

```bash
cargo test -p feral-processes-engine a_companion_killed_in_battle_returns_none
cargo test -p feral-processes-engine only_the_outcome_death_line_follows
```

Expected: FAIL — the routine test may pass vacuously (nothing drops today either), but `only_the_outcome_death_line_follows` fails because no death line exists after the battle, and the strengthened test from Step 1 fails because the entity still exists.

- [ ] **Step 4: Write the implementation**

In `crates/engine/src/game/combat_status.rs`, in `end_battle`, replace the `downed`/`retain` block (currently lines 622-635) with:

```rust
        let dead: Vec<Entity> = self
            .world
            .resource::<Party>()
            .0
            .iter()
            .copied()
            .filter(|&e| !self.creature_alive(e))
            .collect();
        // Before `retain_outcomes_since_battle` below, deliberately: the
        // detachment lines `dissolve_tamed_program` writes are `Info` kind,
        // so running the reap first is what prunes them and leaves the
        // `Outcome` death line to reach the map alone.
        for program in dead {
            self.dissolve_tamed_program(program);
        }
```

Then update `end_battle`'s doc comment. It currently says companions are "finally stood down"; they are now destroyed. Change the first paragraph to:

```rust
    /// Tears the current battle down: every combat-only effect cleared from
    /// both sides, companions killed during the fight finally reaped, and
    /// `BattleState` dropped.
    ///
    /// Reaping the dead happens here rather than the moment they fall
    /// because `BattleState::planned` indexes `Party` positionally (see
    /// `actor_entity`) — removing a member mid-battle shifts every member
    /// behind it into the wrong slot. The death itself is announced when it
    /// happens, in `apply_damage`; only the despawn waits.
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cargo test -p feral-processes-engine a_companion_killed_in_battle_returns_none
cargo test -p feral-processes-engine only_the_outcome_death_line_follows
cargo test -p feral-processes-engine -- party
```

Expected: all PASS.

- [ ] **Step 6: Run the engine suite**

```bash
cargo test -p feral-processes-engine
```

Expected: PASS. Watch for `tests/combat_abilities.rs:226` (`a_whole_party_heal_raises_every_living_member_and_skips_the_downed`) — it parks a companion at 0 HP by writing `Stats` directly and never ends the battle, so it should be unaffected. If it fails, read it before changing it; the fixture may need the battle left open rather than the reap weakened.

- [ ] **Step 7: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/game/combat_status.rs crates/engine/src/tests/party.rs
git commit -m "feat: a party member killed in battle is destroyed, not stood down"
```

---

### Task 3: Reap a raid defender killed away from the player

**Files:**
- Modify: `crates/engine/src/game/upkeep.rs:164-170` (the tail of `raid_check`)
- Test: `crates/engine/src/tests/raids.rs` (append)

**Interfaces:**
- Consumes: `Game::dissolve_tamed_program` (as Task 2), `Game::creature_alive`.
- Produces: no new surface.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/raids.rs`. Follow the fixture style already in that file — the test at line ~52 (`damage_structure_destroys_it_and_clears_its_cronjob_at_zero_durability`) shows how to stand up a structure with a worker assigned:

```rust
/// Programs have no passive regen and the player is not present, so raid
/// chip damage is pure attrition: a worker left on a cronjob long enough
/// dies unattended. That is the intended cost, not an oversight.
#[test]
fn a_raid_defender_brought_to_zero_is_destroyed_rather_than_standing_down() {
    let mut game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let existing: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Durability>>();
        query.iter(&game.world).collect()
    };
    for e in existing {
        game.world.despawn(e);
    }
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability {
                hp: 1000,
                max_hp: 1000,
            },
        ))
        .id();
    // Exactly one raid's worth of defender damage left, so the first raid
    // that lands kills it and the test never depends on how many fire.
    let worker = spawn_tamed(&mut game, RAID_DEFENDER_DAMAGE, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    for _ in 0..2000 {
        game.raid_check();
        if game.world.get::<Stats>(worker).is_none() {
            break;
        }
    }

    assert!(
        game.world.get::<Stats>(worker).is_none(),
        "a defender knocked to 0 HP is destroyed, not stood down"
    );
    assert!(
        game.message_log(200)
            .iter()
            .any(|(k, l)| *k == MessageKind::Raid && l.contains("destroyed defending")),
        "the loss is reported as a Raid line, since the player wasn't there to see it"
    );
}
```

The loop bound is generous because `RAID_CHANCE_PER_TICK` is 0.012 — roughly 1 raid per 83 calls, so 2000 calls is ~24 expected raids against a need for one. It breaks as soon as the worker is gone, so it costs nothing when it works. If the assertion is flaky across seeds, that is a real signal: read `raid_check` rather than re-running.

`RAID_DEFENDER_DAMAGE` is already imported at the top of `tests/raids.rs`. Confirm `Task`, `TaskKind`, and `Stats` are in scope via `use crate::*;`.

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p feral-processes-engine a_raid_defender_brought_to_zero
```

Expected: FAIL — the worker still exists, because today it only sheds its `Task`.

- [ ] **Step 3: Write the implementation**

In `crates/engine/src/game/upkeep.rs`, replace the closing block of `raid_check` (currently lines 164-170) with:

```rust
        self.apply_damage(worker, RAID_DEFENDER_DAMAGE);
        if !self.creature_alive(worker) {
            self.log_kind(
                MessageKind::Raid,
                format!("{worker_label} is destroyed defending {target_label}."),
            );
            // Stripped before the dissolve, not by it. `raid_check` finds its
            // defender *by* this `Task`, so the program is always working the
            // structure the line above already names — leaving the `Task` on
            // would have `sale_detachments` add a redundant "stops working
            // the Mining Node" directly beneath it.
            self.world.entity_mut(worker).remove::<Task>();
            self.dissolve_tamed_program(worker);
        }
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test -p feral-processes-engine a_raid_defender_brought_to_zero
```

Expected: PASS.

- [ ] **Step 5: Run the engine suite**

```bash
cargo test -p feral-processes-engine
```

Expected: PASS.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace
git add crates/engine/src/game/upkeep.rs crates/engine/src/tests/raids.rs
git commit -m "feat: a raid defender killed defending a structure is destroyed"
```

---

### Task 4: Flag a program in danger of being lost

**Files:**
- Modify: `crates/gui/src/render/mod.rs` (add the constant, the helper, and a test to the existing `mod tests` at line 444)
- Modify: `crates/gui/src/render/popup.rs:39-56` (add `critical_item_row` beside `spent_item_row`)
- Modify: `crates/gui/src/render/battle.rs:302` (bar colour)
- Modify: `crates/gui/src/render/party.rs:27-43` (party menu row)

**Interfaces:**
- Consumes: `RED` (`render/mod.rs:68`), `Row` and `item_row` (`render/popup.rs:9,27`), `PetInfo` (`feral_processes_engine::views`) for its `hp`/`max_hp` fields, `BattlePartyView` rows in `render/battle.rs` for the same.
- Produces: `hp_critical(hp: i32, max_hp: i32) -> bool` in `render/mod.rs`, reachable from every child of `render` via its `use super::*;`. `critical_item_row(s: impl Into<String>, selected: bool) -> Row` in `render/popup.rs`.

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` block in `crates/gui/src/render/mod.rs` (line 444):

```rust
    #[test]
    fn hp_critical_triggers_at_exactly_a_third_and_not_a_point_above() {
        assert!(hp_critical(10, 30), "exactly a third is already critical");
        assert!(!hp_critical(11, 30), "a point above a third is not");
        assert!(hp_critical(0, 30), "a dead program reads as critical");
        assert!(!hp_critical(30, 30), "full health is never critical");
        assert!(
            !hp_critical(0, 0),
            "a program with no max HP is a malformed fixture, not a warning"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p feral-processes-gui hp_critical_triggers_at_exactly_a_third
```

Expected: FAIL to compile — `hp_critical` is not defined.

If `-p feral-processes-gui` is not the right package name, read the `[package] name` in `crates/gui/Cargo.toml` and use that.

- [ ] **Step 3: Add the constant and the helper**

In `crates/gui/src/render/mod.rs`, beside `BACK_RANK_DESATURATION` (line 79):

```rust
/// A program at or below `1 / CRITICAL_HP_DIVISOR` of its Integrity is
/// flagged as about to be lost. At 0 it is deleted for good, so the warning
/// has to arrive before the hit that gets it there rather than after.
///
/// A presentation threshold, not a difficulty knob — nothing in the sim
/// reads it — so it lives here with the colours rather than in the engine's
/// `tuning.rs`.
const CRITICAL_HP_DIVISOR: i32 = 3;

/// Whether a program is close enough to deletion to warrant the warning
/// colour. The single definition both the battle pane and the party menu
/// call, so the threshold cannot come to mean two different things on two
/// screens.
pub(super) fn hp_critical(hp: i32, max_hp: i32) -> bool {
    max_hp > 0 && hp * CRITICAL_HP_DIVISOR <= max_hp
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test -p feral-processes-gui hp_critical_triggers_at_exactly_a_third
```

Expected: PASS.

- [ ] **Step 5: Colour the battle bar**

In `crates/gui/src/render/battle.rs`, replace line 302:

```rust
        let color = if active { CYAN } else { GREEN };
```

with:

```rust
        // Danger outranks the active-slot cue: the `>` prefix and the bold
        // face already mark who is acting, and nothing else on this screen
        // says "one more hit and this is gone".
        let color = if hp_critical(p.hp, p.max_hp) {
            RED
        } else if active {
            CYAN
        } else {
            GREEN
        };
```

Leave the `draw_ghost_band` call below it alone — it already takes `color` and inherits the change.

- [ ] **Step 6: Add `critical_item_row`**

In `crates/gui/src/render/popup.rs`, after `spent_item_row` (line 46):

```rust
/// `item_row` for a program close enough to 0 HP that another fight could
/// delete it for good — see `hp_critical`. Callers pair it with a CRITICAL
/// tag in the row text, so the warning still reads without colour.
pub(super) fn critical_item_row(s: impl Into<String>, selected: bool) -> Row {
    Row::Item {
        text: s.into(),
        selected,
        bold: false,
        color: RED,
    }
}
```

- [ ] **Step 7: Flag the party menu rows**

In `crates/gui/src/render/party.rs`, replace the loop body at lines 19-43 with:

```rust
    for (i, p) in pets.iter().enumerate() {
        let activity = activity_tag(&p.activity);
        let quality = p
            .quality
            .as_ref()
            .map(|q| format!(" [{q}]"))
            .unwrap_or_default();
        let fused = fusion_tag(p.fusions);
        let critical = hp_critical(p.hp, p.max_hp);
        let text = format!(
            "[{}] {} Lv{} - HP {}/{}  ATK {}  DEF {}  PWR {}{}{}{}{}",
            menu_shortcut(i),
            p.name,
            p.level,
            p.hp,
            p.max_hp,
            p.atk,
            p.def,
            p.power,
            quality,
            fused,
            activity,
            if critical { " - CRITICAL" } else { "" }
        );
        rows.push(if critical {
            critical_item_row(text, i == selected)
        } else {
            item_row(text, i == selected)
        });
    }
```

`critical_item_row` reaches `party.rs` through its existing `use super::popup::*;` at line 3.

- [ ] **Step 8: Build, lint, and run the gui suite**

```bash
cargo fmt
cargo clippy --workspace
cargo test -p feral-processes-gui
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/gui/src/render/mod.rs crates/gui/src/render/popup.rs crates/gui/src/render/battle.rs crates/gui/src/render/party.rs
git commit -m "feat: flag a program in danger of being deleted"
```

---

### Task 5: Correct the docs the change falsifies

**Files:**
- Modify: `docs/manual.md:124`, `docs/manual.md:683-684`, `docs/manual.md:976-977`
- Modify: `CHANGELOG.md` (Unreleased section)

**Interfaces:** none — prose only.

- [ ] **Step 1: Fix the party-knockout claim**

`docs/manual.md` lines 683-684 currently read:

```
  member holding Defend draws more still, which is what makes bracing a
  play for the whole party rather than a selfish one. A party member knocked
  to 0 HP stands down automatically — it isn't lost, just no longer active;
  re-add it (`p`) and recharge overnight (`r`) to heal it back up.
```

Replace the two sentences from "A party member knocked" onward with:

```
  member holding Defend draws more still, which is what makes bracing a
  play for the whole party rather than a selfish one. A party member brought
  to 0 HP is **deleted for good** at the end of the fight, taking every
  routine installed on it — nothing drops, and there is no reviving it. The
  battle pane turns a member's bar red once it is down to a third of its
  Integrity; that is your warning to jack out.
```

- [ ] **Step 2: Fix the raid-defender claim**

`docs/manual.md` lines 976-977 currently read:

```
  lose. A defender knocked to 0 HP stands down (like a knocked-out
  companion), but isn't destroyed.
```

Replace with:

```
  lose. A defender brought to 0 HP is **destroyed** — and since programs
  have no passive healing, raid damage accumulates until you come home and
  recharge (`r`). A program left on a cronjob long enough will eventually
  be lost to raids while you are elsewhere.
```

- [ ] **Step 3: Fix the "shed a program for good" claim**

`docs/manual.md` line 124 currently ends:

```
Standing one down frees a battle slot, not a roster slot; to shed a program for good, sell it at a Market (`t`) or fuse it (`f`) |
```

Replace that clause with:

```
Standing one down frees a battle slot, not a roster slot; to shed a program for good, sell it at a Market (`t`), fuse it (`f`), or extract a routine from it (`M`) — and losing one in a fight or a raid does the same, permanently |
```

- [ ] **Step 4: Add the CHANGELOG entry**

In `CHANGELOG.md`, under `## Unreleased`, add a `### Balance` bullet. If a `### Balance` heading already exists in Unreleased, append to it rather than adding a second:

```markdown
- **A program brought to 0 HP is deleted for good.** It used to be knocked
  offline — dropped from your party at the end of the fight, then healed
  back to full for free the next time you recharged. Now it is destroyed,
  along with every routine installed on it, and nothing drops. This applies
  in Forgiving as well as Permadeath (that setting still governs only what
  happens when *you* flatline), and outside battle as well as in it: a
  cronjob worker that runs out of Integrity defending a structure is lost
  even though you were not there. Programs have no passive healing, so raid
  damage is attrition you have to manage by coming home to recharge. The
  battle pane and the party menu now flag any program at or below a third of
  its Integrity in red.
```

- [ ] **Step 5: Check for other claims the change falsifies**

```bash
grep -rn "stands down\|knocked offline\|isn't lost\|not destroyed" docs/manual.md README.md
```

Read each hit. Fix any that now assert a program survives 0 HP. The README's permadeath sentence (line 28) describes *player* death and is still accurate — leave it.

- [ ] **Step 6: Commit**

```bash
git add docs/manual.md CHANGELOG.md
git commit -m "docs: programs are destroyed at 0 HP, not knocked offline"
```

---

### Task 6: Full-suite and balance gate

**Files:** none modified unless a gate fails.

- [ ] **Step 1: Run the balance regression gate**

```bash
cargo test -p feral-processes-engine balance_sim
```

Expected: PASS with no curve moving. `balance_sim.rs` is an RNG-free battle simulator asserting hardcoded empirical level curves; nothing in this change touches the damage maths, so a moved curve means something unintended happened. If one moves, stop and investigate rather than updating the constant.

- [ ] **Step 2: Run the full suite**

```bash
cargo test --workspace
```

Expected: PASS. Baseline was 681; this plan adds 8 tests (4 in Task 1, 2 in Task 2, 1 in Task 3, 1 in Task 4) and strengthens 1 existing one, so expect **689 passing**. A different total means a test was lost or an extra one crept in — reconcile before continuing.

- [ ] **Step 3: Lint clean**

```bash
cargo fmt --check
cargo clippy --workspace
```

Expected: no diff from `fmt`, no warnings from `clippy`.

- [ ] **Step 4: Confirm the pre-existing asset edits are still untouched**

```bash
git status --short
```

Expected: the four `assets/abilities/*.ron` files still showing as modified-but-unstaged, and nothing else. They were in the tree before this work and are not part of it.

- [ ] **Step 5: Hand back for visual sign-off**

Do not merge. Report to the user that the engine and doc work is complete and verified, and that the red-bar treatment in the battle pane and the party menu needs their eyes:

```bash
cargo run -p feral-processes
```

Standing policy in this repo is that drawing changes are verified by unit test, with final visual sign-off by the user. This is that hand-off point.
