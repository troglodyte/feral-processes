//! Damage types, Stage 4: a species' own resistance, summed with armour
//! before the cap (`Game::type_resistances`).

use super::support::*;
use crate::*;

/// Resists Thermal and Noise by species alone, below the block threshold.
const HUSK: &str = r#"(id: "husk", name: "Husk", glyph: 'h', color: Red,
    base_hp: 38, base_atk: 6, base_mitigation: 1, taming_difficulty: 0.15,
    habitats: [OpenGrid], base_speed: 11,
    moves: [(name: "Jab", power: 20, spread: 0)],
    resist: (thermal: 30, noise: 30),
    work_resource: Some("power_cell"),
    taunts: ["rattles."],
    attributes: {"analysis": 5, "persistence": 26, "entropy": 78,
        "bandwidth": 52, "footprint": 22, "parity": 24})"#;
const HEAT_PLATE: &str = r#"(id: "heat_plate", name: "Heat Plate", glyph: Some('"'),
    description: "test", value: Some(1),
    equipment: Some((Armor, (resist: (thermal: 60)))))"#;
const HUM_PLATE: &str = r#"(id: "hum_plate", name: "Hum Plate", glyph: Some('"'),
    description: "test", value: Some(1),
    equipment: Some((Armor, (resist: (noise: 20)))))"#;

fn husk_game() -> Game {
    let dir = modded_assets_dir(
        "damage_species",
        &[],
        &[("heat_plate.ron", HEAT_PLATE), ("hum_plate.ron", HUM_PLATE)],
        &[("husk.ron", HUSK)],
        &[],
        &[],
    );
    Game::new(4200, DifficultyMode::Forgiving, &dir).unwrap()
}

fn sturdy(game: &mut Game, e: Entity) {
    let mut s = game.world.get_mut::<Stats>(e).unwrap();
    s.hp = 5000;
    s.max_hp = 5000;
}

/// One 20-wide swing of `ty` from a glitch at a sturdy `species`, optionally
/// wearing `armour`, on `stream`. Returns the outcome and the game after it.
fn swing_at(
    species: &str,
    armour: Option<&str>,
    ty: Option<damage::DamageType>,
    stream: u64,
) -> (battle::AttackOutcome, Game, Entity) {
    let mut game = husk_game();
    let player = game.player_entity();
    let target = game.spawn_wild_creature(species, 5, 5).unwrap();
    sturdy(&mut game, target);
    if let Some(item) = armour {
        equip_armor(&mut game, target, item);
    }
    let attacker = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    insert_battle(&mut game, player, vec![target]);
    reseed_rng(&mut game, stream);
    let band = battle::DamageRange { min: 20, max: 20 };
    let outcome =
        game.resolve_and_apply_attack(attacker, target, battle::Swing::plain(band).with_type(ty));
    (outcome, game, target)
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

/// A stream on which the swing lands well on a worm, which resists nothing.
fn landing_stream(ty: Option<damage::DamageType>) -> u64 {
    (0..512u64)
        .find(|&s| landed(swing_at("worm", None, ty, s).0) >= 10)
        .expect("no stream in 0..512 landed the swing")
}

#[test]
fn a_species_resistance_cuts_a_typed_hit_and_reports_it() {
    let ty = Some(damage::DamageType::Thermal);
    let stream = landing_stream(ty);
    let (open, ..) = swing_at("worm", None, ty, stream);
    let (cut, ..) = swing_at("husk", None, ty, stream);
    let note = resisted_of(cut).expect("the species cut was not reported");
    assert_eq!(note.ty, damage::DamageType::Thermal);
    assert!(note.amount > 0);
    assert!(landed(cut) < landed(open));
}

#[test]
fn an_untyped_hit_ignores_a_species_resistance() {
    let stream = landing_stream(None);
    let (hit, ..) = swing_at("husk", None, None, stream);
    assert!(landed(hit) > 0);
    assert_eq!(resisted_of(hit), None);
}

#[test]
fn species_and_armour_resistance_sum_before_the_cap() {
    let mut game = husk_game();
    let husk = game.spawn_wild_creature("husk", 5, 5).unwrap();
    equip_armor(&mut game, husk, "heat_plate");
    let res = game.type_resistances(husk);
    assert_eq!(
        res.thermal, 90,
        "30 species + 60 armour, uncapped in the sum"
    );
    assert_eq!(
        damage::resist_against(res, Some(damage::DamageType::Thermal)),
        tuning::DAMAGE_RESIST_CAP
    );
}

fn is_jammed(game: &Game, e: Entity) -> bool {
    game.world
        .get::<StatusEffects>(e)
        .is_some_and(|s| s.active.iter().any(|a| a.id.0 == "jammed"))
}

/// Species 30 alone and armour 20 alone each let the jam land; together they
/// reach the threshold and block it.
#[test]
fn species_and_armour_together_block_a_side_effect() {
    let ty = Some(damage::DamageType::Noise);
    let stream = landing_stream(ty);
    let (hit, game, target) = swing_at("husk", None, ty, stream);
    assert!(landed(hit) > 0);
    assert!(is_jammed(&game, target), "species alone does not block");
    let (hit, game, target) = swing_at("worm", Some("hum_plate"), ty, stream);
    assert!(landed(hit) > 0);
    assert!(is_jammed(&game, target), "armour alone does not block");
    let (hit, game, target) = swing_at("husk", Some("hum_plate"), ty, stream);
    assert!(landed(hit) > 0);
    assert!(!is_jammed(&game, target), "30 + 20 reaches the threshold");
}

#[test]
fn an_emulating_player_borrows_the_species_resistance() {
    let mut game = husk_game();
    let player = game.player_entity();
    assert_eq!(game.type_resistances(player).thermal, 0);
    game.world.entity_mut(player).insert(Emulation {
        species: SpeciesId::from("husk"),
        rounds_left: 3,
    });
    assert_eq!(game.type_resistances(player).thermal, 30);
}

#[test]
fn a_programs_manifest_lists_its_species_resistances_and_not_its_armour() {
    let mut game = husk_game();
    let husk = game.spawn_wild_creature("husk", 5, 5).unwrap();
    equip_armor(&mut game, husk, "hum_plate");
    let worm = game.spawn_wild_creature("worm", 5, 5).unwrap();
    let resistances = |e| {
        let view = game.manifest(e).expect("a manifest");
        let ManifestSubject::Program(p) = view.subject else {
            panic!("a program's manifest");
        };
        p.resistances
    };
    assert_eq!(
        resistances(husk),
        vec![
            (damage::DamageType::Thermal, 30),
            (damage::DamageType::Noise, 30)
        ]
    );
    assert!(resistances(worm).is_empty());
}
