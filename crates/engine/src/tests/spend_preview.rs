//! `Game::preview_stat_spend`, `preview_perk` and `buy_perk`: the fight
//! figures a spend or a perk would change, read off a trial that is rolled
//! back. The two things worth holding are that the preview is what the real
//! commit then does, and that it leaves nothing behind.

use super::support::*;
use crate::attributes::AttributeId;
use crate::components::{
    Attributes, BoughtStats, Decompiler, Derived, Emulation, PowerReserve, StatPoints,
};
use crate::game::level_up::duel_comparison;
use crate::perks::Perk;
use crate::progression::StatOwner;
use crate::*;

fn game() -> Game {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<StatPoints>(player).unwrap().0 = 6;
    game.world.get_mut::<Perks>(player).unwrap().points = 1000;
    game
}

fn spend(pairs: &[(&str, u32)]) -> Vec<(AttributeId, u32)> {
    pairs
        .iter()
        .map(|(id, n)| (AttributeId::from(*id), *n))
        .collect()
}

fn a_spend() -> Vec<(AttributeId, u32)> {
    spend(&[
        ("analysis", 2),
        ("parity", 1),
        ("footprint", 1),
        ("bandwidth", 1),
    ])
}

/// The duel `duel_comparison` gives for two snapshots of the live game, the
/// way `buy_perk` and the level-up page build it.
fn duel_between(
    game: &Game,
    before: &crate::resources::LevelSnapshot,
    after: &crate::resources::LevelSnapshot,
) -> DuelComparison {
    let (foe, foe_ehp) = game.typical_foe();
    let zone = game.world.resource::<ZoneLevel>().0;
    duel_comparison(before, after, foe, foe_ehp, zone)
}

/// Every component a trial writes, as one comparable string. `Debug` rather
/// than `PartialEq` because `PowerReserve` has no equality and this needs no
/// new derive to read.
fn components(game: &Game) -> String {
    let p = game.player_entity();
    let w = &game.world;
    format!(
        "{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
        w.get::<Attributes>(p),
        w.get::<StatPoints>(p),
        w.get::<Perks>(p),
        w.get::<BoughtStats>(p),
        w.get::<Stats>(p),
        w.get::<Decompiler>(p),
        w.get::<PowerReserve>(p),
        w.get::<Derived>(p),
    )
}

fn save_bytes(game: &mut Game, tag: &str) -> Vec<u8> {
    let path = std::env::temp_dir().join(format!(
        "feral_spend_preview_{tag}_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    bytes
}

#[test]
fn a_spend_previews_exactly_what_it_then_does() {
    let mut game = game();
    let player = game.player_entity();
    let before = game.snapshot_player(player);
    let spend = a_spend();

    let preview = game.preview_stat_spend(&spend).unwrap();
    game.spend_stat_points(StatOwner::Player, &spend).unwrap();
    let after = game.snapshot_player(player);

    assert_eq!(preview, duel_between(&game, &before, &after));
    assert!(
        preview.per_swing.1 > preview.per_swing.0,
        "the fixture spend must move a figure, or equality proves nothing: {preview:?}"
    );
}

#[test]
fn every_perk_previews_exactly_what_buying_it_does() {
    let perks = game().perk_defs();
    assert!(!perks.is_empty());
    let mut moved = 0;
    for def in perks {
        let mut game = game();
        let preview = game.preview_perk(def.id).unwrap();
        let report = game.buy_perk(def.id).unwrap();
        assert_eq!(report.preview, preview, "{} preview != outcome", def.name);
        assert_eq!(report.name, def.name);
        assert_eq!(report.description, def.description);
        assert_eq!(report.level, 1);
        moved += usize::from(!preview.stats.is_empty());
    }
    assert!(moved >= 3, "Attacker, Defender and Buffer each move a stat");
}

#[test]
fn a_second_level_of_a_perk_reports_level_two() {
    let mut game = game();
    game.buy_perk(Perk::Attacker).unwrap();
    assert_eq!(game.buy_perk(Perk::Attacker).unwrap().level, 2);
}

/// Emulation replaces ATK with the emulated species' figure plus gear and
/// the perk receipt, so an attribute's ATK never reaches a fight. The
/// preview is the real computation and must say so.
#[test]
fn an_attribute_spend_under_emulation_previews_no_atk_change() {
    let analysis = spend(&[("analysis", 3)]);

    let mut plain = game();
    let unemulated = plain.preview_stat_spend(&analysis).unwrap();
    assert!(
        unemulated.per_swing.1 > unemulated.per_swing.0,
        "control: unemulated, Analysis adds ATK"
    );

    let mut game = game();
    let player = game.player_entity();
    game.world.entity_mut(player).insert(Emulation {
        species: "drone".into(),
        rounds_left: 3,
    });
    let emulated = game.preview_stat_spend(&analysis).unwrap();
    assert_eq!(emulated.per_swing.0, emulated.per_swing.1);
}

#[test]
fn a_preview_leaves_no_trace_in_the_game() {
    let mut game = game();
    let player = game.player_entity();
    // A reserve above the derived maximum: any recompute that escaped the
    // trial would clamp it down, and the restore would have to bring it back.
    let mut attrs = game.world.get::<Attributes>(player).cloned().unwrap();
    attrs.set(&AttributeId::from("bandwidth"), 0);
    game.world.entity_mut(player).insert(attrs);
    game.recompute_derived(player);
    assert!(
        game.max_power(player) < 100.0,
        "the fixture needs headroom to clamp"
    );
    game.world
        .entity_mut(player)
        .insert(PowerReserve::new(100.0, 100.0));

    let components_before = components(&game);
    let save_before = save_bytes(&mut game, "a");

    // Checked after every preview, not once at the end: recompute writes
    // absolute values, so a later preview would paper over an earlier leak.
    game.preview_stat_spend(&a_spend()).unwrap();
    assert_eq!(components(&game), components_before, "after the spend");
    assert_eq!(save_bytes(&mut game, "b"), save_before, "after the spend");
    for def in game.perk_defs() {
        game.preview_perk(def.id).unwrap();
        assert_eq!(components(&game), components_before, "after {}", def.name);
        assert_eq!(
            save_bytes(&mut game, "c"),
            save_before,
            "after {}",
            def.name
        );
    }
}

#[test]
fn a_component_the_player_lacks_stays_absent() {
    let mut game = game();
    let player = game.player_entity();
    game.world.entity_mut(player).remove::<Derived>();

    game.preview_stat_spend(&a_spend()).unwrap();
    game.preview_perk(Perk::Attacker).unwrap();

    assert!(game.world.get::<Derived>(player).is_none());
}

#[test]
fn buying_a_perk_needs_points_but_previewing_one_does_not() {
    let mut game = game();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 0;
    let before = components(&game);

    assert!(game.buy_perk(Perk::Attacker).is_err());
    assert_eq!(components(&game), before, "a refusal writes nothing");
    assert!(game.preview_perk(Perk::Attacker).is_some());
}
