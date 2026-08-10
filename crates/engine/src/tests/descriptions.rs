//! The description bank: loading it, resolving a variant, and composing the
//! three lengths of one cell's prose.
//!
//! Nothing here touches `resources::GameRng`, by construction — selection is
//! a fold of the frame spec. A test that needed a seeded `Game` to be stable
//! would be evidence the fold had been replaced by a draw.

use crate::descriptions::{DescriptionDb, DescriptionDef, Slot, fold, index, merge};

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

/// `merge`'s sort is the one load-bearing line in this module, and it
/// cannot be pinned through `load_dir`: `std::fs::read_dir`'s order is
/// unspecified, and on at least one real filesystem it already comes back
/// alphabetically — the same order the sort produces — so a test that walks
/// a directory can pass with `sort_by` deleted and prove nothing. (An
/// earlier version of this test did exactly that, driving `load_dir` with
/// files named to defeat *creation* order; it never noticed the sort was
/// gone because the filesystem's `read_dir` order matched the sorted answer
/// anyway.) Calling `merge` directly with a `Vec` handed in already
/// mis-ordered — `"zebra"` before `"alpha"` — is the only way to prove the
/// sort itself is doing the work.
///
/// Verified by deletion: with the `sort_by` line in `merge` commented out,
/// this test fails (`Some("from z")` where `Some("from a")` was expected —
/// the unsorted input order survives verbatim). Restoring the line makes it
/// pass again.
#[test]
fn merge_sorts_by_subject_then_file_id_before_concatenating_pools() {
    let a: DescriptionDef =
        ron::from_str(r#"(subject: "stack.door", variants: [(underfoot: ["from a"])])"#).unwrap();
    let z: DescriptionDef =
        ron::from_str(r#"(subject: "stack.door", variants: [(underfoot: ["from z"])])"#).unwrap();
    // Hand-built input order is z-then-a — the reverse of file id order —
    // so only the sort inside `merge`, not incidental Vec order, can put
    // "from a" first.
    let defs = vec![("zebra".to_string(), z), ("alpha".to_string(), a)];

    let subjects = merge(defs);

    let door = subjects.get("stack.door").expect("stack.door merged");
    assert_eq!(door.len(), 1, "one fallback condition");
    assert_eq!(door[0].underfoot, vec!["from a", "from z"]);
}

/// The end-to-end path through `load_dir`: two files on one subject still
/// merge additively into one pool rather than one file winning. This does
/// not exercise ordering — see `merge_sorts_by_subject_then_file_id_before_concatenating_pools`
/// for that — only that both files' fragments survive the walk.
#[test]
fn two_files_on_one_subject_merge_their_pools() {
    let a = r#"(subject: "stack.door", variants: [(underfoot: ["from a"])])"#;
    let z = r#"(subject: "stack.door", variants: [(underfoot: ["from z"])])"#;
    let dir = bank_dir("merge", &[("zebra.ron", z), ("alpha.ron", a)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    // One condition, both files' fragments inside it — additive, not
    // first-wins.
    assert_eq!(db.variant_count("stack.door"), 1);
    let reachable: std::collections::HashSet<_> = (0..8u64)
        .filter_map(|s| db.underfoot("stack.door", None, s))
        .collect();
    assert_eq!(
        reachable,
        std::collections::HashSet::from(["from a", "from z"]),
        "both files' fragments must be reachable"
    );
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
            (when: Some("opened"), underfoot: ["open"]),
        ],
    )"#;
    let dir = bank_dir("merge_within", &[("door.ron", body)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(
        db.variant_count("stack.door"),
        2,
        "one fallback, one condition"
    );
    let reachable: std::collections::HashSet<_> = (0..8u64)
        .filter_map(|s| db.underfoot("stack.door", None, s))
        .collect();
    assert_eq!(
        reachable.len(),
        2,
        "both fallback fragments must be reachable"
    );
    assert_eq!(db.underfoot("stack.door", Some("opened"), 0), Some("open"));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// An opener that picks as `""` is authoring error, not a blessed short
/// form the way an empty `detail` or `coda` is — the opener is what makes a
/// paragraph a reading of something at all, so `paragraph` must treat it as
/// no opener rather than returning an empty-ish `Some(String)`.
#[test]
fn an_empty_opener_counts_as_no_paragraph() {
    let body = r#"(
        subject: "stack.door",
        variants: [
            (openers: [""], details: ["a detail"], codas: ["a coda"]),
        ],
    )"#;
    let dir = bank_dir("empty_opener", &[("door.ron", body)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.paragraph("stack.door", None, 0), None);
    std::fs::remove_dir_all(&dir).unwrap();
}

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
            when: Some("spent"),
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
    assert_eq!(
        db.underfoot("stack.cache", Some("spent"), 0),
        Some("An empty casing")
    );
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
        assert_eq!(
            db.underfoot("stack.cache", None, seed),
            db.underfoot("stack.cache", None, seed)
        );
        assert_eq!(
            db.paragraph("stack.cache", None, seed),
            db.paragraph("stack.cache", None, seed)
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Different seeds have to actually reach different fragments — the whole
/// point. Asserting over a sweep rather than one pair, because any single
/// pair can legitimately collide on a three-deep pool.
#[test]
fn different_seeds_reach_different_fragments() {
    let (db, dir) = cache_bank("varied");
    let paragraphs: std::collections::HashSet<_> = (0..64u64)
        .filter_map(|s| db.paragraph("stack.cache", None, s))
        .collect();
    assert!(
        paragraphs.len() >= 4,
        "64 seeds produced only {} distinct paragraphs",
        paragraphs.len()
    );
    let underfoot: std::collections::HashSet<_> = (0..64u64)
        .filter_map(|s| db.underfoot("stack.cache", None, s))
        .collect();
    assert_eq!(
        underfoot.len(),
        2,
        "both underfoot fragments should be reachable"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The three lengths of one cell are independent draws. If they folded the
/// same seed they would move in lockstep, and the paragraph would only ever
/// pair opener 0 with underfoot 0.
#[test]
fn the_three_lengths_of_one_cell_do_not_move_in_lockstep() {
    let (db, dir) = cache_bank("lockstep");
    let pairs: std::collections::HashSet<_> = (0..64u64)
        .map(|s| {
            (
                db.underfoot("stack.cache", None, s),
                db.sighted("stack.cache", None, s),
            )
        })
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
    let all: Vec<_> = (0..64u64)
        .filter_map(|s| db.paragraph("stack.cache", None, s))
        .collect();
    assert!(
        all.iter().any(|p| p.split_whitespace().count() < 8),
        "no seed produced a short paragraph: {all:?}"
    );
    for p in &all {
        assert!(!p.contains("  "), "double space in {p:?}");
        assert!(
            !p.starts_with(' ') && !p.ends_with(' '),
            "stray edge space in {p:?}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A subject whose fallback variant has no `openers` at all has no
/// paragraph, rather than one made only of a detail. This is a different
/// path from `an_empty_opener_counts_as_no_paragraph` above: there, the pool
/// holds one entry that picks as `""`; here, the pool itself is empty, so
/// `pick` returns `None` before the empty-string check ever runs. The
/// corridor fallback covers this case at the call site.
#[test]
fn a_subject_with_no_opener_has_no_paragraph() {
    let body = r#"(subject: "stack.floor", variants: [(details: ["Just corridor."])])"#;
    let dir = bank_dir("no_opener", &[("floor.ron", body)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.paragraph("stack.floor", None, 0), None);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The claim `Slot::tags`'s doc makes precise: **every pair** of the five
/// real slots reaches **every** possible joint outcome, at every pool size
/// a bank might plausibly use — not merely "looks different" for one
/// bank's particular pools, the way `different_seeds_reach_different_fragments`
/// and `the_three_lengths_of_one_cell_do_not_move_in_lockstep` above check
/// only the specific pool sizes `CACHE` happens to have.
///
/// This is the test a badly chosen future sixth slot has to fail. Verified
/// by mutation: temporarily setting `Slot::Coda`'s tags to `Slot::Detail`'s
/// with a single bit of the second word flipped reproduces exactly the
/// lockstep bug this suite already caught once — see the fix report for the
/// observed failure and the restore-to-green.
#[test]
fn every_pair_of_slots_is_independent() {
    const SLOTS: [(&str, Slot); 5] = [
        ("Underfoot", Slot::Underfoot),
        ("Sighted", Slot::Sighted),
        ("Opener", Slot::Opener),
        ("Detail", Slot::Detail),
        ("Coda", Slot::Coda),
    ];
    const SWEEP: u64 = 4096;

    for (i, &(name_a, a)) in SLOTS.iter().enumerate() {
        for &(name_b, b) in &SLOTS[i + 1..] {
            for pool_len in [2usize, 3, 4] {
                let joints: std::collections::HashSet<_> = (0..SWEEP)
                    .map(|seed| {
                        (
                            index(fold(seed, a), pool_len),
                            index(fold(seed, b), pool_len),
                        )
                    })
                    .collect();
                let possible = pool_len * pool_len;
                assert_eq!(
                    joints.len(),
                    possible,
                    "{name_a} and {name_b} reached only {}/{possible} joint outcomes at pool size {pool_len}",
                    joints.len(),
                );
            }
        }
    }
}

/// Every subject the engine will ask for, and every condition it will ask
/// for it under, plus the key-prompt suffix width `standing_on` appends
/// after this subject's descriptive clause on that row.
///
/// The third element mirrors the `standing_on` match a later task builds in
/// `crates/engine/src/game/stack_view.rs`: `stack.link_down` gets
/// `"  [>] descend"` (13), `stack.link_up` gets `"  [<] climb"` or, at
/// depth 1, `"  [<] surface"` — 13 is that subject's worst case — and
/// `stack.orphan` gets `"  [o] adopt"` (11). `stack.corruption` is the
/// outlier at 19, for `"  — moving on costs"`. Every other arm — `cache`,
/// `lair`, `door`, `sealed_door`, `breakpoint`, `floor`, `fault` — reports
/// rather than offers, so it appends nothing; `frame.arrival` has no
/// underfoot pool at all. **These two places have to change together.** If
/// `standing_on`'s match ever disagrees with this table, this test stops
/// being a gate and starts rubber-stamping whatever ships — the same way a
/// drifted fixture once hid a real overflow behind a green suite elsewhere
/// in this repo (see `manifest-column-packer-is-suboptimal`).
///
/// A content edit that empties a pool fails here instead of shipping
/// silence at a cell nobody happened to walk onto during testing. Same
/// shape as `every_biome_a_stack_link_can_open_in_fields_a_boss`.
const SHIPPED: &[(&str, &[&str], usize)] = &[
    ("stack.floor", &[], 0),
    ("stack.door", &[], 0),
    ("stack.sealed_door", &["opened"], 0),
    ("stack.cache", &["spent"], 0),
    ("stack.lair", &["cleared"], 0),
    ("stack.orphan", &["spent"], 11),
    ("stack.breakpoint", &["spent"], 0),
    ("stack.link_up", &["surface"], 13),
    ("stack.link_down", &[], 13),
    ("stack.fault", &[], 0),
    ("stack.corruption", &[], 19),
    (
        "stack.frame.arrival",
        &["shallow", "bottom", "traced", "hunted"],
        0,
    ),
];

#[test]
fn every_describable_cell_kind_has_a_shipped_bank_entry() {
    let dir = crate::tests::support::test_assets_dir().join("descriptions");
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "the shipped bank warned: {warnings:?}");

    for (subject, conditions, _) in SHIPPED {
        // Every subject answers at its fallback, in all three lengths...
        assert!(
            db.underfoot(subject, None, 0).is_some() || *subject == "stack.frame.arrival",
            "{subject} has no underfoot line"
        );
        assert!(
            db.sighted(subject, None, 0).is_some(),
            "{subject} has no sighted line"
        );
        assert!(
            db.paragraph(subject, None, 0).is_some(),
            "{subject} has no paragraph"
        );
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
        // `Game::arrival_line` is the one `sighted` reader that never runs
        // `fill_bearing` — an arrival line has no cell for the token to
        // point at (see its doc comment) — so unlike every other subject's
        // `sighted` pool, `stack.frame.arrival`'s can never carry `{bearing}`
        // without it reaching the screen unexpanded.
        if *subject == "stack.frame.arrival" {
            for condition in std::iter::once(None).chain(conditions.iter().map(|c| Some(*c))) {
                for seed in 0..64u64 {
                    if let Some(line) = db.sighted(subject, condition, seed) {
                        assert!(
                            !line.contains("{bearing}"),
                            "{subject} {condition:?} uses {{bearing}} in `sighted`, but \
                             arrival_line never fills it"
                        );
                    }
                }
            }
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
    for (subject, conditions, suffix) in SHIPPED {
        for condition in std::iter::once(None).chain(conditions.iter().map(|c| Some(*c))) {
            for seed in 0..64u64 {
                let Some(line) = db.underfoot(subject, condition, seed) else {
                    continue;
                };
                assert!(
                    line.chars().count() + suffix <= crate::MAX_UNDERFOOT_LINE,
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

// ---- the `Game` half: subjects, seeds and bearings --------------------

use crate::game::stack::StackPos;
use crate::stack::{CellKind, Dir};
use crate::*;

fn game() -> Game {
    Game::new(
        16,
        DifficultyMode::Forgiving,
        &crate::tests::support::test_assets_dir(),
    )
    .unwrap()
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

    let path = std::env::temp_dir().join(format!(
        "feral_description_roundtrip_{}.bin",
        std::process::id()
    ));
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
    assert_eq!(
        game.cell_paragraph(pos, cell),
        game.cell_paragraph(pos, cell)
    );
    assert_eq!(
        game.underfoot_description(pos),
        game.underfoot_description(pos)
    );
}

/// Two frames of one stack are two different places and must read as two.
/// `stack.floor` carries four openers, four details and four codas, so this
/// is not vacuous — with one fragment per slot it would pass regardless.
///
/// The coordinate is held fixed across depths on purpose: comparing "the
/// first `Floor` cell" per frame (an earlier version of this test) compares
/// two different *coordinates* whenever the maze layout shifts between
/// frames, which it does at seed 16 — depth 1's first floor is `(4, 1)`,
/// depth 2's is `(13, 1)`. That version passed even with depth stripped out
/// of `description_seed` entirely, because it was never actually holding
/// the cell steady. This version finds a coordinate that reads `Floor` in
/// every sampled frame and re-reads *that* one, so only depth varies.
///
/// If no such coordinate exists for a given seed, the property under test —
/// that `description_seed` depends on depth — is reached directly instead
/// of being given up on, since going through a subject/condition pair that
/// isn't even the same subject across frames would no longer be testing the
/// same claim.
#[test]
fn two_different_frames_describe_the_same_cell_differently() {
    let mut game = game();
    crate::tests::support::descend(&mut game);

    let mut common: Option<std::collections::BTreeSet<(i32, i32)>> = None;
    let mut depths = Vec::new();
    let (mut frames_seen, mut entrance_seen) = (0u32, (0i32, 0i32));
    for depth in 1..=4u32 {
        let Locale::Stack {
            frames, entrance, ..
        } = game.locale()
        else {
            unreachable!("not underground")
        };
        if depth > frames {
            break;
        }
        game.descend_to(depth, frames, entrance);
        depths.push(depth);
        (frames_seen, entrance_seen) = (frames, entrance);
        let level = crate::tests::support::frame(&game);
        let floors: std::collections::BTreeSet<(i32, i32)> =
            crate::tests::support::every_cell(&level)
                .filter(|&(x, y)| level.cell(x, y) == CellKind::Floor)
                .collect();
        common = Some(match common {
            None => floors,
            Some(prev) => prev.intersection(&floors).copied().collect(),
        });
    }
    assert!(
        depths.len() > 1,
        "the stack needs at least two frames to compare"
    );

    if let Some(cell) = common.and_then(|set| set.into_iter().next()) {
        let readings: std::collections::HashSet<_> = depths
            .iter()
            .filter_map(|&depth| {
                game.descend_to(depth, frames_seen, entrance_seen);
                let pos = game.stack_pos().unwrap();
                game.cell_paragraph(pos, cell)
            })
            .collect();
        assert!(
            readings.len() > 1,
            "coordinate {cell:?} read identically at every sampled depth: {readings:?}"
        );
    } else {
        let seeds: std::collections::HashSet<_> = depths
            .iter()
            .map(|&depth| {
                game.descend_to(depth, frames_seen, entrance_seen);
                let pos = game.stack_pos().unwrap();
                game.description_seed(pos, (0, 0))
            })
            .collect();
        assert!(
            seeds.len() > 1,
            "description_seed did not vary with depth: {seeds:?}"
        );
    }
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
    assert!(
        !north.unwrap().contains("{bearing}"),
        "the token was left unfilled"
    );
}

/// Spent features stop being worth a line; plain corridor never was.
///
/// `open_cache` takes no arguments and only ever loots the cell the party is
/// physically standing on (per `game/stack_features.rs`), which the cache
/// found by `cell_of` need not be — so the loot is marked directly through
/// `frame_memory_mut`, exactly as this test's own brief suggested as the
/// fallback.
#[test]
fn notability_ranks_unspent_features_over_terrain() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();
    let floor = cell_of(&game, CellKind::Floor).unwrap();
    assert_eq!(
        game.notability(pos, floor),
        None,
        "plain corridor is not news"
    );

    if let Some(cache) = cell_of(&game, CellKind::Cache) {
        let unopened = game
            .notability(pos, cache)
            .expect("an unopened cache is notable");
        game.frame_memory_mut(pos).looted.insert(cache);
        assert!(
            game.notability(pos, cache)
                .is_none_or(|spent| spent < unopened),
            "an emptied cache should not outrank itself unopened"
        );
    }
}

/// `arrival_line` is a property of the frame, not of a sighted cell, so
/// unlike `sighted_description` and `cell_paragraph` it is never run through
/// `fill_bearing` — there is no `cell` for the token to point at. Proven by
/// standing somewhere else in the same frame and getting the same line back
/// — the thing that would break if `arrival_line` were ever seeded off the
/// party's position instead of the frame's. Not in the brief's fixture
/// list, but added here because this task is what gives `arrival_line` its
/// first caller (mirroring `FrameSpec::salted` above), and leaving it
/// uncalled would leave it dead code.
#[test]
fn arrival_line_reads_the_frame_not_a_sighted_cell() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();

    let here = game
        .arrival_line(pos)
        .expect("depth 1 should read as shallow");
    assert!(
        !here.contains("{bearing}"),
        "arrival has no cell to point a bearing at: {here:?}"
    );

    let elsewhere = cell_of(&game, CellKind::Floor)
        .filter(|&cell| cell != (pos.x, pos.y))
        .or_else(|| {
            // The first `Floor` cell can coincide with where the party
            // already stands; fall back to any other floor cell in the
            // frame so the two reads are genuinely at different positions.
            let level = crate::tests::support::frame(&game);
            crate::tests::support::every_cell(&level).find(|&cell| {
                cell != (pos.x, pos.y) && level.cell(cell.0, cell.1) == CellKind::Floor
            })
        })
        .expect("a frame with more than one floor cell");
    crate::tests::support::stand_at(&mut game, elsewhere, Dir::North);
    let there = game
        .arrival_line(game.stack_pos().unwrap())
        .expect("still depth 1, still shallow");

    assert_eq!(
        here, there,
        "arrival_line changed when only the party's position moved, not the frame"
    );
}

/// Pins `Game::subject_of`'s bank routing against the shipped bank, through
/// only the public entry points — `subject_of` itself is private, and a
/// typo'd or unauthored condition string would fall back to the general
/// reading with a fully green suite otherwise. The census test above only
/// proves the bank *has* every entry; this proves the game actually asks
/// for the right one.
///
/// For the five state-driven axes, `pos` and `cell` are held identical
/// before and after the flip, so `description_seed` is identical on both
/// sides and only the condition passed to the bank can have changed — each
/// side is checked for exact equality against `DescriptionDb::paragraph`
/// computed at that same seed, not merely asserted to differ, since a bare
/// inequality could pass on a coincidental seed collision for the wrong
/// reason (the trap `two_different_frames_describe_the_same_cell_differently`
/// fell into above).
///
/// `link_up` cannot be held to the same shape: its condition **is**
/// `pos.depth == 1`, and depth also feeds `description_seed`
/// (`FrameSpec::rng_seed` folds it in — see that method's doc), so there is
/// no way to flip this one axis without the seed moving too. It gets the
/// same exact-match treatment against the bank, just at two different
/// seeds — the synthetic depth-2 `StackPos` still points at the real
/// depth-1 frame data (`subject_of` reads the cell kind from `CurrentStack`,
/// not from `pos.depth`), so this is still pinning the same `subject_of`
/// arm, not a different frame.
///
/// Paragraph fragments never carry `{bearing}` in the shipped bank (only
/// `sighted` lines do — see `every_shipped_underfoot_line_fits_the_standing_on_row`'s
/// sibling check below for `sighted`), so `cell_paragraph`'s `fill_bearing`
/// pass is a no-op here and the exact-match comparison against the raw bank
/// text holds.
///
/// Not every condition axis has a matching cell kind in every sampled
/// frame (seed 16's depth-1 frame has no `SealedDoor` or `Lair`, for
/// instance), so every reached frame of the stack is checked — up to depth
/// 4, mirroring `two_different_frames_describe_the_same_cell_differently`'s
/// bound above — and an axis is skipped only once none of them offered a
/// matching cell. The test fails if it manages to skip all six, since that
/// would mean it proved nothing.
#[test]
fn subject_of_asks_the_bank_for_the_condition_the_predicates_say() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let mut exercised: Vec<&str> = Vec::new();

    fn bank(game: &Game, subject: &str, condition: Option<&str>, seed: u64) -> Option<String> {
        game.world
            .resource::<DescriptionDb>()
            .paragraph(subject, condition, seed)
    }

    for depth in 1..=4u32 {
        let Locale::Stack {
            frames, entrance, ..
        } = game.locale()
        else {
            unreachable!("not underground")
        };
        if depth > frames {
            break;
        }
        game.descend_to(depth, frames, entrance);
        let pos = game.stack_pos().unwrap();

        if !exercised.contains(&"cache")
            && let Some(cell) = cell_of(&game, CellKind::Cache)
        {
            let seed = game.description_seed(pos, cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.cache", None, seed)
            );
            game.frame_memory_mut(pos).looted.insert(cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.cache", Some("spent"), seed)
            );
            exercised.push("cache");
        }

        if !exercised.contains(&"sealed_door")
            && let Some(cell) = cell_of(&game, CellKind::SealedDoor)
        {
            let seed = game.description_seed(pos, cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.sealed_door", None, seed)
            );
            game.frame_memory_mut(pos).opened.insert(cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.sealed_door", Some("opened"), seed)
            );
            exercised.push("sealed_door");
        }

        if !exercised.contains(&"breakpoint")
            && let Some(cell) = cell_of(&game, CellKind::Breakpoint)
        {
            let seed = game.description_seed(pos, cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.breakpoint", None, seed)
            );
            game.frame_memory_mut(pos).jacked.insert(cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.breakpoint", Some("spent"), seed)
            );
            exercised.push("breakpoint");
        }

        if !exercised.contains(&"orphan")
            && let Some(cell) = cell_of(&game, CellKind::Orphan)
        {
            let seed = game.description_seed(pos, cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.orphan", None, seed)
            );
            game.frame_memory_mut(pos).adopted.insert(cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.orphan", Some("spent"), seed)
            );
            exercised.push("orphan");
        }

        if !exercised.contains(&"lair")
            && let Some(cell) = cell_of(&game, CellKind::Lair)
        {
            let seed = game.description_seed(pos, cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.lair", None, seed)
            );
            game.frame_memory_mut(pos).cleared = true;
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.lair", Some("cleared"), seed)
            );
            exercised.push("lair");
        }

        // `link_up`'s condition is `pos.depth == 1` itself, so only a frame
        // reached at real depth 1 gives the "surface" side natively; the
        // "not surface" side is reached by a synthetic depth override below
        // rather than by waiting for a deeper frame, since a deeper frame's
        // `LinkUp` cell may not be at the same coordinate at all.
        if !exercised.contains(&"link_up")
            && pos.depth == 1
            && let Some(cell) = cell_of(&game, CellKind::LinkUp)
        {
            let seed_surface = game.description_seed(pos, cell);
            assert_eq!(
                game.cell_paragraph(pos, cell),
                bank(&game, "stack.link_up", Some("surface"), seed_surface)
            );

            let deeper = StackPos {
                depth: pos.depth + 1,
                ..pos
            };
            let seed_deep = game.description_seed(deeper, cell);
            assert_eq!(
                game.cell_paragraph(deeper, cell),
                bank(&game, "stack.link_up", None, seed_deep)
            );
            exercised.push("link_up");
        }
    }

    assert!(
        !exercised.is_empty(),
        "no condition axis had a matching cell kind in any sampled frame — the test proved nothing"
    );
    eprintln!("subject_of condition axes exercised: {exercised:?}");
}
