//! A damage type rides the attacker's swing and meets the target's
//! resistance. Every test here compares the *same* random stream with and
//! without the resisting armour, so the only difference between the two runs
//! is the type's cut.
//!
//! The typed defs live on this file's own scratch asset directory, so the
//! scenarios do not depend on which shipped content carries a type.

use super::support::*;
use crate::tactical::TacticalBattle;
use crate::*;

const THERMAL_LANCE: &str = r#"(id: "thermal_lance", name: "Thermal Lance", glyph: Some('/'),
    description: "test", value: Some(1),
    equipment: Some((Weapon, (damage: (min: 20, max: 20), damage_type: Some(Thermal)))))"#;
const NOISE_HORN: &str = r#"(id: "noise_horn", name: "Noise Horn", glyph: Some('/'),
    description: "test", value: Some(1),
    equipment: Some((Weapon, (damage: (min: 20, max: 20), damage_type: Some(Noise)))))"#;
const NOISE_PLATE: &str = r#"(id: "noise_plate", name: "Noise Plate", glyph: Some('"'),
    description: "test", value: Some(1),
    equipment: Some((Armor, (resist: (noise: 50)))))"#;
const RAD_EMITTER: &str = r#"(id: "rad_emitter", name: "Rad Emitter", glyph: Some('/'),
    description: "test", value: Some(1),
    equipment: Some((Weapon, (damage: (min: 20, max: 20), damage_type: Some(Radiation)))))"#;
const RAD_PLATE: &str = r#"(id: "rad_plate", name: "Rad Plate", glyph: Some('"'),
    description: "test", value: Some(1),
    equipment: Some((Armor, (resist: (radiation: 50)))))"#;
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
            ("noise_horn.ron", NOISE_HORN),
            ("noise_plate.ron", NOISE_PLATE),
            ("rad_emitter.ron", RAD_EMITTER),
            ("rad_plate.ron", RAD_PLATE),
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
        "+4 MIT +30 Thermal RES -10 Voltage RES"
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
// The log names the type when resistance cut the hit, on every path.

const NOTE: &str = "stopped by Thermal resist)";

fn log_texts(game: &Game) -> Vec<String> {
    game.world
        .resource::<MessageLog>()
        .lines
        .iter()
        .map(|l| l.text.clone())
        .collect()
}

fn is_crit_line(l: &str) -> bool {
    l.contains("straight through") || l.contains("overruns")
}

/// The verbs of a landed plain hit on each path; a fumble line shares none.
fn is_plain_hit_line(l: &str) -> bool {
    l.contains(" inject") || l.contains("lands on")
}

/// Runs `scenario` on the first 512 streams, armoured and not. Every landed
/// line armoured must carry the note, and a plain hit and a crit must each
/// have been seen doing so; unarmoured, none may.
fn assert_note_in_log(scenario: impl Fn(bool, u64) -> Vec<String>) {
    let (mut hit, mut crit) = (false, false);
    for stream in 0..512u64 {
        let open = scenario(false, stream);
        assert!(
            !open.iter().any(|l| l.contains(NOTE)),
            "stream {stream}: an uncut hit named a resistance: {open:?}"
        );
        for line in scenario(true, stream) {
            let landed = is_crit_line(&line) || is_plain_hit_line(&line);
            if landed {
                assert!(line.contains(NOTE), "stream {stream}: {line:?}");
                hit |= is_plain_hit_line(&line);
                crit |= is_crit_line(&line);
            }
        }
    }
    assert!(hit, "no stream landed a plain hit");
    assert!(crit, "no stream landed a crit");
}

#[test]
fn the_abstract_party_swing_line_names_what_was_stopped() {
    assert_note_in_log(|armoured, stream| {
        let mut game = typed_game();
        let player = game.player_entity();
        let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        sturdy(&mut game, target);
        if armoured {
            equip_armor(&mut game, target, "heat_plate");
        }
        equip_weapon(&mut game, player, "thermal_lance");
        insert_battle(&mut game, player, vec![target]);
        reseed_rng(&mut game, stream);
        game.party_member_attacks(0, player, 0, player);
        log_texts(&game)
    });
}

/// The non-player arm of `party_swing_line`: a companion's swing, which is
/// also the arm a hostile's tactical attack and a reaction read through.
#[test]
fn a_companions_party_swing_line_names_what_was_stopped() {
    assert_note_in_log(|armoured, stream| {
        let mut game = typed_game();
        let player = game.player_entity();
        let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        sturdy(&mut game, target);
        if armoured {
            equip_armor(&mut game, target, "heat_plate");
        }
        let pyre = game.spawn_wild_creature("pyre", 5, 5).unwrap();
        insert_battle(&mut game, player, vec![target]);
        reseed_rng(&mut game, stream);
        game.party_member_attacks(0, pyre, 0, player);
        log_texts(&game)
    });
}

/// `wild_swings_once` has a line of its own when the target is a companion
/// rather than the player. Both bodies wear the plate so whoever it picks
/// the note is owed, and only the companion's lines are kept.
#[test]
fn the_wild_swing_line_at_a_companion_names_what_was_stopped() {
    assert_note_in_log(|armoured, stream| {
        let mut game = typed_game();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 5000, 1);
        enlist(&mut game, companion);
        let wild = game.spawn_wild_creature("pyre", 5, 5).unwrap();
        for body in [player, companion] {
            sturdy(&mut game, body);
            if armoured {
                equip_armor(&mut game, body, "heat_plate");
            }
        }
        insert_battle(&mut game, player, vec![wild]);
        reseed_rng(&mut game, stream);
        game.wild_retaliate(wild, 0, player);
        log_texts(&game)
            .into_iter()
            .filter(|l| l.contains(" into ") || l.contains("unchecked on"))
            .collect()
    });
}

#[test]
fn the_tactical_swing_line_names_what_was_stopped() {
    assert_note_in_log(|armoured, stream| {
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
        assert!(game.tactical_attack(target), "the swing must happen");
        log_texts(&game)
    });
}

#[test]
fn the_ability_line_names_what_was_stopped() {
    assert_note_in_log(|armoured, stream| {
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
        game.use_ability(&ability, pyre, "Pyre", &[target]);
        log_texts(&game)
    });
}

#[test]
fn the_wild_swing_line_names_what_was_stopped() {
    assert_note_in_log(|armoured, stream| {
        let mut game = typed_game();
        let player = game.player_entity();
        let wild = game.spawn_wild_creature("pyre", 5, 5).unwrap();
        sturdy(&mut game, player);
        if armoured {
            equip_armor(&mut game, player, "heat_plate");
        }
        insert_battle(&mut game, player, vec![wild]);
        reseed_rng(&mut game, stream);
        game.wild_retaliate(wild, 0, player);
        log_texts(&game)
    });
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

// ---------------------------------------------------------------------------
// Noise jams the target's aim.

fn is_jammed(game: &Game, e: Entity) -> bool {
    game.world
        .get::<StatusEffects>(e)
        .is_some_and(|s| s.active.iter().any(|a| a.id.0 == "jammed"))
}

/// What one Noise swing did to a sturdy target that does, or does not, wear
/// `noise_plate`, on `stream`: the Integrity it lost and whether it is jammed.
/// `battle_map` picks the model; the abstract round otherwise.
fn noise_swing(battle_map: bool, armoured: bool, stream: u64) -> (i32, bool) {
    let mut game = typed_game();
    let player = game.player_entity();
    equip_weapon(&mut game, player, "noise_horn");
    if battle_map {
        let pack = crate::tests::tactical::tactical_fight(&mut game, 1, 5000);
        let target = pack[0];
        if armoured {
            equip_armor(&mut game, target, "noise_plate");
        }
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
        (before - hp(&game, target), is_jammed(&game, target))
    } else {
        let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        sturdy(&mut game, target);
        if armoured {
            equip_armor(&mut game, target, "noise_plate");
        }
        insert_battle(&mut game, player, vec![target]);
        reseed_rng(&mut game, stream);
        let before = hp(&game, target);
        game.party_member_attacks(0, player, 0, player);
        (before - hp(&game, target), is_jammed(&game, target))
    }
}

fn noise_hit_jams_blocks_and_miss_applies_nothing(battle_map: bool) {
    let (stream, _) = (0..512u64)
        .map(|s| (s, noise_swing(battle_map, false, s)))
        .find(|&(_, (dmg, _))| dmg >= 10)
        .expect("no stream in 0..512 landed the swing");
    assert!(
        noise_swing(battle_map, false, stream).1,
        "a landed hit jams"
    );
    let (dmg, jammed) = noise_swing(battle_map, true, stream);
    assert!(dmg > 0, "the plate must not turn the hit into a miss");
    assert!(!jammed, "50% noise resistance blocks the jam");
    let miss = (0..512u64)
        .map(|s| noise_swing(battle_map, false, s))
        .find(|&(dmg, _)| dmg == 0)
        .expect("no stream in 0..512 missed");
    assert!(!miss.1, "a miss applies nothing");
}

#[test]
fn a_noise_hit_jams_in_the_abstract_round() {
    noise_hit_jams_blocks_and_miss_applies_nothing(false);
}

#[test]
fn a_noise_hit_jams_on_the_battle_map() {
    noise_hit_jams_blocks_and_miss_applies_nothing(true);
}

#[test]
fn a_jammed_attackers_accuracy_is_cut() {
    let mut game = typed_game();
    let wild = game.spawn_wild_creature("pyre", 5, 5).unwrap();
    let swing = battle::Swing::plain(battle::DamageRange { min: 1, max: 1 });
    let before = game.combatant_profile(wild, swing).accuracy;
    game.arm_status(
        wild,
        &StatusId::from("jammed"),
        crate::tuning::NOISE_JAM_ROUNDS,
        0,
    );
    let after = game.combatant_profile(wild, swing).accuracy;
    assert!(before > 0.0);
    let want = before * (100 - crate::tuning::NOISE_JAM_ACCURACY_CUT) as f64 / 100.0;
    assert!((after - want).abs() < 1e-9, "{after} against {want}");
}

#[test]
fn the_built_in_jammed_survives_a_save_and_load() {
    let mut game = typed_game();
    let wild = game.spawn_wild_creature("pyre", 5, 5).unwrap();
    let id = StatusId::from("jammed");
    game.arm_status(wild, &id, 2, 0);
    assert!(is_jammed(&game, wild));
    let path =
        std::env::temp_dir().join(format!("feral_processes_jammed_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(
        loaded.world.resource::<StatusDb>().contains(&id),
        "the load path must define the built-in"
    );
    // Statuses are scoped to one intrusion and are not written to a save, so
    // the status itself is gone; arming it again on the loaded game works.
    let again = loaded.spawn_wild_creature("glitch", 5, 5).unwrap();
    loaded.arm_status(again, &id, 2, 0);
    assert!(is_jammed(&loaded, again));
}

// ---------------------------------------------------------------------------
// Radiation corrupts the target.

fn is_corrupted(game: &Game, e: Entity) -> bool {
    game.world
        .get::<StatusEffects>(e)
        .is_some_and(|s| s.active.iter().any(|a| a.id.0 == "corrupted"))
}

/// `noise_swing` for a Radiation swing: the Integrity lost and whether the
/// target is corrupted.
fn radiation_swing(battle_map: bool, armoured: bool, stream: u64) -> (i32, bool) {
    let mut game = typed_game();
    let player = game.player_entity();
    equip_weapon(&mut game, player, "rad_emitter");
    if battle_map {
        let pack = crate::tests::tactical::tactical_fight(&mut game, 1, 5000);
        let target = pack[0];
        if armoured {
            equip_armor(&mut game, target, "rad_plate");
        }
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
        (before - hp(&game, target), is_corrupted(&game, target))
    } else {
        let target = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        sturdy(&mut game, target);
        if armoured {
            equip_armor(&mut game, target, "rad_plate");
        }
        insert_battle(&mut game, player, vec![target]);
        reseed_rng(&mut game, stream);
        let before = hp(&game, target);
        game.party_member_attacks(0, player, 0, player);
        (before - hp(&game, target), is_corrupted(&game, target))
    }
}

fn radiation_hit_corrupts_blocks_and_miss_applies_nothing(battle_map: bool) {
    let (stream, _) = (0..512u64)
        .map(|s| (s, radiation_swing(battle_map, false, s)))
        .find(|&(_, (dmg, _))| dmg >= 10)
        .expect("no stream in 0..512 landed the swing");
    assert!(
        radiation_swing(battle_map, false, stream).1,
        "a landed hit corrupts"
    );
    let (dmg, corrupted) = radiation_swing(battle_map, true, stream);
    assert!(dmg > 0, "the plate must not turn the hit into a miss");
    assert!(!corrupted, "50% radiation resistance blocks the corruption");
    let miss = (0..512u64)
        .map(|s| radiation_swing(battle_map, false, s))
        .find(|&(dmg, _)| dmg == 0)
        .expect("no stream in 0..512 missed");
    assert!(!miss.1, "a miss applies nothing");
}

#[test]
fn a_radiation_hit_corrupts_in_the_abstract_round() {
    radiation_hit_corrupts_blocks_and_miss_applies_nothing(false);
}

#[test]
fn a_radiation_hit_corrupts_on_the_battle_map() {
    radiation_hit_corrupts_blocks_and_miss_applies_nothing(true);
}

#[test]
fn corruption_costs_integrity_each_round_and_a_heal_still_lands() {
    let mut game = typed_game();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    sturdy(&mut game, wild);
    let id = StatusId::from("corrupted");
    game.arm_status(
        wild,
        &id,
        crate::tuning::RADIATION_CORRUPTION_ROUNDS,
        crate::tuning::RADIATION_CORRUPTION_POWER,
    );
    // The landing round is exempt; the next one charges.
    game.tick_status_effects(wild, "It");
    assert_eq!(hp(&game, wild), 5000);
    game.tick_status_effects(wild, "It");
    assert_eq!(
        hp(&game, wild),
        5000 - crate::tuning::RADIATION_CORRUPTION_POWER
    );
    assert!(is_corrupted(&game, wild));
    let hurt = hp(&game, wild);
    assert_eq!(
        game.restore_hp(wild, 1),
        1,
        "corruption does not block healing"
    );
    assert_eq!(hp(&game, wild), hurt + 1);
}

#[test]
fn corruption_lines_read_for_you_and_for_a_program() {
    let defs = crate::damage::statuses();
    let def = defs.iter().find(|d| d.id.0 == "corrupted").unwrap();
    for who in ["You", "Glitch"] {
        for line in [
            def.inflict_line(who),
            def.tick_line(who, 2),
            def.expire_line(who),
        ] {
            assert!(line.contains(who) && !line.contains('{'), "{line}");
            assert!(!line.contains("'s") && !line.contains("loses"), "{line}");
        }
    }
}
