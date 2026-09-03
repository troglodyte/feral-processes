//! The ledger's wiring: that it is registered, that it survives a reload,
//! and that a save written before it existed still loads.
//!
//! The fold itself is unit-tested in `crate::base_ledger`. What is here is
//! everything that touches the `World` and the save file, because
//! `a_save_survives_a_round_trip_through_ron_unchanged` **cannot** catch a
//! field that never reaches the disk — a `#[serde(skip)]` leaves it green.

use super::support::*;
use crate::base_ledger::{BaseLedger, Event};
use crate::items::ItemId;
use crate::*;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn item(id: &str) -> ItemId {
    ItemId(id.to_string())
}

#[test]
fn a_fresh_run_carries_an_empty_ledger() {
    let game = game(4021);
    let ledger = game.world.resource::<BaseLedger>();
    assert!(ledger.lifetime.is_empty());
    assert!(ledger.buckets.is_empty());
}

#[test]
fn what_the_base_produced_survives_a_real_save_and_load() {
    let mut game = game(4022);
    game.world.resource_mut::<BaseLedger>().fold(
        0,
        1,
        &Event::Extract {
            item: item("core_fragment"),
            rolled: 7,
            landed: 5,
            ok: true,
        },
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_base_ledger_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let totals = loaded.world.resource::<BaseLedger>().lifetime[&item("core_fragment")];
    assert_eq!(totals.mined, 5, "the lifetime total came back");
    assert_eq!(totals.lost, 2, "and so did what the clog ate");
    assert_eq!(
        loaded.world.resource::<BaseLedger>().buckets.len(),
        1,
        "the bucketed history came back too, not just the lifetime fold"
    );
}

/// The field is additive behind `#[serde(default)]`, so it costs no
/// `SAVE_FORMAT_VERSION` bump — a save written before it existed loads with
/// an empty ledger, which is exactly what that run had recorded.
#[test]
fn a_save_written_before_the_ledger_existed_still_loads() {
    let mut game = game(4023);
    let path = std::env::temp_dir().join(format!(
        "feral_processes_base_ledger_legacy_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let (version, ron) = text.split_once('\n').unwrap();
    let stripped = without_field(ron, "base_ledger");
    assert!(
        !stripped.contains("base_ledger"),
        "the fixture must actually remove the field to be a real test"
    );
    std::fs::write(&path, format!("{version}\n{stripped}")).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).expect("a pre-ledger save still loads");
    let _ = std::fs::remove_file(&path);
    assert!(loaded.world.resource::<BaseLedger>().lifetime.is_empty());
}

/// Cuts one top-level field and its whole value out of pretty RON, matching
/// brackets so a nested struct goes with it. Dropping the `field:` line
/// alone leaves the value's body behind, which parses as trailing
/// characters rather than as the older file this is imitating.
fn without_field(ron: &str, field: &str) -> String {
    let start = ron
        .find(&format!("    {field}: "))
        .expect("the field is written at the top level");
    let mut depth = 0i32;
    let mut end = start;
    for (offset, ch) in ron[start..].char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ')' if depth == 0 => break,
            ',' if depth == 0 => {
                end = start + offset + 1;
                break;
            }
            _ => {}
        }
    }
    assert!(end > start, "the field's value must be delimited");
    format!("{}{}", &ron[..start], &ron[end..])
}
