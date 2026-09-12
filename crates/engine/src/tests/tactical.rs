//! The `Game`-facing half of the tactical battle model.

use crate::Experience;
use crate::Game;
use crate::components::{Creature, Hostile, Position, Stats, StatusEffects};
use crate::resources::{BattleState, DifficultyMode, Party};
use crate::species::SpeciesDb;
use crate::tactical::TacticalBattle;
use crate::tactical::reach::allowance;
use crate::tactical::turn::StepOutcome;
use crate::tests::support::{equip_weapon, generic_species, test_assets_dir};
use crate::tuning::{DEFAULT_BASE_SPEED, PLAYER_BASE_SPEED, TACTICAL_MOVE_MAX};
use bevy_ecs::prelude::Entity;

fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A body of `species`, standing nowhere in particular. Only its species
/// and its speed matter here.
fn body(game: &mut Game, species: &str) -> Entity {
    game.world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Position { x: 0, y: 0 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 3,
                mitigation: 0,
            },
        ))
        .id()
}

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

fn log_texts(game: &Game) -> Vec<String> {
    game.message_log(crate::MESSAGE_LOG_CAP)
        .into_iter()
        .map(|l| l.text)
        .collect()
}

/// A tactical fight opened around `count` hostiles standing next to the
/// player, each on `hp`.
fn tactical_fight(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let pack = tactical_pack(game, count, hp);
    game.open_tactical_battle(pack.clone());
    pack
}

/// `count` hostiles standing next to the player, with no fight opened around
/// them yet — so a test may open one on a bearing of its own.
fn tactical_pack(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let player = game.player_entity();
    let at = *game
        .world
        .get::<Position>(player)
        .expect("the player stands somewhere");
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species ships");
    let pack: Vec<Entity> = (0..count)
        .map(|i| {
            game.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Hostile,
                    Position {
                        x: at.x + 1 + i as i32,
                        y: at.y,
                    },
                    Stats {
                        hp,
                        max_hp: hp,
                        atk: 1,
                        mitigation: 0,
                    },
                    StatusEffects::default(),
                ))
                .id()
        })
        .collect();
    pack
}

/// Hands turns on until it is `who`'s again, or the fight ends. Bounded, so
/// a model that stops handing the turn on fails rather than hangs.
fn wait_for_turn(game: &mut Game, who: Entity) -> bool {
    for _ in 0..64 {
        match game.tactical_actor() {
            None => return false,
            Some(actor) if actor == who => return true,
            Some(_) => game.tactical_end_turn(),
        }
    }
    panic!("the turn never came back round");
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
    assert!(game.tactical_drive_turn(), "the arena door drove nobody");
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
            game.tactical_drive_turn(),
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
        !game.world.resource::<TacticalBattle>().acted(),
        "a fresh turn came in already spent"
    );
}

/// A free walkable cell orthogonally or diagonally adjacent to `cell`.
fn beside(game: &Game, cell: (i32, i32)) -> Option<(i32, i32)> {
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

/// A free walkable cell on the board's western edge.
fn western_edge(game: &Game) -> (i32, i32) {
    let battle = game.world.resource::<TacticalBattle>();
    (0..battle.board.side)
        .map(|y| (0, y))
        .find(|&at| battle.board.walkable(at.0, at.1) && battle.occupant(at).is_none())
        .expect("a walkable cell on the western edge")
}

/// Teardown parity: a tactical fight pays and closes through the same door a
/// group fight does — the win headline, the XP, and both fight resources
/// gone.
#[test]
fn killing_the_last_hostile_ends_the_fight_and_pays_for_it() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 1);
    let player = game.player_entity();
    let target = pack[0];
    let before = game.world.get::<Experience>(player).unwrap().xp;

    for _ in 0..64 {
        if game.world.get_resource::<TacticalBattle>().is_none() {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let at = game.world.resource::<TacticalBattle>().cell_of(target);
        let Some(at) = at else { break };
        if let Some(beside) = beside(&game, at) {
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, beside);
        }
        game.tactical_attack(target);
    }

    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the fight is still open"
    );
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "a group fight was opened along the way"
    );
    assert!(
        game.world.get::<Stats>(target).is_none(),
        "the dead hostile is still standing"
    );
    let lines = log_texts(&game);
    assert!(
        lines.iter().any(|t| *t == "You won!"),
        "the win was never announced: {lines:#?}"
    );
    assert!(
        game.world.get::<Experience>(player).unwrap().xp > before,
        "the kill paid no experience"
    );
}

/// A tactical fight's telemetry says what the model actually is: one group
/// per hostile body, because groups dissolve on a battle map, and a party
/// list read off `Party` rather than off a battle line it does not have.
#[test]
fn a_tactical_fight_reports_a_group_per_body_and_a_matching_end() {
    let mut game = game();
    game.enable_battle_telemetry();
    let pack = tactical_fight(&mut game, 2, 1);
    let player = game.player_entity();

    for _ in 0..64 {
        if game.world.get_resource::<TacticalBattle>().is_none() {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let target = pack
            .iter()
            .copied()
            .find(|&e| game.world.resource::<TacticalBattle>().cell_of(e).is_some());
        let Some(target) = target else { break };
        let at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(target)
            .unwrap();
        if let Some(beside) = beside(&game, at) {
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, beside);
        }
        game.tactical_attack(target);
    }

    let records = game.take_battle_telemetry();
    let start = records
        .iter()
        .find_map(|r| match r {
            crate::telemetry::Record::FightStart {
                fight,
                party,
                enemies,
                ..
            } => Some((*fight, party.len(), enemies.clone())),
            _ => None,
        })
        .expect("a tactical fight opened without a FightStart");
    assert_eq!(start.1, 1, "the party list read off a battle line it lacks");
    assert_eq!(start.2.len(), 2, "the pack was not one group per body");
    assert!(
        start.2.iter().all(|g| g.count == 1),
        "a group on a battle map holds more than one body"
    );

    let end = records
        .iter()
        .find_map(|r| match r {
            crate::telemetry::Record::FightEnd { fight, won, .. } => Some((*fight, *won)),
            _ => None,
        })
        .expect("the fight ended without a FightEnd");
    assert_eq!(end.0, start.0, "the end names a different fight");
    assert!(end.1, "clearing the board did not read as a win");
}

/// The gate roughly a hundred refusals ask through knows about both models.
#[test]
fn a_tactical_fight_counts_as_an_active_battle() {
    let mut game = game();
    assert!(!game.has_active_battle());
    tactical_fight(&mut game, 1, 10);
    assert!(
        game.has_active_battle(),
        "every screen that refuses mid-fight would have opened on a battle map"
    );
}

/// A free walkable cell next to `cell`, for a test that needs two bodies
/// standing beside each other rather than wherever deployment put them.
fn free_neighbour(game: &Game, cell: (i32, i32)) -> (i32, i32) {
    let battle = game.world.resource::<TacticalBattle>();
    [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (-1, -1),
        (1, -1),
        (-1, 1),
    ]
    .into_iter()
    .map(|(dx, dy)| (cell.0 + dx, cell.1 + dy))
    .find(|&at| battle.board.walkable(at.0, at.1) && battle.occupant(at).is_none())
    .expect("a body with no free cell beside it")
}

fn hp_of(game: &Game, body: Entity) -> i32 {
    game.world
        .get::<Stats>(body)
        .expect("a body with no stats")
        .hp
}

/// Installs `routine` as the acting body's only one, so its index is zero.
fn only_routine(game: &mut Game, body: Entity, routine: &str) {
    game.world
        .entity_mut(body)
        .insert(crate::components::Routines(vec![routine.to_string()]));
}

/// Full friendly fire, and the assertion is a *heal*: a patch centred on the
/// player mends the hostile standing beside them, because `recipients` never
/// reads `Hostile`. A blast would prove the same thing through a roll that
/// can miss.
#[test]
fn a_routine_lands_on_everyone_inside_its_shape_whichever_side_they_are_on() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "mirror_restore");
    assert!(wait_for_turn(&mut game, player));

    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    let beside = free_neighbour(&game, at);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(pack[0], beside)
    );
    game.world.get_mut::<Stats>(pack[0]).unwrap().hp = 10;
    let before = hp_of(&game, pack[0]);

    assert!(
        game.tactical_use_routine(0, at),
        "a patch aimed at the caster's own cell was refused"
    );
    assert!(
        hp_of(&game, pack[0]) > before,
        "the patch spared the hostile standing inside it — recipients read a side"
    );
}

/// The action ends the turn, whatever the action was.
#[test]
fn running_a_routine_hands_the_turn_on() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "mirror_restore");
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();

    assert!(game.tactical_use_routine(0, at));
    assert_ne!(
        game.tactical_actor(),
        Some(player),
        "the routine ran and the player kept the turn"
    );
}

/// Every refusal lands before anything is spent — the Power, the cooldown
/// and the turn alike.
#[test]
fn a_routine_aimed_out_of_range_is_refused_before_it_is_charged() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    // `WholeParty` derives a range of 0..0: it is aimed at the invoker's own
    // cell and nowhere else.
    only_routine(&mut game, player, "mirror_restore");
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();
    let beside = free_neighbour(&game, at);
    let power = game
        .world
        .get::<crate::components::PowerReserve>(player)
        .map(|r| r.get());

    assert!(
        !game.tactical_use_routine(0, beside),
        "an out-of-range aim ran"
    );
    assert_eq!(
        game.world
            .get::<crate::components::PowerReserve>(player)
            .map(|r| r.get()),
        power,
        "a refused routine was charged anyway"
    );
    assert!(
        game.world
            .get::<crate::components::AbilityCooldowns>(player)
            .is_none_or(|c| c.0.is_empty()),
        "a refused routine armed its cooldown"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "a refused routine cost the turn"
    );
}

/// A field-only routine has nothing to resolve against a body on a battle
/// map, and a passive is never chosen at all — `battle_special_options`'
/// two exclusions, applied at the other end.
#[test]
fn a_routine_that_is_never_run_in_a_fight_is_refused_on_a_battle_map() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    let field_only = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .all()
        .find(|d| d.effect.field_only())
        .expect("no field-only routine ships")
        .id
        .clone();
    only_routine(&mut game, player, &field_only);
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .unwrap();

    assert!(!game.tactical_use_routine(0, at));
    assert_eq!(game.tactical_actor(), Some(player));
}

/// A capture on a battle map is aimed at a body, and it is the same capture:
/// `decompile_body`, reached through a cell rather than a group index.
#[test]
fn a_capture_on_a_battle_map_turns_the_program_it_was_aimed_at() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "decompile");
    game.world.get_mut::<Stats>(pack[0]).unwrap().hp = 1;
    game.world
        .get_mut::<crate::components::Decompiler>(player)
        .unwrap()
        .skill = 50;
    crate::tests::support::set_inventory(&mut game, &[(crate::items::ids::ICE_BREAKER, 50)]);

    for _ in 0..50 {
        if game
            .world
            .get::<crate::components::Tamed>(pack[0])
            .is_some()
        {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(pack[0])
            .expect("the target left the board");
        // Walk into reach: a capture is a `Single` at arm's length.
        while game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .is_some_and(|from| crate::tactical::reach::distance(from, at) > 1)
        {
            let from = game
                .world
                .resource::<TacticalBattle>()
                .cell_of(player)
                .unwrap();
            let dir = ((at.0 - from.0).signum(), (at.1 - from.1).signum());
            if game.tactical_step(dir) != StepOutcome::Moved {
                break;
            }
        }
        // Out of reach with the turn's movement spent: hand the turn on and
        // close the rest of the gap on the next one.
        if !game.tactical_use_routine(0, at) {
            game.tactical_end_turn();
        }
    }

    assert!(
        game.world
            .get::<crate::components::Tamed>(pack[0])
            .is_some(),
        "the program was never captured"
    );
    assert!(
        game.world.get::<Hostile>(pack[0]).is_none(),
        "a captured program is still hostile"
    );
    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the last hostile left the board and the fight stayed open"
    );
}

/// Hands the fight round until `bound` turns have passed, driving every
/// hostile through the AI and ending every party body's turn at once.
/// Bounded, so a model that stops handing the turn on fails rather than
/// hangs.
fn run_ai_rounds(game: &mut Game, bound: usize) {
    for _ in 0..bound {
        if game.tactical_actor().is_none() {
            return;
        }
        if !game.tactical_ai_turn() {
            game.tactical_end_turn();
        }
    }
}

fn cell_of(game: &Game, body: Entity) -> Option<(i32, i32)> {
    game.world.resource::<TacticalBattle>().cell_of(body)
}

/// The whole point of the file: a hostile left to itself closes the
/// deployment gap and swings, so a tactical fight finishes without the
/// player driving both sides.
#[test]
fn a_hostile_closes_the_deployment_gap_on_its_own_and_swings() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    let opened = crate::tactical::reach::distance(
        cell_of(&game, player).expect("the player is on the board"),
        cell_of(&game, pack[0]).expect("the hostile is on the board"),
    );
    let before = hp_of(&game, player);

    run_ai_rounds(&mut game, 40);

    assert!(
        hp_of(&game, player) < before,
        "a hostile that crossed {opened} cells never swung: {before} HP untouched"
    );
}

/// A companion's turn is the player's turn, and the screen has to say so.
///
/// **The third predicate.** `tactical_ai_actor` and
/// `Game::tactical_awaits_input` are deliberately one definition, gated on
/// `Hostile`; `TacticalView::player_turn` was a third, gated on `Player` —
/// so on a companion's turn app-core sat waiting for a key while the screen
/// said the wild side was moving and offered no keys to press.
#[test]
fn a_companion_s_turn_reads_as_the_player_s_on_the_screen() {
    let mut game = game();
    let companion = body(&mut game, &generic_species().id);
    game.world.resource_mut::<Party>().0.push(companion);
    tactical_fight(&mut game, 1, 40);
    assert!(
        wait_for_turn(&mut game, companion),
        "the companion never got a turn"
    );

    assert!(
        game.tactical_awaits_input(),
        "the engine is waiting on a key for this body"
    );
    let view = game.tactical_view().expect("the fight is open");
    assert!(
        view.player_turn,
        "...and the screen must agree: a companion's turn is the player's, so \
         the keybar owes it the action keys rather than `the wild side is moving`"
    );
}

/// A hostile's turn is nobody's to command, on both surfaces.
#[test]
fn a_hostile_s_turn_reads_as_the_wild_side_s() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let hostile = pack[0];
    assert!(wait_for_turn(&mut game, hostile), "the hostile never acted");

    assert!(!game.tactical_awaits_input());
    let view = game.tactical_view().expect("the fight is open");
    assert!(!view.player_turn, "the screen offered keys for a wild body");
}

/// The AI drives one side. A party body's turn is the player's to spend, and
/// `false` is what tells a driver to wait for input rather than to hand the
/// turn on.
#[test]
fn a_party_body_s_turn_is_not_the_ai_s_to_drive() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the player never acted");
    let stood = cell_of(&game, player);

    assert!(
        !game.tactical_ai_turn(),
        "the AI claimed a turn that belongs to the player"
    );
    assert_eq!(cell_of(&game, player), stood, "the AI moved the player");
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "a refused turn must not be handed on"
    );
}

/// The trap this phase was shaped around. A hostile holds no `PowerReserve`
/// by design, so `ability_unavailable` refuses it every priced routine there
/// is — and every routine that can be run is priced. Routed through the
/// player's door the AI's routine arm would compile, test green and never
/// once fire.
///
/// Asserted in both halves deliberately: that the player's gate really does
/// refuse this hostile this routine, and that it runs anyway.
///
/// Read off the log and the cooldown after **one** turn rather than off a
/// finished fight: teardown wipes every battle-scoped component, so a
/// cooldown asked about after the last blow is gone whether it was armed or
/// not.
#[test]
fn a_hostile_runs_a_routine_the_player_s_own_gate_would_refuse_it() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    only_routine(&mut game, wild, "acid_wash");
    let def = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .get("acid_wash")
        .cloned()
        .expect("acid_wash ships");
    assert!(
        game.ability_unavailable(wild, &def).is_some(),
        "a hostile with no reserve must be refused a priced routine, or this \
         test is not covering the trap it was written for"
    );

    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    assert!(
        log_texts(&game).iter().any(|l| l.contains(&def.name)),
        "the hostile never ran the routine it was carrying: {:?}",
        log_texts(&game)
    );
    assert_eq!(
        game.world
            .get::<crate::components::AbilityCooldowns>(wild)
            .and_then(|c| c.0.get("acid_wash").copied()),
        Some(crate::abilities::armed_cooldown(
            def.cooldown,
            crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN
        )),
        "a routine that ran must be cooled at the enemy side's own floor"
    );
}

/// A hostile's routine is floored at `ENEMY_ROUTINE_MIN_COOLDOWN`, so one
/// whose file authors no cooldown cannot be run every turn of the fight.
///
/// The fixture is a shipped routine with its cooldown edited to zero,
/// because **no shipped routine can reach this branch**: `field_only_dead_
/// fields` warns about a cooldown on a field-only effect, so every shipped
/// `cooldown: 0` routine is field-only and `wild_routine_ready` excludes it.
/// The branch guards a mod, and the edit is what stands in for one — driven
/// through a real turn, since the floor the AI *passes* is the half a call
/// to `run_tactical_routine` would not cover.
#[test]
fn a_hostile_s_routine_is_floored_even_when_its_file_authors_no_cooldown() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    let mut free = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .get("acid_wash")
        .cloned()
        .expect("acid_wash ships");
    free.cooldown = 0;
    game.world
        .resource_mut::<crate::abilities::AbilityDb>()
        .insert(free.clone());
    only_routine(&mut game, wild, &free.id);

    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    assert_eq!(
        game.world
            .get::<crate::components::AbilityCooldowns>(wild)
            .and_then(|c| c.0.get(&free.id).copied()),
        Some(crate::abilities::armed_cooldown(
            0,
            crate::tuning::ENEMY_ROUTINE_MIN_COOLDOWN
        )),
        "a routine its file left uncooled must still be floored for a hostile"
    );
}

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
    /// A fight with its one hostile marooned in the far corner, where no
    /// allowance closes on anybody, so its turn is a walk and nothing else.
    fn marooned() -> (Game, Entity) {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let wild = pack[0];
        assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
        let side = game.world.resource::<TacticalBattle>().board.side;
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(wild, (side - 1, side - 1)),
            "the far corner must be standable"
        );
        (game, wild)
    }

    fn next_draw(game: &mut Game) -> u64 {
        use rand::RngExt;
        game.world
            .resource_mut::<crate::resources::GameRng>()
            .0
            .random::<u64>()
    }

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
        .next()
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

/// A hostile's turn hands the turn on **once**.
///
/// `tactical_attack` and `tactical_use_routine` both end the turn
/// themselves — the action ends the turn — so an AI turn that ends it again
/// at the tail spends two rungs of the order and skips whoever came next.
/// With one hostile and one party body that is a fight the player never
/// gets a turn in: the hostile takes every turn, for ever.
#[test]
fn a_hostiles_turn_costs_exactly_one_rung_of_the_order() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let hostile = pack[0];
    assert!(wait_for_turn(&mut game, hostile), "the hostile gets a turn");

    let order: Vec<Entity> = game
        .world
        .resource::<TacticalBattle>()
        .initiative()
        .to_vec();
    let at = order
        .iter()
        .position(|&e| e == hostile)
        .expect("the hostile is in the order");
    let next = order[(at + 1) % order.len()];

    assert!(game.tactical_ai_turn(), "the AI drove the hostile's turn");

    assert_eq!(
        game.tactical_actor(),
        Some(next),
        "the hostile's turn skipped the body that came after it"
    );
}

/// The player is handed control after a lone hostile has swung, which is
/// the shape every wandering encounter has.
#[test]
fn a_one_on_one_fight_hands_control_back_to_the_player() {
    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();

    for _ in 0..8 {
        if game.tactical_awaits_input() {
            assert_eq!(game.tactical_actor(), Some(player));
            return;
        }
        assert!(game.tactical_ai_turn(), "the AI has a turn to drive");
    }
    panic!("the wild side never handed the turn back");
}

/// A walkable, empty cell at least `clear` away from every body on the
/// board — somewhere a blast can be aimed with nobody but its invoker in it.
fn lonely_cell(game: &Game, clear: u32) -> (i32, i32) {
    let battle = game.world.resource::<TacticalBattle>();
    let bodies: Vec<(i32, i32)> = battle.bodies().map(|(_, cell)| cell).collect();
    battle
        .board
        .cells()
        .map(|(cell, _)| cell)
        .find(|&(x, y)| {
            battle.board.walkable(x, y)
                && battle.occupant((x, y)).is_none()
                && bodies
                    .iter()
                    .all(|&b| crate::tactical::reach::distance(b, (x, y)) > clear)
        })
        .expect("no cell on the board is clear of every body")
}

/// The order as it stands, and who follows `who` around it.
fn after(game: &mut Game, who: Entity) -> Entity {
    let order: Vec<Entity> = game
        .tactical_view()
        .expect("no fight is open")
        .order
        .iter()
        .map(|row| row.entity)
        .collect();
    let idx = order
        .iter()
        .position(|&e| e == who)
        .expect("that body is not in the order");
    order[(idx + 1) % order.len()]
}

/// A body killed by its own action has already left the order, and
/// `TacticalBattle::remove` handed the turn on when it went. Ending the turn
/// again on top of that skips whoever was standing behind it — a companion
/// who fumbles fatally costs the player their turn.
#[test]
fn a_body_that_kills_itself_with_its_own_action_hands_the_turn_on_once() {
    let mut game = game();
    // A companion rather than a hostile: a hostile holds no `PowerReserve`,
    // so the player's door refuses it every priced routine there is.
    let actor = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, actor);
    tactical_fight(&mut game, 1, 400);
    assert!(wait_for_turn(&mut game, actor));
    only_routine(&mut game, actor, "cascade_overflow");

    // Alone in its own blast: the roll is forced for one recipient, so the
    // one that matters has to be the only one there is.
    let alone = lonely_cell(&game, 3);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(actor, alone)
    );
    game.world.get_mut::<Stats>(actor).unwrap().hp = 1;
    let next = after(&mut game, actor);
    crate::tests::support::force_the_next_attack_to_land(&mut game);

    assert!(
        game.tactical_use_routine(0, alone),
        "the blast was refused before it could land"
    );
    assert!(
        !game.creature_alive(actor),
        "the blast spared its own invoker — this fixture needs a lethal roll"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(next),
        "the turn was handed on twice: the body behind the one that died never acted"
    );
}

/// A round on a battle map spends the upkeep an abstract round spends —
/// cooldowns and status effects tick, and the world clock moves. Without it
/// every routine is once per fight and a fight costs the world no time at
/// all.
#[test]
fn a_round_on_a_battle_map_cools_a_routine_and_spends_a_world_tick() {
    let mut game = game();
    tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    only_routine(&mut game, player, "cascade_overflow");
    assert!(wait_for_turn(&mut game, player));
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player was not seated");
    assert!(
        game.tactical_use_routine(0, at),
        "the blast was refused before it could be charged"
    );
    let armed = cooldown_of(&game, player, "cascade_overflow");
    assert!(armed > 0, "a routine with a cooldown was not put on one");

    let round = game.tactical_view().expect("the fight closed").round;
    let tick = game.current_tick();
    for _ in 0..64 {
        if game.tactical_actor().is_none() {
            break;
        }
        if game.tactical_view().is_some_and(|v| v.round > round) {
            break;
        }
        game.tactical_end_turn();
    }

    assert!(
        cooldown_of(&game, player, "cascade_overflow") < armed,
        "a full round passed and the routine never cooled"
    );
    assert!(
        game.current_tick() > tick,
        "a round of a battle map cost the world no time"
    );
}

fn cooldown_of(game: &Game, body: Entity, routine: &str) -> u32 {
    game.world
        .get::<crate::components::AbilityCooldowns>(body)
        .and_then(|c| c.0.get(routine).copied())
        .unwrap_or(0)
}

/// A defeat is absorbed inside the fight that lands it, exactly as the
/// abstract model's trailing tick absorbs one. Left to the next idle tick,
/// the player walks off a battle map at zero Integrity and reboots a moment
/// later on the map.
#[test]
fn a_player_dropped_on_a_battle_map_does_not_walk_away_dead() {
    let mut game = game();
    tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    only_routine(&mut game, player, "cascade_overflow");
    assert!(wait_for_turn(&mut game, player));

    let alone = lonely_cell(&game, 3);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, alone)
    );
    game.world.get_mut::<Stats>(player).unwrap().hp = 1;
    crate::tests::support::force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_use_routine(0, alone));

    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the player went down and the fight stayed open"
    );
    assert!(
        game.world.get::<Stats>(player).unwrap().hp > 0,
        "the player left the battle map dead: the reboot was deferred"
    );
}

/// A capture is aimed at something hostile. Aimed at one of your own it used
/// to spend the catalyst and, on a good roll, hand the companion back
/// through `roster_parts` — a fresh `ProgramId`, level one, no memories.
#[test]
fn a_capture_refuses_a_body_of_your_own() {
    let mut game = game();
    let friend = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, friend);
    let level_before = game.world.get::<Experience>(friend).map(|e| e.level);
    let id_before = game
        .world
        .get::<crate::components::ProgramId>(friend)
        .map(|p| p.0);

    tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    only_routine(&mut game, player, "decompile");
    game.world
        .get_mut::<crate::components::Decompiler>(player)
        .unwrap()
        .skill = 50;
    crate::tests::support::set_inventory(&mut game, &[(crate::items::ids::ICE_BREAKER, 50)]);
    assert!(wait_for_turn(&mut game, player));

    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(friend)
        .expect("the companion was not seated");
    let beside = beside(&game, at).expect("no cell beside the companion");
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(player, beside)
    );
    let catalysts = catalysts_held(&game);

    assert!(
        !game.tactical_use_routine(0, at),
        "a capture aimed at your own companion was accepted"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "the refused capture spent the turn"
    );
    assert_eq!(
        catalysts_held(&game),
        catalysts,
        "the refused capture spent a catalyst"
    );
    assert_eq!(
        game.world.get::<Experience>(friend).map(|e| e.level),
        level_before,
        "the companion was handed back through roster_parts"
    );
    assert_eq!(
        game.world
            .get::<crate::components::ProgramId>(friend)
            .map(|p| p.0),
        id_before,
        "the companion was minted a new identity"
    );
}

/// The results page draws from `BattleTimeline::closing`, which only the
/// group model ever filled — so a tactical fight ended on a blank screen
/// with the win, the salvage and the XP written nowhere the player looks.
#[test]
fn a_finished_tactical_fight_leaves_a_results_roster() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 1);
    let player = game.player_entity();
    let target = pack[0];

    for _ in 0..64 {
        if game.world.get_resource::<TacticalBattle>().is_none() {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let Some(at) = game.world.resource::<TacticalBattle>().cell_of(target) else {
            break;
        };
        if let Some(beside) = beside(&game, at) {
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, beside);
        }
        game.tactical_attack(target);
    }
    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the fight is still open"
    );

    let view = game
        .battle_result_view()
        .expect("a finished tactical fight left no results roster: the screen draws nothing");
    let named: Vec<String> = view.party.iter().map(|slot| slot.name.clone()).collect();
    assert!(
        named.iter().any(|name| name == "You"),
        "the results roster names no player: {named:?}"
    );
    assert!(
        view.groups.is_empty(),
        "the board was cleared and the results still list a hostile"
    );
}

fn catalysts_held(game: &Game) -> u32 {
    game.world
        .get::<crate::components::Inventory>(game.player_entity())
        .expect("the player carries nothing")
        .count(&crate::items::ItemId::from(crate::items::ids::ICE_BREAKER))
}

/// The round's upkeep can kill, and what it killed has to be answered for
/// *before* the world tick — the order `battle_resolve_round` keeps. Ticked
/// first, `difficulty::death_handling_system` reboots a Forgiving player
/// inside a fight that is still open: they read as alive again, the fight
/// never ends, and their world `Position` has been warped to the anchor
/// while they stand on the board.
#[test]
fn a_bleed_that_kills_the_player_at_a_round_boundary_ends_the_fight() {
    let mut game = game();
    tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 1;
    game.world
        .get_mut::<crate::components::StatusEffects>(player)
        .unwrap()
        .active = Some(crate::components::ActiveStatus {
        kind: crate::components::StatusKind::Bleed,
        remaining: 4,
        power: 20,
        landed_this_round: false,
    });

    let round = game.tactical_view().expect("the fight closed").round;
    for _ in 0..64 {
        if game.tactical_actor().is_none() {
            break;
        }
        if game.tactical_view().is_some_and(|v| v.round > round) {
            break;
        }
        game.tactical_end_turn();
    }

    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the bleed took the player down and the fight stayed open around them"
    );
}

/// `TacticalBattle::remove` wraps the order itself, so a body that dies on
/// the *last* rung starts the next round without `end_turn` ever being
/// called. Read off `end_turn` alone, that round's upkeep is skipped: no
/// cooldowns come down, no status ticks, and the world clock stands still
/// for a round nobody can see went by.
#[test]
fn a_round_begun_by_a_death_on_the_last_rung_still_costs_its_upkeep() {
    let mut game = game();
    let actor = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, actor);
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    // Seated by hand: the property is about the *last* rung, and initiative
    // is rolled off speed.
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0], actor]);
    assert!(wait_for_turn(&mut game, actor));
    only_routine(&mut game, actor, "cascade_overflow");

    let alone = lonely_cell(&game, 3);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(actor, alone)
    );
    game.world.get_mut::<Stats>(actor).unwrap().hp = 1;
    crate::tests::support::force_the_next_attack_to_land(&mut game);
    let tick = game.current_tick();

    assert!(game.tactical_use_routine(0, alone));
    assert!(
        !game.creature_alive(actor),
        "the blast spared its own invoker — this fixture needs a lethal roll"
    );
    assert!(
        game.current_tick() > tick,
        "the death started a new round and its upkeep was never spent"
    );
}

/// A brace has to actually cost the next swing something, or it is a spent
/// turn dressed up as a choice.
///
/// The swing is driven by hand rather than through `tactical_ai_turn`,
/// because the AI's cell pick draws and what is being measured is the
/// mitigation, not the AI's aim. The stream is reseeded *after* the brace
/// and immediately before the swing, so the two runs meet the swing on an
/// identical stream and the only difference between them is the buff —
/// `force_the_next_attack_to_land` cannot do that job here, since
/// `swing_move` rolls the move first and eats the forced roll. The seed is
/// searched for rather than pinned, `first_rng_seed_where`'s rule.
///
/// Seated by hand for the reason the last-rung test is: initiative is
/// rolled off speed, and a player landing on the *last* rung has their
/// brace ticked off by the wrap before anybody swings.
#[test]
fn bracing_reduces_what_the_next_swing_lands() {
    let damage_taken = |brace: bool, stream: u64| {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 400);
        let player = game.player_entity();
        let wild = pack[0];
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![player, wild]);

        // Hard enough that a fifth off it is visible in whole points, and
        // survivable so the fight is still open when the HP is read back.
        game.world.get_mut::<Stats>(wild).unwrap().atk = 60;
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.max_hp = 4000;
            stats.hp = 4000;
        }
        let beside = {
            let battle = game.world.resource::<TacticalBattle>();
            let (px, py) = battle.cell_of(player).expect("the player is seated");
            (px + 1, py)
        };
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(wild, beside),
            "fixture: the hostile must stand within reach"
        );

        if brace {
            assert!(game.tactical_defend(), "the player may brace on its turn");
        } else {
            game.tactical_end_turn();
        }
        assert_eq!(
            game.tactical_actor(),
            Some(wild),
            "fixture: the hostile must be the one swinging"
        );

        crate::tests::support::reseed_rng(&mut game, stream);
        let before = game.world.get::<Stats>(player).unwrap().hp;
        assert!(game.tactical_attack(player), "the hostile must reach");
        before - game.world.get::<Stats>(player).unwrap().hp
    };

    // A stream on which the swing lands enough for a fifth of it to be a
    // whole point at all — every matchup has a miss chance by design.
    let stream = (0..512u64)
        .find(|&s| damage_taken(false, s) >= 5)
        .expect("no stream in 0..512 landed the swing");
    let open = damage_taken(false, stream);
    let braced = damage_taken(true, stream);
    assert!(
        braced < open,
        "bracing cost the swing nothing: {braced} against {open}"
    );
}

/// Bracing is an action, and an action ends the turn — the same rule
/// `tactical_attack` and `tactical_use_routine` hold.
#[test]
fn bracing_ends_the_turn() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);

    assert!(game.tactical_defend());
    assert_eq!(
        game.tactical_actor(),
        Some(pack[0]),
        "a brace that does not hand the turn on lets a body brace for ever"
    );
}

/// A body that has already spent its action may not brace on top of it.
#[test]
fn a_body_that_has_acted_may_not_brace() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    let raw = game.effective_mitigation(player);
    game.world.resource_mut::<TacticalBattle>().mark_acted();

    assert!(!game.tactical_defend(), "an acted body braced anyway");
    assert_eq!(
        game.effective_mitigation(player),
        raw,
        "the refusal armed the buff on its way out"
    );
}

/// The brace lasts the rest of the round and no longer: it is armed for one
/// round and the order coming back round is what spends that.
#[test]
fn a_brace_is_gone_once_the_order_comes_round() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    let raw = game.effective_mitigation(player);

    assert!(game.tactical_defend());
    assert_eq!(
        game.effective_mitigation(player),
        raw + crate::tuning::DEFEND_MITIGATION_BONUS,
        "the brace never took"
    );

    // The last rung's turn ending is what wraps the round, and the wrap is
    // what spends the upkeep the buff ages under.
    game.tactical_end_turn();
    assert_eq!(
        game.effective_mitigation(player),
        raw,
        "the brace outlived the round it was armed for"
    );
}

/// The brace's one accepted cost, pinned rather than reasoned about: a body
/// on the **last** rung braces against nobody, because `hand_on_turn`'s wrap
/// fires the moment it hands the turn on and the wrap is what ages the buff.
///
/// Stated in `CLAUDE.md`, in the `seams` skill and in
/// `seam:a-brace-on-a-board-is-the-group-models-and-only-the`, so it is a
/// claim that has to be checked rather than remembered — and it is the thing
/// that would silently become false if the brace were ever rearmed for longer.
#[test]
fn a_body_on_the_last_rung_braces_against_nobody() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    // The player last, where the wrap lands directly behind its turn.
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![pack[0], player]);
    assert!(wait_for_turn(&mut game, player));
    let raw = game.effective_mitigation(player);

    assert!(game.tactical_defend());

    assert_eq!(
        game.effective_mitigation(player),
        raw,
        "the last rung's brace outlived the wrap that follows it"
    );
}

/// A hostile's approach is walked, not teleported: every beat that steps
/// moves it exactly one cell, and it takes as many beats as the walk is
/// long.
///
/// **The regression this whole seam exists for.** Committed as one
/// placement, a hostile crossed its entire allowance between two frames and
/// read as a teleport.
#[test]
fn a_hostiles_walk_is_spent_one_cell_a_beat() {
    use crate::tactical::ai::AiBeat;
    use crate::tactical::reach::distance;

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");

    let mut at = cell_of(&game, wild).expect("the hostile is on the board");
    let mut steps = 0;
    loop {
        match game.tactical_ai_beat() {
            AiBeat::Stepped => {
                let now = cell_of(&game, wild).expect("the walker left the board");
                assert_eq!(distance(at, now), 1, "a beat crossed more than one cell");
                at = now;
                steps += 1;
                assert!(steps <= TACTICAL_MOVE_MAX, "the walk never ended");
            }
            AiBeat::Acted => break,
            AiBeat::Idle => panic!("the hostile's turn was not the AI's to drive"),
        }
    }
    assert!(
        steps > 1,
        "the fixture never made the hostile walk, so nothing was tested"
    );
}

/// The walk is planned on the beat that takes its first step and read back
/// by every beat after it — **one `GameRng` draw a turn, not one a cell**.
///
/// The case that holds it is a walk already spent: `Some(vec![])` is a body
/// that has arrived and `None` is one that has not chosen yet, and a beat
/// that read the two as one would plan a fresh approach — and draw again —
/// every beat for the rest of the turn instead of acting.
#[test]
fn a_spent_walk_acts_rather_than_planning_a_fresh_one() {
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    assert_eq!(
        game.tactical_ai_beat(),
        AiBeat::Stepped,
        "the hostile had no approach to walk"
    );

    // Arrived: the plan is made and there is nothing left of it.
    game.world
        .resource_mut::<TacticalBattle>()
        .commit_walk(Vec::new());
    let stood = cell_of(&game, wild).expect("the walker left the board");

    assert_eq!(
        game.tactical_ai_beat(),
        AiBeat::Acted,
        "a spent walk was re-planned instead of acted on"
    );
    assert_eq!(
        cell_of(&game, wild).or(Some(stood)),
        Some(stood),
        "the body walked again on a turn it had already finished walking"
    );
}

/// The two granularities are one turn. A fight watched a beat at a time and
/// the same fight resolved in a single call have to reach the same board, or
/// the arena is measuring a different game from the one being played.
#[test]
fn a_turn_and_the_beats_it_is_made_of_reach_the_same_board() {
    use crate::tactical::ai::AiBeat;

    let mut whole = game();
    let mut paced = game();
    let wild_whole = tactical_fight(&mut whole, 1, 200)[0];
    let wild_paced = tactical_fight(&mut paced, 1, 200)[0];
    assert!(wait_for_turn(&mut whole, wild_whole));
    assert!(wait_for_turn(&mut paced, wild_paced));

    assert!(whole.tactical_ai_turn(), "the whole turn was not driven");
    for _ in 0..=TACTICAL_MOVE_MAX {
        if paced.tactical_ai_beat() == AiBeat::Acted {
            break;
        }
    }

    assert_eq!(
        cell_of(&whole, wild_whole),
        cell_of(&paced, wild_paced),
        "the paced fight ended the turn somewhere else"
    );
    let player = whole.player_entity();
    assert_eq!(
        hp_of(&whole, player),
        hp_of(&paced, paced.player_entity()),
        "the paced fight landed a different blow"
    );
}

/// What a driver reads to know it owes a step rather than a turn. A body
/// that has not set off yet is not walking — which is what buys the beat of
/// anticipation before it does — and one that has arrived is not either.
#[test]
fn a_body_is_walking_only_between_its_first_step_and_its_last() {
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 200);
    let wild = pack[0];
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    assert!(
        !game.tactical_walking(),
        "a body that has chosen nothing yet was called mid-walk"
    );

    assert_eq!(game.tactical_ai_beat(), AiBeat::Stepped);
    assert!(
        game.tactical_walking(),
        "a body mid-approach reads as still"
    );

    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_ai_beat() == AiBeat::Acted {
            break;
        }
    }
    assert!(
        !game.tactical_walking(),
        "the turn ended with a walk still owed"
    );
}

/// The player unarmed swings at arm's length, and nothing else.
#[test]
fn an_unarmed_body_swings_at_arms_length() {
    let game = game();
    let player = game.player_entity();
    assert_eq!(
        game.swing_range(player),
        crate::tuning::TACTICAL_MELEE_RANGE
    );
}

/// A species carrying a `ranged` move reaches past arm's length.
#[test]
fn a_ranged_species_reaches_past_arms_length() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    assert_eq!(
        game.swing_range(shooter),
        crate::tuning::TACTICAL_RANGED_MOVE_RANGE,
        "Drone's Recon Ping is `ranged: true`"
    );
}

/// A species with no ranged move stays at arm's length.
#[test]
fn a_melee_species_stays_at_arms_length() {
    let mut game = game();
    let bruiser = body(&mut game, "construct");
    assert_eq!(
        game.swing_range(bruiser),
        crate::tuning::TACTICAL_MELEE_RANGE,
        "Construct authors no ranged move"
    );
}

/// A worn weapon **replaces** the species figure rather than being maxed
/// against it — `Game::attack_range`'s precedent, where worn damage replaces
/// a natural band outright. A ranged program holding a melee blade swings at
/// arm's length, because the weapon is what it is swinging.
#[test]
fn a_worn_weapon_replaces_the_species_range() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    equip_weapon(&mut game, shooter, "shim_blade");
    assert_eq!(
        game.swing_range(shooter),
        crate::tuning::TACTICAL_MELEE_RANGE
    );

    equip_weapon(&mut game, shooter, "plasma_router");
    assert_eq!(game.swing_range(shooter), 3);
}

/// Stands `player` and `other` on chosen cells inside an open fight.
///
/// `move_to` rather than `place` — both are already seated by the time a
/// fight is open, and `place` refuses a body that is on the board.
fn place_bodies(
    game: &mut Game,
    player: Entity,
    at: (i32, i32),
    other: Entity,
    theirs: (i32, i32),
) {
    place_one(game, player, at);
    place_one(game, other, theirs);
}

fn place_one(game: &mut Game, body: Entity, cell: (i32, i32)) {
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(body, cell),
        "{cell:?} would not take a body"
    );
}

/// Puts sight-blocking cover on one cell. `Cover` and not `Blocked` —
/// `blocks_sight` is true of exactly one kind, and the two are deliberately
/// not complements.
fn block_cell(game: &mut Game, cell: (i32, i32)) {
    game.world.resource_mut::<TacticalBattle>().board.put(
        cell.0,
        cell.1,
        crate::tactical::map::BattleCell::Cover,
    );
}

/// Opens a fight and hands the turn to the player, with the board's own
/// furniture cleared off the cells a test is about to use.
fn ranged_fight(game: &mut Game, count: usize) -> Vec<Entity> {
    let pack = tactical_fight(game, count, 20);
    let player = game.player_entity();
    assert!(wait_for_turn(game, player), "the player never got a turn");
    {
        let battle = &mut *game.world.resource_mut::<TacticalBattle>();
        for x in 0..6 {
            for y in 0..3 {
                battle
                    .board
                    .put(x, y, crate::tactical::map::BattleCell::Open);
            }
        }
    }
    pack
}

/// A swing at exactly the weapon's range lands. Its other half is below, and
/// the two are one pair — at the bound and one past it.
#[test]
fn a_swing_lands_at_exactly_its_range() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router"); // range 3
    place_bodies(&mut game, player, (0, 0), pack[0], (3, 0));
    assert!(
        game.tactical_attack(pack[0]),
        "three cells is inside range 3"
    );
}

/// One cell past the range is refused.
#[test]
fn a_swing_one_cell_past_its_range_is_refused() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router"); // range 3
    place_bodies(&mut game, player, (0, 0), pack[0], (4, 0));
    assert!(!game.tactical_attack(pack[0]), "four cells is past range 3");
}

/// Cover blocks a swing. Delete the `line_of_sight` check and this must
/// fail — a test that passes with the fix removed is not coverage.
#[test]
fn a_ranged_swing_is_blocked_by_cover() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    block_cell(&mut game, (1, 0));
    assert!(
        !game.tactical_attack(pack[0]),
        "cover did not stop the swing"
    );
}

/// The same swing with the cover gone lands — so the test above is measuring
/// the cover and not the placement.
#[test]
fn the_same_swing_lands_once_the_cover_is_gone() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    assert!(game.tactical_attack(pack[0]));
}

/// An adjacent swing is untouched by the sight check — `line_of_sight`
/// excludes its endpoints, so for neighbours its loop is empty.
#[test]
fn an_adjacent_swing_is_unaffected_by_the_sight_check() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
}

/// A sweep fired from range still sweeps. The shape is cast from the actor
/// toward the aim whatever the distance, so a reach weapon that quietly went
/// single-target at range would read as a nerf rather than a bug.
#[test]
fn a_reach_weapon_still_sweeps_when_fired_from_range() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 2);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "scatter_lance"); // range 2, WholeEnemyGroup
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    place_one(&mut game, pack[1], (2, 1));
    let before = hp_of(&game, pack[1]);
    crate::tests::support::force_the_next_attack_to_land(&mut game);
    assert!(game.tactical_attack(pack[0]));
    assert!(
        hp_of(&game, pack[1]) < before,
        "the neighbour was not caught by a sweep fired from two cells"
    );
}
