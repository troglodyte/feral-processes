//! A Shield softens a fought siege only under its circle: hits on bodies
//! and on structures, and the one coil it spends at the end.

use super::*;
use crate::resources::TacticalFxKind;
use crate::tactical::TacticalBattle;
use crate::tactical::map::{BattleCell, BattleSpec, Board};
use crate::world::Biome;

/// Field coordinates map to the board by this shift.
const ORIGIN: (i32, i32) = (2, 3);
const SHIELD_AT: (i32, i32) = (10, 10);
const INSIDE: (i32, i32) = (11, 10);
const OUTSIDE: (i32, i32) = (25, 10);

fn game() -> Game {
    Game::new(4302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn on_board(field: (i32, i32)) -> (i32, i32) {
    (field.0 - ORIGIN.0, field.1 - ORIGIN.1)
}

fn coil() -> ItemId {
    ItemId::from(crate::items::ids::CHARGE_COIL)
}

/// An open 30x30 siege board whose `(0, 0)` is field `ORIGIN`.
fn open_siege(game: &mut Game) {
    let mut board = Board::solid(30);
    for y in 0..30 {
        for x in 0..30 {
            board.set(x, y, BattleCell::Open);
        }
    }
    let spec = BattleSpec {
        world_seed: 1,
        site: (0, 0),
        tick: 0,
        zone: 1,
        biome: Biome::OpenGrid,
        bodies: 1,
    };
    let mut battle = TacticalBattle::open(spec, board);
    battle.siege_pack = 1;
    battle.siege_origin = ORIGIN;
    let player = game.player_entity();
    battle.place(player, (29, 29));
    game.world.insert_resource(battle);
}

/// A Shield at `at` (field coordinates) holding `coils`, seated on the board.
fn shield_at(game: &mut Game, at: (i32, i32), coils: u32) -> Entity {
    let mut stock = Stock::new(10);
    if coils > 0 {
        stock.input.insert(coil(), coils);
    }
    let shield = spawn_structure_at(game, "shield", at.0, at.1);
    game.world
        .entity_mut(shield)
        .insert((stock, Durability { hp: 50, max_hp: 50 }));
    game.world
        .resource_mut::<TacticalBattle>()
        .place(shield, on_board(at));
    shield
}

fn coils_in(game: &Game, shield: Entity) -> u32 {
    game.world
        .get::<Stock>(shield)
        .and_then(|s| s.input.get(&coil()).copied())
        .unwrap_or(0)
}

/// A plain body (no mitigation) seated at field `at`.
fn body_at(game: &mut Game, at: (i32, i32), hostile: bool) -> Entity {
    let mut e = game.world.spawn(Stats {
        hp: 1000,
        max_hp: 1000,
        atk: 0,
        mitigation: 0,
    });
    if hostile {
        e.insert(Hostile);
    }
    let e = e.id();
    game.world
        .resource_mut::<TacticalBattle>()
        .place(e, on_board(at));
    e
}

fn hit(game: &mut Game, e: Entity, dmg: i32) -> i32 {
    game.apply_damage_typed(e, dmg, None)
}

fn shielded_cues(game: &mut Game) -> usize {
    game.take_tactical_fx()
        .iter()
        .filter(|c| matches!(c.kind, TacticalFxKind::Shielded { .. }))
        .count()
}

#[test]
fn a_staff_hit_under_a_shield_loses_two() {
    let mut game = game();
    open_siege(&mut game);
    shield_at(&mut game, SHIELD_AT, 3);
    let inside = body_at(&mut game, INSIDE, false);
    let outside = body_at(&mut game, OUTSIDE, false);
    let a = hit(&mut game, outside, 10);
    let b = hit(&mut game, inside, 10);
    assert_eq!((a, b), (10, 8));
}

#[test]
fn a_shield_cut_never_drops_a_hit_below_one() {
    let mut game = game();
    open_siege(&mut game);
    shield_at(&mut game, SHIELD_AT, 3);
    shield_at(&mut game, (10, 11), 3);
    let inside = body_at(&mut game, INSIDE, false);
    assert_eq!(hit(&mut game, inside, 2), 1);
}

#[test]
fn shield_cut_shares_the_deflection_cap() {
    use crate::tuning::DEFLECTION_MAX;
    let plate = format!(
        r#"(id: "cap_plate", name: "Cap Plate", glyph: Some('"'), description: "test", value: Some(1),
        equipment: Some((Armor, (deflection: {DEFLECTION_MAX}))))"#
    );
    let dir = modded_assets_dir(
        "shield_cap",
        &[],
        &[("cap_plate.ron", &plate)],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(4303, DifficultyMode::Forgiving, &dir).unwrap();
    open_siege(&mut game);
    shield_at(&mut game, SHIELD_AT, 3);
    let inside = body_at(&mut game, INSIDE, false);
    let outside = body_at(&mut game, OUTSIDE, false);
    equip_armor(&mut game, inside, "cap_plate");
    equip_armor(&mut game, outside, "cap_plate");
    let a = hit(&mut game, outside, 20);
    let b = hit(&mut game, inside, 20);
    assert_eq!(a, 20 - DEFLECTION_MAX);
    assert_eq!(b, a, "the Shield's cut shares the deflection cap");
}

#[test]
fn a_besieger_under_the_circle_is_not_shielded() {
    let mut game = game();
    open_siege(&mut game);
    shield_at(&mut game, SHIELD_AT, 3);
    let raider = body_at(&mut game, INSIDE, true);
    assert_eq!(hit(&mut game, raider, 10), 10);
}

#[test]
fn an_unarmed_shield_softens_nothing_in_a_siege() {
    let mut game = game();
    open_siege(&mut game);
    shield_at(&mut game, SHIELD_AT, 0);
    let inside = body_at(&mut game, INSIDE, false);
    game.take_tactical_fx();
    assert_eq!(hit(&mut game, inside, 10), 10);
    assert_eq!(shielded_cues(&mut game), 0);
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .shields_softened
            .is_empty()
    );
}

#[test]
fn a_softened_hit_flashes_and_is_recorded() {
    let mut game = game();
    open_siege(&mut game);
    shield_at(&mut game, SHIELD_AT, 3);
    let inside = body_at(&mut game, INSIDE, false);
    game.take_tactical_fx();
    hit(&mut game, inside, 10);
    let fx = game.take_tactical_fx();
    assert!(
        fx.iter().any(|c| c.pos == on_board(SHIELD_AT)
            && matches!(c.kind, TacticalFxKind::Shielded { radius: 4 })),
        "no flash at the board anchor"
    );
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .shields_softened
            .contains(&SHIELD_AT)
    );
}

#[test]
fn a_typed_hit_under_a_shield_keeps_the_two_mitigate_doors_consistent() {
    let mut game = game();
    open_siege(&mut game);
    shield_at(&mut game, SHIELD_AT, 3);
    let inside = body_at(&mut game, INSIDE, false);
    let typed = Some(crate::damage::DamageType::Thermal);
    assert_eq!(game.mitigate_incoming_damage_typed(inside, 10, typed), 8);
    assert_eq!(game.mitigate_incoming_damage(inside, 10), 8);
}

#[test]
fn no_shield_cut_outside_a_siege() {
    let mut game = game();
    open_siege(&mut game);
    game.world.resource_mut::<TacticalBattle>().siege_pack = 0;
    shield_at(&mut game, SHIELD_AT, 3);
    let inside = body_at(&mut game, INSIDE, false);
    assert_eq!(hit(&mut game, inside, 10), 10);
}

#[test]
fn a_shield_destroyed_mid_siege_stops_protecting() {
    let mut game = game();
    open_siege(&mut game);
    let shield = shield_at(&mut game, SHIELD_AT, 3);
    let inside = body_at(&mut game, INSIDE, false);
    assert_eq!(hit(&mut game, inside, 10), 8);
    game.damage_structure(shield, 500, "Shield", "a siege");
    assert!(game.world.get::<Structure>(shield).is_none());
    game.world.resource_mut::<TacticalBattle>().remove(shield);
    assert_eq!(hit(&mut game, inside, 10), 10);
    assert_eq!(coils_in(&game, shield), 0);
}
