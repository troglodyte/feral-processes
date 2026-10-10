//! Fighting the terrain: a swing or a routine at a prop, and what a volatile
//! one does when it goes.
//!
//! Boards are `Board::from_rows` and bodies are seated by hand — where a
//! prop stands is the thing under test, and `map::generate` puts it wherever
//! the seed says.

use super::tactical::{only_routine, place_one, tactical_fight, wait_for_turn};
use crate::Game;
use crate::components::{Experience, Stats};
use crate::resources::{DifficultyMode, TacticalFxKind};
use crate::tactical::TacticalBattle;
use crate::tactical::map::{Board, PropCell};
use crate::tests::support::{HOSTILE_SWEEP, equip_weapon, test_assets_dir};
use crate::tuning::TACTICAL_PROP_CHAIN_MAX;
use bevy_ecs::prelude::Entity;

const SIDE: usize = 7;

/// Hand-placed prop markers: `(x, y)` and a `Board::from_rows` letter.
type Marks = Vec<((usize, usize), char)>;

fn rows(marks: &[((usize, usize), char)]) -> Vec<String> {
    let mut grid = vec![vec!['.'; SIDE]; SIDE];
    for &((x, y), c) in marks {
        grid[y][x] = c;
    }
    grid.into_iter().map(|r| r.into_iter().collect()).collect()
}

/// A fight on a hand-written 7x7 board with `count` hostiles at 200 hp, the
/// player holding a reach-3 weapon at `player_at` and on the move. Hostiles
/// are parked in the far corner.
fn fight(
    marks: &[((usize, usize), char)],
    player_at: (i32, i32),
    count: usize,
) -> (Game, Vec<Entity>) {
    let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = tactical_fight(&mut game, count, 200);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    let grid = rows(marks);
    let refs: Vec<&str> = grid.iter().map(String::as_str).collect();
    game.world.resource_mut::<TacticalBattle>().board = Board::from_rows(&refs);
    place_one(&mut game, player, player_at);
    for (i, &w) in wild.iter().enumerate() {
        place_one(&mut game, w, (6, 6 - i as i32));
    }
    assert!(
        wait_for_turn(&mut game, player),
        "the player never got a turn"
    );
    (game, wild)
}

fn prop_hp(game: &Game, cell: (i32, i32)) -> Option<u32> {
    game.world
        .resource::<TacticalBattle>()
        .board
        .prop_at(cell.0, cell.1)
        .and_then(|p| p.hp)
}

fn weaken(game: &mut Game, cell: (i32, i32), to: u32) {
    let board = &mut game.world.resource_mut::<TacticalBattle>().board;
    let hp = board.prop_at(cell.0, cell.1).and_then(|p| p.hp).unwrap();
    board.damage_prop(cell, hp - to);
}

fn hp_of(game: &Game, body: Entity) -> i32 {
    game.world.get::<Stats>(body).unwrap().hp
}

fn player_is_acting(game: &Game) -> bool {
    game.tactical_actor() == Some(game.player_entity())
}

#[test]
fn a_swing_at_a_prop_damages_it_and_spends_the_turn() {
    let (mut game, _) = fight(&[((3, 2), 'D')], (3, 0), 1);
    assert!(game.tactical_attack_prop((3, 2)));
    assert!(prop_hp(&game, (3, 2)).is_none_or(|hp| hp < 10));
    assert!(!player_is_acting(&game), "the action was not spent");
}

#[test]
fn a_swing_that_cannot_land_is_refused_and_spends_nothing() {
    let scorch = {
        let db = crate::tactical::props::PropDb::load_dir(&test_assets_dir().join("battle-props"))
            .unwrap()
            .0;
        PropCell::from_def(db.piece("scorch").unwrap())
    };
    let cases: [(&str, Marks, (i32, i32)); 4] = [
        ("empty cell", vec![], (3, 2)),
        ("out of range", vec![((3, 5), 'D')], (3, 5)),
        ("blocked sight", vec![((3, 1), 'P'), ((3, 2), 'D')], (3, 2)),
        ("indestructible", vec![((3, 1), 'P')], (3, 1)),
    ];
    for (name, marks, cell) in cases {
        let (mut game, _) = fight(&marks, (3, 0), 1);
        let before = prop_hp(&game, cell);
        assert!(!game.tactical_attack_prop(cell), "{name}: swing landed");
        assert!(player_is_acting(&game), "{name}: an action was spent");
        assert_eq!(prop_hp(&game, cell), before, "{name}: the prop changed");
    }
    let (mut game, _) = fight(&[], (3, 0), 1);
    game.world
        .resource_mut::<TacticalBattle>()
        .board
        .place_prop((3, 1), scorch);
    assert!(!game.tactical_attack_prop((3, 1)), "decoration was struck");
    assert!(player_is_acting(&game));
}

#[test]
fn a_destroyed_prop_leaves_rubble_opens_sight_and_cues_without_paying() {
    let (mut game, _) = fight(&[((3, 1), 'D')], (3, 0), 1);
    let player = game.player_entity();
    let xp = game.world.get::<Experience>(player).unwrap().xp;
    weaken(&mut game, (3, 1), 1);
    {
        let battle = game.world.resource::<TacticalBattle>();
        assert!(battle.board.blocks_sight(3, 1));
    }
    game.take_tactical_fx();
    assert!(game.tactical_attack_prop((3, 1)));
    let battle = game.world.resource::<TacticalBattle>();
    let left = battle.board.prop_at(3, 1).expect("rubble stands there");
    assert_eq!(left.piece, crate::tactical::props::RUBBLE_PIECE);
    assert!(!battle.board.blocks_sight(3, 1));
    assert!(crate::tactical::reach::line_of_sight(
        &battle.board,
        (3, 0),
        (3, 4)
    ));
    let cues = game.take_tactical_fx();
    assert!(
        cues.iter().any(|c| c.pos == (3, 1)
            && c.kind == TacticalFxKind::PropDestroyed { volatile: false }),
        "{cues:?}"
    );
    assert_eq!(game.world.get::<Experience>(player).unwrap().xp, xp);
}

#[test]
fn a_volatile_prop_blasts_the_bodies_beside_it() {
    let (mut game, wild) = fight(&[((5, 5), 'V')], (5, 2), 1);
    place_one(&mut game, wild[0], (6, 6));
    weaken(&mut game, (5, 5), 1);
    let before = hp_of(&game, wild[0]);
    game.take_tactical_fx();
    assert!(game.tactical_attack_prop((5, 5)));
    assert_eq!(hp_of(&game, wild[0]), before - 12);
    let cues = game.take_tactical_fx();
    assert!(
        cues.iter()
            .any(|c| c.pos == (5, 5) && c.kind == TacticalFxKind::PropDestroyed { volatile: true })
    );
}

#[test]
fn a_blast_sets_off_the_volatile_prop_beside_it() {
    let (mut game, _) = fight(&[((3, 3), 'V'), ((4, 3), 'V')], (3, 0), 1);
    weaken(&mut game, (3, 3), 1);
    assert!(game.tactical_attack_prop((3, 3)));
    let battle = game.world.resource::<TacticalBattle>();
    for cell in [(3, 3), (4, 3)] {
        assert_eq!(
            battle
                .board
                .prop_at(cell.0, cell.1)
                .map(|p| p.piece.as_str()),
            Some("rubble"),
            "{cell:?}"
        );
    }
}

#[test]
fn a_chain_of_volatile_props_stops_at_the_chain_cap() {
    let marks: Vec<_> = (1..=6).map(|x| ((x, 3), 'V')).collect();
    let (mut game, _) = fight(&marks, (0, 0), 1);
    for x in 1..=6 {
        weaken(&mut game, (x, 3), 1);
    }
    game.take_tactical_fx();
    assert!(game.tactical_attack_prop((1, 3)));
    let blasts = game
        .take_tactical_fx()
        .iter()
        .filter(|c| c.kind == TacticalFxKind::PropDestroyed { volatile: true })
        .count();
    // The cap counts blasts: the last prop in the chain is left standing
    // rather than going off as a dud.
    assert_eq!(blasts, TACTICAL_PROP_CHAIN_MAX as usize);
    let battle = game.world.resource::<TacticalBattle>();
    let standing = (1..=6)
        .filter(|&x| {
            battle
                .board
                .prop_at(x, 3)
                .is_some_and(|p| p.volatile.is_some())
        })
        .count();
    assert_eq!(standing, 6 - TACTICAL_PROP_CHAIN_MAX as usize);
}

#[test]
fn a_body_killed_by_the_blast_it_set_off_hands_the_turn_on() {
    let (mut game, wild) = fight(&[((3, 3), 'V')], (3, 0), 2);
    place_one(&mut game, wild[0], (3, 4));
    weaken(&mut game, (3, 3), 1);
    // The acting body is the hostile standing beside the cell.
    game.world.get_mut::<Stats>(wild[0]).unwrap().hp = 1;
    assert!(wait_for_turn(&mut game, wild[0]), "no turn for the hostile");
    place_one(&mut game, wild[0], (3, 4));
    assert!(game.tactical_attack_prop((3, 3)));
    let battle = game.world.resource::<TacticalBattle>();
    assert!(
        battle.cell_of(wild[0]).is_none(),
        "it survived its own blast"
    );
    assert_ne!(battle.actor(), Some(wild[0]));
    assert!(battle.actor().is_some());
}

#[test]
fn the_last_hostile_killed_by_a_blast_ends_the_fight_and_pays() {
    let (mut game, wild) = fight(&[((5, 5), 'V')], (5, 2), 1);
    let player = game.player_entity();
    let xp = game.world.get::<Experience>(player).unwrap().xp;
    game.world.get_mut::<Stats>(wild[0]).unwrap().hp = 1;
    place_one(&mut game, wild[0], (6, 6));
    weaken(&mut game, (5, 5), 1);
    assert!(game.tactical_attack_prop((5, 5)));
    assert!(game.world.get_resource::<TacticalBattle>().is_none());
    assert!(game.world.get::<Experience>(player).unwrap().xp > xp);
}

#[test]
fn an_area_routine_damages_every_destructible_prop_it_covers() {
    let (mut game, _) = fight(&[((3, 3), 'D'), ((4, 3), 'D')], (0, 0), 1);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(game.tactical_use_routine(0, (3, 2)));
    for cell in [(3, 3), (4, 3)] {
        assert!(prop_hp(&game, cell).is_none_or(|hp| hp < 10), "{cell:?}");
    }
}

#[test]
fn a_routine_leaves_an_indestructible_prop_alone() {
    let (mut game, _) = fight(&[((3, 3), 'P')], (0, 0), 1);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(game.tactical_use_routine(0, (3, 2)));
    assert!(
        game.world
            .resource::<TacticalBattle>()
            .board
            .prop_at(3, 3)
            .is_some()
    );
}

// --- The hostile AI ------------------------------------------------------

/// A hostile standing on `wild_at` with the turn, the player on `player_at`
/// and nothing else about; the board is `marks` over open ground.
fn ai_fight(
    marks: &[((usize, usize), char)],
    wild_at: (i32, i32),
    player_at: (i32, i32),
) -> (Game, Entity) {
    let (mut game, wild) = fight(marks, (0, 0), 1);
    let player = game.player_entity();
    place_one(&mut game, wild[0], wild_at);
    place_one(&mut game, player, player_at);
    // The hostile's turn, with the player's spent behind it.
    assert!(wait_for_turn(&mut game, wild[0]));
    (game, wild[0])
}

fn row_hp(game: &Game, y: i32) -> u32 {
    (0..SIDE as i32).filter_map(|x| prop_hp(game, (x, y))).sum()
}

#[test]
fn a_hostile_walled_off_from_its_target_breaks_the_wall() {
    let wall: Marks = (0..SIDE).map(|x| ((x, 3), 'D')).collect();
    let (mut game, _) = ai_fight(&wall, (3, 2), (3, 6));
    let before = row_hp(&game, 3);
    assert!(game.tactical_ai_turn_at(0.0));
    assert!(row_hp(&game, 3) < before, "the wall was not touched");
}

#[test]
fn a_hostile_with_a_clear_shot_leaves_the_props_alone() {
    let (mut game, _) = ai_fight(&[((2, 2), 'D'), ((4, 2), 'V')], (3, 2), (3, 3));
    assert!(game.tactical_ai_turn_at(0.0));
    assert_eq!(prop_hp(&game, (2, 2)), Some(10));
    assert_eq!(prop_hp(&game, (4, 2)), Some(5));
}

#[test]
fn a_hostile_detonates_a_volatile_prop_over_its_target() {
    let (mut game, wild) = ai_fight(&[((3, 3), 'V')], (3, 0), (3, 4));
    equip_weapon(&mut game, wild, "plasma_router");
    let player = game.player_entity();
    let before = hp_of(&game, player);
    assert!(game.tactical_ai_turn_at(0.0));
    assert_eq!(prop_hp(&game, (3, 3)), None, "the cell was not set off");
    assert!(hp_of(&game, player) <= before - 10, "the blast missed");
}

#[test]
fn a_hostile_will_not_blow_a_volatile_prop_beside_itself() {
    let (mut game, wild) = ai_fight(&[((3, 3), 'V')], (3, 2), (0, 6));
    equip_weapon(&mut game, wild, "plasma_router");
    assert!(game.tactical_ai_turn_at(0.0));
    assert_eq!(prop_hp(&game, (3, 3)), Some(5));
}

/// A party body beside a volatile prop with two hostiles in its blast, and
/// a companion too when `with_companion`. Returns the swing the planner
/// would take from the player's cell.
fn party_swing_at_a_crowded_volatile(with_companion: bool) -> Option<(i32, i32)> {
    let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = with_companion.then(|| {
        let friend = crate::tests::support::spawn_tamed(&mut game, 40, 3);
        game.world
            .resource_mut::<crate::resources::Party>()
            .0
            .push(friend);
        friend
    });
    let wild = tactical_fight(&mut game, 2, 200);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    let grid = rows(&[((3, 3), 'V')]);
    let refs: Vec<&str> = grid.iter().map(String::as_str).collect();
    game.world.resource_mut::<TacticalBattle>().board = Board::from_rows(&refs);
    place_one(&mut game, player, (3, 0));
    place_one(&mut game, wild[0], (2, 4));
    place_one(&mut game, wild[1], (4, 4));
    if let Some(friend) = companion {
        place_one(&mut game, friend, (3, 4));
    }
    let range = game.swing_range(player);
    game.best_prop_swing(player, (3, 0), range, None, &[(2, 4), (4, 4)])
}

#[test]
fn a_party_body_will_not_detonate_a_blast_that_catches_its_own_side() {
    assert_eq!(
        party_swing_at_a_crowded_volatile(false),
        Some((3, 3)),
        "two hostiles and no friend is worth a blast"
    );
    assert_eq!(
        party_swing_at_a_crowded_volatile(true),
        None,
        "the companion in the blast should veto it, not be netted off"
    );
}

#[test]
fn the_same_board_gets_the_same_choice() {
    let wall: Marks = (0..SIDE).map(|x| ((x, 3), 'D')).collect();
    let run = || {
        let (mut game, _) = ai_fight(&wall, (3, 2), (3, 6));
        game.tactical_ai_turn_at(0.0);
        (0..SIDE as i32)
            .map(|x| prop_hp(&game, (x, 3)))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_hostile_with_a_way_round_does_not_break_the_wall() {
    let wall: Marks = (0..SIDE - 1).map(|x| ((x, 3), 'D')).collect();
    let (mut game, _) = ai_fight(&wall, (3, 2), (3, 6));
    let before = row_hp(&game, 3);
    assert!(game.tactical_ai_turn_at(0.0));
    assert_eq!(
        row_hp(&game, 3),
        before,
        "it hit the wall instead of going round"
    );
}

#[test]
fn the_view_lists_props_sorted_by_cell() {
    let (mut game, _) = fight(&[((5, 4), 'V'), ((2, 3), 'D'), ((2, 1), 'P')], (0, 0), 1);

    let view = game.tactical_view().expect("the fight is open");

    let cells: Vec<(i32, i32)> = view.props.iter().map(|p| p.cell).collect();
    assert_eq!(cells, vec![(2, 1), (2, 3), (5, 4)]);
    assert!(view.props[2].volatile);
    assert_eq!(view.props[1].hp, Some(10));
    assert_eq!(view.props[0].hp, None);
}
