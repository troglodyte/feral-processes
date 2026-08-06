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

#[test]
fn wielding_a_party_member_stands_it_down() {
    let mut game = Game::new(9104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    game.add_companion(program).unwrap();

    game.wield_program(program).unwrap();

    assert!(
        !game.world.resource::<Party>().0.contains(&program),
        "a weapon is not a combatant"
    );
    assert_eq!(game.wielded_program(), Some(program));
}

#[test]
fn adding_a_wielded_program_to_the_party_unwields_it() {
    let mut game = Game::new(9105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    game.wield_program(program).unwrap();

    game.add_companion(program).unwrap();

    assert_eq!(game.wielded_program(), None, "the other door");
    assert!(game.world.resource::<Party>().0.contains(&program));
}

#[test]
fn wielding_returns_the_worn_weapon_and_removes_its_bonus() {
    let mut game = Game::new(9106, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let bare_atk = game.world.get::<Stats>(player).unwrap().atk;
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
    assert!(game.world.get::<Stats>(player).unwrap().atk > bare_atk);

    let program = spawn_tamed(&mut game, 40, 20);
    game.wield_program(program).unwrap();

    assert_eq!(
        game.player_status().weapon,
        None,
        "the slot holds an item or a program, never both"
    );
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().atk,
        bare_atk,
        "the item's delta comes off; the program's bonus never touches Stats"
    );
    assert!(
        game.player_status()
            .inventory
            .iter()
            .any(|(i, n)| *i == ItemId::from(ids::OVERCLOCK_CORE) && *n == 1),
        "the displaced item goes back to cargo rather than being destroyed"
    );
}

#[test]
fn a_wield_refused_in_battle_changes_nothing() {
    let mut game = Game::new(9107, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
    let program = spawn_tamed(&mut game, 40, 20);
    game.add_companion(program).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);

    assert!(game.wield_program(program).is_err());

    assert!(
        game.world.resource::<Party>().0.contains(&program),
        "the refusal resolves before anything moves"
    );
    assert!(game.player_status().weapon.is_some());
    assert_eq!(game.wielded_program(), None);
}

#[test]
fn wielding_costs_one_turn_whether_or_not_it_displaces_a_weapon() {
    let mut bare = Game::new(9108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut bare, 40, 20);
    let before = bare.world.resource::<GameClock>().tick;
    bare.wield_program(program).unwrap();
    let bare_cost = bare.world.resource::<GameClock>().tick - before;

    let mut armed = Game::new(9108, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = armed.player_entity();
    armed
        .world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
    armed.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
    let program = spawn_tamed(&mut armed, 40, 20);
    let before = armed.world.resource::<GameClock>().tick;
    armed.wield_program(program).unwrap();
    let armed_cost = armed.world.resource::<GameClock>().tick - before;

    assert_eq!(bare_cost, 1);
    assert_eq!(
        armed_cost, bare_cost,
        "one player action is one tick, whether or not a weapon was displaced"
    );
}

#[test]
fn unwield_program_clears_the_bonus() {
    let mut game = Game::new(9109, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 60);
    game.wield_program(program).unwrap();
    assert_ne!(game.wielded_stat_bonus(), (0, 0));

    game.unwield_program().unwrap();

    assert_eq!(game.wielded_program(), None);
    assert_eq!(game.wielded_stat_bonus(), (0, 0));
    assert!(game.unwield_program().is_err(), "nothing to put down twice");
}

#[test]
fn selling_the_wielded_program_ends_the_wield() {
    let mut game = Game::new(9110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 60);
    game.wield_program(program).unwrap();

    game.dissolve_tamed_program(program);

    assert_eq!(game.wielded_program(), None);
    assert_eq!(
        game.wielded_stat_bonus(),
        (0, 0),
        "asserted without `dissolve_tamed_program` knowing this feature exists"
    );
}

#[test]
fn fusing_away_the_wielded_program_ends_the_wield() {
    let mut game = Game::new(9111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let program = spawn_tamed(&mut game, 40, 60);
    let other = spawn_tamed(&mut game, 30, 30);
    game.wield_program(program).unwrap();

    game.fuse_companions(program, other, None).unwrap();

    assert_eq!(game.wielded_program(), None);
    assert_eq!(
        game.wielded_stat_bonus(),
        (0, 0),
        "the second destruction path is covered by the same omission"
    );
}
