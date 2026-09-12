# Weapon Range and Travelling Bolts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give a weapon and a species' `ranged` move a distance they can be swung from on the tactical battle map, and draw every basic attack as a streak travelling from the swinger to each body it lands on.

**Architecture:** A new `#[serde(default)] range: Option<u32>` on `ItemDef`, sibling to `reach` and never inside it. One door, `Game::swing_range(actor)`, answers what a body's swing reaches; three existing hard-coded `TACTICAL_MELEE_RANGE` readers become calls to it. `tactical_attack` gains a distance check and an unconditional line-of-sight check. A new `resources::BoltQueue` carries `BoltCue { from, to, color }` in `TacticalBattle` cells, drained by `Game::take_bolts()` on `TransitQueue`'s rules, and `crate::fx::Fx` draws each as a `Painter::line` streak in the tactical pane.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine, standalone), `bevy` + `bevy_egui` (gui), RON assets.

**Spec:** `docs/superpowers/specs/2026-09-12-weapon-range-and-bolts-design.md`

## Global Constraints

- **The engine's `Game` is the whole public API.** The renderer never touches
  the ECS `World`. Never add a `world()` / `world_mut()` accessor.
- **`crates/gui/src/paint.rs` is the only file that may name a graphics
  library.** Everything under `crates/gui/src/render/` draws through
  `Painter`. This change adds **no** sixteenth `Painter` operation —
  `Painter::line` already exists.
- **Content stays moddable.** The new field is data on `ItemDef`; a
  malformed or out-of-range value is **skipped with a logged warning**, never
  a panic, following `ItemDb::load_dir`'s existing pattern.
- **New asset fields are `#[serde(default)]`** so every existing item file
  and every mod's keeps parsing untouched.
- **Tuning values live in `crates/engine/src/tuning.rs`** as documented
  `pub const`s, never inline in a formula.
- **No backwards-compat cruft.** No shims, no `// removed` comments.
- **Comments explain *why*.** Never restate what the code says.
- **`cargo fmt` and `cargo clippy --workspace --all-targets` after every
  task.** `--all-targets` is load-bearing — a bare clippy leaves test modules
  unlinted.
- **Commit at every green step.** Do **not** push; do **not** merge; do
  **not** bump the version or write a `CHANGELOG.md` section — the version
  bump happens once, at the merge, and is not this plan's job.
- **This is a git worktree.** Run everything from the worktree root. Never
  use bare `git stash` / `git stash pop` — the stash stack is shared with
  other sessions.
- Exact values fixed by the spec:
  - `tuning::TACTICAL_WEAPON_RANGE_MAX = 3`
  - `tuning::TACTICAL_RANGED_MOVE_RANGE = 2`
  - `tuning::TACTICAL_MELEE_RANGE = 1` (already exists, unchanged)
  - `TACTICAL_TURNS_PER_SECOND = 1.6` (already exists in app-core, unchanged)
  - Authored weapon ranges: Arc Lance 2, Scatter Lance 2, Interrupt Coil 2,
    Monofilament Whip 2, Plasma Router 3, Siege Compiler 3,
    Broadcast Storm 3.

---

## File Structure

**Engine (`crates/engine/`)**

| file | responsibility |
|---|---|
| `src/tuning.rs` | the two new constants, in the tactical section beside `TACTICAL_MELEE_RANGE` |
| `src/items_db.rs` | `ItemDef::range` field; `unreachable_range` load refusal beside `unreachable_reach` |
| `src/game/combat.rs` | `Game::swing_range` — the one door |
| `src/game/combat_round.rs` | `swing_move_at` / `roll_species_move_in_range` — the distance-constrained roll |
| `src/tactical/turn.rs` | `tactical_attack`'s range gate, LOS gate, and the bolt push |
| `src/tactical/ai.rs` | `Intent::Swing { range }`, `swing_at_best_neighbour`'s filter |
| `src/resources.rs` | `BoltCue`, `BoltQueue` |
| `src/lib.rs` | register `BoltQueue`; re-export `BoltCue` |
| `src/game/base/upkeep.rs` | `Game::take_bolts` beside `take_transits` |
| `src/game/catalog.rs` | `range_line` for the gear inspect page |
| `src/views.rs` | `WornDetailView::range` |
| `src/tests/assets.rs` | the authored-range census |
| `src/tests/tactical.rs` | the gate, the AI and the cue tests |

**gui (`crates/gui/`)**

| file | responsibility |
|---|---|
| `src/fx.rs` | `Bolt`, `Fx::bolts`, `BOLT_SECONDS`, `Fx::draw_bolts`, `bolt_progress` |
| `src/lib.rs` | drain `game.take_bolts()` every frame and feed `begin_frame` |
| `src/render/tactical.rs` | call `fx.draw_bolts` over the tiles |

**Assets**

| file | responsibility |
|---|---|
| `assets/items/README.md` | document `range:` |
| 7 weapon `.ron` files | author `range:` |

---

### Task 1: The `range` field, its refusal, and the shipped content

The field parses and is validated, and the seven weapons author it. **Nothing
reads it yet** — this task changes no behaviour, which is what makes it
independently reviewable.

**Files:**
- Modify: `crates/engine/src/tuning.rs` (beside `TACTICAL_MELEE_RANGE`, ~line 5025)
- Modify: `crates/engine/src/items_db.rs:210` (the field), `:346` (`unreachable_reach`, add a sibling)
- Modify: `assets/items/README.md`
- Modify: `assets/items/arc_lance.ron`, `scatter_lance.ron`, `interrupt_coil.ron`, `monofilament_whip.ron`, `plasma_router.ron`, `siege_compiler.ron`, `broadcast_storm.ron`
- Test: `crates/engine/src/tests/assets.rs`, and the `mod tests` already at the foot of `crates/engine/src/items_db.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `ItemDef::range: Option<u32>`;
  `tuning::TACTICAL_WEAPON_RANGE_MAX: u32 = 3`;
  `tuning::TACTICAL_RANGED_MOVE_RANGE: u32 = 2`.

- [ ] **Step 1: Write the failing census and refusal tests**

In `crates/engine/src/tests/assets.rs`, beside the existing item censuses:

```rust
/// A range outside the window is refused at load, so a shipped one sitting
/// outside it would be a weapon that silently cannot be swung at all.
#[test]
fn every_authored_weapon_range_is_inside_its_window() {
    let (items, _) = load_items();
    let mut authored = 0;
    for def in items.iter() {
        let Some(range) = def.range else { continue };
        authored += 1;
        assert!(
            (1..=crate::tuning::TACTICAL_WEAPON_RANGE_MAX).contains(&range),
            "{} authors range {range}, outside 1..={}",
            def.id,
            crate::tuning::TACTICAL_WEAPON_RANGE_MAX
        );
        assert!(
            matches!(
                def.equipment,
                Some((crate::items::EquipmentSlot::Weapon, _))
            ),
            "{} authors a range and is not a weapon",
            def.id
        );
    }
    assert!(
        authored >= 7,
        "the shipped reaching weapons stopped authoring a range: {authored}"
    );
}
```

`load_items` and `items.iter()` may be spelled differently in this file —
**read the neighbouring censuses in `tests/assets.rs` and copy their exact
loader idiom** rather than inventing one.

In `crates/engine/src/items_db.rs`'s own `mod tests`, three refusals, each
its own test because one test over one path passes against an
implementation that refuses only that path:

```rust
#[test]
fn a_range_on_something_that_is_not_a_weapon_is_refused() {
    let def = ItemDef {
        range: Some(2),
        equipment: None,
        ..weapon_def()
    };
    assert!(def.unreachable_range().is_some());
}

#[test]
fn a_range_of_zero_is_refused() {
    let def = ItemDef {
        range: Some(0),
        ..weapon_def()
    };
    assert!(def.unreachable_range().is_some());
}

#[test]
fn a_range_past_the_ceiling_is_refused() {
    let def = ItemDef {
        range: Some(crate::tuning::TACTICAL_WEAPON_RANGE_MAX + 1),
        ..weapon_def()
    };
    assert!(def.unreachable_range().is_some());
}

#[test]
fn a_range_inside_the_window_on_a_weapon_is_accepted() {
    let def = ItemDef {
        range: Some(2),
        ..weapon_def()
    };
    assert!(def.unreachable_range().is_none());
}
```

`weapon_def()` is a fixture returning a minimal weapon `ItemDef`. **Check
whether `mod tests` in `items_db.rs` already has one** (the `unreachable_reach`
tests need the same thing); reuse it, and only write one if there is none.

- [ ] **Step 2: Run the tests and confirm they fail**

```bash
cargo test -p feral-processes-engine range 2>&1 | tail -30
```

Expected: compile failure — no field `range` on `ItemDef`, no
`unreachable_range`, no `TACTICAL_WEAPON_RANGE_MAX`.

- [ ] **Step 3: Add the two tuning constants**

In `crates/engine/src/tuning.rs`, immediately after `TACTICAL_MELEE_RANGE`:

```rust
/// The furthest a weapon may author a swing, in cells.
///
/// Three, which is half `TACTICAL_DEPLOY_GAP`: a weapon that reached the far
/// side's deployment from the near side's would make the approach this model
/// is built on optional, and a ceiling is what keeps a mod from authoring
/// one. Refused at load past this, rather than clamped — a weapon quietly
/// swinging shorter than its file says reads as a nerf rather than a bad
/// value.
pub const TACTICAL_WEAPON_RANGE_MAX: u32 = 3;

/// How far a species' `ranged` basic attack reaches on a battle map.
///
/// Two, and the number is load-bearing. Every shipped species carries
/// exactly two moves and **fourteen of seventeen carry exactly one
/// `ranged: true`** — only Construct, Scrapper and Sentinel are pure melee.
/// At three, nearly every wild body on the board becomes a shooter and the
/// closing this whole model is built on stops mattering. Two is one cell of
/// standoff: real, and closed by a single step.
///
/// Not to be confused with `MoveDef::ranged`'s *group*-model meaning, which
/// is a yes-or-no about the front line and is untouched by this.
pub const TACTICAL_RANGED_MOVE_RANGE: u32 = 2;
```

- [ ] **Step 4: Add the field**

In `crates/engine/src/items_db.rs`, immediately after the `reach` field
(currently line 210):

```rust
    /// How far from the swinger this weapon may be swung, in cells. Absent
    /// means melee — `tuning::TACTICAL_MELEE_RANGE`.
    ///
    /// **`reach`'s sibling and not part of it.** That field is *breadth* —
    /// what a swing lands on past the body it is aimed at — and folding
    /// distance into it would force a single-target weapon that reaches
    /// three cells to author a `target` and a `recharge` it does not want.
    ///
    /// On the def rather than in `equipment`'s `EquipmentStats` for
    /// `reach`'s own reason: `Game::copy_bonus`'s four scaling axes must not
    /// touch it, and `EquipmentStats::is_empty`/`has_upside` destructure.
    ///
    /// Read in tactical fights alone, through `Game::swing_range` — the
    /// group model has no geometry to spend it on.
    #[serde(default)]
    pub range: Option<u32>,
```

- [ ] **Step 5: Add the load refusal**

In `crates/engine/src/items_db.rs`, immediately after `unreachable_reach`:

```rust
    /// Why this item's `range` cannot be honoured, or `None` if it can.
    ///
    /// `unreachable_reach`'s sibling, and refused rather than clamped for
    /// the reason `TACTICAL_WEAPON_RANGE_MAX` states.
    fn unreachable_range(&self) -> Option<String> {
        let range = self.range?;
        if !matches!(self.equipment, Some((EquipmentSlot::Weapon, _))) {
            return Some("range: only a weapon swings".into());
        }
        if !(1..=crate::tuning::TACTICAL_WEAPON_RANGE_MAX).contains(&range) {
            return Some(format!(
                "range: {range} is outside 1..={}",
                crate::tuning::TACTICAL_WEAPON_RANGE_MAX
            ));
        }
        None
    }
```

Then call it wherever `unreachable_reach` is called in `load_dir`, warning
and **dropping the field** (`def.range = None`) rather than skipping the whole
item — the same grace a bad `reach` gets. **Read that call site first and
match exactly what it does with a bad reach**; if it skips the whole item,
skip the whole item here too. Consistency with the neighbour beats this
paragraph.

- [ ] **Step 6: Author the seven weapons**

Add one line to each file. Example, `assets/items/arc_lance.ron`:

```ron
    range: Some(2),
```

Ranges: `arc_lance` 2, `scatter_lance` 2, `interrupt_coil` 2,
`monofilament_whip` 2, `plasma_router` 3, `siege_compiler` 3,
`broadcast_storm` 3. Change nothing else in those files.

- [ ] **Step 7: Document the field**

In `assets/items/README.md`, beside the `reach:` documentation, add a
commented block in that file's existing house style covering: that it is
cells, that absent means melee, that it is read on the battle map only, that
it is refused outside `1..=3`, and that it is distance where `reach` is
breadth.

- [ ] **Step 8: Run the tests and confirm they pass**

```bash
cargo test -p feral-processes-engine range 2>&1 | tail -20
cargo test -p feral-processes-engine assets 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 9: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -20
git add -- crates/engine/src/tuning.rs crates/engine/src/items_db.rs crates/engine/src/tests/assets.rs assets/items/
git commit -m "feat(items): a weapon may author the distance it swings from

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

Stage **explicit paths**, never `git add -A` — another session's worktree
gitlink lives under `.claude/worktrees/`.

---

### Task 2: `Game::swing_range`, the one door

**Files:**
- Modify: `crates/engine/src/game/combat.rs` (beside `swing_reach`, ~line 54)
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Consumes: `ItemDef::range`, `tuning::TACTICAL_RANGED_MOVE_RANGE` (Task 1).
- Produces: `pub(crate) fn swing_range(&self, actor: Entity) -> u32`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/tactical.rs`. The fixtures `game()`,
`body(game, species)` and `tactical_fight(game, count, hp)` are already in
that file.

```rust
/// The player unarmed swings at arm's length, and nothing else.
#[test]
fn an_unarmed_body_swings_at_arms_length() {
    let mut game = game();
    let player = game.player_entity();
    assert_eq!(game.swing_range(player), crate::tuning::TACTICAL_MELEE_RANGE);
}

/// A species carrying a `ranged` move reaches past arm's length.
#[test]
fn a_ranged_species_reaches_past_arms_length() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    assert_eq!(
        game.swing_range(shooter),
        crate::tuning::TACTICAL_RANGED_MOVE_RANGE,
        "Drone's Recon Ping is `ranged: true`"
    );
}

/// A species with no ranged move stays at arm's length.
#[test]
fn a_melee_species_stays_at_arms_length() {
    let mut game = game();
    let bruiser = body(&mut game, "construct");
    assert_eq!(
        game.swing_range(bruiser),
        crate::tuning::TACTICAL_MELEE_RANGE,
        "Construct authors no ranged move"
    );
}
```

The test asset tree is `crates/engine/assets` or the repo's `assets/` via
`test_assets_dir()` — **confirm `drone` and `construct` both resolve through
`game.species_defs()` in that tree** before relying on their names; if the
test tree is a reduced fixture, use `generic_species` plus a hand-written
species instead.

A fourth test pins the override, and needs a weapon equipped on a ranged
species. Equip through the real door:

```rust
/// A worn weapon **replaces** the species figure rather than being maxed
/// against it — `Game::attack_range`'s precedent, where worn damage replaces
/// a natural band outright. A ranged program holding a melee blade swings at
/// arm's length, because the weapon is what it is swinging.
#[test]
fn a_worn_weapon_replaces_the_species_range() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    equip_weapon(&mut game, shooter, "shim_blade");
    assert_eq!(game.swing_range(shooter), crate::tuning::TACTICAL_MELEE_RANGE);

    equip_weapon(&mut game, shooter, "plasma_router");
    assert_eq!(game.swing_range(shooter), 3);
}
```

`equip_weapon` is a fixture: **look in `crates/engine/src/tests/support.rs`
first** — CLAUDE.md says engine fixtures live there and to look before writing
a new one. If none exists, write the smallest one that inserts an
`Equipment` with a `weapon: Some(EquippedItem { copy: GearCopy { item, .. }, .. })`,
and put it in `support.rs`, not inline.

- [ ] **Step 2: Run the tests and confirm they fail**

```bash
cargo test -p feral-processes-engine swing_range 2>&1 | tail -20
cargo test -p feral-processes-engine arms_length 2>&1 | tail -20
```

Expected: compile failure — no method `swing_range`.

- [ ] **Step 3: Implement the door**

In `crates/engine/src/game/combat.rs`, immediately after `swing_reach`:

```rust
    /// How far from itself `actor` may swing its basic attack, in cells.
    ///
    /// **The one door, and its three readers are calls rather than copies.**
    /// `tactical_attack`'s gate, `Intent::Swing`'s band and
    /// `swing_at_best_neighbour`'s filter each hard-coded
    /// `TACTICAL_MELEE_RANGE` before this existed, which is exactly the
    /// drift a shared doc comment cannot hold: a body whose band says two
    /// and whose filter says one walks into position and then passes.
    ///
    /// A property of the **body**, never of the move it happens to roll. The
    /// AI decides its intent before it walks, so a range read off a move
    /// drawn at swing time lets a body plan a standoff and then draw the
    /// melee half of its pair.
    ///
    /// A worn weapon **replaces** the species figure rather than being maxed
    /// against it — `attack_range`'s own rule, where worn damage replaces a
    /// natural band outright.
    ///
    /// **Read in tactical fights alone.** The group model has no geometry to
    /// spend it on, which is `AbilityDef::tactical_shape`'s rule; `ranged`
    /// keeps its separate group-model meaning untouched.
    pub(crate) fn swing_range(&self, actor: Entity) -> u32 {
        if let Some(worn) = self
            .world
            .get::<Equipment>(actor)
            .and_then(|e| e.weapon.as_ref())
            && let Some(range) = self
                .world
                .resource::<ItemDb>()
                .get(worn.copy.item.as_str())
                .and_then(|def| def.range)
        {
            return range;
        }
        let species_range = self
            .world
            .get::<Creature>(actor)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|species| {
                species
                    .basic_attacks()
                    .iter()
                    .map(|a| match a.ranged {
                        true => crate::tuning::TACTICAL_RANGED_MOVE_RANGE,
                        false => crate::tuning::TACTICAL_MELEE_RANGE,
                    })
                    .max()
                    .unwrap_or(crate::tuning::TACTICAL_MELEE_RANGE)
            });
        species_range.unwrap_or(crate::tuning::TACTICAL_MELEE_RANGE)
    }
```

`Equipment`, `ItemDb`, `Creature` and `SpeciesDb` should already be in scope
through `use crate::*;` at the head of that file. If any is not, add the
specific import rather than widening the glob.

The `if let ... && let ...` chain is Rust 2024 let-chains; **if the
workspace's edition rejects it, split into nested `if let`** rather than
reaching for `unwrap`.

- [ ] **Step 4: Run the tests and confirm they pass**

```bash
cargo test -p feral-processes-engine swing_range 2>&1 | tail -20
cargo test -p feral-processes-engine arms_length 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 5: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -20
git add -- crates/engine/src/game/combat.rs crates/engine/src/tests/
git commit -m "feat(combat): one door for how far a body swings

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: The attack gate — distance and line of sight

**Files:**
- Modify: `crates/engine/src/tactical/turn.rs:243` (the `abs() > 1` refusal inside `tactical_attack`)
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Consumes: `Game::swing_range` (Task 2); `tactical::reach::distance`,
  `tactical::reach::line_of_sight` (both already `pub` in
  `crates/engine/src/tactical/reach.rs`).
- Produces: `tactical_attack` landing at range; no new public names.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/tactical.rs`. These need bodies placed at chosen
cells: `TacticalBattle::place(entity, cell)` is what the existing
`reach.rs` tests use — **read how `tests/tactical.rs` currently positions
bodies inside an open fight and use that idiom**, since the resource is
reached through `game.world` here rather than held directly.

```rust
/// Exactly at the range lands; one cell past it is refused.
#[test]
fn a_swing_lands_at_its_range_and_no_further() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router"); // range 3
    place_bodies(&mut game, player, (0, 0), pack[0], (3, 0));
    assert!(game.tactical_attack(pack[0]), "three cells is inside range 3");

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (4, 0));
    assert!(!game.tactical_attack(pack[0]), "four cells is past range 3");
}

/// Cover blocks a swing. **Delete the `line_of_sight` line and this must
/// fail** — a test that passes with the fix removed is not coverage.
#[test]
fn a_ranged_swing_is_blocked_by_cover() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    block_cell(&mut game, (1, 0));
    assert!(!game.tactical_attack(pack[0]), "cover did not stop the swing");
}

/// The same swing with the cover gone lands — so the test above is measuring
/// the cover and not the placement.
#[test]
fn the_same_swing_lands_once_the_cover_is_gone() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    assert!(game.tactical_attack(pack[0]));
}

/// An adjacent swing is untouched by the sight check — `line_of_sight`
/// excludes its endpoints, so for neighbours its loop is empty.
#[test]
fn an_adjacent_swing_is_unaffected_by_the_sight_check() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
}
```

Write `place_bodies`, `place_one`, `hp_of` and `block_cell` as small local
helpers at the top of the tests module (`hp_of` reads
`world.get::<Stats>(e).hp`, and `place_one` is `place_bodies`' single-body
half). **Confirm first what `AbilityTarget::WholeEnemyGroup::derived_shape`
resolves to** — the sweep test assumes a shape centred on the aimed cell
reaches an orthogonal neighbour, which holds for a `Radius` of one or more
and not for a `Line`. If it derives something else, place the second hostile
where that shape actually covers rather than changing the shape.

Write the rest of the helpers at the top of the tests module — `block_cell` writes a `BattleCell::Blocked` into the
board. **Read `tactical/map.rs` for how a `Board` cell is written**; if the
board is immutable after construction, build the fight on a board authored
from rows with the cover already in it (`Board::from_rows`, which
`tactical/reach.rs`'s own tests use) instead.

One more engine test, that a reach weapon's sweep is unchanged by the
distance — the spec says a `Line` fired from three cells out simply runs
further, which is `reach::recipients`' existing rule and must stay one:

```rust
/// A sweep fired from range still sweeps. The shape is cast from the actor
/// toward the aim whatever the distance, so a reach weapon that quietly went
/// single-target at range would read as a nerf rather than a bug.
#[test]
fn a_reach_weapon_still_sweeps_when_fired_from_range() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 20);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "scatter_lance"); // range 2, WholeEnemyGroup
    // Both hostiles two cells out and adjacent to each other, so the sweep
    // centred on the aimed one covers the second.
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    place_one(&mut game, pack[1], (2, 1));
    let before = hp_of(&game, pack[1]);
    assert!(game.tactical_attack(pack[0]));
    assert!(
        hp_of(&game, pack[1]) < before,
        "the neighbour was not caught by a sweep fired from two cells"
    );
}
```

**No app-core test in this task, deliberately.** `App` has no logic change:
the aim cursor already points anywhere on the board, `commit_tactical_aim`
already calls `tactical_attack` and already refuses on `false`, and app-core
cannot reach the engine's `World` to stand a hostile at a chosen distance.
A test there would be measuring the engine through a keyhole.

- [ ] **Step 2: Run the tests and confirm they fail**

```bash
cargo test -p feral-processes-engine swing_lands 2>&1 | tail -30
cargo test -p feral-processes-engine blocked_by_cover 2>&1 | tail -30
```

Expected: FAIL — the swing at 3 and at 2 are both refused by the adjacency
check.

- [ ] **Step 3: Replace the adjacency check**

In `crates/engine/src/tactical/turn.rs`, replace:

```rust
        if actor == target || (from.0 - at.0).abs() > 1 || (from.1 - at.1).abs() > 1 {
            return false;
        }
```

with:

```rust
        if actor == target || reach::distance(from, at) > self.swing_range(actor) {
            return false;
        }
        // **Unconditional, with no melee branch.** `line_of_sight` excludes
        // its endpoints, so for neighbours its loop is empty and this is
        // already a no-op — one rule, and no second place
        // `TACTICAL_MELEE_RANGE` has to be restated. Cover earns a second
        // job for free.
        {
            let battle = self.world.resource::<TacticalBattle>();
            if !reach::line_of_sight(&battle.board, from, at) {
                return false;
            }
        }
```

`reach::distance` returns the Chebyshev distance the rest of the model uses,
so this is the same geometry the old `abs()` pair expressed. Keep the borrow
of `TacticalBattle` in its own block — the swing below needs `&mut self`.

`reach::distance` is `pub fn distance(a: (i32, i32), b: (i32, i32)) -> u32`
(`tactical/reach.rs:167`) and `swing_range` returns `u32`, so the comparison
is direct with no cast. `reach` is already imported in `turn.rs` via
`use crate::tactical::{TacticalBattle, reach};`; confirm `battle.board` is
the field name before using it.

- [ ] **Step 4: Run the tests and confirm they pass**

```bash
cargo test -p feral-processes-engine tactical 2>&1 | tail -30
cargo test -p feral-processes-engine sweeps 2>&1 | tail -30
```

Expected: PASS. Existing tactical tests that assumed adjacency-only must
still pass — if one fails because it asserted a *refusal* at distance, read
it: it is now asserting the old rule and needs updating to assert the new
one, not deleting.

- [ ] **Step 5: Prove the LOS test is not vacuous**

Comment out the `line_of_sight` block, run
`cargo test -p feral-processes-engine blocked_by_cover`, and confirm it
**fails**. Restore the block and confirm it passes again. Do not commit with
it commented out.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -20
git add -- crates/engine/src/tactical/turn.rs crates/engine/src/tests/
git commit -m "feat(tactical): a swing reaches its weapon's range, and needs sight

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: The rolled move is constrained by the distance

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs:493` (`swing_move`), `:603` (`roll_species_move`)
- Modify: `crates/engine/src/tactical/turn.rs` (pass the distance)
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Consumes: `Game::swing_range` (Task 2).
- Produces: `pub(crate) fn swing_move_at(&mut self, entity: Entity, distance: Option<u32>) -> (String, battle::DamageRange)`;
  `swing_move(entity)` becomes a call to it with `None`;
  `pub(crate) fn roll_species_move_in_range(&mut self, entity: Entity, distance: Option<u32>) -> Option<AbilityDef>`,
  with `roll_species_move(entity)` a call to it with `None`.

- [ ] **Step 1: Write the failing tests**

```rust
/// A body standing at two cells fires the ranged half of its pair, never the
/// melee half — the AI decides its intent before it walks, and a roll that
/// could draw a move it cannot fire makes a planned standoff a coin flip.
#[test]
fn a_body_at_range_rolls_only_a_move_that_reaches() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    for _ in 0..20 {
        let rolled = game
            .roll_species_move_in_range(shooter, Some(2))
            .expect("Drone has moves");
        assert!(rolled.ranged, "rolled {} at two cells", rolled.name);
    }
}

/// Adjacent, either half of the pair is fair game — which is the variety
/// `swing_move` exists for.
#[test]
fn a_body_adjacent_may_roll_either_move() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    let mut saw_melee = false;
    for _ in 0..40 {
        let rolled = game
            .roll_species_move_in_range(shooter, Some(1))
            .expect("Drone has moves");
        saw_melee |= !rolled.ranged;
    }
    assert!(saw_melee, "the melee half was never drawn in forty rolls");
}

/// The group model passes `None` and is untouched.
#[test]
fn the_group_model_rolls_over_every_move() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    let mut saw_melee = false;
    let mut saw_ranged = false;
    for _ in 0..40 {
        let rolled = game.roll_species_move(shooter).expect("Drone has moves");
        saw_melee |= !rolled.ranged;
        saw_ranged |= rolled.ranged;
    }
    assert!(saw_melee && saw_ranged, "the unconstrained roll narrowed");
}
```

Twenty and forty iterations are fine here — this is a seeded `GameRng`, not
wall-clock or unseeded RNG, so it is deterministic and not a flake.

- [ ] **Step 2: Run and confirm they fail**

```bash
cargo test -p feral-processes-engine rolls_only 2>&1 | tail -20
```

Expected: compile failure — no `roll_species_move_in_range`.

- [ ] **Step 3: Split the roll**

In `crates/engine/src/game/combat_round.rs`, replace `roll_species_move`'s
body with a call, and add the constrained form:

```rust
    /// One of `entity`'s species moves, drawn at random.
    pub(crate) fn roll_species_move(&mut self, entity: Entity) -> Option<AbilityDef> {
        self.roll_species_move_in_range(entity, None)
    }

    /// One of `entity`'s species moves that could actually be fired from
    /// `distance` cells away, drawn at random.
    ///
    /// `None` is the **group model's** distance: it has no geometry, so it
    /// draws from every move exactly as it always has. Only a battle map
    /// passes a number.
    ///
    /// One `random_range` draw whatever the candidate count, so the seeded
    /// stream's *position* is unchanged by this narrowing — the value drawn
    /// may differ, which is an ordinary retune signal and not a stream
    /// shift.
    pub(crate) fn roll_species_move_in_range(
        &mut self,
        entity: Entity,
        distance: Option<u32>,
    ) -> Option<AbilityDef> {
        let species_id = self.world.get::<Creature>(entity)?.species.clone();
        let all = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .map(|s| s.basic_attacks())?;
        let moves: Vec<AbilityDef> = match distance {
            None => all,
            Some(d) => all
                .into_iter()
                .filter(|mv| {
                    let reach = match mv.ranged {
                        true => crate::tuning::TACTICAL_RANGED_MOVE_RANGE,
                        false => crate::tuning::TACTICAL_MELEE_RANGE,
                    };
                    d <= reach
                })
                .collect(),
        };
        if moves.is_empty() {
            return None;
        }
        let idx = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..moves.len())
        };
        Some(moves[idx].clone())
    }
```

Then give `swing_move` the same treatment:

```rust
    pub(crate) fn swing_move(&mut self, entity: Entity) -> (String, battle::DamageRange) {
        self.swing_move_at(entity, None)
    }

    /// `swing_move` for a body swinging from `distance` cells away.
    ///
    /// The narrowing falls back to the unconstrained roll when nothing
    /// reaches, rather than answering the raw-signal-burst line: a body the
    /// gate has already let swing must have *something* to swing, and a
    /// modded species whose only reaching move was filtered out would
    /// otherwise narrate a fumble it did not make.
    pub(crate) fn swing_move_at(
        &mut self,
        entity: Entity,
        distance: Option<u32>,
    ) -> (String, battle::DamageRange) {
        if entity == self.player_entity() {
            return ("data strike".to_string(), PLAYER_UNARMED_DAMAGE);
        }
        let rolled = self
            .roll_species_move_in_range(entity, distance)
            .or_else(|| self.roll_species_move(entity));
        match rolled {
            Some(mv) => (mv.name.clone(), mv.attack_parts().0),
            None => ("a raw signal burst".to_string(), PLAYER_UNARMED_DAMAGE),
        }
    }
```

- [ ] **Step 4: Pass the distance from the battle map**

In `crates/engine/src/tactical/turn.rs`'s `tactical_attack`, replace

```rust
        let (move_name, natural) = self.swing_move(actor);
```

with

```rust
        let (move_name, natural) = self.swing_move_at(actor, Some(reach::distance(from, at)));
```

- [ ] **Step 5: Run and confirm they pass**

```bash
cargo test -p feral-processes-engine rolls_only 2>&1 | tail -20
cargo test -p feral-processes-engine either_move 2>&1 | tail -20
cargo test -p feral-processes-engine 2>&1 | tail -20
```

Expected: PASS. If a *battle* test's numbers moved, that is the drawn value
changing and is expected; read it, confirm the shape is the same, and update
the expectation. If a **count of draws** changed anywhere, stop — that is a
stream shift and this design says there should be none.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -20
git add -- crates/engine/src/game/combat_round.rs crates/engine/src/tactical/turn.rs crates/engine/src/tests/
git commit -m "feat(combat): a swing at range rolls only a move that reaches

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: The AI holds at its band

**Files:**
- Modify: `crates/engine/src/tactical/ai.rs:42` (`Intent`), `:52` (`band`), `:539` (`swing_at_best_neighbour`), and the site that builds `Intent::Swing`
- Test: `crates/engine/src/tests/tactical.rs`, and `ai.rs`'s own `mod tests`

**Interfaces:**
- Consumes: `Game::swing_range` (Task 2), the range gate (Task 3).
- Produces: `Intent::Swing { range: u32 }` (crate-private).

- [ ] **Step 1: Write the failing tests**

In `ai.rs`'s own `mod tests`, beside the existing `MELEE`/`STANDOFF`
constants:

```rust
/// A reaching body's band is its own range, not the melee constant — the
/// whole of what makes it hold off rather than walk into arm's length.
#[test]
fn a_reaching_bodys_band_is_its_own_range() {
    let swing = Intent::Swing { range: 2 };
    assert_eq!(swing.band(), AbilityRange { min: 0, max: 2 });
    assert_eq!(
        shortfall((0, 0), (2, 0), swing.band()),
        0,
        "two cells is inside a range-2 band"
    );
    assert_eq!(
        shortfall((0, 0), (4, 0), swing.band()),
        2,
        "four cells is two short of the band"
    );
}
```

In `tests/tactical.rs`, one behavioural test that the filter widened:

```rust
/// A reaching hostile swings from where it stands instead of passing.
#[test]
fn a_reaching_hostile_swings_without_closing() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    // A `drone` reaches two. Stand it exactly two cells off and hand it the
    // turn; it must spend its action rather than only walking.
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    let before = hp_of(&game, player);
    drive_turn_of(&mut game, pack[0]);
    assert!(hp_of(&game, player) < before, "the reaching hostile never swung");
}
```

`hp_of` and `drive_turn_of` — **check `tests/tactical.rs` and
`tests/support.rs` for existing spellings first**; `wait_for_turn` already
exists in that file and is likely half of `drive_turn_of`. The pack's species
must be one with a ranged move; `tactical_pack` takes the *first* species in
the db, so this test needs to spawn its own `drone` hostile rather than use
`tactical_pack` — do that with `body(&mut game, "drone")` plus a `Hostile`
component, following `tactical_pack`'s own spawn.

- [ ] **Step 2: Run and confirm they fail**

```bash
cargo test -p feral-processes-engine band_is_its_own_range 2>&1 | tail -20
```

Expected: compile failure — `Intent::Swing` is a unit variant.

- [ ] **Step 3: Carry the range on the intent**

```rust
enum Intent {
    /// The basic attack `tactical_attack` swings, carrying the range
    /// `Game::swing_range` answered for the body about to act.
    ///
    /// Carried on the variant rather than read inside `band()` because the
    /// band is decided **before the walk** and `band` takes no actor — and
    /// because a range read at swing time would let a body plan a standoff
    /// and then draw the melee half of its move pair.
    Swing { range: u32 },
    Routine(AbilityDef),
}
```

```rust
    fn band(&self) -> AbilityRange {
        match self {
            Intent::Swing { range } => AbilityRange {
                min: 0,
                max: *range,
            },
            Intent::Routine(def) => def.tactical_range(),
        }
    }
```

`helpful()`'s `Intent::Swing => false` arm becomes
`Intent::Swing { .. } => false`.

At the site that constructs `Intent::Swing`, build it as
`Intent::Swing { range: self.swing_range(actor) }`. **Find every construction
site** — `rg 'Intent::Swing' crates/engine/src/tactical/ai.rs` — and fix each;
the compiler will point at them.

- [ ] **Step 4: Widen the target filter**

In `swing_at_best_neighbour`, replace the `TACTICAL_MELEE_RANGE` filter:

```rust
        let range = self.swing_range(actor);
        let battle = self.world.resource::<TacticalBattle>();
        let Some(from) = battle.cell_of(actor) else {
            return;
        };
        let mut reachable: Vec<(i32, i32, Entity)> = targets
            .iter()
            .filter(|&&cell| distance(from, cell) <= range)
            // Asked here as well as at the gate, so a body does not spend
            // its turn swinging at something it cannot see and calling that
            // its action.
            .filter(|&&cell| line_of_sight(&battle.board, from, cell))
            .filter_map(|&cell| battle.occupant(cell).map(|e| (cell.1, cell.0, e)))
            .collect();
```

`swing_range` takes `&self` and the `battle` borrow is also `&self`, so the
`let range = ...` must come **before** the resource borrow, as written.
`line_of_sight` is already imported at the head of `ai.rs`.

Rename nothing else — `swing_at_best_neighbour`'s name is now slightly
generous, but a rename is churn outside this change's blast radius. Update
its doc comment's first line to "Swings at the reachable target with the
least Integrity left."

If `TACTICAL_MELEE_RANGE` is now unused in `ai.rs`, drop it from the `use`.

- [ ] **Step 5: Run and confirm they pass**

```bash
cargo test -p feral-processes-engine tactical 2>&1 | tail -30
cargo test -p feral-processes-engine ai 2>&1 | tail -30
```

Expected: PASS.

- [ ] **Step 6: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -20
git add -- crates/engine/src/tactical/ai.rs crates/engine/src/tests/
git commit -m "feat(tactical): a reaching hostile holds at its band

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: The bolt cue and its queue

**Files:**
- Modify: `crates/engine/src/resources.rs` (beside `TransitCue`/`TransitQueue`, ~line 690-730)
- Modify: `crates/engine/src/lib.rs:118` (resource registration and re-export)
- Modify: `crates/engine/src/game/base/upkeep.rs:206` (`take_bolts` beside `take_transits`)
- Modify: `crates/engine/src/tactical/turn.rs` (push inside `tactical_attack`)
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Consumes: the range gate (Task 3).
- Produces: `resources::BoltCue { from: (i32, i32), to: (i32, i32), color: components::GlyphColor }`;
  `resources::BoltQueue`;
  `pub fn Game::take_bolts(&mut self) -> Vec<crate::resources::BoltCue>`;
  `BoltCue` re-exported from the crate root beside `TransitCue`.

- [ ] **Step 1: Write the failing tests**

```rust
/// One streak per body actually swung at, from the swinger's cell to each
/// recipient's — so a reach weapon's sweep fires one at every body its shape
/// caught.
#[test]
fn a_swing_queues_one_bolt_per_body_it_lands_on() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (3, 0));
    assert!(game.tactical_attack(pack[0]));

    let bolts = game.take_bolts();
    assert_eq!(bolts.len(), 1, "one body swung at, one streak");
    assert_eq!(bolts[0].from, (0, 0));
    assert_eq!(bolts[0].to, (3, 0));
}

/// Draining is a drain — a second call comes back empty, so a frontend
/// cannot draw one streak twice.
#[test]
fn taking_the_bolts_empties_the_queue() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
    assert_eq!(game.take_bolts().len(), 1);
    assert!(game.take_bolts().is_empty());
}

/// An adjacent swing queues one too — the streak is one rule at every
/// distance, and at one cell it is the melee feedback.
#[test]
fn an_adjacent_swing_queues_a_bolt_too() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
    assert_eq!(game.take_bolts().len(), 1);
}
```

A save round-trip test that no bolt survives: **look for the existing
save/load round-trip test in `crates/engine/src/tests/`** and add an
assertion to it rather than building a second round trip. `BoltQueue` carries
no `Serialize` derive and is not in `SaveData`, so the property is an
omission; the assertion is that `take_bolts()` is empty on a freshly loaded
`Game` after one was queued before the save.

- [ ] **Step 2: Run and confirm they fail**

```bash
cargo test -p feral-processes-engine bolt 2>&1 | tail -20
```

Expected: compile failure — no `take_bolts`.

- [ ] **Step 3: Add the value and the queue**

In `crates/engine/src/resources.rs`, after `TransitQueue`:

```rust
/// A blow travelling from the body that swung it to the body it landed on,
/// for a frontend to draw as a streak.
///
/// **In `TacticalBattle` cells and not world coordinates**, which is why
/// this is its own value rather than a fifth `EffectKind`: `VisualEffect`'s
/// whole shape is a *world* tile, and pushing a board cell into that queue
/// would pin a flash to an unrelated tile out in the zone — exactly the
/// convenience the `Position` seam refuses.
///
/// **No kind field.** The streak travels `from` → `to` by one rule, and at
/// one cell that is a short flick across a single square — which is the
/// melee feedback, for free, and keeps a renderer from restating the melee
/// threshold to choose between two draws.
///
/// The colour is carried because it is the swinger's, and the swinger may be
/// dead by the time this is drawn: a fumble's Recoil rung can kill the body
/// that swung, and a lookup would then have nothing to ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoltCue {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub color: crate::components::GlyphColor,
}

/// Bolts queued since the last `Game::take_bolts` — `TransitQueue`'s
/// counterpart on a battle map, capped and drained the same way, and
/// **deliberately not serialized**: a blow in flight has nothing to say to a
/// reloaded save.
#[derive(Resource, Default)]
pub struct BoltQueue {
    cues: Vec<BoltCue>,
}

impl BoltQueue {
    pub(crate) fn push(&mut self, cue: BoltCue) {
        self.cues.push(cue);
        if self.cues.len() > EFFECT_QUEUE_CAP {
            let excess = self.cues.len() - EFFECT_QUEUE_CAP;
            self.cues.drain(0..excess);
        }
    }

    pub fn take(&mut self) -> Vec<BoltCue> {
        std::mem::take(&mut self.cues)
    }
}
```

- [ ] **Step 4: Register and re-export**

In `crates/engine/src/lib.rs`, add `BoltQueue` to the `use crate::resources::{...}`
list at line 118 and to wherever `TransitQueue` is inserted as a resource at
`Game::new` — **follow `TransitQueue` exactly; `rg 'TransitQueue' crates/engine/src/lib.rs`
finds both sites.** Re-export `BoltCue` from the crate root beside
`TransitCue` so gui can name it.

**A new `Resource` shifts bevy's query iteration order.** Expect unrelated
seeded tests to move. If any do, that is the known consequence and the fix is
to update the expectation, not to avoid the resource.

- [ ] **Step 5: Add the drain**

In `crates/engine/src/game/base/upkeep.rs`, after `take_transits`:

```rust
    /// Drains every `BoltCue` queued since the last call — `take_transits`'
    /// counterpart on a battle map.
    ///
    /// A frontend that draws no bolts must still call it, or the queue sits
    /// at its cap forever.
    pub fn take_bolts(&mut self) -> Vec<crate::resources::BoltCue> {
        self.world
            .resource_mut::<crate::resources::BoltQueue>()
            .take()
    }
```

- [ ] **Step 6: Push from the swing**

In `crates/engine/src/tactical/turn.rs`'s `tactical_attack`, inside the
`for (index, body) in bodies.into_iter().enumerate()` loop, **before**
`resolve_and_apply_attack` — so a body that dies to the blow still gets its
streak drawn:

```rust
            if let Some(to) = self.world.resource::<TacticalBattle>().cell_of(body) {
                let color = self.glyph_color_of(actor);
                self.world
                    .resource_mut::<crate::resources::BoltQueue>()
                    .push(crate::resources::BoltCue { from, to, color });
            }
```

`glyph_color_of` is the accessor for a body's authored `GlyphColor`. **Find
what this codebase actually calls it** — `rg 'GlyphColor' crates/engine/src/game/`
— and use that; `components::GlyphColor` is likely just a component on the
body, in which case read it directly with
`self.world.get::<GlyphColor>(actor).copied().unwrap_or_default()`. Do not
invent a new accessor if one exists.

- [ ] **Step 7: Run and confirm they pass**

```bash
cargo test -p feral-processes-engine bolt 2>&1 | tail -20
cargo test -p feral-processes-engine 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -20
git add -- crates/engine/src/resources.rs crates/engine/src/lib.rs crates/engine/src/game/base/upkeep.rs crates/engine/src/tactical/turn.rs crates/engine/src/tests/
git commit -m "feat(tactical): a swing queues a travelling bolt per body it lands on

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: gui draws the bolt

**Files:**
- Modify: `crates/gui/src/fx.rs` (constants at the head, `Bolt` beside `Walker:455`, `bolts` field `:489`/`:503`, `begin_frame:516`, `draw_bolts` beside `draw_walkers:651`)
- Modify: `crates/gui/src/lib.rs:596-601` (the drain) and `:855` (the second drain site)
- Modify: `crates/gui/src/render/tactical.rs` (`draw_tactical_map`, over the tiles)
- Test: `crates/gui/src/fx.rs`'s own `mod tests`, `crates/gui/src/render/tactical.rs`'s own `mod tests`

**Interfaces:**
- Consumes: `Game::take_bolts()`, `BoltCue` (Task 6).
- Produces: `Fx::draw_bolts(&self, painter: &Painter, to_px: impl Fn((i32, i32)) -> (f32, f32), tile_px: f32)`;
  `fx::BOLT_SECONDS`.

- [ ] **Step 1: Write the failing tests**

In `fx.rs`'s `mod tests`:

```rust
/// A bolt is gone before the next body acts. One tactical beat is
/// `1.0 / TACTICAL_TURNS_PER_SECOND`; a streak still on screen when the next
/// swing lands reads as two attacks at once.
#[test]
fn a_bolt_expires_inside_one_tactical_beat() {
    assert!(
        BOLT_SECONDS < (1.0 / feral_processes_app_core::TACTICAL_TURNS_PER_SECOND) as f64,
        "a bolt outlives the beat it was fired in"
    );
}

/// It is drawn while it lives and not once it has expired.
#[test]
fn a_bolt_is_drawn_while_it_lives_and_not_after() {
    let mut fx = Fx::new();
    let cue = BoltCue {
        from: (0, 0),
        to: (3, 0),
        color: GlyphColor::Cyan,
    };
    fx.begin_frame(0.0, Vec::new(), Vec::new(), vec![cue], true);
    let to_px = |(x, y): (i32, i32)| (x as f32 * 16.0, y as f32 * 16.0);
    let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
    assert!(!shapes.is_empty(), "a live bolt drew nothing");

    fx.begin_frame(BOLT_SECONDS + 0.01, Vec::new(), Vec::new(), Vec::new(), true);
    let (_, shapes) = crate::paint::with_painter(|p| fx.draw_bolts(p, to_px, 16.0));
    assert!(shapes.is_empty(), "an expired bolt is still drawing");
}
```

`Fx::new()` and `crate::paint::with_painter` are both already used by this
module's `draw_walkers` tests at `fx.rs:926` — copy their exact idiom.
`GlyphColor::Cyan` may be spelled differently; check `components::GlyphColor`'s
variants.

- [ ] **Step 2: Run and confirm they fail**

```bash
cargo test -p feral-processes-gui bolt 2>&1 | tail -20
```

Expected: compile failure — no `BOLT_SECONDS`, no `draw_bolts`, and
`begin_frame` takes four arguments.

- [ ] **Step 3: Add the constant**

At the head of `crates/gui/src/fx.rs`, near `HIT_FLASH_SECONDS`:

```rust
/// How long a blow's streak takes to cross from the swinger to what it hit.
///
/// **Derived from the tactical turn rate rather than restated**, so a retune
/// of the pace cannot leave a streak still in flight when the next body
/// acts — which reads as two attacks landing at once. A third of a beat: long
/// enough to follow across three cells, short enough to leave a clear gap
/// before the next turn.
const BOLT_BEAT_FRACTION: f64 = 1.0 / 3.0;
pub const BOLT_SECONDS: f64 =
    (1.0 / feral_processes_app_core::TACTICAL_TURNS_PER_SECOND as f64) * BOLT_BEAT_FRACTION;

/// How thick the streak is drawn, and how long its lit head is as a fraction
/// of the whole flight. A head rather than a full line, so the eye reads a
/// direction rather than a static beam.
const BOLT_THICKNESS_PX: f32 = 2.5;
const BOLT_HEAD_FRACTION: f32 = 0.35;
```

`TACTICAL_TURNS_PER_SECOND` is `f32` and `pub` in app-core; confirm gui's
`Cargo.toml` already depends on `feral_processes_app_core` (it does — `lib.rs`
uses `Mode` from it). If `const` arithmetic over the `as f64` cast is
rejected, make `BOLT_SECONDS` a `fn` returning the same expression rather
than hard-coding `0.208`.

- [ ] **Step 4: Add the record, the field and the feed**

Beside `Walker`:

```rust
struct Bolt {
    from: (i32, i32),
    to: (i32, i32),
    color: GlyphColor,
    start: f64,
}
```

Add `bolts: Vec<Bolt>` to `Fx` and `bolts: Vec::new()` to its constructor.
Widen `begin_frame` to take `bolts: Vec<BoltCue>` after `transits`, push one
`Bolt` per cue inside the existing `if self.enabled` block, and retain
alongside the others:

```rust
        self.bolts.retain(|b| now - b.start < BOLT_SECONDS);
```

**No stagger.** Walkers stagger because a squad files out one behind the
other; a sweep's streaks all leave the same swinger in the same instant and
staggering them would read as several attacks.

- [ ] **Step 5: Add the draw**

Beside `draw_walkers`:

```rust
    /// Every blow currently in flight, as a lit head running from the body
    /// that swung to the body it landed on.
    ///
    /// `to_px` answers a cell's top-left, so the streak is offset half a tile
    /// to run centre to centre — a line drawn corner to corner leaves the
    /// glyph it came from and arrives beside the one it hit.
    pub fn draw_bolts(
        &self,
        painter: &Painter,
        to_px: impl Fn((i32, i32)) -> (f32, f32),
        tile_px: f32,
    ) {
        let half = tile_px / 2.0;
        for bolt in &self.bolts {
            let along = ((self.now - bolt.start) / BOLT_SECONDS).clamp(0.0, 1.0) as f32;
            let (ax, ay) = to_px(bolt.from);
            let (bx, by) = to_px(bolt.to);
            let (ax, ay) = (ax + half, ay + half);
            let (bx, by) = (bx + half, by + half);
            let tail = (along - BOLT_HEAD_FRACTION).max(0.0);
            let point = |t: f32| (ax + (bx - ax) * t, ay + (by - ay) * t);
            let (hx, hy) = point(along);
            let (tx, ty) = point(tail);
            let base = palette::glyph(bolt.color);
            // Fades as it travels, so the eye is pulled to the arrival
            // rather than left looking at a beam.
            let color = Color::new(base.r, base.g, base.b, base.a * (1.0 - along * 0.4));
            painter.line(tx, ty, hx, hy, BOLT_THICKNESS_PX, color);
        }
    }
```

- [ ] **Step 6: Drain it and draw it**

In `crates/gui/src/lib.rs`, extend **both** drain sites (~line 596 and ~line
855) to take `game.take_bolts()` and pass it to `begin_frame`. The `None`
arm gains a third `Vec::new()`. The comment already there — "Effects are
drained every frame whether or not they'll be drawn, so a disabled `Fx` can't
leave the engine's queue at its cap" — covers bolts too and needs no change.

In `crates/gui/src/render/tactical.rs`'s `draw_tactical_map`, after the
bodies are drawn and before the pane border, mirroring
`render/base.rs:1153`:

```rust
    // Over the bodies, because a blow travelling to a body passes in front
    // of it, and because a streak under a glyph is invisible at three cells.
    fx.draw_bolts(
        painter,
        |cell| tile_origin_px(cell, center, (half_w, half_h), (off_x, off_y), tile_px, pane),
        tile_px,
    );
```

Confirm `tile_origin_px`'s exact argument order against its use earlier in
the same function rather than trusting this snippet.

Add a render test in that file's `mod tests` following the module's existing
pattern (there are already several using `Fx::new()`): a fight with one live
bolt draws more shapes than the same fight with none.

- [ ] **Step 7: Run and confirm they pass**

```bash
cargo test -p feral-processes-gui bolt 2>&1 | tail -20
cargo test -p feral-processes-gui 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 8: Format, lint, commit**

```bash
cargo fmt && cargo clippy --workspace --all-targets 2>&1 | tail -20
git add -- crates/gui/src/fx.rs crates/gui/src/lib.rs crates/gui/src/render/tactical.rs
git commit -m "feat(gui): a blow travels from the swinger to what it hit

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: The inspect page states the range, and the whole-branch gate

**Files:**
- Modify: `crates/engine/src/views.rs:417` (beside `WornDetailView::reach`)
- Modify: `crates/engine/src/game/catalog.rs:529` (the struct literal), `:544` (beside `reach_line`)
- Test: `crates/engine/src/tests/` — wherever `reach_line`'s row is already asserted

**Interfaces:**
- Consumes: `ItemDef::range` (Task 1).
- Produces: `views::WornDetailView::range: Option<String>`.

- [ ] **Step 1: Write the failing test**

```rust
/// A weapon that reaches says so on its own page — a range the player can
/// only discover by being refused a swing is a mechanic with no surface.
#[test]
fn a_reaching_weapons_page_states_its_range() {
    let mut game = game();
    let detail = game
        .gear_detail(&plain_copy("plasma_router"))
        .expect("a shipped weapon has a page");
    let worn = detail.worn.expect("a weapon is wearable");
    assert_eq!(worn.range.as_deref(), Some("Range: 3 cells"));
}

/// A melee weapon states nothing — an "arm's length" row on every blade in
/// the game is a row that stops being read.
#[test]
fn a_melee_weapons_page_states_no_range() {
    let mut game = game();
    let detail = game
        .gear_detail(&plain_copy("shim_blade"))
        .expect("a shipped weapon has a page");
    assert!(detail.worn.expect("wearable").range.is_none());
}
```

`gear_detail`'s exact signature and `plain_copy` — **read the existing
`reach_line` tests and copy their setup**; `GearDetail`'s shape (whether
`worn` is an `Option` at all) must come from `views.rs`, not from this
snippet.

- [ ] **Step 2: Run and confirm they fail**

```bash
cargo test -p feral-processes-engine states_its_range 2>&1 | tail -20
```

Expected: compile failure — no field `range` on `WornDetailView`.

- [ ] **Step 3: Add the field and the line**

In `crates/engine/src/views.rs`, after `reach`:

```rust
    /// How far this weapon may be swung from, as the inspect page says it,
    /// or `None` for one that swings at arm's length.
    ///
    /// Absent rather than "Range: 1 cell": every blade in the game would
    /// carry that row, and a row on everything is a row nobody reads.
    ///
    /// Formatted in the engine for `reach`'s reason — a read-only screen's
    /// row count is app-core's, and a per-row transform in the renderer
    /// opens the page on a row that is not drawn.
    pub range: Option<String>,
```

In `crates/engine/src/game/catalog.rs`, after `reach_line`:

```rust
    /// What a weapon's range is, as the inspect page says it.
    ///
    /// Read off the def rather than through `Game::swing_range`, which
    /// answers for a *body* and would report a species figure for a weapon
    /// nobody is holding — `reach_line`'s own split between what a weapon
    /// *is* and what this instant's swing would do.
    fn range_line(&self, item: &ItemId) -> Option<String> {
        let range = self.world.resource::<ItemDb>().get(item.as_str())?.range?;
        Some(format!("Range: {range} cells"))
    }
```

and add `range: self.range_line(&copy.item),` to the `WornDetailView` literal
beside `reach`.

- [ ] **Step 4: Run and confirm they pass**

```bash
cargo test -p feral-processes-engine states_its_range 2>&1 | tail -20
cargo test -p feral-processes-engine 2>&1 | tail -20
```

Expected: PASS. If the gui's gear-inspect renderer destructures
`WornDetailView`, it will fail to compile until it draws the new row — draw
it beside the reach row, in the same style.

- [ ] **Step 5: The whole-branch gate**

```bash
cargo fmt
cargo clippy --workspace --all-targets 2>&1 | tail -30
cargo test --workspace 2>&1 | tail -30
```

All three must be clean. Do not pipe the test run through `grep` or `head`
in a way that swallows the exit code — this repo has been bitten by that;
read the tail and confirm the summary line says `0 failed`.

- [ ] **Step 6: The balance check**

```bash
cargo test -p feral-processes-engine balance_sim 2>&1 | tail -20
```

**Expected: no change.** `balance_sim` models no geometry and cannot see a
range, so a moved curve here means something other than this feature moved —
most likely the new `BoltQueue` resource shifting bevy's query order, which
is a known consequence and not a difficulty change. If a curve moves,
investigate before updating it.

Then the real instrument:

```bash
cargo run --bin arena -- dev-arenas/opening-fight.ron
cargo run --bin arena -- dev-arenas/full-group.ron
```

Record what the reaching weapons did to the outcome. **Arena numbers compare
within one build only** — a moved baseline is a reshuffled RNG stream, not a
difficulty change — so run the same two scenarios on `main` for comparison if
a number looks alarming, rather than reasoning from memory.

- [ ] **Step 7: Commit**

```bash
git add -- crates/engine/src/views.rs crates/engine/src/game/catalog.rs crates/engine/src/tests/ crates/gui/src/render/
git commit -m "feat(items): a weapon's page states the range it swings from

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

- [ ] **Step 8: Record the three seam writes**

This change adds load-bearing seams. Per the `seams` skill, each is **three
writes in this order** — the argument to the memory graph, the trap to
`.claude/skills/seams/references/combat.md`, and the rule to `CLAUDE.md` as
**exactly one sentence**.

Two seams to record:

1. `Game::swing_range` is the one door for how far a body swings, and its
   three readers are calls rather than copies. The trap: all three hard-coded
   `TACTICAL_MELEE_RANGE` and nothing fails to compile when one drifts — a
   body whose band says two and whose filter says one walks into position and
   passes. Plus: a worn weapon replaces the species figure rather than being
   maxed against it, and the range is a property of the body rather than of
   the move it rolls, because the AI picks its intent before it walks.
2. `BoltCue` lives in `TacticalBattle` cells and is its own queue, never a
   fifth `EffectKind`. The trap: `VisualEffect::pos` is world coordinates, so
   a board cell pushed into `EffectQueue` pins a flash to an unrelated tile
   out in the zone — the `Position` seam's convenience in a new place. Plus:
   no `kind` field, because one travel rule at every distance is also the
   melee feedback and keeps gui from restating the melee threshold.

Also worth a line under **Tactical battles** in `CLAUDE.md`: a swing needs
line of sight, checked unconditionally because `line_of_sight` excludes its
endpoints and is already a no-op for neighbours.

```bash
git add -- CLAUDE.md .claude/skills/seams/references/combat.md
git commit -m "docs(seams): the swing-range door and the bolt cue

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

- [ ] **Step 9: Report honestly**

State plainly, once, that the suite is green and **nothing here has been seen
on a screen** — an agent in this environment cannot run the game. The bolt's
appearance, its speed and whether a two-cell standoff feels right are play
questions and belong to the user.

---

## Notes for the executor

- **Don't reach for a fresh `Game::new` when a `dev-saves/` template would
  do**, but for these engine tests the existing `tests/tactical.rs` fixtures
  are the right tool and are already written.
- **Engine test fixtures live in `crates/engine/src/tests/support.rs`.** Look
  there before writing a new one.
- If many tests fail at once with `NotFound` on an assets path, that is stale
  build artifacts from an old directory rename, not a real failure:
  `cargo clean -p feral-processes-engine -p feral-processes-app-core`. Never a
  full `cargo clean` — that costs a ~3.5-minute cold rebuild of the Bevy
  graph.
- `cargo test -p feral-processes-engine` and `cargo test --workspace` are
  **different builds and different RNG streams**. A test green under one and
  red under the other is that, not a flake.
- Do not push. Do not merge. Do not bump the version or write a changelog
  section.
