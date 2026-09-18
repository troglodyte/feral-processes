# Player Emulation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In a battle, the player can emulate a species whose image they have learned, fighting with that species' kit for a fixed number of rounds (todo #100).

**Architecture:** One derivation, `Game::kit_of`, answers where a body's kit comes from; every reader of a kit figure becomes an exhaustive match on `Kit`. Emulation is a never-saved `Emulation` component read by `kit_of`; images are a saved `EmulationImages` resource written by a new extraction tool category.

**Tech Stack:** Rust, standalone `bevy_ecs` (engine), app-core state machine, Bevy + egui renderer.

**Spec:** `docs/superpowers/specs/2026-09-16-player-emulation-design.md`. Read it before any task; this plan records only where the code differs from it and the decisions it left open.

## Global Constraints

- Work in the worktree `.claude/worktrees/player-emulation`, branch `feat/player-emulation`. **Never push, never merge, never tag** — landing is the controller's job.
- Part A (Task 1) lands alone as its own release before Part B starts. Part B is one further release.
- TDD: failing test first, watched failing, then the code. Commit per green step. Stage explicit paths, never `git add -A`.
- Gates at every task end: `cargo fmt`, `cargo clippy --workspace --all-targets` clean, targeted `cargo test -p <crate> <name>`. Full `cargo test --workspace` at the end of Task 1 and at the end of Part B only.
- Cargo's exit code is lost through a pipe: don't judge pass/fail from `| tail`.
- Vocabulary: *emulate / emulation / image*. Never "transform", "shapeshift", "form" in player-facing text (`FormLook` is an internal name only). No occult words.
- `Perk` variant order is save format: **append** `EmulationFidelity`.
- New save field behind `#[serde(default)]`, no `SAVE_FORMAT_VERSION` bump, and a save→load test (RON round-trip alone cannot catch a skipped field).
- Tuning constants in `crates/engine/src/tuning.rs`, documented `pub const`.
- Any new `Mode` goes into `ALL_MODES` (`crates/gui/src/render/mod.rs:1470`) by hand, and its draw arm must exist — the list does not fail to compile.
- Screen actions are UPPERCASE keys; lowercase letters select rows.
- Help text never mentions the `W` wield key.
- Don't edit `docs/manual.md`, root `README.md`, or the repo's `TODO.md`.

## Where the code differs from the spec (decisions)

1. **`Kit` has three variants, not two.** `Kit::Unarmed`, `Kit::Innate(&SpeciesDef)`, `Kit::Emulated { def: &SpeciesDef, stats: EmulatedStats }`. The spec's `Species { emulated: Option<_> }` hides the one distinction every reader in Part B branches on inside an `Option`; three variants keep "a new source fails to compile" true for it. `kit_of` is `pub(crate) fn kit_of(&self, entity: Entity) -> Kit<'_>`: `Emulation` first, then `Creature`→`SpeciesDb`, else `Unarmed`. In Task 1 `Emulated` does not exist yet; Task 3 adds it and every reader then fails to compile until it answers.
2. **`actor_abilities` has no player branch today** — it reads the `Routines` component. It becomes a `Kit` match in Task 3: `Emulated` → the species list, `Unarmed | Innate` → `Routines` as now. It is not touched in Task 1.
3. **`natural_range_of` and `swing_range` have no explicit player branch**; the player falls through a missing `Creature`. Task 1 makes that fall-through an explicit `Kit::Unarmed` arm.
4. **Gear is already baked into `Stats`** (`apply_equipment_delta`), and `effective_atk`/`effective_mitigation` start from `Stats`. For `Kit::Emulated` the base is `stats.atk + self.gear_bonus(entity).atk` (and the mitigation analogue) — `gear_bonus` (`crafting.rs:848`) is called, not recomputed. Everything after the base is unchanged, including the player-only wielded/Power terms.
5. **`routine_slots` stays a player/companion branch** — it is about who the body *is*, not its kit. The emulated routine list is capped by it.
6. **Extraction: an `Image` tool follows the `Routines` category, not `extraction_yield`.** `extraction_yield` returns `Vec<(ItemId, u32)>` and has no room for "teaches an image". Instead: `ToolCategory::Image`; `extract_program`'s category branch gains an `Image` arm calling a new `extract_image_from_program`; `ExtractionPreview` (`views.rs:3102`) gains `Image(SpeciesId)` and reuses `NothingToLearn` for an image already held. The preview and the grant read one helper, `Game::image_yield(&DownedProgram) -> Option<SpeciesId>` (`None` = already known), so they cannot differ.
7. **The battle map has no engine-built action list** — app-core's `handle_tactical_key` hardcodes keys. Revert there is `Game::tactical_revert(&mut self) -> bool` plus an uppercase key and a drawn hint. The group model gets `BattleAction::Revert` and `ActionKind::Revert` in `battle_action_options`. Both call one engine core, `Game::drop_emulation(&mut self, entity, reason)`.
8. **Invoking carries the image on the action.** Group: `BattleAction::Special` gains `image: Option<SpeciesId>` (every construction site gets `image: None`). Battle map: `tactical_use_routine` gains nothing; a sibling `Game::tactical_emulate(&mut self, index: usize, species: &SpeciesId) -> bool` resolves the Emulate routine at `index`, since it has no aim. Both go through `use_ability`'s `AbilityEffect::Emulate` arm, which reads the image from a field the caller set — the implementer picks the smallest threading that keeps `use_ability` the one door (a pending-image field on `Game` set and cleared around the call is acceptable if a parameter would touch every caller).
9. **The research shortcut.** `exclusive: true` is not usable (an exclusive routine never enters `KnownRoutines`). `creation_shelf` has no filter for ability disks. Task 4 first checks whether installing an etched disk is gated by `Game::node_researched`; if it is, nothing changes; if not, `creation_shelf` and `stock_pool` exclude the `emulate` disk and a test asserts both.
10. **`ConRead::of(difficulty, is_boss, drew_sprite)` already takes the boss flag** — pass `true` for an emulated tile; no signature change.

---

# Part A — the seam, no behaviour change (release on its own)

### Task 1: `Kit` and `kit_of`, readers converted

**Files:**
- Create: `crates/engine/src/game/kit.rs` (the enum, `kit_of`, and its doc comment carrying the seam)
- Modify: `crates/engine/src/game/mod.rs` (module), `game/combat_round.rs:570` (`swing_move_at`), `game/combat_damage.rs:332` (`natural_range_of`), `game/combat.rs:93` (`swing_range`), `game/combat.rs:1198` (`ability_affinity`)
- Test: `crates/engine/src/tests/kit.rs` (register in `tests/mod.rs`)

**Interfaces:**
- Produces: `pub(crate) enum Kit<'a> { Unarmed, Innate(&'a SpeciesDef) }`; `pub(crate) fn kit_of(&self, entity: Entity) -> Kit<'_>`.

Each reader becomes an exhaustive `match self.kit_of(entity)`. `ability_affinity`'s class-affinity arm is `Kit::Unarmed` **and** the body is the player; `swing_move_at`'s unarmed strike is `Kit::Unarmed`. No `_ =>` arms. `swing_move_at` takes `&mut self`, so take what it needs from `kit_of` (e.g. the species id) before the mutable roll — don't clone the def to fight the borrow checker.

- [x] **Step 1: Failing tests.** In `tests/kit.rs`: `kit_of` answers `Unarmed` for the player and `Innate` (with the right species id) for a companion and for a wild program. Also pin the four former player paths so the refactor has a witness: a non-emulating player's `swing_move_at` yields `PLAYER_UNARMED_DAMAGE`'s band; `natural_range_of(player)` is `PLAYER_UNARMED_DAMAGE`; `swing_range(player)` is `TACTICAL_MELEE_RANGE` (see `an_unarmed_body_swings_at_arms_length`, `tests/tactical.rs:2444`); `ability_affinity(player, …)` equals the class affinity for the player's class.
- [x] **Step 2:** `cargo test -p feral-processes-engine kit` — fails to compile (no `kit_of`).
- [x] **Step 3:** Write `kit.rs`, convert the four readers.
- [x] **Step 4:** Targeted tests pass; then `cargo test --workspace` green (this is the regression gate the spec asks for).
- [x] **Step 5: Seam, three writes** (the `seams` skill documents the order): argument to the memory graph as `seam:kit-of`; the trap (a reader branching on `player_entity()` or `Creature` directly for a kit figure) to the combat reference under `.claude/skills/seams/`; one sentence in CLAUDE.md's *Combat, progression and balance* section: "**`Game::kit_of` is the one answer to where a body's kit comes from**, and every reader of a kit figure is an exhaustive match on `Kit`."
- [x] **Step 6:** Commit: `Engine: Game::kit_of is the one answer to where a body's kit comes from (todo #100)`.

**Part A ends here.** Report to the controller; it runs the opus review of Part A and lands it as its own release before Part B starts.

---

# Part B — the feature (one release)

### Task 2: Strength — `emulated_stats`, tuning, the perk

**Files:**
- Modify: `crates/engine/src/progression.rs` (beside `stats_after_levels`, line 159), `tuning.rs`, `perks.rs` (append variant, query `emulation_fidelity_level`), `assets/perks/groups.ron`
- Create: `assets/perks/emulation_fidelity.ron` (format: `assets/perks/keen_scavenger.ron`)
- Test: `crates/engine/src/tests/kit.rs`

**Interfaces:**
- Produces: `pub struct EmulatedStats { pub atk: i32, pub mitigation: i32 }` (`Clone, Copy, Debug, PartialEq`); `pub fn emulated_stats(def: &SpeciesDef, player_level: u32, fidelity_level: u32) -> EmulatedStats`; `pub fn emulation_fidelity_level(perks: Option<&Perks>) -> u32`; `tuning::EMULATION_EDGE: f32`, `EMULATION_EDGE_PER_PERK_LEVEL: f32`, `EMULATION_ROUNDS: u32` (the default the ability file authors; the file is the source of truth).

Formula is spec §2. Start `EMULATION_EDGE = 1.25`, `EMULATION_EDGE_PER_PERK_LEVEL = 0.1` — guesses, retuned in Task 7. Mitigation is not level-scaled; it takes the multiplier, rounded; the cap stays `effective_mitigation`'s.

- [ ] Failing tests: an emulation's atk exceeds `stats_after_levels` of the same species at the same level; each fidelity level raises atk; mitigation is unscaled by level and takes the multiplier; `every_perk_has_a_query_that_answers_what_it_is_worth` compiles and passes with the new variant.
- [ ] Watch fail, implement, pass. Perk query doc follows the file's pattern (takes `Option<&Perks>`, names its own variant).
- [ ] Update `assets/perks/README.md` if it lists the catalogue. Commit.

### Task 3: The emulation state, and every reader answering `Kit::Emulated`

**Files:**
- Modify: `components.rs` (`Emulation`), `game/kit.rs` (third variant, reads `Emulation`, calls `emulated_stats` with the player's level and `emulation_fidelity_level`), the four Task 1 readers, `actor_abilities` (`combat.rs:1257`), `effective_atk`/`effective_mitigation` (`combat_round.rs:1766`, `:1826`), `combat_status.rs:520` (`tick_one_combatant` ages it after `tick_combat_buff`), `combat_teardown.rs:164` (`clear_battle_status_effects` removes it, no-op-safe like `uncloak`), `combat.rs:1013` (extract the species filter out of `install_innate_routines`)
- Test: `tests/kit.rs`

**Interfaces:**
- Consumes: Task 2's `emulated_stats`, `EmulatedStats`, `emulation_fidelity_level`.
- Produces: `#[derive(Component)] pub struct Emulation { pub species: SpeciesId, pub rounds_left: u32 }` (never saved); `Kit::Emulated { def, stats }`; `fn innate_routine_ids(def: &SpeciesDef, level: u32, abilities: &AbilityDb) -> Vec<AbilityId>` (the filter both `install_innate_routines` and `actor_abilities` call); `pub(crate) fn drop_emulation(&mut self, entity: Entity, line: &str)` — removes the component and logs the line.

Readers: `Emulated` swings the species' `basic_attacks()`, reaches the species' natural range (a worn weapon still overrides via the existing weapon-first check), uses the species' affinities **without** class affinity and **without** `talent_affinity_mult`, and lists `innate_routine_ids` at the player's level truncated to `routine_slots(entity)`. Lapse logs *"Your emulation lapses."*

- [ ] Failing tests (spec §7): `kit_of` answers `Emulated` for an emulating player; outhits a same-level wild program of the species; worn gear still adds atk and mitigation; mitigation cap holds; HP unchanged by emulating, lapsing and teardown; `actor_abilities` returns the species list filtered and capped while emulating and the player's own after; lapses after `rounds` rounds in the group model and on a battle map (`tactical_fight`, `tests/tactical.rs:135`); `finish_fight` removes it after a win, a loss and a jack-out. Insert `Emulation` directly in these tests — invoking comes in Task 4.
- [ ] Watch fail, implement, pass. **Mutation check:** delete the `Emulation` read in `kit_of`, confirm strength and kit tests fail, restore.
- [ ] Commit.

### Task 4: Invoking and changing back

**Files:**
- Create: `assets/abilities/emulate.ron`
- Modify: `abilities.rs` (`AbilityEffect::Emulate { rounds: u32 }`; `tactical_only`/`field_only` answer false; census matches), `battle.rs` (`BattleAction::Special.image`, `BattleAction::Revert`, `ActionKind::Revert`), `combat.rs` (`battle_action_options` offers Revert only while `Emulation` is present; `ability_unavailable` refuses Emulate with no images and while emulating), `combat_round.rs` (`use_ability` arm inserts `Emulation`, logs *"You emulate a Scrapper."*), `tactical/turn.rs` (`tactical_emulate`, `tactical_revert`, both ending via `hand_on_turn`), a new `Game::emulation_options() -> Vec<EmulationOption>` in `views.rs`/`game/kit.rs`, `assets/abilities/README.md`
- Test: `tests/kit.rs`, plus a `tests/tactical.rs` case

**Interfaces:**
- Consumes: `drop_emulation`, `emulated_stats`, `EmulationImages` (**create the resource in this task** as `resources::EmulationImages(pub BTreeSet<SpeciesId>)`, empty by default; Task 5 adds its save field and writer).
- Produces: `pub struct EmulationOption { pub species: SpeciesId, pub name: String, pub glyph: char, pub atk: i32, pub mitigation: i32 }`; `pub fn emulation_options(&self) -> Vec<EmulationOption>` sorted by name; `pub fn tactical_emulate(&mut self, index: usize, species: &SpeciesId) -> bool`; `pub fn tactical_revert(&mut self) -> bool`. Revert logs *"You drop the emulation."*, costs the turn and no Power.

Research node: `routine/emulate` is synthesised by `routine_tree::synthesise_nodes` — no research file. Verify `routine_tree::family` gives "Emulate" a single root rung, and set `research_zone` in `emulate.ron` (start at 2; Task 7 may move it). Then decision 9: find whether installing an etched disk requires `node_researched`; if not, exclude the `emulate` disk from `ItemDb::creation_shelf` (`items_db.rs:702`) and `Game::stock_pool` (`caravan.rs:481`) through one predicate, with a test on each.

- [ ] Failing tests: `emulation_options` rows carry `emulated_stats` figures (a call, compared against it); Emulate refused with no images and while emulating; invoking in the group model inserts the component; Revert is offered only while emulating, costs the turn and no Power, in both models; the research node exists and is visible at its zone; the shelf check per decision 9.
- [ ] Watch fail, implement, pass. Commit.

### Task 5: Learning an image

**Files:**
- Modify: `tools.rs:52` (`ToolCategory::Image`), `extraction.rs` (`image_yield`, `extract_image_from_program`, the `Image` arm in `extract_program` at `:630`), `views.rs:3102` (`ExtractionPreview::Image(SpeciesId)`, built from `image_yield`), `resources.rs`, `save.rs` (`SaveData::emulation_images: Vec<SpeciesId>`, `#[serde(default)]`, sorted on write — follow `known_tools` at `save.rs:1301`), `notifications.rs` (`NotificationKind::LearnedImage`, latch key `"milestone_learned_image"`, `def` and both censuses), `assets/tools/README.md`
- Create: `assets/tools/image_capture.ron` (name the tool in-fiction; follow a shipped tool file)
- Test: `tests/extraction.rs` (or wherever extraction tests live), `tests/save.rs`

- [ ] Failing tests: an `Image` extraction inserts the species and consumes the program; a duplicate is refused **before anything is spent** (program still in `DownedPrograms`, tool untouched); preview equals grant (`ExtractionPreview::Image(s)` and the learned `s` come from the same `image_yield`); no `GameRng` draw (compare the RNG state before and after); save→load keeps `emulation_images`; the notification fires once.
- [ ] Watch fail, implement, pass. Update gui's `render/extraction.rs` match for the new preview arm (compile will point at it). Commit.

### Task 6: Screens

**Files:**
- Modify: app-core `lib.rs` (`Mode::BattleEmulate` beside `Mode::BattleSpecial` at `:1408`, and a battle-map equivalent or one mode both models share), `app/battle.rs` and `app/tactical.rs` (picker, Revert key, cancel returns to the routine list spending nothing); `views.rs:986` `EntityView::form: Option<FormLook>` and `tactical/view.rs:39` `TacticalBody::form`, filled by the engine; gui `render/base.rs` (~`:920-975`) and `render/tactical.rs` (~`:604`) draw the form's sprite or glyph in boss magenta with `ConRead::of(.., true, ..)`, a player-colour outline (`player_look_color`), and neither `@` nor `"@drawn"`; the picker's draw arm; `ALL_MODES`; `assets/help/` page for emulation, linked from the extraction page
- Test: app-core picker tests; gui headless paint tests (`paint::with_painter` measures real text)

**Interfaces:**
- Produces: `pub struct FormLook { pub glyph: char, pub sprite: Option<String> }`.

- [ ] Failing tests: picker lists `emulation_options`, a lowercase letter selects, Esc returns to the routine list with nothing spent; the Revert key is uppercase and only offered while emulating; gui paint test asserts the form sprite **and** the absence of both `@` and `@drawn` **and** the outline **and** the con read in the earmark (the overdraw trap); a census that the widest shipped row and a full image library fit the picker popup (it does not scroll); the help page clears the manual's censuses.
- [ ] Watch fail, implement, pass. Commit.

### Task 7: Instruments and tuning

**Files:**
- Create: `dev-arenas/emulation.ron` (read `dev-arenas/README.md`; `equip` is top-level, not inside `Fresh(...)`), `docs/measurements/2026-09-18-emulation-edge.md` (follow that directory's README), a `dev-saves` template with a few images learned and Emulate installed (`savetool capture` after an engine-side test drives the state — see `dev-saves/README.md`)
- Modify: `tuning.rs` constants, `emulate.ron` `research_zone` if the numbers say so

- [ ] Run the arena: the player emulating vs. the player's own kit at the same level against a same-zone pack, several species including the strongest. Pick `EMULATION_EDGE` so an emulation is worth taking but the best one doesn't outclass the player's own geared kit. Arena numbers compare within one build only.
- [ ] Record commands, numbers and blind spots in the measurement file. Commit.
- [ ] `cargo test --workspace` and `cargo clippy --workspace --all-targets` green. Then the docs: `CHANGELOG.md` is written at the merge, not here; update `docs/superpowers/INDEX.md`'s row for this spec to built.

**Part B ends here.** Report to the controller for the final opus whole-branch review.
