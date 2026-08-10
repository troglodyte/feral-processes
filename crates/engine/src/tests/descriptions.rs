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
