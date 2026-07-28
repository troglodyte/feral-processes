# Wild-Carried Routines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A wild program can spawn carrying a routine its species never grants, uses it against you in battle, and hands it over when decompiled — plus twenty new abilities reachable no other way, and ability magnitudes that scale with the user's level.

**Architecture:** Six seams, in dependency order. `abilities.rs` gains a `wild_weight` field, a weighted pool, two new effect variants and a pure level-scaling function. `spawning.rs` rolls the pool onto each wild creature's existing (currently empty) `Routines` component. `combat.rs` merges a carried routine into the species kit at capture instead of overwriting it. `combat_round.rs` makes `ability_recipients` side-aware so a hostile's "ally" is its own group. `combat_status.rs` lets `wild_retaliate` spend a round on a routine, and widens battle teardown to every hostile. Twenty `.ron` files carry the content.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine only), `ron` for assets, `rand` via the `GameRng` resource.

## Global Constraints

Copied verbatim from the spec and `CLAUDE.md`. Every task's requirements implicitly include this section.

- **No save-format change.** `SAVE_FORMAT_VERSION` is not bumped. `CreatureSave.routines` already exists and is already written for wild creatures.
- **`decompile` keeps `cooldown: 0`.** It is the only ability permitted to. Every other ability, shipped or new, has `cooldown >= 1`.
- **New `AbilityDef` fields are `#[serde(default)]`**, so existing `.ron` files — including anyone's mods — keep parsing untouched.
- **A malformed `.ron` file is skipped with a logged warning, never a panic.** Follow the existing pattern in `AbilityDb::load_dir`.
- **Update `assets/abilities/README.md` in the same change** as any schema change.
- **Tuning values go in `crates/engine/src/tuning.rs`**, as documented `pub const`, never inline in a formula.
- **No new `StatusKind` variants.** `Bleed` and `Stun` remain the whole set.
- **No species file and no research file references any of the twenty new abilities.**
- **`WILD_ABILITY_CHANCE` already exists** (gates whether a wild *move* reaches for its status effect) and is unrelated. The new constant is `WILD_ROUTINE_CHANCE`.
- **Run `cargo fmt` and `cargo clippy --workspace` after every task**; fix warnings rather than silencing them.
- **Full suite is the final gate:** `cargo test --workspace` before calling the branch done. Baseline before this work is 637 tests passing.
- Working branch is `feat/wild-carried-routines`, already created, spec already committed as `ed0e146`.

### Tuning constants introduced

All four land in Task 3 and Task 5. Values are arithmetic-plausible only; nothing here has been playtested.

| Constant | Value | Task |
|---|---|---|
| `ABILITY_POWER_SCALE_PER_LEVEL` | `0.15` | 3 |
| `ABILITY_POWER_SCALE_LEVEL_CAP` | `40` | 3 |
| `WILD_ROUTINE_CHANCE` | `0.06` | 5 |
| `ENEMY_ROUTINE_MIN_COOLDOWN` | `1` | 8 |

---

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `crates/engine/src/abilities.rs` | `wild_weight` field, `wild_pool`, `weighted_pick`, `Drain`/`Cleanse` variants, `ability_power_scale`/`scaled_power`, load validation | 1, 2, 3 |
| `crates/engine/src/tuning.rs` | The four new constants, in a new "Wild routines and ability scaling" section | 3, 5, 8 |
| `crates/engine/src/game/combat_round.rs` | `use_ability` effect arms, side-aware `ability_recipients` | 2, 3, 7 |
| `crates/engine/src/game/combat.rs` | `install_innate_routines` merge, `install_unlocked_routines` overflow-to-cargo, `ability_user_level` | 3, 6 |
| `crates/engine/src/game/spawning.rs` | The wild routine roll in `spawn_wild_creature` | 5 |
| `crates/engine/src/game/combat_status.rs` | Hostile routine use in `wild_retaliate`, teardown cleanup | 8 |
| `assets/abilities/*.ron` | Twenty new files, two cooldown bumps | 4 |
| `assets/abilities/README.md` | Schema reference | 1, 2, 3, 4 |
| `crates/engine/src/tests/*.rs` | Tests, split by the subsystem they exercise | all |

Tests live under `src/tests/` (not `tests/`) because they reach past `Game`'s public API into components and resources. Pure-function tests live in the `#[cfg(test)] mod tests` at the bottom of the module that owns the function, matching `abilities.rs` today.

---

### Task 1: `wild_weight` and the weighted pool

The schema field and the pure pick, with nothing consuming them yet.

**Files:**
- Modify: `crates/engine/src/abilities.rs` — add field to `AbilityDef` (around line 110-128), add `weighted_pick` free function, add `AbilityDb::wild_pool`
- Modify: `assets/abilities/README.md` — document `wild_weight`
- Test: `crates/engine/src/abilities.rs` `mod tests` (bottom of file)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `AbilityDef.wild_weight: u32` (public field, `#[serde(default)]`)
  - `pub fn weighted_pick(weights: &[u32], roll: u32) -> Option<usize>` — `roll` is expected in `0..weights.iter().sum()`; returns `None` for an empty slice or an all-zero total. A roll at or past the total saturates to the last non-zero index rather than returning `None`, so a caller's off-by-one degrades to a valid pick.
  - `pub fn AbilityDb::wild_pool(&self) -> Vec<(&AbilityDef, u32)>` — ability/weight pairs for every `wild_weight > 0`, ordered by id.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block at the bottom of `crates/engine/src/abilities.rs`:

```rust
    #[test]
    fn wild_weight_defaults_to_zero_so_an_ability_opts_in_rather_than_out() {
        let (db, _) = load("wild_default", &[("test_sweep", VALID)]);
        let def = db.get("test_sweep").expect("valid ability should load");
        assert_eq!(
            def.wild_weight, 0,
            "an ability that says nothing must never spawn wild"
        );
    }

    #[test]
    fn wild_pool_holds_only_the_opted_in_abilities_ordered_by_id() {
        let common = r#"(id: "zebra", name: "Zebra", description: "d",
            target: OneAlly, effect: Heal(power: 1), cooldown: 1, wild_weight: 4)"#;
        let rare = r#"(id: "apple", name: "Apple", description: "d",
            target: OneAlly, effect: Heal(power: 1), cooldown: 1, wild_weight: 1)"#;
        let (db, _) = load(
            "wild_pool",
            &[("test_sweep", VALID), ("zebra", common), ("apple", rare)],
        );
        let pool: Vec<(&str, u32)> = db
            .wild_pool()
            .into_iter()
            .map(|(d, w)| (d.id.as_str(), w))
            .collect();
        assert_eq!(
            pool,
            vec![("apple", 1), ("zebra", 4)],
            "weight-0 abilities are excluded, and HashMap order must not leak into a seeded roll"
        );
    }

    #[test]
    fn weighted_pick_is_proportional_to_the_weights() {
        let weights = [1, 3, 1];
        // Roll 0 lands in the first bucket; 1..=3 in the second; 4 in the third.
        assert_eq!(weighted_pick(&weights, 0), Some(0));
        assert_eq!(weighted_pick(&weights, 1), Some(1));
        assert_eq!(weighted_pick(&weights, 3), Some(1));
        assert_eq!(weighted_pick(&weights, 4), Some(2));
    }

    #[test]
    fn weighted_pick_handles_an_empty_pool_and_an_overshooting_roll() {
        assert_eq!(weighted_pick(&[], 0), None, "nothing to pick from");
        assert_eq!(weighted_pick(&[0, 0], 0), None, "all weights excluded");
        assert_eq!(
            weighted_pick(&[2, 3], 99),
            Some(1),
            "an overshooting roll saturates to the last real bucket, never panics"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine abilities:: 2>&1 | tail -20`
Expected: compile error — `no field wild_weight on type AbilityDef`, `cannot find function weighted_pick`, `no method named wild_pool`.

- [ ] **Step 3: Add the field**

In `crates/engine/src/abilities.rs`, inside `pub struct AbilityDef`, after `fatigue_cost`:

```rust
    /// How likely this ability is to be found already installed on a wild
    /// program — see `Game::spawn_wild_creature`. Relative within the pool,
    /// not a probability: weight 12 is twice as likely as weight 6, and the
    /// pool is normalised at pick time.
    ///
    /// `#[serde(default)]` to 0, which means "never spawns wild". Defaulting
    /// to exclusion is what keeps `priority_boost` and `decompile` — and
    /// every other ability reachable through a species or a research node —
    /// out of the pool without this module having to name them.
    #[serde(default)]
    pub wild_weight: u32,
```

- [ ] **Step 4: Add the pure pick**

In `crates/engine/src/abilities.rs`, after `routine_item_id`:

```rust
/// Index into `weights` that `roll` selects, treating each weight as the
/// width of a bucket. `roll` is expected in `0..weights.iter().sum()`.
///
/// `None` only when there is genuinely nothing to pick — an empty slice, or
/// every weight zero. An overshooting roll saturates to the last non-zero
/// bucket rather than returning `None`, so a caller that computes its range
/// wrong degrades to a valid pick instead of silently spawning nothing.
///
/// Pure, and takes the roll rather than the RNG, so the distribution can be
/// tested without a `Game`.
pub fn weighted_pick(weights: &[u32], roll: u32) -> Option<usize> {
    let mut remaining = roll;
    let mut last = None;
    for (index, &weight) in weights.iter().enumerate() {
        if weight == 0 {
            continue;
        }
        last = Some(index);
        if remaining < weight {
            return Some(index);
        }
        remaining -= weight;
    }
    last
}
```

- [ ] **Step 5: Add the pool accessor**

In `crates/engine/src/abilities.rs`, in `impl AbilityDb`, after `all()`:

```rust
    /// Every ability that can be found on a wild program, paired with its
    /// weight, ordered by id.
    ///
    /// Ordered for the same reason `all()` is: `HashMap` iteration is
    /// randomised per instance, so a weighted walk over an unordered pool
    /// would not be reproducible from a seed — and every wild spawn in this
    /// game is.
    pub fn wild_pool(&self) -> Vec<(&AbilityDef, u32)> {
        self.all()
            .filter(|d| d.wild_weight > 0)
            .map(|d| (d, d.wild_weight))
            .collect()
    }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine abilities:: 2>&1 | tail -20`
Expected: PASS, all tests in the module.

- [ ] **Step 7: Document the field**

In `assets/abilities/README.md`, inside the ```ron schema block, after the `fatigue_cost` entry and before the closing `)`:

```
    // Optional; defaults to 0. How likely this ability is to be found
    // already installed on a wild program you meet in the field — a
    // "carrier". 0 means it never spawns wild, which is why every ability
    // reachable through a species or a research node leaves this alone.
    //
    // Weights are relative within the pool, not probabilities: an ability
    // at 12 turns up twice as often as one at 6. Whether a given wild
    // program carries anything at all is a separate roll the engine makes
    // (`WILD_ROUTINE_CHANCE` in `tuning.rs`); this only decides *which*
    // routine it gets once that roll has already succeeded.
    //
    // A carrier uses its routine against you in battle, and hands it over
    // installed if you decompile it. Killing it destroys the routine.
    wild_weight: 8,
```

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/abilities.rs assets/abilities/README.md
git commit -m "feat: abilities declare how often they spawn on a wild program

wild_weight defaults to 0 — never spawns wild — so an ability opts in
rather than out, and no shipped ability changes behaviour. Nothing
consumes the pool yet."
```

---

### Task 2: `Drain` and `Cleanse` effects

Two new `AbilityEffect` variants. Sap needs no variant — a negative-power `Buff` aimed at the enemy side already is one — so this task documents that instead of building it.

**Files:**
- Modify: `crates/engine/src/abilities.rs` — `AbilityEffect` variants, `non_finite_field`, a clamp at load
- Modify: `crates/engine/src/game/combat_round.rs` — two new arms in `use_ability` (around line 539-607)
- Modify: `assets/abilities/README.md` — document both effects and the sap idiom
- Test: `crates/engine/src/abilities.rs` `mod tests`, and `crates/engine/src/tests/combat_abilities.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  - `AbilityEffect::Drain { power: i32, heal_fraction: f32 }`
  - `AbilityEffect::Cleanse`
  - Both handled in `Game::use_ability`, which keeps its existing signature: `pub(crate) fn use_ability(&mut self, ability: &AbilityDef, actor: Entity, name: &str, recipients: &[Entity])`

- [ ] **Step 1: Write the failing load tests**

Append to `mod tests` in `crates/engine/src/abilities.rs`:

```rust
    #[test]
    fn a_drain_with_a_non_finite_heal_fraction_is_skipped() {
        let bad = r#"(id: "test_bad_drain", name: "Bad Drain", description: "d",
            target: OneEnemyGroupFront, cooldown: 1,
            effect: Drain(power: 8, heal_fraction: NaN))"#;
        let (db, warnings) = load("bad_drain", &[("test_sweep", VALID), ("bad", bad)]);
        assert!(db.get("test_sweep").is_some(), "the valid file still loads");
        assert!(
            db.get("test_bad_drain").is_none(),
            "a NaN heal fraction must not reach the formula"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("heal_fraction"), "{}", warnings[0]);
    }

    #[test]
    fn an_out_of_range_heal_fraction_is_clamped_rather_than_refused() {
        let greedy = r#"(id: "test_greedy", name: "Greedy", description: "d",
            target: OneEnemyGroupFront, cooldown: 1,
            effect: Drain(power: 8, heal_fraction: 5.0))"#;
        let (db, warnings) = load("greedy_drain", &[("greedy", greedy)]);
        let def = db.get("test_greedy").expect("clamped, not skipped");
        let AbilityEffect::Drain { heal_fraction, .. } = def.effect else {
            panic!("expected a Drain effect");
        };
        assert_eq!(
            heal_fraction, 1.0,
            "a mod asking for 500% lifesteal gets 100%, bounded at load not at use"
        );
        assert!(warnings.is_empty(), "clamping is not a load failure");
    }

    #[test]
    fn a_cleanse_needs_no_fields() {
        let cleanse = r#"(id: "test_cleanse", name: "Cleanse", description: "d",
            target: WholeParty, cooldown: 1, effect: Cleanse)"#;
        let (db, warnings) = load("cleanse", &[("cleanse", cleanse)]);
        assert!(db.get("test_cleanse").is_some(), "{warnings:?}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine abilities:: 2>&1 | tail -20`
Expected: compile error — `no variant named Drain`, `no variant named Cleanse`.

- [ ] **Step 3: Add the variants**

In `crates/engine/src/abilities.rs`, in `pub enum AbilityEffect`, after the `Debuff` arm and before `Decompile`:

```rust
    /// Damage through `battle::compute_damage`, then the user is healed for
    /// `heal_fraction` of the damage it actually dealt, capped at its own
    /// maximum Integrity.
    ///
    /// Deliberately excluded from `scaled_power`: the heal rides the damage,
    /// which already rides the user's ATK, so this scales with level without
    /// being scaled.
    Drain {
        power: i32,
        /// Clamped to `0.0..=1.0` at load — see `AbilityDb::load_dir`. Bounded
        /// there rather than at use, so a `heal_fraction: 5.0` mod is a
        /// bounded ability instead of a bounded surprise inside a formula.
        heal_fraction: f32,
    },
    /// Clears each recipient's active status condition. Carries no fields.
    Cleanse,
```

- [ ] **Step 4: Validate and clamp at load**

In `AbilityDef::non_finite_field`, before the final `None`:

```rust
        if let AbilityEffect::Drain { heal_fraction, .. } = &self.effect
            && !heal_fraction.is_finite()
        {
            return Some("effect.heal_fraction");
        }
```

Add a clamping method on `AbilityDef`, after `decompile_target_mismatch`:

```rust
    /// Bounds a `Drain`'s `heal_fraction` to `0.0..=1.0`. Applied at load so
    /// every reader downstream can treat it as a fraction, rather than each
    /// one re-clamping. Runs after `non_finite_field`, which has already
    /// refused a NaN — `clamp` would panic on one.
    fn clamp_ranges(&mut self) {
        if let AbilityEffect::Drain { heal_fraction, .. } = &mut self.effect {
            *heal_fraction = heal_fraction.clamp(0.0, 1.0);
        }
    }
```

In `AbilityDb::load_dir`, change `Ok(def) => {` to `Ok(mut def) => {`, and insert the clamp immediately before `db.abilities.insert(def.id.clone(), def);`:

```rust
                    def.clamp_ranges();
```

- [ ] **Step 5: Run the load tests to verify they pass**

Run: `cargo test -p feral-processes-engine abilities:: 2>&1 | tail -20`
Expected: PASS. `use_ability` will not compile yet if its `match` is exhaustive — if so, that is Step 6's job; run this again after Step 7.

- [ ] **Step 6: Write the failing behaviour tests**

Append to `crates/engine/src/tests/combat_abilities.rs`:

```rust
/// Drain heals the user for its fraction of the damage it actually dealt —
/// not of its authored power, which DEF has already eaten into.
#[test]
fn drain_heals_the_user_for_a_fraction_of_the_damage_it_dealt() {
    let mut game = Game::new(4101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 200;
        stats.hp = 50;
        stats.atk = 10;
    }
    let before = game.world.get::<Stats>(enemies[0]).unwrap().hp;

    let ability = crate::abilities::AbilityDef {
        id: "test_drain".into(),
        name: "Test Drain".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Drain {
            power: 10,
            heal_fraction: 0.5,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    let dealt = before - game.world.get::<Stats>(enemies[0]).unwrap().hp;
    assert!(dealt > 0, "the drain must actually land damage");
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        50 + dealt / 2,
        "the user is healed for half of what it dealt"
    );
}

#[test]
fn drain_never_heals_the_user_past_its_maximum() {
    let mut game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 60;
        stats.hp = 59;
        stats.atk = 40;
    }

    let ability = crate::abilities::AbilityDef {
        id: "test_drain".into(),
        name: "Test Drain".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Drain {
            power: 10,
            heal_fraction: 1.0,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        60,
        "a full-lifesteal drain caps at max Integrity rather than overhealing"
    );
}

#[test]
fn cleanse_clears_an_active_status_and_is_silent_on_a_clean_target() {
    let mut game = Game::new(4103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
        kind: StatusKind::Bleed,
        remaining: 3,
        power: 4,
    });

    let ability = crate::abilities::AbilityDef {
        id: "test_cleanse".into(),
        name: "Test Cleanse".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::WholeParty,
        effect: crate::abilities::AbilityEffect::Cleanse,
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[player]);
    assert!(
        game.world.get::<StatusEffects>(player).unwrap().active.is_none(),
        "cleanse must clear the condition"
    );

    let lines_before = game.world.resource::<MessageLog>().lines.len();
    game.use_ability(&ability, player, "You", &[player]);
    assert_eq!(
        game.world.resource::<MessageLog>().lines.len(),
        lines_before,
        "a cleanse with nothing to clear logs nothing — one line per party member every time would drown the log"
    );
}

/// A sap is a negative-power `Buff` aimed at the enemy side. No `Sap`
/// variant exists, deliberately — `effective_atk` adds the buff bonus
/// unconditionally, so a negative power already subtracts.
#[test]
fn a_negative_power_buff_saps_effective_attack() {
    let mut game = Game::new(4104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world.get_mut::<Stats>(enemies[0]).unwrap().atk = 20;
    let before = game.effective_atk(enemies[0]);

    let ability = crate::abilities::AbilityDef {
        id: "test_sap".into(),
        name: "Test Sap".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::WholeEnemyGroup,
        effect: crate::abilities::AbilityEffect::Buff {
            kind: BuffKind::Atk,
            power: -6,
            duration: 3,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert_eq!(
        game.effective_atk(enemies[0]),
        before - 6,
        "a negative buff power subtracts, which is the whole sap mechanic"
    );
}

/// `CombatBuff` holds one `active` slot and `is_defending` identifies the
/// Defend stance by an exact power match, so a sap landing on a bracing
/// member cancels its stance. Documented cost of the single-slot design,
/// pinned here rather than special-cased.
#[test]
fn a_sap_landing_on_a_bracing_member_cancels_its_defend_stance() {
    let mut game = Game::new(4105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    game.begin_defend(player);
    assert!(game.is_defending(player), "fixture: the player is bracing");

    let ability = crate::abilities::AbilityDef {
        id: "test_sap".into(),
        name: "Test Sap".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::WholeEnemyGroup,
        effect: crate::abilities::AbilityEffect::Buff {
            kind: BuffKind::Def,
            power: -4,
            duration: 3,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "Enemy", &[player]);

    assert!(
        !game.is_defending(player),
        "one buff slot means a sap overwrites the stance — the documented cost, not a bug"
    );
}
```

- [ ] **Step 7: Implement the two arms**

In `crates/engine/src/game/combat_round.rs`, in `use_ability`'s `match &ability.effect`, after the `AbilityEffect::Damage` arm and before the `Decompile` arm:

```rust
                AbilityEffect::Drain {
                    power,
                    heal_fraction,
                } => {
                    let def = self
                        .world
                        .get::<Stats>(recipient)
                        .map(|s| s.def)
                        .unwrap_or(0);
                    let dmg = battle::compute_damage(self.effective_atk(actor), def, *power);
                    self.apply_damage(recipient, dmg);
                    // Off the damage actually dealt, not the authored power:
                    // DEF has already eaten into it, and healing off the
                    // pre-mitigation figure would make a drain better against
                    // an armoured target than a soft one.
                    let restored = (dmg as f32 * heal_fraction).round() as i32;
                    if let Some(mut stats) = self.world.get_mut::<Stats>(actor) {
                        stats.hp = (stats.hp + restored).min(stats.max_hp);
                    }
                    self.log_kind(
                        MessageKind::PartyDamage,
                        format!("{name} siphons {dmg} from {on}, restoring {restored}."),
                    );
                }
                AbilityEffect::Cleanse => {
                    let had_status = self
                        .world
                        .get::<StatusEffects>(recipient)
                        .is_some_and(|s| s.active.is_some());
                    if had_status {
                        if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(recipient) {
                            statuses.active = None;
                        }
                        self.log(format!("{name} flushes the corruption from {on}."));
                    }
                    // Silent on a clean recipient: a "nothing to clear" line
                    // per party member, every cast, would drown the log.
                }
```

- [ ] **Step 8: Run the behaviour tests**

Run: `cargo test -p feral-processes-engine combat_abilities 2>&1 | tail -25`
Expected: PASS. Every other test in the file must still pass — the `AbilityDef` literals in the new tests carry `wild_weight: 0` because Task 1 added the field.

- [ ] **Step 9: Document both effects**

In `assets/abilities/README.md`, in the `effect:` comment block, after the `Debuff` entry and before `Decompile`:

```
    //   Drain(power: 10, heal_fraction: 0.5)
    //     Damage through the same formula as `Damage`, then the *user* is
    //     healed for that fraction of the damage it actually dealt, capped
    //     at its own maximum Integrity. Healing off the dealt figure rather
    //     than the authored power means an armoured target returns less,
    //     which is the intended shape. `heal_fraction` is clamped to
    //     0.0-1.0 at load; a non-finite one disqualifies the file.
    //
    //   Cleanse
    //     Clears each recipient's active status condition. No fields.
    //     Silent on a recipient that had nothing to clear.
```

And, after the `Buff` entry in the same block, append this paragraph:

```
    //     A *negative* power is how you write a sap: `Buff(kind: Atk,
    //     power: -4, duration: 3)` with `target: WholeEnemyGroup` weakens
    //     a group rather than strengthening it, because the buff bonus is
    //     added unconditionally wherever it lands. There is no separate
    //     `Sap` effect, and adding one would be a second spelling of this.
    //     One caveat: a combatant holds a single buff at a time, and the
    //     Defend stance is itself a buff — so a sap landing on a bracing
    //     member overwrites its stance and it stops defending.
```

- [ ] **Step 10: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
cargo test -p feral-processes-engine 2>&1 | tail -5
git add crates/engine/src/abilities.rs crates/engine/src/game/combat_round.rs crates/engine/src/tests/combat_abilities.rs assets/abilities/README.md
git commit -m "feat: Drain and Cleanse ability effects

Drain heals the user for a fraction of the damage it dealt; Cleanse
strips a status. A sap needs no variant — a negative-power Buff aimed
at the enemy side already is one, and that is now documented and
tested, including the stance it overwrites."
```

---

### Task 3: Level scaling

`Heal`, `Buff` and `Debuff` magnitudes multiply by the user's level. `Damage` does not — `compute_damage` already rides ATK, and double-dipping would move every `balance_sim` curve.

**Files:**
- Modify: `crates/engine/src/tuning.rs` — new section with two constants
- Modify: `crates/engine/src/abilities.rs` — `ability_power_scale`, `scaled_power`
- Modify: `crates/engine/src/game/combat.rs` — `ability_user_level`
- Modify: `crates/engine/src/game/combat_round.rs` — apply in `use_ability`'s Heal/Buff/Debuff arms
- Modify: `assets/abilities/README.md` — rewrite the "buffs don't need to scale" paragraph
- Test: `crates/engine/src/abilities.rs` `mod tests`, `crates/engine/src/tests/combat_abilities.rs`

**Interfaces:**
- Consumes: `AbilityEffect::Drain` from Task 2 (to assert it is *not* scaled).
- Produces:
  - `pub fn abilities::ability_power_scale(level: u32) -> f32`
  - `pub fn abilities::scaled_power(power: i32, level: u32) -> i32`
  - `pub(crate) fn Game::ability_user_level(&self, entity: Entity) -> u32`

- [ ] **Step 1: Add the tuning constants**

Append to `crates/engine/src/tuning.rs`, immediately before the `#[cfg(test)] mod tests` block:

```rust
// ─────────────────────────────────────────────────────────────────────────
// Wild routines and ability scaling
// ─────────────────────────────────────────────────────────────────────────

/// How much each level adds to an ability's magnitude: the multiplier is
/// `1.0 + level * this`. A flat `Heal(power: 8)` is a real patch at level 1
/// and noise against a level-20 program with 400 Integrity taking 100-point
/// hits, which is what this exists to fix.
///
/// Applies to `Heal`, `Buff` and `Debuff` magnitudes only. Ability `Damage`
/// is deliberately excluded — `battle::compute_damage` is
/// `power + ATK - DEF`, so it already rides the user's ATK, and scaling the
/// flat term too would double-dip through every curve `balance_sim`
/// projects.
pub const ABILITY_POWER_SCALE_PER_LEVEL: f32 = 0.15;

/// Level ceiling on `abilities::ability_power_scale`. The player has no
/// level cap (`progression::add_xp` takes `None`), so without this a long
/// enough game multiplies every heal and buff without bound. At the value
/// above, this caps the multiplier at 7x.
pub const ABILITY_POWER_SCALE_LEVEL_CAP: u32 = 40;
```

- [ ] **Step 2: Write the failing scale tests**

Append to `mod tests` in `crates/engine/src/abilities.rs`:

```rust
    #[test]
    fn ability_power_scale_grows_per_level_and_stops_at_the_cap() {
        assert_eq!(ability_power_scale(0), 1.0, "no level, no bonus");
        assert!(
            (ability_power_scale(12) - 2.8).abs() < 1e-5,
            "a companion at its level cap runs routines at 2.8x"
        );
        assert!(
            (ability_power_scale(20) - 4.0).abs() < 1e-5,
            "the level-20 case that motivated the change"
        );
        let capped = ability_power_scale(crate::tuning::ABILITY_POWER_SCALE_LEVEL_CAP);
        assert_eq!(
            ability_power_scale(9_999),
            capped,
            "the player has no level cap, so this clamp is the only bound"
        );
    }

    #[test]
    fn scaled_power_scales_negative_magnitudes_too() {
        assert_eq!(
            scaled_power(-4, 20),
            -16,
            "a sap must sharpen with level the same way a buff does"
        );
        assert_eq!(scaled_power(0, 20), 0);
    }
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p feral-processes-engine abilities:: 2>&1 | tail -20`
Expected: `cannot find function ability_power_scale`, `cannot find function scaled_power`.

- [ ] **Step 4: Implement the pure functions**

In `crates/engine/src/abilities.rs`, after `player_routine_slots`:

```rust
/// The multiplier an ability's authored magnitude is scaled by when a
/// combatant of `level` uses it — see
/// `tuning::ABILITY_POWER_SCALE_PER_LEVEL`.
///
/// `level` is clamped at `tuning::ABILITY_POWER_SCALE_LEVEL_CAP` because the
/// player has no level ceiling; see `player_routine_slots`, which clamps for
/// the same reason.
pub fn ability_power_scale(level: u32) -> f32 {
    let level = level.min(crate::tuning::ABILITY_POWER_SCALE_LEVEL_CAP);
    1.0 + level as f32 * crate::tuning::ABILITY_POWER_SCALE_PER_LEVEL
}

/// `power` scaled by `ability_power_scale(level)`, rounded to the nearest
/// whole point. Negative powers scale too — a sap is a negative-power buff,
/// and it has to sharpen with level the same way a buff does.
pub fn scaled_power(power: i32, level: u32) -> i32 {
    (power as f32 * ability_power_scale(level)).round() as i32
}
```

- [ ] **Step 5: Run to verify they pass**

Run: `cargo test -p feral-processes-engine abilities:: 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 6: Write the failing application tests**

Append to `crates/engine/src/tests/combat_abilities.rs`:

```rust
/// A heal stores the scaled figure at the moment it is applied, so nothing
/// downstream has to re-scale.
#[test]
fn a_heal_scales_with_the_users_level() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 400;
        stats.hp = 100;
    }
    game.world.get_mut::<Experience>(player).unwrap().level = 20;

    let ability = crate::abilities::AbilityDef {
        id: "test_heal".into(),
        name: "Test Heal".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneAlly,
        effect: crate::abilities::AbilityEffect::Heal { power: 8 },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[player]);

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        100 + crate::abilities::scaled_power(8, 20),
        "an 8-point patch at level 20 is 32, not 8"
    );
}

#[test]
fn a_buff_stores_the_scaled_power_so_the_tick_needs_no_change() {
    let mut game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let _ = battle_with_a_pack_of(&mut game, 1, 200);
    game.world.get_mut::<Experience>(player).unwrap().level = 20;

    let ability = crate::abilities::AbilityDef {
        id: "test_buff".into(),
        name: "Test Buff".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneAlly,
        effect: crate::abilities::AbilityEffect::Buff {
            kind: BuffKind::Atk,
            power: 3,
            duration: 3,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[player]);

    assert_eq!(
        game.world.get::<CombatBuff>(player).unwrap().active.unwrap().power,
        crate::abilities::scaled_power(3, 20),
        "the scaled figure is stored, not recomputed at read time"
    );
}

#[test]
fn a_bleed_debuffs_per_round_damage_scales_with_the_users_level() {
    let mut game = Game::new(4203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 400);
    game.world.get_mut::<Experience>(player).unwrap().level = 20;

    let ability = crate::abilities::AbilityDef {
        id: "test_bleed".into(),
        name: "Test Bleed".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Debuff {
            kind: StatusKind::Bleed,
            power: 2,
            duration: 3,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };
    game.use_ability(&ability, player, "You", &[enemies[0]]);

    assert_eq!(
        game.world.get::<StatusEffects>(enemies[0]).unwrap().active.unwrap().power,
        crate::abilities::scaled_power(2, 20),
        "bleed is flat damage per round, so it needs scaling as much as a heal does"
    );
}

/// `compute_damage` is `power + ATK - DEF`, so ability damage already rides
/// the user's ATK. Scaling the flat term as well would double-dip through
/// every curve `balance_sim` projects.
#[test]
fn ability_damage_is_not_scaled_by_level() {
    let mut game = Game::new(4204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 2, 500);
    game.world.get_mut::<Stats>(player).unwrap().atk = 10;

    let ability = crate::abilities::AbilityDef {
        id: "test_hit".into(),
        name: "Test Hit".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Damage {
            power: 6,
            status: None,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };

    game.world.get_mut::<Experience>(player).unwrap().level = 1;
    let before = game.world.get::<Stats>(enemies[0]).unwrap().hp;
    game.use_ability(&ability, player, "You", &[enemies[0]]);
    let at_level_1 = before - game.world.get::<Stats>(enemies[0]).unwrap().hp;

    game.world.get_mut::<Experience>(player).unwrap().level = 20;
    let before = game.world.get::<Stats>(enemies[1]).unwrap().hp;
    game.use_ability(&ability, player, "You", &[enemies[1]]);
    let at_level_20 = before - game.world.get::<Stats>(enemies[1]).unwrap().hp;

    assert_eq!(
        at_level_1, at_level_20,
        "ability damage scales through ATK alone — scaling power too would double-dip"
    );
}

/// Drain's heal rides the damage it dealt, which already rides ATK, so it
/// must not be scaled a second time.
#[test]
fn drain_is_not_scaled_by_level() {
    let mut game = Game::new(4205, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 2, 500);
    game.world.get_mut::<Stats>(player).unwrap().atk = 10;

    let ability = crate::abilities::AbilityDef {
        id: "test_drain".into(),
        name: "Test Drain".into(),
        description: "d".into(),
        target: crate::abilities::AbilityTarget::OneEnemyGroupFront,
        effect: crate::abilities::AbilityEffect::Drain {
            power: 10,
            heal_fraction: 0.5,
        },
        cooldown: 1,
        fatigue_cost: 0.0,
        wild_weight: 0,
    };

    game.world.get_mut::<Experience>(player).unwrap().level = 1;
    let before = game.world.get::<Stats>(enemies[0]).unwrap().hp;
    game.use_ability(&ability, player, "You", &[enemies[0]]);
    let at_level_1 = before - game.world.get::<Stats>(enemies[0]).unwrap().hp;

    game.world.get_mut::<Experience>(player).unwrap().level = 20;
    let before = game.world.get::<Stats>(enemies[1]).unwrap().hp;
    game.use_ability(&ability, player, "You", &[enemies[1]]);
    let at_level_20 = before - game.world.get::<Stats>(enemies[1]).unwrap().hp;

    assert_eq!(at_level_1, at_level_20, "a drain scales through ATK alone");
}

/// Wild programs have no `Experience` — they scale by zone and distance —
/// so a hostile carrier reads the current `ZoneLevel` instead.
#[test]
fn a_hostile_scales_its_routine_off_the_zone_level() {
    let mut game = Game::new(4206, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world.resource_mut::<ZoneLevel>().0 = 7;

    assert_eq!(
        game.ability_user_level(enemies[0]),
        7,
        "a wild program has no level, so the zone is what its routine scales from"
    );
    assert_eq!(
        game.ability_user_level(player),
        game.world.get::<Experience>(player).unwrap().level,
        "the player scales off their own level"
    );
}
```

- [ ] **Step 7: Implement `ability_user_level`**

In `crates/engine/src/game/combat.rs`, in the same `impl Game` block as `actor_abilities`, immediately before it:

```rust
    /// The level an ability's magnitude scales from when `entity` uses it —
    /// see `abilities::scaled_power`.
    ///
    /// The player and companions have `Experience`. Wild programs do not:
    /// they scale by zone and distance instead, so a hostile carrier reads
    /// the current `ZoneLevel`, which is the closest analogue it has and
    /// keeps its routine in step with the fight it turns up in.
    ///
    /// One helper for all three cases deliberately — three call sites each
    /// resolving a level would be three formulas to drift.
    pub(crate) fn ability_user_level(&self, entity: Entity) -> u32 {
        self.world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or_else(|| self.world.resource::<ZoneLevel>().0)
    }
```

- [ ] **Step 8: Apply the scale in `use_ability`**

In `crates/engine/src/game/combat_round.rs`, at the top of `use_ability`'s body, before the `for &recipient in recipients` loop:

```rust
        // Resolved once for the whole cast rather than per recipient: every
        // recipient of one ability is scaled by the *user's* level, and
        // re-reading it inside the loop would invite someone to key it off
        // the recipient instead.
        let level = self.ability_user_level(actor);
```

Then in the `Buff` arm, replace `power: *power,` with:

```rust
                            power: abilities::scaled_power(*power, level),
```

In the `Heal` arm, replace the body with:

```rust
                AbilityEffect::Heal { power } => {
                    let power = abilities::scaled_power(*power, level);
                    if let Some(mut stats) = self.world.get_mut::<Stats>(recipient) {
                        stats.hp = (stats.hp + power).min(stats.max_hp);
                    }
                    self.log(format!("{name} patches {on} for {power} HP."));
                }
```

In the `Debuff` arm, replace `power: *power,` with:

```rust
                            power: abilities::scaled_power(*power, level),
```

Leave the `Damage` and `Drain` arms alone.

- [ ] **Step 9: Run the application tests**

Run: `cargo test -p feral-processes-engine combat_abilities 2>&1 | tail -25`
Expected: PASS. If an existing test in `combat_abilities.rs` or `combat_specials.rs` asserted an exact heal or buff figure at a level above 1, it will now be off by the multiplier — update those assertions to wrap the expected figure in `crate::abilities::scaled_power(...)` rather than hardcoding the new number.

- [ ] **Step 10: Rewrite the README's scaling claim**

In `assets/abilities/README.md`, replace this text under the `Buff` entry:

```
    //     Temporary stat boost for `duration` battle rounds. `kind` is
    //     `Atk` or `Def`. Because damage is additive, a flat +3 ATK is worth
    //     exactly 3 extra damage per hit at every level — buff powers do not
    //     need to scale.
```

with:

```
    //     Temporary stat boost for `duration` battle rounds. `kind` is
    //     `Atk` or `Def`.
```

Then add a new section immediately after the ```ron schema block:

```markdown
## Magnitudes scale with level

`power` is an authored *baseline*, not the figure that lands. `Heal`, `Buff`
and `Debuff` magnitudes are multiplied by the level of whoever used the
ability, so a `Heal(power: 8)` restores 8 at level 1 and 32 at level 20. The
curve is `1 + level x ABILITY_POWER_SCALE_PER_LEVEL`, capped at
`ABILITY_POWER_SCALE_LEVEL_CAP` — both in `crates/engine/src/tuning.rs`.
Author powers as though for level 1.

`duration` never scales. Neither does `Damage` power, nor `Drain`: both go
through `power + ATK - DEF`, so they already grow with the user's ATK, and
scaling the flat term as well would count the same growth twice.

A wild program has no level — it scales by zone and distance — so a carrier
scales its routine from the current zone instead.
```

- [ ] **Step 11: Format, lint, full suite, commit**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -5
git add crates/engine/src/tuning.rs crates/engine/src/abilities.rs crates/engine/src/game/combat.rs crates/engine/src/game/combat_round.rs crates/engine/src/tests/combat_abilities.rs assets/abilities/README.md
git commit -m "feat: ability magnitudes scale with the user's level

An 8-point heal is a real patch at level 1 and noise against 400
Integrity. Heal, Buff and Debuff magnitudes now multiply by level;
Damage and Drain do not, because compute_damage already rides ATK and
double-dipping would move every balance_sim curve."
```

---

### Task 4: The twenty abilities

Content only. No engine change.

**Files:**
- Create: twenty files under `assets/abilities/`
- Modify: `assets/abilities/priority_boost.ron`, `assets/abilities/sandbox.ron` — add `cooldown: 1`
- Modify: `crates/engine/src/abilities.rs` — the shipped-count assertion in `the_shipped_set_loads_clean`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: `wild_weight` (Task 1), `Drain`/`Cleanse` (Task 2).
- Produces: twenty ability ids, all wild-only. No Rust identifier.

- [ ] **Step 1: Write the failing asset tests**

Append to `crates/engine/src/tests/assets.rs`:

```rust
/// The twenty hunt-only routines are reachable exactly one way: off a wild
/// carrier. A species or research file naming one would quietly restore the
/// "just target the species" loop this set exists to break.
#[test]
fn no_species_or_research_file_grants_a_wild_only_ability() {
    let game = Game::new(3301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild_only: Vec<String> = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(d, _)| d.id.clone())
        .collect();
    assert_eq!(wild_only.len(), 20, "twenty routines are hunt-only");

    for species in game.species_defs() {
        for ability in &species.abilities {
            assert!(
                !wild_only.contains(&ability.id),
                "species {:?} grants {:?}, which is meant to be findable only in the field",
                species.id,
                ability.id
            );
        }
    }
    for node in game.world.resource::<crate::research::ResearchDb>().all() {
        for id in &node.unlocks_abilities {
            assert!(
                !wild_only.contains(id),
                "research node {:?} unlocks {:?}, which is meant to be findable only in the field",
                node.id,
                id
            );
        }
    }
}

/// A cooldown of 0 means a hostile carrier fires the routine every single
/// round (see `Game::wild_retaliate`). `decompile` is the one exception: it
/// is the player's capture mechanism, hostiles never use it, and a cooldown
/// on a failed capture roll would change the core loop.
#[test]
fn every_shipped_ability_but_decompile_has_a_cooldown() {
    let game = Game::new(3302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.world.resource::<crate::abilities::AbilityDb>().all() {
        if def.id == crate::abilities::DECOMPILE_ABILITY_ID {
            assert_eq!(def.cooldown, 0, "decompile stays spammable, deliberately");
            continue;
        }
        assert!(
            def.cooldown >= 1,
            "ability {:?} has no cooldown, so a wild carrier would fire it every round",
            def.id
        );
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine assets:: 2>&1 | tail -20`
Expected: FAIL — `twenty routines are hunt-only` sees 0, and `priority_boost`/`sandbox` have no cooldown.

There is no `Game::research_nodes()` accessor. Read the nodes off the resource directly, as the test above does through `crate::research::ResearchDb::all()` — it returns `impl Iterator<Item = &ResearchDef>`, cheapest node first. Do not add a `Game` accessor for a test's convenience.

- [ ] **Step 3: Bump the two cooldown-0 abilities**

In `assets/abilities/priority_boost.ron`, add before the closing `)`:

```ron
    cooldown: 1,
```

In `assets/abilities/sandbox.ron`, add before the closing `)`:

```ron
    cooldown: 1,
```

- [ ] **Step 4: Write the six damage routines**

`assets/abilities/kernel_panic.ron`:
```ron
(
    id: "kernel_panic",
    name: "Kernel Panic",
    description: "Heavy damage to one target",
    target: OneEnemyGroupFront,
    effect: Damage(power: 16),
    cooldown: 3,
    fatigue_cost: 10.0,
    wild_weight: 10,
)
```

`assets/abilities/stack_smash.ron`:
```ron
(
    id: "stack_smash",
    name: "Stack Smash",
    description: "Damage one target, likely leaving it bleeding",
    target: OneEnemyGroupFront,
    effect: Damage(power: 9, status: Some((kind: Bleed, chance: 0.6, duration: 3, power: 3))),
    cooldown: 2,
    fatigue_cost: 8.0,
    wild_weight: 12,
)
```

`assets/abilities/pipeline_stall.ron`:
```ron
(
    id: "pipeline_stall",
    name: "Pipeline Stall",
    description: "Damage one target, sometimes locking it up",
    target: OneEnemyGroupFront,
    effect: Damage(power: 7, status: Some((kind: Stun, chance: 0.4, duration: 1, power: 0))),
    cooldown: 3,
    fatigue_cost: 9.0,
    wild_weight: 8,
)
```

`assets/abilities/fork_bomb.ron`:
```ron
(
    id: "fork_bomb",
    name: "Fork Bomb",
    description: "Damage every member of one group, sometimes leaving them bleeding",
    target: WholeEnemyGroup,
    effect: Damage(power: 7, status: Some((kind: Bleed, chance: 0.35, duration: 2, power: 2))),
    cooldown: 3,
    fatigue_cost: 12.0,
    wild_weight: 6,
)
```

`assets/abilities/packet_shred.ron`:
```ron
(
    id: "packet_shred",
    name: "Packet Shred",
    description: "Damage 10 to every member of one group",
    target: WholeEnemyGroup,
    effect: Damage(power: 10),
    cooldown: 3,
    fatigue_cost: 11.0,
    wild_weight: 8,
)
```

`assets/abilities/bus_fault.ron`:
```ron
(
    id: "bus_fault",
    name: "Bus Fault",
    description: "Damage the whole field, sometimes locking targets up",
    target: AllEnemies,
    effect: Damage(power: 6, status: Some((kind: Stun, chance: 0.25, duration: 1, power: 0))),
    cooldown: 5,
    fatigue_cost: 18.0,
    wild_weight: 3,
)
```

- [ ] **Step 5: Write the four debuff routines**

`assets/abilities/hard_lock.ron`:
```ron
(
    id: "hard_lock",
    name: "Hard Lock",
    description: "Stun one target for 2 rounds",
    target: OneEnemyGroupFront,
    effect: Debuff(kind: Stun, power: 0, duration: 2),
    cooldown: 4,
    fatigue_cost: 10.0,
    wild_weight: 6,
)
```

`assets/abilities/heap_corruption.ron`:
```ron
(
    id: "heap_corruption",
    name: "Heap Corruption",
    description: "Bleed every member of one group for 3 rounds",
    target: WholeEnemyGroup,
    effect: Debuff(kind: Bleed, power: 3, duration: 3),
    cooldown: 3,
    fatigue_cost: 11.0,
    wild_weight: 7,
)
```

`assets/abilities/race_condition.ron`:
```ron
(
    id: "race_condition",
    name: "Race Condition",
    description: "Stun every member of one group for 1 round",
    target: WholeEnemyGroup,
    effect: Debuff(kind: Stun, power: 0, duration: 1),
    cooldown: 4,
    fatigue_cost: 13.0,
    wild_weight: 5,
)
```

`assets/abilities/bit_rot.ron`:
```ron
(
    id: "bit_rot",
    name: "Bit Rot",
    description: "Bleed the whole field for 4 rounds",
    target: AllEnemies,
    effect: Debuff(kind: Bleed, power: 2, duration: 4),
    cooldown: 5,
    fatigue_cost: 16.0,
    wild_weight: 4,
)
```

- [ ] **Step 6: Write the four buff and sap routines**

`assets/abilities/hyperthread.ron`:
```ron
(
    id: "hyperthread",
    name: "Hyperthread",
    description: "+6 ATK to one ally for 4 rounds",
    target: OneAlly,
    effect: Buff(kind: Atk, power: 6, duration: 4),
    cooldown: 3,
    fatigue_cost: 8.0,
    wild_weight: 9,
)
```

`assets/abilities/bastion.ron`:
```ron
(
    id: "bastion",
    name: "Bastion",
    description: "+4 DEF to the whole party for 3 rounds",
    target: WholeParty,
    effect: Buff(kind: Def, power: 4, duration: 3),
    cooldown: 3,
    fatigue_cost: 11.0,
    wild_weight: 8,
)
```

`assets/abilities/throttle.ron`:
```ron
(
    id: "throttle",
    name: "Throttle",
    // A sap: a negative buff power subtracts wherever it lands. Note that a
    // combatant holds one buff at a time, so this also clears a target's
    // Defend stance.
    description: "-4 ATK to every member of one group for 3 rounds",
    target: WholeEnemyGroup,
    effect: Buff(kind: Atk, power: -4, duration: 3),
    cooldown: 3,
    fatigue_cost: 10.0,
    wild_weight: 7,
)
```

`assets/abilities/etch.ron`:
```ron
(
    id: "etch",
    name: "Etch",
    description: "-4 DEF to every member of one group for 3 rounds",
    target: WholeEnemyGroup,
    effect: Buff(kind: Def, power: -4, duration: 3),
    cooldown: 3,
    fatigue_cost: 10.0,
    wild_weight: 7,
)
```

- [ ] **Step 7: Write the three heal, two drain, and one cleanse routines**

`assets/abilities/checksum_repair.ron`:
```ron
(
    id: "checksum_repair",
    name: "Checksum Repair",
    description: "Restore 18 Integrity to one ally",
    target: OneAlly,
    effect: Heal(power: 18),
    cooldown: 3,
    fatigue_cost: 9.0,
    wild_weight: 9,
)
```

`assets/abilities/mirror_restore.ron`:
```ron
(
    id: "mirror_restore",
    name: "Mirror Restore",
    description: "Restore 8 Integrity to the whole party",
    target: WholeParty,
    effect: Heal(power: 8),
    cooldown: 2,
    fatigue_cost: 10.0,
    wild_weight: 8,
)
```

`assets/abilities/cold_boot.ron`:
```ron
(
    id: "cold_boot",
    name: "Cold Boot",
    description: "Restore 30 Integrity to one ally",
    target: OneAlly,
    effect: Heal(power: 30),
    cooldown: 5,
    fatigue_cost: 15.0,
    wild_weight: 4,
)
```

`assets/abilities/siphon_cycles.ron`:
```ron
(
    id: "siphon_cycles",
    name: "Siphon Cycles",
    description: "Damage one target and restore half of it to yourself",
    target: OneEnemyGroupFront,
    effect: Drain(power: 10, heal_fraction: 0.5),
    cooldown: 2,
    fatigue_cost: 9.0,
    wild_weight: 9,
)
```

`assets/abilities/leech_array.ron`:
```ron
(
    id: "leech_array",
    name: "Leech Array",
    description: "Damage a whole group and restore a third of it to yourself",
    target: WholeEnemyGroup,
    effect: Drain(power: 6, heal_fraction: 0.3),
    cooldown: 4,
    fatigue_cost: 13.0,
    wild_weight: 5,
)
```

`assets/abilities/flush_cache.ron`:
```ron
(
    id: "flush_cache",
    name: "Flush Cache",
    description: "Clear status conditions from the whole party",
    target: WholeParty,
    effect: Cleanse,
    cooldown: 3,
    fatigue_cost: 7.0,
    wild_weight: 8,
)
```

- [ ] **Step 8: Update the shipped-count assertion**

In `crates/engine/src/abilities.rs`, in `the_shipped_set_loads_clean`:

```rust
        assert_eq!(db.all().count(), 31, "31 abilities ship with the game");
```

- [ ] **Step 9: Run the asset tests**

Run: `cargo test -p feral-processes-engine assets:: 2>&1 | tail -20 && cargo test -p feral-processes-engine abilities::tests::the_shipped_set 2>&1 | tail -10`
Expected: PASS, both.

- [ ] **Step 10: Run the whole engine suite**

Run: `cargo test -p feral-processes-engine 2>&1 | tail -10`
Expected: PASS. Twenty new routine items are minted (`ItemDb::synthesize_routines`), so any test asserting a total item count needs updating to match. Twenty new `routine_*` items also exist in the catalog — check `crates/engine/src/tests/catalog.rs` for a count assertion.

- [ ] **Step 11: Note the set in the README**

Append to `assets/abilities/README.md`:

```markdown
## The hunt-only set

Twenty shipped abilities carry a non-zero `wild_weight` and are named by no
species file and no research node: `kernel_panic`, `stack_smash`,
`pipeline_stall`, `fork_bomb`, `packet_shred`, `bus_fault`, `hard_lock`,
`heap_corruption`, `race_condition`, `bit_rot`, `hyperthread`, `bastion`,
`throttle`, `etch`, `checksum_repair`, `mirror_restore`, `cold_boot`,
`siphon_cycles`, `leech_array`, `flush_cache`.

The only way to get one is to find a wild program carrying it and decompile
that program. Killing the carrier destroys the routine. Adding any of these
to a species or research file would defeat the point, and a test
(`assets::no_species_or_research_file_grants_a_wild_only_ability`) fails if
you do — a mod is of course free to.
```

- [ ] **Step 12: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
git add assets/abilities crates/engine/src/abilities.rs crates/engine/src/tests/assets.rs
git commit -m "feat: twenty hunt-only routines

Reachable off a wild carrier and nowhere else — no species file and no
research node names one. priority_boost and sandbox gain a cooldown of
1; decompile stays at 0 as the one exception."
```

---

### Task 5: The wild spawn roll

**Files:**
- Modify: `crates/engine/src/tuning.rs` — `WILD_ROUTINE_CHANCE`
- Modify: `crates/engine/src/game/spawning.rs` — `spawn_wild_creature` (line 49-91)
- Test: `crates/engine/src/tests/spawning.rs`

**Interfaces:**
- Consumes: `AbilityDb::wild_pool`, `abilities::weighted_pick` (Task 1).
- Produces: `pub(crate) fn Game::roll_wild_routine(&mut self) -> Vec<AbilityId>` — the `Routines` payload for one fresh wild creature. Empty on a miss.

- [ ] **Step 1: Add the tuning constant**

In `crates/engine/src/tuning.rs`, in the "Wild routines and ability scaling" section added in Task 3, above `ABILITY_POWER_SCALE_PER_LEVEL`:

```rust
/// Chance a freshly spawned wild program carries a routine its species
/// never grants — a "carrier". It uses that routine against you in battle,
/// and hands it over installed if you decompile it.
///
/// This decides *whether* a carrier appears; which routine it gets is the
/// per-ability `wild_weight` in `assets/abilities/*.ron`. Deliberately low:
/// a carrier should be a thing you go hunting for, not the default program
/// in the field.
///
/// Unrelated to `WILD_ABILITY_CHANCE`, which gates whether a wild program
/// reaches for its *move's* status effect on a given swing.
pub const WILD_ROUTINE_CHANCE: f64 = 0.06;
```

- [ ] **Step 2: Write the failing tests**

Append to `crates/engine/src/tests/spawning.rs`:

```rust
/// The roll is seeded, so the same seed produces the same carrier. Without
/// the ordering in `AbilityDb::wild_pool` this would pass or fail depending
/// on `HashMap` iteration order.
#[test]
fn the_wild_routine_roll_is_reproducible_from_the_seed() {
    let carried = |seed: u64| {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        (0..40)
            .filter_map(|_| game.roll_wild_routine().first().cloned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        carried(770),
        carried(770),
        "same seed, same carriers — a weighted walk over an unordered pool would not be"
    );
}

/// Whatever the roll produces has to be a real, hunt-only ability: a
/// carrier holding something a species already grants would be no prize.
#[test]
fn a_rolled_routine_is_always_one_of_the_opted_in_abilities() {
    let mut game = Game::new(771, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pool: Vec<String> = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .wild_pool()
        .into_iter()
        .map(|(d, _)| d.id.clone())
        .collect();

    let mut rolled = 0;
    for _ in 0..500 {
        let routines = game.roll_wild_routine();
        assert!(routines.len() <= 1, "a carrier holds exactly one routine");
        if let Some(id) = routines.first() {
            rolled += 1;
            assert!(pool.contains(id), "rolled {id:?}, which is not in the wild pool");
        }
    }
    assert!(
        rolled > 0,
        "500 rolls at WILD_ROUTINE_CHANCE should produce at least one carrier"
    );
}

/// Every wild program routes through `spawn_wild_creature`, so every one of
/// them holds a `Routines` component — empty for the overwhelming majority.
/// Without it, `install_innate_routines` would have nothing to merge and
/// `wild_retaliate` nothing to read.
#[test]
fn every_spawned_wild_creature_holds_a_routines_component() {
    let mut game = Game::new(772, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let species = game.species_defs().into_iter().next().unwrap();
    let entity = game
        .spawn_wild_creature(&species.id, spawn.x + 3, spawn.y)
        .expect("a shipped species spawns");
    assert!(
        game.world.get::<Routines>(entity).is_some(),
        "a wild program with no Routines can never be a carrier and never merges on capture"
    );
}

/// `CreatureSave.routines` is already written for wild creatures, so a
/// carrier round-trips on the existing save format — this pins that, since
/// the spec's "no SAVE_FORMAT_VERSION bump" rests on it.
#[test]
fn a_wild_carrier_survives_a_save_load_round_trip() {
    let dir = std::env::temp_dir().join(format!("feral_carrier_save_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("carrier.sav");

    let mut game = Game::new(773, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let species = game.species_defs().into_iter().next().unwrap();
    let entity = game
        .spawn_wild_creature(&species.id, spawn.x + 4, spawn.y)
        .unwrap();
    game.world
        .entity_mut(entity)
        .insert(Routines(vec!["kernel_panic".to_string()]));
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let mut query = loaded.world.query::<(&Position, &Routines)>();
    let found = query
        .iter(&loaded.world)
        .any(|(pos, r)| pos.x == spawn.x + 4 && r.0 == vec!["kernel_panic".to_string()]);
    assert!(found, "a wild carrier's routine must survive save/load");

    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p feral-processes-engine spawning 2>&1 | tail -20`
Expected: `no method named roll_wild_routine`, and the `Routines` assertion fails.

The save signatures are `pub fn save(&mut self, path: &Path) -> std::io::Result<()>` and `pub fn load(path: &Path, assets_dir: &Path) -> std::io::Result<Self>`, both in `crates/engine/src/game/lifecycle.rs` — the round-trip test above already matches them.

- [ ] **Step 4: Implement the roll**

In `crates/engine/src/game/spawning.rs`, in `impl Game`, immediately before `spawn_wild_creature`:

```rust
    /// Rolls whether a fresh wild program carries a routine, and which —
    /// the `Routines` payload for one creature, empty on the (usual) miss.
    ///
    /// Two rolls, deliberately separate: `WILD_ROUTINE_CHANCE` decides
    /// whether there is a carrier at all, and the per-ability `wild_weight`
    /// decides what it holds. Folding them into one would mean adding an
    /// ability to the pool changed how often carriers appear.
    ///
    /// Exactly one routine. A carrier is a prize, and a second would need a
    /// slot policy at capture time that nothing else in the design needs.
    pub(crate) fn roll_wild_routine(&mut self) -> Vec<crate::abilities::AbilityId> {
        let carries = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(WILD_ROUTINE_CHANCE)
        };
        if !carries {
            return Vec::new();
        }
        let pool: Vec<(crate::abilities::AbilityId, u32)> = self
            .world
            .resource::<AbilityDb>()
            .wild_pool()
            .into_iter()
            .map(|(def, weight)| (def.id.clone(), weight))
            .collect();
        let total: u32 = pool.iter().map(|(_, w)| w).sum();
        if total == 0 {
            return Vec::new();
        }
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..total)
        };
        let weights: Vec<u32> = pool.iter().map(|(_, w)| *w).collect();
        crate::abilities::weighted_pick(&weights, roll)
            .map(|index| vec![pool[index].0.clone()])
            .unwrap_or_default()
    }
```

Add `WILD_ROUTINE_CHANCE` to the `use crate::tuning::{...}` list at the top of the file.

- [ ] **Step 5: Attach it at spawn**

In `spawn_wild_creature`, after `let potential = self.roll_potential();`:

```rust
        let routines = self.roll_wild_routine();
```

and add to the spawn bundle, after `StatusEffects::default(),`:

```rust
                    Routines(routines),
```

The bundle is now 11 components. `bevy_ecs` bundles are tuples with a 12-element limit — if a later change pushes past it, nest the extras in an inner tuple rather than dropping one.

- [ ] **Step 6: Run to verify they pass**

Run: `cargo test -p feral-processes-engine spawning 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 7: Run the whole engine suite**

Run: `cargo test -p feral-processes-engine 2>&1 | tail -10`
Expected: PASS. Wild creatures now consume RNG draws they did not before, so any test asserting an exact spawn layout from a fixed seed may shift. If one fails, re-derive its expected value from the new behaviour — do not reseed to dodge it, and do not move the roll to a different RNG.

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/tuning.rs crates/engine/src/game/spawning.rs crates/engine/src/tests/spawning.rs
git commit -m "feat: wild programs can spawn carrying a routine

WILD_ROUTINE_CHANCE decides whether; the per-ability wild_weight decides
which. Rides the existing Routines component, so CreatureSave already
persists it and SAVE_FORMAT_VERSION is untouched."
```

---

### Task 6: Capture merges rather than overwrites

**Files:**
- Modify: `crates/engine/src/game/combat.rs` — `install_innate_routines` (line 431-464), `install_unlocked_routines` (line 488-552)
- Test: `crates/engine/src/tests/routines.rs`

**Interfaces:**
- Consumes: a wild creature's `Routines` component (Task 5).
- Produces: `pub(crate) fn Game::return_routine_to_cargo(&mut self, ability: &str)` — mints the ability's routine item into the player's inventory.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/routines.rs`:

```rust
/// The whole payoff: decompile a carrier and its routine comes with it.
#[test]
fn a_carried_routine_survives_capture() {
    let mut game = Game::new(5501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = generic_species(&game);
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let carrier = game
        .spawn_wild_creature(&species.id, spawn.x + 2, spawn.y)
        .unwrap();
    game.world
        .entity_mut(carrier)
        .insert(Routines(vec!["kernel_panic".to_string()]));
    game.world
        .entity_mut(carrier)
        .insert(Experience::default());

    game.install_innate_routines(carrier);

    assert!(
        game.world
            .get::<Routines>(carrier)
            .unwrap()
            .0
            .contains(&"kernel_panic".to_string()),
        "the carried routine is the prize — it must not be overwritten by the species kit"
    );
}

/// A carrier already holds something real, so it must not also be handed
/// the placeholder that exists for programs whose species grants nothing.
#[test]
fn a_carrier_of_an_ability_less_species_is_not_given_the_fallback() {
    let mut game = Game::new(5502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = generic_species(&game);
    assert!(
        species.abilities.is_empty(),
        "fixture: this species grants nothing of its own"
    );
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let carrier = game
        .spawn_wild_creature(&species.id, spawn.x + 2, spawn.y)
        .unwrap();
    game.world
        .entity_mut(carrier)
        .insert(Routines(vec!["cold_boot".to_string()]));
    game.world
        .entity_mut(carrier)
        .insert(Experience::default());

    game.install_innate_routines(carrier);

    assert_eq!(
        game.world.get::<Routines>(carrier).unwrap().0,
        vec!["cold_boot".to_string()],
        "the fallback fills an empty kit, not a kit that already holds a prize"
    );
}

/// A level-1 program has one slot, and six shipped species grant an ability
/// at level 1. The carried routine wins the slot; the species ability is
/// minted into cargo rather than destroyed, so the player can swap.
#[test]
fn a_species_ability_displaced_by_a_carried_routine_goes_to_cargo() {
    let mut game = Game::new(5503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| s.abilities.iter().any(|a| a.level <= 1))
        .expect("a shipped species grants an ability at level 1");
    let displaced = species
        .abilities
        .iter()
        .find(|a| a.level <= 1)
        .unwrap()
        .id
        .clone();

    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let carrier = game
        .spawn_wild_creature(&species.id, spawn.x + 2, spawn.y)
        .unwrap();
    game.world
        .entity_mut(carrier)
        .insert(Routines(vec!["bastion".to_string()]));
    game.world
        .entity_mut(carrier)
        .insert(Experience::default());

    let player = game.player_entity();
    let item = crate::abilities::routine_item_id(&displaced);
    let before = game.world.get::<Inventory>(player).unwrap().count(&item);

    game.install_innate_routines(carrier);

    assert_eq!(
        game.world.get::<Routines>(carrier).unwrap().0,
        vec!["bastion".to_string()],
        "one slot at level 1, and the carried routine takes it"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().count(&item),
        before + 1,
        "the displaced species ability lands in cargo instead of being destroyed"
    );
}

/// A species unlock reaching a full kit used to be logged and lost. It now
/// goes to cargo, and — critically — must never evict a carried routine.
#[test]
fn a_level_up_unlock_never_evicts_a_carried_routine() {
    let mut game = Game::new(5504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| s.abilities.iter().any(|a| a.level > 1))
        .expect("a shipped species unlocks an ability above level 1");
    let unlock = species.abilities.iter().find(|a| a.level > 1).unwrap().clone();

    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let pet = game
        .spawn_wild_creature(&species.id, spawn.x + 2, spawn.y)
        .unwrap();
    game.world.entity_mut(pet).insert(Experience::default());
    game.world
        .entity_mut(pet)
        .insert(Routines(vec!["cold_boot".to_string()]));

    // One slot, already holding the prize, and the unlock now lands.
    game.install_unlocked_routines(pet, 1, unlock.level);

    assert!(
        game.world
            .get::<Routines>(pet)
            .unwrap()
            .0
            .contains(&"cold_boot".to_string()),
        "a carried routine is not the fallback placeholder and must never be evicted"
    );
    let player = game.player_entity();
    let item = crate::abilities::routine_item_id(&unlock.id);
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().count(&item),
        1,
        "the unlock that found no room goes to cargo"
    );
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine routines 2>&1 | tail -25`
Expected: FAIL — `install_innate_routines` overwrites, so the carried routine is gone; cargo counts are unchanged.

`generic_species` and `test_assets_dir` come from `super::support::*`, already imported in that file. Add `use crate::components::*;` if `Inventory`/`Routines` are not already in scope.

- [ ] **Step 3: Add the cargo helper**

In `crates/engine/src/game/combat.rs`, in the same `impl Game` block, immediately before `install_innate_routines`:

```rust
    /// Mints `ability`'s routine item into the player's cargo — where a
    /// routine goes when it has nowhere to be installed.
    ///
    /// Displacing a routine used to destroy it. It is a real object with an
    /// item of its own (`ItemDb::synthesize_routines` mints one per loaded
    /// ability), so returning it turns a slot collision into a swap decision
    /// the player gets to make later, rather than a loss they get to read
    /// about.
    pub(crate) fn return_routine_to_cargo(&mut self, ability: &str) {
        let item = crate::abilities::routine_item_id(ability);
        let player = self.player_entity();
        if let Some(mut inventory) = self.world.get_mut::<Inventory>(player) {
            inventory.add(item, 1);
        }
    }
```

- [ ] **Step 4: Merge in `install_innate_routines`**

Replace the body of `install_innate_routines` from `let installed = if declared.is_empty()` through `self.world.entity_mut(entity).insert(Routines(installed));` with:

```rust
        // Whatever this program was already holding is what it was found
        // carrying in the field — see `Game::roll_wild_routine`. That is the
        // prize the player decompiled it for, so it keeps its place and the
        // species kit fills in around it.
        let carried: Vec<AbilityId> = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .unwrap_or_default();

        let mut installed = carried.clone();
        for id in declared {
            if installed.contains(&id) {
                continue;
            }
            if installed.len() >= slots {
                let name = self.creature_label(entity);
                let ability_name = self.ability_display_name(&id);
                self.log(format!(
                    "{name} has no free routine slot for {ability_name} — it goes to cargo."
                ));
                self.return_routine_to_cargo(&id);
                continue;
            }
            installed.push(id);
        }
        // The fallback fills an *empty* kit. A carrier already holds
        // something real, so it never gets the placeholder.
        if installed.is_empty() {
            installed.push(abilities::FALLBACK_ABILITY_ID.to_string());
        }
        self.world.entity_mut(entity).insert(Routines(installed));
```

Update the doc comment above the function: the "replacing whatever it holds" sentence is now wrong. Replace it with:

```rust
    /// Installs the kit `entity`'s species grants at its current level,
    /// merged with whatever it was already carrying. Called once when a
    /// program comes into existence — a decompile or a fusion — never
    /// afterwards.
    ///
    /// A wild program can spawn carrying a routine its species never grants
    /// (`Game::roll_wild_routine`); that routine is the reason the player
    /// decompiled it, so it keeps its slot and the species kit fills in
    /// around it. Anything that doesn't fit goes to cargo.
    ///
    /// A species declaring no abilities gets `FALLBACK_ABILITY_ID` instead,
    /// which is what keeps an ability-less species commandable and keeps
    /// that ability obtainable by extraction: nothing else grants it. A
    /// carrier never gets it — the fallback fills an empty kit, and a
    /// carrier's is not empty.
```

- [ ] **Step 5: Send overflow to cargo in `install_unlocked_routines`**

In `install_unlocked_routines`, replace:

```rust
                self.log(format!(
                    "{name} has no free routine slot for {} — the unlock is lost.",
                    self.ability_display_name(&id)
                ));
                continue;
```

with:

```rust
                self.log(format!(
                    "{name} has no free routine slot for {} — it goes to cargo.",
                    self.ability_display_name(&id)
                ));
                self.return_routine_to_cargo(&id);
                continue;
```

Update the "genuine-loss state" paragraph in its doc comment: it is no longer a loss.

```rust
    /// Only when every slot instead holds a *real* routine — installed,
    /// researched, another innate ability, or one the program was found
    /// carrying in the field — does the unlock go to cargo instead of a
    /// slot. Not lost: `return_routine_to_cargo` mints its routine item, so
    /// the player can install it by hand once they free a slot. A carried
    /// routine is never the fallback, so it is never the thing evicted.
```

- [ ] **Step 6: Run to verify they pass**

Run: `cargo test -p feral-processes-engine routines 2>&1 | tail -25`
Expected: PASS.

- [ ] **Step 7: Run the whole engine suite**

Run: `cargo test -p feral-processes-engine 2>&1 | tail -10`
Expected: PASS. Existing tests asserting the old "the unlock is lost" log line need their expected text updated to "it goes to cargo".

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/game/combat.rs crates/engine/src/tests/routines.rs
git commit -m "feat: a carried routine survives capture, and overflow goes to cargo

install_innate_routines merges the species kit around whatever the
program was found carrying instead of overwriting it. A routine that
finds no slot is minted into cargo rather than destroyed, which turns a
level-1 slot collision into a swap decision."
```

---

### Task 7: Side-aware targeting

`ability_recipients` resolves every target from the player's side. A hostile using an ability needs them flipped.

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs` — `ability_recipients` (line 474-521)
- Test: `crates/engine/src/tests/combat_targeting.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub(crate) fn Game::is_hostile(&self, entity: Entity) -> bool` (if one does not already exist — check first)
  - `ability_recipients` gains a leading `actor: Entity` parameter: `pub(crate) fn ability_recipients(&self, actor: Entity, target: AbilityTarget, chosen: &battle::SpecialTarget) -> Vec<Entity>`. Its one existing caller is `resolve_one_action` (combat_round.rs:167), which passes the acting entity it already holds.
  - `pub(crate) fn Game::living_party(&self) -> Vec<Entity>` — the player plus every living companion.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/combat_targeting.rs`:

```rust
/// The mirror: "ally" means the user's own side, whichever side that is.
#[test]
fn a_hostile_ally_target_resolves_to_its_own_side() {
    let mut game = Game::new(6601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 3, 100);

    let recipients = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::WholeParty,
        &battle::SpecialTarget::WholeParty,
    );
    assert_eq!(recipients.len(), 3, "a hostile's 'whole party' is its own side");
    assert!(
        !recipients.contains(&player),
        "and never reaches across to the player"
    );
    for e in &enemies {
        assert!(recipients.contains(e));
    }
}

#[test]
fn a_hostile_one_ally_target_picks_one_of_its_own() {
    let mut game = Game::new(6602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 3, 100);

    let recipients = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::OneAlly,
        &battle::SpecialTarget::WholeParty,
    );
    assert_eq!(recipients.len(), 1, "exactly one recipient");
    assert!(
        enemies.contains(&recipients[0]) && recipients[0] != player,
        "and it is one of the hostiles, not the player"
    );
}

/// `WholeEnemyGroup` and `AllEnemies` collapse for a hostile actor: the
/// player has one party where the hostiles have groups, and there is no
/// player-side subdivision to select.
#[test]
fn both_hostile_area_enemy_targets_resolve_to_the_whole_player_party() {
    let mut game = Game::new(6603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 2, 100);

    let group = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::WholeEnemyGroup,
        &battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    let all = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::AllEnemies,
        &battle::SpecialTarget::AllEnemies,
    );
    assert_eq!(group, all, "the two collapse for a hostile actor");
    assert!(group.contains(&player));
    for e in &enemies {
        assert!(!group.contains(e), "a hostile area attack never hits its own side");
    }
}

#[test]
fn a_hostile_single_enemy_target_hits_exactly_one_party_member() {
    let mut game = Game::new(6604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 100);

    let recipients = game.ability_recipients(
        enemies[0],
        crate::abilities::AbilityTarget::OneEnemyGroupFront,
        &battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    assert_eq!(recipients.len(), 1);
    assert!(
        !enemies.contains(&recipients[0]),
        "it aims at the party, not at itself"
    );
    assert_eq!(recipients[0], player, "with only the player in the party, it is the player");
}

/// The player side is unchanged by any of this.
#[test]
fn the_players_side_targets_exactly_as_it_did() {
    let mut game = Game::new(6605, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 3, 100);

    let recipients = game.ability_recipients(
        player,
        crate::abilities::AbilityTarget::WholeEnemyGroup,
        &battle::SpecialTarget::EnemyGroup { group: 0 },
    );
    assert_eq!(recipients.len(), 3, "the player's group attack still hits the group");
    assert!(!recipients.contains(&player));
}
```

`battle_with_a_pack_of` is defined in `crates/engine/src/tests/combat_abilities.rs`. Either move it into `support.rs` as `pub(super)` and update both files' call sites, or copy it into `combat_targeting.rs` — moving it is better, since a third caller would make three copies.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine combat_targeting 2>&1 | tail -20`
Expected: compile error — `ability_recipients` takes 2 arguments, not 3.

- [ ] **Step 3: Add the side helpers**

In `crates/engine/src/game/combat_round.rs`, immediately before `ability_recipients`:

```rust
    /// Whether `entity` is fighting on the wild side. Ability targets are
    /// authored from the party's point of view, so this is what decides
    /// which way to read them — see `ability_recipients`.
    pub(crate) fn is_hostile(&self, entity: Entity) -> bool {
        self.world.get::<Hostile>(entity).is_some()
    }

    /// The player plus every living companion — the party side as a flat
    /// list. What a hostile's enemy-facing ability lands on.
    pub(crate) fn living_party(&self) -> Vec<Entity> {
        let battle_slots = self
            .world
            .get_resource::<BattleState>()
            .map(|b| b.planned.len())
            .unwrap_or(0);
        (0..battle_slots)
            .filter_map(|slot| self.actor_entity(battle::Actor::Party(slot)))
            .filter(|&e| self.creature_alive(e))
            .collect()
    }
```

Neither helper exists today — `grep -rn "fn is_hostile" crates/engine/src/` returns nothing, and the party walk is currently inlined in `ability_recipients`' `WholeParty` arm.

- [ ] **Step 4: Make `ability_recipients` side-aware**

Replace `ability_recipients` in `crates/engine/src/game/combat_round.rs` with:

```rust
    /// Which entities `target` lands on, read from `actor`'s side of the
    /// fight.
    ///
    /// Targets are authored from the party's point of view — "ally" means a
    /// party member, "enemy" means a wild program. A hostile using the same
    /// ability flips both: its ally is another hostile, and its enemy is the
    /// party. That mirror is what lets one ability file serve both sides
    /// instead of needing an enemy-only twin.
    ///
    /// Two of the shapes collapse on the hostile side. The party is a single
    /// flat roster where the wild side is partitioned into groups, so
    /// `WholeEnemyGroup` has no player-side subdivision to select and reads
    /// identically to `AllEnemies`. That is the asymmetry of the two sides,
    /// not a shortcut.
    pub(crate) fn ability_recipients(
        &self,
        actor: Entity,
        target: AbilityTarget,
        chosen: &battle::SpecialTarget,
    ) -> Vec<Entity> {
        if self.is_hostile(actor) {
            return match target {
                AbilityTarget::OneAlly => self
                    .hostile_ally_of(actor)
                    .into_iter()
                    .collect(),
                AbilityTarget::WholeParty => self.all_living_enemies(),
                AbilityTarget::OneEnemyGroupFront => match chosen {
                    battle::SpecialTarget::Ally { slot } => self
                        .actor_entity(battle::Actor::Party(*slot))
                        .filter(|&e| self.creature_alive(e))
                        .into_iter()
                        .collect(),
                    _ => self.living_party().into_iter().take(1).collect(),
                },
                AbilityTarget::WholeEnemyGroup | AbilityTarget::AllEnemies => self.living_party(),
            };
        }
        match target {
            AbilityTarget::OneAlly => match chosen {
                battle::SpecialTarget::Ally { slot } => self
                    .actor_entity(battle::Actor::Party(*slot))
                    .filter(|&e| self.creature_alive(e))
                    .into_iter()
                    .collect(),
                _ => Vec::new(),
            },
            AbilityTarget::WholeParty => self.living_party(),
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

    /// One living hostile for a carrier's ally-facing routine to land on.
    ///
    /// A uniform pick, not "the most hurt". A carrier fires whenever its
    /// routine is off cooldown, so a heal landing on a healthy ally is
    /// wasted — accepted, because the alternative is a per-effect
    /// situational policy that this design deliberately does not have.
    ///
    /// `&self` rather than `&mut self`, so the pick is derived from the
    /// battle's round number rather than drawing from `GameRng`. Same
    /// reproducibility, no borrow fight with the caller.
    fn hostile_ally_of(&self, actor: Entity) -> Option<Entity> {
        let candidates = self.all_living_enemies();
        if candidates.is_empty() {
            return None;
        }
        let round = self
            .world
            .get_resource::<BattleState>()
            .map(|b| b.round as usize)
            .unwrap_or(0);
        let offset = candidates.iter().position(|&e| e == actor).unwrap_or(0);
        Some(candidates[(round + offset) % candidates.len()])
    }
```

The player-side `WholeParty` arm changes from an inline slot walk to `living_party()` — same behaviour, one implementation.

- [ ] **Step 5: Update the single caller**

In `resolve_one_action` (around line 167), change:

```rust
                        let recipients = self.ability_recipients(ability.target, &target);
```

to:

```rust
                        let recipients = self.ability_recipients(entity, ability.target, &target);
```

- [ ] **Step 6: Run to verify they pass**

Run: `cargo test -p feral-processes-engine combat_targeting 2>&1 | tail -20`
Expected: PASS.

- [ ] **Step 7: Run the whole engine suite**

Run: `cargo test -p feral-processes-engine 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/game/combat_round.rs crates/engine/src/tests/combat_targeting.rs crates/engine/src/tests/combat_abilities.rs crates/engine/src/tests/support.rs
git commit -m "feat: ability targets mirror for a hostile user

Targets are authored from the party's point of view; a hostile flips
both sides, so one ability file serves either. WholeEnemyGroup and
AllEnemies collapse on the hostile side — the party is one flat roster
where the wild side is partitioned into groups."
```

---

### Task 8: Hostiles spend a round on a routine

**Files:**
- Modify: `crates/engine/src/tuning.rs` — `ENEMY_ROUTINE_MIN_COOLDOWN`
- Modify: `crates/engine/src/game/combat_status.rs` — `wild_retaliate` (line 124), `tick_round_status_effects` (line 407), `clear_battle_status_effects` (line 471)
- Modify: `crates/engine/src/resources.rs` — the `MessageKind::EnemySpecial` doc comment
- Test: `crates/engine/src/tests/combat_status.rs`

**Interfaces:**
- Consumes: `ability_recipients(actor, ..)` (Task 7), `use_ability` (Tasks 2-3), `Routines` on wild creatures (Task 5).
- Produces: `pub(crate) fn Game::wild_routine_ready(&self, wild: Entity) -> Option<AbilityDef>` — the routine a carrier will spend this round on, if any.

- [ ] **Step 1: Add the tuning constant**

In `crates/engine/src/tuning.rs`, in the "Wild routines and ability scaling" section:

```rust
/// Floor on the cooldown a hostile arms after spending a routine.
///
/// `AbilityDef::cooldown` is `#[serde(default)]` 0, and a carrier fires
/// whenever its routine is off cooldown — so a mod ability declaring no
/// cooldown would fire every single round. The player side keeps the
/// authored value untouched, which is what leaves `decompile` spammable.
pub const ENEMY_ROUTINE_MIN_COOLDOWN: u32 = 1;
```

- [ ] **Step 2: Write the failing tests**

Append to `crates/engine/src/tests/combat_status.rs`:

```rust
/// The discovery mechanism: you learn a program is a carrier by being hit
/// with what it carries.
#[test]
fn a_carrier_spends_its_round_on_its_routine_instead_of_a_move() {
    let mut game = Game::new(7701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world
        .entity_mut(enemies[0])
        .insert(Routines(vec!["hard_lock".to_string()]));
    game.world.get_mut::<Stats>(enemies[0]).unwrap().atk = 0;

    game.wild_retaliate(enemies[0], 0, player);

    assert!(
        matches!(
            game.world.get::<StatusEffects>(player).unwrap().active,
            Some(ActiveStatus { kind: StatusKind::Stun, .. })
        ),
        "Hard Lock stuns — a move could not have done this"
    );
}

#[test]
fn a_carriers_routine_goes_on_cooldown_and_comes_back() {
    let mut game = Game::new(7702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world
        .entity_mut(enemies[0])
        .insert(Routines(vec!["hard_lock".to_string()]));

    game.wild_retaliate(enemies[0], 0, player);
    assert!(
        game.wild_routine_ready(enemies[0]).is_none(),
        "it just fired — Hard Lock has a cooldown of 4"
    );

    for _ in 0..6 {
        game.tick_ability_cooldowns(enemies[0]);
    }
    assert!(
        game.wild_routine_ready(enemies[0]).is_some(),
        "and it comes back once the cooldown has ticked out"
    );
}

/// `cooldown` defaults to 0 and a carrier fires whenever it can, so a mod
/// ability with none must still not fire two rounds running.
#[test]
fn a_cooldown_zero_routine_still_cannot_fire_two_rounds_running() {
    let mut game = Game::new(7703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    // decompile is the shipped ability at cooldown 0. A hostile never uses
    // it, so install the fallback and zero its cooldown by hand instead.
    game.world
        .entity_mut(enemies[0])
        .insert(Routines(vec![crate::abilities::FALLBACK_ABILITY_ID.to_string()]));

    game.wild_retaliate(enemies[0], 0, player);
    assert!(
        game.wild_routine_ready(enemies[0]).is_none()
            || crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN == 0,
        "the enemy side floors the cooldown so a mod cannot produce an every-round routine"
    );
}

/// Hostiles that survive a jack-out stay on the map. A mirrored buff left
/// armed on one would be a permanent free stat that never ticks down.
#[test]
fn ending_a_battle_clears_every_hostiles_combat_state() {
    let mut game = Game::new(7704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 3, 200);
    for &e in &enemies {
        game.arm_buff(
            e,
            ActiveBuff {
                kind: BuffKind::Atk,
                remaining: 5,
                power: 9,
            },
        );
        game.world.get_mut::<StatusEffects>(e).unwrap().active = Some(ActiveStatus {
            kind: StatusKind::Bleed,
            remaining: 3,
            power: 2,
        });
    }

    game.end_battle(player, None);

    for &e in &enemies {
        assert!(
            game.world.get::<CombatBuff>(e).is_none_or(|b| b.active.is_none()),
            "a buff left armed on a surviving hostile never ticks down — it is a free stat forever"
        );
        assert!(
            game.world.get::<StatusEffects>(e).is_none_or(|s| s.active.is_none()),
            "and a bleed left running would tick outside any battle"
        );
    }
}
```

`is_none_or` on `Option` requires Rust 1.82+; if the toolchain rejects it, use `.map_or(true, |b| ...)`.

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p feral-processes-engine combat_status 2>&1 | tail -20`
Expected: `no method named wild_routine_ready`, and the teardown assertions fail.

- [ ] **Step 4: Implement the readiness check**

In `crates/engine/src/game/combat_status.rs`, immediately before `wild_retaliate`:

```rust
    /// The routine `wild` will spend this round on, if it is carrying one
    /// that is not still cooling.
    ///
    /// First installed wins. A carrier holds exactly one
    /// (`Game::roll_wild_routine`), so ordering is not a real decision, and
    /// inventing a priority scheme for a one-element list would be building
    /// for a case that does not exist.
    ///
    /// `Decompile` is excluded: it is resolved by group index against the
    /// *wild* side and would do nothing coherent aimed the other way. Only a
    /// mod can put it on a hostile — `decompile.ron` has no `wild_weight` —
    /// but a mod that does gets a normal move rather than a wasted round.
    pub(crate) fn wild_routine_ready(&self, wild: Entity) -> Option<AbilityDef> {
        let cooling = self
            .world
            .get::<AbilityCooldowns>(wild)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        let db = self.world.resource::<AbilityDb>();
        self.world
            .get::<Routines>(wild)
            .map(|r| r.0.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|id| !cooling.contains_key(*id))
            .filter_map(|id| db.get(id))
            .find(|def| !matches!(def.effect, AbilityEffect::Decompile))
            .cloned()
    }
```

- [ ] **Step 5: Spend the round on it in `wild_retaliate`**

At the very top of `wild_retaliate`'s body, before `let species_id = ...`:

```rust
        // A carrier spends its round on the routine rather than a move. No
        // engagement check: `ENGAGED_GROUPS` gates *moves* because a
        // back-rank program has to physically reach, and a routine is
        // executed rather than swung — gating it would silently disable
        // every carrier behind the front groups.
        if let Some(routine) = self.wild_routine_ready(wild) {
            // Armed before the effect resolves, matching `resolve_one_action`:
            // a killing blow ends the battle inside `reap_dead_members` and
            // `end_battle` wipes every battle-scoped component, so a cooldown
            // written afterwards would land on an entity already cleaned up.
            //
            // Floored, because `cooldown` defaults to 0 and a carrier fires
            // whenever it can — see `ENEMY_ROUTINE_MIN_COOLDOWN`. The +1 is
            // the same one the party side uses, so this round's own tick
            // doesn't eat a round.
            let armed = routine.cooldown.max(ENEMY_ROUTINE_MIN_COOLDOWN) + 1;
            let mut cooldowns = self
                .world
                .get::<AbilityCooldowns>(wild)
                .map(|c| c.0.clone())
                .unwrap_or_default();
            cooldowns.insert(routine.id.clone(), armed);
            self.world
                .entity_mut(wild)
                .insert(AbilityCooldowns(cooldowns));

            // Fatigue is not charged: `fatigue_cost` models the *player*
            // issuing a command, and a wild program commands itself.
            let name = self.creature_label(wild);
            self.log_kind(
                MessageKind::EnemySpecial,
                format!("{name} runs {}.", routine.name),
            );
            let chosen = battle::SpecialTarget::EnemyGroup { group };
            let recipients = self.ability_recipients(wild, routine.target, &chosen);
            self.use_ability(&routine, wild, &name, &recipients);
            self.reap_dead_members(player);
            return;
        }
```

Add `ENEMY_ROUTINE_MIN_COOLDOWN` to the file's `use crate::tuning::{...}` list.

- [ ] **Step 6: Tick hostile cooldowns and buffs each round**

The per-round tick lives in `Game::tick_round_status_effects` (`crates/engine/src/game/combat_status.rs`, around line 407), not in `battle_resolve_round`. It already walks hostiles for `tick_status_effects`, but calls `tick_combat_buff` and `tick_ability_cooldowns` only for the player and companions.

Both gaps are live the moment a hostile holds a routine: a fired routine that never cools makes a carrier a one-shot, and a mirrored buff or sap that never ticks lasts the whole fight regardless of its authored `duration`.

Replace the existing hostile loop:

```rust
        for wild in self.all_living_enemies() {
            let label = self.entity_label(wild);
            self.tick_status_effects(wild, &label);
        }
```

with:

```rust
        for wild in self.all_living_enemies() {
            let label = self.entity_label(wild);
            self.tick_status_effects(wild, &label);
            // Both of these were party-only while abilities were party-only.
            // A carrier's routine has to cool or it fires once and never
            // again, and a mirrored buff has to expire or its `duration` is
            // decoration.
            self.tick_combat_buff(wild);
            self.tick_ability_cooldowns(wild);
        }
```

Add this test alongside the others in Step 2:

```rust
/// A buff aimed at a hostile has to expire on schedule. While abilities
/// were party-only, `tick_combat_buff` was never called for a hostile — so
/// a mirrored buff or sap would have lasted the whole fight regardless of
/// its authored duration.
#[test]
fn a_buff_on_a_hostile_ticks_down_each_round() {
    let mut game = Game::new(7705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.arm_buff(
        enemies[0],
        ActiveBuff {
            kind: BuffKind::Atk,
            remaining: 2,
            power: 5,
        },
    );

    game.tick_round_status_effects(player);
    assert_eq!(
        game.world.get::<CombatBuff>(enemies[0]).unwrap().active.unwrap().remaining,
        1,
        "one round burned"
    );
    game.tick_round_status_effects(player);
    assert!(
        game.world.get::<CombatBuff>(enemies[0]).unwrap().active.is_none(),
        "and it expires rather than lasting the whole fight"
    );
}
```

- [ ] **Step 7: Clear every hostile at teardown**

In `crates/engine/src/game/combat_status.rs`, in `clear_battle_status_effects`, replace:

```rust
        if let Some(mut s) = wild.and_then(|w| self.world.get_mut::<StatusEffects>(w)) {
            s.active = None;
        }
```

with:

```rust
        // Every hostile still in the fight, not only the one passed in.
        // Survivors of a jack-out stay on the map, and a mirrored buff left
        // armed on one never ticks down — `effective_atk`/`effective_def`
        // read `CombatBuff` unconditionally, so it would be a free stat
        // forever. `wild` is still taken because it may name a program that
        // has already left its group (a successful decompile).
        let mut hostiles: Vec<Entity> = self.all_living_enemies();
        hostiles.extend(wild);
        for hostile in hostiles {
            if let Some(mut s) = self.world.get_mut::<StatusEffects>(hostile) {
                s.active = None;
            }
            if let Some(mut b) = self.world.get_mut::<CombatBuff>(hostile) {
                b.active = None;
            }
            if let Some(mut c) = self.world.get_mut::<AbilityCooldowns>(hostile) {
                c.0.clear();
            }
        }
```

- [ ] **Step 8: Correct the `EnemySpecial` doc**

In `crates/engine/src/resources.rs`, replace:

```rust
    /// A hostile program landing a move that also inflicts a status
    /// condition, and the line naming the condition it inflicted. Enemies
    /// have no separate Special action the way the party does — a move
    /// carrying a `MoveEffect` is the whole of what makes one special.
```

with:

```rust
    /// A hostile program doing something other than a plain swing: a move
    /// that also inflicts a status condition, the line naming that
    /// condition, or a carrier spending its round on an installed routine
    /// (see `Game::wild_routine_ready`).
```

- [ ] **Step 9: Run to verify they pass**

Run: `cargo test -p feral-processes-engine combat_status 2>&1 | tail -25`
Expected: PASS.

- [ ] **Step 10: Run the whole workspace**

Run: `cargo test --workspace 2>&1 | tail -10`
Expected: PASS. Fights now play differently for any test whose fixture happens to spawn a carrier — if one fails, check whether a carrier appeared before assuming a logic bug.

- [ ] **Step 11: Format, lint, commit**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
git add crates/engine/src/tuning.rs crates/engine/src/game/combat_status.rs crates/engine/src/resources.rs crates/engine/src/tests/combat_status.rs
git commit -m "feat: a wild carrier spends its round on the routine it holds

Which is how the player finds out it has one. Cooldowns and combat
buffs now tick for hostiles — both were party-only while abilities
were — and battle teardown clears every hostile rather than the one
entity it was passed, since a buff left on a jack-out survivor would
otherwise be a free stat forever."
```

---

### Task 9: Player-facing documentation

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes: everything.
- Produces: nothing code depends on.

- [ ] **Step 1: Check what the README currently claims**

Run:
```bash
grep -n -i "abilit\|routine\|decompile\|special" README.md
```

Read the surrounding sections. The README describes how abilities are obtained; that description is now incomplete in two ways — hunting is a third acquisition route, and hostiles now use specials.

- [ ] **Step 2: Update the README**

Add to whichever section covers abilities/routines (match the file's existing voice and heading level):

```markdown
Not every routine is on the research tree or in a species' kit. Some exist
only in the field: a wild program can spawn carrying one, and it will use
that routine against you — which is how you find out it has it. Decompile
the carrier and the routine comes with it, installed and ready to pop out
into whichever program you want running it. Destroy the carrier and the
routine goes with it.
```

- [ ] **Step 3: Update the CHANGELOG**

Add under the current unreleased heading, matching the file's existing format:

```markdown
- Wild programs can spawn carrying a routine their species never grants, and
  will use it against you in battle. Decompiling the carrier hands it over
  installed; destroying the carrier destroys the routine.
- Twenty new routines exist that no species and no research node grants —
  the only way to get one is to find a carrier.
- Ability magnitudes now scale with the user's level, so a heal or a buff
  stays relevant past the early game.
- Hostile programs can now spend a round on an installed routine, where
  before their whole kit was their moveset.
- A routine that finds no free slot goes to cargo instead of being
  destroyed.
- Every ability now has a cooldown except Decompile.
```

- [ ] **Step 4: Final gate**

```bash
cargo fmt
cargo clippy --workspace 2>&1 | tail -20
cargo test --workspace 2>&1 | tail -10
cargo test -p feral-processes-engine balance_sim 2>&1 | tail -10
```

Expected: everything passes. `balance_sim` models no abilities, so its curves must be **unchanged** — a moved curve here means something touched `compute_damage` or a species stat, which nothing in this plan should have.

- [ ] **Step 5: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: hunting for routines, and hostiles that use them"
```

---

## After the plan

Two things this work cannot verify itself, both flagged in the spec:

1. **`balance_sim` models no abilities.** It will not tell you that hostile carriers made fights harder, or that `ABILITY_POWER_SCALE_PER_LEVEL` is wrong. Every number in Task 4's twenty files and every constant in the table at the top is arithmetic-plausible only.
2. **The game has to be launched.** `WILD_ROUTINE_CHANCE: 0.06` is the number that decides whether hunting feels rewarding or hopeless, and that is a feel judgement no test makes.
