//! Autofit: the pure pick and `Game::autofit` end to end.

use super::support::*;
use crate::game::autofit::{Candidate, Rating, SlotInput, pick_autofit};
use crate::items::{EquipmentSlot, GearCopy};
use crate::*;

fn copy(name: &str) -> GearCopy {
    gear(&ItemId::from(name), 0)
}

fn rated(power: Option<i32>, decompiler: i32) -> Rating {
    Rating { power, decompiler }
}

fn cand(name: &str, power: Option<i32>, decompiler: i32) -> Candidate {
    Candidate {
        copy: copy(name),
        rating: rated(power, decompiler),
    }
}

fn slot(slot: EquipmentSlot, worn: Option<Rating>, candidates: Vec<Candidate>) -> SlotInput {
    SlotInput {
        slot,
        worn,
        candidates,
    }
}

fn picked(input: SlotInput, is_player: bool) -> Option<String> {
    let picks = pick_autofit(&[input], is_player);
    picks.first().map(|(_, c)| c.item.as_str().to_string())
}

use EquipmentSlot::{Armor, Module, Weapon};

#[test]
fn picks_the_highest_rating() {
    let input = slot(
        Weapon,
        Some(rated(Some(5), 0)),
        vec![
            cand("a", Some(7), 0),
            cand("b", Some(9), 0),
            cand("c", Some(3), 0),
        ],
    );
    assert_eq!(picked(input, true).as_deref(), Some("b"));
}

#[test]
fn a_tie_with_the_worn_piece_keeps_it() {
    let input = slot(Armor, Some(rated(Some(9), 0)), vec![cand("a", Some(9), 0)]);
    assert_eq!(picked(input.clone(), true), None);
    assert_eq!(picked(input, false), None);
}

#[test]
fn equal_rated_candidates_resolve_to_the_first_in_order() {
    let input = slot(
        Weapon,
        None,
        vec![cand("first", Some(9), 0), cand("second", Some(9), 0)],
    );
    assert_eq!(picked(input, true).as_deref(), Some("first"));
}

#[test]
fn an_unrated_worn_piece_loses_to_any_rated_candidate() {
    let input = slot(Weapon, Some(rated(None, 0)), vec![cand("a", Some(-4), 0)]);
    assert_eq!(picked(input, false).as_deref(), Some("a"));
}

#[test]
fn an_empty_slot_gets_filled() {
    let input = slot(Armor, None, vec![cand("a", Some(1), 0)]);
    assert_eq!(picked(input, false).as_deref(), Some("a"));
}

#[test]
fn the_players_module_takes_decompile_chance_over_combat() {
    let input = slot(
        Module,
        None,
        vec![cand("fighter", Some(50), 0), cand("capture", Some(1), 2)],
    );
    assert_eq!(picked(input, true).as_deref(), Some("capture"));
}

#[test]
fn the_players_module_breaks_a_decompile_tie_on_combat() {
    let input = slot(
        Module,
        Some(rated(Some(3), 2)),
        vec![cand("weak", Some(1), 2), cand("strong", Some(8), 2)],
    );
    assert_eq!(picked(input, true).as_deref(), Some("strong"));
}

#[test]
fn a_decompiler_only_module_is_eligible_for_the_player() {
    let input = slot(Module, None, vec![cand("amp", None, 2)]);
    assert_eq!(picked(input, true).as_deref(), Some("amp"));
}

#[test]
fn a_worn_decompiler_only_module_holds_against_combat_modules_on_the_player() {
    let input = slot(
        Module,
        Some(rated(None, 2)),
        vec![cand("fighter", Some(50), 0)],
    );
    assert_eq!(picked(input, true), None);
}

#[test]
fn a_companion_ignores_decompile_chance() {
    let input = slot(
        Module,
        None,
        vec![cand("capture", Some(2), 3), cand("fighter", Some(6), 0)],
    );
    assert_eq!(picked(input, false).as_deref(), Some("fighter"));
}

#[test]
fn a_companion_never_takes_a_decompiler_only_module() {
    let input = slot(Module, None, vec![cand("amp", None, 2)]);
    assert_eq!(picked(input, false), None);
}

#[test]
fn the_players_weapon_and_armor_ignore_decompile_chance() {
    for s in [Weapon, Armor] {
        let input = slot(
            s,
            None,
            vec![cand("capture", Some(2), 3), cand("fighter", Some(6), 0)],
        );
        assert_eq!(picked(input, true).as_deref(), Some("fighter"), "{s:?}");
    }
}

// ---- Game::autofit ----

fn fresh() -> Game {
    Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn worn(game: &Game, wearer: Entity, slot: EquipmentSlot) -> Option<ItemId> {
    game.world
        .get::<Equipment>(wearer)
        .and_then(|e| e.get(slot))
        .map(|w| w.copy.item)
}

fn autofit_lines(game: &Game) -> usize {
    game.message_log(50)
        .iter()
        .filter(|l| l.text.contains("equip"))
        .count()
}

#[test]
fn autofit_swaps_in_the_better_weapon_once_and_returns_the_old_one() {
    let mut game = fresh();
    let player = game.player_entity();
    let weak = ItemId::from("kinetic_edge");
    let strong = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weak, 1);
    game.equip(player, &gear(&weak, 0)).unwrap();
    give(&mut game, &strong, 1);
    let tick = game.current_tick();
    let lines = autofit_lines(&game);

    let report = game.autofit(player).unwrap();

    assert_eq!(worn(&game, player, Weapon), Some(strong.clone()));
    assert_eq!(
        held(&game, &weak),
        1,
        "the swapped-out copy is back in the pack"
    );
    assert_eq!(held(&game, &strong), 0);
    assert_eq!(game.current_tick(), tick + 1, "one autofit is one tick");
    assert_eq!(
        autofit_lines(&game),
        lines + 1,
        "one log line, no per-item line"
    );
    assert_eq!(report.equipped.len(), 1);
    assert!(
        report.message.starts_with("Autofit: you equip"),
        "{}",
        report.message
    );
}

#[test]
fn autofit_with_nothing_better_does_not_tick() {
    let mut game = fresh();
    let player = game.player_entity();
    let strong = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &strong, 1);
    game.equip(player, &gear(&strong, 0)).unwrap();
    give(&mut game, &ItemId::from("kinetic_edge"), 1);
    let tick = game.current_tick();

    let report = game.autofit(player).unwrap();

    assert!(report.equipped.is_empty());
    assert_eq!(report.message, "Autofit: already wearing the best gear.");
    assert_eq!(game.current_tick(), tick, "nothing changed costs nothing");
}

#[test]
fn autofit_is_refused_in_battle() {
    let mut game = fresh();
    let player = game.player_entity();
    give(&mut game, &ItemId::from(ids::OVERCLOCK_CORE), 1);
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    assert!(game.autofit(player).is_err());
    assert_eq!(worn(&game, player, Weapon), None);
}

#[test]
fn autofit_is_refused_for_a_program_the_player_does_not_own() {
    let mut game = fresh();
    give(&mut game, &ItemId::from(ids::OVERCLOCK_CORE), 1);
    let wild = spawn_wild_on_player_tile(&mut game);

    assert!(game.autofit(wild).is_err());
    assert_eq!(held(&game, &ItemId::from(ids::OVERCLOCK_CORE)), 1);
}

#[test]
fn a_companion_autofits_from_the_players_pack() {
    let mut game = fresh();
    let companion = spawn_tamed(&mut game, 10, 3);
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 1);
    give(&mut game, &ItemId::from(ids::NEURAL_AMPLIFIER), 1);

    let report = game.autofit(companion).unwrap();

    assert_eq!(worn(&game, companion, Weapon), Some(weapon.clone()));
    assert_eq!(
        worn(&game, companion, Module),
        None,
        "a decompiler-only module does nothing on a companion"
    );
    assert_eq!(held(&game, &weapon), 0);
    assert!(report.message.contains(" equips "), "{}", report.message);
}

/// Pins the behaviour, not gather's `is_broken` filter: a broken copy's
/// `copy_bonus` is zero, which rates as unrated and could never outrank an
/// empty slot anyway, so this passes with the filter removed.
#[test]
fn autofit_never_equips_a_broken_copy() {
    let mut game = fresh();
    let player = game.player_entity();
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    let mut broken = gear(&weapon, 0);
    broken.wear = game.durability_of(&weapon);
    game.add_copies(&broken, 1);
    assert!(game.is_broken(&broken), "fixture: the copy is spent");

    let report = game.autofit(player).unwrap();

    assert!(report.equipped.is_empty());
    assert_eq!(worn(&game, player, Weapon), None);
}
