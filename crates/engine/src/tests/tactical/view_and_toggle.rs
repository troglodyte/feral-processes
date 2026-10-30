use super::*;

/// Where a hostile chooses to stand is one draw a turn, and none at all at
/// temperature zero — `sample_scored` answers the argmax before it touches
/// the RNG. That is what lets a test pin the choice without moving the
/// seeded stream every later roll in the run depends on.
///
/// Two identical runs rather than a snapshot, because `StdRng` is not
/// `Clone`. Asserted on a turn that ends in no action, because a swing rolls
/// to hit and this is a claim about the *walk*.
#[test]
fn choosing_a_cell_at_zero_temperature_does_not_move_the_seeded_stream() {
    let (mut ran, _) = marooned();
    let (mut untouched, _) = marooned();
    assert!(
        ran.tactical_ai_turn_at(0.0),
        "the hostile's turn was not run"
    );

    assert_eq!(
        next_draw(&mut ran),
        next_draw(&mut untouched),
        "an argmax turn spent a draw and shifted every later roll"
    );
}

/// The view stands every body where the board stands it, and never reads a
/// world `Position` to do it — the seam the whole module rests on.
#[test]
fn a_tactical_view_stands_every_body_where_the_board_does() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 3, 10);
    let player = game.player_entity();

    let placed: Vec<(Entity, (i32, i32))> =
        game.world.resource::<TacticalBattle>().bodies().collect();
    let view = game.tactical_view().expect("a fight is open");

    assert_eq!(view.bodies.len(), placed.len());
    for (entity, cell) in placed {
        let body = view
            .bodies
            .iter()
            .find(|b| b.entity == entity)
            .expect("every placed body is in the view");
        assert_eq!(body.cell, cell, "the view moved a body off its cell");
    }
    assert!(
        view.bodies
            .iter()
            .any(|b| b.is_player && b.entity == player),
        "the player is on the board"
    );
    assert_eq!(
        view.bodies.iter().filter(|b| b.is_hostile).count(),
        pack.len(),
        "every hostile is drawn as one"
    );
    // The con read is the hostiles' alone, or the map draws a danger
    // rung under a companion.
    assert!(
        view.bodies
            .iter()
            .all(|b| b.difficulty.is_none() || b.is_hostile)
    );
}

/// A body's rare-spawn tier rides into the view too, or the tactical board
/// has no way to draw the bar the surface map already puts on it — see
/// `render/marks.rs::draw_rarity_bar`.
#[test]
fn a_tactical_bodys_rarity_rides_into_the_view() {
    let mut game = game();
    let pack = tactical_pack(&mut game, 1, 10);
    let hostile = pack[0];
    game.world.entity_mut(hostile).insert(Rarity::Gold);
    game.open_tactical_battle(pack);

    let view = game.tactical_view().expect("a fight is open");
    let body = view
        .bodies
        .iter()
        .find(|b| b.entity == hostile)
        .expect("the spawned body is on the board");
    assert_eq!(body.rarity, Rarity::Gold, "the view dropped the tier");
}

#[test]
fn the_view_names_whose_turn_it_is_and_what_is_left_of_it() {
    let mut game = game();
    tactical_fight(&mut game, 2, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the player gets a turn");

    let full = game.movement_allowance(player);
    let before = game.tactical_view().expect("a fight is open");
    assert!(before.player_turn, "it is the player's turn");
    assert_eq!(
        before.order[before.active.expect("somebody is acting")].entity,
        player,
    );
    assert_eq!(before.allowance, full, "an untouched turn has it all");
    assert!(!before.acted);
    assert!(
        before.reachable.contains(
            &before
                .bodies
                .iter()
                .find(|b| b.entity == player)
                .expect("the player is on the board")
                .cell
        ),
        "a body can always stand where it already stands"
    );

    // One step spent is one step gone. Which direction is open depends on
    // the generated board, so this takes whichever one moved.
    let stepped = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .any(|dir| matches!(game.tactical_step(dir), StepOutcome::Moved));
    assert!(stepped, "some neighbour is open");
    let after = game.tactical_view().expect("a fight is open");
    assert!(
        after.allowance < full,
        "a step is spent out of the allowance the view reports"
    );
}

/// The preview and the delivery are the same geometry, which is the whole
/// reason the preview is a call rather than a second derivation.
#[test]
fn the_aim_preview_is_the_cells_the_routine_would_actually_cover() {
    let mut game = game();
    tactical_fight(&mut game, 3, 10);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the player gets a turn");

    let from = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player is on the board");
    let ability = game
        .actor_abilities(player)
        .into_iter()
        .next()
        .expect("the player knows a routine");

    let covered = game.tactical_shape_cells(0, from);
    let battle = game.world.resource::<TacticalBattle>();
    let hit = crate::tactical::reach::recipients(battle, player, from, ability.tactical_shape());

    for body in hit {
        let cell = battle.cell_of(body).expect("a recipient is on the board");
        assert!(
            covered.contains(&cell),
            "a body was hit on a cell the preview did not draw"
        );
    }
    for (body, cell) in battle.bodies() {
        if covered.contains(&cell) {
            let hit =
                crate::tactical::reach::recipients(battle, player, from, ability.tactical_shape());
            assert!(
                hit.contains(&body),
                "the preview drew a cell whose occupant is not hit"
            );
        }
    }
}

#[test]
fn there_is_no_tactical_view_without_a_tactical_fight() {
    let mut game = game();
    assert!(!game.in_tactical_battle());
    assert!(game.tactical_view().is_none());
    assert!(game.tactical_shape_cells(0, (0, 0)).is_empty());
}

/// Turns the tactical model on for this run.
fn with_tactical_on(game: &mut Game) {
    let mut profile = game.profile().clone();
    profile.tactical_battles = true;
    game.install_profile(profile);
}

/// A pack of one hostile standing beside the player, unopened.
fn loose_pack(game: &mut Game, guardian: bool) -> Vec<Entity> {
    let player = game.player_entity();
    let at = *game
        .world
        .get::<Position>(player)
        .expect("the player stands somewhere");
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .expect("at least one species ships");
    let body = game.world.spawn((
        Creature {
            species: species.id.clone(),
        },
        Hostile,
        Position {
            x: at.x + 1,
            y: at.y,
        },
        Stats {
            hp: 10,
            max_hp: 10,
            atk: 1,
            mitigation: 0,
        },
        StatusEffects::default(),
    ));
    let id = body.id();
    if guardian {
        // A nest to be tethered to. What it *is* does not matter here; that
        // the pack carries a `NestGuardian` at all is the whole gate.
        let nest = game
            .world
            .spawn(Position {
                x: at.x + 4,
                y: at.y,
            })
            .id();
        game.world
            .entity_mut(id)
            .insert(crate::components::NestGuardian { nest });
    }
    vec![id]
}

#[test]
fn the_toggle_on_the_surface_is_what_opens_a_tactical_fight() {
    let mut game = game();
    with_tactical_on(&mut game);
    let pack = loose_pack(&mut game, false);
    game.start_battle(pack);

    assert!(game.in_tactical_battle(), "the tactical model took it");
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "and the abstract one did not"
    );
}

#[test]
fn the_toggle_off_leaves_every_fight_abstract() {
    let mut game = game();
    let pack = loose_pack(&mut game, false);
    game.start_battle(pack);

    assert!(!game.in_tactical_battle());
    assert!(game.world.get_resource::<BattleState>().is_some());
}

/// The pursuit path is shared by nest guardians and town patrols, so the
/// scope split cannot be a per-call-site decision — inspecting the pack is
/// what separates them.
#[test]
fn a_nest_guardian_is_fought_abstract_however_the_toggle_is_set() {
    let mut game = game();
    with_tactical_on(&mut game);
    let pack = loose_pack(&mut game, true);
    game.start_battle(pack);

    assert!(!game.in_tactical_battle(), "a guardian stays abstract");
    assert!(game.world.get_resource::<BattleState>().is_some());
}

#[test]
fn the_stack_stays_abstract_however_the_toggle_is_set() {
    let mut game = game();
    with_tactical_on(&mut game);
    let pack = loose_pack(&mut game, false);
    game.world.insert_resource(crate::resources::Locale::Stack {
        depth: 1,
        frames: 3,
        x: 1,
        y: 1,
        facing: crate::stack::Dir::North,
        entrance: (0, 0),
    });
    game.start_battle(pack);

    assert!(!game.in_tactical_battle(), "the Stack keeps its own model");
    assert!(game.world.get_resource::<BattleState>().is_some());
}
