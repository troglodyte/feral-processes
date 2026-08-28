# Sorties (engine) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A squad of base staff can be dispatched from a Relay to a named
site, where it fights a run of real battles off-screen over a number of ticks
and returns with loot, or with fewer bodies than it left with.

**Architecture:** A fourth `ProgramRole` variant takes an away program out of
the base's scheduler, drift, entropy and map by *omission* rather than by four
new checks. The board of offers is derived per read and never stored, on the
Broker board's pattern. Each battle resolves **atomically inside a single
tick** — spawn, fight, despawn — so no bevy system ever observes the
opposition, which is what keeps the feature out of the "which space is this?"
bug class. Fights use the real swing and ability doors, both of which are
already `BattleState`-free.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, serde.

**Spec:** `docs/superpowers/specs/2026-08-28-sorties-design.md` — read it
before Task 1. This plan argues from it and does not restate its reasoning.

**Scope:** This plan is the **engine only**, and is complete and testable on
its own: everything below is exercised headlessly by `cargo test -p
feral-processes-engine`. The app-core `Mode` and the gui screens are a second
plan (`2026-08-28-sorties-screens.md`), written after this one lands.

## Global Constraints

Copied verbatim from `CLAUDE.md` and the spec. Every task's requirements
implicitly include these.

- **The renderer never touches the ECS `World`.** Everything here is behind
  `Game`.
- **No new content hardcoded in Rust.** Sites are `.ron` files; the Relay is a
  `.ron` structure; its gate is a `.ron` research node.
- **A malformed `.ron` file is skipped with a logged warning, never a panic.**
  Follow `SpeciesDb::load_dir`.
- **An absent asset directory loads silently empty**, and no system or screen
  is ever gated on the catalogue being non-empty.
- **New tuning values go in `crates/engine/src/tuning.rs`** as documented
  `pub const`, never inline in a formula.
- **Additive save fields go behind `#[serde(default)]` and cost no
  `SAVE_FORMAT_VERSION` bump.** Do not bump it in this plan.
- **No flaky tests.** No `sleep()`, no wall-clock dependence, no unseeded RNG.
  Background systems will interfere with a naive assertion.
- **Full suite is the final gate:** `cargo test --workspace`. Run
  `cargo fmt` and `cargo clippy --workspace` after every task and fix
  warnings rather than silencing them.
- **Commit at every green step.** Branch is `feat/expedition-groups`; check
  `git branch --show-current` before every commit.
- **Do not push.** Landing is the user's call.

## Vocabulary

The word is **sortie** throughout — `Sortie`, `ProgramRole::Sortie`,
`assets/sorties/`, `SORTIE_*`. It is deliberately not `expedition`, which
already means the player's own outing from base in prose in `difficulty.rs`,
`components.rs` and the Repair Bay spec. **Leave those existing uses alone;
they are correct as written.**

## File Structure

| File | Responsibility |
|---|---|
| `crates/engine/src/sorties.rs` (new) | `SortieDef`, `SortieDb::load_dir`. The catalogue only — no game logic. |
| `crates/engine/src/game/sortie.rs` (new) | `Game` methods: reach, board, dispatch, the trip, return. The whole feature's behaviour. |
| `crates/engine/src/resources.rs` | `Sortie` runtime record + `Sorties` resource. |
| `crates/engine/src/game/party.rs` | `ProgramRole::Sortie`, widened `role_of`. |
| `crates/engine/src/game/spawning.rs` | `habitat_pools` widened by a step offset. |
| `crates/engine/src/save.rs` | `SortieSave`, `PlayerSave::sorties`, `CreatureSave::sortie_index`. |
| `crates/engine/src/tuning.rs` | The nine `SORTIE_*` constants. |
| `crates/engine/src/tests/sorties.rs` (new) | Every test in this plan. |
| `assets/sorties/*.ron` + `README.md` (new) | The site catalogue and its schema doc. |
| `assets/structures/relay.ron` (new) | The Relay. |
| `assets/research/dispatch.ron` (new) | The gate. |

`game/sortie.rs` is a new module rather than an addition to an existing one
because it is a self-contained subsystem with one public surface, and because
`game/base/work_orders.rs` is already large.

---

### Task 1: The role

**Files:**
- Modify: `crates/engine/src/resources.rs`
- Modify: `crates/engine/src/game/party.rs:19-56` (enum + `role_of`), `:533`
- Modify: `crates/engine/src/game/base/entropy.rs:77`
- Modify: `crates/engine/src/systems.rs:107`
- Test: `crates/engine/src/tests/sorties.rs` (new), registered in
  `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `resources::Sortie` — runtime record, fields listed below. Not
    `Serialize`; the save form is Task 8.
  - `resources::Sorties(pub Vec<Sortie>)`, a bevy `Resource`, `Default`.
  - `Sorties::contains(&self, e: Entity) -> bool` — the only accessor this
    plan needs. Do not add a `members()` iterator until something consumes
    it; an unused one is a clippy dead-code warning, and this plan forbids
    silencing those.
  - `ProgramRole::Sortie`
  - `party::role_of(creature, owner, player, party, wielded, sorties) -> Option<ProgramRole>`

- [ ] **Step 1: Write the failing test**

Add to `crates/engine/src/tests/sorties.rs` and declare
`mod sorties;` in `crates/engine/src/tests/mod.rs`:

```rust
use super::support::{stand_in_base, test_assets_dir};
use crate::components::{Position, Stats, Tamed};
use crate::difficulty::DifficultyMode;
use crate::game::party::ProgramRole;
use crate::resources::{Sorties, Sortie};
use crate::Game;

/// A program named by an in-flight sortie is `Sortie`, not `Staff` — and
/// `Staff` stays the leftover rather than becoming something assigned.
#[test]
fn a_dispatched_program_is_not_staff() {
    let mut game = Game::new(4200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let program = game
        .adopt_program("generic", 0, 0, 1.0)
        .expect("test roster program");

    assert_eq!(game.program_role(program), Some(ProgramRole::Staff));

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));

    assert_eq!(
        game.program_role(program),
        Some(ProgramRole::Sortie),
        "a program named by an in-flight sortie has left the labour pool"
    );
}

/// The map and the examine ray both drop an away program, and neither
/// needed a new rule: `position_is_honest` tests for `Staff` exactly.
#[test]
fn a_dispatched_program_leaves_the_map() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let program = game
        .adopt_program("generic", 0, 0, 1.0)
        .expect("test roster program");

    assert!(game.position_is_honest(program), "idle staff are drawn");

    game.world
        .resource_mut::<Sorties>()
        .0
        .push(Sortie::test_stub(vec![program]));

    assert!(
        !game.position_is_honest(program),
        "an away program must not claim a tile it is not standing on"
    );
}
```

`Sortie::test_stub` is a `#[cfg(test)]` constructor added in Step 3 — a
fixture that hand-builds the struct will silently rot when fields are added,
which is `work_node_parts()`' rule.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: FAIL — `Sorties`, `Sortie`, `ProgramRole::Sortie` do not exist.

- [ ] **Step 3: Implement**

In `crates/engine/src/resources.rs`:

```rust
/// A squad of base staff currently away from the base.
///
/// **The site is stored resolved, never as an id or a board index.** A board
/// that rotates while the squad is out, or an `assets/sorties/` file edited
/// between sessions, must not be able to rewrite or strand a trip already in
/// flight — `ActiveContract` stores a whole `ContractDef` for exactly this
/// reason.
#[derive(Clone, Debug)]
pub struct Sortie {
    pub site: crate::sorties::SortieDef,
    /// Steps above the zone baseline this site was offered at. Kept beside
    /// the def because the board applies it, not the file.
    pub risk: u32,
    pub members: Vec<Entity>,
    pub ticks_total: u64,
    pub ticks_elapsed: u64,
    pub battles_total: u32,
    pub battles_done: u32,
    /// Set when a member goes down. Remaining battles are skipped; the
    /// return travel still runs.
    pub aborted: bool,
    pub loot: Vec<(crate::items::ItemId, u32)>,
    pub xp: u32,
    pub kills: u32,
}

/// Every sortie currently away. Saved; see `save::SortieSave`.
#[derive(Resource, Default, Clone, Debug)]
pub struct Sorties(pub Vec<Sortie>);

impl Sorties {
    pub(crate) fn contains(&self, creature: Entity) -> bool {
        self.0.iter().any(|s| s.members.contains(&creature))
    }

}
```

Add the `#[cfg(test)]` stub next to it:

```rust
#[cfg(test)]
impl Sortie {
    /// A minimal in-flight record for tests that only care about
    /// membership. Goes through the real struct so a new field is a
    /// compile error here rather than a silently unset default.
    pub(crate) fn test_stub(members: Vec<Entity>) -> Self {
        Self {
            site: crate::sorties::SortieDef::test_stub(),
            risk: 0,
            members,
            ticks_total: 100,
            ticks_elapsed: 0,
            battles_total: 1,
            battles_done: 0,
            aborted: false,
            loot: Vec::new(),
            xp: 0,
            kills: 0,
        }
    }
}
```

This depends on `SortieDef`, which is Task 2. **Write Task 2's `SortieDef`
struct and its `test_stub` first if you are executing strictly in order** —
or land Task 1 with a placeholder-free minimal `SortieDef` and let Task 2
add its loader. Prefer the former.

Register the resource wherever `Game::new` inserts the other defaulted
resources (search for `insert_resource(PopulatedChunks` and follow it).

In `crates/engine/src/game/party.rs`, add the variant **between `InParty`
and `Staff`**:

```rust
    /// Away from the base on a sortie — `resources::Sorties`. Ranked
    /// between the party and the labour pool: a dispatched program is not
    /// staff, which is what takes it out of `schedule_base_labour`,
    /// `drift_idle_staff`, `base_entropy_system` and the surface map in one
    /// edit rather than four.
    Sortie,
```

and widen the rule:

```rust
pub(crate) fn role_of(
    creature: Entity,
    owner: Entity,
    player: Entity,
    party: &Party,
    wielded: Option<Entity>,
    sorties: &Sorties,
) -> Option<ProgramRole> {
    if owner != player {
        return None;
    }
    if wielded == Some(creature) {
        return Some(ProgramRole::Wielded);
    }
    if party.0.contains(&creature) {
        return Some(ProgramRole::InParty);
    }
    if sorties.contains(creature) {
        return Some(ProgramRole::Sortie);
    }
    Some(ProgramRole::Staff)
}
```

The three call sites will now fail to compile. **That is the point** — fix
each by reading `Sorties` alongside the resources it already reads:

- `crates/engine/src/game/party.rs:533` (`Game::program_role`)
- `crates/engine/src/game/base/entropy.rs:77` — add `sorties: Res<Sorties>`
  to the system signature
- `crates/engine/src/systems.rs:107` — same

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: PASS (2 tests)

- [ ] **Step 5: Check nothing else moved**

Run: `cargo test -p feral-processes-engine`
Expected: PASS. A failure in an untouched subsystem here is most likely a
**latent unsorted-query test** — registering a new `Resource` shifts bevy's
query iteration order. Read the failing test before assuming a regression.

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/resources.rs crates/engine/src/game/party.rs \
        crates/engine/src/game/base/entropy.rs crates/engine/src/systems.rs \
        crates/engine/src/tests/sorties.rs crates/engine/src/tests/mod.rs
git commit -m "feat(sortie): a dispatched program is not base staff"
```

---

### Task 2: The site catalogue

**Files:**
- Create: `crates/engine/src/sorties.rs`, declared in `crates/engine/src/lib.rs`
- Create: `assets/sorties/README.md`, and four site files
- Test: `crates/engine/src/tests/sorties.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `sorties::SortieId(pub String)` — a string newtype, `ItemId`'s idiom
  - `sorties::SortieDef { id, name, description, risk: u32, battles_min: u32, battles_max: u32 }`
  - `sorties::SortieDb::load_dir(dir: &Path) -> (SortieDb, Vec<String>)` — db
    and warnings, `SpeciesDb::load_dir`'s shape
  - `SortieDb::iter(&self) -> impl Iterator<Item = &SortieDef>` — **sorted by
    id**, because every caller walks it
  - `SortieDb::is_empty(&self) -> bool`

- [ ] **Step 1: Write the failing test**

```rust
use crate::sorties::SortieDb;

/// An absent directory is a supported install: it loads empty, warns about
/// nothing, and the feature is simply absent. `NeedDb` and `MemoryDb`'s
/// property, and the reason nothing may ever gate on the db being
/// non-empty.
#[test]
fn an_absent_catalogue_loads_empty_and_quiet() {
    let (db, warnings) = SortieDb::load_dir(std::path::Path::new("/nonexistent/sorties"));
    assert!(db.is_empty());
    assert!(warnings.is_empty(), "an absent directory is not a fault");
}

/// A malformed file is skipped with a warning, never a panic that takes
/// startup down with it.
#[test]
fn a_malformed_site_is_skipped_with_a_warning() {
    let scratch = super::support::scratch_assets_dir("sortie_malformed");
    let dir = scratch.path().join("sorties");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("broken.ron"), "(this is not ron").unwrap();
    std::fs::write(
        dir.join("good.ron"),
        r#"(
    id: "good",
    name: "A Good Site",
    description: "Fine.",
    risk: 0,
    battles_min: 4,
    battles_max: 6,
)"#,
    )
    .unwrap();

    let (db, warnings) = SortieDb::load_dir(&dir);
    assert_eq!(db.iter().count(), 1, "the good file still loads");
    assert_eq!(warnings.len(), 1, "the broken one is reported, not fatal");
}

/// Sorted by id, because every caller walks it and an unsorted walk is how
/// a seeded board stops being reproducible.
#[test]
fn the_catalogue_iterates_in_id_order() {
    let (db, _) = SortieDb::load_dir(&super::support::test_assets_dir().join("sorties"));
    let ids: Vec<&str> = db.iter().map(|d| d.id.0.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
}

/// A site whose battle range is inverted or empty is a content fault and is
/// refused at load, the way `field_buff_duration_mismatch` refuses its
/// corners — a `battles_max` below `battles_min` would silently roll an
/// empty range at board time, far from the file that caused it.
#[test]
fn an_inverted_battle_range_is_refused_at_load() {
    let scratch = super::support::scratch_assets_dir("sortie_inverted");
    let dir = scratch.path().join("sorties");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("backwards.ron"),
        r#"(
    id: "backwards",
    name: "Backwards",
    description: "Bad.",
    risk: 1,
    battles_min: 9,
    battles_max: 3,
)"#,
    )
    .unwrap();

    let (db, warnings) = SortieDb::load_dir(&dir);
    assert!(db.is_empty());
    assert_eq!(warnings.len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: FAIL — `crate::sorties` does not exist.

- [ ] **Step 3: Implement**

Create `crates/engine/src/sorties.rs`, following `crates/engine/src/needs.rs`
as the closest existing loader (read it first — match its warning strings and
its directory-absent branch exactly rather than inventing a second style):

```rust
//! The catalogue of places a sortie can be sent.
//!
//! Data only. A site says what it is called, how far above the zone
//! baseline it sits, and how many fights it takes — never how long it
//! takes, which `Game::sortie_duration` derives, nor what it pays, which
//! falls out of the fights actually had.

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SortieId(pub String);

#[derive(Clone, Debug, serde::Deserialize)]
pub struct SortieDef {
    pub id: SortieId,
    pub name: String,
    pub description: String,
    /// Steps **above the zone baseline**, never an absolute danger band.
    /// The opposition is drawn at `danger_steps(None) + risk`, so a site
    /// stays as dangerous relative to the run in zone 9 as in zone 1.
    #[serde(default)]
    pub risk: u32,
    pub battles_min: u32,
    pub battles_max: u32,
}
```

`SortieDb` holds `Vec<SortieDef>` sorted by id at load. `load_dir` returns
`(SortieDb, Vec<String>)`; a missing directory returns an empty db and **no**
warnings; a file that fails to parse, or whose `battles_max < battles_min`,
or whose `battles_min == 0`, is skipped with one warning naming the file.

Add `#[cfg(test)] impl SortieDef { pub(crate) fn test_stub() -> Self }`
returning id `"stub"`, risk 0, battles 1..=1.

Declare `pub mod sorties;` in `crates/engine/src/lib.rs`, load it in
`Game::new` beside the other dbs, and insert it as a resource.

Write `assets/sorties/README.md` documenting all six fields, the risk-offset
semantics, and the two load-time refusals. Write four site files spanning
risk 0, 1, 1 and 2 with battle ranges of roughly 5-7, 10-14, 10-14 and
18-22. Names are content; keep them in the setting's register and out of the
occult vocabulary `CLAUDE.md` forbids.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: PASS (6 tests — the two from Task 1 and four here)

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/sorties.rs crates/engine/src/lib.rs \
        crates/engine/src/tests/sorties.rs assets/sorties/
git commit -m "feat(sortie): the site catalogue"
```

---

### Task 3: Tuning constants

**Files:**
- Modify: `crates/engine/src/tuning.rs`

**Interfaces:**
- Produces: the nine constants below, all `pub const`.

- [ ] **Step 1: Add the section**

There is no test for a constants block; the tests that read them are Tasks
4-8. Add one labelled section, matching the file's existing comment density —
each constant gets a doc comment saying what it is and what breaks if it
moves.

```rust
// ---------------------------------------------------------------- sorties

/// Ticks of travel every sortie pays regardless of where it is going.
pub const SORTIE_TRAVEL_BASE_TICKS: u64 = 150;

/// Extra travel ticks per step of a site's risk **offset** — never its
/// absolute danger band. Read against the absolute band, every trip in a
/// deep zone would take enormously longer for no reason the player could
/// name, and the feature would quietly stop being usable late in a run.
pub const SORTIE_TRAVEL_PER_RISK_TICKS: u64 = 75;

/// Ticks each battle adds to a trip. Travel deliberately dominates: a fight
/// is quick and getting there is not.
pub const SORTIE_TICKS_PER_BATTLE: u64 = 20;

/// How long one board of offers stands before it rotates. **Longer than the
/// longest trip**, so a board cannot rotate twice while the player is
/// deliberating over it.
pub const SORTIE_BOARD_ROTATION_TICKS: u64 = 1200;

/// Offers on a board.
pub const SORTIE_BOARD_SLOTS: usize = 3;

/// The board's own salt. Its own constant and never a reused one, following
/// `CARAVAN_SALT`.
pub const SORTIE_SALT: u64 = 0xE7ED_1710_5EED_0003;

/// A program below this fraction of max Integrity is refused at dispatch.
/// Sending a hurt program on a twenty-fight trip is the mistake the
/// abort-on-first-casualty rule cannot save you from, because it fires on
/// the first battle.
pub const SORTIE_MIN_HP_FRACTION: f32 = 0.5;

/// Fraction of `max_hp` restored to each member between battles, paid for by
/// the provisioning charged at dispatch. A **fraction** rather than flat HP,
/// so provisioning keeps meaning something at the level cap.
pub const SORTIE_PROVISION_HEAL_FRACTION: f32 = 0.15;

/// What a sortie kill pays against what the same kill pays with the player
/// in the fight. Below 1.0 deliberately: this is the one *tuned* lever on
/// the yield, and it exists so the cap can move without disturbing the two
/// mechanisms that earn it — Power not recovering in the field, and no rest
/// out there.
pub const SORTIE_XP_MULTIPLIER: f32 = 0.6;
```

- [ ] **Step 2: Verify it compiles and the balance gate is unmoved**

Run: `cargo test -p feral-processes-engine balance_sim`
Expected: PASS, unchanged. Nothing reads these yet; a moved curve here means
you edited something else.

- [ ] **Step 3: Commit**

```bash
cargo fmt && git add crates/engine/src/tuning.rs
git commit -m "feat(sortie): tuning constants"
```

---

### Task 4: Widen `habitat_pools` by a risk offset

**Files:**
- Modify: `crates/engine/src/game/spawning.rs:1042-1052`
- Modify: callers at `game/stack_features.rs:522`, `game/contracts.rs:354`,
  `game/spawning.rs:985`, `game/spawning.rs:1168`
- Modify: test callers at `tests/spawning.rs:458, 2837, 2862, 2889, 2890`
- Test: `crates/engine/src/tests/sorties.rs`

**Interfaces:**
- Produces:
  `Game::habitat_pools(&mut self, x: i32, y: i32, depth: Option<u32>, step_bonus: u32) -> Option<(Vec<String>, Vec<String>)>`

`CLAUDE.md` names this explicitly: *"`Game::habitat_pools` is the shared seam
— widen it rather than copying the biome rules."* Do not add a second
pool-building function.

- [ ] **Step 1: Write the failing test**

```rust
/// A risk offset reaches the same window `depth` does, so a sortie can ask
/// for tougher opposition without the caller re-deriving the biome rules.
#[test]
fn a_risk_offset_raises_the_habitat_window() {
    let mut game = Game::new(4300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(crate::resources::ZoneLevel(1));
    let (x, y) = (0, 0);

    let Some((base, _)) = game.habitat_pools(x, y, None, 0) else {
        panic!("zone 1 origin should be habitable");
    };
    let Some((raised, _)) = game.habitat_pools(x, y, None, 4) else {
        panic!("a raised window should still resolve");
    };

    assert_ne!(
        base, raised,
        "a four-step offset must move the window, or the parameter is inert"
    );
}

/// Zero is exactly today's behaviour, which is what lets every existing
/// caller pass it and nothing move.
#[test]
fn a_zero_offset_is_todays_window() {
    let mut game = Game::new(4301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(crate::resources::ZoneLevel(3));
    let a = game.habitat_pools(0, 0, None, 0);
    game.world.insert_resource(crate::resources::ZoneLevel(3));
    let b = game.habitat_pools(0, 0, None, 0);
    assert_eq!(a, b);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p feral-processes-engine sorties::a_risk_offset`
Expected: FAIL — `habitat_pools` takes three arguments.

- [ ] **Step 3: Implement**

Add the parameter and fold it into the step:

```rust
    pub(crate) fn habitat_pools(
        &mut self,
        x: i32,
        y: i32,
        depth: Option<u32>,
        step_bonus: u32,
    ) -> Option<(Vec<String>, Vec<String>)> {
        let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        if !tile.walkable {
            return None;
        }
        let step = self.danger_steps(depth).saturating_add(step_bonus);
```

Pass `0` at all four existing call sites and all five test call sites. **Do
not** change any other behaviour in this task.

- [ ] **Step 4: Run tests**

Run: `cargo test -p feral-processes-engine`
Expected: PASS. Every spawning test must be unchanged — a moved spawn test
means the zero case is not actually today's behaviour.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/game/ crates/engine/src/tests/
git commit -m "feat(sortie): habitat_pools takes a risk offset"
```

---

### Task 5: The Relay, its gate, and reach

**Files:**
- Create: `assets/structures/relay.ron`
- Create: `assets/research/dispatch.ron`
- Create: `crates/engine/src/game/sortie.rs`, declared in `crates/engine/src/game/mod.rs`
- Test: `crates/engine/src/tests/sorties.rs`

**Interfaces:**
- Produces:
  - `game::sortie::SortieReach { NoRelay, OffBase, AtRelay }` — `PartialEq`, `Debug`
  - `Game::sortie_reach(&mut self) -> SortieReach`

- [ ] **Step 1: Write the failing test**

```rust
use crate::game::sortie::SortieReach;

/// Three states rather than two booleans, `NoPost::BoxedIn`'s rule: "no
/// Relay built" and "not standing in base" leave the player different
/// errands, and a screen that cannot tell them apart says the wrong
/// sentence.
#[test]
fn sortie_reach_reports_the_three_states() {
    let mut game = Game::new(4400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.sortie_reach(), SortieReach::NoRelay);

    super::support::stand_in_base(&mut game);
    deploy_relay(&mut game);
    assert_eq!(game.sortie_reach(), SortieReach::AtRelay);

    // Out of base space entirely.
    game.world
        .insert_resource(crate::resources::Locale::Surface);
    assert_eq!(game.sortie_reach(), SortieReach::OffBase);
}
```

`deploy_relay` is a local helper in the test module that stands a Home up and
then a Relay, following whatever `deploy_broker` in
`crates/engine/src/tests/contracts.rs` does — **read that function and mirror
it**. A fixture that stands a Relay up without a Home does not survive a save
the test later loads.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p feral-processes-engine sortie_reach`
Expected: FAIL — `game::sortie` does not exist.

- [ ] **Step 3: Implement**

`assets/structures/relay.ron` — a normal `StructureDef` with `id: "relay"`, a
name, a description, a glyph and colour that no other structure uses, and a
`build_cost` in the current zone's material. No `work`, no `assembles`, no
`stores`.

`assets/research/dispatch.ron`:

```ron
(
    id: "dispatch",
    name: "Dispatch Protocol",
    description: "Deploy a Relay, and send idle programs out on sorties.",
    cost: <in line with neighbouring nodes>,
    min_zone: 2,
    requires: [<an existing early node>],
    unlocks_structures: ["relay"],
)
```

Read three existing files in `assets/research/` first and match their `cost`
scale and `requires` chain. `unlocks_structures` already exists on
`ResearchDef` (`research.rs:57`), so **no Rust is needed for the gate**.

`crates/engine/src/game/sortie.rs` — the reach check mirrors
`Game::broker_reach` (`game/contracts.rs:585`) exactly:

```rust
/// Whether the player can read the board, and whether they can sign for a
/// squad. Three states rather than two booleans for `NoPost::BoxedIn`'s
/// reason — the two refusals leave different errands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortieReach {
    NoRelay,
    OffBase,
    AtRelay,
}

impl Game {
    pub fn sortie_reach(&mut self) -> SortieReach {
        if !self.has_relay() {
            return SortieReach::NoRelay;
        }
        let Some((x, y)) = self.base_pos() else {
            return SortieReach::OffBase;
        };
        if self.world.resource::<crate::base_grid::BaseGrid>().is_floor(x, y) {
            SortieReach::AtRelay
        } else {
            SortieReach::OffBase
        }
    }
}
```

**It measures the base, never the distance to the Relay** — a Relay stands on
laid floor by construction, so its own tile says nothing the base does not.
It is emphatically not `Platform::covers`: `resources::Platform` no longer
exists, and `CLAUDE.md`'s base section is stale on that point.

- [ ] **Step 4: Run tests**

Run: `cargo test -p feral-processes-engine sortie`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add assets/structures/relay.ron assets/research/dispatch.ron \
        crates/engine/src/game/sortie.rs crates/engine/src/game/mod.rs \
        crates/engine/src/tests/sorties.rs
git commit -m "feat(sortie): the Relay, its research gate and reach"
```

---

### Task 6: The board

**Files:**
- Modify: `crates/engine/src/game/sortie.rs`
- Test: `crates/engine/src/tests/sorties.rs`

**Interfaces:**
- Produces:
  - `views::SortieRow { id: SortieId, name: String, description: String, risk: u32, battles: u32, ticks: u64 }`
  - `Game::sortie_board(&mut self) -> Option<Vec<views::SortieRow>>` — `None`
    on `NoRelay`
  - `Game::sortie_duration(risk: u32, battles: u32) -> u64` — a free
    function or an associated fn; **the one place the figure is computed**

- [ ] **Step 1: Write the failing test**

```rust
/// Derived, never stored: reloading reproduces the identical board, because
/// the inputs are identical and there is no stored roll to reroll.
#[test]
fn the_board_survives_a_save_and_load_unchanged() {
    let mut game = Game::new(4500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    super::support::stand_in_base(&mut game);
    deploy_relay(&mut game);

    let before: Vec<String> = game
        .sortie_board()
        .expect("a Relay stands")
        .iter()
        .map(|r| r.id.0.clone())
        .collect();

    let scratch = super::support::scratch_assets_dir("sortie_board_roundtrip");
    let path = scratch.path().join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let after: Vec<String> = loaded
        .sortie_board()
        .expect("a Relay stands")
        .iter()
        .map(|r| r.id.0.clone())
        .collect();

    assert_eq!(before, after, "a reload must not reroll the board");
}

/// Drawing the board spends no `GameRng`. A draw would not survive a reload
/// and would shift every later roll in the run — `stack::generate`'s rule.
/// Asserted by comparing the stream, since a test that only checks the board
/// is stable passes against a board that draws and discards.
#[test]
fn drawing_the_board_spends_no_rng() {
    let mut game = Game::new(4501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    super::support::stand_in_base(&mut game);
    deploy_relay(&mut game);

    let peek = |g: &mut Game| -> u64 {
        use rand::Rng;
        g.world.resource_mut::<crate::resources::GameRng>().0.random()
    };

    super::support::reseed_rng(&mut game, 77);
    let without = peek(&mut game);

    super::support::reseed_rng(&mut game, 77);
    let _ = game.sortie_board();
    let with = peek(&mut game);

    assert_eq!(without, with, "the board must not touch the run's stream");
}

/// It rotates on its own as the epoch advances — which is what makes "no
/// save-scumming" a property rather than a lockout.
#[test]
fn the_board_rotates_with_the_clock() {
    let mut game = Game::new(4502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    super::support::stand_in_base(&mut game);
    deploy_relay(&mut game);

    let first: Vec<String> = game.sortie_board().unwrap().iter().map(|r| r.id.0.clone()).collect();
    game.world.resource_mut::<crate::resources::GameClock>().0 +=
        crate::tuning::SORTIE_BOARD_ROTATION_TICKS * 3;
    let later: Vec<String> = game.sortie_board().unwrap().iter().map(|r| r.id.0.clone()).collect();

    assert_ne!(first, later, "three epochs on, the offers have turned over");
}

/// The screen and the trip quote the same number, `BuildOrderRow`'s rule
/// that every figure is a call.
#[test]
fn a_row_quotes_the_duration_the_trip_will_run() {
    let mut game = Game::new(4503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    super::support::stand_in_base(&mut game);
    deploy_relay(&mut game);

    for row in game.sortie_board().unwrap() {
        assert_eq!(row.ticks, Game::sortie_duration(row.risk, row.battles));
    }
}

/// Duration reads the risk **offset**, never the absolute band — so a deep
/// zone does not silently make every trip enormous.
#[test]
fn the_zone_does_not_lengthen_a_trip() {
    let short = Game::sortie_duration(0, 6);
    assert_eq!(
        short,
        crate::tuning::SORTIE_TRAVEL_BASE_TICKS
            + crate::tuning::SORTIE_TICKS_PER_BATTLE * 6
    );
}

/// No board without a Relay, and no panic either.
#[test]
fn no_relay_means_no_board() {
    let mut game = Game::new(4504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.sortie_board().is_none());
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: FAIL — `sortie_board` does not exist.

- [ ] **Step 3: Implement**

```rust
    /// The offers standing at the Relay. **Derived, never stored** — the
    /// Broker board's rule, and for its reasons: recomputed on every read
    /// from the world seed, `ZoneLevel` and the clock epoch, so there is no
    /// save field, no roll to scum, and it rotates on its own.
    ///
    /// Draws **no** `GameRng`: a draw here would not survive a reload and
    /// would shift every later roll in the run.
    pub fn sortie_board(&mut self) -> Option<Vec<crate::views::SortieRow>> {
        if self.sortie_reach() == SortieReach::NoRelay {
            return None;
        }
        let epoch = self.world.resource::<GameClock>().0
            / crate::tuning::SORTIE_BOARD_ROTATION_TICKS;
        let zone = self.world.resource::<ZoneLevel>().0 as u64;
        // `WorldMap::seed()` is the door to the run's seed — confirm the
        // exact accessor before writing this line; do not invent a second.
        let seed = self
            .world
            .resource::<WorldMap>()
            .seed()
            .wrapping_mul(crate::tuning::SORTIE_SALT)
            .wrapping_add(zone.wrapping_mul(0x9E37_79B9))
            .wrapping_add(epoch);
        // ... fold, then pick SORTIE_BOARD_SLOTS distinct sites and roll each
        // one's battle count inside its own range.
        ...
    }
```

Selection and the battle-count roll both go through `derive::index` and
Lemire's high-bit reducer, **never `%`** — `descriptions.rs`' rule, and the
`description-selection-reads-high-bits` trap: `% pool.len()` silently
anti-correlates two draws off one fold.

Sites are taken from `SortieDb::iter()` (already id-sorted). If the
catalogue holds fewer than `SORTIE_BOARD_SLOTS` sites, offer what there is;
if it is empty, return an empty `Vec` — **not** `None`, which means "no
Relay".

`sortie_duration` is the one computation:

```rust
    pub fn sortie_duration(risk: u32, battles: u32) -> u64 {
        crate::tuning::SORTIE_TRAVEL_BASE_TICKS
            + crate::tuning::SORTIE_TRAVEL_PER_RISK_TICKS * risk as u64
            + crate::tuning::SORTIE_TICKS_PER_BATTLE * battles as u64
    }
```

**No term for member count, level or power.** A stronger squad shows up as
better outcomes, never as a faster cycle.

Add `views::SortieRow` to `crates/engine/src/views.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/game/sortie.rs crates/engine/src/views.rs \
        crates/engine/src/tests/sorties.rs
git commit -m "feat(sortie): the Relay board, derived and never stored"
```

---

### Task 7: Dispatch

**Files:**
- Modify: `crates/engine/src/game/sortie.rs`
- Test: `crates/engine/src/tests/sorties.rs`

**Interfaces:**
- Produces:
  - `game::sortie::SortieRefusal` — one variant per refusal below, each
    carrying what the sentence needs
  - `Game::dispatch_sortie(&mut self, site: &SortieId, members: &[Entity]) -> Result<(), SortieRefusal>`
  - `Game::sortie_provision_cost(battles: u32, squad: usize) -> Vec<(ItemId, u32)>`

- [ ] **Step 1: Write the failing tests**

One test per refusal, each asserting **base stock is unchanged** — every
refusal lands before anything is spent, `commit_caravan_basket`'s rule, and
asserted per refusal rather than once:

```rust
/// Every refusal lands before anything is spent. Asserted per refusal: a
/// single test over one of them passes against six paths where five spend.
#[test]
fn every_refusal_spends_nothing() {
    for case in refusal_cases() {
        let (mut game, site, members) = case.build();
        let before = game.base_stock();
        assert!(game.dispatch_sortie(&site, &members).is_err(), "{}", case.name);
        assert_eq!(game.base_stock(), before, "{} spent something", case.name);
    }
}

/// Party and wielded programs are refused, so seconding one is an explicit
/// act rather than a side effect of a dispatch screen.
#[test]
fn a_party_member_cannot_be_dispatched() { ... }

/// A hurt program is refused: sending one on a twenty-fight trip is the
/// mistake the abort rule cannot save you from, because it fires on the
/// first battle.
#[test]
fn a_wounded_program_is_refused() { ... }

/// The base is never emptied. Production stops dead and a raid lands on an
/// empty base — the same category of guard as `max_deployed`.
#[test]
fn a_dispatch_may_not_empty_the_roster() { ... }

/// A successful dispatch charges the provisioning and takes the bodies off
/// the labour pool in the same call.
#[test]
fn a_dispatch_charges_and_takes_the_bodies() { ... }
```

Write each `...` body out in full when implementing — they are listed
compressed here only because they share one shape. Each stands a Home, a
Relay and a Depot, stocks the provisioning material, adopts enough programs,
and asserts one thing.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: FAIL — `dispatch_sortie` does not exist.

- [ ] **Step 3: Implement**

Refusals **in this order**, all before any mutation:

1. `sortie_reach() != AtRelay`
2. the site is not on the current board
3. `members` is empty, or names a duplicate
4. a member's `program_role` is not `Staff`
5. a member has `Downed`
6. a member's `hp < max_hp as f32 * SORTIE_MIN_HP_FRACTION`
7. the dispatch would leave zero `Staff` behind
8. the provisioning cost is not in base stock

Then, and only then: `stock::spend_from_base` for the cost (a teleport off
the shelf is right — this is a base cost paid at the Relay, not a build a
body walks to), push the `Sortie` onto `Sorties`, and log one dispatch line
through the base's own `MessageSource`.

The record stores `site: SortieDef` **cloned in full**, never the id.

- [ ] **Step 4: Run tests, then the suite**

Run: `cargo test -p feral-processes-engine sorties::` then
`cargo test -p feral-processes-engine`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/game/sortie.rs crates/engine/src/tests/sorties.rs
git commit -m "feat(sortie): dispatch, and its six refusals"
```

---

### Task 8: The trip

**Files:**
- Modify: `crates/engine/src/game/sortie.rs`
- Modify: `crates/engine/src/game/turn.rs` (call `run_sorties` from the tick)
- Test: `crates/engine/src/tests/sorties.rs`

**Interfaces:**
- Produces:
  - `Game::run_sorties(&mut self)` — called once per tick from `tick_inner`
  - `Game::resolve_sortie_battle(&mut self, index: usize)` — private

This is the task the whole feature turns on. Read Part 5 of the spec before
starting.

- [ ] **Step 1: Write the failing tests**

```rust
/// **The load-bearing test of the feature.** A battle spawns its
/// opposition, fights it and despawns it inside one call, so no bevy system
/// ever observes it. A hostile that outlives its battle is a defect, not a
/// tuning question.
#[test]
fn a_battle_leaves_no_hostile_behind() {
    let (mut game, ..) = a_dispatched_sortie(4600);
    let before = game.world.iter_entities().count();

    // Advance far enough for several battles to have fired.
    for _ in 0..400 {
        game.wait();
    }

    let after = game.world.iter_entities().count();
    assert_eq!(
        before, after,
        "a sortie battle must spawn and despawn inside one tick"
    );
}

/// The trip aborts on the first casualty — remaining battles are skipped,
/// the loot so far is kept, and the return travel still runs. It does not
/// come home early.
#[test]
fn the_first_casualty_aborts_but_does_not_shorten_the_trip() { ... }

/// One rule, two meanings: Forgiving benches and keeps the roster slot,
/// Permadeath dissolves.
#[test]
fn a_casualty_is_benched_under_forgiving_and_dissolved_under_permadeath() { ... }

/// Provisions restore Integrity between battles, which is the single dial
/// that decides whether a twenty-fight trip is survivable.
#[test]
fn provisions_restore_integrity_between_battles() { ... }

/// Power does not recover in the field, so Specials taper across a trip.
/// This is what earns the lower yield rather than tuning it.
#[test]
fn power_does_not_recover_in_the_field() { ... }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: FAIL — `run_sorties` does not exist.

- [ ] **Step 3: Implement**

`Game::run_sorties` is a **`Game` method, not a bevy system** —
`run_dig_crew` and `run_repair_bays`' reason: it names programs through
`creature_label`, it logs, and it damages through `apply_damage`.

Per tick, per in-flight sortie: `ticks_elapsed += 1`. A battle is due when
`ticks_elapsed` crosses the next evenly-spaced threshold inside the on-site
window (travel is split half out, half back), and the sortie is not
`aborted`.

One battle, entirely inside one call:

1. `habitat_pools(base_x, base_y, None, sortie.risk)` for the species pool.
2. `spawn_pack(species, false, SENTINEL_X, SENTINEL_Y, SpawnEscalation::surface())`.
3. Round loop until one side is out or a round cap is hit. Each combatant
   acts once per round, **sorted by entity** for `assembler_system`'s reason
   — bevy's iteration order is not stable and two squads would resolve
   differently between runs.
   - Action: the highest-priority ability the actor can afford that is off
     cooldown, else a basic attack. Specials go through
     `Game::use_ability(&def, actor, name, &recipients)`; basic attacks
     through `Game::resolve_and_apply_attack(attacker, defender,
     battle::Swing::plain(self.natural_range_of(attacker)))`. **Both are
     already `BattleState`-free** — do not open a `BattleState`, and do not
     use `choose_wild_action`, whose selection path reads one.
   - A member reaching 0 HP goes through `Game::bench_or_dissolve`, sets
     `aborted = true`, and ends the battle.
4. Award XP per kill through the existing path, scaled by
   `SORTIE_XP_MULTIPLIER`, accumulating into the record rather than logging
   per kill.
5. **Despawn every surviving hostile.** This is unconditional and is the last
   thing the function does before returning.
6. If not aborted, restore `max_hp * SORTIE_PROVISION_HEAL_FRACTION` to each
   living member via `restore_hp`.

The sentinel position is a fixed coordinate far outside any base or zone
traffic. It does not need to be walkable-checked: nothing observes these
entities, and `spawn_pack`'s scatter is harmless at a sentinel.

Call `run_sorties` from `tick_inner`, **after** the base's own systems and
guarded by `is_game_over().is_some() || has_active_battle()` returning early —
`run_dig_crew`'s guard, and `nest_aggro_tick`'s obligation: anything that can
change the world from inside a tick inherits the battle check.

- [ ] **Step 4: Run tests, then the suite**

Run: `cargo test -p feral-processes-engine sorties::` then
`cargo test -p feral-processes-engine`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/game/sortie.rs crates/engine/src/game/turn.rs \
        crates/engine/src/tests/sorties.rs
git commit -m "feat(sortie): the trip, resolved atomically inside a tick"
```

---

### Task 9: Return, the report, and the save

**Files:**
- Modify: `crates/engine/src/game/sortie.rs`
- Modify: `crates/engine/src/save.rs`
- Modify: `crates/engine/src/views.rs`
- Test: `crates/engine/src/tests/sorties.rs`

**Interfaces:**
- Produces:
  - `views::SortieReport { site: String, members: Vec<String>, kills: u32, xp: u32, loot: Vec<(ItemId, u32)>, casualties: Vec<String>, aborted: bool }`
  - `Game::sortie_reports(&self) -> Vec<views::SortieReport>`
  - `save::SortieSave`, `PlayerSave::sorties: Vec<SortieSave>`,
    `CreatureSave::sortie_index: Option<u32>`

- [ ] **Step 1: Write the failing tests**

```rust
/// An in-flight sortie survives a save and load: the same members, the same
/// site, the same countdown.
#[test]
fn an_in_flight_sortie_survives_a_save_and_load() { ... }

/// Membership rides `CreatureSave`, `party_slot`'s precedent — entity ids
/// are not stable across a save, which is exactly why the party does it this
/// way.
#[test]
fn membership_is_restored_from_the_creature_side() { ... }

/// The save format is not bumped: the fields are additive behind
/// `#[serde(default)]`, so a save written before sorties loads with none.
#[test]
fn a_pre_sortie_save_loads_with_no_sorties() { ... }

/// Loot lands in depots; what does not fit is logged rather than dropped in
/// silence — `return_to_depots`' existing rule.
#[test]
fn overflow_loot_is_logged_rather_than_lost() { ... }

/// Members are staff again by omission the moment the record drops.
#[test]
fn a_returned_program_is_staff_again() { ... }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine sorties::`
Expected: FAIL

- [ ] **Step 3: Implement**

Return, when `ticks_elapsed >= ticks_total`: drop the record from `Sorties`
(members become `Staff` by omission — do **not** write a role anywhere),
push loot through `stock::return_to_depots`, log one return line naming what
came back and who did not, and — under Forgiving — leave the `Downed` marker
alone so `drift_idle_staff`'s existing `Downed` arm walks the casualty to a
Repair Bay.

Save: `PlayerSave::sorties: Vec<SortieSave>` behind `#[serde(default)]`, and
`CreatureSave::sortie_index: Option<u32>` behind `#[serde(default)]`, naming
which in-flight sortie a program belongs to. `SortieSave` carries **no member
list** — membership is reassembled from the creature side on load, which is
`party_slot`'s precedent and exists because entity ids are not stable across
a save.

`SortieSave` is a **named struct, never a positional tuple** — the one shape
field-named RON does not save you from.

**Do not bump `SAVE_FORMAT_VERSION`.**

- [ ] **Step 4: Verify the RON round trip AND a real save/load**

A `#[serde(skip)]` or a field the RON round trip does not exercise leaves
that test green while the real save loses the field. Run both:

```bash
cargo test -p feral-processes-engine sorties::
cargo test -p feral-processes-engine save
```

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add crates/engine/src/game/sortie.rs crates/engine/src/save.rs \
        crates/engine/src/views.rs crates/engine/src/tests/sorties.rs
git commit -m "feat(sortie): return, the report, and the save"
```

---

### Task 10: The censuses and the final gate

**Files:**
- Modify: `crates/engine/src/tests/assets.rs`
- Test: `crates/engine/src/tests/sorties.rs`

- [ ] **Step 1: Write the census tests**

```rust
/// Deleting `assets/sorties/` restores the pre-sortie game rather than
/// breaking one — `NeedDb` and `MemoryDb`'s property. Never gate a system
/// or a screen on the catalogue being non-empty.
#[test]
fn an_empty_catalogue_is_a_supported_install() {
    let scratch = super::support::scratch_assets_dir("sortie_empty");
    // Copy the shipped assets, then remove `sorties/` entirely.
    ...
    let mut game = Game::new(4700, DifficultyMode::Forgiving, scratch.path()).unwrap();
    super::support::stand_in_base(&mut game);
    deploy_relay(&mut game);
    assert_eq!(game.sortie_board(), Some(Vec::new()));
    for _ in 0..200 { game.wait(); }   // must not panic
}

/// Every shipped site's battle range is sane and its risk is inside the
/// window `habitat_pools` can actually serve — a site nothing can be drawn
/// for is an offer that refuses when taken.
#[test]
fn every_shipped_site_can_be_populated() { ... }

/// A sortie kill must pay less than the same kill taken with the player in
/// the fight. `balance_sim` models no base production and no abilities, so
/// it cannot gate this — the assertion lives here, over the real assets.
#[test]
fn a_sortie_kill_pays_less_than_fighting_it_yourself() { ... }
```

- [ ] **Step 2: Run the full workspace suite**

Run: `cargo test --workspace`
Expected: PASS. This is the gate — passing only the tests in this plan is not
evidence of correctness.

- [ ] **Step 3: Run the balance regression gate**

Run: `cargo test -p feral-processes-engine balance_sim`
Expected: PASS, curves unmoved. Nothing in this plan touches a species file,
an item file or an existing `tuning.rs` value, so a moved curve means
something was changed that should not have been.

- [ ] **Step 4: Update the docs**

- `CHANGELOG.md` — a new `## X.Y.Z` section. Which digit moves is decided by
  `CHANGELOG.md`'s own preamble; read it. No save-format bump here, so this
  is not breaking.
- `assets/sorties/README.md` — already written in Task 2; verify it still
  matches the shipped fields.
- `assets/structures/README.md` and `assets/research/README.md` — only if a
  field changed meaning. Adding a file that uses existing fields needs no doc
  edit.
- **Do not** touch `docs/manual.md`, the root `README.md` or `TODO.md`.
- Add the load-bearing seams to `CLAUDE.md` and `docs/seams.md`: the
  atomic-battle rule, `role_of`'s fourth variant, and that the board is
  derived. Argument in `docs/seams.md`, rule only in `CLAUDE.md`.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -u
git commit -m "feat(sortie): censuses, changelog and seam docs"
```

---

## Self-review notes

**Spec coverage.** Part 1 → Task 1. Part 2 → Task 5. Part 3 → Tasks 2 and 6.
Part 4 → Task 7. Part 5 → Tasks 4 and 8. Part 6 → Task 9. Part 7 → Task 9.
Part 8 → Task 3. Part 9 → spread across every task, with the cross-cutting
censuses in Task 10. Part 10 (out of scope) needs no task by construction.

**Known compressions.** Tasks 7, 8, 9 and 10 list some test bodies as `...`
where several tests share one shape. Those bodies must be written out in full
during implementation — the shape is stated above each one and the assertion
is named. This is the one place this plan is deliberately not literal, and it
is flagged rather than silent.

**Ordering dependency.** Task 1's `Sortie::test_stub` needs Task 2's
`SortieDef`. Land `SortieDef`'s struct with Task 1 and its loader with Task 2,
or execute Task 2 first.
