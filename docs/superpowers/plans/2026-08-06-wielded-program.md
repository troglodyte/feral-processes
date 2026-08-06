# Wielded Program Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the player equip a tamed program as their weapon from a hidden key on the companion screen — a live-computed ATK/DEF bonus plus a chance for each attack to fire one of that program's installed routines.

**Architecture:** A `WieldedProgram(Option<Entity>)` resource, read through a single accessor that returns `None` for a despawned entity. The bonus is computed live in `effective_atk`/`effective_def` beside `party_stat_bonus`, never baked into `Stats`. The proc hangs off `party_member_attacks` for slot 0 only, with the wielded program as the ability actor.

**Spec:** `docs/superpowers/specs/2026-08-06-wielded-program-design.md` — read it first. It records *why* each of these choices is what it is, and two of them look like bugs to a reader who hasn't.

**Branch:** `wielded-program` (already exists, spec already committed).

## Global Constraints

- **Follow CLAUDE.md.** Comments explain *why*, never *what*. Named constants in `crates/engine/src/tuning.rs`, never inline magic numbers. `cargo fmt` and `cargo clippy --workspace` after every change, fixing warnings rather than silencing them.
- **TDD.** Failing test first, every task. `cargo test --workspace` is the final gate and only the final gate — iterate with `cargo test -p feral-processes-engine <name>`.
- **The renderer never touches the ECS `World`.** Everything gui needs arrives through `Game`'s public API via app-core views.
- **`crates/gui/src/paint.rs` is the only file that may name a graphics library.** Nothing in this plan should go near it.
- **No occult naming** in any player-facing string.
- **New tuning values:** `WIELDED_PROGRAM_STAT_DIVISOR: i32 = 10`, `WIELDED_ROUTINE_PROC_CHANCE: f64 = 0.25`.
- **Save format:** `SAVE_FORMAT_VERSION` goes `22` → `23` in Task 5, and not before.
- **Do not** add an explicit "clear the wielded program" call to `dissolve_tamed_program` or `fuse_companions`. The live-compute is what makes that unnecessary; see the spec's section 1.

---

### Task 1: The resource, the accessor, and the passive bonus

**Files:**
- Modify: `crates/engine/src/resources.rs` — add `WieldedProgram`
- Modify: `crates/engine/src/game/lifecycle.rs:48-124` (`Game::new`) and the resource block in `Game::load` (~line 195) — insert it as a default
- Modify: `crates/engine/src/tuning.rs` — `WIELDED_PROGRAM_STAT_DIVISOR`, in the same section as `PARTY_PASSIVE_STAT_DIVISOR` (~line 300)
- Modify: `crates/engine/src/game/combat_round.rs:793-861` — `wielded_program`, `wielded_stat_bonus`, and the two `effective_*` call sites
- Test: `crates/engine/src/tests/` — new file `wielded.rs`, registered in the tests module

**Interfaces produced:**

```rust
// resources.rs
#[derive(Resource, Default)]
pub struct WieldedProgram(pub Option<Entity>);

// game/combat_round.rs
pub(crate) fn wielded_program(&self) -> Option<Entity>;
pub(crate) fn wielded_stat_bonus(&self) -> (i32, i32);
```

`wielded_program` returns `Some(e)` only when `self.world.get::<Stats>(e).is_some()` — the repo's idiom for "this entity still exists" (`tests/trade.rs`). `wielded_stat_bonus` reads that program's current `Stats` and yields `((atk / WIELDED_PROGRAM_STAT_DIVISOR).max(1), (def / …).max(1))`, or `(0, 0)` when nothing is wielded.

Both are added at the same player-only point `party_stat_bonus` is: inside `effective_atk` **before** the `power_attack_multiplier` is applied (so a hungry player is weakened on the whole sum), and at the tail of `effective_def`.

- [ ] **Step 1: Write the failing tests** in `crates/engine/src/tests/wielded.rs`. Use `spawn_tamed` from `tests/support.rs`; set the resource directly since nothing can wield yet.
  - `wielding_a_program_raises_the_players_attack_and_defense` — measure `effective_atk`/`effective_def` before and after setting `WieldedProgram`, assert both moved by the program's stat over the divisor.
  - `the_wielded_bonus_floors_at_one_per_stat` — a program whose ATK is below the divisor still contributes 1.
  - `the_wielded_bonus_tracks_the_programs_current_stats` — raise the program's `Stats` after wielding and assert the bonus moved without re-wielding. This is the test that pins the bonus as *live* rather than captured.
  - `a_despawned_wielded_program_lends_nothing` — despawn the entity behind the resource, assert `wielded_program()` is `None` and the bonus is `(0, 0)`. This is the safety net standing in for the two destruction paths.
- [ ] **Step 2: Run and confirm they fail** — `cargo test -p feral-processes-engine wielded`, expect compile failure on the missing items.
- [ ] **Step 3: Implement.** The tuning constant's doc must state it is deliberately independent of `PARTY_PASSIVE_STAT_DIVISOR` and must not be re-expressed in terms of it — the party buff is a removal candidate and this must not move with it. Do **not** write "mirrors" in any doc comment here; CLAUDE.md forbids a comment that claims to hold two copies in sync.
- [ ] **Step 4: Run and confirm they pass**, then `cargo test -p feral-processes-engine` for the whole engine suite.
- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

---

### Task 2: Wielding and unwielding

**Files:**
- Modify: `crates/engine/src/game/party.rs:272-320` — `wield_program`, `unwield_program`, and the `add_companion` guard
- Test: `crates/engine/src/tests/wielded.rs`

**Interfaces consumed:** `wielded_program`, `WieldedProgram` (Task 1).

**Interfaces produced:**

```rust
pub fn wield_program(&mut self, entity: Entity) -> Result<(), String>;
pub fn unwield_program(&mut self) -> Result<(), String>;
```

**The ordering is the whole task** and follows the `use_symlink` rule in CLAUDE.md — every refusal resolves before any state moves, so a rejected wield can neither strand a program between roles nor destroy a weapon item:

1. Refuse if `is_game_over().is_some()` or `has_active_battle()` — same guard and same message as `Game::equip` (`game/crafting.rs:210`).
2. Refuse if `entity` is not a tamed program owned by the player.
3. Stand it down from `Party` if it is a member (`remove_companion`).
4. `unequip(EquipmentSlot::Weapon)` if an item is worn, so its stat delta comes off `Stats` and the item returns to inventory.
5. Set `WieldedProgram`.

`add_companion` enforces the other door: adding the wielded program to the party unwields it first, by the same ordering.

**Ticking:** wielding costs one turn, like `equip`/`unequip`. Step 4 calls `unequip`, which ticks on its own — so the wield path must tick only when it did *not* displace an item. One player action is one tick either way.

- [ ] **Step 1: Write the failing tests.**
  - `wielding_a_party_member_stands_it_down` — a party member, wielded, leaves `Party`.
  - `adding_a_wielded_program_to_the_party_unwields_it` — the other door.
  - `wielding_returns_the_worn_weapon_and_removes_its_bonus` — equip a weapon item, record `Stats::atk`, wield a program, assert the item is back in inventory and the item's delta has come off. (The program's own bonus is live and does not touch `Stats`, so the base stat should return to exactly its pre-equip value.)
  - `a_wield_refused_in_battle_changes_nothing` — with a battle active, assert `Err`, and that `Party`, the weapon slot and `WieldedProgram` are all untouched. This is the ordering test; it should fail loudly if any state moves before a refusal.
  - `wielding_costs_one_turn_whether_or_not_it_displaces_a_weapon` — compare `GameClock` across both paths.
  - `unwield_program_clears_the_bonus`.
  - `selling_the_wielded_program_ends_the_wield` and `fusing_away_the_wielded_program_ends_the_wield` — one per destruction path (`dissolve_tamed_program` and `fuse_companions`). Assert the bonus is gone **without** either path having been modified: that is the point, and these two tests are what stop someone "fixing" the missing clear later.
- [ ] **Step 2: Run and confirm they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and confirm they pass**, then the whole engine suite.
- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

---

### Task 3: Eligibility and the proc

**Files:**
- Modify: `crates/engine/src/game/combat.rs:685-694` — `wieldable_routines` beside `actor_abilities`
- Modify: `crates/engine/src/game/combat_round.rs:196-237` — `party_member_attacks`
- Modify: `crates/engine/src/tuning.rs` — `WIELDED_ROUTINE_PROC_CHANCE`
- Test: `crates/engine/src/tests/wielded.rs`

**Interfaces consumed:** `wielded_program` (Task 1).

**Interfaces produced:**

```rust
pub(crate) fn wieldable_routines(&self, entity: Entity) -> Vec<AbilityDef>;
```

`actor_abilities(entity)` filtered by `!effect.field_only()` and `!matches!(effect, AbilityEffect::Decompile)`. Reuse the existing `field_only()` predicate — do not respell its three variants; it is the one predicate `field_routines`, `battle_special_options`, `wild_routine_ready` and `use_ability`'s `unreachable!` arm already agree on.

**The proc, in `party_member_attacks`:**

The current tail is the hard part. It reads:

```rust
if !self.creature_alive(front) {
    return self.finish_group_member(group, player);
}
false
```

The proc must land *after* that check so a routine never resolves against a corpse, and must be skipped when the battle has ended. Restructure so `finish_group_member`'s `true` still short-circuits, then — for `slot == 0` only — roll the proc, then `reap_dead_members(player)` as `resolve_one_action`'s `Special` arm already does. Re-resolve the group through `retarget` inside the proc, since the strike may have just emptied it.

Roll `WIELDED_ROUTINE_PROC_CHANCE` from `GameRng` (a battle roll — the world-generation prohibition does not apply), then a uniform pick from `wieldable_routines`. Call `use_ability(&ability, program, &name, &recipients)` with **the program as actor**, so `ability_user_level`, `ability_affinity` and `effective_atk` all read it. No fatigue drain, no cooldown armed.

Targeting is synthesized, since a proc has no picker:

| `AbilityTarget`      | Synthesized `battle::SpecialTarget`         |
| -------------------- | ------------------------------------------- |
| `OneEnemyGroupFront` | `EnemyGroup { group }` — the attacked group |
| `WholeEnemyGroup`    | `EnemyGroup { group }`                      |
| `AllEnemies`         | not consulted by `ability_recipients`       |
| `OneAlly`            | `Ally { slot: 0 }` — the player             |
| `WholeParty`         | not consulted by `ability_recipients`       |

`is_hostile` is `get::<Hostile>().is_some()` and a tamed program has none, so `ability_recipients` takes the friendly branch without further work.

- [ ] **Step 1: Write the failing tests.**
  - `no_wieldable_routine_is_field_only_or_decompile` — a census over the real `assets/abilities/` set via `test_assets_dir()`, so a new ability file is covered without editing the test.
  - `a_proc_scales_off_the_wielded_programs_stats` — give the program and the player deliberately different ATK and level, force the proc, and assert the damage followed the program. Seed `GameRng` rather than looping.
  - `a_proc_lands_on_top_of_the_strike` — total damage in a procced round exceeds the strike alone.
  - `no_proc_fires_when_the_strike_ended_the_battle` — the ordering test.
  - `a_companion_attack_never_procs` — slot != 0 is untouched.
  - `nothing_procs_when_no_program_is_wielded`.
- [ ] **Step 2: Run and confirm they fail.**
- [ ] **Step 3: Implement.** Document the two orderings on `party_member_attacks` the way `Game::arrive` documents its own — the death check before the proc, and `reap_dead_members` after it.
- [ ] **Step 4: Run and confirm they pass.** Then `cargo test -p feral-processes-engine` — combat is the most-tested area in the repo and this touches its hot path, so a regression here shows up immediately.
- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

---

### Task 4: Views, activity, and the sale warning

**Files:**
- Modify: `crates/engine/src/views.rs:143-177` (`PetInfo`) and `:49-79` (`PlayerStatus`)
- Modify: `crates/engine/src/game/party.rs:227-242` — `PetInfo` construction
- Modify: `crates/engine/src/game/trade.rs:305-335` — `program_activity` and `sale_detachments`
- Modify: wherever `PlayerStatus` is built (`Game::player_status`)
- Test: `crates/engine/src/tests/wielded.rs`

**Interfaces consumed:** `wielded_program` (Task 1), `wield_program` (Task 2).

**Interfaces produced:**
- `PetInfo::wielded: bool`
- `PlayerStatus::wielded: Option<WieldedView>` — a new view struct in `views.rs` carrying the program's `name: String`, `level: u32`, and `bonus: (i32, i32)`. `weapon` is always `None` when this is `Some`, since the two are mutually exclusive, so no screen renders both.
- `program_activity` returns `"equipped as weapon"`, checked **ahead of** the `Party` membership check.
- `sale_detachments` pushes `"stops being your weapon"`.

- [ ] **Step 1: Write the failing tests.**
  - `program_activity_names_the_wielded_program` — and that it wins over "in party" (they are mutually exclusive today, but the order is stated so it stays that way).
  - `selling_your_weapon_warns_you_first` — `sale_detachments` carries the line.
  - `pet_info_flags_the_wielded_program` — exactly one row has it.
  - `player_status_shows_the_wielded_program_in_place_of_a_weapon` — `weapon` is `None`, `wielded` is `Some` with the right name and bonus.
- [ ] **Step 2: Run and confirm they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and confirm they pass**, then the whole engine suite.
- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

---

### Task 5: Save and load

**Files:**
- Modify: `crates/engine/src/save.rs:60-100` — `CreatureSave::wielded: bool`; `:307` — `SAVE_FORMAT_VERSION` 22 → 23
- Modify: `crates/engine/src/game/lifecycle.rs:637-659` — write it in `Game::save`, alongside `party_slot`
- Modify: `crates/engine/src/game/lifecycle.rs:360-450` — restore it in `Game::load`, alongside the `party_slots` collection, inserting `WieldedProgram` after the creature loop the way `Party` is inserted at line 450
- Test: `crates/engine/src/tests/wielded.rs`

**Interfaces consumed:** everything from Tasks 1–4.

Bincode has no field-level compatibility — `CreatureSave::custom_name`'s own doc records this — so the field is a shape change and the version bump is mandatory, not optional. `#[serde(default)]` does **not** help here; that rule in CLAUDE.md is about `.ron` asset schemas, not the bincode save.

At most one creature may have `wielded: true`. Restore defensively: take the first and ignore any others, the way `Party` is truncated to `MAX_PARTY_SIZE` at line 449 rather than trusting the file.

`WieldedProgram` is **not** zone-local. The program travels with you across a breach exactly as the party does, so unlike `BuybackLedger` and `StackMemory` it must *not* be wiped by name in `enter_next_zone`.

- [ ] **Step 1: Write the failing test** — `a_wielded_program_survives_a_save_and_load`: wield, save to a `tempfile` dir, load, assert the same program is wielded (match on species and level, since entity ids are not stable across the round trip) and that the bonus is back in `effective_atk`.
- [ ] **Step 2: Run and confirm it fails.**
- [ ] **Step 3: Implement,** including the `SAVE_FORMAT_VERSION` bump.
- [ ] **Step 4: Run and confirm it passes**, then the whole engine suite — several tests round-trip saves and will catch a half-wired field.
- [ ] **Step 5: Recapture the `dev-saves/` templates.** The bump invalidates them. `cargo run --bin savetool -- template` lists them; `dev-saves/README.md` says what each sets up. Confirm the launcher's three `dev_template` tests still pass.
- [ ] **Step 6: `cargo fmt && cargo clippy --workspace`, then commit.**

---

### Task 6: The hidden key and the `(WEP)` tag

**Files:**
- Modify: `crates/app-core/src/app/party.rs:11-40` — `handle_companion_key`
- Modify: `crates/gui/src/render/party.rs:6-63` — `draw_companion_menu`
- Test: `crates/app-core/src/tests/` — companion menu tests; `crates/gui/src/render/party.rs` — the help-text census

**Interfaces consumed:** `wield_program` / `unwield_program` (Task 2), `PetInfo::wielded` (Task 4).

`GameKey::Char('W')` toggles wielding on the highlighted row, handled **before** `selected_index` is consulted, the same way `<` and `>` are. Uppercase reaches app-core as a distinct key and is already used that way (`Char('S')` in `app/inventory.rs:21`, `Char('L')` in `app/playing.rs:51`), so it can never collide with `menu_shortcut`'s digits-then-lowercase scheme however large the roster grows. On `Err`, set `status_line` as the existing add/remove path does.

The companion row renders ` (WEP)` following the existing `fusion_tag` / `activity_tag` pattern. It does **not** get its own row colour: `draw_companion_menu` already resolves CRITICAL over `fusion_color`, and a third meaning on that axis makes all three unreadable.

**The easter egg:** extract `draw_companion_menu`'s two help lines into a module-level `const [&str; 2]` so the census below can read them, and comment that const with why the key is absent from it.

- [ ] **Step 1: Write the failing tests.**
  - app-core: `the_hidden_key_wields_the_highlighted_program`, and again to unwield.
  - app-core: `the_hidden_key_is_ignored_on_an_empty_roster` — no panic, no status line.
  - gui: `the_companion_screen_never_advertises_the_hidden_key` — assert neither help line contains `'W'` as the key nor the word "weapon". A later "helpful" edit then fails rather than quietly spoiling the egg.
- [ ] **Step 2: Run and confirm they fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run and confirm they pass** — `cargo test -p feral-processes-app-core` and `cargo test -p feral-processes-gui`.
- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit.**

---

### Task 7: Documentation and the full gate

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking to catch drift

**Do not touch** `README.md` or `docs/manual.md` — both are explicitly carved out of the documentation obligation.

No `assets/*/README.md` changes: this adds no field to any `.ron` schema.

CLAUDE.md earns a **Load-bearing seams** entry, because two things here read as bugs to someone who hasn't seen the spec:
- The wielded bonus is computed live and `wielded_program` filters despawned entities, which is *why* neither `dissolve_tamed_program` nor `fuse_companions` needed wiring — a later "fix" adding an explicit clear to both is the regression to head off.
- The proc's actor is the program, not the player, so level, affinity and ATK all come off what you are wielding.

- [ ] **Step 1: Write the CHANGELOG entry** — player-facing wording, and say nothing that gives the key away.
- [ ] **Step 2: Add the CLAUDE.md seam entry**, then `cp CLAUDE.md AGENTS.md`.
- [ ] **Step 3: Run the full gate** — `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt --check`. All three must be clean.
- [ ] **Step 4: Commit.**
- [ ] **Step 5: Play it.** A green suite is not evidence of play, and `balance_sim` models no abilities at all — it cannot see the proc rate, the routines that fire, or their magnitudes. `cargo run -- --template extraction` puts a full party in reach. What you are checking: whether `0.25` feels right, whether the `(WEP)` tag reads on the row, and whether wielding is so much better than fielding that nobody would field.

---

## Notes for the implementer

**Don't start from `Game::new(seed)`.** Testing this by hand otherwise starts with an hour of play. `cargo run -- --template extraction` gives you a mid-run world with programs to wield; `cargo run --bin savetool -- template` lists the rest.

**If the whole suite fails at once with `NotFound` on an asset path,** that is stale build artifacts from this repo's old `/home/trog/code/petmud` location, not a real breakage. Fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core` — not a full `cargo clean`, which throws away ~4 GB.

**Warm builds are ~1s for `cargo check --workspace` and ~3s for the engine suite.** There is no tooling problem to solve here; iterate tightly with `cargo test -p feral-processes-engine <name>`.
