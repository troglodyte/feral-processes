//! Charge routines on the battle map: the root, the aim cells, the turn
//! shape and the auto-fire at full charge.

use crate::components::*;
use crate::resources::DifficultyMode;
use crate::tactical::TacticalBattle;
use crate::tactical::turn::StepOutcome;
use crate::tests::support::{force_the_next_attack_to_land, test_assets_dir};
use crate::tests::tactical::{free_neighbour, only_routine, tactical_fight, wait_for_turn};
use crate::{Game, StatusId};
use bevy_ecs::prelude::Entity;

use super::charge::{ID, TARGET_HP, charge_def, hp, on_cooldown, progress};

/// A player holding the charge routine, with one tough hostile standing
/// beside them and the turn the player's.
fn fight(rounds: u32, power_cost: u32) -> (Game, Entity, Entity) {
    let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::abilities::AbilityDb>()
        .insert(charge_def(40, rounds, 5, power_cost));
    let pack = tactical_fight(&mut game, 1, TARGET_HP);
    let wild = pack[0];
    let player = game.player_entity();
    only_routine(&mut game, player, ID);
    let mut stats = game.world.get_mut::<Stats>(player).unwrap();
    stats.hp = 100_000;
    stats.max_hp = 100_000;
    assert!(wait_for_turn(&mut game, player));
    let at = cell_of(&game, player);
    let beside = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(wild, beside)
    );
    (game, player, wild)
}

fn cell_of(game: &Game, body: Entity) -> (i32, i32) {
    game.world
        .resource::<TacticalBattle>()
        .cell_of(body)
        .expect("not on the board")
}

fn start_on(game: &mut Game, cell: (i32, i32)) {
    assert!(game.tactical_use_routine(0, cell), "the start was refused");
}

/// Back to the player's turn with the hostile having done nothing.
fn back_to(game: &mut Game, who: Entity) {
    assert!(wait_for_turn(game, who));
}

#[test]
fn starting_a_charge_lands_nothing_spends_power_and_ends_the_turn() {
    let (mut game, player, wild) = fight(3, 10);
    let power = game.world.get::<PowerReserve>(player).unwrap().get();
    let at = cell_of(&game, wild);

    start_on(&mut game, at);

    assert_eq!(progress(&game, player), Some(1));
    assert_eq!(hp(&game, wild), TARGET_HP);
    assert!(game.world.get::<PowerReserve>(player).unwrap().get() < power);
    assert!(!on_cooldown(&game, player), "the cooldown arms at the end");
    assert_ne!(game.tactical_actor(), Some(player), "the turn was kept");
}

#[test]
fn a_charger_cannot_step_bump_or_do_anything_else() {
    let (mut game, player, wild) = fight(3, 0);
    let at = cell_of(&game, wild);
    start_on(&mut game, at);
    back_to(&mut game, player);

    assert_eq!(game.movement_allowance(player), 0);
    for dir in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
        assert_eq!(game.tactical_step(dir), StepOutcome::Refused, "{dir:?}");
    }
    // The bump: a step straight into the hostile.
    let from = cell_of(&game, player);
    let dir = (at.0 - from.0, at.1 - from.1);
    assert_eq!(game.tactical_step(dir), StepOutcome::Refused);
    assert!(!game.tactical_attack(wild));
    assert!(!game.tactical_use_routine(0, at));
    assert_eq!(hp(&game, wild), TARGET_HP);
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "a refusal cost the turn"
    );
}

#[test]
fn hold_advances_and_release_fires_ends_the_charge_and_arms_the_cooldown() {
    let (mut game, player, wild) = fight(3, 0);
    let at = cell_of(&game, wild);
    start_on(&mut game, at);
    back_to(&mut game, player);

    assert!(game.tactical_charge_hold());
    assert_eq!(progress(&game, player), Some(2));
    assert_eq!(game.tactical_actor(), Some(wild), "hold hands on once");
    back_to(&mut game, player);

    force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_charge_release());
    assert_eq!(progress(&game, player), None);
    assert!(hp(&game, wild) < TARGET_HP, "the release landed nothing");
    assert!(on_cooldown(&game, player));
    assert_eq!(game.tactical_actor(), Some(wild), "release hands on once");
}

#[test]
fn the_player_doors_refuse_a_body_that_is_not_charging() {
    let (mut game, player, _) = fight(3, 0);
    assert!(!game.tactical_charge_hold());
    assert!(!game.tactical_charge_release());
    assert_eq!(game.tactical_actor(), Some(player));
}

#[test]
fn a_victim_who_steps_out_is_missed_and_one_who_steps_in_is_hit() {
    let (mut game, player, wild) = fight(3, 0);
    let aimed = cell_of(&game, wild);
    start_on(&mut game, aimed);
    back_to(&mut game, player);
    let away = free_neighbour(&game, aimed);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(wild, away)
    );
    force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_charge_release());
    assert_eq!(hp(&game, wild), TARGET_HP, "the victim had left the cells");

    let (mut game, player, wild) = fight(3, 0);
    let from = cell_of(&game, player);
    let wild_at = cell_of(&game, wild);
    let empty = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| (from.0 + dx, from.1 + dy))
        .find(|&c| {
            c != wild_at && {
                let b = game.world.resource::<TacticalBattle>();
                b.board.walkable(c.0, c.1) && b.occupant(c).is_none()
            }
        })
        .unwrap();
    start_on(&mut game, empty);
    back_to(&mut game, player);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(wild, empty)
    );
    force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_charge_release());
    assert!(hp(&game, wild) < TARGET_HP, "the victim stood in the cells");
}

#[test]
fn displacing_the_charger_keeps_the_aim_cells() {
    let (mut game, player, wild) = fight(3, 0);
    let aimed = cell_of(&game, wild);
    start_on(&mut game, aimed);
    back_to(&mut game, player);
    let from = cell_of(&game, player);
    let elsewhere = free_neighbour(&game, from);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, elsewhere)
    );
    let ChargeAim::Cells(cells) = game.world.get::<Charging>(player).unwrap().aim.clone() else {
        panic!("a battle-map charge locks cells")
    };
    assert_eq!(cells, vec![aimed]);
    force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_charge_release());
    assert!(hp(&game, wild) < TARGET_HP, "range is not rechecked");
}

#[test]
fn a_squad_charger_forfeits_its_remaining_actions() {
    let (mut game, player, wild) = fight(3, 0);
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);
    let at = cell_of(&game, wild);
    start_on(&mut game, at);
    assert_ne!(game.tactical_actor(), Some(player));
}

#[test]
fn full_charge_fires_on_arrival_for_a_party_body_and_hands_on_once() {
    let (mut game, player, wild) = fight(2, 0);
    let at = cell_of(&game, wild);
    start_on(&mut game, at);
    back_to(&mut game, player);
    assert!(game.tactical_charge_hold());
    assert_eq!(progress(&game, player), Some(2));
    force_the_next_attack_to_land(&mut game);

    // The hostile passes; the player's turn comes back as the auto-fire.
    game.tactical_end_turn();

    assert_eq!(progress(&game, player), None, "it did not fire on arrival");
    assert!(hp(&game, wild) < TARGET_HP);
    assert!(on_cooldown(&game, player));
    assert_eq!(
        game.tactical_actor(),
        Some(wild),
        "the turn was kept or doubled"
    );
}

#[test]
fn a_stun_cancels_a_battle_map_charge_and_arms_the_cooldown() {
    let (mut game, player, wild) = fight(3, 10);
    let at = cell_of(&game, wild);
    start_on(&mut game, at);
    game.arm_status(player, &StatusId::from("stun"), 1, 0);
    assert_eq!(progress(&game, player), None);
    assert!(on_cooldown(&game, player));
}

#[test]
fn the_charger_dying_mid_charge_leaves_the_cursor_on_the_next_body() {
    let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<crate::abilities::AbilityDb>()
        .insert(charge_def(40, 3, 5, 0));
    let pack = tactical_fight(&mut game, 2, TARGET_HP);
    let (charger, other) = (pack[0], pack[1]);
    let player = game.player_entity();
    only_routine(&mut game, charger, ID);
    game.world.get_mut::<Stats>(charger).unwrap().hp = 1;
    assert!(wait_for_turn(&mut game, charger));
    let at = cell_of(&game, player);
    let beside = crate::tests::tactical::free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(charger, beside)
    );
    assert!(game.tactical_ai_turn());
    assert!(game.world.get::<Charging>(charger).is_some());
    assert!(wait_for_turn(&mut game, player));
    let next = game
        .world
        .resource::<TacticalBattle>()
        .initiative()
        .to_vec();

    force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_attack(charger));

    assert!(!game.creature_alive(charger));
    let order = game
        .world
        .resource::<TacticalBattle>()
        .initiative()
        .to_vec();
    assert!(!order.contains(&charger));
    assert_eq!(order.len(), next.len() - 1);
    assert_ne!(game.tactical_actor(), Some(charger));
    assert!(
        game.tactical_actor()
            .is_some_and(|a| a == other || a == player)
    );
}

#[test]
fn ending_the_fight_clears_a_standing_charge() {
    let (mut game, player, wild) = fight(3, 0);
    let at = cell_of(&game, wild);
    start_on(&mut game, at);
    game.world.resource_mut::<TacticalBattle>().remove(wild);
    game.end_tactical_battle(None);
    assert!(!game.in_tactical_battle(), "the fight did not close");
    assert!(game.world.get::<Charging>(player).is_none());
}

#[test]
fn an_ai_charger_holds_on_its_cell_and_hands_on_exactly_once() {
    let (mut game, player, wild) = fight(3, 0);
    only_routine(&mut game, wild, ID);
    assert!(wait_for_turn(&mut game, wild));
    let cell = cell_of(&game, wild);

    assert!(game.tactical_ai_turn());
    assert_eq!(progress(&game, wild), Some(1));
    assert_eq!(game.tactical_actor(), Some(player), "handed on once");
    assert_eq!(cell_of(&game, wild), cell);
    game.tactical_end_turn();

    assert!(game.tactical_ai_turn());
    assert_eq!(progress(&game, wild), Some(2), "nothing to kill: hold");
    assert_eq!(cell_of(&game, wild), cell);
    assert_eq!(game.tactical_actor(), Some(player), "handed on once");
}

#[test]
fn an_ai_charger_with_nobody_left_in_its_aim_cancels() {
    let (mut game, player, wild) = fight(3, 0);
    only_routine(&mut game, wild, ID);
    assert!(wait_for_turn(&mut game, wild));
    assert!(game.tactical_ai_turn());
    assert!(game.world.get::<Charging>(wild).is_some());
    // The player walks out of the aimed cell.
    let from = cell_of(&game, player);
    let away = {
        let b = game.world.resource::<TacticalBattle>();
        b.board
            .cells()
            .map(|(c, _)| c)
            .find(|&c| {
                b.board.walkable(c.0, c.1)
                    && b.occupant(c).is_none()
                    && crate::tactical::reach::distance(c, from) > 3
            })
            .unwrap()
    };
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, away)
    );
    assert!(wait_for_turn(&mut game, wild));

    assert!(game.tactical_ai_turn());

    assert!(game.world.get::<Charging>(wild).is_none(), "it held on");
    assert!(on_cooldown(&game, wild));
    assert_eq!(game.tactical_actor(), Some(player), "handed on once");
}

#[test]
fn the_view_shows_who_is_charging_and_where_it_will_land() {
    let (mut game, player, wild) = fight(3, 0);
    let at = cell_of(&game, wild);
    start_on(&mut game, at);
    let view = game.tactical_view().unwrap();
    let body = view.bodies.iter().find(|b| b.entity == player).unwrap();
    assert_eq!(body.charge, Some((1, 3)));
    assert!(
        view.bodies
            .iter()
            .find(|b| b.entity == wild)
            .unwrap()
            .charge
            .is_none()
    );
    assert_eq!(view.charge_aims.len(), 1);
    assert_eq!(view.charge_aims[0].cells, vec![at]);
    assert!(view.charge_aims[0].party_side);
    assert!(view.frozen().charge_aims.is_empty());
}
