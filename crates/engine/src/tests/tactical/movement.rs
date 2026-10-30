use super::*;

/// The player has no `Creature` and so no species to author anything.
///
/// This pins the shape and not the door: `PLAYER_BASE_SPEED` sits one
/// point above `DEFAULT_BASE_SPEED` and the two land in the same band, so
/// no assertion here could tell `combat_speed` from `species_base_speed`.
/// What it does hold is that the player has an allowance at all and that
/// it comes out of the one derivation everybody else's does.
#[test]
fn the_player_moves_at_their_own_baseline() {
    let game = game();
    let player = game.player_entity();
    assert_eq!(
        game.movement_allowance(player),
        allowance(PLAYER_BASE_SPEED, None)
    );
}

#[test]
fn a_body_with_no_authored_figure_derives_one_from_its_speed() {
    let mut game = game();
    let mut species = generic_species();
    species.base_speed = DEFAULT_BASE_SPEED + 4;
    species.movement = None;
    let id = species.id.clone();
    game.world.resource_mut::<SpeciesDb>().insert(species);

    let entity = body(&mut game, &id);
    assert_eq!(
        game.movement_allowance(entity),
        allowance(DEFAULT_BASE_SPEED + 4, None)
    );
}

/// The whole of the escape hatch: a species that authors a figure moves at
/// it, whatever its initiative says.
#[test]
fn an_authored_movement_is_what_that_species_moves() {
    let mut game = game();
    let authored = TACTICAL_MOVE_MAX - 1;
    let mut species = generic_species();
    species.base_speed = DEFAULT_BASE_SPEED;
    species.movement = Some(authored);
    let id = species.id.clone();
    game.world.resource_mut::<SpeciesDb>().insert(species);

    let entity = body(&mut game, &id);
    assert_ne!(
        allowance(DEFAULT_BASE_SPEED, None),
        authored,
        "the fixture must be able to tell the authored figure from the derived one"
    );
    assert_eq!(game.movement_allowance(entity), authored);
}

/// The field has to survive the round trip a mod actually takes: a `.ron`
/// file through serde. Nothing shipped authors one, so without this the
/// only proof it works at all goes through `SpeciesDb::insert`, which never
/// parses anything.
#[test]
fn a_species_file_can_author_a_movement_figure() {
    const BARE: &str = r#"(
        id: "test_mover",
        name: "Test Mover",
        glyph: 'm',
        color: White,
        base_hp: 10,
        base_atk: 2,
        base_mitigation: 0,
        taming_difficulty: 0.5,
        habitats: [],
        moves: [],
        work_resource: None,
    )"#;

    let plain: crate::species::SpeciesDef =
        ron::from_str(BARE).expect("the fixture must parse without the field");
    assert_eq!(plain.movement, None, "absent must mean derived");

    let authored: crate::species::SpeciesDef = ron::from_str(&BARE.replace(
        "work_resource: None,",
        "work_resource: None, movement: Some(5),",
    ))
    .expect("the fixture must parse with the field");
    assert_eq!(authored.movement, Some(5));
}

/// The mean y of `bodies`' cells — which side of the board a rank sits on.
fn mean_y(game: &Game, bodies: &[Entity]) -> f32 {
    let battle = game.world.resource::<TacticalBattle>();
    let ys: Vec<i32> = bodies
        .iter()
        .filter_map(|&b| battle.cell_of(b))
        .map(|(_, y)| y)
        .collect();
    assert!(!ys.is_empty(), "nobody was seated");
    ys.iter().sum::<i32>() as f32 / ys.len() as f32
}

#[test]
fn an_authored_bearing_seats_the_pack_on_that_side() {
    // The pack stands *east* of the player either way, so a deployment
    // reading the tiles would answer the same thing twice. This is what
    // `open_tactical_battle_at` exists for.
    let mut north = game();
    let pack = tactical_pack(&mut north, 2, 10);
    north.open_tactical_battle_at(pack.clone(), (0, -1));
    let player = north.player_entity();
    assert!(
        mean_y(&north, &pack) < mean_y(&north, &[player]),
        "a northward approach did not seat the pack north"
    );

    let mut south = game();
    let pack = tactical_pack(&mut south, 2, 10);
    south.open_tactical_battle_at(pack.clone(), (0, 1));
    let player = south.player_entity();
    assert!(
        mean_y(&south, &pack) > mean_y(&south, &[player]),
        "a southward approach did not seat the pack south"
    );
}

#[test]
fn the_arena_door_drives_a_party_body_where_the_ai_door_declines_it() {
    let mut game = game();
    tactical_fight(&mut game, 1, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));

    assert!(
        !game.tactical_ai_turn(),
        "the AI door drove a body the player commands"
    );
    assert!(
        game.tactical_drive_turn(PartyTurns::SwingOnly),
        "the arena door drove nobody"
    );
    assert_ne!(
        game.tactical_actor(),
        Some(player),
        "a driven turn was not handed on"
    );
}

#[test]
fn a_fight_driven_from_both_sides_resolves() {
    // The player alone against one hostile, so this fails rather than
    // merely reading oddly if sidedness is taken absolutely: a party body
    // that thinks the party is the enemy has nobody to close on, ends every
    // turn where it stands, and the fight runs to the bound below for ever.
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 10);

    for _ in 0..2000 {
        if !game.has_active_battle() {
            break;
        }
        assert!(
            game.tactical_drive_turn(PartyTurns::SwingOnly),
            "a fight open with nobody acting"
        );
    }

    assert!(!game.has_active_battle(), "the driven fight never resolved");
    assert!(
        game.world.get::<Stats>(pack[0]).is_none_or(|s| s.hp <= 0),
        "the fight ended with the hostile still up"
    );
}

#[test]
fn a_tactical_fight_seats_every_body_and_rolls_one_order_over_all_of_them() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 10);
    let player = game.player_entity();
    let battle = game.world.resource::<TacticalBattle>();

    assert!(
        battle.cell_of(player).is_some(),
        "the player was not seated"
    );
    for body in &pack {
        assert!(battle.cell_of(*body).is_some(), "a hostile was not seated");
    }
    assert_eq!(battle.bodies().count(), battle.initiative().len());
    assert_eq!(battle.round, 1);
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "the two combat models are never both open"
    );
}

/// The seam: a battle map's coordinates live in `TacticalBattle`, and a body
/// walking one is standing exactly where the fight opened as far as the
/// world is concerned.
#[test]
fn walking_the_battle_map_writes_no_world_position() {
    let mut game = game();
    tactical_fight(&mut game, 1, 10);
    let player = game.player_entity();
    let before = *game.world.get::<Position>(player).unwrap();

    assert!(wait_for_turn(&mut game, player));
    let mut moved = false;
    for dir in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        if game.tactical_step(dir) == StepOutcome::Moved {
            moved = true;
            break;
        }
    }
    assert!(moved, "the player could not take a single step");

    let after = *game.world.get::<Position>(player).unwrap();
    assert_eq!(before, after, "a battle-map step moved a world Position");
}

#[test]
fn a_step_is_charged_the_cell_it_enters_and_stops_at_the_allowance() {
    let mut game = game();
    tactical_fight(&mut game, 1, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    let allowance = game.movement_allowance(player);

    let mut steps = 0;
    while game.world.resource::<TacticalBattle>().spent() < allowance {
        let spent = game.world.resource::<TacticalBattle>().spent();
        let stepped = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, -1)]
            .into_iter()
            .any(|dir| game.tactical_step(dir) == StepOutcome::Moved);
        if !stepped {
            break;
        }
        assert!(
            game.world.resource::<TacticalBattle>().spent() > spent,
            "a step that moved cost nothing"
        );
        steps += 1;
        assert!(steps < 32, "the allowance never ran out");
    }
    assert!(
        game.world.resource::<TacticalBattle>().spent() <= allowance,
        "a body walked further than its allowance"
    );
    for dir in [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, -1)] {
        assert_eq!(
            game.tactical_step(dir),
            StepOutcome::Refused,
            "a spent body kept walking"
        );
    }
}

#[test]
fn the_action_ends_the_turn_and_no_second_one_is_offered() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 100);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    // Stood next to the target, since a swing is melee-only for now.
    let target = pack[0];
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(target)
        .unwrap();
    let beside = beside(&game, at).expect("a free cell beside the target");
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, beside)
    );

    assert!(game.tactical_attack(target), "the swing was refused");
    assert_ne!(
        game.tactical_actor(),
        Some(player),
        "the action did not end the turn"
    );
    assert!(wait_for_turn(&mut game, player));
    assert!(
        game.world.resource::<TacticalBattle>().actions_left() > 0,
        "a fresh turn came in already spent"
    );
}

/// A free walkable cell orthogonally or diagonally adjacent to `cell`.
pub(super) fn beside(game: &Game, cell: (i32, i32)) -> Option<(i32, i32)> {
    let battle = game.world.resource::<TacticalBattle>();
    (-1..=1)
        .flat_map(|dx| (-1..=1).map(move |dy| (dx, dy)))
        .filter(|&(dx, dy)| (dx, dy) != (0, 0))
        .map(|(dx, dy)| (cell.0 + dx, cell.1 + dy))
        .find(|&at| battle.board.walkable(at.0, at.1) && battle.occupant(at).is_none())
}

/// Disengage: the edge is not a wall, and walking out is how a body leaves a
/// fight it does not want.
#[test]
fn walking_off_the_edge_leaves_the_fight() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 10);
    let leaver = pack[0];
    assert!(wait_for_turn(&mut game, leaver));
    let edge = western_edge(&game);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(leaver, edge)
    );

    assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);
    let battle = game.world.resource::<TacticalBattle>();
    assert_eq!(battle.cell_of(leaver), None, "the body stayed on the board");
    assert!(
        !battle.initiative().contains(&leaver),
        "the body kept its place in the order"
    );
    assert!(
        game.world.get::<Stats>(leaver).is_some(),
        "breaking off is not dying"
    );
    assert!(battle.fallen().is_empty(), "breaking off left a mark");
}

/// The last hostile's committed walk steps off the board: the departure
/// closes the fight and drops `TacticalBattle`, and the beat that took the
/// step must end there rather than read the board it just lost.
#[test]
fn a_walk_step_that_closes_the_fight_ends_the_beat() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 10);
    let leaver = pack[0];
    assert!(wait_for_turn(&mut game, leaver));
    let edge = western_edge(&game);
    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        assert!(battle.move_to(leaver, edge));
        battle.commit_walk(vec![(edge.0 - 1, edge.1)]);
    }

    assert_eq!(game.tactical_ai_beat(), AiBeat::Acted);
    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the fight stayed open with no hostile left in it"
    );
}

/// The player walking out is the jack-out, and it is not a win.
#[test]
fn the_player_walking_out_closes_the_fight_without_winning_it() {
    let mut game = game();
    tactical_fight(&mut game, 2, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player));
    let edge = western_edge(&game);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, edge)
    );

    assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);
    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the fight stayed open with nobody holding it"
    );
    let lines = log_texts(&game);
    assert!(
        !lines.iter().any(|t| *t == "You won!"),
        "walking out was announced as a win: {lines:#?}"
    );
}
