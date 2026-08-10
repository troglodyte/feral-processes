//! The description bank: loading it, resolving a variant, and composing the
//! three lengths of one cell's prose.
//!
//! Nothing here touches `resources::GameRng`, by construction — selection is
//! a fold of the frame spec. A test that needed a seeded `Game` to be stable
//! would be evidence the fold had been replaced by a draw.

use crate::descriptions::{DescriptionDb, DescriptionDef, Slot, fold, index, merge};

/// A scratch bank directory holding `files` as `(filename, body)`.
///
/// Built on `support::scratch_assets_dir`'s `ScratchAssets` RAII guard
/// (the same mechanism `modded_assets_dir` and its siblings use) rather
/// than a hand-rolled `/tmp` path cleaned up by a call at the end of each
/// test body: a panic between creation and that call used to leak the
/// directory, and this module's `feral_descriptions_*` leftovers were the
/// evidence. `Drop` runs unconditionally, including on an unwind.
pub(crate) fn bank_dir(tag: &str, files: &[(&str, &str)]) -> crate::tests::support::ScratchAssets {
    let dir = crate::tests::support::scratch_assets_dir(tag);
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
}

#[test]
fn a_non_ron_file_is_ignored_without_a_warning() {
    let dir = bank_dir("non_ron", &[("door.ron", DOOR), ("README.md", "# not ron")]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    assert_eq!(db.subjects().count(), 1);
}

#[test]
fn an_empty_bank_directory_loads_clean() {
    let dir = bank_dir("empty", &[]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();

    assert!(warnings.is_empty());
    assert_eq!(db.subjects().count(), 0);
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

fn cache_bank(tag: &str) -> (DescriptionDb, crate::tests::support::ScratchAssets) {
    let dir = bank_dir(tag, &[("cache.ron", CACHE)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "warnings were {warnings:?}");
    (db, dir)
}

#[test]
fn a_condition_resolves_to_its_own_variant() {
    let (db, _dir) = cache_bank("condition");
    assert_eq!(
        db.underfoot("stack.cache", Some("spent"), 0),
        Some("An empty casing")
    );
}

/// An unauthored condition falls back rather than going silent, so writing a
/// new spent state is additive.
#[test]
fn an_unmatched_condition_falls_back() {
    let (db, _dir) = cache_bank("fallback");
    let general = db.underfoot("stack.cache", None, 0);
    assert_eq!(db.underfoot("stack.cache", Some("scorched"), 0), general);
    assert!(general.is_some());
}

#[test]
fn an_unknown_subject_reads_nothing() {
    let (db, _dir) = cache_bank("unknown");
    assert_eq!(db.underfoot("stack.nowhere", None, 0), None);
    assert_eq!(db.sighted("stack.nowhere", None, 0), None);
    assert_eq!(db.paragraph("stack.nowhere", None, 0), None);
}

/// The same seed reads the same way, every time. This is the test that
/// fails the day someone reaches for a draw instead of a fold.
#[test]
fn the_same_seed_reads_the_same_description_twice() {
    let (db, _dir) = cache_bank("stable");
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
}

/// Different seeds have to actually reach different fragments — the whole
/// point. Asserting over a sweep rather than one pair, because any single
/// pair can legitimately collide on a three-deep pool.
#[test]
fn different_seeds_reach_different_fragments() {
    let (db, _dir) = cache_bank("varied");
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
}

/// The three lengths of one cell are independent draws. If they folded the
/// same seed they would move in lockstep, and the paragraph would only ever
/// pair opener 0 with underfoot 0.
#[test]
fn the_three_lengths_of_one_cell_do_not_move_in_lockstep() {
    let (db, _dir) = cache_bank("lockstep");
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
}

/// Empty fragments are how a shorter paragraph is authored — they drop out
/// rather than leaving a double space or a dangling sentence.
#[test]
fn empty_slots_compose_into_a_shorter_paragraph() {
    let (db, _dir) = cache_bank("short");
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
/// for it under.
///
/// The key-prompt suffix width `standing_on` appends after each subject's
/// descriptive clause is deliberately *not* repeated here as a hand-copied
/// number: `every_shipped_underfoot_line_fits_the_standing_on_row` below
/// reads it straight from `game::stack_view::underfoot_suffix`, the same
/// table `Game::stack_view`'s match consults to build the row. That closes
/// the drift this table used to be exposed to — a widths column that only
/// agreed with the match by hand-copying — the same way a drifted fixture
/// once hid a real overflow behind a green suite elsewhere in this repo
/// (see `manifest-column-packer-is-suboptimal`).
///
/// A content edit that empties a pool fails here instead of shipping
/// silence at a cell nobody happened to walk onto during testing. Same
/// shape as `every_biome_a_stack_link_can_open_in_fields_a_boss`.
///
/// **The `sighted` requirement below is deliberately broader than what
/// `Game::notability` will ever let reach the screen.** `sighted_description`
/// has exactly one production caller, `announce_sighting`, and it only
/// calls it for a cell `notability` returned `Some` for. `notability`
/// returns `None` unconditionally for `stack.floor`, `stack.door` and
/// `stack.link_up` — they are never worth a line at all, by the design
/// argument in `notability`'s own doc comment — and it returns `None` for
/// every "already used" condition: `spent` (cache, breakpoint, orphan),
/// `opened` (sealed door) and `cleared` (lair). Their `sighted` pools are
/// authored anyway and checked here anyway, on purpose: `underfoot` and
/// `paragraph` (via `x`) read any cell regardless of `notability`, so a
/// player can still stand on or examine an emptied cache and see its
/// `sighted`-length prose reused nowhere — and holding every subject to
/// the same three-length shape is cheaper than tracking which subjects are
/// exempt, catches a malformed `sighted` fragment before an author has to
/// remember it is inert, and stops being inert the moment `notability`'s
/// ranking ever changes. This test's shape does not claim these particular
/// `sighted` pools play back in the current build — only `notability`'s
/// doc comment and `announce_sighting`'s call site are the source of truth
/// for that.
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
    (
        "stack.frame.arrival",
        &["shallow", "bottom", "traced", "hunted"],
    ),
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
///
/// The suffix width is read from `underfoot_suffix` per condition, rather
/// than a hand-copied worst case, so a mismatch between this test and the
/// live match in `stack_view.rs` is structurally impossible.
#[test]
fn every_shipped_underfoot_line_fits_the_standing_on_row() {
    let dir = crate::tests::support::test_assets_dir().join("descriptions");
    let (db, _) = DescriptionDb::load_dir(&dir).unwrap();
    for (subject, conditions) in SHIPPED {
        for condition in std::iter::once(None).chain(conditions.iter().map(|c| Some(*c))) {
            let suffix = crate::game::stack_view::underfoot_suffix(subject, condition)
                .chars()
                .count();
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

/// `cell_paragraph` calls `fill_bearing` unconditionally — same as
/// `sighted_description` above — but the shipped bank only ever puts
/// `{bearing}` in `sighted` pools (see every `assets/descriptions/*.ron`),
/// never in an `opener`/`detail`/`coda`. That makes any check against the
/// shipped bank's paragraph text true whether or not the substitution ever
/// actually ran — a permanently-green assertion an app-core test once
/// carried, which read as coverage it wasn't. A custom bank with the token
/// in an `opener` is what makes the claim fallible: if `cell_paragraph`
/// ever stopped calling `fill_bearing`, or `fill_bearing` were bypassed for
/// paragraph text specifically, this is what would catch it.
#[test]
fn cell_paragraph_expands_bearing_even_in_a_field_the_shipped_bank_never_uses_it_in() {
    let body = r#"(
        subject: "stack.floor",
        variants: [(
            openers: ["Somewhere {bearing}, the corridor goes on."],
        )],
    )"#;
    let dir = bank_dir("bearing_in_opener", &[("floor.ron", body)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "warnings were {warnings:?}");

    let mut game = game();
    crate::tests::support::descend(&mut game);
    let floor = cell_of(&game, CellKind::Floor).expect("every frame has floor");
    game.world.insert_resource(db);
    let pos = game.stack_pos().unwrap();

    let text = game.cell_paragraph(pos, floor).expect("floor describes");
    assert!(
        !text.contains("{bearing}"),
        "the token reached the screen unexpanded: {text:?}"
    );
}

/// `fill_bearing`'s `cell == (pos.x, pos.y)` branch — "right under you" for
/// the cell the party is standing on, spelled out because `relative_bearing`
/// would otherwise answer "behind" for it — has no coverage from the shipped
/// bank at all: the census forbids `{bearing}` in `underfoot` pools, and no
/// shipped `opener`/`detail`/`coda` carries it either, so the branch is only
/// live for a modded bank read through `x`+`Underfoot` or `Z`-listen.
/// `cell_paragraph_expands_bearing_even_in_a_field_the_shipped_bank_never_uses_it_in`
/// above proves the token gets expanded at all, but its fixture cell need
/// not be where the party stands, so it can pass by taking the `else`
/// branch alone. This one stands the party on the cell it paragraphs, so
/// only the standing-on-it branch can produce the expected text.
///
/// Verified by deletion: with `fill_bearing`'s `if cell == (pos.x, pos.y)`
/// special case removed (falling through to `relative_bearing` for every
/// cell, including this one), this test fails — `relative_bearing` reads
/// "behind" for a cell coincident with the party's own position — and
/// restoring the branch makes it pass again.
#[test]
fn fill_bearing_reads_right_under_you_for_the_cell_the_party_stands_on() {
    let body = r#"(
        subject: "stack.floor",
        variants: [(
            openers: ["The corridor continues {bearing}."],
        )],
    )"#;
    let dir = bank_dir("bearing_underfoot", &[("floor.ron", body)]);
    let (db, warnings) = DescriptionDb::load_dir(&dir).unwrap();
    assert!(warnings.is_empty(), "warnings were {warnings:?}");

    let mut game = game();
    crate::tests::support::descend(&mut game);
    let floor = cell_of(&game, CellKind::Floor).expect("every frame has floor");
    crate::tests::support::stand_at(&mut game, floor, Dir::North);
    game.world.insert_resource(db);
    let pos = game.stack_pos().unwrap();
    assert_eq!(
        (pos.x, pos.y),
        floor,
        "the party must be standing on the cell it paragraphs"
    );

    let text = game
        .cell_paragraph(pos, floor)
        .expect("floor underfoot describes");
    assert_eq!(
        text, "The corridor continues right under you.",
        "fill_bearing did not take the standing-on-it branch: {text:?}"
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

/// The twin of `subject_of_asks_the_bank_for_the_condition_the_predicates_say`
/// below, for `arrival_line`'s own condition axis. Replacing that `match`
/// with a constant `None` left every engine test green, because nothing
/// tied its four condition strings — `shallow`, `bottom`, `traced`,
/// `hunted` — to the code that picks between them; the census only proves
/// the bank *has* all four, not that `arrival_line` ever asks for the
/// right one.
///
/// Pinned by exact match against `DescriptionDb::sighted` at the frame's
/// own seed, the same shape `subject_of_asks_the_bank_for_the_condition_the_predicates_say`
/// uses — and simpler here, because unlike `subject_of`'s state-driven
/// axes, `frame_description_seed` folds only depth/frames/entrance, never
/// Trace, so raising Trace never moves the seed out from under the
/// comparison. That also means every band can be reached through a
/// synthetic `StackPos` (`frame_description_seed`/`arrival_line` read only
/// `pos.depth`/`pos.frames`, never `CurrentStack`), so this does not need
/// to search the stack for a frame at the right depth the way
/// `two_different_frames_describe_the_same_cell_differently` has to.
///
/// Depth bands: `shallow` at depth 1, `bottom` at `depth >= frames`, and
/// the unconditioned fallback for a depth strictly in between — asserted
/// against the shipped bank both as an exact match *and* as a change from
/// its neighbours, since three exact matches against three different
/// pools already prove they read differently, but the explicit `assert_ne!`
/// pairs are what the brief asks for directly.
///
/// Precedence: Trace overrides depth once it is loud enough, per
/// `arrival_line`'s own doc comment. A test that only ever raises depth
/// cannot tell that from a version where depth always wins, so the
/// decisive cases hold depth fixed at the shallow and bottom extremes —
/// the two bands most likely to be checked first in a depth-primary
/// rewrite — and raise Trace across them: depth 1 with Trace `Hunted`
/// must read `hunted`, not `shallow`; the bottom frame with Trace `Traced`
/// must read `traced`, not `bottom`.
///
/// Verified by two mutations, both restored before this test was reported
/// done: forcing `arrival_line`'s whole `match` to a constant `None` turns
/// this test red (every band reads the unconditioned fallback instead of
/// its own pool); separately rewriting the `match` so depth is checked
/// before `trace_band()` — `if pos.depth >= pos.frames { bottom } else if
/// pos.depth == 1 { shallow } else { match trace_band() { Hunted =>
/// hunted, Traced => traced, _ => None } }` — also turns it red, at the
/// two precedence assertions specifically.
#[test]
fn arrival_line_reads_its_condition_axis() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();

    fn bank(game: &Game, condition: Option<&str>, seed: u64) -> Option<String> {
        game.world
            .resource::<DescriptionDb>()
            .sighted("stack.frame.arrival", condition, seed)
            .map(str::to_string)
    }

    fn set_trace(game: &mut Game, value: u32) {
        game.world.resource_mut::<crate::resources::Trace>().0 = value;
    }

    // `frame_description_seed`/`arrival_line` read only `pos.depth`,
    // `pos.frames` and `pos.entrance` — never `CurrentStack` — so a
    // synthetic `frames` picked here, wide enough to leave room for a
    // depth that is neither 1 nor the bottom, reaches all three depth
    // bands without needing the real stack to happen to run that deep
    // (`STACK_FRAMES_MIN` is 2, so a live descent cannot be relied on for
    // this). The same freedom `link_up`'s test above already takes with a
    // synthetic depth.
    let frames = 5u32;
    let shallow_pos = StackPos {
        depth: 1,
        frames,
        ..pos
    };
    let mid_pos = StackPos {
        depth: 3,
        frames,
        ..pos
    };
    let bottom_pos = StackPos {
        depth: frames,
        frames,
        ..pos
    };

    // ---- depth bands, Trace held Quiet ----
    set_trace(&mut game, 0);
    let shallow_seed = game.frame_description_seed(shallow_pos);
    let mid_seed = game.frame_description_seed(mid_pos);
    let bottom_seed = game.frame_description_seed(bottom_pos);

    let shallow_line = game.arrival_line(shallow_pos);
    let mid_line = game.arrival_line(mid_pos);
    let bottom_line = game.arrival_line(bottom_pos);

    assert_eq!(shallow_line, bank(&game, Some("shallow"), shallow_seed));
    assert_eq!(mid_line, bank(&game, None, mid_seed));
    assert_eq!(bottom_line, bank(&game, Some("bottom"), bottom_seed));

    assert_ne!(shallow_line, mid_line, "shallow and mid read identically");
    assert_ne!(mid_line, bottom_line, "mid and bottom read identically");
    assert_ne!(
        shallow_line, bottom_line,
        "shallow and bottom read identically"
    );

    // ---- Trace bands, depth held at mid (neither shallow nor bottom) ----
    set_trace(&mut game, crate::tuning::TRACE_TRACED);
    let traced_line = game.arrival_line(mid_pos);
    assert_eq!(traced_line, bank(&game, Some("traced"), mid_seed));
    assert_ne!(
        traced_line, mid_line,
        "raising Trace to Traced changed nothing"
    );

    set_trace(&mut game, crate::tuning::TRACE_HUNTED);
    let hunted_line = game.arrival_line(mid_pos);
    assert_eq!(hunted_line, bank(&game, Some("hunted"), mid_seed));
    assert_ne!(
        hunted_line, traced_line,
        "Traced and Hunted read identically"
    );

    // ---- precedence: Trace overrides depth, checked at both depth extremes ----
    set_trace(&mut game, crate::tuning::TRACE_HUNTED);
    let shallow_hunted = game.arrival_line(shallow_pos);
    assert_eq!(
        shallow_hunted,
        bank(&game, Some("hunted"), shallow_seed),
        "depth 1 with Trace Hunted must read hunted, not shallow"
    );
    assert_ne!(
        shallow_hunted, shallow_line,
        "Trace Hunted did not override the shallow band at depth 1"
    );

    set_trace(&mut game, crate::tuning::TRACE_TRACED);
    let bottom_traced = game.arrival_line(bottom_pos);
    assert_eq!(
        bottom_traced,
        bank(&game, Some("traced"), bottom_seed),
        "the bottom frame with Trace Traced must read traced, not bottom"
    );
    assert_ne!(
        bottom_traced, bottom_line,
        "Trace Traced did not override the bottom band at the bottom frame"
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

// ---- `standing_on`: the row itself ------------------------------------

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
    assert!(
        game.stack_view().unwrap().standing_on.is_some(),
        "an unspent orphan offers"
    );

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
    let row = game
        .stack_view()
        .unwrap()
        .standing_on
        .expect("the way out reads");
    assert!(row.ends_with("[<] surface"), "lost the prompt: {row:?}");
    assert!(
        row.chars().count() <= MAX_UNDERFOOT_LINE,
        "row is {} chars: {row:?}",
        row.chars().count()
    );
}

/// The four other suffixed arms, pinned by exact literal text the same way
/// `the_underfoot_row_keeps_its_key_prompt` above already pins `LinkUp`'s
/// surface case.
///
/// Review found the gap this closes: `underfoot_suffix`'s call sites in
/// `stack_view.rs` pass `(subject, condition)` as bare string literals that
/// nothing checks against `UNDERFOOT_SUFFIXES`'s keys, and a miss resolves
/// to `""` (`.map_or("", ...)`) rather than failing loud. A typo'd subject
/// on any one arm silently drops that arm's key prompt from the screen
/// while every other test — including the width census, which a
/// zero-width suffix satisfies trivially — stays green. Only `LinkUp`'s
/// surface case had literal-text coverage before this test.
///
/// `LinkDown` and `Corruption` are asserted firmly rather than skipped:
/// `CellKind::LinkDown` is "never generated on the bottom frame"
/// (`stack.rs`), and depth 1 is proven non-bottom for this seed by
/// `two_different_frames_describe_the_same_cell_differently` above
/// (multiple frames); `tests/stack.rs`'s own
/// `stand_facing(&mut game, CellKind::Corruption)` call is documented
/// "every frame grows corruption". `LinkUp`'s non-surface ("climb") case
/// needs a frame past the entrance, which that same multi-frame proof
/// guarantees for this seed. `Orphan` is the one genuinely optional case —
/// `a_spent_orphan_still_offers_nothing_underfoot` above skips it the same
/// way — so it alone is skipped explicitly, with a comment, rather than
/// asserted.
#[test]
fn every_other_suffixed_arm_keeps_its_exact_key_prompt() {
    let mut game = game();
    crate::tests::support::descend(&mut game);

    let down = cell_of(&game, CellKind::LinkDown).expect("every non-bottom frame links down");
    crate::tests::support::stand_at(&mut game, down, Dir::North);
    let row = game
        .stack_view()
        .unwrap()
        .standing_on
        .expect("a link down reads");
    assert!(row.ends_with("  [>] descend"), "lost the prompt: {row:?}");

    let rot = cell_of(&game, CellKind::Corruption).expect("every frame grows corruption");
    crate::tests::support::stand_at(&mut game, rot, Dir::North);
    let row = game
        .stack_view()
        .unwrap()
        .standing_on
        .expect("corruption reads");
    assert!(
        row.ends_with("  — moving on costs"),
        "lost the prompt: {row:?}"
    );

    if let Some(orphan) = cell_of(&game, CellKind::Orphan) {
        crate::tests::support::stand_at(&mut game, orphan, Dir::North);
        let row = game
            .stack_view()
            .unwrap()
            .standing_on
            .expect("an unspent orphan reads");
        assert!(row.ends_with("  [o] adopt"), "lost the prompt: {row:?}");
    } else {
        eprintln!("skipped Orphan: not every frame grows one");
    }

    let Locale::Stack {
        frames, entrance, ..
    } = game.locale()
    else {
        unreachable!("not underground")
    };
    game.descend_to(2, frames, entrance);
    let up = cell_of(&game, CellKind::LinkUp).expect("every frame has its entry");
    crate::tests::support::stand_at(&mut game, up, Dir::North);
    let row = game
        .stack_view()
        .unwrap()
        .standing_on
        .expect("a link up reads");
    assert!(row.ends_with("  [<] climb"), "lost the prompt: {row:?}");
}

/// An *empty* bank leaves the game working — the same argument `crash_logs`
/// made, and the reason the bank returns `Option`. Simulated here with
/// `DescriptionDb::default()` rather than an actually-deleted
/// `assets/descriptions/` directory: `DescriptionDb::load_dir` calls
/// `read_dir(dir)?`, so a genuinely absent directory makes `Game::new`
/// return `NotFound` before a `Game` exists to call `stack_view` on. What a
/// mod can safely delete is the *contents*, not the directory itself.
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

// ---- sightings and the frame-arrival mood line -------------------------

fn log_lines(game: &Game) -> Vec<String> {
    game.message_log(crate::MESSAGE_LOG_CAP)
        .into_iter()
        .map(|l| l.text)
        .collect()
}

/// Searches every walkable cell and facing in `game`'s current frame for a
/// vantage whose view cone holds at least `min_notable` notable cells that
/// are not yet in `FrameMemory::seen` — asked through the real
/// `remember_view_silent`/`notability`, never reimplemented, so the search
/// exercises exactly the machinery the tests that call this are pinning.
///
/// At an arbitrary vantage point (the entry, facing north) a view cone
/// rarely holds more than zero or one notable cells, so fixtures built on
/// `descend` alone are the same vacuous trap this task has already hit
/// twice: comparing "nothing" to "nothing" passes with the code under test
/// deleted. This is how both fixes route around it.
///
/// Leaves the search's own bookkeeping behind: `FrameMemory::seen` for the
/// found vantage is cleared and the party is standing there, but
/// `remember_view` has *not* been called, so the caller controls exactly
/// which call is under test.
fn find_vantage_with_notable_cells(game: &mut Game, min_notable: usize) -> (i32, i32, Dir) {
    let level = crate::tests::support::frame(game);
    let pos = game.stack_pos().unwrap();
    for (x, y) in crate::tests::support::every_cell(&level) {
        if !level.walkable(x, y) {
            continue;
        }
        for facing in [Dir::North, Dir::East, Dir::South, Dir::West] {
            game.frame_memory_mut(pos).seen.clear();
            crate::tests::support::stand_at(game, (x, y), facing);
            let here = game.stack_pos().unwrap();
            let notable = game
                .remember_view_silent()
                .into_iter()
                .filter(|&cell| cell != (x, y) && game.notability(here, cell).is_some())
                .count();
            if notable >= min_notable {
                game.frame_memory_mut(pos).seen.clear();
                crate::tests::support::stand_at(game, (x, y), facing);
                return (x, y, facing);
            }
        }
    }
    panic!("no vantage in this frame sees {min_notable} notable cells at once");
}

/// A corridor opening onto four features must not push four rows into a
/// pane that shows a handful — one line per call, for the most notable
/// thing.
///
/// Built on `find_vantage_with_notable_cells`'s search rather than turning
/// in place at the entry, which manufactures the case the cap exists for
/// instead of hoping the default vantage happens to offer it.
#[test]
fn a_newly_seen_notable_cell_logs_once_per_move() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let (x, y, facing) = find_vantage_with_notable_cells(&mut game, 2);
    crate::tests::support::stand_at(&mut game, (x, y), facing);

    let before = log_lines(&game).len();
    game.remember_view();
    let after = log_lines(&game).len();
    assert_eq!(
        after - before,
        1,
        "two or more notable cells came into view at once and logged {} lines",
        after - before
    );
}

/// A step that reveals nothing new says nothing — the diff is the whole
/// point of splitting `remember_view_silent` out, and this is the test that
/// pins it: it must fail if `remember_view_silent` ever goes back to
/// returning the entire view cone instead of the cells not already in
/// `FrameMemory::seen`, which is the exact regression (re-announcing
/// everything currently in view, every single call) this task exists to
/// prevent.
///
/// The first draft of this test turned in place at the post-`descend`
/// vantage and compared `turn_left`/`turn_right`'s line counts, but that
/// vantage's view cone holds no notable cell at all — both turns log
/// nothing, so the assertion compared zero lines to zero lines and would
/// have passed with the diff logic deleted just as readily as with it kept.
/// Reusing `find_vantage_with_notable_cells` instead: stand somewhere a
/// notable cell genuinely is newly visible, require the first
/// `remember_view` to announce it, then require a second call from the same
/// spot — nothing left unseen — to announce nothing at all.
#[test]
fn a_step_revealing_nothing_new_logs_nothing() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let (x, y, facing) = find_vantage_with_notable_cells(&mut game, 1);
    crate::tests::support::stand_at(&mut game, (x, y), facing);

    let before = log_lines(&game).len();
    game.remember_view();
    let after_first = log_lines(&game).len();
    assert!(
        after_first > before,
        "a notable cell came into view for the first time and said nothing"
    );

    game.remember_view();
    let after_second = log_lines(&game).len();
    assert_eq!(after_second, after_first, "a repeated view announced again");
}

/// `notability`'s ranks are not a total order, so ties are common — see
/// `Game::notability`'s doc comment for the list. `announce_sighting`
/// breaks a tie on Manhattan distance before falling back to the cell
/// coordinate; reducing the comparator to rank alone still passes the whole
/// engine suite unless something pins the nearer-wins behaviour directly,
/// so this does.
///
/// Searches seed 16 (this file's fixture seed, via `game()`) across every
/// frame of its stack first, since that is the seed every other test here
/// already commits to. If none of those frames offers two newly-seen cells
/// that share a rank and differ in distance, a handful of the other seeds
/// this suite already uses elsewhere (`every_seed_puts_one_link_inside_the_opening_view`
/// in `tests/stack.rs`) are tried next, across their own depth-1 frames,
/// rather than skipping the assertion — see this test's own failure message
/// if none of them turn one up. In practice seed 16's own depth-1 frame
/// already offers one: standing at `(4, 2)` facing west sees two corruption
/// cells tied at rank 1, `(3, 2)` one step away and `(3, 3)` two steps away
/// — and `(3, 3)` is scanned *before* the nearer `(3, 2)` in `view_cone`'s
/// ahead-then-lateral order, so this fixture only passes if the tiebreak is
/// actually consulting distance rather than agreeing with scan order by
/// coincidence.
#[test]
fn a_rank_tie_is_broken_by_distance_not_scan_order() {
    // A vantage (stand + facing) and the (near, far) cells found there.
    type Vantage = (i32, i32, Dir);
    type TiedPair = (Vantage, (i32, i32), (i32, i32));

    // The pair returned must be tied at the *top* rank a vantage's view
    // offers, not just tied with each other — a same-rank pair buried under
    // a third, higher-ranked newly-seen cell would have that third cell
    // announced instead, which would make the assertion below fail for a
    // reason that has nothing to do with the tiebreak this test is pinning.
    // The true nearest top-rank cell (`near`) is required to be unique (no
    // second top-rank cell at the same minimal distance — that case is the
    // coordinate tiebreak's territory, not distance's, and this test isn't
    // making a claim about it) *and* to be scanned later than some other
    // top-rank cell (`far`) in `view_cone`'s ahead-then-lateral order. That
    // second requirement is the one that actually exercises the tiebreak:
    // reducing the comparator to rank alone leaves a stable sort, which
    // resolves a tie by scan order, so a vantage where scan order already
    // agrees with distance order (the nearest cell happens to be scanned
    // first anyway) would pass with the tiebreak deleted just as readily as
    // with it in place.
    fn rank_tie_in_current_frame(game: &mut Game) -> Option<TiedPair> {
        let level = crate::tests::support::frame(game);
        let pos = game.stack_pos().unwrap();
        for (x, y) in crate::tests::support::every_cell(&level) {
            if !level.walkable(x, y) {
                continue;
            }
            for facing in [Dir::North, Dir::East, Dir::South, Dir::West] {
                game.frame_memory_mut(pos).seen.clear();
                crate::tests::support::stand_at(game, (x, y), facing);
                let here = game.stack_pos().unwrap();
                // `idx` is the cell's position in `remember_view_silent`'s
                // returned order, which is `view_cone`'s ahead-then-lateral
                // scan order (filtering preserves relative order).
                let ranked: Vec<(usize, u8, i32, (i32, i32))> = game
                    .remember_view_silent()
                    .into_iter()
                    .enumerate()
                    .filter(|&(_, cell)| cell != (x, y))
                    .filter_map(|(idx, cell)| {
                        game.notability(here, cell).map(|rank| {
                            let steps = (cell.0 - x).abs() + (cell.1 - y).abs();
                            (idx, rank, steps, cell)
                        })
                    })
                    .collect();
                let Some(max_rank) = ranked.iter().map(|&(_, r, _, _)| r).max() else {
                    continue;
                };
                let mut top: Vec<(usize, i32, (i32, i32))> = ranked
                    .iter()
                    .filter(|&&(_, r, _, _)| r == max_rank)
                    .map(|&(idx, _, s, c)| (idx, s, c))
                    .collect();
                top.sort_by_key(|&(_, s, _)| s);
                if top.len() < 2 || top[0].1 == top[1].1 {
                    continue;
                }
                let (near_idx, _, near_cell) = top[0];
                // Among the top-rank cells scanned before the true nearest
                // one, the earliest-scanned is exactly what a scan-order
                // tiebreak (rank alone, stable sort) would pick instead.
                if let Some(&(_, _, far_cell)) = top
                    .iter()
                    .filter(|&&(idx, _, _)| idx < near_idx)
                    .min_by_key(|&&(idx, _, _)| idx)
                {
                    return Some(((x, y, facing), near_cell, far_cell));
                }
            }
        }
        None
    }

    fn rank_tie_across_frames(mut game: Game) -> Option<(Game, TiedPair)> {
        crate::tests::support::descend(&mut game);
        loop {
            if let Some(tied) = rank_tie_in_current_frame(&mut game) {
                return Some((game, tied));
            }
            let Locale::Stack {
                depth,
                frames,
                entrance,
                ..
            } = game.locale()
            else {
                return None;
            };
            if depth >= frames {
                return None;
            }
            game.descend_to(depth + 1, frames, entrance);
        }
    }

    let mut outcome = rank_tie_across_frames(game());
    if outcome.is_none() {
        for seed in [43u32, 77, 101, 2024, 7, 999, 31337] {
            let candidate = Game::new(
                seed,
                DifficultyMode::Forgiving,
                &crate::tests::support::test_assets_dir(),
            )
            .unwrap();
            outcome = rank_tie_across_frames(candidate);
            if outcome.is_some() {
                break;
            }
        }
    }
    let (mut game, ((x, y, facing), near, far)) = outcome
        .expect("no frame across seed 16 or the fallback seeds offers a same-rank tie at different distances");

    game.frame_memory_mut(game.stack_pos().unwrap())
        .seen
        .clear();
    crate::tests::support::stand_at(&mut game, (x, y), facing);
    let pos = game.stack_pos().unwrap();
    let near_line = game
        .sighted_description(pos, near)
        .expect("the nearer tied cell has a line");
    let far_line = game.sighted_description(pos, far);

    let before = log_lines(&game).len();
    game.remember_view();
    let logged = &log_lines(&game)[before..];
    assert_eq!(logged.len(), 1, "a rank tie logged {} lines", logged.len());
    assert_eq!(
        logged[0], near_line,
        "a rank tie picked the farther cell over the nearer one: {logged:?} vs far candidate {far_line:?}"
    );
}

/// `restore_locale` calls into the same view walk, and a save reloading into
/// a corridor would otherwise replay sightings the player already read.
///
/// A save made immediately after `descend` is the wrong fixture for this:
/// every mutator that changes `Locale::Stack` (`enter_frame`, `set_facing`,
/// `step`, `relocate_within_frame`) ends by calling `remember_view`, which
/// leaves the current view fully inside `FrameMemory::seen` before the save
/// ever happens. Reload that and `remember_view_silent`'s diff is empty
/// regardless of which variant `restore_locale` calls — the announcing
/// variant would have nothing new to announce either, and this test would
/// pass with the fix reverted just as readily as with it in place.
///
/// `tests::support::stand_at` is the way around that: it teleports the party
/// by writing `Locale::Stack` directly, the one path that does *not* call
/// `remember_view`. Standing next to the frame's link down (every frame has
/// exactly one, and `notability` ranks it unconditionally, so there is
/// always something to announce) leaves it inside the new view but outside
/// `FrameMemory::seen` in the save that follows. `Game::load` never restores
/// `MessageLog` (see `game::lifecycle::load`, which always inserts
/// `MessageLog::default()`), so the reloaded log holds nothing but what the
/// load itself produced — a blank slate a wrongly-announcing load path
/// cannot hide in.
#[test]
fn loading_a_save_announces_no_sightings() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let link_down = cell_of(&game, CellKind::LinkDown).expect("every frame has a link down");
    let level = crate::tests::support::frame(&game);
    let (stand, facing) = [Dir::North, Dir::East, Dir::South, Dir::West]
        .into_iter()
        .find_map(|facing| {
            let (dx, dy) = facing.delta();
            // Standing one step behind `link_down` along `facing` puts it
            // dead ahead (`ahead == 1`, dead center) once the party turns to
            // face it.
            let stand = (link_down.0 - dx, link_down.1 - dy);
            level.walkable(stand.0, stand.1).then_some((stand, facing))
        })
        .expect("a reachable link down has at least one walkable neighbor");
    crate::tests::support::stand_at(&mut game, stand, facing);

    let path = std::env::temp_dir().join(format!("feral_sighting_load_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let reloaded = Game::load(&path, &crate::tests::support::test_assets_dir()).unwrap();
    std::fs::remove_file(&path).unwrap();

    // The only line a correct silent load produces is the fixed "session
    // restored" narration — no sighting of the link down or anything else
    // freshly in view.
    assert_eq!(
        log_lines(&reloaded),
        vec!["Session restored. Reconnecting to the Grid."],
        "the load path replayed sightings"
    );
}

/// Once per frame, not once per step.
#[test]
fn a_frame_arrival_logs_a_mood_line_and_a_step_does_not() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let pos = game.stack_pos().unwrap();
    let arrival = game
        .arrival_line(pos)
        .expect("the bank ships arrival lines");
    let count = |game: &Game| log_lines(game).iter().filter(|l| **l == arrival).count();
    assert_eq!(count(&game), 1, "arriving should say it once");

    game.turn_left();
    game.step_forward();
    assert_eq!(count(&game), 1, "walking re-fired the arrival line");
}

// ---- the examine ray: `x` + a direction, in view space -----------------

use crate::ExamineDir;

/// The key always answers: a ray with nothing notable on it still describes
/// the corridor, so `x` is never a keypress that does nothing.
#[test]
fn examining_an_empty_direction_still_describes_the_corridor() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    for dir in [
        ExamineDir::Ahead,
        ExamineDir::Left,
        ExamineDir::Right,
        ExamineDir::Underfoot,
    ] {
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

/// The examine ray must not see through a wall or a shut door: seed 16's
/// depth-1 frame has a `Door` at (2, 18) sitting directly between the party
/// and an unopened `Cache` at (2, 19), so standing at (2, 16) or (2, 17)
/// facing South puts the cache one and two cells past the door on the
/// `Ahead` ray. Before `visible_rows` existed, `describe_view_direction`
/// walked the raw `view_cone` with no occlusion at all and read straight
/// through the door — a player would learn what was behind a closed door by
/// looking at the door. This is that exact repro, pinned directly rather
/// than searched for, since a search that merely required "some notable
/// cell is hidden behind some sight-blocker" would be satisfied by a case
/// the fix already handles and could miss the one that broke it.
#[test]
fn examining_ahead_does_not_see_through_a_shut_door() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let level = crate::tests::support::frame(&game);
    assert_eq!(
        level.cell(2, 18),
        CellKind::Door,
        "fixture drifted: no door at (2, 18)"
    );
    assert_eq!(
        level.cell(2, 19),
        CellKind::Cache,
        "fixture drifted: no cache at (2, 19)"
    );

    let pos = game.stack_pos().unwrap();
    let behind_the_door = game
        .cell_paragraph(pos, (2, 19))
        .expect("an unopened cache has a paragraph");

    for stand in [(2, 16), (2, 17)] {
        crate::tests::support::stand_at(&mut game, stand, Dir::South);
        let seen = game.describe_view_direction(ExamineDir::Ahead);
        assert_ne!(
            seen,
            Some(behind_the_door.clone()),
            "standing at {stand:?} facing South, x+Ahead described the cache through the shut door: {seen:?}"
        );
    }
}

/// The map's memory has to stop at the same wall the eye does — the same
/// door/cache fixture `examining_ahead_does_not_see_through_a_shut_door`
/// above pins for the examine ray. Before `visible_rows` existed as one
/// walk shared by both consumers, `remember_view_silent` had its own copy
/// of the occlusion check and this property was never at risk from a ray
/// bug; now that both consumers are built off the same function, deleting
/// occlusion from `visible_rows` breaks the map's memory too, and this is
/// the test on *this* side of that shared function — removing occlusion
/// entirely from `visible_rows` must turn this test red as well as the
/// examine one, not just the examine one, or the consolidation quietly
/// concentrated map coverage onto a test that belongs to a different
/// feature.
#[test]
fn the_map_does_not_remember_a_cell_behind_a_shut_door() {
    let mut game = game();
    crate::tests::support::descend(&mut game);
    let level = crate::tests::support::frame(&game);
    assert_eq!(
        level.cell(2, 18),
        CellKind::Door,
        "fixture drifted: no door at (2, 18)"
    );
    assert_eq!(
        level.cell(2, 19),
        CellKind::Cache,
        "fixture drifted: no cache at (2, 19)"
    );

    crate::tests::support::stand_at(&mut game, (2, 16), Dir::South);
    let seen = game.remember_view_silent();
    assert!(
        !seen.contains(&(2, 19)),
        "the map recorded a cell behind a shut door as seen: {seen:?}"
    );
}

/// A vantage (stand, facing) where the cell immediately beside the party —
/// `ahead == 0`, not the corridor ahead — is notable, and which `ExamineDir`
/// (`Left` or `Right`) finds it. Because it is the nearest possible cell on
/// its lateral column, a notable flank always wins the ray's nearest-first
/// search regardless of what lies beyond it, which is what makes it usable
/// to pin both "the flank is reachable at all" and "which side is which".
///
/// Searches every walkable cell and facing across the frames of a fresh
/// stack, descending until one is found or the bottom frame is reached,
/// rather than a hardcoded coordinate — mirroring
/// `rank_tie_across_frames` above — since where the maze happens to grow an
/// adjacent feature is an accident of generation. `want` restricts the
/// search to one specific side when the caller needs that side in
/// particular, e.g. to pin `Left` independently of `Right`.
type AdjacentVantage = (Game, (i32, i32), Dir, ExamineDir, (i32, i32));

fn adjacent_notable_vantage(mut game: Game, want: Option<ExamineDir>) -> Option<AdjacentVantage> {
    crate::tests::support::descend(&mut game);
    loop {
        let level = crate::tests::support::frame(&game);
        for (x, y) in crate::tests::support::every_cell(&level) {
            if !level.walkable(x, y) {
                continue;
            }
            for facing in [Dir::North, Dir::East, Dir::South, Dir::West] {
                let (rx, ry) = facing.right_delta();
                let candidates = [
                    (ExamineDir::Left, (x - rx, y - ry)),
                    (ExamineDir::Right, (x + rx, y + ry)),
                ];
                crate::tests::support::stand_at(&mut game, (x, y), facing);
                let pos = game.stack_pos().unwrap();
                for (dir, flank) in candidates {
                    if want.is_some_and(|w| w != dir) {
                        continue;
                    }
                    if game.notability(pos, flank).is_some() {
                        return Some((game, (x, y), facing, dir, flank));
                    }
                }
            }
        }
        let Locale::Stack {
            depth,
            frames,
            entrance,
            ..
        } = game.locale()
        else {
            return None;
        };
        if depth >= frames {
            return None;
        }
        game.descend_to(depth + 1, frames, entrance);
    }
}

/// `skip(1)` used to drop the whole nearest row rather than just the party's
/// own cell, so a notable cell immediately beside the party — reachable at
/// `ahead == 0` — was invisible to `Left`/`Right` and the ray answered with
/// whatever came next along that column instead. Pins that the flank is
/// reachable at all, for whichever side the search finds first.
#[test]
fn examining_a_flank_reads_the_cell_immediately_beside_the_party() {
    let (mut game, (x, y), facing, dir, flank) = adjacent_notable_vantage(game(), None)
        .expect("no frame across this stack offers a notable cell beside a walkable tile");
    crate::tests::support::stand_at(&mut game, (x, y), facing);
    let pos = game.stack_pos().unwrap();
    assert_eq!(
        game.describe_view_direction(dir),
        game.cell_paragraph(pos, flank),
        "{dir:?} did not read the cell immediately beside the party at {flank:?}"
    );
}

/// Nothing pins which lateral index is `Left` and which is `Right` except
/// the match arms in `describe_view_direction` — swapping their two indices
/// leaves every other test in this file green, since most of them either
/// don't distinguish a side or use a fixture where both sides happen to
/// answer with plain corridor. This searches specifically for a vantage
/// where the party's *left* flank is notable, so `Left`'s answer is pinned
/// exactly (nearest-first guarantees a notable flank wins), and then checks
/// that `Right` does **not** return that same left-flank text — which is
/// exactly what a `Left`/`Right` index swap would produce, since `Right`
/// would then read the same lateral column `Left` just read.
#[test]
fn left_and_right_read_opposite_flanks_not_each_others() {
    let (mut game, (x, y), facing, dir, left_flank) =
        adjacent_notable_vantage(game(), Some(ExamineDir::Left))
            .expect("no frame across this stack offers a notable cell on the party's left");
    assert_eq!(dir, ExamineDir::Left, "search returned the wrong side");
    crate::tests::support::stand_at(&mut game, (x, y), facing);
    let pos = game.stack_pos().unwrap();
    let left_text = game
        .cell_paragraph(pos, left_flank)
        .expect("a notable cell has a paragraph");

    assert_eq!(
        game.describe_view_direction(ExamineDir::Left),
        Some(left_text.clone())
    );
    assert_ne!(
        game.describe_view_direction(ExamineDir::Right),
        Some(left_text),
        "Right described the party's LEFT flank — Left/Right are swapped"
    );
}
