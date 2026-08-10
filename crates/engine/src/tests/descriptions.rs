//! The description bank: loading it, resolving a variant, and composing the
//! three lengths of one cell's prose.
//!
//! Nothing here touches `resources::GameRng`, by construction — selection is
//! a fold of the frame spec. A test that needed a seeded `Game` to be stable
//! would be evidence the fold had been replaced by a draw.

use crate::descriptions::{DescriptionDb, DescriptionDef, merge};

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
