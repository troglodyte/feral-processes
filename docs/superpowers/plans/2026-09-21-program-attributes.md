# Program Attributes and the Dossier Page — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A second, non-combat stat block — five attributes authored in `assets/attributes/` — carried by the player and by every creature, minted from a deterministic fold rather than an RNG draw, stored in the save, and read on a new no-scroll page reached with `[D]` from the manifest.

**Architecture:** Four seams, each with an exact precedent in the tree. **The catalogue** is `crates/engine/src/attributes.rs`, `needs::NeedDb` copied line for line — an absent directory loads empty and silent, a malformed file costs one attribute. **The mint** is a pure function over `derive::fold`/`derive::index`, called at the four sites that build a creature's component list, and it spends no `resources::GameRng` draw. **The store** is `components::Attributes`, a `BTreeMap<AttributeId, i32>`, additive in the save behind `#[serde(default)]`. **The page** is `Mode::Dossier` drawing one engine derivation, `draw_companion_memories` copied shape for shape — a `draw_popup` with no `Row::Item`, so no scroll and two censuses instead.

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets, `serde`.

**Spec:** `docs/superpowers/specs/2026-09-21-program-attributes-design.md` — read it before Task 1, together with the corrections below, which supersede it where they conflict. Its §2 "Decisions taken" and §11 "Tests" are the contract; its §10 file table is wrong in three rows and its §5/§9 are wrong about two mechanisms.

---

## Eight corrections to the spec

Each was found by reading the code the spec describes. Six change a file, a name or a mechanism; two are design calls the spec left open or got backwards. None reverses a decision in §2.

### 1. `Coherence` is already a shipped need, so attribute 1 is renamed

`assets/needs/coherence.ron` ships today with `name: "Coherence"` and the blurb *"A process that never yields comes apart at the edges."* The spec's first attribute is `Coherence (Willpower)`, glossed "keeps its own state when things go wrong around it". Those are the same words for two different things, and the player meets both: the need on the manifest's WORK box, the attribute on the dossier. A number that shares a name with a bar somewhere else on the same program reads as that bar.

**The attribute ships as `Persistence (Willpower)`** — id `"persistence"`. It is the setting's own word for state that survives, it collides with nothing (`rg persistence crates/` is empty), and it keeps the legacy name the spec chose. Everything else about attribute 1 is unchanged.

This is the one player-facing content decision this plan makes on its own. It is cheap to veto: one `.ron` file, one row in one census, one line in the README.

### 2. `Profile` is the worst available name; the component is `Attributes`

"Profile" already means three things in this crate: the cross-run achievement record (`profile.ron`, `install_profile`, `grant_profile_rewards`, `tuning::MAX_PROFILE_*`), a combat roll's stat snapshot (`Game::combatant_profile`, `defender_profile_against`), and `components::PlayerIdentity`'s neighbours. A fourth meaning on a save field named `profile` is a trap for every future reader.

`Attributes` is entirely unused as an identifier in `crates/` (two prose hits in test comments, no type, no field, no module). So:

| Spec | This plan |
|---|---|
| `components::Profile` | `components::Attributes` |
| `CreatureSave::profile` | `CreatureSave::attributes` |
| `PlayerSave::profile` | `PlayerSave::attributes` |
| `Game::dossier_report` | unchanged |

### 3. The mint does **not** go in `roster_parts`, and this makes the feature better

§5 puts one of the three fold inputs "through `Game::roster_parts()`: the `ProgramId` just minted." That is the wrong door, and it would be a bug rather than a style point.

Every roster door except fusion routes through the wild spawn first and then *adds* the roster components: `grant_starting_program` (`lifecycle.rs:2841` — `spawn_wild_creature_scaled`, then `remove::<(Hostile, WanderAi)>`, then `insert(parts)`), `adopt_program` the same shape, and a capture keeps the very body that was fought. So a body arriving at `roster_parts` **already has** its attributes from the wild spawn, and minting there a second time would overwrite them — a program's attributes would change at the instant you tamed it, which is the opposite of "the player can read who a program is".

`fuse_companions` (`party.rs:1382`) is the one door that spawns fresh and then `insert`s `roster_parts()`'s tuple, so it is the one that needs its own mint.

**The mint sites are the four places a body's component list is written:**

1. `Game::spawn_wild_creature_scaled` — every wild body, every nest guardian, every patrol, the arena's rosters (`arena/setup.rs:194`), every summoned fork, and by way of the strip-and-insert shape, every adopted or starting program.
2. `Game::fuse_companions`' own spawn — a new program with no place, minted off the `ProgramId` `roster_parts` just handed it.
3. The player — see correction 4.
4. `Game::spawn_creature_from_save` — see correction 6.

`Game::roster_parts` is **not** touched. Its doc comment's four-doors argument stays true and stays about the components it does mint.

### 4. The player is not spawned in `creation.rs`, and its class is not known where it is

§10 lists `crates/engine/src/game/creation.rs` for "mint for the player". The player entity is spawned by `lifecycle.rs::spawn_player` (line 120), a free function over `&mut World` with no `Game`, no `ClassDb` and no class — `CharacterChoice` has not been applied yet, and `PlayerIdentity::default()` is seeded there precisely because the class arrives later. Its bundle tuple is also already at bevy's 15-element ceiling and carries a nested tuple to get under it.

So the player's attributes are minted in `creation.rs`, but **in `apply_creation_identity`** (line 257), which is the first point `choice.class` is known, and it inserts the component rather than joining a tuple. `CharacterChoice::default()` carries `class: None`, which is a supported run — it falls through to each def's own `base`, exactly as an unauthored species does.

### 5. Mint at load, not "on first read"

§9 says an existing save "loads with empty profiles and mints them on first read". §11's test 4 says "An old save mints on load". These contradict, and read-time minting would force `Game::dossier_report` to take `&mut self`, which no sibling derivation does (`Game::manifest` is `&self`, `Game::memory_report` is `&self`).

**Minting happens on load**, inside `spawn_creature_from_save` and the player's restore, when the saved map is empty. `dossier_report` stays `&self` and the screen stays a pure read.

### 6. `spawn_creature_from_save` is a fourth mint site and its doc comment is the trap

`spawn_creature_from_save` (`lifecycle.rs:1641`) carries a doc comment that is the exact warning this task needs: it mints the same component set `roster_parts` does and *pointedly does not call it*, so "a new component that belongs to every owned program has to be added in both places, or a program will have it from the moment it is tamed and quietly lose it on the next reload." Attributes belong to every creature, not only owned ones, so they go here for both.

### 7. The Entropy gloss must not say it rises

§1's table describes Entropy as "accumulated noise — the one that rises over a run". Nothing in this change raises it: a value is minted once and stored. §2's decision 2 forbids exactly this — prose that promises a mechanic the player will test and find false. **Entropy's `short` and `meaning` describe accumulated noise as a property this program already has**, and say nothing about rising. The hook §2's decision 3 reserves for it (fumble rate) is recorded in the spec and not in the asset.

### 8. Two smaller ones

- **`MAX_NEED_ROWS` is a gui constant** (`render/manifest_layout.rs:110`), not the `tuning.rs` precedent §8 cites. The conclusion §8 draws is still right and `tuning::MAX_TOOL_ROWS` (line 4905) is the precedent that actually holds — a no-scroll ceiling the engine truncates against.
- **No new `descriptions` accessor is needed.** §7 asks for one sentence of provenance from a `"program.dossier"` subject. `DescriptionDb::paragraph(subject, condition, seed)` (`descriptions.rs:279`) already answers exactly that from a subject id, returns `None` when the bank has nothing, and joins opener/detail/coda. The pool is authored in `openers:` and read through that call. The `Game`-side doors in `game/descriptions.rs` are all `StackPos`-keyed and none of them is used.

**One census this plan must not leave lying:** `Game::spawn_squad`'s doc comment (`game/squads.rs:18-49`) is an explicit component-by-component census against `spawn_wild_creature_scaled`'s tuple. Adding a component to that tuple without adding a line there leaves a doc comment that claims to enumerate an omission it no longer enumerates. Task 5 updates it: a squad is a stand-in for its members inside one fight, never inspected and never saved, so `Attributes` is a deliberate omission.

---

## Global constraints

- **`#[serde(default)]` on every new asset and save field** — `SpeciesDef::attributes`, `ClassDef::attributes`, `CreatureSave::attributes`, `PlayerSave::attributes`. No existing `.ron`, including a mod's, may need editing.
- **No `save::SAVE_FORMAT_VERSION` bump.** It is 32; the on-disk save is field-named RON with the version on its first line (`save.rs:1629`), so an added field is free. Do not bump it and do not recapture `dev-saves/`.
- **No `resources::GameRng` draw, anywhere in this change.** This is the constraint every seeded baseline in the repo depends on: `balance_sim`'s curves, the arena reports, and every seed-luck test. Task 5's test is what holds it.
- **`derive::index`, never `%`.** A modulo on a fold reads little but the low bits the final multiply provably never disturbs, which anti-correlates small pools while looking reasonable. `derive.rs:80` carries the argument.
- **No new `Resource`.** `AttributeDb` is one, and it is an asset database registered exactly where `NeedDb` is. Nothing else. A new resource shifts bevy's query iteration order across the engine, which this repo has recorded as a cause of seeded tests moving.
- **An absent `assets/attributes/` is the pre-feature game**, held at both ends: the mint resolves a def before storing anything, and every reader skips what it cannot resolve. Never gate a system, a screen or a draw on the catalogue being non-empty.
- **Nothing reads an attribute for a mechanic.** No formula, no roll, no gate. If a task seems to want one, it is out of scope.
- **Gates before every commit:** `cargo fmt`, then `cargo clippy --workspace --all-targets` (`--all-targets` is load-bearing; a bare run leaves test modules unlinted), then the task's own tests. `cargo test --workspace` is the gate at Task 6 and Task 11 only. `cargo test -p feral-processes-engine balance_sim` is the balance gate at Task 6 and Task 11 — the curves must not move, and a moved curve means something is reading an attribute.
- **Engine tests may reach `Game`'s `world`.** The field is declared private in `lib.rs:173`, but it is declared in the *crate root*, and a private item is visible to the defining module and every descendant — so every module under `crate::tests` can read it, which `tests/base_grid.rs:86` already does. That is what CLAUDE.md's "compiler barrier from outside the crate" means, and four tests in this plan depend on it.
- **A test that writes a save builds its own path and removes it.** `std::env::temp_dir().join(format!("feral_processes_<tag>_{}.bin", std::process::id()))`, then `std::fs::remove_file` — `tests/base_grid.rs:92` is the idiom. The pid suffix is not decoration: a fixed temp path has already collided with another test's loop in this repo, and the panic named the innocent line.
- **Do not bump the workspace version or write a `CHANGELOG.md` section on this branch.** That happens once, at the merge.
- **Vocabulary.** Player-facing copy says *attribute*, and each row shows the setting name with the legacy name parenthesised after it. A routine is *run*, not cast. The unit of health is Integrity. No occult words.

---

## File structure

| File | Task | Responsibility |
|---|---|---|
| `crates/engine/src/attributes.rs` | 1, 3, 7 | `AttributeId`, `AttributeDef`, `AttributeDb`, `mint`, `revision`, `checksum` |
| `crates/engine/src/lib.rs` | 1 | `pub mod attributes;` |
| `crates/engine/src/game/lifecycle.rs` | 1, 5, 6 | `AssetDbs::attributes`, both resource registrations, the load-path mint |
| `crates/engine/src/game/catalog.rs` | 1 | `Game::attribute_defs` |
| `assets/attributes/*.ron` | 2 | the five shipped defs |
| `assets/attributes/README.md` | 2 | the schema, per the moddability rule |
| `crates/engine/src/derive.rs` | 3 | `fold_bytes`, with `fold` expressed through it |
| `crates/engine/src/game/contracts.rs` | 3 | `fold` delegates rather than duplicating |
| `crates/engine/src/components.rs` | 3 | `Attributes` |
| `crates/engine/src/species.rs` | 4 | `SpeciesDef::attributes` |
| `crates/engine/src/classes.rs` | 4 | `ClassDef::attributes` |
| `assets/species/README.md`, `assets/classes/README.md` | 4 | the two schema docs |
| `crates/engine/src/game/spawning.rs` | 5 | the wild-door mint |
| `crates/engine/src/game/party.rs` | 5 | the fusion mint |
| `crates/engine/src/game/creation.rs` | 5 | the player's mint |
| `crates/engine/src/game/squads.rs` | 5 | the omission census |
| `crates/engine/src/save.rs` | 6 | two `attributes` fields |
| `assets/descriptions/program_dossier.ron` | 7 | the provenance pool |
| `crates/engine/src/views.rs` | 8 | `DossierReport`, `AttributeRow` |
| `crates/engine/src/game/inspection.rs` | 8 | `Game::dossier_report` |
| `crates/engine/src/tuning.rs` | 8 | `MAX_ATTRIBUTE_ROWS` |
| `crates/engine/src/tests/attributes.rs` | 3–8 | the feature's engine tests (new module) |
| `crates/engine/src/tests/assets.rs` | 2, 4 | the four censuses |
| `crates/app-core/src/lib.rs` | 9 | `Mode::Dossier` |
| `crates/app-core/src/app/inspection.rs` | 9 | `D` on the manifest, `handle_dossier_key` |
| `crates/app-core/src/app/input.rs` | 9 | the route |
| `crates/gui/src/render/dossier.rs` | 10 | the page (new file) |
| `crates/gui/src/render/mod.rs` | 10 | the draw arm, `ALL_MODES` 114 → 115 |
| `assets/help/` | 11 | the manual page |

**Why the page is its own gui file.** `render/party.rs` already holds the roster, the gear page, the fusion picker and the memories page. The dossier is a second sheet about one body, read from the manifest rather than from the roster, so it goes beside `render/manifest.rs`'s concerns as its own module — and the two censuses in Task 10 want a file of their own to live in.

---

### Task 1: The catalogue

**Files:**
- Create: `crates/engine/src/attributes.rs`
- Modify: `crates/engine/src/lib.rs` (the module list)
- Modify: `crates/engine/src/game/lifecycle.rs` (`AssetDbs` at 2854, the loader beside `NeedDb` at 2979, **both** resource registrations — ~435 in the `Game::new` path and ~1141 in the `Game::load` path)
- Modify: `crates/engine/src/game/catalog.rs` (`Game::attribute_defs`, beside `item_defs`)
- Test: `crates/engine/src/attributes.rs`'s own `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `attributes::AttributeId` (transparent string newtype, `Ord`), `attributes::AttributeDef { id, name, legacy, short, meaning, base: i32, spread: i32 }`, `attributes::AttributeDb` with `load_dir(&Path) -> io::Result<(Self, Vec<String>)>`, `get(&AttributeId) -> Option<&AttributeDef>`, `iter() -> impl Iterator<Item = &AttributeDef>`, `len()`, `is_empty()`; `Game::attribute_defs(&self) -> Vec<AttributeDef>`.
- Consumes: nothing.

`NeedDb` is the template and the copy should be close to line-for-line — the same `NotFound`-is-silent arm, the same `paths.sort()` with the same comment, the same per-file warning string shape, the same `iter()`-is-sorted doc comment. Read `crates/engine/src/needs.rs` in full first; it is 250 lines and it is the spec for this file.

- [ ] **Step 1: Write the failing tests.** Four, mirroring `needs.rs`'s own four exactly:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed def, differing only in the fields a test cares about.
    fn def_text(id: &str, name: &str) -> String {
        format!(
            "(\n    id: \"{id}\",\n    name: \"{name}\",\n    legacy: \"Willpower\",\n    \
             short: \"holds together under stress\",\n    meaning: \"How cleanly this \
             program keeps its own state.\",\n    base: 50,\n    spread: 12,\n)\n"
        )
    }

    fn load(files: &[(&str, String)]) -> (AttributeDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("attributes");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        AttributeDb::load_dir(&dir).unwrap()
    }

    #[test]
    fn the_shipped_defs_load_and_are_found_by_id() {
        let (db, warnings) =
            AttributeDb::load_dir(&crate::tests::support::test_assets_dir().join("attributes"))
                .unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        for id in ["persistence", "entropy", "bandwidth", "footprint", "parity"] {
            let def = db
                .get(&AttributeId::from(id))
                .unwrap_or_else(|| panic!("{id}"));
            assert!(
                def.spread < def.base,
                "{id}: a spread at or above the base can mint a negative attribute"
            );
            assert!(!def.legacy.is_empty(), "{id} must name its old-school word");
        }
    }

    #[test]
    fn a_malformed_file_is_skipped_and_warns_without_losing_its_neighbours() {
        let (db, warnings) = load(&[
            ("bad.ron", "(id: \"broken\", name:".to_string()),
            ("good.ron", def_text("persistence", "Persistence")),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("bad.ron"), "{warnings:?}");
        assert!(db.get(&AttributeId::from("persistence")).is_some());
        assert!(db.get(&AttributeId::from("broken")).is_none());
    }

    /// Deleting `assets/attributes/` is a supported way to play, so an
    /// absent directory is not even a warning.
    #[test]
    fn an_absent_directory_loads_an_empty_database_silently() {
        let dir = crate::tests::support::scratch_assets_dir("attributes_absent");
        assert!(!dir.exists(), "the fixture must not create the directory");
        let (db, warnings) =
            AttributeDb::load_dir(&dir).expect("an absent directory is not an error");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(db.iter().count(), 0);
    }

    /// Every caller iterates `iter`, and the page draws it in that order, so
    /// it is sorted by id whatever order the directory hands its entries
    /// back in.
    #[test]
    fn iteration_is_in_id_order_however_the_files_were_written() {
        let (db, warnings) = load(&[
            ("z.ron", def_text("parity", "Parity")),
            ("a.ron", def_text("bandwidth", "Bandwidth")),
        ]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let ids: Vec<&str> = db.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["bandwidth", "parity"]);
    }
}
```

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine attributes::` — expected: does not compile, unresolved module `attributes`. The shipped-defs test will still fail after the module exists, until Task 2 writes the files; leave it failing and say so in the commit message, or `#[ignore]` it and un-ignore in Task 2. Prefer leaving it failing and committing Task 1 and Task 2 together if that reads better to you — but do not weaken the assertion to make it pass early.

- [ ] **Step 3: Write the module.** The header carries the argument for why this is data where `disposition.rs` is not, because that is the question a reader arrives with:

```rust
//! What a program is like, apart from what it can do in a fight — loaded
//! from `assets/attributes/`.
//!
//! One def per attribute: its name in this setting's words, the old-school
//! word beside it, two lengths of player-facing prose, and the base and
//! spread a body's own value is minted within. **Nothing reads these
//! numbers.** They are what the dossier page shows, and each one's eventual
//! mechanic is designed for the number rather than around it — see
//! `docs/superpowers/specs/2026-09-21-program-attributes-design.md` §2.
//!
//! **Data where `disposition.rs` is Rust, and that line is the whole
//! reason this directory exists.** A `Disposition` ships no name, no blurb
//! and no glyph, only multipliers on numbers the sim already computes,
//! which puts it with `tuning.rs` on the not-moddable side. An attribute is
//! the inverse: a name, a legacy name and two lines of prose, and today no
//! multiplier at all. That is content by the same test that put species,
//! needs, memories and the perk catalogue in `assets/`.
//!
//! **An empty database is valid and inert**, exactly like `NeedDb`: nothing
//! is minted, every body's store stays empty and the dossier page reports
//! no rows. Deleting `assets/attributes/` restores the pre-attribute game
//! rather than breaking an install. Never gate a mint, a reader or a draw
//! on the database being non-empty — that makes the property hold by
//! accident at one site and lapse at another.
```

Then the id, the def and the db. `AttributeId` is `NeedId` with the names changed: `#[serde(transparent)]`, deriving `Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize`, with `as_str`, `From<&str>` and `Display`. It derives `Ord` for `Attributes`' `BTreeMap` — the reason `Stock` keys by an `Ord` id, so the save encoding cannot differ run to run.

```rust
/// One attribute.
///
/// These seven fields are the initial schema and every one of them is
/// **required**: an attribute with no prose is the thing this feature
/// exists to avoid, and one with no `base` cannot be minted. Any field
/// added *later* must be `#[serde(default)]`, per the standing rule for
/// `SpeciesDef`/`StructureDef`/`ItemDef`, so a mod's existing files keep
/// parsing untouched — but do not retroactively default these.
///
/// The field that is deliberately **absent** is the one saying what the
/// attribute *does*. It lands with the mechanic that does it, as a second
/// authored field, because a gloss promising an effect is a claim the
/// player will test and find false.
#[derive(Clone, Debug, Deserialize)]
pub struct AttributeDef {
    pub id: AttributeId,
    /// What the dossier row leads with, in this setting's vocabulary.
    pub name: String,
    /// The old-school word in parentheses after it — `"Willpower"`,
    /// `"Luck"`. What lets a player who has never read a line of code know
    /// what they are looking at.
    pub legacy: String,
    /// The one-line gloss beside the number.
    pub short: String,
    /// The page's prose: what a high or low value says about this program.
    /// Never what it does — see the struct doc.
    pub meaning: String,
    /// The value a body with nothing authored for it mints around.
    pub base: i32,
    /// How far either side of the base a body's own value may land. `0` is
    /// legal and means every body reads the same number.
    pub spread: i32,
}
```

`AttributeDb` is `#[derive(Resource, Default)]` over `defs: BTreeMap<AttributeId, AttributeDef>`, with `load_dir`, `get`, `iter`, `len` and `is_empty`.

- [ ] **Step 4: Register it.** Four edits in `lifecycle.rs` and one in `catalog.rs`, and **all four in `lifecycle.rs` are required** — the struct field, the load, and the resource insertion on *both* the `Game::new` and `Game::load` paths. A field added to `AssetDbs` fails to compile until both literals name it, which is what makes this safe; the load is the one that could silently be left out.

```rust
// in AssetDbs, beside `needs`:
attributes: crate::attributes::AttributeDb,

// in the loader, beside NeedDb's:
// Same absent-is-silent rule again — see `AttributeDb`'s own doc. An empty
// catalogue mints nothing and leaves the dossier page with no rows, which
// is the pre-attribute game.
let (attributes, attribute_warnings) =
    crate::attributes::AttributeDb::load_dir(&assets_dir.join("attributes"))?;
warnings.extend(attribute_warnings);
```

`Game::attribute_defs` follows `Game::item_defs`' shape in `game/catalog.rs` — a `Vec<AttributeDef>` cloned out of the resource, so gui and app-core can read the catalogue without a borrow on `Game`.

- [ ] **Step 5: Run the module's tests.** `cargo test -p feral-processes-engine attributes::` — three pass, `the_shipped_defs_load_and_are_found_by_id` fails on a missing directory until Task 2.
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 2: The five shipped defs, the README and the def census

**Files:**
- Create: `assets/attributes/persistence.ron`, `entropy.ron`, `bandwidth.ron`, `footprint.ron`, `parity.ron`
- Create: `assets/attributes/README.md`
- Modify: `crates/engine/src/tests/assets.rs`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: `AttributeDb`, `AttributeDef`, `Game::attribute_defs` (Task 1).
- Produces: five shipped ids — `"persistence"`, `"entropy"`, `"bandwidth"`, `"footprint"`, `"parity"` — which Task 4's censuses and Task 2's own census both name.

- [ ] **Step 1: Write the failing census.** In `tests/assets.rs`, following `every_shipped_item_and_structure_has_description_text` (line 232) — a `Game::new` against `test_assets_dir()`, then a loop over the defs. This is the census §3 of the spec asks for, and it is what pays for the catalogue being data: a `#[serde(default)]` field can ship inert and unauthored, as `AbilityDef::spread` did.

```rust
/// Every shipped attribute authors all seven fields, and the two prose
/// fields are actually prose.
///
/// The cost of the catalogue being data, paid: nothing in Rust enumerates
/// the shipped five, so without this an attribute could ship with an empty
/// `meaning` and the dossier page would draw a blank line under it.
#[test]
fn every_shipped_attribute_says_what_it_means() {
    let game = Game::new(905, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let defs = game.attribute_defs();
    assert_eq!(defs.len(), 5, "the shipped catalogue is five attributes");
    for def in &defs {
        assert!(!def.name.is_empty(), "{} has no name", def.id);
        assert!(!def.legacy.is_empty(), "{} has no legacy name", def.id);
        assert!(
            def.short.len() >= 10,
            "{}'s gloss is too short to say anything: {:?}",
            def.id,
            def.short
        );
        assert!(
            def.meaning.split_whitespace().count() >= 15,
            "{}'s meaning is a phrase, not prose: {:?}",
            def.id,
            def.meaning
        );
        assert!(
            def.spread < def.base,
            "{}: a spread at or above the base can mint a negative attribute",
            def.id
        );
    }
}

/// The gloss may not promise a mechanic. Decision 2 of the spec: the
/// sentence saying what an attribute *does* lands with the mechanic that
/// does it, and until then a claim the player tests and finds false is
/// worse than no claim.
#[test]
fn no_shipped_attribute_claims_an_effect() {
    let game = Game::new(906, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for def in game.attribute_defs() {
        let prose = format!("{} {}", def.short, def.meaning).to_lowercase();
        for claim in [
            "increases", "reduces", "improves", "raises", "lowers", "bonus",
            "chance to", "per point", "rises over", "grows over",
        ] {
            assert!(
                !prose.contains(claim),
                "{} promises a mechanic ({claim:?}) that nothing implements: {prose}",
                def.id
            );
        }
    }
}
```

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine assets::every_shipped_attribute` — expected: `the shipped catalogue is five attributes`, left `0`.

- [ ] **Step 3: Write the five files.** `base` and `spread` are flavour, not difficulty — nothing reads them — so they are authored here rather than in `tuning.rs`. A base near the middle of 0..100 and a spread around a tenth of it gives a page where two programs plainly differ without any value looking broken.

```ron
( id: "persistence",
  name: "Persistence",
  legacy: "Willpower",
  short: "holds its own state under stress",
  meaning: "How cleanly this program keeps its own state when things go \
            wrong around it. A program low in Persistence is the one that \
            comes back from a bad fight not quite the same as it went in.",
  base: 50,
  spread: 12 )
```

```ron
( id: "entropy",
  name: "Entropy",
  legacy: "Luck",
  short: "carries accumulated noise",
  meaning: "How much accumulated noise this program runs with — the drift \
            that collects in anything left running long enough. A \
            high-Entropy program is one whose edges have gone soft in ways \
            nobody deliberately wrote.",
  base: 40,
  spread: 15 )
```

```ron
( id: "bandwidth",
  name: "Bandwidth",
  legacy: "Stamina",
  short: "how much it pushes at once",
  meaning: "How much this program can push through at one time. A \
            narrow-Bandwidth program does one thing at a time and does it \
            properly; a wide one spreads itself across whatever it is \
            pointed at.",
  base: 50,
  spread: 14 )
```

```ron
( id: "footprint",
  name: "Footprint",
  legacy: "Size",
  short: "how much heap it occupies",
  meaning: "How much of the heap this program takes up while it runs. A \
            large-Footprint program is a presence wherever it is loaded, \
            and a small one is the kind you forget is resident at all.",
  base: 45,
  spread: 16 )
```

```ron
( id: "parity",
  name: "Parity",
  legacy: "Vitality",
  short: "corrects its own errors",
  meaning: "How well this program catches and corrects its own errors \
            without anyone looking. High Parity is a program that quietly \
            puts itself right; low Parity is one whose small faults stay \
            where they landed.",
  base: 50,
  spread: 13 )
```

Note the `\`-continued strings: that is `assets/needs/`'s own style for prose that would otherwise run past the column, and RON handles it.

- [ ] **Step 4: Write `assets/attributes/README.md`.** `assets/needs/README.md` is the template — the same "edit or add a `.ron` and it is picked up next session", the same "this directory may be deleted" paragraph naming what the pre-feature game is, the same one worked example followed by a field table. It must say, in the author's own terms:
  - all seven fields are required, and a malformed file costs that one attribute;
  - `base` and `spread` are flavour and read by nothing, so tuning them changes no outcome;
  - **a value is minted once, from the place and the body, and stored** — so editing a `base` does not change a program that already exists;
  - a sixth attribute is a file drop, and nothing in Rust enumerates the five;
  - `meaning` must not promise an effect, and why (the census in `tests/assets.rs` enforces it);
  - the id is what a save records, so renaming one drops whatever a body had under the old name and it mints again on the next load.
- [ ] **Step 5: Run the whole set.** `cargo test -p feral-processes-engine attributes:: assets::every_shipped_attribute assets::no_shipped_attribute` — all green now, including Task 1's shipped-defs test.
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 3: The store and the mint

**Files:**
- Modify: `crates/engine/src/derive.rs` (`fold_bytes`; `fold` expressed through it)
- Modify: `crates/engine/src/game/contracts.rs` (`fold` delegates, line 417)
- Modify: `crates/engine/src/components.rs` (`Attributes`)
- Modify: `crates/engine/src/attributes.rs` (`mint`, `ATTRIBUTE_SALT`)
- Create: `crates/engine/src/tests/attributes.rs`, registered in `crates/engine/src/tests/mod.rs`
- Test: `crates/engine/src/attributes.rs`'s own tests and `crates/engine/src/tests/attributes.rs`

**Interfaces:**
- Consumes: `AttributeDb`, `AttributeDef` (Task 1).
- Produces: `derive::fold_bytes(seed: u64, bytes: &[u8]) -> u64`; `components::Attributes(BTreeMap<AttributeId, i32>)` with `get(&AttributeId) -> Option<i32>`, `set(&AttributeId, i32)`, `iter() -> impl Iterator<Item = (&AttributeId, i32)>`, `is_empty()`; `attributes::mint(db: &AttributeDb, seed: u64, authored: &BTreeMap<String, i32>) -> Attributes`; `attributes::body_seed(world_seed: u32, x: i32, y: i32, species: &str, zone: u32) -> u64`; `attributes::program_seed(program_id: u32) -> u64`; `attributes::player_seed(world_seed: u32) -> u64`.

**One fold loop in the crate, not two.** `derive::fold` mixes `&[u64]` a byte at a time; `game::contracts::fold` (line 417) is the same FNV-1a loop over `&[u8]`, added later for the same job. The mint needs the byte form, because an attribute's salt is its **id string** — an index into the catalogue would reshuffle everyone the moment a sixth file was dropped in. Rather than importing `game::contracts` from a top-level module, or writing a third copy, add the byte form to `derive.rs` and express both through it. This is a five-line change that removes a duplicate instead of adding one, and `derive.rs`'s own `fold_is_fnv_1a_one_byte_at_a_time` test pins the result unchanged.

- [ ] **Step 1: Write the failing tests.** In `crates/engine/src/attributes.rs`'s test module, four of them — the mint's whole contract:

```rust
    fn two_defs() -> AttributeDb {
        let (db, _) = load(&[
            ("a.ron", def_text("bandwidth", "Bandwidth")),
            ("b.ron", def_text("entropy", "Entropy")),
        ]);
        db
    }

    /// The property every seeded baseline in the repo rests on, at this
    /// level: the same place and the same body always mint the same
    /// numbers, so a save/load cannot change who a program is.
    #[test]
    fn minting_is_deterministic() {
        let db = two_defs();
        let authored = BTreeMap::new();
        let seed = body_seed(1234, -7, 19, "crawler", 3);
        assert_eq!(mint(&db, seed, &authored), mint(&db, seed, &authored));
    }

    /// Every minted value lands inside the def's own window, including at
    /// the window's edges, so no reader needs to clamp.
    #[test]
    fn every_minted_value_is_inside_base_plus_or_minus_spread() {
        let db = two_defs();
        let authored = BTreeMap::new();
        for n in 0..500u32 {
            let attrs = mint(&db, body_seed(n, n as i32, -(n as i32), "crawler", 1), &authored);
            for (id, value) in attrs.iter() {
                let def = db.get(id).unwrap();
                assert!(
                    (def.base - def.spread..=def.base + def.spread).contains(&value),
                    "{id} minted {value} outside {}±{}",
                    def.base,
                    def.spread
                );
            }
        }
    }

    /// An authored base replaces the def's, and the spread still applies
    /// around it — which is what makes a species' own numbers show through
    /// while two of that species still differ.
    #[test]
    fn an_authored_base_replaces_the_defs_own() {
        let db = two_defs();
        let mut authored = BTreeMap::new();
        authored.insert("bandwidth".to_string(), 90);
        let def = db.get(&AttributeId::from("bandwidth")).unwrap();
        for n in 0..200u32 {
            let value = mint(&db, body_seed(n, 0, 0, "crawler", 1), &authored)
                .get(&AttributeId::from("bandwidth"))
                .unwrap();
            assert!(
                (90 - def.spread..=90 + def.spread).contains(&value),
                "authored base ignored: minted {value}"
            );
        }
    }

    /// Two attributes must not move in lockstep. `derive::index` reads the
    /// high bits and each attribute salts the body's seed with its own id,
    /// so a two-entry catalogue decorrelates the same as a larger one —
    /// the trap `descriptions::Slot::tags` carries the measurement for, and
    /// the one a `%` reduction passes every casual test while failing.
    #[test]
    fn two_attributes_do_not_move_in_lockstep() {
        let db = two_defs();
        let authored = BTreeMap::new();
        let mut joint = std::collections::HashSet::new();
        for n in 0..2000u32 {
            let attrs = mint(&db, body_seed(n, 0, 0, "crawler", 1), &authored);
            let a = attrs.get(&AttributeId::from("bandwidth")).unwrap();
            let b = attrs.get(&AttributeId::from("entropy")).unwrap();
            joint.insert((a > 50, b > 50));
        }
        assert_eq!(
            joint.len(),
            4,
            "two attributes reached only {} of 4 joint outcomes; the \
             reduction is correlating them",
            joint.len()
        );
    }

    /// An empty catalogue mints an empty store, with no branch at any call
    /// site — the pre-attribute game.
    #[test]
    fn an_empty_catalogue_mints_an_empty_store() {
        let db = AttributeDb::default();
        let attrs = mint(&db, body_seed(1, 2, 3, "crawler", 1), &BTreeMap::new());
        assert!(attrs.is_empty());
    }
```

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine attributes::` — expected: `cannot find function 'mint'`.

- [ ] **Step 3: Add `derive::fold_bytes` and route `fold` through it.** In `derive.rs`, keeping the existing doc comment on `fold` and adding the new one:

```rust
/// Continues an FNV-1a fold with further **bytes**.
///
/// The byte form of `fold`, and the one every string-keyed caller wants: an
/// id folded as text cannot be reshuffled by a file being added to the
/// directory beside it, where an index into a sorted catalogue can.
///
/// `fold` is expressed through this rather than beside it, so the crate has
/// exactly one FNV-1a loop. `game::contracts::fold` was the second and now
/// delegates here.
pub(crate) fn fold_bytes(seed: u64, bytes: &[u8]) -> u64 {
    let mut h = seed;
    for byte in bytes {
        h ^= *byte as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

pub(crate) fn fold(seed: u64, words: &[u64]) -> u64 {
    let mut h = seed;
    for &word in words {
        h = fold_bytes(h, &word.to_le_bytes());
    }
    h
}
```

Then `game/contracts.rs:417` becomes a delegation, its doc comment amended to point at the shared door rather than restating the byte-wise argument:

```rust
pub(crate) fn fold(h: u64, bytes: &[u8]) -> u64 {
    crate::derive::fold_bytes(h, bytes)
}
```

`derive.rs`'s four existing tests must stay green untouched — `fold_is_fnv_1a_one_byte_at_a_time` writes the loop out longhand and is what proves the refactor changed no value. Do not edit them.

- [ ] **Step 4: Add the component.** In `components.rs`, beside the other map-shaped stores:

```rust
/// What a body is like apart from what it can do in a fight — see
/// `attributes.rs`.
///
/// **A component of its own and pointedly not a field on `Stats`.** `Stats`
/// is level-scaled by `progression::stats_after_levels`, zone-multiplied by
/// `ZoneLevel::stat_multiplier`, folded into by
/// `Game::apply_equipment_delta` and summed by `Stats::power()`, which
/// `Game::difficulty_color`, `progression::kill_xp` and trade valuation all
/// read. A sixth field there would enter every one of those whether it was
/// wanted or not, and `balance_sim`'s empirical curves would move. Being
/// separate is also what keeps "nothing reads an attribute" true by
/// construction: nothing that reads `Stats` can start reading one by
/// accident.
///
/// **Minted once and stored**, never re-derived on read: the value is a
/// property of the body and the place it came from, and `Disposition`'s own
/// argument applies — a fold that ran on every read would be free to
/// disagree with the one the save recorded.
///
/// `BTreeMap` for `Stock`'s reason: iteration order feeds the save
/// encoding, and a `HashMap` would make the file differ run to run. An
/// entry whose def no file defines is kept and skipped by every reader,
/// exactly as a `Memories` entry is — which is what makes a catalogue
/// edited between sessions a supported thing to do.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Attributes(BTreeMap<crate::attributes::AttributeId, i32>);
```

with `get`, `set`, `iter` and `is_empty`. Follow `components::Needs`' accessor shape; keep the map private for `Stock`'s reason — the field being private is what stops a caller writing a value the mint never produced.

- [ ] **Step 5: Write the mint.** In `attributes.rs`:

```rust
/// This feature's own fold salt, so nothing derived here can collide with
/// anything else derived from the same world seed — `FrameSpec::salted`'s
/// rule, one scheme rather than a second seed source.
const ATTRIBUTE_SALT: u64 = 0xA771_B17E_5EED_0001;

/// A wild body's seed: the world, the tile it stood on, what it is, and how
/// deep the run had gone.
///
/// The species id is folded **last and as bytes**, so every byte of it gets
/// its own XOR-and-multiply round. That is what carries a one-character
/// difference up to bit 63, which is the bit `derive::index` actually reads
/// — see `derive::index`'s doc for why a value folded in as one whole word
/// never reaches it.
///
/// Two bodies of one species spawned on one tile in one zone mint
/// identically. Accepted: it is invisible flavour, and the alternative — a
/// spawn counter as a new `Resource` — reshuffles bevy's query iteration
/// order across the whole engine.
pub fn body_seed(world_seed: u32, x: i32, y: i32, species: &str, zone: u32) -> u64 {
    let h = derive::fold(
        derive::FNV_BASIS,
        &[
            ATTRIBUTE_SALT,
            world_seed as u64,
            x as i64 as u64,
            y as i64 as u64,
            zone as u64,
        ],
    );
    derive::fold_bytes(h, species.as_bytes())
}

/// A program with no place: one fused from two parents, minted off the
/// `ProgramId` `Game::roster_parts` just handed it. `Disposition::seed`'s
/// own input, for its reason.
pub fn program_seed(program_id: u32) -> u64 {
    derive::fold(derive::FNV_BASIS, &[ATTRIBUTE_SALT, program_id as u64])
}

/// The player, who has no species and no spawn tile worth folding.
pub fn player_seed(world_seed: u32) -> u64 {
    derive::fold(derive::FNV_BASIS, &[ATTRIBUTE_SALT, world_seed as u64])
}

/// Every attribute the catalogue knows about, minted for one body.
///
/// `authored` is the body's own base per attribute — a species' or a
/// class's `attributes:` map — and an id it does not name falls through to
/// the def's own `base`. Keyed by `String` rather than `AttributeId`
/// because that is the shape a `#[serde(default)]` field on `SpeciesDef`
/// takes without either db depending on the other.
///
/// **Spends no `resources::GameRng` draw.** A draw does not survive a
/// save/load, so the same program would read differently after a reload,
/// and a fourth draw at the wild spawn would shift every later roll in the
/// run — moving seed-luck tests, arena baselines and `balance_sim`'s
/// reference numbers for a cosmetic value.
///
/// Each attribute salts the body's seed with **its own id** rather than
/// reading a second slice of one number, `CaravanVisit`'s rule: a sixth
/// attribute dropped into the directory must not move the five beside it.
pub fn mint(
    db: &AttributeDb,
    seed: u64,
    authored: &std::collections::BTreeMap<String, i32>,
) -> Attributes {
    let mut out = Attributes::default();
    for def in db.iter() {
        let base = authored
            .get(def.id.as_str())
            .copied()
            .unwrap_or(def.base);
        // `spread: 0` gives a span of one, which `index` answers `0` for —
        // a fixed attribute, and no special case.
        let span = (2 * def.spread + 1).max(1) as usize;
        let offset = derive::index(derive::fold_bytes(seed, def.id.as_str().as_bytes()), span);
        out.set(&def.id, base - def.spread + offset as i32);
    }
    out
}
```

- [ ] **Step 6: Create `crates/engine/src/tests/attributes.rs`** as an empty module with a header doc comment naming what it holds, and register it in `crates/engine/src/tests/mod.rs`. Tasks 5–8 fill it.
- [ ] **Step 7: Run the tests.** `cargo test -p feral-processes-engine attributes:: derive::` — all green, including `derive.rs`'s four untouched.
- [ ] **Step 8: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 4: Where an authored base comes from

**Files:**
- Modify: `crates/engine/src/species.rs` (`SpeciesDef`, after `equipment_drop` at line 317)
- Modify: `crates/engine/src/classes.rs` (`ClassDef`, after `kit` at line 149)
- Modify: `assets/species/README.md`, `assets/classes/README.md`
- Modify: `crates/engine/src/tests/assets.rs`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: the five shipped ids (Task 2), `attributes::mint`'s `authored` parameter (Task 3).
- Produces: `SpeciesDef::attributes: BTreeMap<String, i32>` and `ClassDef::attributes: BTreeMap<String, i32>`, both `#[serde(default)]`.

The spec's §6 asks the shipped species and classes to author every shipped attribute, and §11's test 6 is the census that makes that true. **Authoring is still optional by schema** — that is what keeps a mod's species files parsing — and the census binds the *shipped* files only, exactly as `ZONE_MATERIALS` in `tests/assets.rs` binds shipped recipes.

- [ ] **Step 1: Write the two failing censuses.**

```rust
/// Every shipped species authors every shipped attribute.
///
/// The field is `#[serde(default)]`, so nothing in the compiler or the
/// loader notices a species that authors none — it simply mints the
/// catalogue's own bases and reads identically to every other species,
/// which is the `AbilityDef::spread` failure: a field authored nowhere,
/// with nobody to find out.
#[test]
fn every_shipped_species_authors_every_attribute() {
    let game = Game::new(907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<String> = game
        .attribute_defs()
        .iter()
        .map(|d| d.id.as_str().to_string())
        .collect();
    for species in game.species_defs() {
        for id in &ids {
            assert!(
                species.attributes.contains_key(id),
                "species {} authors no {id}",
                species.id
            );
        }
        for authored in species.attributes.keys() {
            assert!(
                ids.contains(authored),
                "species {} authors {authored:?}, which no attribute def defines",
                species.id
            );
        }
    }
}

/// And every shipped class, which is the player's only authored source —
/// the player carries no `Creature` and no species.
#[test]
fn every_shipped_class_authors_every_attribute() {
    let game = Game::new(908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<String> = game
        .attribute_defs()
        .iter()
        .map(|d| d.id.as_str().to_string())
        .collect();
    let classes = game.class_defs();
    assert_eq!(classes.len(), 8, "the shipped catalogue is eight classes");
    for class in classes {
        for id in &ids {
            assert!(
                class.attributes.contains_key(id),
                "class {} authors no {id}",
                class.name
            );
        }
    }
}
```

`Game::species_defs` and `Game::class_defs` may not exist under those names — check `game/catalog.rs` for what is already exposed (`item_defs`, `structure_defs` are there) and use the existing reader, or add one in the same shape as `attribute_defs`. Do not add a second way to reach a db.

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine assets::every_shipped_species_authors assets::every_shipped_class_authors` — expected: does not compile, no field `attributes`.
- [ ] **Step 3: Add the two fields.**

```rust
/// This species' own base for each attribute, by `attributes::AttributeId`
/// — see `attributes.rs`. Absent, or an id the catalogue does not define,
/// falls through to that def's own `base`, which is what makes
/// `assets/attributes/` and `assets/species/` independent of each other.
///
/// `#[serde(default)]`, so no existing species file — including a mod's —
/// needs editing. The shipped seventeen are held to authoring all five by
/// `every_shipped_species_authors_every_attribute`.
#[serde(default)]
pub attributes: std::collections::BTreeMap<String, i32>,
```

and the same on `ClassDef`, with "the player's only authored source, since the player carries no `Creature`" in place of the first sentence.

- [ ] **Step 4: Author them.** 17 species files and 8 class files, five entries each. These are flavour and read by nothing, so author them to characterise: a heavy species high in Footprint and low in Bandwidth, a scout the inverse; the Bastion high in Persistence, the Saboteur high in Entropy, the Medic high in Parity. Stay inside roughly 20..80 so the spread cannot take a value somewhere odd, and do not make any one class best at everything.

```ron
    attributes: {
        "persistence": 62,
        "entropy": 35,
        "bandwidth": 48,
        "footprint": 55,
        "parity": 58,
    },
```

- [ ] **Step 5: Update both READMEs** in the same change — the standing rule is that `assets/*/README.md` moves whenever a field is added. Each gets a row in its field table and a short paragraph: the map is keyed by attribute id, an absent id takes the catalogue's base, the value is a *base* the body mints a spread around rather than the number the body ends up with, and editing it does not change a body that already exists.
- [ ] **Step 6: Run them.** `cargo test -p feral-processes-engine assets::` — the two new ones green, and `every_shipped_asset_file_loads_without_a_warning` still green, which is what proves 25 hand-edited RON files still parse.
- [ ] **Step 7: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 5: Minting at the four doors

**Files:**
- Modify: `crates/engine/src/game/spawning.rs` (`spawn_wild_creature_scaled`, after the `Boss` insert at ~line 345)
- Modify: `crates/engine/src/game/party.rs` (`fuse_companions`, the spawn at ~1383)
- Modify: `crates/engine/src/game/creation.rs` (`apply_creation_identity`, line 257)
- Modify: `crates/engine/src/game/squads.rs` (the omission census in `spawn_squad`'s doc comment, lines 18–49)
- Test: `crates/engine/src/tests/attributes.rs`

**Interfaces:**
- Consumes: `attributes::mint`, `body_seed`, `program_seed`, `player_seed`, `components::Attributes` (Task 3); `SpeciesDef::attributes`, `ClassDef::attributes` (Task 4).
- Produces: every creature and the player carry `Attributes` from the moment they exist.

**Insert, do not extend the tuple.** `spawn_wild_creature_scaled`'s bundle carries a comment saying it is already near bevy's arity limit and that `Boss` is a conditional insert for that reason; `spawn_player`'s is at the 15-element ceiling with a nested tuple. So the mint is an `.insert` after the spawn at both, and `fuse_companions` already `insert`s its `roster_parts` tuple separately.

- [ ] **Step 1: Write the failing tests.** In `crates/engine/src/tests/attributes.rs`. The first is the one the whole repo's seeded baselines depend on; write it first and treat a failure as a blocker rather than a puzzle.

```rust
/// **The test that protects every seeded baseline in the repo.** If the
/// mint ever draws from `resources::GameRng`, the next roll in the run
/// moves — and with it `balance_sim`'s curves, the arena's reports and
/// every test that spawns against a fixed seed.
///
/// Measured on the *second* creature's `Potential`, because that is the
/// first value a draw inside the first spawn would displace. An empty
/// catalogue stands in for "the mint does no work": if the two agree, the
/// mint's work is off the stream.
#[test]
fn minting_attributes_spends_no_rng_draw() {
    let rolls = |empty: bool| {
        let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        if empty {
            game.world
                .insert_resource(crate::attributes::AttributeDb::default());
        }
        let species = game.species_defs()[0].id.to_string();
        let a = game
            .spawn_wild_creature_scaled(&species, 40, 40, 1.0, false)
            .unwrap();
        let b = game
            .spawn_wild_creature_scaled(&species, 41, 40, 1.0, false)
            .unwrap();
        [a, b].map(|e| {
            let p = game.world.get::<crate::components::Potential>(e).unwrap();
            (p.hp_roll, p.atk_roll, p.def_roll)
        })
    };
    assert_eq!(
        rolls(false),
        rolls(true),
        "minting an attribute moved the RNG stream"
    );
}

/// Every wild body has attributes, and two of one species on two tiles
/// differ — which is the whole observable point of the mint.
#[test]
fn two_bodies_of_one_species_on_two_tiles_differ() {
    let mut game = Game::new(4243, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let a = game.spawn_wild_creature_scaled(&species, 60, 60, 1.0, false).unwrap();
    let b = game.spawn_wild_creature_scaled(&species, 61, 77, 1.0, false).unwrap();
    let of = |e| game.world.get::<crate::components::Attributes>(e).cloned().unwrap();
    assert!(!of(a).is_empty(), "a wild body minted no attributes");
    assert_ne!(of(a), of(b), "two bodies on two tiles minted identically");
}

/// The player's own, off the class it chose. A run with no class is
/// supported — `CharacterChoice::default()` has none — and mints the
/// catalogue's bases.
#[test]
fn the_player_has_attributes_from_its_class() {
    let game = Game::new(4244, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let attrs = game
        .world
        .get::<crate::components::Attributes>(player)
        .expect("the player mints attributes at creation");
    assert_eq!(attrs.iter().count(), 5);
}

/// Taming does not rewrite who a program is. A captured body keeps the
/// attributes it had in the wild, which is why the mint is not in
/// `Game::roster_parts` — see the plan's correction 3.
#[test]
fn adopting_a_program_keeps_the_attributes_it_had_wild() {
    let mut game = Game::new(4245, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let wild = game.spawn_wild_creature_scaled(&species, 70, 70, 1.0, false).unwrap();
    let before = game.world.get::<crate::components::Attributes>(wild).cloned().unwrap();
    let parts = game.roster_parts();
    game.world.entity_mut(wild).insert(parts);
    let after = game.world.get::<crate::components::Attributes>(wild).cloned().unwrap();
    assert_eq!(before, after, "joining the roster rewrote the program's attributes");
}
```

The exact fixture calls — `game.species_defs()[0].id`, `Game::new`'s signature, the `Potential` field names — must be checked against `crates/engine/src/tests/support.rs` and the real types before running; prefer a helper already there over a new one.

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine tests::attributes::` — expected: `the player mints attributes at creation` panics, and the wild ones find no component.
- [ ] **Step 3: Mint at the wild door.** In `spawn_wild_creature_scaled`, after the `Boss` insert and with the same reason given:

```rust
        // Kept out of the spawn tuple for the reason above, and minted
        // rather than rolled: `attributes::mint` reads no `GameRng`, so a
        // body's attributes cost the run's stream nothing. The species' own
        // authored bases come off the def this function already cloned.
        let attrs = crate::attributes::mint(
            self.world.resource::<crate::attributes::AttributeDb>(),
            crate::attributes::body_seed(
                self.world.resource::<crate::world::WorldMap>().seed(),
                x,
                y,
                species.id.as_str(),
                zone,
            ),
            &species.attributes,
        );
        self.world.entity_mut(entity).insert(attrs);
```

Check `WorldMap`'s resource path and `seed()`'s signature at `crates/engine/src/world.rs:207` before writing this; the map may be reached off a field rather than a resource.

- [ ] **Step 4: Mint at fusion.** `fuse_companions` spawns a new program with a species and a fresh `ProgramId` from `roster_parts`, so it mints off `program_seed(id)` — a new program has no place, and inheriting a parent's attributes would be a fifth thing fusion carries across with nothing saying so. Insert it beside the `Experience` override, with a one-line comment saying a fused program is a new program and mints its own.
- [ ] **Step 5: Mint for the player.** In `apply_creation_identity`, after the `PlayerIdentity` write — the first point `choice.class` is known:

```rust
        // The class's authored bases, or the catalogue's own where the run
        // has no class — `CharacterChoice::default()`'s supported state.
        // Here rather than in `spawn_player` because that function is a
        // free fn over `&mut World` with no class and no `ClassDb`, and its
        // bundle is already at bevy's 15-element ceiling.
        let authored = choice
            .class
            .and_then(|class| self.world.resource::<ClassDb>().get(class))
            .map(|def| def.attributes.clone())
            .unwrap_or_default();
        let attrs = crate::attributes::mint(
            self.world.resource::<crate::attributes::AttributeDb>(),
            crate::attributes::player_seed(self.world.resource::<crate::world::WorldMap>().seed()),
            &authored,
        );
        self.world.entity_mut(player).insert(attrs);
```

`ClassDb::get`'s real name and signature are in `crates/engine/src/classes.rs` below line 187 — use it rather than reaching into `defs`.

- [ ] **Step 6: Update `spawn_squad`'s census.** Add one bullet to the list in its doc comment, in the voice of the five already there:

```
    /// - `Attributes` — a squad is a stand-in for its members inside one
    ///   fight, is never inspected on a dossier and is never saved, and its
    ///   members keep their own. Nothing reads an attribute, so this is an
    ///   omission with no live consequence — but the census above claims to
    ///   enumerate the tuple, and a claim like that is only worth anything
    ///   while it is complete.
```

- [ ] **Step 7: Run them.** `cargo test -p feral-processes-engine tests::attributes::` — all green. Then `cargo test -p feral-processes-engine spawning:: squads::` as a spot check that no existing spawn test moved.
- [ ] **Step 8: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 6: The save, the load, and the old save

**Files:**
- Modify: `crates/engine/src/save.rs` (`CreatureSave` after `ring` at ~441, `PlayerSave` in the same shape)
- Modify: `crates/engine/src/game/lifecycle.rs` (`creature_save_for` at ~1994, `spawn_creature_from_save` at ~1641, `player_save_for` at 2279, the player's restore at ~2380 / `spawn_player_from_save` at 249)
- Test: `crates/engine/src/tests/attributes.rs`

**Interfaces:**
- Consumes: `components::Attributes`, `attributes::mint`, the three seed functions (Task 3); the mint sites (Task 5).
- Produces: `CreatureSave::attributes: BTreeMap<AttributeId, i32>`, `PlayerSave::attributes: BTreeMap<AttributeId, i32>`, both `#[serde(default)]`; a save written before this change mints on load.

**No version bump.** `SAVE_FORMAT_VERSION` is 32 and the file is field-named RON with the version on its own first line. An added field is additive and free; an older file carries no key and loads empty, which is what the mint-on-load arm is for. Do not bump it.

**The trap this task exists inside.** `spawn_creature_from_save`'s doc comment says it mints the same component set `roster_parts` does and *pointedly does not call it*, so the two must be kept in step by hand — "a new component that belongs to every owned program has to be added in both places, or a program will have it from the moment it is tamed and quietly lose it on the next reload." Attributes belong to *every* creature rather than every owned one, which makes the restore arm unconditional and puts it above the `if c.tamed` block.

- [ ] **Step 1: Write the failing tests.** A **real save→load through a file**, never a RON round trip: `#[serde(skip)]` and a field left out of the builder both leave a RON round-trip test green. `crates/gui/src/render/party.rs:1440-1449` is the idiom — build a `SaveData`, `save::save_to_file`, `Game::load`.

```rust
/// A real file round trip, not a RON round trip: a field left out of
/// `creature_save_for`, or marked `#[serde(skip)]`, leaves a
/// serialize-then-deserialize test perfectly green.
#[test]
fn attributes_survive_a_save_and_a_load() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_attributes_roundtrip_{}.bin",
        std::process::id()
    ));
    let mut game = Game::new(4246, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let wild = game.spawn_wild_creature_scaled(&species, 80, 80, 1.0, false).unwrap();
    let before = game.world.get::<crate::components::Attributes>(wild).cloned().unwrap();
    let player_before = game
        .world
        .get::<crate::components::Attributes>(game.player_entity())
        .cloned()
        .unwrap();
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let found = loaded
        .creature_attributes_at((80, 80))
        .expect("the creature came back");
    assert_eq!(found, before, "a creature's attributes changed across a load");
    assert_eq!(
        loaded
            .world
            .get::<crate::components::Attributes>(loaded.player_entity())
            .cloned()
            .unwrap(),
        player_before,
        "the player's attributes changed across a load"
    );
}

/// A save written before this feature carries no key, so the load mints —
/// which is what gets a run in progress its attributes with no migration
/// and no version bump. Modelled by clearing the field on the way out,
/// which is exactly what an older file's absent key deserialises to.
#[test]
fn an_old_save_mints_on_load_rather_than_staying_blank() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_attributes_old_save_{}.bin",
        std::process::id()
    ));
    let mut game = Game::new(4247, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    game.spawn_wild_creature_scaled(&species, 90, 90, 1.0, false).unwrap();
    game.save(&path).unwrap();

    let mut data = crate::save::load_from_file(&path).unwrap();
    for c in &mut data.creatures {
        c.attributes.clear();
    }
    data.player.attributes.clear();
    crate::save::save_to_file(&path, &data).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        loaded.creature_attributes_at((90, 90)).map(|a| a.iter().count()),
        Some(5),
        "an old save's creature stayed blank instead of minting"
    );
    assert_eq!(
        loaded
            .world
            .get::<crate::components::Attributes>(loaded.player_entity())
            .map(|a| a.iter().count()),
        Some(5),
        "an old save's player stayed blank instead of minting"
    );
}

/// And what it mints is what a fresh spawn on that tile would have minted,
/// which is what makes the mint a property of the place rather than of when
/// you happened to load.
#[test]
fn a_minted_old_save_agrees_with_a_fresh_spawn() {
    let path = std::env::temp_dir().join(format!(
        "feral_processes_attributes_agree_{}.bin",
        std::process::id()
    ));
    // Spawned fresh, and never saved: this is the answer the place gives.
    let mut fresh = Game::new(4251, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = fresh.species_defs()[0].id.to_string();
    let body = fresh
        .spawn_wild_creature_scaled(&species, 95, 95, 1.0, false)
        .unwrap();
    let expected = fresh
        .world
        .get::<crate::components::Attributes>(body)
        .cloned()
        .unwrap();
    // The same world, saved with the key stripped, so the load has to mint.
    fresh.save(&path).unwrap();
    let mut data = crate::save::load_from_file(&path).unwrap();
    for c in &mut data.creatures {
        c.attributes.clear();
    }
    crate::save::save_to_file(&path, &data).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.creature_attributes_at((95, 95)),
        Some(expected),
        "the load minted a different body than the place would have"
    );
}
```

`Game::creature_attributes_at` is a test-only reader; if `crates/engine/src/tests/support.rs` already has a "find the creature at this tile" helper, use that and read the component directly rather than adding a method to `Game`. Check before adding anything.

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine tests::attributes::` — expected: no field `attributes` on `CreatureSave`.
- [ ] **Step 3: Add the two save fields**, each with the doc comment the neighbours have — what it is, why it is persisted, and the explicit "additive on a field-named RON struct, so it costs no `SAVE_FORMAT_VERSION` bump" sentence that `ring` at line 440 models:

```rust
    /// What this body is like apart from a fight — see
    /// `components::Attributes`. Persisted rather than re-derived because
    /// the fold's inputs include the tile it was spawned on, which a body
    /// that has since walked away no longer stands on.
    ///
    /// Additive on a field-named RON struct, so it costs no
    /// `SAVE_FORMAT_VERSION` bump: an older file carries no key, loads
    /// empty, and `spawn_creature_from_save` mints it — which is how a run
    /// in progress gets attributes with no migration.
    #[serde(default)]
    pub attributes: std::collections::BTreeMap<crate::attributes::AttributeId, i32>,
```

- [ ] **Step 4: Write both halves.** In `creature_save_for`, read the component and collect (absent reads as empty, the same shape `memories` takes for a wild body). In `spawn_creature_from_save`, **above** the `if c.tamed` block and unconditional:

```rust
        // Every creature has these, not only owned ones, so this sits above
        // the `tamed` block. An empty map is what a save written before the
        // feature carries, and minting here rather than on first read is
        // what keeps `Game::dossier_report` a `&self` derivation.
        let attrs = if c.attributes.is_empty() {
            crate::attributes::mint(
                self.world.resource::<crate::attributes::AttributeDb>(),
                crate::attributes::body_seed(
                    self.world.resource::<crate::world::WorldMap>().seed(),
                    c.position.0,
                    c.position.1,
                    c.species.as_str(),
                    c.zone,
                ),
                &species.attributes,
            )
        } else {
            let mut restored = crate::components::Attributes::default();
            for (id, value) in &c.attributes {
                restored.set(id, *value);
            }
            restored
        };
        entity.insert(attrs);
```

The player's two sites are the same shape with `player_seed` and the class's authored map, reached off the restored `PlayerIdentity::class`. Note `spawn_player_from_save` is a free function with no `Game`, exactly like `spawn_player` — if the class or the db is not reachable there, do it in `Game::load`'s body after the world exists, the way the creature loop does, and say so in a comment.

- [ ] **Step 5: Run them.** `cargo test -p feral-processes-engine tests::attributes:: save::` — green.
- [ ] **Step 6: Run the two gates.** `cargo test --workspace` and `cargo test -p feral-processes-engine balance_sim`. The balance curves must be **untouched**; a moved curve here means something is reading an attribute, which is out of scope, so stop and find it rather than updating a number.
- [ ] **Step 7: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 7: The dossier header

**Files:**
- Modify: `crates/engine/src/attributes.rs` (`revision`, `checksum`)
- Create: `assets/descriptions/program_dossier.ron`
- Test: `crates/engine/src/attributes.rs`'s test module

**Interfaces:**
- Consumes: `derive::fold_bytes`, `derive::index`, `ATTRIBUTE_SALT` (Task 3).
- Produces: `attributes::revision(seed: u64) -> String`, `attributes::checksum(seed: u64) -> String`; the `"program.dossier"` description subject.

Three header rows, none of them an attribute and **none of them stored**. Two are pure functions of the same seed the attributes were minted from, on `handles::of`'s model — no pool, no storage, no content. The third is one authored sentence.

**No new descriptions accessor.** `DescriptionDb::paragraph(subject, condition, seed)` (`descriptions.rs:279`) already answers "one reading of this subject, derived from a seed", returns `None` when the bank has nothing for it, and joins opener/detail/coda dropping empties. The pool is authored in `openers:` and read through that call from Task 8. `descriptions.rs`' own doc comment names exactly this as its expansion seam.

- [ ] **Step 1: Write the failing tests.** `handles_are_pinned` is the model — a derived string that anything may print but nothing may depend on still gets pinned, because the alternative is a test that passes against a function returning a constant.

```rust
    /// Pinned, `handles_are_pinned`'s reason: a derived label with no pool
    /// and no storage is exactly the kind of function a refactor can
    /// quietly turn into a constant, and these assertions are the only
    /// thing that would notice.
    #[test]
    fn the_header_is_pinned() {
        let seed = body_seed(7, 3, -4, "crawler", 2);
        assert_eq!(revision(seed), revision(seed));
        assert_eq!(checksum(seed), checksum(seed));
        assert!(revision(seed).starts_with("rev "), "{}", revision(seed));
        assert_eq!(checksum(seed).len(), 8, "{}", checksum(seed));
        assert!(checksum(seed).starts_with("0x"), "{}", checksum(seed));
    }

    /// Two bodies do not share a header. Both halves are folded off the
    /// body's own seed with their own tag, `CaravanVisit`'s rule, so
    /// neither can shift the other.
    #[test]
    fn two_bodies_get_two_headers() {
        let mut revisions = std::collections::HashSet::new();
        let mut checksums = std::collections::HashSet::new();
        for n in 0..500u32 {
            let seed = body_seed(n, 0, 0, "crawler", 1);
            revisions.insert(revision(seed));
            checksums.insert(checksum(seed));
        }
        assert!(revisions.len() > 100, "only {} revisions in 500", revisions.len());
        assert!(checksums.len() > 450, "only {} checksums in 500", checksums.len());
    }
```

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine attributes::the_header_is_pinned` — expected: `cannot find function 'revision'`.
- [ ] **Step 3: Write the two functions.**

```rust
/// A build revision for one body — `"rev 4.17"`.
///
/// Derived, never stored, `handles::of`'s model: no pool, no storage and no
/// content, so a save written before this feature has one the moment it
/// loads. Each half folds the body's seed with **its own tag** rather than
/// reading two slices of one number, so neither can move the other.
pub fn revision(seed: u64) -> String {
    let major = 1 + derive::index(derive::fold_bytes(seed, b"rev.major"), 9);
    let minor = derive::index(derive::fold_bytes(seed, b"rev.minor"), 100);
    format!("rev {major}.{minor}")
}

/// A checksum for one body — `"0x3F9A21"`, six hex digits.
///
/// The high bits of its own fold, for `derive::index`'s reason: the low
/// bits of a fold are the ones the final multiply provably never disturbs,
/// so a mask over them would leave two neighbouring bodies rhyming.
pub fn checksum(seed: u64) -> String {
    let h = derive::fold_bytes(seed, b"checksum");
    format!("0x{:06X}", ((h >> 40) as u32) & 0x00FF_FFFF)
}
```

- [ ] **Step 4: Author the provenance pool.** `assets/descriptions/program_dossier.ron`, following a shipped file in that directory for its exact shape (`assets/descriptions/orphan.ron` is a good one to copy structurally). Subject `"program.dossier"`, one variant with `when: None`, and the sentences in `openers:` — six to ten of them, each one line of where a program came from, in the setting's voice and **saying nothing about what its numbers do**. The pool is drawn against the body's own seed, so a program's provenance is as stable as its attributes.
- [ ] **Step 5: Run them.** `cargo test -p feral-processes-engine attributes:: descriptions::` — green, and `assets::every_shipped_asset_file_loads_without_a_warning` proves the new description file parses.
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 8: `Game::dossier_report`

**Files:**
- Modify: `crates/engine/src/views.rs` (`DossierReport`, `AttributeRow`, beside `ManifestSubject` at ~2342)
- Modify: `crates/engine/src/game/inspection.rs` (`Game::dossier_report`, beside `Game::manifest` at 1912)
- Modify: `crates/engine/src/tuning.rs` (`MAX_ATTRIBUTE_ROWS`, beside `MAX_TOOL_ROWS` at 4905)
- Test: `crates/engine/src/tests/attributes.rs`

**Interfaces:**
- Consumes: everything above.
- Produces:

```rust
pub struct DossierReport {
    /// What the page's title row leads with — `Game::creature_label`'s
    /// answer, so the dossier and the manifest name the same body the same
    /// way.
    pub name: String,
    pub revision: String,
    pub checksum: String,
    /// One sentence of where this program came from, or `None` when
    /// `assets/descriptions/` has nothing for the subject — a mod's
    /// prerogative, and the page simply draws no row.
    pub provenance: Option<String>,
    pub rows: Vec<AttributeRow>,
}

pub struct AttributeRow {
    pub name: String,
    pub legacy: String,
    pub value: i32,
    pub short: String,
    pub meaning: String,
}
```

and `Game::dossier_report(&self, entity: Entity) -> Option<DossierReport>`.

`&self` and `Option`, mirroring `Game::manifest(&self, entity: Entity) -> Option<ManifestView>` exactly — the subject is `App::pending_manifest`, which is an `Option<Entity>` the manifest already pages through with ←/→, and `None` is a body that is gone. A row whose def the catalogue no longer defines is **skipped**, every `Memories` reader's rule, which is what makes a catalogue edited between sessions a supported thing.

- [ ] **Step 1: Write the failing tests.**

```rust
/// The page's whole derivation, on the player and on a wild body alike —
/// one call, read by app-core for nothing and by gui for everything.
#[test]
fn the_dossier_reports_every_attribute_with_both_its_names() {
    let mut game = Game::new(4248, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let wild = game.spawn_wild_creature_scaled(&species, 100, 100, 1.0, false).unwrap();
    for subject in [game.player_entity(), wild] {
        let report = game.dossier_report(subject).expect("a live body has a dossier");
        assert_eq!(report.rows.len(), 5);
        for row in &report.rows {
            assert!(!row.name.is_empty());
            assert!(!row.legacy.is_empty(), "{} lost its old-school name", row.name);
            assert!(!row.meaning.is_empty(), "{} has no prose", row.name);
        }
        assert!(report.revision.starts_with("rev "));
        assert!(report.checksum.starts_with("0x"));
    }
}

/// An empty catalogue is the pre-attribute game, held at the reader's end
/// too: the page opens, reports no rows and claims nothing.
#[test]
fn an_empty_catalogue_reports_a_dossier_with_no_rows() {
    let mut game = Game::new(4249, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(crate::attributes::AttributeDb::default());
    let report = game.dossier_report(game.player_entity()).unwrap();
    assert!(report.rows.is_empty());
    assert!(!report.revision.is_empty(), "the header is derived, not authored");
}

/// The catalogue is trimmed **before** the page's own cap, so a modded
/// catalogue cannot push the header off the end of a page with no scroll.
#[test]
fn the_dossier_is_trimmed_to_its_row_ceiling() {
    let over = crate::tuning::MAX_ATTRIBUTE_ROWS + 4;
    let dir = crate::tests::support::scratch_assets_dir("attributes_over_cap");
    std::fs::create_dir_all(&*dir).unwrap();
    for n in 0..over {
        // Zero-padded, so the sort `load_dir` performs is the numeric order
        // and the trim is observably the *tail* being dropped.
        std::fs::write(
            dir.join(format!("a{n:02}.ron")),
            format!(
                "(id: \"a{n:02}\", name: \"A{n:02}\", legacy: \"Luck\", \
                 short: \"a gloss long enough\", meaning: \"Fifteen words of \
                 prose about this attribute so the shipped census would be \
                 satisfied by it too.\", base: 50, spread: 10)\n"
            ),
        )
        .unwrap();
    }
    let (db, warnings) = crate::attributes::AttributeDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(db.iter().count(), over, "the fixture itself is over the cap");

    let mut game = Game::new(4252, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(db);
    let report = game.dossier_report(game.player_entity()).unwrap();
    assert_eq!(
        report.rows.len(),
        crate::tuning::MAX_ATTRIBUTE_ROWS,
        "a modded catalogue was drawn past the page's ceiling"
    );
    assert!(
        !report.checksum.is_empty(),
        "the header must survive the trim — it is what the trim protects"
    );
}

/// A body that is gone has no dossier, `Game::manifest`'s own answer.
#[test]
fn a_despawned_body_has_no_dossier() {
    let mut game = Game::new(4250, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.species_defs()[0].id.to_string();
    let wild = game.spawn_wild_creature_scaled(&species, 110, 110, 1.0, false).unwrap();
    game.world.despawn(wild);
    assert!(game.dossier_report(wild).is_none());
}
```

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine tests::attributes::the_dossier` — expected: no method `dossier_report`.
- [ ] **Step 3: Add the constant.** In `tuning.rs` beside `MAX_TOOL_ROWS`, in its voice:

```rust
/// How many attribute rows `Game::dossier_report` may return —
/// `MAX_TOOL_ROWS`' own reason: `Mode::Dossier` is a read-only row list
/// with no scroll, so this doubles as a layout constraint, asserted by
/// `the_tallest_dossier_fits_its_popup` and verified by mutation. The
/// catalogue is trimmed against this **before** the page builds its rows,
/// so a modded twentieth attribute costs the last rows rather than pushing
/// the header off the bottom in silence. Twice the shipped five, headroom
/// for a wider catalogue without reopening the page to an unbounded one.
pub const MAX_ATTRIBUTE_ROWS: usize = 10;
```

- [ ] **Step 4: Write the derivation.** In `game/inspection.rs`, beside `Game::manifest`. Read the body's `Attributes`, walk `AttributeDb::iter()` (sorted, and the trim is `.take(MAX_ATTRIBUTE_ROWS)` on that walk), skip an id the store has no value for and an entry the catalogue no longer defines, and build the header from the same seed the body was minted with — which means the seed has to be **re-derived here from the body's current facts**, and that is the one thing this function must not do.

  The body has walked since it spawned, so its `Position` is no longer the tile it was minted on. **The header therefore folds the stored attribute values**, not a re-derived body seed: `derive::fold` over the stored `(id, value)` pairs in `BTreeMap` order. That makes the revision and the checksum a *function of the numbers on the page*, which is what a checksum should be, and it is stable across a save/load for free because the values are. Write that reasoning into the doc comment — it is the non-obvious decision in this task, and a later "fix" re-deriving `body_seed` here would make a program's checksum change every time it took a step.

- [ ] **Step 5: Run them.** `cargo test -p feral-processes-engine tests::attributes::` — green.
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 9: `Mode::Dossier` and the key

**Files:**
- Modify: `crates/app-core/src/lib.rs` (the `Mode` enum, beside `Manifest` at 1542)
- Modify: `crates/app-core/src/app/inspection.rs` (`handle_manifest_key` at 257, and a new `handle_dossier_key`)
- Modify: `crates/app-core/src/app/input.rs` (the route, beside `Mode::CompanionMemories`'s at 261)
- Test: `crates/app-core/src/tests/` (a new module or the existing inspection tests)

**Interfaces:**
- Consumes: `Game::dossier_report` (Task 8).
- Produces: `Mode::Dossier`; `D` on the manifest opens it; `Esc` returns to `Mode::Manifest`.

**No app-core row count.** The spec's §8 says app-core reads the report "for its row count". It does not: the memories page is the precedent and it exposes nothing to app-core, because a `draw_popup` with no `Row::Item` has no scroll, no window and no selection, so there is no row count for app-core to own. The engine owns the trim (Task 8) and gui owns the drawing. The CLAUDE.md rule this seems to violate — "a read-only screen's row count is owned by app-core" — is about screens that *scroll*, and `Mode::CompanionMemories` is the standing proof.

**`D` is free.** `handle_manifest_key` consumes `w`, Left, Right and Esc and ends in `_ => return`. Uppercase is required because lowercase letters are row selectors everywhere in this game.

- [ ] **Step 1: Write the failing tests.**

```rust
/// `[D]` on a manifest opens the dossier for the body the sheet is showing,
/// and `Esc` comes back to it — not to the list the manifest was opened
/// from, which is `leave_manifest`'s job and stays its job.
/// Opens the player's own manifest without walking the menus to get
/// there — `tests/watch.rs:36`'s `open_manifest` helper, three lines
/// inline because this module needs it twice.
fn open_player_manifest(app: &mut App) -> Option<Entity> {
    let player = app.game.as_ref().unwrap().player_entity();
    app.pending_manifest = Some(player);
    app.manifest_origin = ManifestOrigin::Map;
    app.mode = Mode::Manifest;
    Some(player)
}

#[test]
fn d_opens_the_dossier_and_esc_returns_to_the_manifest() {
    let mut app = test_app(7301);
    let subject = open_player_manifest(&mut app);
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::Dossier);
    assert_eq!(app.pending_manifest, subject, "the subject must not move");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.pending_manifest, subject);
}

/// Lowercase `d` is not it. On the map `d` is a verb already, and the rule
/// across every screen in this game is that a lowercase letter picks a row.
#[test]
fn lowercase_d_does_nothing_on_a_manifest() {
    let mut app = test_app(7302);
    open_player_manifest(&mut app);
    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::Manifest);
}
```

`test_app(seed)` is `crates/app-core/src/tests/support.rs:65`; `app.game.as_ref().unwrap().player_entity()` is how the other modules reach the player. `ManifestOrigin::Map` is the origin that sends `Esc` from the *manifest* back to the map, which is what makes the dossier's own `Esc` observably different from the manifest's.

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-app-core dossier` — expected: no variant `Dossier`.
- [ ] **Step 3: Add the variant** with the doc comment the neighbours have — what it shows, what opens it, what leaves it, and that it is a second page about the manifest's own subject rather than a screen with a subject of its own:

```rust
    /// The dossier — the second page about whatever body `Mode::Manifest`
    /// is showing. Five attributes with their old-school names beside them,
    /// over a derived header. `App::pending_manifest` is the subject here
    /// too, deliberately: this is a page of the manifest, not a screen that
    /// tracks its own body, so paging with ←/→ and then opening `[D]`
    /// cannot show one program's sheet and another's attributes.
    ///
    /// **Nothing here reads a number for a mechanic.** See
    /// `attributes.rs`.
    ///
    /// A popup like every other list, so it does not join
    /// `needs_status_banner`.
    Dossier,
```

- [ ] **Step 4: Wire the two keys.** One arm in `handle_manifest_key` above the `step` match, and a `handle_dossier_key` that takes `Esc` back to `Mode::Manifest` and swallows nothing else. Route it in `input.rs` beside `Mode::CompanionMemories => self.handle_companion_memories_key(key)`. Check `input.rs`'s mode-transition pair list (~lines 63–80, the `keeps_highlight` set) and add the pair only if the manifest actually parks a highlight — read what that list is for before adding to it.
- [ ] **Step 5: Run them.** `cargo test -p feral-processes-app-core` — green. The app-core suite has a known-flaky `tests::creation` module that fails on an unmodified binary with a varying test name; if that is what fails, re-run that module rather than debugging it inside this task.
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 10: The page

**Files:**
- Create: `crates/gui/src/render/dossier.rs`, registered in `crates/gui/src/render/mod.rs`'s module list
- Modify: `crates/gui/src/render/mod.rs` (the `match app.mode` arm beside `Mode::CompanionMemories`'s at 1219; `ALL_MODES` at 1509, `114` → `115`)
- Test: `crates/gui/src/render/dossier.rs`'s own test module

**Interfaces:**
- Consumes: `Game::dossier_report`, `views::DossierReport`, `views::AttributeRow` (Task 8); `Mode::Dossier` (Task 9).
- Produces: `draw_dossier(game, subject: Option<Entity>, refusal, painter, m)`; `dossier_page_rows(report: &DossierReport, cols: usize) -> Vec<Row>`, where `cols` is the popup's own column budget and the only thing the wrap needs.

Three helpers the censuses want and that this task writes beside them: `shipped_attribute_defs()` (an `AttributeDb::load_dir` against `test_assets_dir()`), `longest_shipped_program_name()` (`party.rs:1420-1457` already builds one of these for the memories census — read it and follow it rather than inventing a second), and `longest_shipped_provenance()` (the longest `openers:` entry on the `"program.dossier"` subject). `popup_cols(PopupSize)` may not exist; if the popup's column budget is not already a function in `render/popup.rs`, pass the same figure the census measures against and say so in a comment rather than adding an abstraction for one caller.

`render/party.rs:103-200` is the template and should be followed closely: a thin `draw_*` that resolves the subject and hands off, plus a **pure** `*_page_rows` that takes view data rather than a `Game`. That split is not cosmetic — the height and width censuses have to measure the page at its worst case, and a worst case is something a fixture can state and a `Game` would have to be played into.

- [ ] **Step 1: Write the failing censuses.** Both of them, copied from `party.rs:1469` and `party.rs:1489` and pointed at the new page. These are the whole of the spec's §8 width-and-height budget, and the page has no scroll, so a row past the bottom and a line past the right edge are both **lost in silence**.

```rust
    /// The worst page the shipped game can build: `MAX_ATTRIBUTE_ROWS` rows
    /// of the widest shipped def, over a full header. Stated rather than
    /// played into, which is what `dossier_page_rows` taking view data is
    /// for.
    /// Built by hand, not played into: the census has to measure the page
    /// at a worst case the shipped game can reach but no save is likely to
    /// be in. `MAX_ATTRIBUTE_ROWS` rows, each carrying the longest name,
    /// legacy word and prose any shipped def uses, over a full header with
    /// the longest shipped provenance sentence.
    fn tallest_dossier() -> Vec<Row> {
        let widest = |pick: fn(&AttributeDef) -> &str| {
            shipped_attribute_defs()
                .iter()
                .map(|d| pick(d).to_string())
                .max_by_key(|s| s.chars().count())
                .expect("the shipped catalogue is not empty")
        };
        let row = views::AttributeRow {
            name: widest(|d| &d.name),
            legacy: widest(|d| &d.legacy),
            // The widest value the mint can produce from any shipped def,
            // and negative-signed nowhere — but measured at three digits,
            // because a modded `base` may reach them.
            value: 100,
            short: widest(|d| &d.short),
            meaning: widest(|d| &d.meaning),
        };
        let report = views::DossierReport {
            name: longest_shipped_program_name(),
            revision: "rev 9.99".to_string(),
            checksum: "0xFFFFFF".to_string(),
            provenance: Some(longest_shipped_provenance()),
            rows: vec![row; feral_processes_engine::tuning::MAX_ATTRIBUTE_ROWS],
        };
        dossier_page_rows(&report, popup_cols(PopupSize::Large))
    }

    /// **The page has no scroll.** `draw_popup` pages a `Row::Item` span
    /// and this page has none, so a row past the bottom is dropped in
    /// silence — `the_tallest_memory_page_fits_its_popup`'s trap exactly.
    /// Raising `MAX_ATTRIBUTE_ROWS` past what fits means giving the page a
    /// scroll first.
    ///
    /// Swept rather than measured at one window: `ui_metrics` clamps the
    /// font at both ends, so below the clamp the box keeps shrinking while
    /// the line height stops and the tightest window is the smallest one.
    #[test]
    fn the_tallest_dossier_fits_its_popup() {
        let rows = tallest_dossier().len();
        for h in (600..=2160).step_by(60) {
            let m = ui_metrics(h as f32);
            let cap = popup_max_rows(h as f32, PopupSize::Large, &m);
            assert!(
                rows + REFUSAL_MAX_LINES <= cap,
                "the dossier builds a {rows}-row page into a {cap}-row popup at {h}px"
            );
        }
    }

    /// The other axis, and the one nothing clamps at all: `draw_row` clips a
    /// row vertically and never horizontally, so a line past the right edge
    /// is simply lost. On this page the tail of a row is the number and the
    /// legacy name — which is most of what the row is read for.
    #[test]
    fn no_dossier_row_overflows_its_popup() {
        let rows = tallest_dossier();
        with_painter(|p| {
            let m = ui_metrics(900.0);
            // 0.88 is `PopupSize::Large`'s width fraction, against the
            // 1440x900 geometry `ui_metrics` is calibrated for.
            let room = 1440.0 * 0.88 - m.pad * 2.0;
            for row in &rows {
                let line = match row {
                    Row::Text(t) | Row::TextColored(t, _) => t,
                    _ => continue,
                };
                let drawn = p.measure_ui_advance(line, m.font_size);
                assert!(
                    drawn <= room,
                    "a dossier row overflows the page by {:.0}px \
                     ({drawn:.0} drawn into {room:.0} of room):\n{line}",
                    drawn - room
                );
            }
        });
    }
```

- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-gui dossier` — expected: no module `dossier`.
- [ ] **Step 3: Write the page.** `draw_dossier` resolves the subject through `game.dossier_report(subject)` and falls back to a `PopupSize::Small` "That program is gone." exactly as `draw_companion_memories` does. `dossier_page_rows` builds:
  - a `Row::TextColored(format!("{name} — dossier"), CYAN)` title;
  - the header: `rev`, checksum, and the provenance sentence **wrapped** through `text::wrap` (the engine's, which `render/popup.rs::wrap_text` is a call to — use that call, never a second wrap);
  - a blank row;
  - one row per attribute: the name, the legacy name in parentheses, and the number, with the `short` gloss after it;
  - the `meaning` prose wrapped underneath, if it fits the row budget — **measure first**: five attributes at three wrapped lines each is 15 rows before the header, and `MAX_ATTRIBUTE_ROWS` is 10. If the census fails, the page shows `short` per row and drops `meaning`, and `AttributeRow::meaning` stays in the report for the inspect page a later change adds. Decide this with the census, not by eye, and write the decision into the function's doc comment.
  - a trailing `Esc to go back`.
- [ ] **Step 4: Add the draw arm and grow `ALL_MODES`.** `Mode::Dossier => draw_dossier(game, app.pending_manifest, refusal, painter, m)` beside `Mode::CompanionMemories`'s. Then `ALL_MODES` from `[Mode; 114]` to `[Mode; 115]` with the variant added — **this is the step that makes the mode real.** A new `Mode` variant does not fail to compile anywhere: the draw match ends in a wildcard and `ALL_MODES` is hand-written, so without both edits the screen ships blank and `every_screen_draws_a_refusal_exactly_once` never looks at it. With them, that census is what proves the page draws its refusal exactly once.
- [ ] **Step 5: Run them.** `cargo test -p feral-processes-gui dossier every_screen_draws_a_refusal` — green.
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace --all-targets`, commit.**

---

### Task 11: The manual page and the gates

**Files:**
- Create: `assets/help/<nn>-attributes.md` (the numeric prefix is the ordering *and* the id a `[label](topic-id)` link points at)
- Modify: whichever shipped help page should link to it
- Test: the existing help censuses

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1: Read `assets/help/README.md` and one shipped page.** Five block rules and no more; the filename is the ordering and the id, so there is no front matter. Find the tests that hold help pages to the format (`rg 'assets/help' crates/`) and read what they assert before writing.
- [ ] **Step 2: Write the page.** What an attribute is, that each has an old-school name beside it so it can be read without knowing the setting's vocabulary, where to find them (`[D]` from the manifest), and — plainly — that **they do not do anything yet**. That last sentence is the one the spec's decision 2 is about: a manual that implies an effect is the same false claim as a gloss that does, and saying so straight is what keeps the page honest until the mechanics land. Link it from the manifest's own page with a `[label](topic-id)` row.
- [ ] **Step 3: Run the help censuses.** `cargo test -p feral-processes-engine help` and `cargo test -p feral-processes-gui help`.
- [ ] **Step 4: The full gates.** `cargo test --workspace`, then `cargo test -p feral-processes-engine balance_sim`, then `cargo clippy --workspace --all-targets` and `cargo fmt --check`. The balance curves must be untouched — that is the spec's test 9, and it is asserted by the existing curves staying green with no edit.
- [ ] **Step 5: Commit.**
- [ ] **Step 6: Report what is unverified.** The suite is green and nothing here has been on a screen: no agent in this repo can run `cargo run`. Say so in one line, and name the two things that need a human at the keyboard — whether the dossier's five rows read as characterising a program rather than as five numbers, and whether the header reads as flavour rather than as debug output.

---

## What this plan deliberately leaves out

- Any mechanic reading an attribute. The spec's decision 1, and the reason `Attributes` is its own component.
- The "what it does" prose line. Decision 2, and the reason Task 2's census forbids one.
- `Rapport (Charisma)`. Decision 8 — a sixth attribute is a file drop and Task 2's census row.
- Showing `Disposition`, which is hidden by its own design and stays hidden.
- Attributes on structures. The request was bodies and the player.
- A dossier row on the manifest itself. The manifest's status column holds 38.5 monospace cells and the widest shipped buff row already spends all but 3.8 of them; a sixth box there is its own change with its own layout census.
