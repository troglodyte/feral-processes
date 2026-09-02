# Companion equipment

**Date:** 2026-08-12
**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header.
**Save format:** v27 → v28 (breaking; version 0.7.7 → 0.8.0)

## Problem

The player equips gear; companions cannot. As fights get harder the player
outgrows the programs fighting beside them, so a tough encounter turns into a
keep-your-companions-alive problem rather than a fight. Meanwhile the player
holds three slots and hoards every other copy of gear they own, unusable.

## What is being built

Any program the player owns can wear the same three slots the player wears —
Weapon, Armor, Module — drawn from and returned to the same cargo. A copy is
interchangeable: what goes on the player can come off and go on a program.
Wild and hostile creatures stay bare.

## Settled decisions

Each of these was chosen deliberately; they are recorded so they are not
relitigated during implementation.

- **Any owned program may wear gear**, not just the party. No interaction with
  `add_companion`/`remove_companion`, and no rule needed for standing a member
  down.
- **Gear always returns to cargo when the wearer is destroyed** — sold,
  extracted, fused away, or killed in battle. Gear is the player's property;
  the program is only wearing it.
- **A `decompiler` bonus is worn and does nothing on a program.** Programs
  never attempt a capture, and `components::Decompiler` is player-only. Ten
  shipped items carry the stat. `apply_equipment_delta` already skips it on an
  entity without the component, so this is the existing behaviour made
  legible rather than new code: the companion picker carries one header line
  saying so.
- **The entry point is the roster screen** (`Mode::Companion`, the `B` key),
  not a target picker on the inventory's equip action. One place shows a
  program's three slots together.
- **Out of scope:** wild/hostile creatures, arena scenario `equip` for
  companions, and the manifest screen (which keeps showing the player's
  loadout only).

## The trap this design is shaped around

Three operations read a companion's `Stats` and would scale or bank a gear
bonus sitting in it:

- `refactor.rs::refactored` **multiplies** `atk`/`def` — `*= ZoneLevel::
  tier_step(tier)` on a zone bump, and `raised(x, percent)` on each track.
- `party.rs::fuse_companions`'s `fuse_stat` combines both parents' numbers
  into a new entity's.
- `trade.rs::program_payout` prices a program off `Stats::power()`.

In every case the later unequip subtracts only the *unscaled* bonus, welding
the difference permanently into the program's base stats with no record of
where it came from. This is `components::EquippedItem::fusion_tier`'s trap,
already documented in `CLAUDE.md`, reached by a new route.

The rule that follows: **no stats operation may run while a gear bonus is in
`Stats`.** Every one of the four sites below either strips first or lifts and
replaces.

## Design

### 1. One equip path, parameterized on the wearer

`Game::equip` and `Game::unequip` take the wearing entity as a parameter. The
player is not a special case — it is the entity app-core passes when the
inventory screen calls them.

The bodies barely change: `apply_equipment_delta` and
`slot_occupant_with_mods` already take an entity, and `count_copies` /
`take_copies` / `add_copies` already resolve the player themselves. That last
point is the feature: **gear comes from and returns to the player's cargo
whoever wears it**, which is what makes a copy interchangeable.

New refusal, checked before anything moves: the target must be the player, or
a `Tamed` whose owner is the player. Both existing refusals (game over,
active battle) stay.

The log line branches on which — `You equip X` against `Rustling equips X` —
via the existing `entity_label`.

`components::Equipment` is inserted on demand at the first equip rather than
at every spawn site. Absence already reads as an empty loadout everywhere
(`world.get::<Equipment>` returns `Option`), so `adopt_program`, decompile
capture, `fuse_companions` and `Game::load` need no new copy of anything —
which is the third-copy trap `Game::adopt_program`'s doc warns about, avoided
rather than paid.

`Equipment`'s doc comment currently says "Player-only" and stops being true.

### 2. Gear never sees a stats operation

One private helper, `Game::gear_bonus(entity) -> EquipmentStats`, summing each
worn slot's `scaled_for_level(..).fused_for_tier(..)`. It is the single
definition of "what is this entity's gear currently worth", and every site
below reads it rather than walking the slots itself.

A second helper, `Game::strip_gear(entity)`, unequips every worn slot into
cargo (`add_copies` at the copy's own tier, `apply_equipment_delta` with
`-1`), returning the log lines. Idempotent on an entity with nothing worn.

The four sites:

| Site | What it does | Why |
|---|---|---|
| `refactor_companion` | subtract `gear_bonus`, run `refactored()`, add it back | The program survives, so its gear stays on. The recorded `EquippedItem` is untouched, so the add-back is exact. Invisible to the player. |
| `dissolve_tamed_program` | `strip_gear` at the top | One call covers sale, extraction **and** death — the three paths this function already unifies. |
| `fuse_companions` | `strip_gear` on both parents **before** the `Stats` snapshot | This function does its own `Party::retain`/`despawn` and never calls `dissolve_tamed_program` (`CLAUDE.md`: "Destroying a tamed program has two paths"). The ordering is the whole correctness argument. |
| `sell_companion` | `strip_gear` after every refusal, before `program_payout` | Appraisal must price the program, not the gear the player is about to get back. Placed after the refusals so a refused sale leaves the loadout alone. |

### 3. UI — the roster screen

`E` on `Mode::Companion` opens a new `Mode::CompanionEquip` for the
highlighted program. Uppercase, handled before `selected_index` the way `N`
and `W` are, so it can never collide with `menu_shortcut`'s
digits-then-lowercase scheme however large the roster grows.

`Mode::CompanionEquip` lists that program's three slots — the same three rows
the inventory screen leads with, through the same `stat_summary` formatter.
Its header names the program and carries the decompiler line. Selecting a slot
opens the **existing** `Mode::EquipSwap` picker.

`App` gains `pending_swap_target: Option<Entity>` beside `pending_swap_slot`.
`equip_swap_rows(game, target, slot)` takes the wearer, so the worn copy is
measured at *its* recorded level against candidates at the current zone's —
the asymmetry `CLAUDE.md` records for the player's picker, now asked of the
right wearer. Esc from `EquipSwap` returns to `CompanionEquip` when a target
is set and to `Inventory` when it is not.

`render/party.rs`'s `companion_help` gains `[E]quip`. It must still never name
`W` — a gui test holds it to that, and that omission is a shipped easter egg.

A program with nothing in cargo that fits a slot gets the same
"Nothing in cargo fits…" status line the player's picker gives, rather than an
empty screen.

### 4. Save format

`CreatureSave` gains one field:

```rust
#[serde(default)]
pub equipment: Vec<(EquipmentSlot, EquippedItem)>,
```

Both types already derive `Serialize`/`Deserialize`. A `Vec` rather than
`PlayerSave`'s nine flat fields, for one reason that decides it: a single
defaulted field means an **older RON dump packs with no hand-editing at all**.
`PlayerSave` keeps its existing shape — changing it would break that.

`#[serde(default)]` does nothing for the bincode encoding, which is positional
— that is why this bumps `SAVE_FORMAT_VERSION` to 28 at all. It is there for
the field-named RON that `savetool dump`/`pack` round-trips through, which is
the migration path.

### 5. Save migration

The RON dump of the newest non-dev save (`saves/save_1786492847.bin`, written
2026-08-12 06:54) was taken **before** any format change, while the build still
reads v27, and is held in the session scratchpad as
`save_1786492847.v27.ron`. After the engine change it is packed back with the
new build:

```sh
cargo run --bin savetool -- pack <scratchpad>/save_1786492847.v27.ron \
    saves/save_1786492847.bin
```

The `equipment` key is absent from that RON and defaults to empty, which is
correct: no program in that save has ever worn anything.

The three `dev-saves/*.ron` templates are stored as field-named RON, so they
keep loading untouched for exactly the same reason. `saves/dev_*.bin` are
regenerable with `savetool template` and are not migrated.

## Testing

Engine, `crates/engine/src/tests/equipment.rs` unless noted:

- A companion's ATK rises by the worn weapon's scaled bonus, and drops back on
  unequip.
- Gear equipped on a companion leaves the player's own `Stats` alone, and vice
  versa — the two wearers are independent.
- A copy taken off the player goes on a companion (interchangeability, one
  test walking both directions through cargo).
- Equipping onto a wild creature or a structure is refused — the guard is
  "the player, or a `Tamed` they own", and a wild program is the reachable
  case a battle screen could hand over.
- A module whose only stat is `decompiler` equips onto a program and changes
  none of its stats — pins the "worn, bonus is dead" decision rather than
  leaving it to be discovered.
- **Refactoring a geared program scales its base stats only.** The mutation
  check: with the lift-and-replace removed, the assertion must fail. Then
  unequip and assert the program is back to exactly its pre-gear numbers —
  which is the half that catches the welded bonus.
- Selling a geared program returns the gear to cargo **and** pays the same
  price as selling the identical program bare (`tests/trade.rs`).
- Fusing away a geared program returns its gear, and the fused child's stats
  match those of a fusion of the same two programs bare (`tests/party.rs`).
  Mutation check: move the strip after the `Stats` snapshot and it must fail.
- A companion killed in battle returns its gear to cargo (`tests/combat*`).
- Round trip: a geared companion survives save and load with its slots, levels
  and fusion tiers intact (`tests/save.rs`), and a v27-shaped RON without the
  `equipment` key still parses.

app-core, `crates/app-core/src/tests/party.rs`:

- `E` on the roster opens `CompanionEquip` for the highlighted program, and
  Esc backs out to the roster.
- Picking a slot opens `EquipSwap` with the target set; Esc from there returns
  to `CompanionEquip`, not to `Inventory`.
- The picker's rows are the companion's, measured against the companion's worn
  copy — a program and the player wearing different copies of the same item
  produce different rows.
- A slot with nothing in cargo to fit it produces the status line, not an
  empty screen.

gui: the existing test holding `companion_help` to never naming `W` must still
pass with `[E]quip` added.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
`balance_sim` is unaffected — it models no equipment and no companions'
gear — and is not a gate for the magnitudes here.

## Documentation obligations

- `CLAUDE.md` — the "Destroying a tamed program has two paths" seam now has a
  third thing both paths must do, and the stats-operation rule above is a new
  load-bearing seam. Copy to `AGENTS.md` after editing (gitignored twins).
- `CHANGELOG.md` — a `## 0.8.0` section; save-format bump takes the minor.
- Root `Cargo.toml` — version 0.8.0, at the merge, not on the branch.
- `assets/items/README.md` — no schema change, but it states who can wear
  gear; check the claim.
- `docs/manual.md` and the root `README.md` are carved out and stay stale.
