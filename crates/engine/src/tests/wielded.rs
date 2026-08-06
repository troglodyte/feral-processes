//! Wielding a tamed program as your weapon — the live-computed passive
//! bonus, the wield/unwield doors, and the routine proc.

use super::support::*;
use crate::tuning::WIELDED_PROGRAM_STAT_DIVISOR;
use crate::*;

/// Sets the resource straight, for the tests that predate `wield_program`
/// or deliberately want to bypass its refusals.
fn wield_directly(game: &mut Game, entity: Entity) {
    game.world.insert_resource(WieldedProgram(Some(entity)));
}

#[test]
fn wielding_a_program_raises_the_players_attack_and_defense() {
    let mut game = Game::new(9100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (atk_before, def_before) = (game.effective_atk(player), game.effective_def(player));

    let program = spawn_tamed(&mut game, 40, 60);
    game.world.get_mut::<Stats>(program).unwrap().def = 30;
    wield_directly(&mut game, program);

    assert_eq!(
        game.effective_atk(player) - atk_before,
        60 / WIELDED_PROGRAM_STAT_DIVISOR,
        "the wielded program lends a share of its ATK"
    );
    assert_eq!(
        game.effective_def(player) - def_before,
        30 / WIELDED_PROGRAM_STAT_DIVISOR,
        "and a share of its DEF"
    );
}

#[test]
fn the_wielded_bonus_floors_at_one_per_stat() {
    let mut game = Game::new(9101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 10, 1);
    game.world.get_mut::<Stats>(program).unwrap().def = 1;
    wield_directly(&mut game, program);

    assert_eq!(
        game.wielded_stat_bonus(),
        (1, 1),
        "a weak program is still worth something in the hand"
    );
}

#[test]
fn the_wielded_bonus_tracks_the_programs_current_stats() {
    let mut game = Game::new(9102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    wield_directly(&mut game, program);
    let before = game.wielded_stat_bonus().0;

    game.world.get_mut::<Stats>(program).unwrap().atk = 200;

    assert!(
        game.wielded_stat_bonus().0 > before,
        "the bonus is computed live, not captured at wield time"
    );
}

#[test]
fn a_despawned_wielded_program_lends_nothing() {
    let mut game = Game::new(9103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let program = spawn_tamed(&mut game, 40, 60);
    wield_directly(&mut game, program);
    let armed = game.effective_atk(player);

    game.world.despawn(program);

    assert_eq!(game.wielded_program(), None);
    assert_eq!(game.wielded_stat_bonus(), (0, 0));
    assert!(
        game.effective_atk(player) < armed,
        "the bonus goes with the program, with nothing to clear"
    );
}
