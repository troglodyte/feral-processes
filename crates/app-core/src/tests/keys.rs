//! `App::handle_key`'s modifier fold — the census that keeps its allowlist
//! and the screens that actually read a modifier from drifting apart.

/// Every `app/*.rs` file carrying a match arm that *consumes* a modifier
/// arrow, by file stem.
///
/// The fold's own `GameKey::ShiftLeft | GameKey::CtrlLeft => GameKey::Left`
/// is deliberately not this shape, which is what keeps `input.rs` out of
/// the answer without an exclusion list.
fn files_consuming_a_modifier() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
    let mut found: Vec<String> = std::fs::read_dir(&dir)
        .expect("app module directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .filter(|p| {
            std::fs::read_to_string(p)
                .expect("readable source")
                .contains("GameKey::ShiftLeft =>")
        })
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

/// **The other half of the fold at `app/input.rs`.** That match names the
/// screens allowed to see `ShiftLeft`/`ShiftRight`/`CtrlLeft`/`CtrlRight`
/// unfolded; every other screen has them rewritten to bare `Left`/`Right`
/// before its handler runs. A screen that grows the four arms without
/// being named there gets them **unreachable** — Shift and Ctrl silently
/// become plain steps, and nothing fails to compile.
///
/// That direction cannot be caught at runtime: the fold happens before
/// dispatch, so an unlisted screen's modifier arms are dead by
/// construction and its behaviour is identical to bare `Left` whether the
/// bug is present or not. Reading the source is the only place the two
/// lists can be compared at all.
///
/// It is a file-level census rather than a per-handler one because the
/// creation wizard already defeats a finer mapping — its arms sit in
/// `spend_on_item`, `spend_on_row` and `buy_perk_level` rather than in the
/// `handle_*_key` the dispatcher names.
#[test]
fn every_screen_that_consumes_a_modifier_is_named_in_the_fold() {
    assert_eq!(
        files_consuming_a_modifier(),
        [
            "basket",
            "caravan",
            "crafting",
            "creation",
            "dispatch",
            "settlement_market"
        ],
        "a screen gained modifier arms; add its Mode to `handle_key`'s \
         fold in app/input.rs or the four keys never reach it"
    );
}
