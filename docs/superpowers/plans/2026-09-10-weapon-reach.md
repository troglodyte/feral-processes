# Weapon Reach Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A weapon may declare how wide its swing is. The wielder attacks the
way they always have; when the weapon's charge is up, the swing lands on
every member of the group in the abstract model and on every body in the
shape on a battle map.

**Architecture:** One `#[serde(default)]` field on `ItemDef` carrying an
enemy-facing plural `AbilityTarget`, an optional `AbilityShape` override and
a `recharge`. `Game::swing_reach` is the one door that decides whether a
swing is wide; the conversion to bodies is each combat model's own existing
converter — `ability_recipients` for the abstract model, `reach::recipients`
for the board. A battle-scoped `components::ReachCharge` holds the recharge
and is cleared where every other battle-scoped component is.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine is standalone `bevy_ecs`; gui
is full Bevy + `bevy_egui`). RON assets. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-10-weapon-reach-design.md` — read
it first. This plan argues from it and does not repeat its reasoning.

## Global Constraints

- **Repo conventions are in `CLAUDE.md` and are not optional.** Read it. In
  particular: no hardcoded content in Rust that could be data; new tuning
  values go in `crates/engine/src/tuning.rs` with a doc comment stating the
  number's provenance; comments explain *why*, never *what*.
- **This plan deliberately contains no finished implementation code**, per
  `CLAUDE.md`'s process-weight rule. You get the file list, exact signatures,
  the intent of each test, and the gates. Write the code yourself. The two
  code blocks below are there because they are non-obvious: a `Copy` derive
  that decides whether the sweep borrow-checks, and an ownership trick that
  arms the charge once a turn.
- **No `SAVE_FORMAT_VERSION` bump.** The reach lives on `ItemDef` and is
  resolved from the id every load; `ReachCharge` is battle-scoped and must
  appear nowhere in `crates/engine/src/save.rs`. If you find yourself adding
  a save field, stop — you have put the reach in the wrong place.
- **`EquipmentStats` must not gain a field.** Its `is_empty` and `has_upside`
  destructure on `cell_mark`'s rule, and `Game::copy_bonus`'s four scaling
  axes would scale a reach. The existing
  `every_weapon_authors_a_range_and_nothing_else_does`
  (`crates/engine/src/tests/assets.rs:2324`) must pass untouched.
- **`reach::recipients` must not learn the word `Hostile`.** Full friendly
  fire on a board is the seam's own rule and Task 4 has a test that holds it.
  A side filter there is the trap; do not add one, and do not add one to the
  weapon sweep either.
- **`Game::copy_power` and `balance_sim.rs` gain no term.** The spec's
  decision. If a `balance_sim` curve moves, the shipped weapons are priced
  wrong — fix the `.ron`, not the test.
- **TDD, every task.** Failing test first, watch it fail for the right
  reason, minimal implementation, watch it pass, commit.
- **A test that passes with the fix removed is not a test.** For every
  behavioural assertion, delete the implementation line it targets and
  confirm the test goes red before you commit. This repo has shipped vacuous
  tests before.
- **Every refusal is asserted separately.** Task 1 ships three load refusals
  and needs three tests; one test over one path passes against the other two.
- **Gate for every task:** `cargo test -p feral-processes-engine` plus the
  named tests. **Gate before the final commit:** `cargo test --workspace`,
  `cargo clippy --workspace`, `cargo fmt`.
- **Engine test fixtures live in `crates/engine/src/tests/support.rs`.** Look
  there before writing a new one. `dev-saves/` templates exist too — see
  `CLAUDE.md` — but nothing in this plan needs a mid-run world.
- **Do not push.** Commit freely on the current branch
  (`claude/weapon-reach`); landing is the user's call.

## File Structure

**Engine — new behaviour**

- `crates/engine/src/items.rs` — `WeaponReach` beside `EquipmentStats`. It
  goes here and not in `items_db.rs` because `items.rs` is where the
  gear-shaped types already live (`GearCopy`, `EquipmentStats`,
  `EquipmentSlot`).
- `crates/engine/src/game/combat.rs` — `Game::swing_reach`, beside
  `attack_range` and `attacks_for`, which are the other two "what does this
  body's weapon do to its swing" questions.

**Engine — touched**

- `crates/engine/src/items_db.rs` — the `ItemDef::reach` field and
  `ItemDef::unreachable_reach`, called from `ItemDb::load_dir` in
  `ungrantable_ability`'s position (~line 367).
- `crates/engine/src/components.rs` — `ReachCharge`, beside `Cloaked`.
- `crates/engine/src/game/combat_teardown.rs` —
  `clear_battle_status_effects` (~line 162) drops it on all three sweeps.
- `crates/engine/src/game/combat_round.rs` — `party_member_attacks` (~line
  367) and `party_member_swing` (~line 406).
- `crates/engine/src/tactical/turn.rs` — `tactical_attack` (~line 231).
- `crates/engine/src/game/catalog.rs` — `Game::gear_detail` (~line 427).
- `crates/engine/src/views.rs` — one field on `WornDetailView`
  (reached through `GearDetailView::worn`, ~line 206). It goes there and not
  on `GearDetailView` because `worn` is documented as *the block that only
  means something for a wearable copy*, which is exactly what a reach is.

**Assets**

- `assets/items/scatter_lance.ron`, `assets/items/broadcast_storm.ron`
  *(new)*.
- `assets/items/README.md` — the `reach` field.

**gui**

- `crates/gui/src/render/inventory.rs` — the gear page row and
  `GEAR_AFFIX_ROW_CAP`'s neighbourhood (~line 533), plus
  `the_tallest_gear_page_fits_its_popup` (~line 975).

**Tests**

- `crates/engine/src/tests/weapon_reach.rs` *(new)* — declare it in
  `crates/engine/src/tests/mod.rs` alongside `cloak`, `combat_targeting` and
  `equipment`.
- `crates/engine/src/tests/assets.rs` — two new censuses.
- `crates/engine/src/tests/gear_detail.rs` — the page row.

---

### Task 1: The field and its three refusals

Nothing swings wide at the end of this task. The deliverable is that an item
file can declare a reach, that a malformed one is skipped with a warning
rather than a panic, and that the existing weapon census still passes.

**Files:**
- Modify: `crates/engine/src/items.rs` (add `WeaponReach` near
  `EquipmentStats`, ~line 507)
- Modify: `crates/engine/src/items_db.rs` (`ItemDef` ~line 111,
  `unreachable_reach` beside `ungrantable_ability` ~line 315, the call in
  `load_dir` ~line 367)
- Test: `crates/engine/src/tests/weapon_reach.rs` (create; declare in
  `crates/engine/src/tests/mod.rs`)

**Interfaces:**
- Consumes: `crate::abilities::{AbilityTarget, AbilityShape}` — both already
  `Copy + Serialize + Deserialize`. `AbilityTarget::is_ally_facing(self) ->
  bool`.
- Produces: `items::WeaponReach { target: AbilityTarget, shape:
  Option<AbilityShape>, recharge: u32 }`;
  `WeaponReach::tactical_shape(&self) -> AbilityShape` (authored, else
  `self.target.derived_shape()` — the same one-door rule
  `AbilityDef::tactical_shape` follows, and for the same reason: a reader
  taking `self.shape` directly resolves every shipped weapon to nothing);
  `ItemDef::reach: Option<WeaponReach>`;
  `ItemDef::unreachable_reach(&self) -> Option<String>`.

**`WeaponReach` must derive `Copy`.** This is the non-obvious part and it
decides whether Tasks 3 and 4 compile at all:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaponReach { /* … */ }
```

The def lives inside `ItemDb`, a world resource. Every sweep below holds
`&mut self` to call `resolve_and_apply_attack`, so a `&WeaponReach` borrowed
out of the resource cannot be held across the swing. `Copy` makes the
problem vanish instead of being fought with a clone at each site.

- [ ] **Step 1: Write three failing refusal tests plus one acceptance test.**

In `crates/engine/src/tests/weapon_reach.rs`. Each refusal test writes one
`.ron` into a temp asset directory and asserts `ItemDb::load_dir` returns a
warning naming that file and that the id is **absent** from the db. Follow
the pattern the existing `ungrantable_ability` tests use — find them with
`rg -n "ungrantable|is chosen on a turn" crates/engine/src/tests/`.

- `a_reach_on_a_non_weapon_is_refused_at_load` — an item with `reach:` and
  no `equipment: Some((Weapon, _))`.
- `a_reach_aimed_at_an_ally_is_refused_at_load` — `target: WholeParty`.
- `a_reach_that_reaches_one_body_is_refused_at_load` — `target:
  OneEnemyGroupFront`, and a second case with `shape: Some(Single)`. Both in
  this one test is fine; they are the same fault in two vocabularies.
- `a_reach_weapon_loads_and_derives_its_shape` — a well-formed weapon with
  `target: WholeEnemyGroup` and no `shape:` loads, and
  `tactical_shape()` answers `Radius { radius: TACTICAL_GROUP_RADIUS }`.

**Three separate refusal tests, not one.** A single test over one path
passes against an implementation that refuses only that path, which is the
whole point of the constraint above.

- [ ] **Step 2: Run them and confirm they fail.**

`cargo test -p feral-processes-engine weapon_reach`

Expected: all four fail to compile (`WeaponReach` does not exist). That is a
legitimate red — fix by adding the type, then re-run and confirm the
refusals fail on behaviour before implementing `unreachable_reach`.

- [ ] **Step 3: Add `WeaponReach` and the `ItemDef` field.**

`#[serde(default)]` on the field, so every existing item `.ron` and every
mod's keeps parsing. Doc comments state *why* the reach is on `ItemDef` and
not on `EquipmentStats` — the four scaling axes and the destructured
censuses. Cite the spec.

- [ ] **Step 4: Write `unreachable_reach` and call it from `load_dir`.**

Same shape as `ungrantable_ability`: `Option<String>` naming the fault,
pushed as `"skipped invalid item file {path:?}: {reason}"` and `continue`.
Place the call directly after the `ungrantable_ability` block. Never panic —
that is `SpeciesDb::load_dir`'s rule and it applies to every catalogue.

- [ ] **Step 5: Run the tests and the existing weapon census.**

```
cargo test -p feral-processes-engine weapon_reach
cargo test -p feral-processes-engine every_weapon_authors_a_range_and_nothing_else_does
```

Expected: PASS. The second must pass **untouched** — if you had to edit it,
the reach went into `EquipmentStats`.

- [ ] **Step 6: Verify the tests are not vacuous.**

Delete each of the three arms of `unreachable_reach` in turn, re-run, confirm
the matching test goes red and the other two stay green. Restore.

- [ ] **Step 7: Commit.**

`feat(items): a weapon may declare how wide its swing is`

---

### Task 2: The one door, and the charge that gates it

Still nothing swings wide. The deliverable is the shared decision and its
battle-scoped state, testable on its own.

**Files:**
- Modify: `crates/engine/src/components.rs` (`ReachCharge`, beside
  `Cloaked`)
- Modify: `crates/engine/src/game/combat.rs` (`Game::swing_reach`, beside
  `attack_range` ~line 25 and `attacks_for` ~line 63)
- Modify: `crates/engine/src/game/combat_teardown.rs`
  (`clear_battle_status_effects` ~line 162)
- Test: `crates/engine/src/tests/weapon_reach.rs`

**Interfaces:**
- Consumes: `Game::gear_bonus`'s route to the def —
  `world.get::<Equipment>(actor)` → `.weapon` → `EquippedItem::copy` →
  `GearCopy::item` → `ItemDb::get`. Do **not** route this through
  `gear_bonus`, which returns `EquipmentStats` and has no def to give you.
- Produces: `components::ReachCharge { pub ready_on: u32 }`;
  `Game::swing_reach(&self, actor: Entity) -> Option<items::WeaponReach>`;
  `Game::arm_reach_charge(&mut self, actor: Entity, recharge: u32)`.

`swing_reach` answers `Some` only when all three hold: the actor wears a
weapon whose def declares a reach; a fight is open (either
`BattleState` or `TacticalBattle` — read whichever is present for the round
number); and either the actor carries no `ReachCharge` or its `ready_on` is
`<=` the current round.

`arm_reach_charge` writes `ready_on = round + recharge`, inserting the
component on demand (`insert_if_new`, the idiom `Game::equip` uses for
`Equipment`). `recharge: 0` must be inert rather than special-cased — it
arms for the same round and so is immediately ready again. Nothing divides
by it.

The teardown **removes** the component rather than clearing a field —
`Cloaked`'s treatment, not `AbilityCooldowns`'. Use `get_entity_mut` and let
a missing entity be a no-op, because teardown is reached with entities that
have already despawned or left their group. It goes in all three sweeps
(player, every hostile still in the fight, every party member): a companion
can wear a reach weapon, and a mod can equip a hostile.

- [ ] **Step 1: Write three failing tests.**

- `a_weapon_with_no_reach_answers_none` — the baseline, so a later "always
  Some" bug is caught.
- `a_reach_is_unavailable_until_its_charge_is_ready` — arm it at round R with
  recharge 3, assert `swing_reach` is `None` at R+1 and R+2 and `Some` at
  R+3.
- `a_reach_charge_does_not_survive_the_fight` — arm it, run the fight to
  teardown, assert the component is gone and `swing_reach` answers `Some`.

- [ ] **Step 2: Run them and confirm they fail.**

`cargo test -p feral-processes-engine weapon_reach`

- [ ] **Step 3: Add `ReachCharge`.**

Doc comment says it is battle-scoped and therefore absent from `save.rs` —
`Cloaked`'s doc comment is the model to copy, including *why* that matters
(left set, it would follow the wielder out of the fight).

- [ ] **Step 4: Write `swing_reach` and `arm_reach_charge`.**

- [ ] **Step 5: Add the removal to `clear_battle_status_effects`.**

All three sweeps. The function's doc comment lists what it clears — update
it; a stale list there is how the fourth thing gets missed.

- [ ] **Step 6: Run the tests.**

`cargo test -p feral-processes-engine weapon_reach`

Expected: PASS.

- [ ] **Step 7: Verify the teardown test is not vacuous.**

Delete the removal from the player's sweep, re-run, confirm
`a_reach_charge_does_not_survive_the_fight` goes red. Restore.

- [ ] **Step 8: Commit.**

`feat(combat): one door decides whether a swing is wide`

---

### Task 3: The abstract model's sweep

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs`
  (`party_member_attacks` ~line 367, `party_member_swing` ~line 406)
- Test: `crates/engine/src/tests/weapon_reach.rs`

**Interfaces:**
- Consumes: `Game::swing_reach`, `Game::arm_reach_charge` (Task 2);
  `Game::ability_recipients(&self, actor: Entity, target: AbilityTarget,
  chosen: &battle::SpecialTarget) -> Vec<Entity>`;
  `Game::resolve_and_apply_attack`; `Game::party_swing_line`;
  `Game::log_swing`; `Game::reap_dead_members`.
- Produces: no new public surface. `party_member_swing`'s signature gains
  `reach: &mut Option<items::WeaponReach>`.

**Arming once a turn, not once a swing.** `attacks_for` loops a Striker's
second swing *inside* the turn, so a per-swing arm doubles the weapon
silently and `BattleState::planned` never learns the feature exists — this
is `proc_wielded_routine`'s rule. Do it with ownership rather than a
counter:

```rust
// in party_member_attacks, above the `for swing in 0..` loop
let mut reach = self.swing_reach(entity);
// …and inside party_member_swing, on the swing that spends it:
if let Some(spec) = reach.take() { /* sweep, then arm the charge */ }
```

`take()` leaves `None` behind, so every later swing this turn is narrow with
no second question asked.

The sweep itself: build the recipient list **once**, from
`ability_recipients(entity, spec.target, &SpecialTarget::EnemyGroup { group:
live })` where `live` is the already-retargeted index. Swing at the group's
front first, then at every other recipient. Each body takes its own
`resolve_and_apply_attack(entity, body, Swing::plain(range))` with the same
`range` — no new damage path, so mitigation, affinity and the fumble ladder
hold for free. Log one line per body through `party_swing_line`, which is
already one wording for both models.

Break the sweep on `!self.creature_alive(entity)`: a Recoil rung damages the
fumbler, so the swinger really can die on its own first body. `party_member_
attacks` already carries this guard for the same reason.

`reap_dead_members` stays where it is — after the sweep, not per body — so a
swing that empties a group cannot re-letter it half way through.

- [ ] **Step 1: Write four failing tests.**

- `a_weapon_reach_lands_on_every_member_of_the_group` — three members, one
  swing, all three lose Integrity. Seed the fight; do not rely on a natural
  encounter.
- `a_reach_arms_once_a_turn_and_not_once_a_swing` — a wielder whose
  `attacks_for` is 2 (a Striker; see the `attacks_for_resolves_the_player_
  and_a_wild_program_alike` test at `crates/engine/src/tests/combat.rs:1050`
  for how to set the class up). Assert the back rank is hit **once**, not
  twice.
- `a_recharging_reach_swings_narrow_until_it_is_ready` — round 1 wide, round
  2 narrow, back rank untouched on round 2.
- `a_fumble_that_kills_the_swinger_stops_the_sweep` — drive the swinger to
  1 HP against a fumble-forcing setup and assert the later recipients are
  untouched. If forcing a Recoil rung deterministically is impractical,
  assert the guard directly instead: a dead swinger mid-sweep leaves the
  remaining bodies at full Integrity. Say in the test's comment which of the
  two it is.

- [ ] **Step 2: Run them and confirm they fail.**

`cargo test -p feral-processes-engine weapon_reach`

- [ ] **Step 3: Thread `reach` through `party_member_attacks` and
      `party_member_swing`.**

- [ ] **Step 4: Write the sweep.**

- [ ] **Step 5: Run the tests, then the whole combat suite.**

```
cargo test -p feral-processes-engine weapon_reach
cargo test -p feral-processes-engine combat
```

Expected: PASS, and **no existing combat test may change**. A moved
assertion here means the narrow path is no longer narrow — a body with no
reach weapon must take exactly the swings it took before.

- [ ] **Step 6: Verify the tests are not vacuous.**

Delete the `take()` and arm unconditionally: `a_reach_arms_once_a_turn_and_
not_once_a_swing` must go red. Delete the recharge check in `swing_reach`:
`a_recharging_reach_swings_narrow_until_it_is_ready` must go red. Restore
both.

- [ ] **Step 7: Commit.**

`feat(combat): a reach weapon sweeps the whole group`

---

### Task 4: The battle map's sweep

**Files:**
- Modify: `crates/engine/src/tactical/turn.rs` (`tactical_attack` ~line 231)
- Test: `crates/engine/src/tests/weapon_reach.rs`

**Interfaces:**
- Consumes: `Game::swing_reach`, `Game::arm_reach_charge`;
  `tactical::reach::recipients(battle: &TacticalBattle, actor: Entity, aim:
  (i32, i32), shape: AbilityShape) -> Vec<Entity>`;
  `WeaponReach::tactical_shape` (Task 1).
- Produces: no new public surface.

The aim is the **cell of the adjacent body already being swung at**
(`battle.cell_of(target)`), which `tactical_attack` has resolved by the time
it reaches the swing. `recipients` reads it as a destination for a blast and
a bearing for a line or cone — that is its existing rule and needs no
special handling here.

The adjacency gate stays exactly as it is. A reach is breadth and never
distance; a weapon that reached further is a different feature.

`tactical_attack` is one swing per turn, so there is no once-per-turn
problem here — arm the charge after the sweep and move on.

**`reap_tactical_dead` needs no change.** It already sweeps every body on
the board and filters on `!creature_alive`, so multiple deaths in one swing
are handled. Do not "fix" it; its `Option<Entity>` argument is for
`settle_tactical`'s teardown, not for the body that was hit.

- [ ] **Step 1: Write three failing tests.**

Set `profile.tactical_battles = true` — see
`crates/engine/src/tests/tactical.rs:1212` for the fixture.

- `a_weapon_reach_lands_on_every_body_in_the_blast` — hostiles placed
  around the target, all inside the derived radius take Integrity.
- `a_weapon_reach_catches_a_companion_standing_beside_the_target` — **the
  friendly-fire rule, held explicitly.** Place a party member adjacent to the
  target and assert it takes damage. This test is what stops a later reader
  adding the side filter the seam warns about. Follow `recipients`' own test
  and place bodies carrying no components beyond what the sweep needs.
- `a_body_outside_the_shape_is_untouched` — the negative half, or the first
  test passes against "hit everything on the board".

- [ ] **Step 2: Run them and confirm they fail.**

`cargo test -p feral-processes-engine weapon_reach`

- [ ] **Step 3: Write the sweep in `tactical_attack`.**

- [ ] **Step 4: Run the tests, then the tactical suite.**

```
cargo test -p feral-processes-engine weapon_reach
cargo test -p feral-processes-engine tactical
```

Expected: PASS, no existing tactical test changed.

- [ ] **Step 5: Verify the friendly-fire test is not vacuous.**

Add a `Hostile` filter to the weapon sweep, re-run, confirm
`a_weapon_reach_catches_a_companion_standing_beside_the_target` goes red.
Remove the filter again — it must **not** ship.

- [ ] **Step 6: Commit.**

`feat(tactical): a reach weapon sweeps its shape`

---

### Task 5: One row on the gear page

**Files:**
- Modify: `crates/engine/src/game/catalog.rs` (`Game::gear_detail` ~line 427)
- Modify: `crates/engine/src/views.rs` (`GearDetailView` ~line 206)
- Modify: `crates/gui/src/render/inventory.rs` (~line 533 and the page
  builder below it)
- Test: `crates/engine/src/tests/gear_detail.rs`
- Test: `crates/gui/src/render/inventory.rs`
  (`the_tallest_gear_page_fits_its_popup` ~line 975)

**Interfaces:**
- Consumes: `Game::swing_reach`'s def lookup — but **not** `swing_reach`
  itself, which reads a live `ReachCharge` and a round number the page does
  not have. Read the def off the copy directly.
- Produces: `WornDetailView::reach: Option<String>`, `None` for a copy
  with no reach.

The row says what the swing lands on and how often, both read off the same
`WeaponReach` the swing reads. Formatted **in the engine**, for
`copy_name`'s reason and because a read-only screen's row count is owned by
app-core: a per-row transform in the renderer opens the page on a row that
is not drawn.

**That page has zero headroom and no scroll.** `draw_popup` pages a
`Row::Item` span and this page has none, so a row past the bottom is dropped
in silence. `GEAR_AFFIX_ROW_CAP` is where the affix rows were bought from
when they landed; buy this one the same way and re-run the census against a
reach weapon.

- [ ] **Step 1: Write the failing engine test.**

`the_gear_page_says_what_a_wide_swing_lands_on` in `gear_detail.rs`, beside
`the_worn_block_prices_the_whole_copy`. Assert the row exists on a reach
weapon and is `None` on `arc_lance`.

- [ ] **Step 2: Run it and confirm it fails.**

`cargo test -p feral-processes-engine gear_detail`

- [ ] **Step 3: Add the field and fill it in `gear_detail`.**

- [ ] **Step 4: Draw it in gui and buy the row.**

- [ ] **Step 5: Extend `the_tallest_gear_page_fits_its_popup`.**

The census must measure a copy carrying a reach **and** the affix rows and
the grant block at once — the tallest page, not a page with a reach on it.

- [ ] **Step 6: Run both.**

```
cargo test -p feral-processes-engine gear_detail
cargo test -p feral-processes-gui the_tallest_gear_page_fits_its_popup
```

Expected: PASS.

- [ ] **Step 7: Verify the layout census is not vacuous.**

Add a second reach row by hand, re-run the gui census, confirm it goes red.
Remove it.

- [ ] **Step 8: Commit.**

`feat(gui): the gear page says how wide a swing lands`

---

### Task 6: Two weapons, and the balance gate

**Files:**
- Create: `assets/items/scatter_lance.ron`,
  `assets/items/broadcast_storm.ron`
- Modify: `crates/engine/src/tests/assets.rs` (two censuses)
- Modify: `assets/items/README.md`

**Interfaces:**
- Consumes: everything above.
- Produces: item ids `"scatter_lance"` (Scatter Lance) and
  `"broadcast_storm"` (Broadcast Storm). Both names are network vocabulary,
  which is the register `CLAUDE.md` holds combat prose to — security, not
  swordplay.

Copy the house style from `assets/items/arc_lance.ron` — one line per field,
`equipment: Some((Weapon, (…)))`, a `description` that says what the item is
for.

| item | target | recharge | band | source |
| --- | --- | --- | --- | --- |
| Scatter Lance | `WholeEnemyGroup` | short | **below** the single-target ladder | `craftable`, `requires_structure` |
| Broadcast Storm | `AllEnemies` | long | **below** the single-target ladder | `droppable`, rare tier |

Three pricing rules bind these, all already enforced by censuses in
`crates/engine/src/tests/assets.rs`:

- `no_craftable_item_is_worth_more_than_its_ingredients` (~line 314) — a
  craftable worth more than its cost is an infinite Credit loop.
- `every_zone_gated_gear_recipe_asks_for_a_zone_material` (~line 1851) —
  applies **only** if you unlock the craftable through a research node with
  `min_zone > 0`. If you do, its recipe must name a `ZONE_MATERIALS` entry
  (~line 1839, currently `cache_grain`). If you gate it with
  `requires_structure` alone, this census does not apply — say which you
  chose in the commit message.
- The band is what pays for the reach. Set it below `arc_lance`'s
  `(min: 5, max: 10)` scaled for its tier, not above.

**Two new censuses**, in the house style of the file (a doc comment saying
what the rule protects, then the assertion over the real assets):

- `every_shipped_reach_weapon_authors_a_recharge` — `recharge > 0`. A reach
  on every swing is the balance hole the recharge exists to close, and
  `serde`'s `u32` default is 0, so nothing but this catches a forgotten
  field.
- `every_shipped_reach_weapon_is_enemy_facing` — belt and braces over
  `unreachable_reach`, asserted against the shipped tree rather than a temp
  file.

- [ ] **Step 1: Write the two censuses first, with no assets to satisfy
      them.**

They pass vacuously over an empty set. That is fine and expected — they are
guards for what Step 2 adds, and writing them first is what stops them being
written to match whatever got authored.

- [ ] **Step 2: Author the two `.ron` files.**

- [ ] **Step 3: Run the whole asset suite.**

`cargo test -p feral-processes-engine assets`

Expected: PASS, including the three pricing censuses above.

- [ ] **Step 4: Run the balance gate.**

`cargo test -p feral-processes-engine balance_sim`

Expected: PASS, **with no curve moved**. `balance_sim`'s fitted party
carries no reach weapon, so a moved curve means one of these two items
changed something it should not have — retune the `.ron`, never the test.
This is the spec's decision and it is the whole reason the weapons are
priced below the ladder.

- [ ] **Step 5: Document the field in `assets/items/README.md`.**

The `reach` field, its three fields, the three load refusals, and that the
shape is derived from `target` unless authored. This is the schema reference
a modder reads and `CLAUDE.md` requires it in the same change.

- [ ] **Step 6: Commit.**

`feat(items): the Scatter Lance and the Broadcast Storm swing wide`

---

### Task 7: Documentation, the seam, and the full gate

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md` (the **Items, gear and economy** seam list)
- Modify: `.claude/skills/seams/references/items.md`
- Create: a file under `docs/measurements/`
- Memory graph: one `seam:<slug>` entity

**A new seam is three writes, in this order** — the `seams` skill documents
it and the order matters:

1. **The argument** to the memory graph:
   `memory_add_observations(entity: "seam:<slug>", entityType: "seam",
   subsystem: "seams", observations: [<title>, <argument>])`. Two
   observations, title first. The argument is why the reach lives on
   `ItemDef` rather than `EquipmentStats`, why the weapon authors a *target*
   and not a shape, and why friendly fire on a board was kept.
2. **The trap** to `.claude/skills/seams/references/items.md`, as a bullet in
   the existing house style: a bold rule sentence, then the trap.
3. **The rule** to `CLAUDE.md` under **Items, gear and economy**, as **one
   sentence**. That budget is the point; `CLAUDE.md` is loaded every turn.

The rule sentence to write, or something better in the same shape:

> **A weapon's reach lives on `ItemDef` and is authored as an
> enemy-facing plural `AbilityTarget`**, so it is off `copy_bonus`'s four
> scaling axes by construction and each combat model converts it with the
> converter it already has.

- [ ] **Step 1: Write the measurement file.**

`docs/measurements/2026-09-10-weapon-reach-throughput.md`, following that
directory's `README.md` convention: the question, the commands that produced
the numbers, the numbers, and what the run was blind to. The question is the
throughput gap between the two shipped reach weapons and the single-target
ladder, and the blind spot is that `balance_sim` models no reach at all.

- [ ] **Step 2: The three seam writes, in order.**

- [ ] **Step 3: Add the `CHANGELOG.md` section.**

Its own `## X.Y.Z` section. Which digit moves is decided by
`CHANGELOG.md`'s own preamble — read it. No save format broke, so this is
not a breaking change by this repo's definition.

- [ ] **Step 4: The full gate.**

```
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

Expected: all green. Fix warnings rather than silencing them.

- [ ] **Step 5: Commit.**

`docs: the weapon reach seam, its measurement and its changelog`

---

## What this plan does not do

- **It does not extend a weapon's range.** Breadth only; the adjacency gate
  and the group choice are untouched. A reaching weapon is a different
  feature with a different aiming problem.
- **It does not add a term to `Game::copy_power` or to `balance_sim.rs`.**
  Both are the spec's explicit decisions. A reach weapon will rate below its
  single-target peer on the swap picker; the gear page row is the answer.
- **It does not ship a per-zone ladder.** Two weapons prove both target arms
  and both ends of the recharge. Widening the content is a second change,
  and it should wait until someone has played this one — **nothing here will
  have been seen on a screen**, and a green suite is not evidence of play.
- **It gives hostiles nothing.** Nothing inserts `Equipment` on a wild
  program, so the mechanic is player-side by omission rather than by a gate.
  The engine rules are written on the entity, so a mod that equips a hostile
  gets a coherent sweep for free.
