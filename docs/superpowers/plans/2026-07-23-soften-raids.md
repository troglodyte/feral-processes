# Soften Raids Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retune raid damage, frequency, and structure regeneration so raids stop destroying structures through unavoidable attrition.

**Architecture:** Pure balance change — three `const` values in `crates/engine/src/lib.rs`, one value in `assets/structures/shield.ron`, and the schema doc that cites it. No new mechanics, no new types, no renderer or save-format changes. A preparatory task first hardens six seed-hunting tests that would otherwise become flaky at the lower raid roll.

**Tech Stack:** Rust, `bevy_ecs` (standalone), RON assets, built-in `#[test]` harness.

**Spec:** `docs/superpowers/specs/2026-07-23-soften-raids-design.md`

## Global Constraints

- Run `cargo fmt` and `cargo clippy --workspace` after every change; fix warnings rather than silencing them.
- `cargo test --workspace` is the final gate for every task. Passing only the tests you wrote is not evidence of correctness.
- No flaky tests: no `sleep()`, no wall-clock dependence, no unseeded RNG.
- Comments explain *why*, never *what*. Do not write a justification you have not verified.
- Never commit unless the task says to. Never push.
- If many tests fail at once with `NotFound` on an assets path, that is stale build artifacts from an old directory rename, not real failures. Fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core` (NOT a full `cargo clean` — `target/` is ~4 GB).
- The working tree has a pre-existing uncommitted change in `crates/engine/src/lib.rs` (a `battle_view_integrity_matches_the_map_status_panel` test) and an untracked file `docs/superpowers/plans/2026-07-23-non-raidable-structures-and-slot-labels.md`. Both are unrelated. Do not commit, revert, or modify either — stage files by explicit path only, never `git add -A` or `git commit -a`.

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `crates/engine/src/lib.rs` (test module) | Six seed-hunting raid tests hardened against a lower roll | 1 |
| `crates/engine/src/lib.rs:209-247` | `RAID_DAMAGE`, `RAID_CHANCE_PER_TICK`, `STRUCTURE_REGEN_AMOUNT` | 2 |
| `crates/engine/src/lib.rs` (test module) | Three new regression tests pinning the retuned behaviour | 2 |
| `assets/structures/shield.ron` | `raid_defense: 4` → `2` | 2 |
| `assets/structures/README.md:96-103` | Schema doc citing the Shield's `raid_defense` | 2 |

---

### Task 1: Harden the seed-hunting raid tests against a lower raid roll

Six tests sweep 300 seeds, call `raid_check()` exactly once per seed, and panic if a raid never fired. `RAID_CHANCE_PER_TICK` drops from 0.02 to 0.012 in Task 2, taking the odds of an all-miss sweep from ~0.23% to ~2.7%. These are seeded and nominally deterministic, but unsorted habitat lookup can shift RNG consumption between runs, making an all-miss sweep a live flake rather than a stable pass.

The fix gives each seed up to 7 `raid_check()` attempts, checking the success condition after **each** call and returning on the first fire.

**Detecting on the first fire is mandatory, not stylistic.** Two of these tests assert exact post-raid values that only hold after exactly one raid:
- `deployed_shields_reduce_raid_damage_to_an_undefended_structure` asserts `hp == 30 - (RAID_DAMAGE - shield_defense)`
- `raid_check_defended_by_a_worker_reduces_structure_damage_and_hurts_the_worker` asserts `worker_hp == 50 - RAID_DEFENDER_DAMAGE`

A loop that ran all 7 attempts and then checked would see accumulated damage and fail. Because every loop returns on first fire, no structure or worker ever takes a second hit, so this change is safe at both the current and the retuned constants.

This task is complete on its own: the suite must be green at the **current** constants when it lands. Nothing here changes game behaviour.

**Files:**
- Modify: `crates/engine/src/lib.rs` — six tests at (current lines) 10439, 10472, 10517, 10630, 10686, 10926

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `RAID_ATTEMPTS_PER_SEED: u32` — a constant in the `mod tests` module, consumed by no later task but referenced by all six rewritten tests.

- [ ] **Step 1: Add the shared attempt-count constant to the test module**

Place this immediately above `fn raid_check_can_damage_an_undefended_structure()` (currently line 10437, just after the `#[test]` attribute block of the preceding test — put it *before* the `#[test]` line so it is a module item, not an attribute target):

```rust
/// How many `raid_check` rolls each seed gets in the sweeps below.
/// `RAID_CHANCE_PER_TICK` is a per-call roll, so a single call per seed
/// leaves a ~2.7% chance of a 300-seed sweep never firing at all — which
/// unsorted habitat lookup can turn from a stable pass into a flake by
/// shifting RNG consumption between runs. Seven attempts takes that to
/// ~1e-11. Every sweep returns on the first fire, so no target ever takes
/// a second hit.
const RAID_ATTEMPTS_PER_SEED: u32 = 7;
```

- [ ] **Step 2: Rewrite `raid_check_can_damage_an_undefended_structure`**

Replace the whole function body. Note the `continue` in the inner loop and that the `return` fires as soon as damage is seen:

```rust
    #[test]
    fn raid_check_can_damage_an_undefended_structure() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let Some(durability) = game.world.get::<Durability>(structure) else {
                    // Destroyed outright — tolerate rather than assume it can't happen.
                    return;
                };
                if durability.hp < 30 {
                    return;
                }
            }
        }
        panic!(
            "raid_check never damaged the structure across 300 seeds — the raid roll may be broken"
        );
    }
```

- [ ] **Step 3: Rewrite `raid_damage_message_is_tagged_message_kind_raid`**

```rust
    #[test]
    fn raid_damage_message_is_tagged_message_kind_raid() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.world.spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ));

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let tagged = game
                    .message_log(10)
                    .into_iter()
                    .any(|(kind, _)| kind == MessageKind::Raid);
                if tagged {
                    return;
                }
            }
        }
        panic!(
            "raid_check never logged a MessageKind::Raid line across 300 seeds — the raid roll may be broken"
        );
    }
```

- [ ] **Step 4: Rewrite `deployed_shields_reduce_raid_damage_to_an_undefended_structure`**

The exact-value assertion is why the check sits inside the attempt loop:

```rust
    #[test]
    fn deployed_shields_reduce_raid_damage_to_an_undefended_structure() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let shield_defense = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == "shield")
                .unwrap()
                .raid_defense;
            game.world.spawn((
                Structure {
                    kind: "shield".to_string(),
                },
                Position { x: 1, y: 1 },
            ));
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let Some(durability) = game.world.get::<Durability>(structure) else {
                    return;
                };
                if durability.hp < 30 {
                    assert_eq!(
                        durability.hp,
                        30 - (RAID_DAMAGE - shield_defense),
                        "a raid on an undefended structure should be reduced by the deployed shield's raid_defense"
                    );
                    return;
                }
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }
```

- [ ] **Step 5: Rewrite `a_raid_fully_absorbed_by_the_shield_network_queues_a_deflected_effect`**

`take_effects()` drains the queue, so calling it once per attempt is correct:

```rust
    #[test]
    fn a_raid_fully_absorbed_by_the_shield_network_queues_a_deflected_effect() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            // Enough shields that RAID_DAMAGE is reduced to zero.
            let shield_defense = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == "shield")
                .unwrap()
                .raid_defense
                .max(1);
            let needed = RAID_DAMAGE.div_ceil(shield_defense);
            for _ in 0..needed {
                game.world.spawn((
                    Structure {
                        kind: "shield".to_string(),
                    },
                    Position { x: 1, y: 1 },
                ));
            }
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let effects = game.take_effects();
                if effects.is_empty() {
                    continue;
                }
                let target = effects
                    .iter()
                    .find(|e| e.pos == (5, 5))
                    .expect("the raid should have targeted the only durable structure");
                assert_eq!(
                    target.kind,
                    EffectKind::Deflected,
                    "a raid the shield network zeroes out should deflect, not hit"
                );
                assert_eq!(
                    game.world.get::<Durability>(structure).unwrap().hp,
                    30,
                    "a deflected raid should leave durability untouched"
                );
                return;
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }
```

- [ ] **Step 6: Rewrite `a_raid_fended_off_by_a_cronjob_worker_queues_a_deflected_effect`**

```rust
    #[test]
    fn a_raid_fended_off_by_a_cronjob_worker_queues_a_deflected_effect() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();
            // Defense far above RAID_DAMAGE, so the worker fully mitigates.
            game.world.spawn((
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 1,
                    def: 500,
                },
                Position { x: 5, y: 5 },
                Task {
                    kind: TaskKind::Guard,
                    target: structure,
                    progress: 0,
                    required: 10,
                },
            ));

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let effects = game.take_effects();
                if effects.is_empty() {
                    continue;
                }
                assert_eq!(effects[0].kind, EffectKind::Deflected);
                assert_eq!(effects[0].pos, (5, 5));
                assert_eq!(
                    game.world.get::<Durability>(structure).unwrap().hp,
                    30,
                    "a fully mitigated raid should leave durability untouched"
                );
                return;
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }
```

- [ ] **Step 7: Rewrite `raid_check_defended_by_a_worker_reduces_structure_damage_and_hurts_the_worker`**

The worker takes `RAID_DEFENDER_DAMAGE` per raid, so the exact-HP assertion depends on returning at the first fire:

```rust
    #[test]
    fn raid_check_defended_by_a_worker_reduces_structure_damage_and_hurts_the_worker() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();
            let worker = spawn_tamed(&mut game, 50, 3);
            game.world.get_mut::<Stats>(worker).unwrap().def = 100; // fully mitigates RAID_DAMAGE
            game.world.entity_mut(worker).insert(Task {
                kind: TaskKind::GatherResource,
                target: structure,
                progress: 0,
                required: 5,
            });

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let worker_hp = game.world.get::<Stats>(worker).unwrap().hp;
                if worker_hp < 50 {
                    // The raid rolled this attempt: the structure should be
                    // untouched (fully mitigated) and the worker should have
                    // taken the defender's cost.
                    assert_eq!(
                        game.world.get::<Durability>(structure).unwrap().hp,
                        30,
                        "a worker with overwhelming Defense should fully mitigate the raid"
                    );
                    assert_eq!(worker_hp, 50 - RAID_DEFENDER_DAMAGE);
                    return;
                }
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }
```

- [ ] **Step 8: Run the six tests at the current, unchanged constants**

```bash
cargo test -p feral-processes-engine raid 2>&1 | tail -20
```

Expected: all raid tests PASS. Game behaviour is unchanged in this task, so a failure here means the rewrite broke a test, not that balance shifted.

- [ ] **Step 9: Format, lint, and run the full suite**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -20 && cargo test --workspace 2>&1 | tail -20
```

Expected: no clippy warnings; all tests pass.

- [ ] **Step 10: Commit**

```bash
git add crates/engine/src/lib.rs
git commit -m "Give raid seed sweeps multiple rolls per seed

A single raid_check per seed leaves a 300-seed sweep with a ~2.7% chance of
never firing once RAID_CHANCE_PER_TICK drops, and unsorted habitat lookup can
shift RNG consumption between runs. Each seed now gets seven rolls, checked
after each call so the exact-value assertions still see a single raid."
```

Note: `crates/engine/src/lib.rs` also contains the pre-existing uncommitted `battle_view_integrity_matches_the_map_status_panel` test. It will be swept into this commit because it is in the same file. That is acceptable — it is a passing test. Do **not** attempt to split it out with `git add -p` unless the user asks.

---

### Task 2: Retune the raid dials

Change the three constants, drop the Shield's `raid_defense` to match, and update the schema doc. The Shield change is not separable: at `RAID_DAMAGE: 4` and `raid_defense: 4`, a single Shield reduces raid damage to zero, which both breaks `deployed_shields_reduce_raid_damage_to_an_undefended_structure` and turns the Shield ramp back into an all-or-nothing cliff.

**Files:**
- Modify: `crates/engine/src/lib.rs:211` (`RAID_CHANCE_PER_TICK`), `:215` (`RAID_DAMAGE`), `:247` (`STRUCTURE_REGEN_AMOUNT`)
- Modify: `crates/engine/src/lib.rs` (test module) — three new tests
- Modify: `assets/structures/shield.ron:8`
- Modify: `assets/structures/README.md:102-103`

**Interfaces:**
- Consumes: `RAID_ATTEMPTS_PER_SEED` from Task 1 (already in the test module; the new tests below do not use it — they call `damage_structure` directly rather than hunting for a roll).
- Produces: nothing consumed by a later task.

- [ ] **Step 1: Write the three new tests**

Add all three at the end of the raid test group, immediately after `raid_check_defended_by_a_worker_reduces_structure_damage_and_hurts_the_worker`. These call `damage_structure` and `structure_regen` directly instead of hunting for a roll, so they are deterministic — no seed sweep needed.

```rust
    /// Raids should be survivable attrition, not a countdown. Eight hits to
    /// destroy a default-durability structure is the property; the exact
    /// constants are free to move underneath it.
    #[test]
    fn a_structure_survives_seven_raids_worth_of_damage() {
        let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let durability = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "mining_node")
            .expect("mining_node.ron should load")
            .durability;
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability {
                    hp: durability,
                    max_hp: durability,
                },
            ))
            .id();

        for _ in 0..7 {
            game.damage_structure(structure, RAID_DAMAGE, "Mining Node");
        }

        assert!(
            game.world.get::<Durability>(structure).is_some(),
            "seven raids should not destroy a structure at full durability"
        );
    }

    /// One regen interval has to fully undo one raid, or the base loses the
    /// attrition race no matter how the player plays.
    #[test]
    fn one_regen_interval_fully_undoes_one_raids_damage() {
        let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();

        game.damage_structure(structure, RAID_DAMAGE, "Mining Node");
        assert_eq!(
            game.world.get::<Durability>(structure).unwrap().hp,
            30 - RAID_DAMAGE,
            "the raid should have landed before regen is tested"
        );

        game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;
        game.structure_regen();

        assert_eq!(
            game.world.get::<Durability>(structure).unwrap().hp,
            30,
            "one regen interval should fully undo one raid's damage"
        );
    }

    /// The shield network should ramp, not cliff: the first Shield has to
    /// leave damage on the table, or `raid_defense` has drifted into
    /// granting total immunity for one build.
    #[test]
    fn a_single_shield_reduces_raid_damage_without_erasing_it() {
        let game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let shield_defense = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "shield")
            .expect("shield.ron should load")
            .raid_defense;

        assert!(
            shield_defense > 0,
            "a Shield that reduces nothing is not a Shield"
        );
        assert!(
            shield_defense < RAID_DAMAGE,
            "one Shield must not fully absorb a raid — the network should ramp, not cliff"
        );
    }
```

- [ ] **Step 2: Run the new tests and confirm the first two fail**

```bash
cargo test -p feral-processes-engine survives_seven_raids one_regen_interval single_shield 2>&1 | tail -25
```

Expected, at the current constants:
- `a_structure_survives_seven_raids_worth_of_damage` — **FAIL**: 7 × `RAID_DAMAGE` 10 = 70 destroys a 30-durability structure.
- `one_regen_interval_fully_undoes_one_raids_damage` — **FAIL**: `assertion left == right` with left `22`, right `30` (30 − 10 damage + 2 regen).
- `a_single_shield_reduces_raid_damage_without_erasing_it` — PASS (4 < 10 today). It goes red in Step 4 and is the reason the Shield change exists.

- [ ] **Step 3: Retune the three constants**

In `crates/engine/src/lib.rs`, replace these three declarations. Update the doc comments — the current `RAID_DEFENDER_DAMAGE` comment references `RAID_DAMAGE.saturating_sub(worker_def)`, which stays accurate and needs no edit.

Line 209-211:
```rust
/// Chance per tick (see `Game::raid_check`) that a random deployed
/// structure comes under raid, if any exist.
const RAID_CHANCE_PER_TICK: f64 = 0.012;
```

Line 213-215:
```rust
/// Damage a raid deals to a structure's `Durability` when it has no
/// assigned cronjob worker defending it. Deliberately small relative to
/// `structures::default_durability` (30): a raid is meant to be attrition
/// the base can recover from, not a three-hit countdown to losing the
/// structure outright.
const RAID_DAMAGE: u32 = 4;
```

Line 245-247:
```rust
/// How much `Durability` a damaged structure regenerates every
/// `STRUCTURE_REGEN_INTERVAL` ticks — set to match `RAID_DAMAGE` so one
/// interval fully undoes one raid. Below that, a base loses the attrition
/// race no matter how it's played.
const STRUCTURE_REGEN_AMOUNT: u32 = 4;
```

- [ ] **Step 4: Run the raid tests and confirm the Shield tests now fail**

```bash
cargo test -p feral-processes-engine raid 2>&1 | tail -30
```

Expected:
- `a_structure_survives_seven_raids_worth_of_damage` — now PASS (7 × 4 = 28 < 30).
- `one_regen_interval_fully_undoes_one_raids_damage` — now PASS (30 − 4 + 4 = 30).
- `a_single_shield_reduces_raid_damage_without_erasing_it` — now **FAIL**: `shield_defense < RAID_DAMAGE` is `4 < 4`.
- `deployed_shields_reduce_raid_damage_to_an_undefended_structure` — now **FAIL** with the 300-seed panic: one Shield zeroes a 4-damage raid, so `durability.hp < 30` never becomes true and the sweep exhausts.

- [ ] **Step 5: Drop the Shield's raid_defense**

`assets/structures/shield.ron` in full:

```ron
(
    id: "shield",
    name: "Shield",
    glyph: '^',
    color: Red,
    build_cost: [("core_fragment", 16)],
    work: None,
    raid_defense: 2,
)
```

- [ ] **Step 6: Run the raid tests and confirm all pass**

```bash
cargo test -p feral-processes-engine raid 2>&1 | tail -30
```

Expected: all PASS. `deployed_shields_reduce_raid_damage_to_an_undefended_structure` now sees `30 - (4 - 2)` = 28, and `a_raid_fully_absorbed_by_the_shield_network_queues_a_deflected_effect` computes `needed = 4.div_ceil(2)` = 2 shields for full absorption.

- [ ] **Step 7: Update the structure schema doc**

`assets/structures/README.md` cites the shipped Shield's value twice, at lines 96-103. CLAUDE.md requires the schema doc to move with the value in the same change.

Replace exactly this block (lines 96-103):

```
    // Optional; can be left out entirely (defaults to 0). Flat raid-damage
    // reduction this structure contributes to *every* raid, against *any*
    // deployed structure, for as long as it's standing — not just itself,
    // and it stacks additively across every deployed structure that sets
    // this (e.g. several Shields). Applied before an assigned worker/guard's
    // own Defense-based mitigation, so the two stack. This is how the
    // Shield structure works: `raid_defense: 4` with no `work` recipe.
    raid_defense: 4,
```

with:

```
    // Optional; can be left out entirely (defaults to 0). Flat raid-damage
    // reduction this structure contributes to *every* raid, against *any*
    // deployed structure, for as long as it's standing — not just itself,
    // and it stacks additively across every deployed structure that sets
    // this (e.g. several Shields). Applied before an assigned worker/guard's
    // own Defense-based mitigation, so the two stack. This is how the
    // Shield structure works: `raid_defense: 2` with no `work` recipe — one
    // Shield halves an ordinary raid, two absorb it entirely.
    raid_defense: 2,
```

Nothing else in this file needs to change: line 77 still correctly says `durability` defaults to 30, and line 82's "Damaged structures slowly regenerate over time regardless" cites no number.

- [ ] **Step 8: Format, lint, and run the full suite**

```bash
cargo fmt && cargo clippy --workspace 2>&1 | tail -20 && cargo test --workspace 2>&1 | tail -20
```

Expected: no clippy warnings; all tests pass. Three groups worth watching:

- `structure_regen_heals_damaged_structures_over_time` and `structure_regen_does_not_exceed_max_durability` — both should still pass (the first is symbolic in `STRUCTURE_REGEN_AMOUNT`; the second clamps 29 + 4 to 30).
- Nest tests. `structure_regen` queries *every* `Durability` holder, so Nests now regenerate 4 per 20 ticks rather than 2 — an accepted side effect, per the spec. `bumping_a_nest_damages_it_and_destroying_it_frees_its_guardians`, `bumping_a_nest_with_high_hp_damages_it_without_destroying_it`, and `raid_check_never_targets_a_nest_even_as_the_only_durability_holder` all act without advancing the clock through a regen interval, so none should be affected. If one of them fails, the side effect is larger than the spec assumed — stop and report rather than adjusting the test.
- `shield_structure_loads_with_no_work_and_a_raid_defense_bonus` asserts only `raid_defense > 0`, so it passes at 2.

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/lib.rs assets/structures/shield.ron assets/structures/README.md
git commit -m "Soften raids: less damage, less often, faster regen

Raids destroyed structures through unavoidable attrition — 10 damage every
~50 ticks against 30 durability, healing back at 2 per 20 ticks. A raid now
does 4 every ~83 ticks and one regen interval fully undoes it, so eight hits
are needed to destroy a structure instead of three.

Shield raid_defense drops 4 -> 2 to match: at 4 a single Shield would zero
out a raid entirely, turning the network from a ramp into a cliff."
```

---

## Manual verification

Automated tests cover the arithmetic but not the feel, which is the actual complaint. After Task 2, the balance is unvalidated by play — the same gap called out in the research-tree work. Offer the user a run:

```bash
cargo run -p feral-processes
```

What to look for: raids should read as an occasional nuisance that prompts a look at the base, not a countdown. If structures still feel fragile, `RAID_DAMAGE` and `RAID_CHANCE_PER_TICK` are the dials — they are plain constants and re-tuning is a one-line change plus a test run.
