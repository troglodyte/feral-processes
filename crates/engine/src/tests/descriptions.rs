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
