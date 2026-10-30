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
    open_siege_with_player(game, (29, 29));
}

/// `open_siege` with the player seated at board cell `player_at`.
fn open_siege_with_player(game: &mut Game, player_at: (i32, i32)) {
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
    battle.place(player, player_at);
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
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .shields_softened
            .is_empty(),
        "a hit the cap left uncut does not charge the Shield"
    );
}

#[test]
fn a_deflection_ignoring_hit_does_not_charge_the_shield() {
    let mut game = game();
    open_siege(&mut game);
    let shield = shield_at(&mut game, SHIELD_AT, 3);
    let inside = body_at(&mut game, INSIDE, false);
    game.take_tactical_fx();
    let magnetic = Some(crate::damage::DamageType::Magnetic);
    assert_eq!(game.apply_damage_typed(inside, 10, magnetic), 10);
    assert_eq!(shielded_cues(&mut game), 0);
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .shields_softened
            .is_empty()
    );
    finish(&mut game, true);
    assert_eq!(coils_in(&game, shield), 3);
}

#[test]
fn a_structure_hit_already_at_one_does_not_charge_the_shield() {
    let mut game = game();
    open_siege(&mut game);
    shield_at(&mut game, SHIELD_AT, 3);
    let wall = wall_at(&mut game, WALL_IN);
    assert_eq!(game.shielded_structure_damage(wall, 1), 1);
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .shields_softened
            .is_empty()
    );
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

// ---------------------------------------------------------------------
// Structure hits, the coil at the end, and persistence.

use crate::game::combat_teardown::FightVerdict;

const WALL_IN: (i32, i32) = (13, 10);
const WALL_OUT: (i32, i32) = (25, 20);

fn wall_at(game: &mut Game, at: (i32, i32)) -> Entity {
    let wall = spawn_structure_at(game, "depot", at.0, at.1);
    game.world.entity_mut(wall).insert(Durability {
        hp: 200,
        max_hp: 200,
    });
    game.world
        .resource_mut::<TacticalBattle>()
        .place(wall, on_board(at));
    wall
}

fn durability(game: &Game, e: Entity) -> u32 {
    game.world.get::<Durability>(e).unwrap().hp
}

fn creature_with_atk(game: &mut Game, at: (i32, i32), atk: i32, hostile: bool) -> Entity {
    let e = body_at(game, at, hostile);
    game.world.get_mut::<Stats>(e).unwrap().atk = atk;
    game.world.entity_mut(e).insert(Creature {
        species: GENERIC_SPECIES_ID.to_string(),
    });
    e
}

/// Durability lost by a wall at `wall` to one `tactical_attack` from an
/// adjacent actor.
fn swing_loss(wall: (i32, i32), hostile: bool, atk: i32) -> u32 {
    let mut game = game();
    let beside = (wall.0, wall.1 + 1);
    // The player stands on the actor's cell unless a hostile takes it.
    open_siege_with_player(&mut game, if hostile { (29, 29) } else { on_board(beside) });
    shield_at(&mut game, SHIELD_AT, 3);
    let wall = wall_at(&mut game, wall);
    let player = game.player_entity();
    let actor = if hostile {
        creature_with_atk(&mut game, beside, atk, true)
    } else {
        game.world.get_mut::<Stats>(player).unwrap().atk = atk;
        player
    };
    let mut battle = game.world.resource_mut::<TacticalBattle>();
    battle.set_initiative(vec![actor, player]);
    battle.set_actions_left(2);
    assert!(
        game.tactical_attack(wall),
        "the swing must land on the wall"
    );
    200 - durability(&game, wall)
}

#[test]
fn a_structure_hit_under_a_shield_loses_two() {
    let out = swing_loss(WALL_OUT, true, 20);
    let inside = swing_loss(WALL_IN, true, 20);
    assert!(out > 2);
    assert_eq!(inside, out - 2);
}

#[test]
fn a_structure_hit_is_never_cut_to_zero() {
    // The natural swing averages 5, so atk -3 lands a 2: all of it cut.
    assert_eq!(swing_loss(WALL_OUT, true, -3), 2);
    assert_eq!(swing_loss(WALL_IN, true, -3), 1);
}

#[test]
fn the_player_hitting_a_structure_is_not_shielded() {
    assert_eq!(
        swing_loss(WALL_IN, false, 20),
        swing_loss(WALL_OUT, false, 20)
    );
}

#[test]
fn a_besieger_wrecking_under_a_shield_loses_two() {
    let wreck = |wall: (i32, i32)| {
        let mut game = game();
        open_siege(&mut game);
        shield_at(&mut game, SHIELD_AT, 3);
        let wall = wall_at(&mut game, wall);
        let raider = creature_with_atk(&mut game, (30 - 1, 28), 20, true);
        let player = game.player_entity();
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        battle.set_initiative(vec![raider, player]);
        battle.set_actions_left(2);
        assert!(game.besieger_wreck(raider, wall));
        200 - durability(&game, wall)
    };
    assert_eq!(wreck(WALL_IN), wreck(WALL_OUT) - 2);
}

fn finish(game: &mut Game, won: bool) {
    let player = game.player_entity();
    let verdict = FightVerdict {
        won,
        rounds: 1,
        outmatched: false,
        lair: None,
    };
    game.finish_fight(player, None, verdict);
}

#[test]
fn a_shield_that_softened_spends_one_coil_at_the_end() {
    for won in [true, false] {
        let mut game = game();
        open_siege(&mut game);
        let shield = shield_at(&mut game, SHIELD_AT, 3);
        let inside = body_at(&mut game, INSIDE, false);
        for _ in 0..4 {
            hit(&mut game, inside, 10);
        }
        assert_eq!(coils_in(&game, shield), 3, "nothing is spent mid-siege");
        finish(&mut game, won);
        assert_eq!(coils_in(&game, shield), 2, "won: {won}");
    }
}

#[test]
fn a_shield_that_softened_nothing_spends_nothing() {
    let mut game = game();
    open_siege(&mut game);
    let shield = shield_at(&mut game, SHIELD_AT, 3);
    let outside = body_at(&mut game, OUTSIDE, false);
    hit(&mut game, outside, 10);
    finish(&mut game, true);
    assert_eq!(coils_in(&game, shield), 3);
}

#[test]
fn a_softening_shield_destroyed_before_the_end_spends_nothing_and_does_not_panic() {
    let mut game = game();
    open_siege(&mut game);
    let shield = shield_at(&mut game, SHIELD_AT, 3);
    let spare = shield_at(&mut game, (10, 12), 3);
    let inside = body_at(&mut game, INSIDE, false);
    hit(&mut game, inside, 10);
    game.damage_structure(shield, 500, "Shield", "a siege");
    game.world.resource_mut::<TacticalBattle>().remove(shield);
    finish(&mut game, true);
    assert!(game.world.get::<Structure>(shield).is_none());
    assert_eq!(coils_in(&game, spare), 2, "the survivor softened it too");
}

#[test]
fn softened_shields_survive_a_mid_siege_reload() {
    let mut game = game();
    game.lay_test_pocket();
    stand_in_base_at(&mut game, 0, 0);
    open_siege_with_player(&mut game, on_board((0, 0)));
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "shield")
        .unwrap();
    let shield = game.spawn_structure(&def, SHIELD_AT.0, SHIELD_AT.1, None);
    game.world
        .get_mut::<Stock>(shield)
        .unwrap()
        .input
        .insert(coil(), 3);
    game.world
        .resource_mut::<TacticalBattle>()
        .place(shield, on_board(SHIELD_AT));
    let staff = spawn_tamed(&mut game, 100, 3);
    {
        let mut pos = game.world.get_mut::<Position>(staff).unwrap();
        (pos.x, pos.y) = INSIDE;
    }
    game.world
        .resource_mut::<TacticalBattle>()
        .place(staff, on_board(INSIDE));
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, staff]);
    hit(&mut game, staff, 10);
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .shields_softened
            .contains(&SHIELD_AT)
    );

    let scratch = scratch_assets_dir("shield_softened_reload");
    std::fs::create_dir_all(&*scratch).unwrap();
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    assert!(loaded.in_tactical_battle(), "the siege must reload");
    assert!(
        loaded
            .world
            .resource::<TacticalBattle>()
            .shields_softened
            .contains(&SHIELD_AT),
        "the softened set must survive the reload"
    );
    let shield = loaded
        .world
        .query::<(Entity, &Structure)>()
        .iter(&loaded.world)
        .find(|(_, s)| s.kind == "shield")
        .map(|(e, _)| e)
        .expect("the shield reloaded");
    assert_eq!(coils_in(&loaded, shield), 3);
    finish(&mut loaded, true);
    assert_eq!(coils_in(&loaded, shield), 2, "exactly one coil, once");
}
