# Plan: companion refactoring

Spec: `docs/superpowers/specs/2026-08-11-companion-refactoring-design.md`. Read
it first — it carries the reasoning behind every decision below, and the
non-obvious ones (why percentages, why the `+1` floor, why the fusion fix is not
optional) are argued there rather than repeated here.

Nine steps, each ending green and committed. TDD throughout: the failing test
first, then the code. Branch is `companion-refactoring`, off `main`.

Assets land at step 2 rather than last because every engine test downstream
resolves real `.ron` files through `test_assets_dir`. A schema with no shipped
items to exercise it forces the whole battery onto modded fixtures.

---

## 1. Item schema

**Files:** `crates/engine/src/items_db.rs`, `assets/items/README.md`.

Add `CompanionUpgradeDef` beside `ConsumeDef` and
`ItemDef::upgrade: Option<CompanionUpgradeDef>`, every field `#[serde(default)]`.
Field list is in the spec.

Add the three percent fields to `ItemDef::non_finite_field` (`items_db.rs:142`).
Miss this and a NaN percent poisons arithmetic instead of rejecting the file.

`assets/items/README.md` gets the new field documented in the same commit — a
standing rule for any schema change, not a courtesy.

**Tests:** a `.ron` declaring `upgrade` parses with the expected values; one with
a NaN percent is skipped with a logged warning rather than panicking, following
`ItemDb::load_dir`'s existing pattern.

## 2. Assets

**Files:** 8 new `assets/items/*.ron`, 2 new `assets/structures/*.ron`
(`annealing_node`, `refactor_bench`), 1 new `assets/research/*.ron`;
`assets/structures/README.md` and `assets/research/README.md` listings.

Item table and chain shape are in the spec. Set `droppable` on the three rare
items: the two boss species at a high chance, a handful of mid-tier ordinary
species at a low one — that is what makes them drop from both bosses and nests.

The research node's `unlocks_structures` names both new structures. The three
craftable buffs are item-declared `craftable` with
`requires_structure: refactor_bench`; gating is transitive through the bench, so
they need no `unlocks_recipes` entry.

**Tests:** a census that every item declaring `upgrade` has finite percentages
and a non-empty description. Then confirm the five existing chain and economy
census tests still pass against the new assets (named in the spec) — those are
the real gate on the chain being shaped right.

## 3. Component and save

**Files:** `crates/engine/src/components.rs`, `tuning.rs`, `save.rs`,
`game/lifecycle.rs`, `lib.rs` (re-export).

`Refactors(pub u32)` near `FusionCount`, absence meaning zero the same way.
`MAX_COMPANION_REFACTORS = 5` in `tuning.rs`, doc-commented with the
unbounded-faucet reasoning — a Mining Node runs forever, so the cap is what
stands between a craftable buff and infinite stats.

`CreatureSave::refactors`, and `SAVE_FORMAT_VERSION` 26 → 27.

Mark the new field `#[serde(default)]`, following the existing fields that carry
it. It does nothing for the bincode save — that encoding is positional, and
several doc comments in `save.rs` say so explicitly — but it is exactly what
makes the dump-to-RON / pack-back migration in step 9 work, because RON is
field-named and an old dump simply won't have the key.

The write side (`lifecycle.rs:640-724`) is the friction point: that query is
already at bevy's 15-tuple maximum and uses a nested group for exactly this
reason. Read side is `lifecycle.rs:400-490`.

**Test:** a companion carrying a refactor count and a raised `ZonePortal`
round-trips through save and load unchanged.

## 4. The action

**Files:** new `crates/engine/src/game/refactor.rs`, `game/mod.rs`.

`Game::refactor_companion(target: Entity, item: &ItemId) -> Result<(), String>`.
Check order and the apply rules are in the spec. Both arms go through one shared
apply function — two copies of the stat math is the drift this repo has been
bitten by four times.

**Tests, each written failing first:**
- a Recompile Kernel doubles the stat block and raises the tier by one
- a Recompile Kernel is refused when the companion is already at the player's zone
- `a_percent_buff_commutes_with_a_zone_bump` — buff-then-bump and bump-then-buff
  land on identical numbers. This is the property that makes percentages the
  right choice; if it fails, ordering has become exploitable
- a +5% ATK buff on a 3-ATK Drone still gains 1 (the floor)
- the sixth percentage buff is refused while a bump still is not
- refused mid-battle
- a refused refactor consumes no item — the "spend last" ordering

## 5. Fusion fix

**Files:** `crates/engine/src/game/party.rs` (`fuse_companions`, ~:679).

`ZonePortal(max(a, b))` and `Refactors(max(a, b))`, carrying the comment shape
`Rarity` already uses four lines above.

**Test:** `fusing_two_bumped_programs_keeps_the_higher_tier`. Verify it by
deleting the fix and watching it fail. A test that passes with its fix removed is
a failure mode this repo has already shipped twice.

## 6. Views

**Files:** `crates/engine/src/views.rs`, `game/inspection.rs`.

`PetInfo::refactors` — same shape as the `fusions` and `rarity` it sits beside.
`ProgramManifest` gains the refactor count and the zone tier; the tier is the
part that matters, because nothing on screen currently tells a player their
companion is behind.

## 7. app-core

**Files:** `crates/app-core/src/lib.rs` (two `Mode` variants),
`app/party.rs` (handlers), `app/input.rs` (dispatch), `app/group_menu.rs`.

`Mode::Refactor` (pick companion) → `Mode::RefactorItem` (pick upgrade) → apply.
Mirror the `Mode::RoutineTarget` install flow rather than inventing a shape.

The `PARTY_ROWS` entry is `surface_only: false` — this reaches no zone-map state
through `Position`, so it works underground. Its `available` predicate requires
both an owned pet and at least one upgrade item in cargo.

No `[R]` action on the inventory item screen. One route in.

**Test:** the menu row is hidden with no upgrade items in cargo and present with
one — the group-menu rule that a row survives only if its first screen has a row.

## 8. gui

**Files:** `crates/gui/src/render/`.

The two new screens, drawn through `Painter` only — no direct backend calls in
`render/`. Plus the new `PetInfo` and manifest rows.

## 9. Gates and release

```sh
cargo test -p feral-processes-engine refactor
cargo test -p feral-processes-engine balance_sim
cargo test --workspace
cargo clippy --workspace && cargo fmt
```

`balance_sim` will not move and cannot gate these magnitudes — see the spec's
last section. The arena is the instrument for that.

Then play it, because a green suite is not evidence of play:

```sh
cargo run --bin savetool -- template     # pick a mid-run template
cargo run -- --template <name>
```

Build the Annealing Node and Refactor Bench, craft a kernel, refactor a zone-1
companion in zone 3, confirm the third bump is refused, confirm the manifest
shows the tier.

Release at the merge, not on the branch: `CHANGELOG.md` section, workspace
version bump, annotated tag. Push and tag both need an explicit ask.

`SAVE_FORMAT_VERSION` moves, so existing saves stop loading — which is
`CHANGELOG.md`'s definition of breaking, and at `0.x` that takes the **minor**:
0.6.0 → 0.7.0. The section opens with the "**Breaking: existing saves will not
load**" warning and the dump/pack migration recipe, exactly as 0.6.0's does;
`refactors` is a new field with a `u32` default, so a RON round-trip fills it in
and a player's game survives the upgrade.
