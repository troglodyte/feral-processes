# Generated flavour prose for the Stack — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Stack an environment-and-lore prose layer — four surfaces
drawing from a shipped `.ron` fragment bank, composed deterministically from
the frame spec so a given cell of a given stack always reads the same way and
a different stack reads differently.

**Architecture:** A new `DescriptionDb` (`crates/engine/src/descriptions.rs`)
generalises the shipped `CrashLogDb`: pools keyed by a subject string plus
slot composition, loaded once at `Game::new`/`Game::load` and held as a bevy
`Resource`. Selection is a modulo of an FNV-1a fold continued off
`FrameSpec::rng_seed`, never `StdRng` and never `GameRng`, so nothing is
cached and the save format does not move. `Game` owns the mixing; no caller
ever constructs a seed. `crash_logs.rs` is absorbed and deleted.

**Tech Stack:** Rust 2024, bevy_ecs, `ron` + `serde` for assets, the
workspace's existing test harness (`crates/engine/src/tests/`), egui-backed
`Painter` for the gui.

**Source spec:** `docs/superpowers/specs/2026-08-10-stack-descriptions-design.md`
— read it before starting. This plan implements it; where they disagree the
spec is right and the plan is a bug.

## Global Constraints

- **No runtime LLM, no network, no thread, no async.** Fragments are authored
  at dev time and ship as `.ron`. Settled in the spec — do not revisit.
- **No cache and no `SAVE_FORMAT_VERSION` bump.** A description is a pure
  function of `FrameSpec` + the cell. Adding either is the signal something
  started reading run state it shouldn't.
- **Never `GameRng`, never `StdRng`.** Selection is `fold % pool.len()`.
  `StdRng`'s sequence is not stable across a `rand` upgrade; a silent
  reshuffle of every description on a dependency bump is the failure being
  designed out. Confirm with `rg 'GameRng|StdRng|DefaultHasher'
  crates/engine/src/descriptions.rs crates/engine/src/game/descriptions.rs` —
  the only matches allowed are the words appearing inside doc comments that
  explain why they are not used.
- **`DESCRIPTION_SALT` must differ from the three existing salts:**
  `LAIR_SALT` `0x1A19_B055` and `ORPHAN_SALT` `0xDEAD_C0DE`
  (`game/stack_features.rs:196, 422`), `FALL_SALT` `0xFA11_1E15`
  (`stack.rs:1165`). This plan uses `0xDE5C_21B3`. Those three are **not**
  migrated to `salted`.
- **Extend, never copy:** `remember_view`, `enter_frame`, `relative_bearing`,
  and the five spent-state predicates (`cache_unopened`, `seal_open`,
  `breakpoint_spent`, `orphan_present`, `lair_cleared`). Never a new
  `FrameMemory` record — the condition axis reads the predicates that exist.
- **Prose respects the standing no-occult-naming rule** — no daemon, demon,
  ghost, wraith or phantom anywhere in the bank. Voice is dry, technical,
  slightly elegiac, matching the shipped crash logs.
- **`{bearing}` is the only substitution token**, filled at the call site from
  `relative_bearing`. Never stored composed.
- **`CellKind::Rock` has no subject.** It is the default reading of a blocked
  corridor and the thing everything else is distinguished against.
- **`CLAUDE.md` and `AGENTS.md` are gitignored twins and are NOT in this
  worktree.** They live only at `/home/trog/code/feral-processes/`. Task 10
  edits `CLAUDE.md` there and `cp CLAUDE.md AGENTS.md`. Nothing about that
  edit shows up in this branch's diff.
- **Verification per task:** `cargo test -p feral-processes-engine
  descriptions` while iterating (~3s), `cargo test --workspace` as the gate,
  then `cargo clippy --workspace && cargo fmt`. `balance_sim` is irrelevant —
  nothing here touches a formula.

## File structure

| File | Responsibility |
|---|---|
| `crates/engine/src/stack.rs` | `FrameSpec::salted` — the one fold everything salts off |
| `crates/engine/src/descriptions.rs` **(new)** | `DescriptionDb`: schema, `load_dir`, variant resolution, slot selection, paragraph composition. Knows nothing about `Game` |
| `crates/engine/src/game/descriptions.rs` **(new)** | The `Game` half: `CellKind` → subject/condition, seed mixing, `{bearing}`, notability, the examine ray's entry point |
| `crates/engine/src/game/stack_view.rs` | `standing_on` draws from the bank; `remember_view` announce/silent split; the view-space examine ray (beside `view_cone`, which stays private to this file) |
| `crates/engine/src/game/stack.rs` | `enter_frame` mood line |
| `crates/engine/src/game/listen.rs` | `relative_bearing` widened to `pub(crate)`; rot branch repointed at the bank |
| `crates/engine/src/game/inspection.rs` | `find_target_in_direction` refuses underground outright |
| `crates/engine/src/game/lifecycle.rs` | `AssetDbs` / `load_asset_dbs` swap |
| `crates/engine/src/crash_logs.rs` | **deleted** |
| `assets/descriptions/*.ron` + `README.md` **(new)** | The bank and its schema doc, including the authoring prompt |
| `assets/crash_logs/` | **deleted** |
| `crates/app-core/src/lib.rs`, `app/inspection.rs` | `Mode::CellDescribe`, `pending_description`, the underground `x` branch |
| `crates/gui/src/render/stack.rs` | `draw_cell_describe` popup + the underfoot width proof |

---

### Task 1: `FrameSpec::salted`

**Files:**
- Modify: `crates/engine/src/stack.rs:294-307` (add after `rng_seed`)
- Test: `crates/engine/src/stack.rs` (the `mod tests` at the bottom of the file)

**Interfaces:**
- Consumes: `FrameSpec::rng_seed` (existing, `pub(crate)`).
- Produces: `pub(crate) fn salted(self, words: &[u64]) -> u64` on `FrameSpec`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/engine/src/stack.rs`'s existing `mod tests`:

```rust
    /// The whole point of continuing the fold rather than XOR-ing once: two
    /// cells of one frame, and the three lengths of one cell, have to
    /// diverge robustly rather than by luck.
    #[test]
    fn salting_the_frame_seed_diverges_on_every_word() {
        let spec = FrameSpec {
            world_seed: 7,
            entrance: (3, -4),
            depth: 2,
            frames: 4,
        };
        let seeds = [
            spec.salted(&[]),
            spec.salted(&[1]),
            spec.salted(&[2]),
            spec.salted(&[1, 0]),
            spec.salted(&[0, 1]),
            spec.salted(&[1, 1]),
        ];
        for (i, a) in seeds.iter().enumerate() {
            for (j, b) in seeds.iter().enumerate().skip(i + 1) {
                assert_ne!(a, b, "salted words {i} and {j} collided on {a}");
            }
        }
    }

    /// Salting is a continuation of `rng_seed`, so two frames that already
    /// differ still differ after any number of words.
    #[test]
    fn salting_keeps_two_frames_apart() {
        let a = FrameSpec {
            world_seed: 7,
            entrance: (3, -4),
            depth: 2,
            frames: 4,
        };
        let b = FrameSpec { depth: 3, ..a };
        assert_ne!(a.salted(&[9, 9, 9]), b.salted(&[9, 9, 9]));
    }

    /// Same inputs, same answer — across calls and, because the mix is
    /// fixed arithmetic rather than a hasher, across builds.
    #[test]
    fn salting_is_stable() {
        let spec = FrameSpec {
            world_seed: 7,
            entrance: (3, -4),
            depth: 2,
            frames: 4,
        };
        assert_eq!(spec.salted(&[4, 5]), spec.salted(&[4, 5]));
        assert_eq!(spec.salted(&[]), spec.rng_seed());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine salting`
Expected: FAIL — `no method named 'salted' found for struct 'FrameSpec'`.

- [ ] **Step 3: Implement `salted`**

Insert immediately after `rng_seed` in `impl FrameSpec` (`stack.rs:306`):

```rust
    /// Continues `rng_seed`'s FNV-1a fold with further words, so anything
    /// that must be a stable property of a *cell* of a stack salts off the
    /// one scheme rather than inventing a second that could collide with it.
    ///
    /// Each word is multiplied through the FNV prime rather than XOR-ed in
    /// once, so `[a, b]` and `[b, a]` diverge and two adjacent cells cannot
    /// rhyme — the same argument `rng_seed` makes about adjacent links.
    ///
    /// `LAIR_SALT`, `ORPHAN_SALT` and `FALL_SALT` are deliberately **not**
    /// migrated onto this: each answers one question per frame, a single XOR
    /// is sufficient there, and all three are pinned by tests.
    pub(crate) fn salted(self, words: &[u64]) -> u64 {
        let mut h = self.rng_seed();
        for &word in words {
            h ^= word;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine salting`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/stack.rs
git commit -m "feat(stack): continue the frame-seed fold with salt words"
```

---

### Task 2: `DescriptionDb` schema and loading

**Files:**
- Create: `crates/engine/src/descriptions.rs`
- Modify: `crates/engine/src/lib.rs` (module declaration, beside `pub mod crash_logs;` at line 7)
- Create: `crates/engine/src/tests/descriptions.rs`
- Modify: `crates/engine/src/tests/mod.rs` (add `mod descriptions;` — the list is alphabetical, so between `mod crafting;` and `mod easter_eggs;`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub struct DescriptionDef { pub subject: String, pub variants: Vec<DescriptionVariant> }`
  - `pub struct DescriptionVariant { pub when: Option<String>, pub underfoot: Vec<String>, pub sighted: Vec<String>, pub openers: Vec<String>, pub details: Vec<String>, pub codas: Vec<String> }`
  - `pub struct DescriptionDb` (a bevy `Resource`, `Default`)
  - `pub fn DescriptionDb::load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)>`
  - `pub fn DescriptionDb::subjects(&self) -> impl Iterator<Item = &str>`
  - `pub fn DescriptionDb::variant_count(&self, subject: &str) -> usize`

- [ ] **Step 1: Write the failing tests**

Create `crates/engine/src/tests/descriptions.rs`:

```rust
//! The description bank: loading it, resolving a variant, and composing the
//! three lengths of one cell's prose.
//!
//! Nothing here touches `resources::GameRng`, by construction — selection is
//! a fold of the frame spec. A test that needed a seeded `Game` to be stable
//! would be evidence the fold had been replaced by a draw.

use crate::descriptions::DescriptionDb;

/// A scratch bank directory holding `files` as `(filename, body)`. The
/// caller removes it. Mirrors `tests/listen.rs`'s `crash_log_dir`.
pub(crate) fn bank_dir(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "feral_descriptions_{tag}_{}_{}",
        std::process::id(),
        files.len()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    dir
}

const DOOR: &str = r#"(
    subject: "stack.door",
    variants: [
        (
            underfoot: ["A doorway"],
            sighted: ["A door stands {bearing}."],
            openers: ["A door.", "A doorway, still framed."],
            details: ["The frame is warm.", ""],
            codas: ["Nothing answers through it."],
        ),
    ],
)"#;

#[test]
fn a_malformed_description_file_is_skipped_with_a_warning() {
    let dir = bank_dir(
        "malformed",
        &[("door.ron", DOOR), ("broken.ron", "( subject: \"oops\" ")],
    );
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert_eq!(warnings.len(), 1, "warnings were {warnings:?}");
    assert!(
        warnings[0].contains("broken.ron"),
        "the warning must name the file: {}",
        warnings[0]
    );
    assert_eq!(db.subjects().collect::<Vec<_>>(), vec!["stack.door"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_non_ron_file_is_ignored_without_a_warning() {
    let dir = bank_dir("non_ron", &[("door.ron", DOOR), ("README.md", "# not ron")]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.subjects().count(), 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_empty_bank_directory_loads_clean() {
    let dir = bank_dir("empty", &[]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty());
    assert_eq!(db.subjects().count(), 0);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Two files may contribute to one subject, and the order their fragments
/// land in the pool has to be the file id — not `read_dir`'s. Without the
/// sort the same cell reads a different fragment between runs, which is the
/// one property the whole system exists to provide. `CrashLogDb::load_dir`
/// carries the same sort for the same reason.
#[test]
fn two_files_on_one_subject_merge_in_file_id_order() {
    let a = r#"(subject: "stack.door", variants: [(underfoot: ["from a"])])"#;
    let z = r#"(subject: "stack.door", variants: [(underfoot: ["from z"])])"#;
    // Written z-then-a so a directory order that happened to match creation
    // order would fail this.
    let dir = bank_dir("merge", &[("zebra.ron", z), ("alpha.ron", a)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    // One condition, both files' fragments inside it — additive, not
    // first-wins.
    assert_eq!(db.variant_count("stack.door"), 1);
    assert_eq!(db.underfoot("stack.door", None, 0), Some("from a"));
    assert_eq!(db.underfoot("stack.door", None, 1), Some("from z"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Two variants sharing a condition merge into one pool — within a single
/// file as well as across two — while a different condition stays its own
/// variant. First-match-wins here would make an author's second variant
/// dead content with nothing to say so.
#[test]
fn two_variants_on_one_condition_merge_into_one_pool() {
    let body = r#"(
        subject: "stack.door",
        variants: [
            (underfoot: ["one"]),
            (underfoot: ["two"]),
            (when: "opened", underfoot: ["open"]),
        ],
    )"#;
    let dir = bank_dir("merge_within", &[("door.ron", body)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.variant_count("stack.door"), 2, "one fallback, one condition");
    let reachable: std::collections::HashSet<_> =
        (0..8u64).filter_map(|s| db.underfoot("stack.door", None, s)).collect();
    assert_eq!(reachable.len(), 2, "both fallback fragments must be reachable");
    assert_eq!(db.underfoot("stack.door", Some("opened"), 0), Some("open"));
    std::fs::remove_dir_all(&dir).unwrap();
}
```

Add to `crates/engine/src/tests/mod.rs`, keeping the list alphabetical:

```rust
mod descriptions;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-engine descriptions`
Expected: FAIL to compile — `unresolved import 'crate::descriptions'`.

- [ ] **Step 3: Write the module**

Create `crates/engine/src/descriptions.rs`:

```rust
//! What a cell of a Stack frame has to say for itself.
//!
//! Content, like species and items — `assets/descriptions/*.ron`, one file
//! per subject, so adding prose is a file drop rather than a code change.
//! This is `crash_logs.rs` generalised and absorbed: pools keyed by a
//! **subject** instead of one flat unkeyed pool, plus slot composition, so
//! one system covers the rot, the doors and everything else rather than two
//! systems covering one cell kind each.
//!
//! **Which fragment a given cell reads is a property of the place** —
//! derived from the frame spec and the cell coordinates, never from
//! `resources::GameRng`. Two reasons, both already learned in
//! `crash_logs.rs`: a draw from the shared stream does not survive a
//! save/load, so the same door would say something different after a
//! reload; and drawing from it to pick a cosmetic string shifts every later
//! roll in the run. Selection is a modulo of an FNV-1a fold rather than an
//! `StdRng` for a third reason — `StdRng`'s output sequence is not
//! guaranteed stable across a `rand` upgrade, and a dependency bump that
//! silently reshuffled every description in the game is exactly the failure
//! worth designing out.
//!
//! This module knows nothing about `Game`. It takes a subject, a condition
//! and a seed; `game/descriptions.rs` owns translating a `CellKind` into the
//! first two and mixing the third.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::Deserialize;

/// One bank file: the subject it describes, and the variants it contributes.
#[derive(Clone, Debug, Deserialize)]
pub struct DescriptionDef {
    /// A dotted domain key — `"stack.door"`, `"stack.cache"`. A `String`
    /// following the `ItemId` precedent rather than an enum, and that is the
    /// whole expansion seam: `"biome.forest"` later is a file drop with no
    /// code change.
    pub subject: String,
    #[serde(default)]
    pub variants: Vec<DescriptionVariant>,
}

/// One reading of a subject, under one condition.
///
/// Three *lengths* of the same content, because they land in three places
/// with different room. They are not truncations of each other: each is
/// authored for where it goes.
#[derive(Clone, Debug, Deserialize)]
pub struct DescriptionVariant {
    /// `None` is the fallback, used when no other variant on this subject
    /// matches.
    ///
    /// Two variants sharing a `when` — in one file or across two — have
    /// their pools concatenated rather than racing, so a mod adds fragments
    /// to a shipped door instead of replacing it and nothing an author
    /// writes goes silently dead. See `DescriptionVariant::absorb`.
    #[serde(default)]
    pub when: Option<String>,
    /// The descriptive clause of the `standing_on` row. Bounded in length:
    /// that row is centred, pixel-measured and **unwrapped**. No `{bearing}`
    /// — you are standing on it.
    #[serde(default)]
    pub underfoot: Vec<String>,
    /// One log line, for a cell coming into view. The log pane draws exactly
    /// one row per line with no wrapping, so this is one sentence by
    /// construction rather than a truncated paragraph. May use `{bearing}`.
    #[serde(default)]
    pub sighted: Vec<String>,
    /// The examine paragraph's first sentence.
    #[serde(default)]
    pub openers: Vec<String>,
    /// Its second. `""` is legal and lets a draw legitimately come out
    /// shorter.
    #[serde(default)]
    pub details: Vec<String>,
    /// Its third. `""` is legal, as in `details`.
    #[serde(default)]
    pub codas: Vec<String>,
}

impl DescriptionVariant {
    /// Concatenates `other`'s pools onto this one's, slot by slot.
    ///
    /// Two files describing one subject under one condition are **additive**
    /// — a mod adds fragments to the shipped door rather than replacing it,
    /// and neither file's prose goes silently dead. First-match-wins would
    /// make the loser dead content with nothing on screen to say so, which
    /// is the failure `crash_logs`' flat pool never had.
    ///
    /// This is also what makes `load_dir`'s sort load-bearing rather than
    /// cosmetic: the pool a cell indexes into has to be in the same order
    /// every run, and concatenation order is that order.
    fn absorb(&mut self, other: DescriptionVariant) {
        self.underfoot.extend(other.underfoot);
        self.sighted.extend(other.sighted);
        self.openers.extend(other.openers);
        self.details.extend(other.details);
        self.codas.extend(other.codas);
    }
}

/// Which pool a draw comes from.
///
/// Carries two fold words rather than one — the *length* and the *slot
/// within it* — so the three lengths of one cell diverge, and so do the
/// three sentences of one paragraph. Folding a single tag would work today
/// and would quietly stop working the moment a fourth slot was added to an
/// existing length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Underfoot,
    Sighted,
    Opener,
    Detail,
    Coda,
}

impl Slot {
    fn tags(self) -> [u64; 2] {
        match self {
            Slot::Underfoot => [0, 0],
            Slot::Sighted => [1, 0],
            Slot::Opener => [2, 0],
            Slot::Detail => [2, 1],
            Slot::Coda => [2, 2],
        }
    }
}

/// Every shipped and modded description, keyed by subject.
///
/// A `BTreeMap` rather than a `HashMap` so `subjects()` is ordered, which
/// makes the census test's failure message readable and a warning list
/// reproducible.
#[derive(Resource, Default)]
pub struct DescriptionDb {
    subjects: BTreeMap<String, Vec<DescriptionVariant>>,
}

impl DescriptionDb {
    /// Loads every `*.ron` bank file in `dir`. A malformed file is skipped
    /// with a returned warning rather than aborting the load, same as
    /// `AbilityDb::load_dir` and `CrashLogDb::load_dir`.
    ///
    /// **Pools are filled in `(subject, file id)` order, not directory
    /// order.** `std::fs::read_dir` returns entries in no defined order, so
    /// without the sort the same cell would read a different fragment
    /// between runs and across a reload — the whole property this module
    /// exists to provide, lost to something no test of a single-file bank
    /// would see. The `assembler_system` position sort and
    /// `CrashLogDb::load_dir` both exist against the same class of bug.
    ///
    /// Variants sharing a condition are merged rather than kept side by
    /// side, so a subject holds exactly one variant per condition and
    /// `variant` never has to choose between two candidates.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut defs: Vec<(String, DescriptionDef)> = Vec::new();
        let mut warnings = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let file_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<DescriptionDef>(&text) {
                Ok(def) => defs.push((file_id, def)),
                Err(e) => warnings.push(format!("skipped invalid description file {path:?}: {e}")),
            }
        }
        defs.sort_by(|(a_id, a), (b_id, b)| (&a.subject, a_id).cmp(&(&b.subject, b_id)));

        let mut subjects: BTreeMap<String, Vec<DescriptionVariant>> = BTreeMap::new();
        for (_, def) in defs {
            let variants = subjects.entry(def.subject).or_default();
            for incoming in def.variants {
                match variants.iter_mut().find(|v| v.when == incoming.when) {
                    Some(existing) => existing.absorb(incoming),
                    None => variants.push(incoming),
                }
            }
        }
        Ok((DescriptionDb { subjects }, warnings))
    }

    /// Every loaded subject, in key order. For the census test and for
    /// reporting; nothing in play iterates the bank.
    pub fn subjects(&self) -> impl Iterator<Item = &str> {
        self.subjects.keys().map(String::as_str)
    }

    /// How many distinct conditions `subject` carries — one per `when`,
    /// including the fallback, since `load_dir` merges duplicates.
    pub fn variant_count(&self, subject: &str) -> usize {
        self.subjects.get(subject).map_or(0, Vec::len)
    }

    /// The variant of `subject` matching `condition`, falling back to the
    /// `when: None` one.
    ///
    /// A missing condition falls back rather than returning nothing, so
    /// authoring a new spent state is additive: the bank keeps answering
    /// with the general reading until someone writes the specific one.
    fn variant(&self, subject: &str, condition: Option<&str>) -> Option<&DescriptionVariant> {
        let variants = self.subjects.get(subject)?;
        condition
            .and_then(|c| variants.iter().find(|v| v.when.as_deref() == Some(c)))
            .or_else(|| variants.iter().find(|v| v.when.is_none()))
    }

    /// One line for the row under the first-person view, or `None` when the
    /// bank has nothing for this subject — which is a mod's prerogative and
    /// leaves the caller free to fall back to its own literal.
    pub(crate) fn underfoot(
        &self,
        subject: &str,
        condition: Option<&str>,
        seed: u64,
    ) -> Option<&str> {
        let v = self.variant(subject, condition)?;
        pick(&v.underfoot, fold(seed, Slot::Underfoot))
    }

    /// One log line for a cell coming into view. May contain `{bearing}`;
    /// the caller fills it.
    pub(crate) fn sighted(&self, subject: &str, condition: Option<&str>, seed: u64) -> Option<&str> {
        let v = self.variant(subject, condition)?;
        pick(&v.sighted, fold(seed, Slot::Sighted))
    }

    /// The examine paragraph — opener, detail and coda joined with a single
    /// space, empty fragments dropped.
    ///
    /// `None` when there is no opener: a paragraph that is only a detail is
    /// not a reading of anything, and the caller has a corridor fallback for
    /// exactly that case. May contain `{bearing}`.
    pub(crate) fn paragraph(
        &self,
        subject: &str,
        condition: Option<&str>,
        seed: u64,
    ) -> Option<String> {
        let v = self.variant(subject, condition)?;
        let opener = pick(&v.openers, fold(seed, Slot::Opener))?;
        let detail = pick(&v.details, fold(seed, Slot::Detail)).unwrap_or_default();
        let coda = pick(&v.codas, fold(seed, Slot::Coda)).unwrap_or_default();
        Some(
            [opener, detail, coda]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

/// Continues the caller's fold with the slot's own two words, so the three
/// lengths of one cell — and the three sentences of one paragraph — do not
/// all land on index 0 of their pools together.
///
/// Deliberately the same FNV-1a step `FrameSpec::salted` uses. Two schemes
/// mixing the same seed differently is how a description ends up stable in
/// one length and not in another.
fn fold(seed: u64, slot: Slot) -> u64 {
    let mut h = seed;
    for word in slot.tags() {
        h ^= word;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Indexes `pool` by `seed`, or `None` when the pool is empty — which is
/// legal at every slot and is how a two-sentence paragraph is authored.
fn pick(pool: &[String], seed: u64) -> Option<&str> {
    if pool.is_empty() {
        return None;
    }
    Some(&pool[(seed % pool.len() as u64) as usize])
}
```

Add to `crates/engine/src/lib.rs`, beside the other content modules (the list
is alphabetical, so `pub mod descriptions;` goes after `pub mod crash_logs;`):

```rust
pub mod descriptions;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine descriptions`
Expected: 5 passed.

- [ ] **Step 5: Verify nothing reads the RNG**

Run: `rg GameRng crates/engine/src/descriptions.rs`
Expected: no output.

- [ ] **Step 6: Commit**

```bash
git add crates/engine/src/descriptions.rs crates/engine/src/lib.rs \
        crates/engine/src/tests/descriptions.rs crates/engine/src/tests/mod.rs
git commit -m "feat(descriptions): load a subject-keyed bank of prose fragments"
```

---

### Task 3: Selection, variants and composition

**Files:**
- Modify: `crates/engine/src/tests/descriptions.rs` (append)

**Interfaces:**
- Consumes: `DescriptionDb::{underfoot, sighted, paragraph, variant_count}` from Task 2.
- Produces: nothing new — this task proves Task 2's selection contract before
  anything depends on it.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/descriptions.rs`:

```rust
/// A bank with enough fragments per slot that a differing seed can actually
/// show, and with a condition variant to resolve against.
const CACHE: &str = r#"(
    subject: "stack.cache",
    variants: [
        (
            underfoot: ["A sealed casing", "A casing, still shut"],
            sighted: ["A casing sits {bearing}.", "Something is stowed {bearing}."],
            openers: ["A cache.", "A stowed casing.", "A casing in the alcove."],
            details: ["The seal is intact.", "", "Its label has rotted off."],
            codas: ["", "Nobody came back for it.", "The lock still holds."],
        ),
        (
            when: "spent",
            underfoot: ["An empty casing"],
            sighted: ["An emptied casing lies {bearing}."],
            openers: ["An emptied casing."],
            details: ["You took what was in it."],
            codas: [""],
        ),
    ],
)"#;

fn cache_bank(tag: &str) -> (DescriptionDb, std::path::PathBuf) {
    let dir = bank_dir(tag, &[("cache.ron", CACHE)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    (db, dir)
}

#[test]
fn a_condition_resolves_to_its_own_variant() {
    let (db, dir) = cache_bank("condition");
    assert_eq!(db.underfoot("stack.cache", Some("spent"), 0), Some("An empty casing"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An unauthored condition falls back rather than going silent, so writing a
/// new spent state is additive.
#[test]
fn an_unmatched_condition_falls_back() {
    let (db, dir) = cache_bank("fallback");
    let general = db.underfoot("stack.cache", None, 0);
    assert_eq!(db.underfoot("stack.cache", Some("scorched"), 0), general);
    assert!(general.is_some());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_unknown_subject_reads_nothing() {
    let (db, dir) = cache_bank("unknown");
    assert_eq!(db.underfoot("stack.nowhere", None, 0), None);
    assert_eq!(db.sighted("stack.nowhere", None, 0), None);
    assert_eq!(db.paragraph("stack.nowhere", None, 0), None);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The same seed reads the same way, every time. This is the test that
/// fails the day someone reaches for a draw instead of a fold.
#[test]
fn the_same_seed_reads_the_same_description_twice() {
    let (db, dir) = cache_bank("stable");
    for seed in [0u64, 1, 17, u64::MAX] {
        assert_eq!(db.underfoot("stack.cache", None, seed), db.underfoot("stack.cache", None, seed));
        assert_eq!(db.paragraph("stack.cache", None, seed), db.paragraph("stack.cache", None, seed));
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Different seeds have to actually reach different fragments — the whole
/// point. Asserting over a sweep rather than one pair, because any single
/// pair can legitimately collide on a three-deep pool.
#[test]
fn different_seeds_reach_different_fragments() {
    let (db, dir) = cache_bank("varied");
    let paragraphs: std::collections::HashSet<_> =
        (0..64u64).filter_map(|s| db.paragraph("stack.cache", None, s)).collect();
    assert!(
        paragraphs.len() >= 4,
        "64 seeds produced only {} distinct paragraphs",
        paragraphs.len()
    );
    let underfoot: std::collections::HashSet<_> =
        (0..64u64).filter_map(|s| db.underfoot("stack.cache", None, s)).collect();
    assert_eq!(underfoot.len(), 2, "both underfoot fragments should be reachable");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The three lengths of one cell are independent draws. If they folded the
/// same seed they would move in lockstep, and the paragraph would only ever
/// pair opener 0 with underfoot 0.
#[test]
fn the_three_lengths_of_one_cell_do_not_move_in_lockstep() {
    let (db, dir) = cache_bank("lockstep");
    let pairs: std::collections::HashSet<_> = (0..64u64)
        .map(|s| (db.underfoot("stack.cache", None, s), db.sighted("stack.cache", None, s)))
        .collect();
    assert!(
        pairs.len() > 2,
        "underfoot and sighted moved together: {} combinations over 64 seeds",
        pairs.len()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Empty fragments are how a shorter paragraph is authored — they drop out
/// rather than leaving a double space or a dangling sentence.
#[test]
fn empty_slots_compose_into_a_shorter_paragraph() {
    let (db, dir) = cache_bank("short");
    let all: Vec<_> = (0..64u64).filter_map(|s| db.paragraph("stack.cache", None, s)).collect();
    assert!(
        all.iter().any(|p| p.split_whitespace().count() < 8),
        "no seed produced a short paragraph: {all:?}"
    );
    for p in &all {
        assert!(!p.contains("  "), "double space in {p:?}");
        assert!(!p.starts_with(' ') && !p.ends_with(' '), "stray edge space in {p:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A subject with no openers has no paragraph, rather than one made of a
/// detail. The corridor fallback covers that case at the call site.
#[test]
fn a_subject_with_no_opener_has_no_paragraph() {
    let body = r#"(subject: "stack.floor", variants: [(details: ["Just corridor."])])"#;
    let dir = bank_dir("no_opener", &[("floor.ron", body)]);
    let (db, _) = DescriptionDb::load_dir(&dir).unwrap();
    assert_eq!(db.paragraph("stack.floor", None, 0), None);
    std::fs::remove_dir_all(&dir).unwrap();
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p feral-processes-engine descriptions`
Expected: all pass — Task 2's implementation already satisfies these. **If any
fail, the bug is in Task 2's module, not in these tests.** Fix
`descriptions.rs` and re-run; do not weaken an assertion to match the code.

- [ ] **Step 3: Commit**

```bash
git add crates/engine/src/tests/descriptions.rs
git commit -m "test(descriptions): pin variant fallback, determinism and composition"
```

---

### Task 4: The shipped bank, and wiring it into `Game`

**Files:**
- Create: `assets/descriptions/README.md`
- Create: `assets/descriptions/{floor,door,sealed_door,cache,lair,orphan,breakpoint,link_up,link_down,fault,corruption,frame_arrival}.ron` (12 files)
- Modify: `crates/engine/src/game/lifecycle.rs` — `AssetDbs` (line 1021), `load_asset_dbs` (line 1040), and both `insert_resource` blocks (lines 57 and 219)
- Modify: `crates/engine/src/tests/support.rs:283-300` — the `copy_shipped_assets` directory list
- Modify: `crates/engine/src/tests/descriptions.rs` (append the census test)

**Interfaces:**
- Consumes: `DescriptionDb::load_dir` (Task 2).
- Produces: a `DescriptionDb` resource in every `Game`, reachable as
  `game.world.resource::<DescriptionDb>()`. The shipped subject list:
  `stack.floor`, `stack.door`, `stack.sealed_door`, `stack.cache`,
  `stack.lair`, `stack.orphan`, `stack.breakpoint`, `stack.link_up`,
  `stack.link_down`, `stack.fault`, `stack.corruption`,
  `stack.frame.arrival`.

**Condition vocabulary** (used by Task 5 and asserted by the census):

| Subject | Fallback means | Conditions |
|---|---|---|
| `stack.cache` | unopened | `spent` |
| `stack.sealed_door` | still sealed | `opened` |
| `stack.breakpoint` | unused | `spent` |
| `stack.orphan` | still there | `spent` |
| `stack.lair` | guardian alive | `cleared` |
| `stack.link_up` | a link further up | `surface` (depth 1 — the way out) |
| `stack.frame.arrival` | ordinary descent | `shallow`, `bottom`, `traced`, `hunted` |
| the rest | the only reading | none |

- [ ] **Step 1: Write the failing census test**

Append to `crates/engine/src/tests/descriptions.rs`:

```rust
/// Every subject the engine will ask for, and every condition it will ask
/// for it under. A content edit that empties a pool fails here instead of
/// shipping silence at a cell nobody happened to walk onto during testing.
/// Same shape as `every_biome_a_stack_link_can_open_in_fields_a_boss`.
const SHIPPED: &[(&str, &[&str])] = &[
    ("stack.floor", &[]),
    ("stack.door", &[]),
    ("stack.sealed_door", &["opened"]),
    ("stack.cache", &["spent"]),
    ("stack.lair", &["cleared"]),
    ("stack.orphan", &["spent"]),
    ("stack.breakpoint", &["spent"]),
    ("stack.link_up", &["surface"]),
    ("stack.link_down", &[]),
    ("stack.fault", &[]),
    ("stack.corruption", &[]),
    ("stack.frame.arrival", &["shallow", "bottom", "traced", "hunted"]),
];

#[test]
fn every_describable_cell_kind_has_a_shipped_bank_entry() {
    let dir = crate::tests::support::test_assets_dir().join("descriptions");
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "the shipped bank warned: {warnings:?}");

    for (subject, conditions) in SHIPPED {
        // Every subject answers at its fallback, in all three lengths...
        assert!(
            db.underfoot(subject, None, 0).is_some() || *subject == "stack.frame.arrival",
            "{subject} has no underfoot line"
        );
        assert!(db.sighted(subject, None, 0).is_some(), "{subject} has no sighted line");
        assert!(db.paragraph(subject, None, 0).is_some(), "{subject} has no paragraph");
        // ...and every authored condition resolves to a variant of its own
        // rather than silently falling back, which would make the condition
        // dead content nobody could see was dead.
        for condition in *conditions {
            let general = db.paragraph(subject, None, 0);
            assert_ne!(
                db.paragraph(subject, Some(condition), 0),
                general,
                "{subject}'s {condition:?} variant is missing and fell back"
            );
        }
    }
}

/// The `standing_on` row is centred and **unwrapped** — nothing clips it, so
/// an over-long fragment runs off the pane. `crates/gui`'s
/// `the_longest_underfoot_line_fits_the_stack_pane` proves this budget in
/// pixels at the narrowest supported window; this one holds the bank to it.
#[test]
fn every_shipped_underfoot_line_fits_the_standing_on_row() {
    let dir = crate::tests::support::test_assets_dir().join("descriptions");
    let (db, _) = DescriptionDb::load_dir(&dir).unwrap();
    // The longest key-prompt suffix any arm appends — "  — moving on costs".
    const LONGEST_SUFFIX: usize = 19;
    for (subject, conditions) in SHIPPED {
        for condition in std::iter::once(None).chain(conditions.iter().map(|c| Some(*c))) {
            for seed in 0..64u64 {
                let Some(line) = db.underfoot(subject, condition, seed) else {
                    continue;
                };
                assert!(
                    line.chars().count() + LONGEST_SUFFIX <= crate::MAX_UNDERFOOT_LINE,
                    "{subject} {condition:?} underfoot is {} chars: {line:?}",
                    line.chars().count()
                );
                assert!(
                    !line.contains("{bearing}"),
                    "{subject} {condition:?} uses {{bearing}} underfoot — you are standing on it"
                );
            }
        }
    }
}

/// The standing no-occult-naming rule, over the whole shipped bank.
#[test]
fn the_shipped_bank_uses_no_occult_naming() {
    let dir = crate::tests::support::test_assets_dir().join("descriptions");
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("ron") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap().to_lowercase();
        for word in ["daemon", "demon", "ghost", "wraith", "phantom"] {
            assert!(!text.contains(word), "{path:?} uses {word:?}");
        }
    }
}
```

`MAX_UNDERFOOT_LINE` is declared in Task 6; for this task, add it now to
`crates/engine/src/lib.rs` beside the other public constants:

```rust
/// How many characters the `standing_on` row will take, descriptive clause
/// and key-prompt suffix together.
///
/// That row is centred, drawn at `Metrics::font_size` and **unwrapped** —
/// nothing clips it, so an over-long line runs off the pane rather than
/// eliding. 48 leaves headroom over the longest literal the row carried
/// before the bank existed ("Rotten substrate  — moving on costs", 35) while
/// still keeping the clause a phrase rather than a sentence.
///
/// Proved in pixels by `crates/gui`'s
/// `the_longest_underfoot_line_fits_the_stack_pane`, at the narrowest
/// window size the UI supports. A number asserted in one place and repeated
/// in a doc comment somewhere else is how this measurement rots.
pub const MAX_UNDERFOOT_LINE: usize = 48;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p feral-processes-engine every_describable`
Expected: FAIL — the `assets/descriptions` directory does not exist
(`No such file or directory`).

- [ ] **Step 3: Write the bank**

Create `assets/descriptions/floor.ron`:

```ron
(
    subject: "stack.floor",
    variants: [
        (
            underfoot: ["Bare corridor"],
            sighted: [
                "The corridor runs on {bearing}.",
                "More of the same {bearing}.",
            ],
            openers: [
                "Corridor.",
                "Bare floor, laid straight.",
                "More corridor, unremarkable.",
                "The substrate here is intact.",
            ],
            details: [
                "",
                "The walls carry the same faint hum as everything else down here.",
                "Something walked this stretch often enough to wear it smooth.",
                "Whatever addressing scheme laid this out has long since been forgotten.",
            ],
            codas: [
                "",
                "There is nothing here.",
                "It goes on.",
                "Nothing is listening.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/door.ron`:

```ron
(
    subject: "stack.door",
    variants: [
        (
            underfoot: ["A doorway", "Standing in the frame"],
            sighted: [
                "A door stands shut {bearing}.",
                "There is a door {bearing}.",
                "Something closed off {bearing}.",
            ],
            openers: [
                "A door.",
                "A doorway, still framed.",
                "A door, shut and holding.",
                "A partition with a door in it.",
            ],
            details: [
                "",
                "The bottom-right corner is phase-shifting, leaking bits and sparking.",
                "The frame is warm to stand near.",
                "Its edges do not quite agree on where they are.",
                "The panel is scored where something forced it once already.",
            ],
            codas: [
                "",
                "There is no telling what is on the other side.",
                "Nothing answers through it.",
                "It has been shut a long time.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/sealed_door.ron`:

```ron
(
    subject: "stack.sealed_door",
    variants: [
        (
            underfoot: ["A heavy seal"],
            sighted: [
                "A sealed partition blocks the way {bearing}.",
                "Something heavy is shut {bearing}.",
                "A seal holds {bearing}.",
            ],
            openers: [
                "A sealed partition.",
                "A seal, set into the wall.",
                "Something built to stay shut.",
            ],
            details: [
                "",
                "It is heavier than the frame around it, and older.",
                "No lock, no reader, no way to ask it politely.",
                "The substrate has grown over the join and been forced apart again.",
            ],
            codas: [
                "",
                "It will move if you put your shoulder into it, and it will be heard.",
                "Whatever it was closed against is still on the other side.",
            ],
        ),
        (
            when: "opened",
            underfoot: ["A forced seal"],
            sighted: [
                "The seal you forced stands open {bearing}.",
            ],
            openers: [
                "The seal, standing open.",
                "A forced partition.",
            ],
            details: [
                "",
                "You put it there — the join is bright where the growth tore.",
            ],
            codas: [
                "",
                "It will not close again.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/cache.ron`:

```ron
(
    subject: "stack.cache",
    variants: [
        (
            underfoot: ["A sealed casing", "A casing, still shut"],
            sighted: [
                "A casing sits {bearing}, unopened.",
                "Something is stowed {bearing}.",
                "There is a casing {bearing}.",
            ],
            openers: [
                "A stowed casing.",
                "A cache, tucked into the alcove.",
                "Someone left a casing here.",
            ],
            details: [
                "",
                "Its label rotted off a long time ago.",
                "The seal is intact, which down here is worth remarking on.",
                "It is stacked square against the wall, the way a thing is put down by someone intending to come back.",
            ],
            codas: [
                "",
                "Nobody came back for it.",
                "It has been waiting.",
            ],
        ),
        (
            when: "spent",
            underfoot: ["An empty casing"],
            sighted: [
                "The casing you emptied lies {bearing}.",
            ],
            openers: [
                "An emptied casing.",
                "The casing, open and light.",
            ],
            details: [
                "",
                "You took what was in it.",
            ],
            codas: [
                "",
                "It is an alcove now, and nothing else.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/lair.ron`:

```ron
(
    subject: "stack.lair",
    variants: [
        (
            underfoot: ["The lair"],
            sighted: [
                "The corridor opens out {bearing}, and something is using the space.",
                "A held room {bearing}.",
            ],
            openers: [
                "The room at the bottom.",
                "A chamber, and something holding it.",
                "The deepest room this stack has.",
            ],
            details: [
                "",
                "The substrate here has been kept clear, which nothing down here does by accident.",
                "Everything the stack still runs, it runs from in here.",
                "The hum is louder, and it is not the walls.",
            ],
            codas: [
                "",
                "Whatever holds this has not moved in a long time.",
                "It knows the shape of the room better than you do.",
            ],
        ),
        (
            when: "cleared",
            underfoot: ["The lair, and nothing left holding it"],
            sighted: [
                "The cleared room lies open {bearing}.",
            ],
            openers: [
                "The room at the bottom, emptied.",
                "The chamber, and nothing in it now.",
            ],
            details: [
                "",
                "The clear ground is already starting to rot back over.",
            ],
            codas: [
                "",
                "The hum here is only the walls again.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/orphan.ron`:

```ron
(
    subject: "stack.orphan",
    variants: [
        (
            underfoot: ["An orphaned process"],
            sighted: [
                "Something is still running {bearing}.",
                "A process idles {bearing}, with nothing to serve.",
            ],
            openers: [
                "A process, still running.",
                "Something left going down here.",
                "An orphan, at the end of the corridor.",
            ],
            details: [
                "",
                "Its parent went down with the rest of this frame and it has not noticed.",
                "It is polling an address that stopped answering years ago.",
                "It cycles, waits, and cycles again.",
            ],
            codas: [
                "",
                "It would come with you.",
                "There is nothing left for it to do here.",
            ],
        ),
        (
            when: "spent",
            underfoot: [""],
            sighted: [
                "The dead end you emptied lies {bearing}.",
            ],
            openers: [
                "The end of the corridor.",
                "An alcove with nothing running in it.",
            ],
            details: [
                "",
                "Whatever was cycling here left with you.",
            ],
            codas: [
                "",
            ],
        ),
    ],
)
```

Create `assets/descriptions/breakpoint.ron`:

```ron
(
    subject: "stack.breakpoint",
    variants: [
        (
            underfoot: ["An exposed debug port"],
            sighted: [
                "A debug port stands open {bearing}.",
                "Something is jacked into the wall {bearing}.",
            ],
            openers: [
                "An exposed debug port.",
                "A maintenance jack, left open.",
                "A port, still live.",
            ],
            details: [
                "",
                "It would hand over the whole frame at a touch.",
                "Someone opened this to read the stack and never closed it again.",
                "The contacts are bright — this is the least corroded thing in the corridor.",
            ],
            codas: [
                "",
                "Whatever is watching this stack would hear you use it.",
                "It is the loudest thing you could do down here.",
            ],
        ),
        (
            when: "spent",
            underfoot: ["A burnt-out debug port"],
            sighted: [
                "The port you burnt out hangs open {bearing}.",
            ],
            openers: [
                "A burnt-out debug port.",
                "The port, dead now.",
            ],
            details: [
                "",
                "You took the whole frame off it and it did not survive the read.",
            ],
            codas: [
                "",
                "It will not answer twice.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/link_up.ron`:

```ron
(
    subject: "stack.link_up",
    variants: [
        (
            underfoot: ["A link leads up"],
            sighted: [
                "A link runs back up {bearing}.",
                "The way up is {bearing}.",
            ],
            openers: [
                "A link, running up.",
                "The way back into the frame above.",
            ],
            details: [
                "",
                "It is the way you came in.",
                "The shaft is clean where everything around it is not.",
            ],
            codas: [
                "",
                "It goes where you have already been.",
            ],
        ),
        (
            when: "surface",
            underfoot: ["The link out"],
            sighted: [
                "The link out is {bearing}.",
            ],
            openers: [
                "The link out.",
                "The way back up to the grid.",
            ],
            details: [
                "",
                "Open sky at the top of it, or what passes for sky.",
            ],
            codas: [
                "",
                "Everything you are carrying goes up with you.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/link_down.ron`:

```ron
(
    subject: "stack.link_down",
    variants: [
        (
            underfoot: ["A link leads down"],
            sighted: [
                "A link drops away {bearing}.",
                "The way down is {bearing}.",
            ],
            openers: [
                "A link, running down.",
                "The way into the next frame.",
                "A shaft, dropping.",
            ],
            details: [
                "",
                "It is a clean descent, which is not the same as a safe one.",
                "The hum from below is a shade lower than the hum up here.",
                "Nothing has come up it in a long time.",
            ],
            codas: [
                "",
                "Whatever is down there is further from the surface than you are.",
                "It goes down.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/fault.ron` — the `recursive_fault` and
`stalled_handshake` lines from `assets/crash_logs/`, kept verbatim and given
composition:

```ron
(
    subject: "stack.fault",
    variants: [
        (
            underfoot: ["A hole in the floor"],
            sighted: [
                "The floor has gone {bearing}.",
                "There is a fault {bearing}, and nothing under it.",
            ],
            openers: [
                "A fault, and no floor across it.",
                "The substrate has failed here.",
                "A hole, straight down.",
            ],
            details: [
                "",
                "The same fault logged all the way down, each line a little more corrupt than the last.",
                "A routine that called itself until there was no floor left underneath it.",
                "A handshake opened and never closed. It is still counting the seconds since.",
                "Something here asked for authorization eleven thousand times and got no reply at all.",
            ],
            codas: [
                "",
                "It is a long way to the next frame, and only one way to find out.",
                "Nothing has patched it and nothing is going to.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/corruption.ron` — the `orphaned_write` and
`silent_eviction` lines, likewise verbatim:

```ron
(
    subject: "stack.corruption",
    variants: [
        (
            underfoot: ["Rotten substrate"],
            sighted: [
                "The substrate is rotten {bearing}.",
                "A rotten stretch runs {bearing}.",
            ],
            openers: [
                "Rotten substrate.",
                "The floor here has gone soft.",
                "A stretch of rot.",
            ],
            details: [
                "",
                "The rot still holds a write that never landed. Something was saving when the floor went.",
                "A buffer, flushed over and over into a device that had already stopped answering.",
                "Eviction notice, timestamped, unread. Whatever lived here was still resident.",
                "The substrate reclaimed this block while it was in use. Nothing logged an objection.",
            ],
            codas: [
                "",
                "Standing on it costs, and standing on it a while costs more.",
                "It spreads slower than you walk.",
            ],
        ),
    ],
)
```

Create `assets/descriptions/frame_arrival.ron` — the one subject that reads
run state, and the reason it is a subject of its own:

```ron
(
    subject: "stack.frame.arrival",
    variants: [
        (
            sighted: [
                "The frame closes around you. Colder than the last.",
                "New corridors, laid to the same forgotten plan.",
                "The hum here is a half-step off the frame above.",
            ],
            openers: ["A frame of the stack."],
            details: [""],
            codas: [""],
        ),
        (
            when: "shallow",
            sighted: [
                "The grid is still overhead somewhere. It does not feel close.",
                "The first frame down. The light behind you is already the wrong colour.",
            ],
            openers: ["The first frame down."],
            details: [""],
            codas: [""],
        ),
        (
            when: "bottom",
            sighted: [
                "The floor here holds. There is nothing below this.",
                "The bottom of the stack. Whatever it was built around is on this frame.",
            ],
            openers: ["The bottom frame."],
            details: [""],
            codas: [""],
        ),
        (
            when: "traced",
            sighted: [
                "Something in the walls turns over as you arrive.",
                "The frame has been expecting somebody, and now it has one.",
            ],
            openers: ["A frame of the stack, and it has noticed you."],
            details: [""],
            codas: [""],
        ),
        (
            when: "hunted",
            sighted: [
                "The corridors are awake. You are the reason.",
                "Whatever is running this stack knows which frame you are on.",
            ],
            openers: ["A frame of the stack, and it is looking for you."],
            details: [""],
            codas: [""],
        ),
    ],
)
```

Create `assets/descriptions/README.md`:

````markdown
# Descriptions (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

## What a description is

Pure flavour. Nothing here has stats, costs or prerequisites: a description
is what a cell of a Stack frame says about itself when you stand on it, walk
past it, examine it, or stop and listen.

This directory replaced `assets/crash_logs/`, whose lines live on here as
`stack.fault` and `stack.corruption`.

## Schema

```ron
(
    subject: "stack.door",           // which thing this describes
    variants: [
        (
            when: None,              // the fallback reading; omit the field entirely
            underfoot: ["A doorway"],
            sighted: ["A door stands shut {bearing}."],
            openers: ["A door."],
            details: ["The frame is warm to stand near.", ""],
            codas: ["", "Nothing answers through it."],
        ),
        (
            when: "opened",          // a condition — see the table below
            underfoot: ["A forced seal"],
            sighted: ["The seal you forced stands open {bearing}."],
            openers: ["The seal, standing open."],
            details: [""],
            codas: [""],
        ),
    ],
)
```

Every field except `subject` is optional and defaults to empty.

### The three lengths

They are not truncations of each other — each is authored for where it goes.

- **`underfoot`** — the one centred row under the first-person view. It is
  **unwrapped and nothing clips it**, so keep it to a short phrase: the
  engine budget is 48 characters including the key prompt the game appends
  (`"  [>] descend"`, `"  — moving on costs"`). Never use `{bearing}` here —
  you are standing on the thing. `engine`'s
  `every_shipped_underfoot_line_fits_the_standing_on_row` holds this.
- **`sighted`** — one log line, fired once when the cell first comes into
  view. The log pane draws exactly one row per line with no wrapping, so
  write one sentence.
- **`openers` / `details` / `codas`** — the examine paragraph, sentence by
  sentence. The engine joins the non-empty parts with a single space and
  does nothing else, so **each fragment must be a complete sentence with its
  own full stop.** An empty string in `details` or `codas` is how a shorter
  paragraph is authored; a subject with no `openers` has no paragraph at
  all.

### `{bearing}`

The only substitution token. It expands to `ahead`, `behind`, `to your left`
or `to your right`, computed from the party's facing at the moment the line
is drawn. Legal in `sighted`, `openers`, `details` and `codas`. Write it as
a bare direction phrase — `"A door stands shut {bearing}."`, not
`"A door stands shut to the {bearing}."`.

### Subjects and conditions

| Subject | Fallback means | Conditions |
|---|---|---|
| `stack.floor` | plain corridor | — |
| `stack.door` | a doorway | — |
| `stack.sealed_door` | still sealed | `opened` |
| `stack.cache` | unopened | `spent` |
| `stack.lair` | guardian alive | `cleared` |
| `stack.orphan` | still there | `spent` |
| `stack.breakpoint` | unused | `spent` |
| `stack.link_up` | a link further up | `surface` (depth 1 — the way out) |
| `stack.link_down` | the way down | — |
| `stack.fault` | a hole in the floor | — |
| `stack.corruption` | rotten substrate | — |
| `stack.frame.arrival` | one line on entering a frame | `shallow`, `bottom`, `traced`, `hunted` |

`CellKind::Rock` has no subject on purpose. It is the default reading of a
blocked corridor and the thing everything else is distinguished against.

`stack.frame.arrival` is the **one** subject that reads run state rather than
only the place — the depth band and the Trace band. It is a separate subject
so that exception stays visible.

A condition with no variant falls back to the `when`-less one, so writing a
new spent state is additive.

**Two files may describe the same subject, and they add rather than
replace.** Variants sharing a `when` have their pools concatenated in
filename order, so dropping `my-doors.ron` beside the shipped `door.ron`
widens the door's pools instead of overriding them. The same holds for two
variants sharing a `when` inside one file. There is no override mechanism and
no precedence to learn: everything authored is reachable.

## How a fragment gets picked

Never at random. A cell's fragment is a fixed function of the frame spec
(world seed, the surface tile the stack hangs from, and the depth) folded
with the cell's own coordinates and the slot being drawn.

Two consequences worth knowing before you add files:

- **The same cell of the same stack always reads the same way**, across a
  save and reload and across sessions. That is deliberate — a place has a
  history, and a history that changed when you reloaded would not be one.
- **A different stack reads differently.** The world seed changes on every
  breach, so a new zone is new text for free.
- **Adding or removing fragments re-shuffles that subject's existing
  readings**, because the pool it indexes into changed length. Nothing
  breaks; the world just says different things in the same places.

An empty directory is legal. With nothing loaded, every surface falls back to
the terse literals the game shipped with before this system existed.

## Authoring prompt

If you are generating fragments with a language model, this is the brief the
shipped bank was written to:

> You are writing environment flavour for a first-person dungeon crawl
> through the innards of a decaying computing substrate. The player walks
> corridors of a "frame" in a "stack" — a maze several frames deep, hanging
> from a link on the surface.
>
> Voice: dry, technical, slightly elegiac. Short declaratives. The
> vocabulary is computing infrastructure — buffers, evictions, handshakes,
> substrate, addressing, ports — used literally, as the physical fabric of
> the place, never as metaphor. No jokes, no exclamation marks, no
> second-guessing the player. Nothing supernatural: no daemon, demon, ghost,
> wraith or phantom. Nothing that changes what the player can do — this text
> alters no gameplay and must never imply an action the game does not offer.
>
> For the subject `<SUBJECT>` under the condition `<CONDITION>`, write:
>
> - 1-2 `underfoot` phrases: at most 28 characters, no full stop, no
>   `{bearing}`. What the row under the view says when you are standing on
>   it.
> - 2-3 `sighted` sentences: exactly one sentence each, containing
>   `{bearing}` once as a bare direction phrase. What the log says when this
>   first comes into view.
> - 3-4 `openers`: one complete sentence each, naming the thing.
> - 3-4 `details`, including one `""`: one complete sentence each, adding an
>   observation about this particular thing.
> - 3-4 `codas`, including one `""`: one complete sentence each, closing the
>   paragraph.
>
> Any opener must read correctly followed by any detail followed by any
> coda, joined with single spaces — they are composed independently.
````

- [ ] **Step 4: Wire the db into `Game`**

In `crates/engine/src/game/lifecycle.rs`, add the field to `AssetDbs`
(after `crash_logs` on line 1024):

```rust
    descriptions: crate::descriptions::DescriptionDb,
```

In `load_asset_dbs`, beside the `crash_logs` load (line 1062):

```rust
    let (descriptions, description_warnings) =
        crate::descriptions::DescriptionDb::load_dir(&assets_dir.join("descriptions"))?;
    warnings.extend(description_warnings);
```

Add `descriptions,` to the returned `AssetDbs` literal (beside `crash_logs,`
at line 1092). Then in **both** constructors — the destructuring at lines 36
and 185, and the `insert_resource` blocks at lines 57 and 219 — add:

```rust
            descriptions: description_db,
```
```rust
        world.insert_resource(description_db);
```

In `crates/engine/src/tests/support.rs`, add `"descriptions"` to the
`copy_shipped_assets` directory list (alongside `"crash_logs"`).

- [ ] **Step 5: Run the census**

Run: `cargo test -p feral-processes-engine descriptions`
Expected: all pass, including `every_describable_cell_kind_has_a_shipped_bank_entry`,
`every_shipped_underfoot_line_fits_the_standing_on_row` and
`the_shipped_bank_uses_no_occult_naming`.

Then: `cargo test --workspace`
Expected: green — nothing consumes the db yet.

- [ ] **Step 6: Commit**

```bash
git add assets/descriptions crates/engine/src/lib.rs \
        crates/engine/src/game/lifecycle.rs crates/engine/src/tests/support.rs \
        crates/engine/src/tests/descriptions.rs
git commit -m "feat(descriptions): ship the Stack prose bank and load it at startup"
```

---

### Task 5: The `Game` half — subjects, seeds and bearings

**Files:**
- Create: `crates/engine/src/game/descriptions.rs`
- Modify: `crates/engine/src/game/mod.rs` (add `mod descriptions;` — check the file for the existing list and keep it alphabetical)
- Modify: `crates/engine/src/game/listen.rs:126` — widen `relative_bearing` to `pub(crate)`
- Modify: `crates/engine/src/tests/support.rs` — promote `descend`, `stand_at`, `frame` and `every_cell` out of `tests/listen.rs`
- Modify: `crates/engine/src/tests/listen.rs` — delete those four helpers, use the promoted ones
- Modify: `crates/engine/src/tests/descriptions.rs` (append)

**Interfaces:**
- Consumes: `FrameSpec::salted` (Task 1); `DescriptionDb::{underfoot, sighted, paragraph}` (Task 2); the `DescriptionDb` resource (Task 4); the five spent-state predicates in `game/stack_features.rs`; `StackPos` (`game/stack.rs:86`); `Game::frame_spec` (`game/stack.rs:295`).
- Produces, all on `Game`:
  - `pub(crate) fn underfoot_description(&self, pos: StackPos) -> Option<String>`
  - `pub(crate) fn sighted_description(&self, pos: StackPos, cell: (i32, i32)) -> Option<String>`
  - `pub(crate) fn cell_paragraph(&self, pos: StackPos, cell: (i32, i32)) -> Option<String>`
  - `pub(crate) fn arrival_line(&self, pos: StackPos) -> Option<String>`
  - `pub(crate) fn notability(&self, pos: StackPos, cell: (i32, i32)) -> Option<u8>`
  - and `pub(crate) fn relative_bearing(pos: StackPos, target: (i32, i32)) -> &'static str` (widened, still in `listen.rs`)

- [ ] **Step 1: Promote the shared test helpers**

Move `descend`, `frame`, `every_cell` and `stand_at` from
`crates/engine/src/tests/listen.rs` (lines 18-58) into
`crates/engine/src/tests/support.rs`, changing each `fn` to `pub(crate) fn`
and keeping the doc comments verbatim. `stand_at` needs
`use crate::stack::Dir;` and `use crate::resources::Locale;` in `support.rs`
if not already imported. Delete them from `listen.rs`, which already does
`use super::support::*;`.

Run: `cargo test -p feral-processes-engine listen`
Expected: green, unchanged.

- [ ] **Step 2: Write the failing tests**

Append to `crates/engine/src/tests/descriptions.rs`:

```rust
use crate::resources::CurrentStack;
use crate::stack::{CellKind, Dir};
use crate::*;

fn game() -> Game {
    Game::new(16, DifficultyMode::Forgiving, &crate::tests::support::test_assets_dir()).unwrap()
}

/// The first cell of the current frame holding `kind`.
fn cell_of(game: &Game, kind: CellKind) -> Option<(i32, i32)> {
    let level = crate::tests::support::frame(game);
    crate::tests::support::every_cell(&level).find(|&(x, y)| level.cell(x, y) == kind)
}

/// The core property: the same cell of the same stack reads the same way,
/// through a save and a reload, with no new save state carrying it. Mirrors
/// `the_species_a_frame_offers_survives_a_save_and_load`.
#[test]
fn a_description_survives_a_save_and_load() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let cell = cell_of(&game, CellKind::Floor).expect("every frame has floor");
    let pos = game.stack_pos().unwrap();
    let before = game.cell_paragraph(pos, cell).expect("floor describes");

    let path = std::env::temp_dir().join(format!("feral_description_roundtrip_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let reloaded = Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
    std::fs::remove_file(&path).unwrap();

    let pos = reloaded.stack_pos().unwrap();
    assert_eq!(reloaded.cell_paragraph(pos, cell), Some(before));
}

#[test]
fn the_same_cell_reads_the_same_description_twice() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();
    let cell = cell_of(&game, CellKind::Floor).unwrap();
    assert_eq!(game.cell_paragraph(pos, cell), game.cell_paragraph(pos, cell));
    assert_eq!(game.underfoot_description(pos), game.underfoot_description(pos));
}

/// Two frames of one stack are two different places and must read as two.
/// `stack.floor` carries four openers, four details and four codas, so this
/// is not vacuous — with one fragment per slot it would pass regardless.
#[test]
fn two_different_frames_describe_the_same_cell_differently() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let mut readings = std::collections::HashSet::new();
    for depth in 1..=4u32 {
        let Locale::Stack { frames, entrance, .. } = game.locale() else {
            unreachable!("not underground")
        };
        if depth > frames {
            break;
        }
        game.descend_to(depth, frames, entrance);
        let pos = game.stack_pos().unwrap();
        if let Some(cell) = cell_of(&game, CellKind::Floor) {
            readings.extend(game.cell_paragraph(pos, cell));
        }
    }
    assert!(
        readings.len() > 1,
        "every frame of the stack read the corridor identically: {readings:?}"
    );
}

/// The bearing is live view geometry, not a stored property — turning in
/// place has to move it, or the token is decoration.
#[test]
fn turning_in_place_moves_the_bearing_in_a_sighted_line() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();
    let target = (pos.x, pos.y - 2);

    crate::tests::support::stand_at(&mut game, (pos.x, pos.y), Dir::North);
    let north = game.sighted_description(game.stack_pos().unwrap(), target);
    crate::tests::support::stand_at(&mut game, (pos.x, pos.y), Dir::South);
    let south = game.sighted_description(game.stack_pos().unwrap(), target);

    assert!(north.is_some() && south.is_some());
    assert_ne!(north, south, "the bearing did not turn with the party");
    assert!(!north.unwrap().contains("{bearing}"), "the token was left unfilled");
}

/// Spent features stop being worth a line; plain corridor never was.
#[test]
fn notability_ranks_unspent_features_over_terrain() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();
    let floor = cell_of(&game, CellKind::Floor).unwrap();
    assert_eq!(game.notability(pos, floor), None, "plain corridor is not news");

    if let Some(cache) = cell_of(&game, CellKind::Cache) {
        let unopened = game.notability(pos, cache).expect("an unopened cache is notable");
        game.open_cache(pos, cache);
        assert!(
            game.notability(pos, cache).is_none_or(|spent| spent < unopened),
            "an emptied cache should not outrank itself unopened"
        );
    }
}
```

> `open_cache`'s exact signature: check `game/stack_features.rs` and adapt
> the call — the assertion is what matters, not the spelling. If emptying a
> cache in a test is awkward, mark the loot directly through
> `frame_memory_mut(pos).looted.insert(cache)`.

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p feral-processes-engine descriptions`
Expected: FAIL to compile — `no method named 'cell_paragraph' found`.

- [ ] **Step 4: Widen `relative_bearing`**

In `crates/engine/src/game/listen.rs:126`, change the signature to
`pub(crate) fn relative_bearing(...)` and append to its doc comment:

```rust
/// `pub(crate)` because the description bank's `sighted` lines and examine
/// paragraphs need the same answer. A second copy of this rotation is
/// exactly the drift the doc comment above warns about.
```

- [ ] **Step 5: Write the `Game` half**

Create `crates/engine/src/game/descriptions.rs`:

```rust
//! Turning a cell of a Stack frame into a bank lookup.
//!
//! `descriptions.rs` at the crate root is content and knows nothing about a
//! run. This is the other half: which subject a `CellKind` is, which
//! condition the spent-state predicates put it in, what seed the place folds
//! down to, and where `{bearing}` comes from.
//!
//! **No caller ever constructs a seed.** Every entry point here takes a
//! place and owns the mixing internally, the same shape as
//! `Game::orphan_species`. A caller-supplied seed parameter is how two call
//! sites drift on *how* they salt, and how a third copy-pastes `LAIR_SALT`.

use super::listen::relative_bearing;
use super::stack::StackPos;
use crate::descriptions::DescriptionDb;
use crate::resources::{CurrentStack, TraceBand};
use crate::stack::CellKind;
use crate::*;

/// Keeps description folds clear of `LAIR_SALT` (`0x1A19_B055`),
/// `ORPHAN_SALT` (`0xDEAD_C0DE`) and `FALL_SALT` (`0xFA11_1E15`). Those three
/// answer one question per frame and stay on their single XOR; this one is
/// asked per cell, per length and per slot, so it rides `FrameSpec::salted`.
const DESCRIPTION_SALT: u64 = 0xDE5C_21B3;

/// The subject key for the frame-arrival mood line — the one description
/// that reads run state (depth band and Trace band) rather than only the
/// place. A separate subject so that exception stays visible.
const ARRIVAL_SUBJECT: &str = "stack.frame.arrival";

/// What a cell with no subject of its own is described as. `CellKind::Rock`
/// has no subject at all and never reaches here.
const FLOOR_SUBJECT: &str = "stack.floor";

impl Game {
    /// Which bank subject `cell` is, and under which condition — the one
    /// place a `CellKind` is translated into bank vocabulary.
    ///
    /// The condition axis reads the five predicates both Stack views already
    /// consult rather than recording anything new, so a looted cache reads
    /// as looted everywhere or nowhere.
    fn subject_of(
        &self,
        pos: StackPos,
        cell: (i32, i32),
    ) -> Option<(&'static str, Option<&'static str>)> {
        let level = self.world.resource::<CurrentStack>().0.as_ref()?;
        Some(match level.cell(cell.0, cell.1) {
            CellKind::Rock => return None,
            CellKind::Floor => (FLOOR_SUBJECT, None),
            CellKind::LinkUp if pos.depth == 1 => ("stack.link_up", Some("surface")),
            CellKind::LinkUp => ("stack.link_up", None),
            CellKind::LinkDown => ("stack.link_down", None),
            CellKind::Door => ("stack.door", None),
            CellKind::SealedDoor if self.seal_open(pos, cell) => {
                ("stack.sealed_door", Some("opened"))
            }
            CellKind::SealedDoor => ("stack.sealed_door", None),
            CellKind::Cache if self.cache_unopened(pos, cell) => ("stack.cache", None),
            CellKind::Cache => ("stack.cache", Some("spent")),
            CellKind::Lair if self.lair_cleared(pos) => ("stack.lair", Some("cleared")),
            CellKind::Lair => ("stack.lair", None),
            CellKind::Breakpoint if self.breakpoint_spent(pos, cell) => {
                ("stack.breakpoint", Some("spent"))
            }
            CellKind::Breakpoint => ("stack.breakpoint", None),
            CellKind::Orphan if self.orphan_present(pos, cell) => ("stack.orphan", None),
            CellKind::Orphan => ("stack.orphan", Some("spent")),
            CellKind::Fault => ("stack.fault", None),
            CellKind::Corruption => ("stack.corruption", None),
        })
    }

    /// The seed a cell's description folds down to.
    ///
    /// `FrameSpec::rng_seed` already folds world seed, entrance tile and
    /// depth — and `world_seed` changes on every breach — so two links in a
    /// sector are two different stacks, two depths are two different frames,
    /// and a new zone is new text, all for free.
    fn description_seed(&self, pos: StackPos, cell: (i32, i32)) -> u64 {
        self.frame_spec(pos.depth, pos.frames, pos.entrance).salted(&[
            DESCRIPTION_SALT,
            cell.0 as u32 as u64,
            cell.1 as u32 as u64,
        ])
    }

    /// The frame's own seed, for the one description that is a property of
    /// the frame rather than of a cell in it.
    fn frame_description_seed(&self, pos: StackPos) -> u64 {
        self.frame_spec(pos.depth, pos.frames, pos.entrance)
            .salted(&[DESCRIPTION_SALT])
    }

    /// The descriptive clause for the row under the first-person view, or
    /// `None` when the bank has nothing — which leaves `stack_view` free to
    /// fall back to its own literal, so deleting the asset directory leaves
    /// the game working. A mod's prerogative, the same argument
    /// `crash_logs` made.
    pub(crate) fn underfoot_description(&self, pos: StackPos) -> Option<String> {
        let (subject, condition) = self.subject_of(pos, (pos.x, pos.y))?;
        let seed = self.description_seed(pos, (pos.x, pos.y));
        self.world
            .resource::<DescriptionDb>()
            .underfoot(subject, condition, seed)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
    }

    /// The one log line for `cell` coming into view, with `{bearing}`
    /// filled from the party's current facing.
    ///
    /// The fragments are a function of the place; the bearing is a function
    /// of live view geometry and is recomputed every call. They are
    /// composed, never stored together.
    pub(crate) fn sighted_description(&self, pos: StackPos, cell: (i32, i32)) -> Option<String> {
        let (subject, condition) = self.subject_of(pos, cell)?;
        let seed = self.description_seed(pos, cell);
        let line = self
            .world
            .resource::<DescriptionDb>()
            .sighted(subject, condition, seed)?;
        Some(fill_bearing(line, pos, cell))
    }

    /// The examine paragraph for `cell`, with `{bearing}` filled.
    pub(crate) fn cell_paragraph(&self, pos: StackPos, cell: (i32, i32)) -> Option<String> {
        let (subject, condition) = self.subject_of(pos, cell)?;
        let seed = self.description_seed(pos, cell);
        let text = self
            .world
            .resource::<DescriptionDb>()
            .paragraph(subject, condition, seed)?;
        Some(fill_bearing(&text, pos, cell))
    }

    /// The mood line fired once on entering a frame.
    ///
    /// The Trace band takes precedence over the depth band once the party is
    /// loud enough to be worth remarking on: being hunted is the more
    /// interesting fact about where you have just arrived, and it is the one
    /// the player has agency over.
    pub(crate) fn arrival_line(&self, pos: StackPos) -> Option<String> {
        let condition = match self.trace_band() {
            TraceBand::Hunted => Some("hunted"),
            TraceBand::Traced => Some("traced"),
            _ if pos.depth >= pos.frames => Some("bottom"),
            _ if pos.depth == 1 => Some("shallow"),
            _ => None,
        };
        self.world
            .resource::<DescriptionDb>()
            .sighted(ARRIVAL_SUBJECT, condition, self.frame_description_seed(pos))
            .map(str::to_string)
    }

    /// How much `cell` is worth a line when it first comes into view, or
    /// `None` when it is not worth one at all.
    ///
    /// Ranks unspent features above terrain and drops spent ones out
    /// entirely, so a corridor opening onto four features announces the one
    /// the player would actually walk to. Also the notability test the
    /// examine ray uses to decide which cell along a direction it is
    /// describing — one definition, so the thing `x` describes is the thing
    /// the log announced.
    ///
    /// `CellKind::LinkUp` is deliberately absent: it is the way the party
    /// came in, and announcing it would fire on arrival in every frame.
    /// Doors are absent for being the most common non-floor cell there is.
    pub(crate) fn notability(&self, pos: StackPos, cell: (i32, i32)) -> Option<u8> {
        let level = self.world.resource::<CurrentStack>().0.as_ref()?;
        Some(match level.cell(cell.0, cell.1) {
            CellKind::Lair if !self.lair_cleared(pos) => 5,
            CellKind::Orphan if self.orphan_present(pos, cell) => 4,
            CellKind::Cache if self.cache_unopened(pos, cell) => 3,
            CellKind::Breakpoint if !self.breakpoint_spent(pos, cell) => 3,
            CellKind::SealedDoor if !self.seal_open(pos, cell) => 2,
            CellKind::LinkDown => 2,
            CellKind::Fault | CellKind::Corruption => 1,
            _ => return None,
        })
    }
}

/// Expands the one substitution token the bank carries.
///
/// A cell the party is standing on has no bearing from itself, and
/// `relative_bearing` would answer "behind" for it, so that case is spelled
/// out rather than left to the dot product.
fn fill_bearing(text: &str, pos: StackPos, cell: (i32, i32)) -> String {
    if !text.contains("{bearing}") {
        return text.to_string();
    }
    let bearing = if cell == (pos.x, pos.y) {
        "right under you"
    } else {
        relative_bearing(pos, cell)
    };
    text.replace("{bearing}", bearing)
}
```

Add `mod descriptions;` to `crates/engine/src/game/mod.rs`, keeping that
list alphabetical.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p feral-processes-engine descriptions`
Expected: all pass.

Run: `rg GameRng crates/engine/src/game/descriptions.rs`
Expected: no output.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/game/descriptions.rs crates/engine/src/game/mod.rs \
        crates/engine/src/game/listen.rs crates/engine/src/tests/
git commit -m "feat(descriptions): resolve a Stack cell to a bank subject and seed"
```

---

### Task 6: `standing_on` draws from the bank

**Files:**
- Modify: `crates/engine/src/game/stack_view.rs:268-293`
- Test: `crates/engine/src/tests/descriptions.rs` (append)
- Check: `crates/engine/src/tests/stack.rs:3608, 3801` must keep passing untouched

**Interfaces:**
- Consumes: `Game::underfoot_description` (Task 5); `MAX_UNDERFOOT_LINE` (Task 4).
- Produces: no signature change — `StackView::standing_on` stays
  `Option<String>`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/descriptions.rs`:

```rust
/// The `None` arms stay `None`. Two existing tests in `tests/stack.rs`
/// depend on this and must keep passing untouched.
#[test]
fn a_spent_orphan_still_offers_nothing_underfoot() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let Some(orphan) = cell_of(&game, CellKind::Orphan) else {
        return; // not every frame grows one
    };
    crate::tests::support::stand_at(&mut game, orphan, Dir::North);
    let pos = game.stack_pos().unwrap();
    assert!(game.stack_view().unwrap().standing_on.is_some(), "an unspent orphan offers");

    game.frame_memory_mut(pos).adopted.insert(orphan);
    assert_eq!(game.stack_view().unwrap().standing_on, None);
}

/// The key prompts are the row's real job and survive the bank.
#[test]
fn the_underfoot_row_keeps_its_key_prompt() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let up = cell_of(&game, CellKind::LinkUp).expect("every frame has its entry");
    crate::tests::support::stand_at(&mut game, up, Dir::North);
    let row = game.stack_view().unwrap().standing_on.expect("the way out reads");
    assert!(row.ends_with("[<] surface"), "lost the prompt: {row:?}");
    assert!(row.chars().count() <= MAX_UNDERFOOT_LINE, "row is {} chars: {row:?}", row.chars().count());
}

/// Deleting the asset directory leaves the game working — the same argument
/// `crash_logs` made, and the reason the bank returns `Option`.
#[test]
fn an_empty_bank_falls_back_to_the_shipped_literals() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let up = cell_of(&game, CellKind::LinkUp).unwrap();
    crate::tests::support::stand_at(&mut game, up, Dir::North);
    game.world.insert_resource(DescriptionDb::default());
    assert_eq!(
        game.stack_view().unwrap().standing_on.as_deref(),
        Some("The link out  [<] surface")
    );
}
```

> `FrameMemory`'s adopted-set field name: confirm against
> `crates/engine/src/resources.rs` and adapt. The assertion is what matters.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine underfoot`
Expected: `the_underfoot_row_keeps_its_key_prompt` passes today (the literal
already ends that way); `an_empty_bank_falls_back_to_the_shipped_literals`
fails only after step 3 introduces the bank path — that is fine, it is the
regression guard for the fallback. `a_spent_orphan_still_offers_nothing_underfoot`
passes today and must keep passing.

- [ ] **Step 3: Rewrite the match**

Replace `crates/engine/src/game/stack_view.rs:268-293` with:

```rust
        // Each arm keeps its key-prompt suffix verbatim and draws only its
        // *descriptive clause* from the bank, falling back to the literal
        // this row shipped with. The `None` arms stay `None`: those are
        // cells with nothing to offer, not cells with nothing to say, and
        // two tests in `tests/stack.rs` pin the difference.
        let described = |fallback: &str| {
            self.underfoot_description(pos)
                .unwrap_or_else(|| fallback.to_string())
        };
        let standing_on = match level.cell(x, y) {
            CellKind::LinkDown => Some(format!("{}  [>] descend", described("A link leads down"))),
            CellKind::LinkUp if depth == 1 => Some(format!("{}  [<] surface", described("The link out"))),
            CellKind::LinkUp => Some(format!("{}  [<] climb", described("A link leads up"))),
            // Emptied on arrival rather than on a key, so this reports what
            // already happened rather than offering a choice.
            CellKind::Cache => Some(described("An empty casing")),
            CellKind::Lair => Some(described("The lair, and nothing left holding it")),
            CellKind::Door | CellKind::SealedDoor => Some(described("A doorway")),
            // Like the cache above, these report rather than offer: all three
            // fire on arrival, so by the time this line is read the port is
            // spent and the substrate has already bitten. A fault never
            // appears here at all — the party is in the frame below before
            // the view is next built.
            CellKind::Breakpoint => Some(described("A burnt-out debug port")),
            CellKind::Corruption => Some(format!("{}  — moving on costs", described("Rotten substrate"))),
            // The one line here that offers rather than reports. Everything
            // else underfoot has already happened by the time this is read;
            // an orphan costs a catalyst, so it waits for the key — and
            // stops offering once it has been taken.
            CellKind::Orphan if self.orphan_present(pos, (x, y)) => {
                Some(format!("{}  [o] adopt", described("An orphaned process")))
            }
            CellKind::Orphan => None,
            CellKind::Rock | CellKind::Floor | CellKind::Fault => None,
        };
```

Note `CellKind::Floor` stays `None`: the corridor has a `stack.floor` subject
for the examine paragraph and the sighting line, but the underfoot row is a
key prompt and plain floor prompts nothing.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p feral-processes-engine`
Expected: green, including `tests/stack.rs`'s two spent-orphan assertions.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/stack_view.rs crates/engine/src/tests/descriptions.rs
git commit -m "feat(stack): draw the standing-on clause from the description bank"
```

---

### Task 7: Sightings and the frame-arrival mood line

**Files:**
- Modify: `crates/engine/src/game/stack_view.rs:79-108` — the announce/silent split
- Modify: `crates/engine/src/game/stack.rs:784` — `restore_locale` calls the silent variant
- Modify: `crates/engine/src/game/stack.rs:322-341` — `enter_frame` logs the mood line
- Test: `crates/engine/src/tests/descriptions.rs` (append)

**Interfaces:**
- Consumes: `Game::sighted_description`, `Game::notability`, `Game::arrival_line` (Task 5).
- Produces:
  - `pub(crate) fn remember_view(&mut self)` — unchanged signature, now announces
  - `pub(crate) fn remember_view_silent(&mut self) -> Vec<(i32, i32)>` — records and returns the newly-seen cells

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/descriptions.rs`:

```rust
use crate::resources::MessageLog;

fn log_lines(game: &Game) -> Vec<String> {
    game.world
        .resource::<MessageLog>()
        .messages()
        .iter()
        .map(|m| m.text.clone())
        .collect()
}

/// A corridor opening onto four features must not push four rows into a
/// pane that shows a handful — one line per call, for the most notable
/// thing.
#[test]
fn a_newly_seen_notable_cell_logs_once_per_move() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let before = log_lines(&game).len();
    game.turn_left();
    game.turn_right();
    let after = log_lines(&game).len();
    assert!(
        after - before <= 2,
        "two turns pushed {} lines into the log",
        after - before
    );
}

/// A step that reveals nothing new says nothing. Turning twice returns the
/// party to the exact view they started from, so the second turn has no
/// newly-seen cells at all.
#[test]
fn a_step_revealing_nothing_new_logs_nothing() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    game.turn_left();
    game.turn_right();
    let settled = log_lines(&game).len();
    game.turn_left();
    game.turn_right();
    assert_eq!(log_lines(&game).len(), settled, "a repeated view announced again");
}

/// `restore_locale` calls into the same view walk, and a save reloading into
/// a corridor would otherwise replay sightings the player already read.
#[test]
fn loading_a_save_announces_no_sightings() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let path = std::env::temp_dir().join(format!("feral_sighting_load_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let reloaded = Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
    std::fs::remove_file(&path).unwrap();

    let lines = log_lines(&reloaded);
    assert_eq!(
        lines.iter().filter(|l| l.contains("ahead") || l.contains("to your")).count(),
        0,
        "the load path replayed sightings: {lines:?}"
    );
}

/// Once per frame, not once per step.
#[test]
fn a_frame_arrival_logs_a_mood_line_and_a_step_does_not() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();
    let arrival = game.arrival_line(pos).expect("the bank ships arrival lines");
    let count = |game: &Game| log_lines(game).iter().filter(|l| **l == arrival).count();
    assert_eq!(count(&game), 1, "arriving should say it once");

    game.turn_left();
    game.step_forward();
    assert_eq!(count(&game), 1, "walking re-fired the arrival line");
}
```

> `MessageLog`'s accessor and message field names: check
> `crates/engine/src/resources.rs` and adapt `log_lines`. The repo already
> has a helper for this in `crates/engine/src/tests/message_log.rs` — reuse
> it if one exists rather than writing a second.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine sighting arrival`
Expected: FAIL — `no method named 'arrival_line'` is not the failure (Task 5
added it); the mood-line test fails on `count == 0`.

- [ ] **Step 3: Split `remember_view`**

Replace `crates/engine/src/game/stack_view.rs:72-108` with:

```rust
impl Game {
    /// Records everything the party can see from where they are standing,
    /// and announces the most notable thing that just came into view.
    ///
    /// Called from every place that moves the party or turns them —
    /// anywhere the view changes, the map has to change with it, or the
    /// player is told they never looked down a corridor they are currently
    /// staring at.
    ///
    /// **The load path calls `remember_view_silent` instead.**
    /// `restore_locale` runs the same walk, and a save reloading into a
    /// corridor would replay sightings the player already read a session
    /// ago. One site, pinned by `loading_a_save_announces_no_sightings`.
    pub(crate) fn remember_view(&mut self) {
        let newly_seen = self.remember_view_silent();
        self.announce_sighting(&newly_seen);
    }

    /// The view walk itself, returning the cells that were not on the map
    /// before this call.
    ///
    /// The diff is free: `FrameMemory::seen` is consulted before the
    /// `extend`, so nothing new is stored to support it and the save format
    /// does not move.
    pub(crate) fn remember_view_silent(&mut self) -> Vec<(i32, i32)> {
        let Some(pos) = self.stack_pos() else {
            return Vec::new();
        };
        let Some(level) = self.world.resource::<CurrentStack>().0.clone() else {
            return Vec::new();
        };

        let mut seen = Vec::new();
        for (ahead, row) in view_cone(pos.x, pos.y, pos.facing).into_iter().enumerate() {
            // The party's own cell can never stop their view out of it. That
            // is not hypothetical: a door both blocks sight and is walkable —
            // the only cell that is both — so standing in a doorway would
            // otherwise blind the party to the corridor they are standing in.
            //
            // The wall that stops the view is itself in plain sight, so the
            // row is recorded before the break, not after the check.
            let blocked = ahead > 0
                && row
                    .get(STACK_VIEW_HALF_WIDTH)
                    .is_some_and(|&(cx, cy)| level.cell(cx, cy).blocks_sight());
            seen.extend(row);
            if blocked {
                break;
            }
        }

        let memory = self.frame_memory_mut(pos);
        let newly_seen: Vec<(i32, i32)> = seen
            .iter()
            .copied()
            .filter(|cell| !memory.seen.contains(cell))
            .collect();
        memory.seen.extend(seen);
        newly_seen
    }

    /// Logs one line for the most notable cell that just came into view, or
    /// nothing when none of them was worth a line.
    ///
    /// **Capped at one.** A corridor opening onto four features must not
    /// push four rows into a pane that shows a handful — and the one row it
    /// does push should be the thing the player would actually walk to.
    /// Ties break on distance and then on coordinates, so the answer is a
    /// property of the frame rather than of iteration order.
    fn announce_sighting(&mut self, newly_seen: &[(i32, i32)]) {
        let Some(pos) = self.stack_pos() else {
            return;
        };
        let Some(best) = newly_seen
            .iter()
            .filter(|&&cell| cell != (pos.x, pos.y))
            .filter_map(|&cell| self.notability(pos, cell).map(|rank| (rank, cell)))
            .max_by_key(|&(rank, cell)| {
                let steps = (cell.0 - pos.x).abs() + (cell.1 - pos.y).abs();
                (rank, std::cmp::Reverse(steps), std::cmp::Reverse(cell))
            })
            .map(|(_, cell)| cell)
        else {
            return;
        };
        if let Some(line) = self.sighted_description(pos, best) {
            self.log(line);
        }
    }
```

- [ ] **Step 4: Silence the load path**

In `crates/engine/src/game/stack.rs`, change `restore_locale`'s call (line
784) from `self.remember_view();` to:

```rust
        // Silent: this is a reload, and every sighting in this view was
        // already announced in the session that saved it.
        self.remember_view_silent();
```

- [ ] **Step 5: Fire the mood line**

In `crates/engine/src/game/stack.rs`, replace `enter_frame`'s trailing
`self.remember_view();` (line 340) with:

```rust
        self.remember_view();
        // After the view, so the frame's own line reads as the thing you
        // notice once you are standing in it. `enter_frame` is the one spine
        // every descent, ascent and fall goes through, which is why the line
        // fires here and not at each of them.
        if let Some(pos) = self.stack_pos()
            && let Some(line) = self.arrival_line(pos)
        {
            self.log(line);
        }
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p feral-processes-engine`
Expected: green. If `tests/stack.rs` or `tests/stack_movement.rs` now fail on
log-line counts, those assertions are counting the mood line — update the
expected counts, do not suppress the line.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/game/stack_view.rs crates/engine/src/game/stack.rs \
        crates/engine/src/tests/descriptions.rs
git commit -m "feat(stack): announce first sightings and a mood line per frame"
```

---

### Task 8: Absorb and delete `crash_logs`

**Files:**
- Modify: `crates/engine/src/game/listen.rs:59-91` — the rot branch
- Delete: `crates/engine/src/crash_logs.rs`
- Delete: `assets/crash_logs/` (4 `.ron` files + `README.md`)
- Modify: `crates/engine/src/lib.rs` — drop `pub mod crash_logs;`
- Modify: `crates/engine/src/game/lifecycle.rs` — drop the field, the load, and both `insert_resource` calls
- Modify: `crates/engine/src/tests/support.rs` — drop `"crash_logs"` from the copy list
- Modify: `crates/engine/src/tests/listen.rs` — port the tests
- Modify: `crates/engine/EASTER_EGGS.md:12`
- Modify: `docs/superpowers/specs/2026-08-06-easter-eggs-design.md`

**Interfaces:**
- Consumes: `Game::cell_paragraph` (Task 5).
- Produces: `CrashLogDb` and `crate::crash_logs` no longer exist.

- [ ] **Step 1: Write the failing test**

In `crates/engine/src/tests/listen.rs`, replace
`standing_on_rot_reads_the_crash_log_instead_of_a_bearing` with:

```rust
/// The rot's own reading now comes from the description bank rather than
/// from a second flavour system beside it.
#[test]
fn listening_on_rot_reads_the_description_bank() {
    let mut game = game();
    descend(&mut game);
    let cell = *rotten_cells(&game)
        .first()
        .expect("every frame grows corruption");
    stand_at(&mut game, cell, Dir::North);
    let pos = game.stack_pos().unwrap();
    let expected = game.cell_paragraph(pos, cell).expect("rot describes");
    let (trace, tick) = (trace_of(&game), game.current_tick());

    let reading = listen(&mut game);

    assert!(
        !reading.starts_with("You go still"),
        "rotten ground should read its own log, not point at something: {reading}"
    );
    assert_eq!(reading, expected);
    assert_eq!(trace_of(&game) - trace, TRACE_PER_LISTEN);
    assert!(game.current_tick() > tick, "reading rot should cost a turn");
}
```

Keep `the_same_rotten_cell_reads_the_same_line_after_a_reload` — it asserts
something still true of the new bank and is exactly the test the spec says to
port rather than drop. Delete
`a_malformed_crash_log_is_skipped_and_the_rest_still_load` and
`an_empty_crash_log_directory_leaves_the_key_working`: their replacements are
`a_malformed_description_file_is_skipped_with_a_warning` and
`an_empty_bank_directory_loads_clean` in `tests/descriptions.rs`. Delete
`crash_log_dir`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p feral-processes-engine listen`
Expected: FAIL — the reading is a `CrashLogDb` line, not the composed
paragraph.

- [ ] **Step 3: Repoint the rot branch**

In `crates/engine/src/game/listen.rs`, replace `crash_log` (lines 75-91)
with:

```rust
    /// What the rot underfoot has to say — the description bank's paragraph
    /// for this exact cell, or `None` when the ground is not rotten, or when
    /// the bank has nothing for it, which is a mod's prerogative and falls
    /// back to the bearing rather than to silence.
    fn rot_reading(&self, pos: StackPos) -> Option<String> {
        if !matches!(
            self.cell_underfoot(),
            Some(CellKind::Fault | CellKind::Corruption)
        ) {
            return None;
        }
        self.cell_paragraph(pos, (pos.x, pos.y))
    }
```

Update `reading`'s call (line 60) to `self.rot_reading(pos)` and its doc
comment's "reads *that place's* crash log" to "reads *that place's* own
description". Drop `use crate::crash_logs::CrashLogDb;` and the now-unused
`ZoneLevel` import if nothing else in the file uses it.

- [ ] **Step 4: Delete the module and its assets**

```bash
git rm crates/engine/src/crash_logs.rs
git rm -r assets/crash_logs
```

Remove `pub mod crash_logs;` from `crates/engine/src/lib.rs:7`. In
`crates/engine/src/game/lifecycle.rs` remove the `crash_logs` field (line
1024), the `CrashLogDb::load_dir` call (lines 1062-1064), the `crash_logs,`
in the returned literal (line 1092), the two destructuring bindings (lines 36
and 185) and the two `world.insert_resource(crash_log_db);` calls (lines 57
and 219). Remove `"crash_logs"` from `copy_shipped_assets`
(`tests/support.rs`).

- [ ] **Step 5: Update the easter-egg docs**

In `crates/engine/EASTER_EGGS.md:12`, change the `Z` row's description to:

```markdown
| `Z` | the Stack (map screen) | Listens: reads the description bank's paragraph for the cell on rotten ground, otherwise gives the bearing and distance of the nearest unspent feature. Costs a turn and raises Trace (`Game::listen`). |
```

In `docs/superpowers/specs/2026-08-06-easter-eggs-design.md`, find every
mention of `CrashLogDb` / `assets/crash_logs` and add a dated note that the
system was absorbed into `assets/descriptions/` on 2026-08-10 — `Z` still
works, still costs a turn and Trace, and still says the thing the frame map
cannot; only what it reads now comes from elsewhere. Do not rewrite the
spec's history, annotate it.

- [ ] **Step 6: Run the full suite**

Run: `cargo test --workspace`
Expected: green. `rg -i crashlog crates/ assets/` should return nothing but
the annotated spec note.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(listen): absorb crash logs into the description bank"
```

---

### Task 9: The inspector stops scanning underground

**Files:**
- Modify: `crates/engine/src/game/inspection.rs:24-93`
- Test: `crates/engine/src/tests/inspection.rs` (beside
  `the_inspector_offers_no_structure_while_the_party_is_underground` at line
  1026)

**Interfaces:**
- Consumes: `Game::is_underground` (existing).
- Produces: `find_target_in_direction` returns `None` underground, for both
  kinds.

This is a bug fix and stands alone: `find_target_in_direction` scans
creatures by world `Position`, and underground `Position` is pinned to the
surface entrance tile — so `x` in a corridor can open a manifest for a wild
program four frames overhead, reported as lying "that way".

- [ ] **Step 1: Write the failing test**

Add to `crates/engine/src/tests/inspection.rs`, immediately after the
existing structures test:

```rust
/// The other half of the same defect. `Position` is pinned to the surface
/// entrance tile while the party is in the Stack, so an unguarded creature
/// scan opens a manifest for a program four frames overhead and reports it
/// as lying that way. The test for whether a `Position` reader needs the
/// guard is not "does it act" but "does it claim something about where the
/// party is", and this claims exactly that.
#[test]
fn the_inspector_scans_no_creature_while_the_party_is_underground() {
    let mut game = game_with_creature_beside_the_player();
    assert!(
        game.find_target_in_direction(1, 0, MENU_SCAN_RADIUS).is_some(),
        "the fixture must put something scannable to the east"
    );

    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    game.enter_stack(pos.x, pos.y);

    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        assert!(
            game.find_target_in_direction(dx, dy, MENU_SCAN_RADIUS).is_none(),
            "the inspector found something at ({dx}, {dy}) from four frames under it"
        );
    }
}
```

> Build the fixture the way the neighbouring structures test does — reuse its
> setup helper rather than writing a second one. If it spawns only a
> structure, extend it to also place a `Creature` with a `Position` one tile
> east of the player, or add a sibling helper beside it.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p feral-processes-engine the_inspector_scans_no_creature`
Expected: FAIL — the scan returns `Some(InspectTarget::Creature(..))` from
underground.

- [ ] **Step 3: Guard the whole scan**

In `crates/engine/src/game/inspection.rs`, rewrite the doc-comment paragraph
at lines 42-47 and add the early return:

```rust
    /// **Nothing is found underground, and that is the whole function's
    /// guard rather than one scan's.** `Position` stays pinned to the
    /// surface entrance tile while the party is in the Stack, so an
    /// unguarded scan reports the base four frames overhead as being off to
    /// your east — and, before this guard covered creatures too, opened a
    /// manifest for a wild program up there as lying "that way". The guard
    /// lives here rather than at the call site for the reason
    /// `require_surface` exists.
    ///
    /// This takes no action and moves nothing, so `require_surface` does not
    /// apply and never would have caught it. The test for whether a
    /// `Position` reader needs the guard is not "does it act" but "does it
    /// claim something about where the party is" — see `CLAUDE.md`'s
    /// load-bearing-seams entry. Underground, `x` describes the cell instead
    /// (`Game::describe_view_direction`), which is a claim about the frame
    /// the party is actually in.
    pub fn find_target_in_direction(
        &mut self,
        dx: i32,
        dy: i32,
        max_range: i32,
    ) -> Option<InspectTarget> {
        if self.is_underground() {
            return None;
        }
        let player = self.player_entity();
        let start = *self.world.get::<Position>(player).unwrap();
        let in_cone = |pos: &Position| -> Option<i32> {
```

Then delete the `let underground = self.is_underground();` binding and unwrap
the `if !underground { ... }` block so both scans run unconditionally, keeping
the "strictly nearer" tie comment verbatim.

- [ ] **Step 4: Run the tests**

Run: `cargo test --workspace`
Expected: green, including the existing
`the_inspector_offers_no_structure_while_the_party_is_underground`.

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/inspection.rs crates/engine/src/tests/inspection.rs
git commit -m "fix(inspection): stop the inspector scanning the surface from underground"
```

---

### Task 10: The examine paragraph, end to end

**Files:**
- Modify: `crates/engine/src/game/stack_view.rs` — `ExamineDir` and the view-space ray, beside `view_cone`
- Modify: `crates/engine/src/lib.rs` — re-export `ExamineDir`
- Modify: `crates/app-core/src/lib.rs` — `Mode::CellDescribe`, `pending_description`, the `is_battle` arm
- Modify: `crates/app-core/src/app/inspection.rs:10-41` — the underground branch
- Modify: `crates/app-core/src/app/input.rs:119` area — dispatch the new mode
- Modify: `crates/gui/src/render/stack.rs` — `draw_cell_describe` and the width proof
- Modify: `crates/gui/src/render/mod.rs` — the dispatch arm
- Modify: `/home/trog/code/feral-processes/CLAUDE.md`, then `cp CLAUDE.md AGENTS.md` **in the primary checkout, not this worktree**
- Test: `crates/engine/src/tests/descriptions.rs`, `crates/app-core/src/tests/stack.rs`, `crates/gui/src/render/stack.rs`'s `mod tests`

**Interfaces:**
- Consumes: `Game::cell_paragraph`, `Game::notability` (Task 5); `view_cone`, `STACK_VIEW_DEPTH`, `STACK_VIEW_HALF_WIDTH` (`game/stack_view.rs`); `wrap_text` and `draw_popup` (`gui/src/render/popup.rs`).
- Produces:
  - `pub enum ExamineDir { Ahead, Left, Right, Underfoot }` (engine, re-exported at the crate root)
  - `pub fn Game::describe_view_direction(&self, dir: ExamineDir) -> Option<String>`
  - `Mode::CellDescribe` and `App::pending_description: Option<String>` (app-core)
  - `pub(super) fn draw_cell_describe(text: Option<&str>, painter: &Painter, m: &Metrics)` (gui)

- [ ] **Step 1: Write the failing engine tests**

Append to `crates/engine/src/tests/descriptions.rs`:

```rust
use crate::ExamineDir;

/// The key always answers: a ray with nothing notable on it still describes
/// the corridor, so `x` is never a keypress that does nothing.
#[test]
fn examining_an_empty_direction_still_describes_the_corridor() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    for dir in [ExamineDir::Ahead, ExamineDir::Left, ExamineDir::Right, ExamineDir::Underfoot] {
        assert!(
            game.describe_view_direction(dir).is_some(),
            "{dir:?} answered nothing"
        );
    }
}

/// View space, not compass space — `Ahead` is the way the party is looking,
/// so turning has to change what it describes.
#[test]
fn examining_ahead_is_read_in_view_space() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let north = game.describe_view_direction(ExamineDir::Ahead);
    game.turn_left();
    game.turn_left();
    let south = game.describe_view_direction(ExamineDir::Ahead);
    assert!(north.is_some() && south.is_some());
    assert_ne!(north, south, "about-facing described the same cell");
}

#[test]
fn examining_underfoot_describes_the_cell_the_party_is_standing_on() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();
    assert_eq!(
        game.describe_view_direction(ExamineDir::Underfoot),
        game.cell_paragraph(pos, (pos.x, pos.y))
    );
}

#[test]
fn examining_on_the_surface_answers_nothing() {
    let game = game();
    assert_eq!(game.describe_view_direction(ExamineDir::Ahead), None);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-engine examining`
Expected: FAIL to compile — `unresolved import 'crate::ExamineDir'`.

- [ ] **Step 3: Write the ray**

Append to `crates/engine/src/game/stack_view.rs`, above `impl Game` — the ray
lives here rather than in `game/descriptions.rs` because this file's module
doc keeps `view_cone` private on purpose, and a third consumer that reached
it from elsewhere would be the agreement that doc exists to make
unnecessary:

```rust
/// Which way the examine key is looking, in **view space**.
///
/// Up is ahead, left and right are the party's own, and down is the cell
/// underfoot. Absolute compass directions are wrong in a first-person view:
/// the same keypress has to mean the same thing to the player whichever way
/// they are facing, and `Dir::delta` is what makes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExamineDir {
    Ahead,
    Left,
    Right,
    Underfoot,
}
```

Add to the `impl Game` block in the same file:

```rust
    /// The examine paragraph for the nearest notable cell that way, or for
    /// the corridor itself when the ray holds nothing notable — so the key
    /// always answers. `None` on the surface, where
    /// `find_target_in_direction` is the inspector instead.
    ///
    /// Reads the same `view_cone` the first-person view is built from, so
    /// `x` can only describe a cell the player can actually see, and the
    /// same `notability` ranking the sighting line uses, so the thing `x`
    /// describes is the thing the log announced.
    pub fn describe_view_direction(&self, dir: ExamineDir) -> Option<String> {
        let pos = self.stack_pos()?;
        if dir == ExamineDir::Underfoot {
            return self.cell_paragraph(pos, (pos.x, pos.y));
        }
        let lateral = match dir {
            ExamineDir::Left => 0,
            ExamineDir::Ahead => STACK_VIEW_HALF_WIDTH,
            ExamineDir::Right => STACK_VIEW_HALF_WIDTH * 2,
            ExamineDir::Underfoot => unreachable!("returned above"),
        };
        let cone = view_cone(pos.x, pos.y, pos.facing);
        // Nearest first, and skipping the party's own row: a cell you are
        // standing in is what `Underfoot` is for.
        let along = cone
            .iter()
            .skip(1)
            .filter_map(|row| row.get(lateral).copied());
        for cell in along.clone() {
            if self.notability(pos, cell).is_some() {
                return self.cell_paragraph(pos, cell);
            }
        }
        // Nothing notable that way, so describe the corridor the ray runs
        // down — the nearest walkable cell on it, or the party's own.
        let fallback = along
            .clone()
            .find(|&(x, y)| {
                self.world
                    .resource::<CurrentStack>()
                    .0
                    .as_ref()
                    .is_some_and(|level| level.cell(x, y).walkable())
            })
            .unwrap_or((pos.x, pos.y));
        self.cell_paragraph(pos, fallback)
    }
```

Re-export from `crates/engine/src/lib.rs` alongside the other view types:

```rust
pub use game::stack_view::ExamineDir;
```

> Check how `lib.rs` currently re-exports from `game::` — if `stack_view` is
> a private module, add `ExamineDir` to whatever `pub use` list already
> carries `StackCellView`/`FrameMapCell`.

- [ ] **Step 4: Run the engine tests**

Run: `cargo test -p feral-processes-engine examining`
Expected: 4 passed.

- [ ] **Step 5: Write the failing app-core test**

Append to `crates/app-core/src/tests/stack.rs`:

```rust
/// Underground, `x` + a direction describes a cell of the frame instead of
/// scanning the surface the party's `Position` is still pinned to.
#[test]
fn x_underground_opens_a_cell_description() {
    let mut app = app_underground();
    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::InspectDirection);

    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::CellDescribe);
    let text = app.pending_description.clone().expect("the key always answers");
    assert!(!text.is_empty());
    assert!(!text.contains("{bearing}"), "the token reached the screen: {text}");

    // A plain popup: any key leaves, and the text goes with it.
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_description.is_none());
}
```

> `app_underground()`: reuse whatever this file already uses to get an `App`
> into the Stack — `crates/app-core/src/tests/stack.rs:25` already asserts
> `game.is_underground()`, so the setup exists.

- [ ] **Step 6: Wire app-core**

In `crates/app-core/src/lib.rs`, add the variant beside `StructureManifest`
(line 581):

```rust
    /// The environment paragraph for one cell of a Stack frame, opened with
    /// `x` + a direction while underground. `App::pending_description` is
    /// the text — already composed by the engine, since what a place says is
    /// the engine's business and not the shell's. A plain popup: any key
    /// leaves, like `Mode::StructureManifest`, because there is nothing to
    /// page through.
    CellDescribe,
```

Add `| Mode::CellDescribe` to the non-battle arm of `Mode::is_battle` (line
765 area), and any other exhaustive `match` on `Mode` the compiler flags.

Add the field beside `pending_structure_manifest` (line 933 area):

```rust
    /// What `Mode::CellDescribe` is showing. Held rather than re-derived per
    /// frame because the paragraph is a function of the party's *facing* at
    /// the moment `x` was pressed, and the popup must not change under the
    /// player if something later moves them.
    pub pending_description: Option<String>,
```

...and to `App`'s constructor/`Default`.

In `crates/app-core/src/app/inspection.rs`, replace the body of
`handle_inspect_direction_key` after the `Esc` check:

```rust
        let Some(game) = &mut self.game else { return };
        // Underground the four keys are read in view space and describe a
        // cell, because `Position` is pinned to the surface entrance tile
        // down there and a scan of it would report the base as lying that
        // way. `Game::find_target_in_direction` refuses underground for the
        // same reason.
        if game.is_underground() {
            let dir = match key {
                GameKey::Up | GameKey::Char('k') => ExamineDir::Ahead,
                GameKey::Down | GameKey::Char('j') => ExamineDir::Underfoot,
                GameKey::Left | GameKey::Char('h') => ExamineDir::Left,
                GameKey::Right | GameKey::Char('l') => ExamineDir::Right,
                _ => return,
            };
            self.pending_description = game.describe_view_direction(dir);
            self.status_line = None;
            self.mode = Mode::CellDescribe;
            return;
        }
        let dir = match key {
            GameKey::Up | GameKey::Char('k') => Some((0, -1)),
            GameKey::Down | GameKey::Char('j') => Some((0, 1)),
            GameKey::Left | GameKey::Char('h') => Some((-1, 0)),
            GameKey::Right | GameKey::Char('l') => Some((1, 0)),
            _ => None,
        };
        let Some((dx, dy)) = dir else { return };
        match game.find_target_in_direction(dx, dy, MENU_SCAN_RADIUS) {
            // ...unchanged...
        }
```

Add beside `handle_structure_manifest_key`:

```rust
    /// The cell description is read-only and reached only from the corridor
    /// view, so there is nothing to page through and no origin to return to.
    /// Any key leaves, the way a plain popup does.
    pub(crate) fn handle_cell_describe_key(&mut self, _key: GameKey) {
        self.pending_description = None;
        self.close_screen();
    }
```

Add `use feral_processes_engine::ExamineDir;` to the imports, and the
dispatch arm in `crates/app-core/src/app/input.rs` beside line 119:

```rust
            Mode::CellDescribe => self.handle_cell_describe_key(key),
```

- [ ] **Step 7: Draw it**

Append to `crates/gui/src/render/stack.rs`:

```rust
/// How wide the cell description lets prose run before wrapping. Matches
/// `inventory::DESCRIBE_WRAP_COLUMNS` and for the same reason — a fixed
/// column count rather than a pixel width derived from the window, which
/// varies per machine.
const DESCRIBE_WRAP_COLUMNS: usize = 72;

/// The environment paragraph reached with `x` + a direction underground.
///
/// The same shape as `inventory::draw_item_describe` — the repo's one
/// prose-on-screen pattern, and `wrap_text` its only wrap helper.
pub(super) fn draw_cell_describe(text: Option<&str>, painter: &Painter, m: &Metrics) {
    let mut rows = Vec::new();
    match text {
        Some(text) => rows.extend(
            super::popup::wrap_text(text, DESCRIBE_WRAP_COLUMNS)
                .into_iter()
                .map(text_row),
        ),
        None => rows.push(text_row("Nothing to say about that.")),
    }
    rows.push(text_row(""));
    rows.push(text_row("Any key to go back"));
    draw_popup("You look", PopupSize::Large, &rows, painter, m);
}
```

> Match this file's existing imports for `text_row`, `draw_popup`,
> `PopupSize`, `Painter` and `Metrics`; add what is missing.

Add the dispatch arm in `crates/gui/src/render/mod.rs` beside
`Mode::StructureManifest` (line 537):

```rust
        Mode::CellDescribe => {
            stack::draw_cell_describe(app.pending_description.as_deref(), painter, m)
        }
```

- [ ] **Step 8: Prove the underfoot budget in pixels**

Add to `crates/gui/src/render/stack.rs`'s `mod tests` — the standing_on row
is centred and **unwrapped**, so nothing clips an over-long line and a green
engine suite alone would not have caught the overflow:

```rust
    /// `engine::MAX_UNDERFOOT_LINE` is a character budget; this is what makes
    /// it a real one. The UI font is DejaVu Sans Mono, so the widest possible
    /// line of that many characters is that many of any glyph — measured
    /// against the corridor pane at the narrowest window the UI supports.
    #[test]
    fn the_longest_underfoot_line_fits_the_stack_pane() {
        const NARROWEST_WINDOW: (f32, f32) = (1280.0, 720.0);
        let m = crate::text::ui_metrics(NARROWEST_WINDOW.1);
        let pane_w = NARROWEST_WINDOW.0 * super::super::base::PANE_W;
        let longest = "M".repeat(feral_processes_engine::MAX_UNDERFOOT_LINE);
        crate::paint::with_painter(|p| {
            let dims = p.measure_ui(&longest, m.font_size);
            assert!(
                dims.width <= pane_w,
                "{} chars measured {:.1}px against a {:.1}px pane",
                feral_processes_engine::MAX_UNDERFOOT_LINE,
                dims.width,
                pane_w
            );
        });
    }
```

> If `PANE_W` is not reachable from this module's path, import it the way
> `frame_map.rs:464` does.

- [ ] **Step 9: Update `CLAUDE.md` in the primary checkout**

**Not in this worktree** — `CLAUDE.md` and `AGENTS.md` are gitignored and
exist only at `/home/trog/code/feral-processes/`. Edit them there:

Rewrite the `find_target_in_direction` sentences at `CLAUDE.md:67-76` so both
halves are excluded for one stated reason:

```markdown
  **A read-only screen can fall into the same hole**, which is why
  `find_target_in_direction` (`game/inspection.rs`) finds nothing at all
  underground — structures and creatures alike. It takes no action and moves
  nothing, so `require_surface` does not apply and never would have caught
  it; it simply *reports*, and what it would report is your base lying off to
  the east, or a wild program four frames overhead lying "that way", while
  you stand in a corridor. The test for whether a `Position` reader needs the
  guard is therefore not "does it act" but "does it claim something about
  where the party is": contrast `maybe_spawn_wild_creature`, which reads the
  same pinned tile and only places things, and `nest_aggro_tick` below, which
  reads it and drags you into a fight. Underground, `x` routes to
  `Game::describe_view_direction` instead — a claim about the frame the party
  is actually in.
```

Add a new bullet to the **Load-bearing seams** section:

```markdown
- **A Stack description is derived, never stored.** `descriptions.rs` picks a
  fragment by `FrameSpec::salted % pool.len()` — a continuation of
  `rng_seed`'s FNV fold, which already carries world seed, entrance tile and
  depth. That is what makes the same door read the same way across a reload
  with **no `SAVE_FORMAT_VERSION` bump and no cache**, and a different stack
  read differently for free. Three things break it: reaching for `GameRng`
  (a draw does not survive a reload and shifts every later roll), reaching
  for `StdRng` (its sequence is not stable across a `rand` upgrade, so a
  dependency bump would silently reshuffle every description in the game),
  and letting a caller pass its own seed (two call sites then drift on *how*
  they salt). `assets/descriptions/README.md` is the schema and the authoring
  prompt. If you ever find yourself adding a cache or a save field for
  description text, something has started reading run state it shouldn't.
```

Then, from the primary checkout: `cp CLAUDE.md AGENTS.md`.

- [ ] **Step 10: Run everything**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
```
Expected: green, no warnings.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat(stack): describe a cell with x and a direction underground"
```

(`CLAUDE.md`/`AGENTS.md` are gitignored and will not appear in this commit —
that is expected.)

---

### Task 11: Playtest, then release

**Files:**
- Modify: `CHANGELOG.md` — a new section
- Modify: `Cargo.toml:11` — the workspace version
- Not `docs/manual.md` and not the root `README.md` — both are carved out of
  the doc obligation.

- [ ] **Step 1: Play it**

A green suite is not evidence any of this reads well on screen.

```bash
FERAL_DEV_REVEAL=1 cargo run -- --template stack
```

Walk a frame and read all four surfaces:

- the `standing_on` row at the bottom of the corridor view — **confirm it
  does not overflow the pane**; it is unwrapped and nothing clips it;
- sighting lines arriving in the log at a readable rate rather than one per
  step;
- `x` in each of the four directions, including down;
- `Z` on a rotten cell.

Descend and confirm the mood line fires once per frame and not once per step.
Then reload the save and confirm the same door reads the same way.

Report what actually happened, including anything that read badly. Feel
problems found here are the point of this step — fix them in the bank rather
than in code where possible, since the bank is a file drop.

- [ ] **Step 2: Bump and changelog**

Set `Cargo.toml:11` to `version = "0.5.20"` — a patch, not a minor: no save
format moved, which is this repo's definition of breaking below `1.0`.

Add a `CHANGELOG.md` section for `0.5.20` covering: the description bank and
its four surfaces; `crash_logs` absorbed and `assets/crash_logs/` removed
(name it explicitly — a mod dropping files there will stop working); `x`
underground now describing a cell; and the `find_target_in_direction` fix.
Match the surrounding sections' voice and structure.

- [ ] **Step 3: Final gate**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
rg -n "crash_log|CrashLog" crates/ assets/
```
Expected: green, no warnings, and no live `crash_log` references.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md Cargo.toml Cargo.lock
git commit -m "chore(release): 0.5.20 — Stack descriptions"
```

- [ ] **Step 5: Hand back before merging**

This work is on `worktree-stack-descriptions` in
`.claude/worktrees/stack-descriptions`, which is **invisible from the primary
checkout the user plays from**. Do not merge, push or tag without asking —
report that the branch is ready and let the user decide.

---

## Self-review notes

- **Spec coverage.** Every numbered file in the spec's "Files, in dependency
  order" maps to a task: 1→T1, 2-3→T2/T3, 4-5+16→T8, 6→T5/T8, 7→T6/T7,
  8→T7, 9→T9, 10→T5, 11-14→T10, 15→T4, 17→T10 step 9, 18→T11. Every named
  test in the spec appears, some renamed to match what they actually assert
  (`the_same_cell_reads_the_same_description_twice`,
  `an_underfoot_line_fits_the_standing_on_row` →
  `every_shipped_underfoot_line_fits_the_standing_on_row` plus the gui pixel
  proof).
- **Two things this plan adds beyond the spec**, both small and both
  defensible: a gui-side pixel measurement backing `MAX_UNDERFOOT_LINE`
  (the spec asks only for a max length, but an unwrapped centred row is
  exactly the class of overflow this repo has shipped behind a green suite
  before), and a `the_shipped_bank_uses_no_occult_naming` census (the spec
  states the rule as prose with nothing enforcing it).
- **Known soft spots for the implementer.** Several test fixtures reference
  helpers whose exact names must be confirmed against the repo:
  `FrameMemory`'s adopted/looted field names, `MessageLog`'s accessor,
  `open_cache`'s signature, `app_underground()` in
  `crates/app-core/src/tests/stack.rs`, and how `lib.rs` re-exports from
  `game::stack_view`. Each is flagged inline. Adapt the spelling, never the
  assertion.
