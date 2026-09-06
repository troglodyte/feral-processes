# Town-Sourced Raids (Settlements Phase 7a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A town at `Standing::Hostile` near the base sends raiders who carry off banked build currency, and the player's raid defense — structures *and* allied garrisons — is what turns them away.

**Architecture:** One new named query on `Standing` (exhaustive, censused, `preys_on_routes`' shape), one roll-then-gate check called from `tick_inner` beside `raid_check`, and one resolver that reads `Game::total_raid_defense` — the same number the ambient sweep reads, which is what makes hostile and allied neighbours meet on one axis. Nothing about `run_raid` changes.

**Tech Stack:** Rust, bevy_ecs, `crates/engine`. Tests are `#[test]` fns in `crates/engine/src/tests/`.

**Spec:** `docs/superpowers/specs/2026-09-06-town-raids-and-hostile-patrols-design.md` — read Phase 7a (§1–§8) and "What already exists" before Task 1. The plan argues from the spec; both travel together.

## Global Constraints

- **Read the seams reference files first.** Before Task 2: `.claude/skills/seams/references/base.md` (raids, upkeep). Before Task 5: `.claude/skills/seams/references/screens.md` (the log, the town page) and `references/notifications.md`. Read the whole file, not a grep — the traps are cross-referenced.
- **Roll first, gate after.** Every new RNG gate follows `raid_check`'s discipline: a miss must leave the RNG stream untouched, or every seeded spawn test in the suite moves.
- **Uppercase for screen actions; lowercase letters are row selectors.** No new key is added by this phase, but the rule stands if one is proposed.
- **`cargo test --workspace`**, never `-p feral-processes-engine` alone for a final check — a single-crate run is a different build and shifts the RNG stream.
- **Never pipe cargo through `tail`/`grep`** for a pass/fail decision; the pipeline's exit code is not cargo's.
- **Do not update `docs/manual.md` or the root `README.md`.** `CHANGELOG.md` and `assets/*/README.md` schema docs still apply.
- **Commit freely; never push.** Landing is a separate, explicitly-requested step.
- Exact constant values are in Task 1 and are copied verbatim from the spec's §7 table.

---

### Task 1: The band query, the constants, and the compile-time invariant

**Files:**
- Modify: `crates/engine/src/tuning.rs` (append near `SETTLEMENT_GARRISON_MAX`, ~line 3502-3531)
- Modify: `crates/engine/src/settlements/relations.rs` (new query on `Standing`; new `const _` beside the one at `:86`; new census test in the existing `mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: `Standing::sends_raiders(self) -> bool`; the six `SETTLEMENT_RAID_*` constants named below. Tasks 2–4 use both.

- [ ] **Step 1: Write the failing census test**

In `crates/engine/src/settlements/relations.rs`, inside the existing `#[cfg(test)] mod tests`, beside `every_standing_band_answers_whether_it_preys_on_routes`:

```rust
    /// The same census for Phase 7a's town-sourced raids: every band
    /// answers, and only the bottom one sends anyone. `preys_on_routes`'
    /// shape rather than `garrison_defense`'s, because a magnitude here
    /// would be a second spelling of "is this town Hostile" — the
    /// frequency and the haul are tuning constants, not band answers.
    #[test]
    fn every_standing_band_answers_whether_it_sends_raiders() {
        for band in [
            Standing::Hostile,
            Standing::Cold,
            Standing::Neutral,
            Standing::Warm,
            Standing::Allied,
        ] {
            assert_eq!(
                band.sends_raiders(),
                band == Standing::Hostile,
                "{} answers the wrong way",
                band.label()
            );
        }
    }

    /// The runtime half of the `const _` beside `band`: a garrison alone,
    /// however many Allied neighbours the party collects, never drives the
    /// haul share to zero. Stated in both places on purpose — a reader of
    /// either file finds the claim, and the build fails before the suite
    /// does.
    #[test]
    fn a_garrison_alone_never_zeroes_a_town_raid() {
        let cut = crate::tuning::SETTLEMENT_GARRISON_MAX
            * crate::tuning::SETTLEMENT_RAID_DEFENSE_PER_POINT;
        assert!(
            cut < crate::tuning::SETTLEMENT_RAID_HAUL_PERCENT,
            "a maxed garrison cuts {cut} of {} points and would delete the mechanic",
            crate::tuning::SETTLEMENT_RAID_HAUL_PERCENT
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine sends_raiders 2>&1 | tail -30`
Expected: FAIL to **compile** — `no method named 'sends_raiders'` and `cannot find value 'SETTLEMENT_RAID_DEFENSE_PER_POINT'`.

- [ ] **Step 3: Add the tuning constants**

In `crates/engine/src/tuning.rs`, immediately after `SETTLEMENT_GARRISON_RADIUS`:

```rust
/// How often a Hostile neighbour tries the party's stores — `Game::town_raid_check`.
///
/// **Half `RAID_CHANCE_PER_TICK`**, so an angry neighbour raises total raid
/// pressure by half again rather than doubling it. The ambient sweep is
/// weather; this is somebody's decision, and it should be the rarer of the
/// two.
pub const SETTLEMENT_RAID_CHANCE_PER_TICK: f64 = 0.006;

/// How close a Hostile town has to be to the anchor to bother, in Chebyshev
/// tiles.
///
/// **Equal to `SETTLEMENT_GARRISON_RADIUS` today, and deliberately its own
/// constant.** The hostile half of the ladder must reach exactly as far as
/// the friendly half or one band's consequence is geometrically rarer than
/// the other's — but retuning aid must not silently retune hostility, so the
/// equality is an argued coincidence rather than a shared symbol. See
/// `docs/measurements/2026-09-05-settlement-aid-reach.md` for the 39%-of-
/// worlds figure this inherits.
pub const SETTLEMENT_RAID_RADIUS: i32 = crate::settlements::placement::REGION_TILES / 2;

/// The share of the party's banked build currency a raid takes, in percent.
pub const SETTLEMENT_RAID_HAUL_PERCENT: u32 = 10;

/// How many percentage points each point of `Game::total_raid_defense` cuts
/// off that share.
///
/// At 2, a maxed garrison (`SETTLEMENT_GARRISON_MAX`, 3) cuts 6 of the 10 —
/// real relief, never immunity, which is the claim the `const _` in
/// `settlements::relations` fails the *build* over. Two Shields plus a
/// garrison do reach zero, because that is a thing the player built.
pub const SETTLEMENT_RAID_DEFENSE_PER_POINT: u32 = 2;

/// The least a raid that lands takes.
///
/// A percentage of a small bank rounds to zero, and a raid that takes
/// nothing while still logging is the mechanic deleted rather than softened
/// — `SETTLEMENT_GARRISON_MAX`'s failure, from the other side. Applied only
/// after the share survives defense; see `Game::run_town_raid` for why that
/// order is load-bearing.
pub const SETTLEMENT_RAID_HAUL_FLOOR: u32 = 1;

/// The most a raid takes, whatever the bank holds.
///
/// A percentage of a large bank scales without limit, which would make
/// banking itself the punished behaviour. A rich base is bled, not gutted.
pub const SETTLEMENT_RAID_HAUL_CAP: u32 = 40;
```

- [ ] **Step 4: Add the query and the compile-time invariant**

In `crates/engine/src/settlements/relations.rs`, add beside the existing `const _` (currently `:86`):

```rust
/// A garrison can never zero a **town** raid either, however many Allied
/// neighbours the party collects — `SETTLEMENT_GARRISON_MAX`'s claim about
/// the ambient sweep, restated for the haul share.
///
/// A `const _` and not a test for the reason the assertion above gives:
/// closing this gap by retune must fail the *build*.
const _: () = assert!(
    crate::tuning::SETTLEMENT_GARRISON_MAX * crate::tuning::SETTLEMENT_RAID_DEFENSE_PER_POINT
        < crate::tuning::SETTLEMENT_RAID_HAUL_PERCENT
);
```

and inside `impl Standing`, after `preys_on_routes`:

```rust
    /// Whether a town at this band sends raiders at the party's base —
    /// Phase 7a, and the fourth consequence named by the module doc.
    ///
    /// Exhaustive, `refuses_service`'s reason. `Hostile` alone, and a
    /// boolean rather than `garrison_defense`'s magnitude: no band in the
    /// middle sends half a raider, so a ramp here would only be a second
    /// spelling of "is this town Hostile". How often and how much are
    /// `SETTLEMENT_RAID_CHANCE_PER_TICK` and `SETTLEMENT_RAID_HAUL_PERCENT`.
    pub fn sends_raiders(self) -> bool {
        match self {
            Standing::Hostile => true,
            Standing::Cold | Standing::Neutral | Standing::Warm | Standing::Allied => false,
        }
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine sends_raiders`
Run: `cargo test -p feral-processes-engine a_garrison_alone_never_zeroes`
Expected: both PASS.

- [ ] **Step 6: Prove the invariant is not vacuous**

Temporarily set `SETTLEMENT_RAID_DEFENSE_PER_POINT` to `4` and run `cargo build -p feral-processes-engine`. Expected: the build **fails** on the `const _`. Restore `2` and rebuild. Do not commit the temporary value.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/tuning.rs crates/engine/src/settlements/relations.rs
git commit -m "feat(settlements): Standing::sends_raiders and the town-raid tuning"
```

---

### Task 2: Which town raids

**Files:**
- Modify: `crates/engine/src/game/base/upkeep.rs` (new `raiding_towns`, beside `garrison_defense`/`town_garrisons` at `:258`)
- Test: `crates/engine/src/tests/settlement_relations.rs` (the file that already owns `town_near_anchor` and the garrison-radius tests)

**Interfaces:**
- Consumes: `Standing::sends_raiders` (Task 1); `SETTLEMENT_RAID_RADIUS` (Task 1).
- Produces: `Game::raiding_towns(&self) -> Vec<SettlementKey>` — every known town near enough to the anchor and angry enough to send anyone, in `Settlements`' `BTreeMap` order (deterministic, which Task 4's single draw relies on).

- [ ] **Step 1: Read `.claude/skills/seams/references/base.md`** — the whole file. Raids and upkeep live there.

- [ ] **Step 2: Write the failing tests**

Append to `crates/engine/src/tests/settlement_relations.rs`. These reuse the existing `game()`, `town_near_anchor` and `SettlementKey` helpers already in that file.

```rust
// ---------------------------------------------------------------------------
// Who sends raiders — three refusals, asserted separately
// ---------------------------------------------------------------------------

/// Three tests and not one, `commit_caravan_basket`'s rule: a single test
/// over one refusal passes against every other path that was never going to
/// fire anyway.
#[test]
fn a_hostile_town_beside_the_anchor_is_a_raid_source() {
    let mut game = game();
    town_near_anchor(
        &mut game,
        SettlementKey { rx: 1, ry: 0 },
        2,
        0,
        SETTLEMENT_HOSTILE_STANDING,
    );
    assert_eq!(game.raiding_towns(), vec![SettlementKey { rx: 1, ry: 0 }]);
}

#[test]
fn a_neutral_town_beside_the_anchor_sends_nobody() {
    let mut game = game();
    town_near_anchor(&mut game, SettlementKey { rx: 1, ry: 0 }, 2, 0, 0);
    assert!(game.raiding_towns().is_empty());
}

#[test]
fn a_hostile_town_beyond_the_raid_radius_sends_nobody() {
    let mut game = game();
    town_near_anchor(
        &mut game,
        SettlementKey { rx: 1, ry: 0 },
        SETTLEMENT_RAID_RADIUS + 1,
        0,
        SETTLEMENT_HOSTILE_STANDING,
    );
    assert!(game.raiding_towns().is_empty());
}

/// Hostility follows discovery, the rule `town_garrisons` already keeps for
/// aid: `Settlements` records a tile only once a town is *found*, and a
/// place the party has never met does not know where they live.
#[test]
fn a_hostile_town_never_found_sends_nobody() {
    let mut game = game();
    let key = SettlementKey { rx: 1, ry: 0 };
    set_standing(&mut game, key, SETTLEMENT_HOSTILE_STANDING);
    assert!(
        game.raiding_towns().is_empty(),
        "an unresolved town has no tile to measure from"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine raid_source raiding 2>&1 | tail -30`
Expected: FAIL to compile — `no method named 'raiding_towns'`.

- [ ] **Step 4: Implement `raiding_towns`**

In `crates/engine/src/game/base/upkeep.rs`, directly after `town_garrisons`:

```rust
    /// Every known town near enough to the anchor and angry enough to send
    /// raiders at it — the hostile mirror of `garrison_defense`'s fold, and
    /// deliberately a *list* rather than a count, because this event has an
    /// author and the log line has to name it.
    ///
    /// Two filters and a discovery rule. The radius is Chebyshev to the
    /// anchor, as the garrison's is. The band is asked through
    /// `Standing::sends_raiders`, never restated here. And a town whose
    /// tile has never been resolved is absent from `Settlements` entirely,
    /// so it is excluded by construction rather than by a third check —
    /// `town_garrisons`' rule, and the same reason: aid and hostility both
    /// follow discovery.
    ///
    /// Order is `Settlements`' own `BTreeMap` order, which is stable across
    /// a save round trip. `town_raid_check` picks from this with one draw
    /// and would otherwise be seed-unstable.
    pub(crate) fn raiding_towns(&self) -> Vec<crate::settlements::SettlementKey> {
        let Some((ax, ay)) = self.anchor_position() else {
            return Vec::new();
        };
        let near: Vec<crate::settlements::SettlementKey> = self
            .world
            .resource::<crate::resources::Settlements>()
            .0
            .iter()
            .filter(|(_, known)| {
                (known.tile.0 - ax).abs().max((known.tile.1 - ay).abs())
                    <= crate::tuning::SETTLEMENT_RAID_RADIUS
            })
            .map(|(key, _)| *key)
            .collect();
        near.into_iter()
            .filter(|&key| self.standing_band(key).sends_raiders())
            .collect()
    }
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine raid_source raiding sends_nobody`
Expected: all four PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/game/base/upkeep.rs crates/engine/src/tests/settlement_relations.rs
git commit -m "feat(settlements): raiding_towns — which neighbours are near enough and angry enough"
```

---

### Task 3: What a raid does

**Files:**
- Modify: `crates/engine/src/game/base/upkeep.rs` (new `run_town_raid` and `dev_force_town_raid`, after `run_raid`)
- Test: `crates/engine/src/tests/raids.rs` (the file that already owns `dev_force_raid` tests)

**Interfaces:**
- Consumes: `Game::raiding_towns` (Task 2); the `SETTLEMENT_RAID_HAUL_*` and `SETTLEMENT_RAID_DEFENSE_PER_POINT` constants (Task 1).
- Produces: `Game::run_town_raid(&mut self, key: SettlementKey)`; `Game::dev_force_town_raid(&mut self, key: SettlementKey)` (the `#[doc(hidden)]` console door, `dev_force_raid`'s precedent and the only way the suite fires a 0.006 event). `NotificationKind::FirstTownRaid` is **added in Task 5** — until then, `run_town_raid` fires `NotificationKind::FirstRaid` and Task 5 changes that one line.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/raids.rs`. `place_settlement`, `set_standing`-equivalents and `SettlementKey` come from `super::support::*` and `crate::*`, already imported by that file; add `use crate::settlements::SettlementKey;` and `use crate::resources::Standings;` if the file lacks them.

```rust
// ---------------------------------------------------------------------------
// Town-sourced raids: what one takes, and what turns it away
// ---------------------------------------------------------------------------

/// Puts a Hostile town `dx` east of the anchor and hands back its key.
fn hostile_neighbour(game: &mut Game, dx: i32) -> SettlementKey {
    let (ax, ay) = game.anchor_position().expect("a new game has an anchor");
    let key = SettlementKey { rx: 1, ry: 0 };
    place_settlement(game, key, ax + dx, ay);
    game.world
        .resource_mut::<Standings>()
        .0
        .entry(key)
        .or_default()
        .standing = SETTLEMENT_HOSTILE_STANDING;
    key
}

/// Gives the player `qty` of the build currency and returns what they hold.
fn stock_the_bank(game: &mut Game, qty: u32) -> u32 {
    let currency = game.currency();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .expect("the player carries an inventory")
        .add(currency.clone(), qty);
    game.banked(&currency)
}

#[test]
fn a_town_raid_carries_off_a_share_of_the_bank() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let key = hostile_neighbour(&mut game, 2);
    let before = stock_the_bank(&mut game, 200);
    assert_eq!(game.total_raid_defense(), 0, "fixture assumes no defense");

    game.dev_force_town_raid(key);

    let after = game.banked(&game.currency());
    let taken = before - after;
    assert_eq!(
        taken,
        before * SETTLEMENT_RAID_HAUL_PERCENT / 100,
        "the share is the whole rule when nothing is capped or floored"
    );
}

#[test]
fn a_town_raid_is_capped_however_rich_the_base_is() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let key = hostile_neighbour(&mut game, 2);
    let before = stock_the_bank(&mut game, 100_000);

    game.dev_force_town_raid(key);

    let taken = before - game.banked(&game.currency());
    assert_eq!(taken, SETTLEMENT_RAID_HAUL_CAP);
}

#[test]
fn a_town_raid_on_a_thin_bank_still_takes_something() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let key = hostile_neighbour(&mut game, 2);
    let before = stock_the_bank(&mut game, 3);
    assert_eq!(
        before * SETTLEMENT_RAID_HAUL_PERCENT / 100,
        0,
        "fixture must be thin enough that the share rounds away"
    );

    game.dev_force_town_raid(key);

    let taken = before - game.banked(&game.currency());
    assert_eq!(taken, SETTLEMENT_RAID_HAUL_FLOOR, "the floor is why this is not a no-op");
}

#[test]
fn a_town_raid_on_an_empty_store_finds_nothing_and_says_so() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let key = hostile_neighbour(&mut game, 2);
    let currency = game.currency();
    let player = game.player_entity();
    let held = game.banked(&currency);
    game.world
        .get_mut::<Inventory>(player)
        .expect("the player carries an inventory")
        .take(currency.clone(), held);

    game.dev_force_town_raid(key);

    assert_eq!(game.banked(&currency), 0);
    let line = &game.message_history(1)[0].text;
    assert!(
        line.contains("bare"),
        "an empty store gets its own line, not a haul of zero: {line}"
    );
}

/// The order in `run_town_raid` that this pins: the zero check runs *before*
/// the floor. Run the other way the floor hands raiders a unit anyway, the
/// shield network silently stops working, and the log still says it worked.
#[test]
fn enough_defense_turns_a_town_raid_away_and_the_floor_does_not_undo_it() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let key = hostile_neighbour(&mut game, 2);
    let before = stock_the_bank(&mut game, 200);
    let needed = SETTLEMENT_RAID_HAUL_PERCENT.div_ceil(SETTLEMENT_RAID_DEFENSE_PER_POINT);
    for i in 0..needed {
        game.world.spawn((
            Structure { kind: "shield".to_string() },
            Position { x: 40 + i as i32, y: 40 },
            Durability { hp: 30, max_hp: 30 },
        ));
    }
    assert!(
        game.total_raid_defense() * SETTLEMENT_RAID_DEFENSE_PER_POINT
            >= SETTLEMENT_RAID_HAUL_PERCENT,
        "fixture must actually reach the deflect threshold"
    );

    game.dev_force_town_raid(key);

    assert_eq!(game.banked(&game.currency()), before, "nothing was taken");
    assert_eq!(game.take_effects().len(), 1, "a deflect draws one effect");
}

/// A maxed garrison is real relief and never immunity — the runtime half of
/// the `const _`, asserted through the actual raid rather than the constants.
#[test]
fn a_garrison_alone_softens_a_town_raid_without_stopping_it() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let key = hostile_neighbour(&mut game, 2);
    let (ax, ay) = game.anchor_position().unwrap();
    for i in 0..4 {
        let ally = SettlementKey { rx: 2 + i, ry: 0 };
        place_settlement(&mut game, ally, ax + 2, ay + 2 + i);
        game.world
            .resource_mut::<Standings>()
            .0
            .entry(ally)
            .or_default()
            .standing = SETTLEMENT_ALLIED_STANDING;
    }
    assert_eq!(
        game.total_raid_defense(),
        SETTLEMENT_GARRISON_MAX,
        "the settlement half is clamped and no structure is standing"
    );
    let before = stock_the_bank(&mut game, 200);

    game.dev_force_town_raid(key);

    let taken = before - game.banked(&game.currency());
    assert!(taken > 0, "a garrison alone must never zero a town raid");
    assert!(
        taken < before * SETTLEMENT_RAID_HAUL_PERCENT / 100,
        "and it must actually soften one"
    );
}

#[test]
fn a_town_raid_names_the_town_that_sent_it() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let key = hostile_neighbour(&mut game, 2);
    stock_the_bank(&mut game, 200);
    let name = game.settlement_name(key);

    game.dev_force_town_raid(key);

    let line = &game.message_history(1)[0].text;
    assert!(line.contains(&name), "the line must have an author: {line}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine town_raid 2>&1 | tail -30`
Expected: FAIL to compile — `no method named 'dev_force_town_raid'`.

- [ ] **Step 3: Implement the resolver and the console door**

In `crates/engine/src/game/base/upkeep.rs`, after `run_raid`:

```rust
    /// Everything a town raid *is*, once it has been decided one happens.
    ///
    /// Split from the roll for `run_raid`'s reason: the console fires the
    /// real thing, and the decision stays with the one caller that should
    /// be making it.
    ///
    /// **It takes rather than breaks**, which is the whole of what makes
    /// this a different event from a sweep. The four economy roles are
    /// separate on purpose; raiders at the *base* take what the *base* runs
    /// on, so the cost is construction. Progression is earned by fighting
    /// and is deliberately untouched.
    pub(crate) fn run_town_raid(&mut self, key: crate::settlements::SettlementKey) {
        let name = self.settlement_name(key);
        // Before the outcome branches, `run_raid`'s placement: the lesson is
        // "something you did caused this", and a raid turned away is still a
        // raid that happened.
        self.notify(crate::notifications::NotificationKind::FirstRaid);

        let cut = self.total_raid_defense() * crate::tuning::SETTLEMENT_RAID_DEFENSE_PER_POINT;
        let percent = crate::tuning::SETTLEMENT_RAID_HAUL_PERCENT.saturating_sub(cut);
        // **Before the floor, and the order is load-bearing.** The floor
        // exists so a share of a *small bank* does not round to nothing; run
        // after a defense that already drove the share to zero it would hand
        // the raiders a unit anyway, delete the deflect outcome, and leave
        // the log claiming a shield network that had stopped working. Two
        // different zeroes, and only one of them is the floor's business.
        if percent == 0 {
            let anchor = self.world.resource::<crate::resources::AnchorEntity>().0;
            self.push_effect(anchor, EffectKind::Deflected);
            self.log_base_kind(
                MessageKind::Raid,
                format!("Raiders out of {name} probe your defences and turn back empty-handed."),
            );
            return;
        }

        let currency = self.currency();
        let money = self.item_name(&currency).to_string();
        let banked = self.banked(&currency);
        let want = (banked * percent / 100)
            .max(crate::tuning::SETTLEMENT_RAID_HAUL_FLOOR)
            .min(crate::tuning::SETTLEMENT_RAID_HAUL_CAP);
        let player = self.player_entity();
        // `Inventory::take` is the third bound: it takes what is there and
        // reports it, so an empty store is an outcome rather than an
        // underflow.
        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .map(|mut inv| inv.take(currency, want))
            .unwrap_or(0);

        if taken == 0 {
            self.log_base_kind(
                MessageKind::Raid,
                format!("Raiders out of {name} ransack your stores and find them bare."),
            );
            return;
        }
        self.log_base_kind(
            MessageKind::Raid,
            format!("Raiders out of {name} carry off {taken} {money} from your stores."),
        );
    }

    /// Fires a town raid now, skipping the roll — the dev console's door and
    /// the only way a test reaches a `SETTLEMENT_RAID_CHANCE_PER_TICK`
    /// event. `dev_force_raid`'s precedent exactly: it calls the real body,
    /// so the console cannot disagree with the game about the haul, the
    /// defense cut or the lines.
    ///
    /// Reachable only through the `FERAL_DEV_CONSOLE` gate.
    #[doc(hidden)]
    pub fn dev_force_town_raid(&mut self, key: crate::settlements::SettlementKey) {
        self.run_town_raid(key);
    }
```

Add `Inventory` to the file's `use crate::components::{...}` list if it is not already imported.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine town_raid`
Expected: all seven PASS.

- [ ] **Step 5: Prove the ordering test is not vacuous**

Temporarily move the `if percent == 0` block to *after* the `want` computation and re-run `enough_defense_turns_a_town_raid_away_and_the_floor_does_not_undo_it`. Expected: it FAILS. Restore the order. Do not commit the temporary change.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/game/base/upkeep.rs crates/engine/src/tests/raids.rs
git commit -m "feat(settlements): run_town_raid — raiders take from the stores, defense turns them away"
```

---

### Task 4: The roll, and the tick

**Files:**
- Modify: `crates/engine/src/game/base/upkeep.rs` (new `town_raid_check`, after `raid_check`)
- Modify: `crates/engine/src/game/turn.rs:236` (call it from `tick_inner`, immediately after `self.raid_check();`)
- Test: `crates/engine/src/tests/raids.rs`

**Interfaces:**
- Consumes: `Game::raiding_towns` (Task 2), `Game::run_town_raid` (Task 3), `SETTLEMENT_RAID_CHANCE_PER_TICK` (Task 1). **Also the two test fixtures Task 3 added to `crates/engine/src/tests/raids.rs`** — `hostile_neighbour(&mut Game, dx: i32) -> SettlementKey` and `stock_the_bank(&mut Game, qty: u32) -> u32`. Task 3 must land first; do not redefine them.
- Produces: `Game::town_raid_check(&mut self)` — `pub(crate)`, called once per `tick_inner`.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/raids.rs`:

```rust
/// The seam every raid gate in this file already respects: a miss must cost
/// exactly its own draw and nothing else, or every seeded spawn test in the
/// suite moves. Asserted by running the check on a world with no angry
/// neighbour at all and comparing the next draw against a world that never
/// ran it.
#[test]
fn a_town_raid_check_with_no_hostile_neighbour_costs_one_draw_and_no_more() {
    use rand::Rng;
    let mut ran = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut untouched = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(ran.raiding_towns().is_empty(), "fixture has no angry neighbour");

    ran.town_raid_check();
    // The one draw the check is allowed, replayed by hand on the control so
    // the two streams are compared from the same offset.
    {
        let mut rng = untouched.world.resource_mut::<crate::resources::GameRng>();
        let _ = rng.0.random_bool(SETTLEMENT_RAID_CHANCE_PER_TICK);
    }

    let a = ran.world.resource_mut::<crate::resources::GameRng>().0.random::<u64>();
    let b = untouched
        .world
        .resource_mut::<crate::resources::GameRng>()
        .0
        .random::<u64>();
    assert_eq!(a, b, "the gate drew more than its roll");
}

/// A raid the roll declined must not resolve — the gate is after the roll,
/// but it is still a gate.
#[test]
fn a_town_with_no_standing_problem_is_never_raided_however_the_roll_lands() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (ax, ay) = game.anchor_position().unwrap();
    let key = SettlementKey { rx: 1, ry: 0 };
    place_settlement(&mut game, key, ax + 2, ay);
    let before = stock_the_bank(&mut game, 500);

    for _ in 0..2000 {
        game.town_raid_check();
    }

    assert_eq!(
        game.banked(&game.currency()),
        before,
        "a Neutral neighbour never sends anyone, whatever the rolls did"
    );
}

/// And the positive: over enough ticks a Hostile neighbour does land one.
#[test]
fn a_hostile_neighbour_eventually_lands_a_raid() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let _key = hostile_neighbour(&mut game, 2);
    let before = stock_the_bank(&mut game, 5_000);

    for _ in 0..5_000 {
        game.town_raid_check();
    }

    assert!(
        game.banked(&game.currency()) < before,
        "5000 ticks at {SETTLEMENT_RAID_CHANCE_PER_TICK} should land many raids"
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p feral-processes-engine town_raid_check hostile_neighbour_eventually 2>&1 | tail -30`
Expected: FAIL to compile — `no method named 'town_raid_check'`.

- [ ] **Step 3: Implement the check**

In `crates/engine/src/game/base/upkeep.rs`, after `raid_check`:

```rust
    /// The roll for a town-sourced raid, and the one caller that decides one
    /// happens.
    ///
    /// **Roll first, gate after** — `raid_check`'s and
    /// `maybe_spawn_wild_creature`'s shared discipline. A world with no
    /// angry neighbour costs exactly one draw a tick and leaves every
    /// seeded spawn test where it was.
    ///
    /// **Neither `RAID_MIN_ZONE` nor `RAID_MIN_BASE_STAFF` applies, and both
    /// omissions are deliberate.** The zone floor exists so a player who has
    /// not engaged the game is not swept; reaching `Hostile` with a town
    /// near the anchor *is* engagement, and gating it on depth would make
    /// the consequence of a choice wait on an unrelated axis. The staff
    /// floor stops a base already reduced to wreckage from being ground
    /// down — but this raid breaks nothing, so there is no attrition spiral
    /// for it to prevent, and a base with nobody on shift is exactly the one
    /// whose stores are easiest to walk off with.
    pub(crate) fn town_raid_check(&mut self) {
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(crate::tuning::SETTLEMENT_RAID_CHANCE_PER_TICK)
        };
        if !roll {
            return;
        }
        let candidates = self.raiding_towns();
        if candidates.is_empty() {
            return;
        }
        // One draw, over an order `raiding_towns` documents as stable.
        let key = {
            let mut rng = self.world.resource_mut::<GameRng>();
            candidates[rng.0.random_range(0..candidates.len())]
        };
        self.run_town_raid(key);
    }
```

- [ ] **Step 4: Wire it into the tick**

In `crates/engine/src/game/turn.rs`, immediately after the existing `self.raid_check();` (currently `:236`):

```rust
        // Immediately after the ambient sweep, so the two raid sources
        // resolve in a fixed order and a tick that produces both reads as
        // two events rather than an interleaving. After `caravan_tick` for
        // the same reason `raid_check` is: a trader must not be caught
        // standing in something resolved the tick it arrives.
        self.town_raid_check();
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p feral-processes-engine town_raid`
Expected: all PASS.

- [ ] **Step 6: Run the whole suite — the RNG-stream check**

Run: `cargo test --workspace`
Expected: PASS. **A new per-tick draw shifts the RNG stream**, and this is the step that finds out. If seeded spawn or balance tests fail here, do **not** rewrite them to match: the roll-first discipline exists precisely so a miss costs nothing extra, so a failure means the gate drew before the roll or the call landed in the wrong place in `tick_inner`. Re-read Step 3 before touching a test.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/game/base/upkeep.rs crates/engine/src/game/turn.rs crates/engine/src/tests/raids.rs
git commit -m "feat(settlements): town_raid_check rolls each tick beside the ambient sweep"
```

---

### Task 5: The notification and the town page

**Files:**
- Modify: `crates/engine/src/notifications.rs` (new `FirstTownRaid` variant, `all()` array `11 → 12`, its `NotificationDef`, its `latch_key`)
- Modify: `crates/engine/src/game/base/upkeep.rs` (point `run_town_raid`'s `notify` at the new kind)
- Modify: `crates/engine/src/game/settlement_relations.rs` (two lines, `AID_LINES` `5 → 7`)
- Modify: `crates/engine/src/lib.rs:91` (re-export the two new constants)
- Test: `crates/engine/src/tests/settlement_relations.rs`

**Interfaces:**
- Consumes: `Game::raiding_towns` (Task 2) — the page asks it rather than restating the radius.
- Produces: `NotificationKind::FirstTownRaid`; `THREAT_RAIDERS` and `THREAT_RAID_REACH` string constants, both carried by `AID_LINES`.

**A deliberate call, recorded here:** the two threat lines join the existing `AID_LINES` array rather than getting a second census or a rename to `TOWN_PAGE_LINES`. **One census is the actual invariant** — the gui width gate measures one array, and a line missing from it is drawn past the right edge with nothing to catch it. Renaming would sweep `lib.rs`, `crates/gui/src/render/settlement.rs:218` and the tests, and would raise the question of renaming `SettlementView.aid` too, which is scope this phase was not given. The array's doc comment is updated to say it now measures the whole status block.

- [ ] **Step 1: Read `.claude/skills/seams/references/screens.md` and `references/notifications.md`** — both whole files.

- [ ] **Step 2: Write the failing tests**

Append to `crates/engine/src/tests/settlement_relations.rs`:

```rust
#[test]
fn a_hostile_neighbour_tells_the_page_it_sends_raiders() {
    let mut game = game();
    let key = SettlementKey { rx: 1, ry: 0 };
    town_near_anchor(&mut game, key, 2, 0, SETTLEMENT_HOSTILE_STANDING);
    let aid = game.settlement_report(key).aid;
    assert!(aid.contains(&THREAT_RAIDERS.to_string()), "got {aid:?}");
}

/// The radius is a per-run coin flip, so a page that says "raiders come from
/// here" about a town half the world away is a lie. The line is gated on the
/// same `raiding_towns` the check reads, never on a restated radius.
#[test]
fn a_hostile_town_too_far_to_raid_says_only_that_it_would() {
    let mut game = game();
    let key = SettlementKey { rx: 1, ry: 0 };
    town_near_anchor(
        &mut game,
        key,
        SETTLEMENT_RAID_RADIUS + 1,
        0,
        SETTLEMENT_HOSTILE_STANDING,
    );
    let aid = game.settlement_report(key).aid;
    assert!(!aid.contains(&THREAT_RAIDERS.to_string()), "got {aid:?}");
    assert!(aid.contains(&THREAT_RAID_REACH.to_string()), "got {aid:?}");
}

#[test]
fn a_neutral_town_still_says_nothing_about_raiders() {
    let mut game = game();
    let key = SettlementKey { rx: 1, ry: 0 };
    town_near_anchor(&mut game, key, 2, 0, 0);
    assert!(game.settlement_report(key).aid.is_empty());
}
```

The existing census test at `:1010` (`AID_LINES.contains(&line.as_str())`) already covers the new lines once they are emitted — **do not weaken it**; extend the band list it walks so a Hostile town is among the fixtures it drives.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p feral-processes-engine raiders 2>&1 | tail -30`
Expected: FAIL to compile — `cannot find value 'THREAT_RAIDERS'`.

- [ ] **Step 4: Add the two constants and extend the census**

In `crates/engine/src/game/settlement_relations.rs`, beside `AID_RELAY`:

```rust
/// See `AID_GARRISON`. The ladder's angry end: a Hostile town near enough to
/// the anchor sends raiders — `Game::raiding_towns`.
pub const THREAT_RAIDERS: &str = "Raiders out of here come for your stores.";
/// See `AID_GARRISON`. A Hostile town too far from the anchor to send
/// anybody. The distinction is worth a line of its own because
/// `SETTLEMENT_RAID_RADIUS` is a per-run coin flip, and a page silent about
/// distance reads as a page saying the band has no consequence at all.
pub const THREAT_RAID_REACH: &str = "They are too far from your base to trouble it.";
```

Change the census to carry them — the array is sized, so the count moves with it:

```rust
/// The census: every sentence above, for the layout gates to measure.
/// A line written by `Game::settlement_aid_lines` and missing here is a line
/// nothing measures, which is the failure this array exists to make
/// impossible.
///
/// **Named for the aid it began as; it measures the whole status block**,
/// threat lines included. One census is the invariant — the gui width gate
/// reads exactly one array, and a second list is the drift this comment's
/// first paragraph exists to prevent.
pub const AID_LINES: [&str; 7] = [
    AID_GARRISON,
    AID_GIFT_READY,
    AID_GIFT_SOON,
    AID_GIFT_LATER,
    AID_RELAY,
    THREAT_RAIDERS,
    THREAT_RAID_REACH,
];
```

- [ ] **Step 5: Emit them**

In `settlement_aid_lines`, after the `town_garrisons` block and before the reach-gated verbs:

```rust
        // The hostile mirror of the garrison line above, and a *call* to
        // `raiding_towns` rather than a restated radius — the same rule, and
        // the same reason: a page that keeps promising (or denying) an event
        // after the check learns a new condition is a page that lies.
        //
        // Not reach-gated. `[G]` and `[T]` are gated because they are doors
        // the player operates from one tile away; this is a fact about the
        // town that is true from wherever the page was opened.
        if band.sends_raiders() {
            if self.raiding_towns().contains(&key) {
                lines.push(THREAT_RAIDERS.to_string());
            } else {
                lines.push(THREAT_RAID_REACH.to_string());
            }
        }
```

- [ ] **Step 6: Re-export the constants**

In `crates/engine/src/lib.rs:91`, add `THREAT_RAIDERS, THREAT_RAID_REACH` to the existing `pub use` list beside `AID_LINES`, keeping alphabetical order if the list is sorted.

- [ ] **Step 7: Add the notification**

In `crates/engine/src/notifications.rs`, four edits:

```rust
    /// Raiders out of a Hostile town reach the base — `Game::run_town_raid`.
    /// Distinct from `FirstRaid`: the sweep is weather and this is a
    /// consequence, and firing the sweep's copy here would teach the wrong
    /// lesson.
    FirstTownRaid,
```

`all()` becomes `[NotificationKind; 12]` with `NotificationKind::FirstTownRaid` added to the array. Then the def:

```rust
            NotificationKind::FirstTownRaid => NotificationDef {
                title: "They Know Where You Live",
                body: "A settlement you have wronged has sent people for your stores. They do not \
                       come to break your machines — they come to carry off what your base runs \
                       on, and they will keep coming while the grudge stands.\n\nWhat you have \
                       built to turn a sweep away turns them away too, and so does a neighbour \
                       who likes you. Standing can be repaired.",
                sprite: None,
                glyph: '!',
                color: GlyphColor::Red,
                repeat: Repeat::OnceEver,
            },
```

and the latch key, beside `FirstRaid`'s:

```rust
            NotificationKind::FirstTownRaid => "tutorial_first_town_raid",
```

Then change `run_town_raid`'s `self.notify(...)` from `FirstRaid` to `FirstTownRaid`.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p feral-processes-engine raiders threat notification`
Expected: PASS. If the notification census fails with a length mismatch, `all()`'s array size was not moved to 12.

- [ ] **Step 9: Run the gui layout gates**

Run: `cargo test -p feral-processes-gui settlement`
Expected: PASS. If a width census fails, the two new sentences are longer than the page measures — **shorten the sentence, do not widen the gate**; the page has no scroll.

- [ ] **Step 10: Commit**

```bash
git add crates/engine/src/notifications.rs crates/engine/src/game/settlement_relations.rs crates/engine/src/game/base/upkeep.rs crates/engine/src/lib.rs crates/engine/src/tests/settlement_relations.rs
git commit -m "feat(settlements): the town page names the threat, and a first town raid teaches it"
```

---

### Task 6: Full verification and the release

**Files:**
- Modify: `CHANGELOG.md`
- Modify: the workspace version (`Cargo.toml` files carrying the workspace version)

**Interfaces:**
- Consumes: everything above.
- Produces: a tagged release.

- [ ] **Step 1: Run the whole suite, clean**

Run: `cargo test --workspace`
Expected: PASS, with no failures. Record the pass/fail counts. **Do not pipe this through `tail` or `grep` when deciding whether it passed** — the pipeline's exit code is not cargo's. Note that `an_unspent_allowance_holds_its_step` in app-core is a known flake on an unmodified binary; if it is the only failure, re-run before investigating.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 3: Write the CHANGELOG section**

Read the top two existing sections first and match their heading form and voice exactly. The content, with `<version>` and the date filled in:

```markdown
## [<version>] - 2026-09-06

### Added

- **Town-sourced raids.** A settlement at `Hostile` standing within half a
  region of your base now sends raiders of its own. Unlike a GC Entropy
  Sweep, they do not break your machines — they carry off a share of the
  banked salvage your base runs on, and the log line names the town that
  sent them.
- Everything that turns a sweep away turns raiders away too: structures with
  raid defense, and the garrison an allied neighbour stations. A maxed
  garrison alone softens a raid and can never stop one; a shield network can.
  Hostile and allied neighbours now meet on one number.
- The town page says whether a hostile settlement is near enough to your base
  to trouble it, since the distance is a per-run coin flip.
- A first-town-raid notification, distinct from the sweep's: the sweep is
  weather, this is a consequence.

Design: `docs/superpowers/specs/2026-09-06-town-raids-and-hostile-patrols-design.md`
(Phase 7a). Hostile patrols are Phase 7b and are not in this release.
```

- [ ] **Step 4: Bump the version**

A **patch** bump. Per the repo's rule, a minor bump moves the internal
dependency requirements and reads as a broken workspace; a patch bump does
not. Find every place the version is written and move them together:

Run: `rg -n '^version = ' Cargo.toml crates/*/Cargo.toml`

Change the workspace version and any crate that pins it literally rather than
inheriting with `version.workspace = true`. Then confirm the tree still
builds: `cargo check --workspace`.

- [ ] **Step 5: Grep for claims this change falsifies**

Run: `rg -n 'GC Entropy Sweep|raid' docs/ --glob '*.md' -l`
Check each hit for a sentence that is now wrong (e.g. "raids are the only thing that damages the base", "nothing in the game has an author"). Fix what this change falsified. **Do not touch `docs/manual.md` or the root `README.md`** — both are carved out.

- [ ] **Step 6: Commit, then stop**

```bash
git add -- CHANGELOG.md Cargo.toml crates
git commit -m "release: <version>"
```

Stage explicit paths, never `git add -A` — another agent's worktree gitlink under `.claude/worktrees/` gets swept up otherwise.

**Do not merge, tag or push.** Landing is `skills:deploy`'s sequence and needs an explicit ask.

---

## Notes for the executor

- **This will not have been played.** The settlements path has landed six phases and a green suite is not evidence of play. Every constant in Task 1 is a guess no instrument in this repo can check — `balance_sim` models no raids, no towns and no loot. Say so when reporting completion; do not describe the numbers as tuned.
- **Phase 7b (hostile patrols) is a separate branch** and is not in this plan. Do not start it.
- If a test in Task 3 or 4 seems to pass too easily, delete the implementation line it covers and confirm it fails. Two tests written on 2026-08-09 were vacuous and read as coverage for weeks.
