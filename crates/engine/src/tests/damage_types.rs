//! A damage type rides the attacker's swing and meets the target's
//! resistance. Every test here compares the *same* random stream with and
//! without the resisting armour, so the only difference between the two runs
//! is the type's cut.
//!
//! The typed defs live on this file's own scratch asset directory; nothing in
//! `assets/` sets a type or a resistance yet.

use super::support::*;
use crate::tactical::TacticalBattle;
use crate::*;

const THERMAL_LANCE: &str = r#"(id: "thermal_lance", name: "Thermal Lance", glyph: Some('/'),
    description: "test", value: Some(1),
    equipment: Some((Weapon, (damage: (min: 20, max: 20), damage_type: Some(Thermal)))))"#;
const STEEL_BLADE: &str = r#"(id: "steel_blade", name: "Steel Blade", glyph: Some('/'),
    description: "test", value: Some(1),
    equipment: Some((Weapon, (damage: (min: 20, max: 20)))))"#;
const HEAT_PLATE: &str = r#"(id: "heat_plate", name: "Heat Plate", glyph: Some('"'),
    description: "test", value: Some(1),
    equipment: Some((Armor, (resist: (thermal: 50)))))"#;
/// First move untyped, second typed: a body that swings without choosing a
/// move swings the first.
const TWIN: &str = r#"(id: "twin", name: "Twin", glyph: 't', color: Red,
    base_hp: 38, base_atk: 6, base_mitigation: 1, taming_difficulty: 0.15,
    habitats: [OpenGrid], base_speed: 11,
    moves: [(name: "Jab", power: 20, spread: 0),
        (name: "Cinder", power: 20, spread: 0, damage_type: Some(Thermal))],
    work_resource: Some("power_cell"),
    taunts: ["glows."],
    attributes: {"analysis": 5, "persistence": 26, "entropy": 78,
        "bandwidth": 52, "footprint": 22, "parity": 24})"#;
const PYRE: &str = r#"(id: "pyre", name: "Pyre", glyph: 'p', color: Red,
    base_hp: 38, base_atk: 6, base_mitigation: 1, taming_difficulty: 0.15,
    habitats: [OpenGrid], base_speed: 11,
    moves: [(name: "Cinder", power: 20, spread: 0, damage_type: Some(Thermal))],
    work_resource: Some("power_cell"),
    taunts: ["glows."],
    attributes: {"analysis": 5, "persistence": 26, "entropy": 78,
        "bandwidth": 52, "footprint": 22, "parity": 24})"#;

fn typed_game() -> Game {
    let dir = modded_assets_dir(
        "damage_types",
        &[],
        &[
            ("thermal_lance.ron", THERMAL_LANCE),
            ("steel_blade.ron", STEEL_BLADE),
            ("heat_plate.ron", HEAT_PLATE),
        ],
        &[("pyre.ron", PYRE), ("twin.ron", TWIN)],
        &[],
        &[],
    );
    Game::new(4200, DifficultyMode::Forgiving, &dir).unwrap()
}

fn hp(game: &Game, e: Entity) -> i32 {
    game.world.get::<Stats>(e).unwrap().hp
}

fn sturdy(game: &mut Game, e: Entity) {
    let mut s = game.world.get_mut::<Stats>(e).unwrap();
    s.hp = 5000;
    s.max_hp = 5000;
}

/// Who swings in an abstract round.
#[derive(Clone, Copy)]
enum Striker {
    /// The player, wielding this weapon (or bare-handed).
    Player(Option<&'static str>),
    /// A `pyre` — typed move, `Kit::Innate` — wielding this weapon.
    Pyre(Option<&'static str>),
}

/// The Integrity one abstract-round swing takes off a target that does, or
/// does not, wear `heat_plate`, on `stream`.
fn abstract_hit(striker: Striker, armoured: bool, stream: u64) -> i32 {
    let mut game = typed_game();
    let player = game.player_entity();
    let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    sturdy(&mut game, target);
    if armoured {
        equip_armor(&mut game, target, "heat_plate");
    }
    let (attacker, weapon) = match striker {
        Striker::Player(w) => (player, w),
        Striker::Pyre(w) => (game.spawn_wild_creature("pyre", 5, 5).unwrap(), w),
    };
    if let Some(w) = weapon {
        equip_weapon(&mut game, attacker, w);
    }
    insert_battle(&mut game, player, vec![target]);
    reseed_rng(&mut game, stream);
    let before = hp(&game, target);
    game.party_member_attacks(0, attacker, 0, player);
    before - hp(&game, target)
}

/// A stream on which the unarmoured swing lands hard enough for half of it to
/// be visible, and the damage that landing was.
fn landing_stream(striker: Striker) -> (u64, i32) {
    (0..512u64)
        .map(|s| (s, abstract_hit(striker, false, s)))
        .find(|&(_, dmg)| dmg >= 10)
        .expect("no stream in 0..512 landed the swing")
}

#[test]
fn a_typed_weapon_lands_less_on_matching_resistance_in_the_abstract_round() {
    let striker = Striker::Player(Some("thermal_lance"));
    let (stream, open) = landing_stream(striker);
    let resisted = abstract_hit(striker, true, stream);
    assert!(
        resisted < open,
        "the type never reached the swing: {resisted} against {open}"
    );
}

#[test]
fn an_untyped_blade_worn_over_typed_moves_stays_untyped() {
    let striker = Striker::Pyre(Some("steel_blade"));
    let (stream, open) = landing_stream(striker);
    assert_eq!(
        abstract_hit(striker, true, stream),
        open,
        "the blade took the move's type"
    );
}

#[test]
fn an_unarmed_companion_swings_its_moves_type() {
    let striker = Striker::Pyre(None);
    let (stream, open) = landing_stream(striker);
    let resisted = abstract_hit(striker, true, stream);
    assert!(
        resisted < open,
        "the move's type never reached the swing: {resisted} against {open}"
    );
}

#[test]
fn a_typed_weapon_lands_less_on_matching_resistance_on_the_battle_map() {
    let hit = |armoured: bool, stream: u64| {
        let mut game = typed_game();
        let pack = crate::tests::tactical::tactical_fight(&mut game, 1, 5000);
        let player = game.player_entity();
        let target = pack[0];
        if armoured {
            equip_armor(&mut game, target, "heat_plate");
        }
        equip_weapon(&mut game, player, "thermal_lance");
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![player, target]);
        let beside = {
            let battle = game.world.resource::<TacticalBattle>();
            let (px, py) = battle.cell_of(player).expect("the player is seated");
            (px + 1, py)
        };
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(target, beside),
            "fixture: the target must stand within reach"
        );
        reseed_rng(&mut game, stream);
        let before = hp(&game, target);
        assert!(game.tactical_attack(target), "the swing must happen");
        before - hp(&game, target)
    };
    let (stream, open) = (0..512u64)
        .map(|s| (s, hit(false, s)))
        .find(|&(_, dmg)| dmg >= 10)
        .expect("no stream in 0..512 landed the swing");
    let resisted = hit(true, stream);
    assert!(
        resisted < open,
        "the type never reached the swing: {resisted} against {open}"
    );
}

#[test]
fn a_species_move_ability_carries_its_type_through_use_ability() {
    let hit = |armoured: bool, stream: u64| {
        let mut game = typed_game();
        let player = game.player_entity();
        let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        sturdy(&mut game, target);
        if armoured {
            equip_armor(&mut game, target, "heat_plate");
        }
        let pyre = game.spawn_wild_creature("pyre", 5, 5).unwrap();
        insert_battle(&mut game, player, vec![target]);
        let ability = game
            .world
            .resource::<SpeciesDb>()
            .get("pyre")
            .unwrap()
            .basic_attacks()
            .remove(0);
        reseed_rng(&mut game, stream);
        let before = hp(&game, target);
        game.use_ability(&ability, pyre, "Pyre", &[target]);
        before - hp(&game, target)
    };
    let (stream, open) = (0..512u64)
        .map(|s| (s, hit(false, s)))
        .find(|&(_, dmg)| dmg >= 10)
        .expect("no stream in 0..512 landed the swing");
    let resisted = hit(true, stream);
    assert!(
        resisted < open,
        "the ability dropped its type: {resisted} against {open}"
    );
}

/// The Integrity a battle-map reaction by a `species` body takes off a mover
/// that does, or does not, wear `heat_plate`, on `stream`.
fn reaction_hit(species: &str, armoured: bool, stream: u64) -> i32 {
    let mut game = typed_game();
    let pack = crate::tests::tactical::tactical_fight(&mut game, 1, 5000);
    let player = game.player_entity();
    let reactor = pack[0];
    game.world.get_mut::<Creature>(reactor).unwrap().species = species.into();
    sturdy(&mut game, player);
    if armoured {
        equip_armor(&mut game, player, "heat_plate");
    }
    reseed_rng(&mut game, stream);
    let before = hp(&game, player);
    game.provoke(player, vec![reactor]);
    before - hp(&game, player)
}

#[test]
fn a_reaction_wears_the_type_of_the_band_it_swings() {
    // Pyre's first (and only) move is typed, so its reaction is.
    let (stream, open) = (0..512u64)
        .map(|s| (s, reaction_hit("pyre", false, s)))
        .find(|&(_, dmg)| dmg >= 10)
        .expect("no stream in 0..512 landed the reaction");
    assert!(reaction_hit("pyre", true, stream) < open);

    // Twin's first move is untyped. The reaction rolls a move for its name,
    // and on some streams that is the typed one; the damage stays untyped
    // because the band it swings is the first move's.
    let mut landed = 0;
    for stream in 0..64u64 {
        let open = reaction_hit("twin", false, stream);
        if open > 0 {
            landed += 1;
            assert_eq!(
                reaction_hit("twin", true, stream),
                open,
                "stream {stream}: the rolled move's type leaked into the reaction"
            );
        }
    }
    assert!(landed > 8, "too few reactions landed to mean anything");
}

#[test]
fn a_sortie_swing_by_an_unarmed_body_wears_its_first_moves_type() {
    let hit = |armoured: bool, stream: u64| {
        let mut game = typed_game();
        let actor = game.spawn_wild_creature("pyre", 5, 5).unwrap();
        let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        sturdy(&mut game, target);
        if armoured {
            equip_armor(&mut game, target, "heat_plate");
        }
        reseed_rng(&mut game, stream);
        let before = hp(&game, target);
        game.swing_for_the_squad(actor, target, &[target]);
        before - hp(&game, target)
    };
    let (stream, open) = (0..512u64)
        .map(|s| (s, hit(false, s)))
        .find(|&(_, dmg)| dmg >= 10)
        .expect("no stream in 0..512 landed the swing");
    let resisted = hit(true, stream);
    assert!(
        resisted < open,
        "the sortie swing dropped the type: {resisted} against {open}"
    );
}

// ---------------------------------------------------------------------------
// Display: the band and its type are one label, resistance reads in summaries.

#[test]
fn a_typed_band_names_its_type_and_an_untyped_one_is_unchanged() {
    let game = typed_game();
    let band = battle::DamageRange { min: 5, max: 10 };
    assert_eq!(
        game.damage_label(band, Some(damage::DamageType::Thermal)),
        "5–10 Thermal"
    );
    assert_eq!(game.damage_label(band, None), game.damage_range_label(band));
}

#[test]
fn a_typed_weapons_summary_names_the_type() {
    let game = typed_game();
    let mods = EquipmentStats {
        damage: battle::DamageRange { min: 20, max: 20 },
        atk: 3,
        damage_type: Some(damage::DamageType::Thermal),
        ..EquipmentStats::default()
    };
    assert_eq!(game.stat_summary(mods), "20 Thermal DMG +3 ATK");
}

#[test]
fn an_untyped_weapons_summary_is_what_it_always_was() {
    let game = typed_game();
    let mods = EquipmentStats {
        damage: battle::DamageRange { min: 14, max: 21 },
        atk: 3,
        ..EquipmentStats::default()
    };
    assert_eq!(game.stat_summary(mods), "14–21 DMG +3 ATK");
}

#[test]
fn armour_resistance_reads_after_the_stats_in_type_order() {
    let game = typed_game();
    let mods = EquipmentStats {
        mitigation: 4,
        resist: damage::Resistances {
            thermal: 30,
            voltage: -10,
            ..Default::default()
        },
        ..EquipmentStats::default()
    };
    assert_eq!(
        game.stat_summary(mods),
        "+4 MIT +30% Thermal resist -10% Voltage resist"
    );
}

#[test]
fn a_bodys_manifest_row_names_the_type_of_its_swing() {
    let mut game = typed_game();
    let pyre = game.spawn_wild_creature("pyre", 5, 5).unwrap();
    let row = game.manifest(pyre).unwrap().damage;
    assert!(row.ends_with(" Thermal"), "a program's row is {row:?}");

    let player = game.player_entity();
    equip_weapon(&mut game, player, "thermal_lance");
    assert_eq!(game.manifest(player).unwrap().damage, "20 Thermal");
    equip_weapon(&mut game, player, "steel_blade");
    assert_eq!(game.manifest(player).unwrap().damage, "20");
}

// ---------------------------------------------------------------------------
// The outcome carries what resistance stopped.

/// One 20-wide swing of `ty` at a sturdy target that does, or does not, wear
/// `heat_plate`, on `stream`.
fn typed_swing(
    ty: Option<damage::DamageType>,
    armoured: bool,
    stream: u64,
) -> battle::AttackOutcome {
    let mut game = typed_game();
    let player = game.player_entity();
    let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    sturdy(&mut game, target);
    if armoured {
        equip_armor(&mut game, target, "heat_plate");
    }
    let attacker = game.spawn_wild_creature("pyre", 5, 5).unwrap();
    insert_battle(&mut game, player, vec![target]);
    reseed_rng(&mut game, stream);
    let band = battle::DamageRange { min: 20, max: 20 };
    game.resolve_and_apply_attack(attacker, target, battle::Swing::plain(band).with_type(ty))
}

fn landed(o: battle::AttackOutcome) -> i32 {
    o.damage_to_defender()
}

fn resisted_of(o: battle::AttackOutcome) -> Option<damage::Resisted> {
    match o {
        battle::AttackOutcome::Hit { resisted, .. }
        | battle::AttackOutcome::Crit { resisted, .. } => resisted,
        _ => None,
    }
}

fn landing_swing_stream(ty: Option<damage::DamageType>) -> u64 {
    (0..512u64)
        .find(|&s| landed(typed_swing(ty, false, s)) >= 10)
        .expect("no stream in 0..512 landed the swing")
}

#[test]
fn a_typed_hit_into_matching_armour_reports_what_was_stopped() {
    let ty = Some(damage::DamageType::Thermal);
    let stream = landing_swing_stream(ty);
    let open = typed_swing(ty, false, stream);
    let cut = typed_swing(ty, true, stream);
    let note = resisted_of(cut).expect("the cut was not reported");
    assert_eq!(note.ty, damage::DamageType::Thermal);
    assert!(note.amount > 0);
    assert_eq!(landed(open) - landed(cut), note.amount);
    assert_eq!(resisted_of(open), None);
}

#[test]
fn an_untyped_hit_and_a_typed_hit_into_no_resistance_report_nothing() {
    let stream = landing_swing_stream(None);
    assert!(landed(typed_swing(None, true, stream)) > 0);
    assert_eq!(resisted_of(typed_swing(None, true, stream)), None);

    // Voltage meets a plate that resists only Thermal.
    let ty = Some(damage::DamageType::Voltage);
    let stream = landing_swing_stream(ty);
    assert!(landed(typed_swing(ty, true, stream)) > 0);
    assert_eq!(resisted_of(typed_swing(ty, true, stream)), None);
}
