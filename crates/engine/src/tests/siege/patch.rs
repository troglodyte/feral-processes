//! A Patch Node heals what its circle covers, every
//! `PATCH_SIEGE_PULSE_ROUNDS` rounds of a fought siege: bodies and board
//! structures alike, never a besieger.

use super::shields::{INSIDE, OUTSIDE, SHIELD_AT as NODE_AT, body_at, durability, game, on_board};
use super::shields::{open_siege, wall_at};
use super::*;
use crate::resources::TacticalFxKind;
use crate::tactical::TacticalBattle;
use crate::tuning::{PATCH_SIEGE_HEAL, PATCH_SIEGE_PULSE_ROUNDS};

fn node_at(game: &mut Game, at: (i32, i32)) -> Entity {
    let node = spawn_structure_at(game, "patch_node", at.0, at.1);
    game.world
        .entity_mut(node)
        .insert(Durability { hp: 50, max_hp: 50 });
    game.world
        .resource_mut::<TacticalBattle>()
        .place(node, on_board(at));
    node
}

fn hurt(game: &mut Game, body: Entity, hp: i32) {
    game.world.get_mut::<Stats>(body).unwrap().hp = hp;
}

fn hp(game: &Game, body: Entity) -> i32 {
    game.world.get::<Stats>(body).unwrap().hp
}

fn set_round(game: &mut Game, round: u32) {
    game.world.resource_mut::<TacticalBattle>().round = round;
}

fn heal_cues(game: &mut Game) -> usize {
    game.take_tactical_fx()
        .iter()
        .filter(|c| matches!(c.kind, TacticalFxKind::Heal(_)))
        .count()
}

#[test]
fn a_patch_node_heals_a_staff_body_in_its_circle_on_a_pulse_round() {
    let mut game = game();
    open_siege(&mut game);
    node_at(&mut game, NODE_AT);
    let inside = body_at(&mut game, INSIDE, false);
    let outside = body_at(&mut game, OUTSIDE, false);
    hurt(&mut game, inside, 10);
    hurt(&mut game, outside, 10);

    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS + 1);
    game.pulse_patch_nodes();
    assert_eq!(hp(&game, inside), 10, "an off round pulses nothing");

    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS);
    game.pulse_patch_nodes();
    assert_eq!(hp(&game, inside), 10 + PATCH_SIEGE_HEAL as i32);
    assert_eq!(hp(&game, outside), 10, "outside the circle");
}

#[test]
fn the_round_upkeep_pulses_the_nodes() {
    let mut game = game();
    open_siege(&mut game);
    node_at(&mut game, NODE_AT);
    let inside = body_at(&mut game, INSIDE, false);
    hurt(&mut game, inside, 10);
    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS);

    game.tactical_round_upkeep();

    assert!(hp(&game, inside) >= 10 + PATCH_SIEGE_HEAL as i32);
}

#[test]
fn a_patch_node_heals_a_structure_on_the_board() {
    let mut game = game();
    open_siege(&mut game);
    node_at(&mut game, NODE_AT);
    let wall = wall_at(&mut game, (13, 10));
    game.world.get_mut::<Durability>(wall).unwrap().hp = 100;
    game.take_tactical_fx();
    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS);

    game.pulse_patch_nodes();

    assert_eq!(durability(&game, wall), 100 + PATCH_SIEGE_HEAL);
    let cues = game.take_tactical_fx();
    assert!(cues.iter().any(|c| {
        c.kind == TacticalFxKind::Heal(PATCH_SIEGE_HEAL as i32) && c.pos == on_board((13, 10))
    }));
}

#[test]
fn a_patch_node_never_heals_a_besieger() {
    let mut game = game();
    open_siege(&mut game);
    node_at(&mut game, NODE_AT);
    let raider = body_at(&mut game, INSIDE, true);
    hurt(&mut game, raider, 10);
    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS);

    game.pulse_patch_nodes();

    assert_eq!(hp(&game, raider), 10);
}

#[test]
fn two_patch_nodes_heal_twice() {
    let mut game = game();
    open_siege(&mut game);
    node_at(&mut game, NODE_AT);
    node_at(&mut game, (10, 11));
    let inside = body_at(&mut game, INSIDE, false);
    hurt(&mut game, inside, 10);
    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS);

    game.pulse_patch_nodes();

    assert_eq!(hp(&game, inside), 10 + 2 * PATCH_SIEGE_HEAL as i32);
}

#[test]
fn a_destroyed_patch_node_stops_pulsing() {
    let mut game = game();
    open_siege(&mut game);
    let node = node_at(&mut game, NODE_AT);
    let inside = body_at(&mut game, INSIDE, false);
    hurt(&mut game, inside, 10);
    game.world.resource_mut::<TacticalBattle>().remove(node);
    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS);

    game.pulse_patch_nodes();

    assert_eq!(hp(&game, inside), 10);
}

#[test]
fn a_full_body_under_a_patch_node_gets_no_heal_cue() {
    let mut game = game();
    open_siege(&mut game);
    node_at(&mut game, NODE_AT);
    body_at(&mut game, INSIDE, false);
    let full_wall = wall_at(&mut game, (13, 10));
    game.take_tactical_fx();
    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS);

    game.pulse_patch_nodes();

    assert_eq!(heal_cues(&mut game), 0);
    assert_eq!(durability(&game, full_wall), 200);
}

#[test]
fn nothing_pulses_outside_a_siege() {
    let mut game = game();
    open_siege(&mut game);
    node_at(&mut game, NODE_AT);
    let inside = body_at(&mut game, INSIDE, false);
    hurt(&mut game, inside, 10);
    game.world.resource_mut::<TacticalBattle>().siege_pack = 0;
    set_round(&mut game, PATCH_SIEGE_PULSE_ROUNDS);

    game.pulse_patch_nodes();

    assert_eq!(hp(&game, inside), 10);
}
