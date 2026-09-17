//! The `Game`-facing half of the tactical battle model.

use crate::Experience;
use crate::Game;
use crate::components::{Creature, Hostile, Position, Rarity, Squad, Stats, StatusEffects};
use crate::resources::{BattleState, DifficultyMode, Party};
use crate::species::SpeciesDb;
use crate::tactical::TacticalBattle;
use crate::tactical::reach::allowance;
use crate::tactical::turn::StepOutcome;
use crate::tests::support::{
    equip_weapon, generic_species, insert_battle, spawn_wild_on_player_tile, test_assets_dir,
};
use crate::tuning::{DEFAULT_BASE_SPEED, PLAYER_BASE_SPEED, TACTICAL_MOVE_MAX};
use bevy_ecs::prelude::Entity;

fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A body of `species`, standing nowhere in particular. Only its species
/// and its speed matter here.
pub(super) fn body(game: &mut Game, species: &str) -> Entity {
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

pub(super) fn log_texts(game: &Game) -> Vec<String> {
    game.message_log(crate::MESSAGE_LOG_CAP)
        .into_iter()
        .map(|l| l.text)
        .collect()
}

/// A tactical fight opened around `count` hostiles standing next to the
/// player, each on `hp`.
pub(super) fn tactical_fight(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
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
pub(super) fn wait_for_turn(game: &mut Game, who: Entity) -> bool {
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
        game.world.resource::<TacticalBattle>().actions_left() > 0,
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
pub(super) fn free_neighbour(game: &Game, cell: (i32, i32)) -> (i32, i32) {
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
pub(super) fn only_routine(game: &mut Game, body: Entity, routine: &str) {
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

/// A fight with its one hostile marooned in the far corner, where no
/// allowance closes on anybody, so its turn is a walk and nothing else —
/// and, with the corner open on more than one side, more than one candidate
/// cell scores.
pub(super) fn marooned() -> (Game, Entity) {
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

/// The next value `GameRng` yields — the "did this turn draw" probe shared
/// by every test that pins a turn to argmax and by `tamper.rs`'s temperature
/// tests, which read it as `tamper_next_draw` before this was lifted here.
pub(super) fn next_draw(game: &mut Game) -> u64 {
    use rand::RngExt;
    game.world
        .resource_mut::<crate::resources::GameRng>()
        .0
        .random::<u64>()
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

/// Nothing carries a `Squad` yet, so every body — the player's own included —
/// gets exactly one action. Task 4's whole reason `actions_per_turn` takes an
/// entity at all.
#[test]
fn every_body_gets_one_action_without_a_squad() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert_eq!(game.actions_per_turn(player), 1);
    assert_eq!(game.actions_per_turn(pack[0]), 1);
}

/// `hand_on_turn` hands the turn on only once no actions are left, and the
/// existing "still the one acting" guard stays alongside the new gate — a
/// body cannot be exercised this way for real yet, since no body carries
/// more than one action, so `actions_left` is driven directly.
#[test]
fn a_body_with_two_actions_keeps_its_turn_after_the_first() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

    assert!(game.tactical_defend(), "the first brace was refused");
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "a second action still owed was handed to the next body early"
    );
    assert_eq!(
        game.world.resource::<TacticalBattle>().actions_left(),
        1,
        "the first of two actions did not spend itself"
    );

    assert!(game.tactical_defend(), "the second brace was refused");
    assert_eq!(
        game.tactical_actor(),
        Some(pack[0]),
        "the last of two actions did not hand the turn on"
    );
}

/// `tactical_attack` reads `actions_left == 0` where it used to read
/// `acted` — a body with nothing left to spend may not swing.
#[test]
fn a_body_with_no_actions_left_may_not_attack() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    // Beside the player, not wherever deployment put it — the refusal has
    // to be `actions_left`'s, not a range or sight refusal that would pass
    // whether or not the gate this test names is even there.
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
    let before = hp_of(&game, pack[0]);
    game.world.resource_mut::<TacticalBattle>().spend_action();

    assert!(!game.tactical_attack(pack[0]), "a spent body swung anyway");
    assert_eq!(
        hp_of(&game, pack[0]),
        before,
        "the refusal still landed a blow"
    );
}

/// The movement allowance is the *turn's*, not one action's — a body walks
/// once for the whole turn however many actions it buys, so what it has
/// already spent must carry from one action into the next and reset only
/// once the turn actually ends.
#[test]
fn two_actions_share_one_turns_movement_allowance() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![player, pack[0]]);
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);
    game.world.resource_mut::<TacticalBattle>().spend(2);

    assert!(game.tactical_defend(), "the first brace was refused");
    assert_eq!(
        game.world.resource::<TacticalBattle>().spent(),
        2,
        "movement already spent this turn must carry into its second action"
    );

    assert!(game.tactical_defend(), "the second brace was refused");
    assert_eq!(
        game.world.resource::<TacticalBattle>().spent(),
        0,
        "the next body's own turn must start with nothing spent"
    );
}

/// After landing an action with another still owed, `run_tactical_beat`
/// clears the walk rather than ending the turn, so the next action plans a
/// fresh one from wherever this one left the body standing.
#[test]
fn a_second_action_plans_a_fresh_walk_rather_than_ending_the_turn() {
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 400);
    let wild = pack[0];
    assert!(wait_for_turn(&mut game, wild));
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

    // Closing the deployment gap can cost a walk of its own beats before the
    // first action lands — every one of them must report `Stepped`, since
    // none of them may end the turn early.
    let mut beats = 0;
    loop {
        assert_eq!(
            game.tactical_ai_beat(),
            AiBeat::Stepped,
            "neither a walk step nor a landed first action may end the turn"
        );
        beats += 1;
        assert!(beats < 20, "the hostile never closed on the player to act");
        if game.world.resource::<TacticalBattle>().actions_left() < 2 {
            break;
        }
    }
    assert_eq!(
        game.world.resource::<TacticalBattle>().actions_left(),
        1,
        "the first action was not spent"
    );
    assert!(
        !game.world.resource::<TacticalBattle>().walk_planned(),
        "the walk was not cleared for the second action to plan its own"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(wild),
        "the body with an action left lost its turn early"
    );

    assert_eq!(
        game.tactical_ai_beat(),
        AiBeat::Acted,
        "the second of two actions must end the turn"
    );
    assert_eq!(
        game.world.resource::<TacticalBattle>().actions_left(),
        1,
        "the next body's own fresh budget must not read as the first's leftover"
    );
}

/// A routine's cooldown arms the moment it runs, not when the turn ends —
/// otherwise a body with two actions could run the same one-shot routine
/// twice in the same turn.
#[test]
fn a_routines_cooldown_arms_the_action_it_runs_in_not_the_turn_it_ends() {
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
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    assert_eq!(
        log_texts(&game)
            .iter()
            .filter(|l| l.contains(&def.name))
            .count(),
        1,
        "the routine ran twice in one turn — its cooldown did not arm until \
         the turn ended"
    );
}

/// `actions_left` must not resurrect a turn `TacticalBattle::remove` already
/// handed on: a body with two actions that kills itself with the first still
/// does not get a second — the body behind it acts next, exactly as it does
/// with one action.
#[test]
fn a_body_with_two_actions_that_kills_itself_on_the_first_gets_no_second() {
    let mut game = game();
    // A companion rather than a hostile: a hostile holds no `PowerReserve`,
    // so the player's door refuses it every priced routine there is.
    let actor = crate::tests::support::spawn_tamed(&mut game, 40, 3);
    crate::tests::support::enlist(&mut game, actor);
    tactical_fight(&mut game, 1, 400);
    assert!(wait_for_turn(&mut game, actor));
    only_routine(&mut game, actor, "cascade_overflow");
    game.world
        .resource_mut::<TacticalBattle>()
        .set_actions_left(2);

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
        "the dead body's second action resurrected its turn"
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
    win_a_lone_tactical_fight(&mut game);

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

/// The results popup is drawn over the board the fight ended on, and
/// `finish_fight` removes `TacticalBattle` — so without a copy taken there,
/// the map pane has nothing to draw under it but the surface.
#[test]
fn a_finished_tactical_fight_leaves_its_board_frozen() {
    let mut game = game();
    win_a_lone_tactical_fight(&mut game);

    let board = game
        .tactical_result_view()
        .expect("a finished tactical fight left no board to draw under its results");
    assert!(
        board
            .bodies
            .iter()
            .any(|body| body.entity == game.player_entity()),
        "the frozen board lost the player"
    );
    assert!(
        board.bodies.iter().all(|body| !body.is_hostile),
        "a won fight's board still stands a hostile on it"
    );
    // Nothing is acting on a finished board: a turn arrow or a reach wash
    // there offers a move there is no fight left to spend.
    assert_eq!(board.active, None, "the frozen board still names an actor");
    assert!(
        board.reachable.is_empty(),
        "the frozen board still washes reach"
    );
    assert!(!board.player_turn, "the frozen board still awaits input");
}

/// The popup lists what the fight came to, and nothing of the blow-by-blow:
/// the same lines `prune_battle_narration` keeps, read before it runs.
#[test]
fn a_finished_tactical_fight_reports_only_its_outcomes() {
    let mut game = game();
    win_a_lone_tactical_fight(&mut game);

    let outcomes = game.battle_outcomes();
    assert!(
        outcomes.iter().any(|entry| entry.text == "You won!"),
        "the outcome lines never say the fight was won: {outcomes:?}"
    );
    assert!(
        outcomes
            .iter()
            .all(|entry| entry.kind.survives_battle_prune()),
        "a narration line reached the outcomes: {outcomes:?}"
    );
}

/// A group fight after a tactical one must not inherit its board.
#[test]
fn a_group_fight_leaves_no_frozen_board() {
    let mut game = game();
    win_a_lone_tactical_fight(&mut game);
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.end_battle(player, None);
    assert!(
        game.tactical_result_view().is_none(),
        "a group fight's results drew the last tactical board"
    );
}

/// Fights a one-body tactical pack to its end, the player walking into reach
/// of it every turn.
fn win_a_lone_tactical_fight(game: &mut Game) {
    let pack = tactical_fight(game, 1, 1);
    let player = game.player_entity();
    let target = pack[0];

    for _ in 0..64 {
        if game.world.get_resource::<TacticalBattle>().is_none() {
            break;
        }
        if !wait_for_turn(game, player) {
            break;
        }
        let Some(at) = game.world.resource::<TacticalBattle>().cell_of(target) else {
            break;
        };
        if let Some(beside) = beside(game, at) {
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
    game.world.resource_mut::<TacticalBattle>().spend_action();

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

pub(super) fn place_one(game: &mut Game, body: Entity, cell: (i32, i32)) {
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

/// A body standing at two cells fires the ranged half of its pair, never the
/// melee half — the AI decides its intent before it walks, and a roll that
/// could draw a move it cannot fire makes a planned standoff a coin flip.
#[test]
fn a_body_at_range_rolls_only_a_move_that_reaches() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    for _ in 0..20 {
        let rolled = game
            .roll_species_move_in_range(shooter, Some(2))
            .expect("Drone has moves");
        assert!(rolled.ranged, "rolled {} at two cells", rolled.name);
    }
}

/// Adjacent, either half of the pair is fair game — which is the variety
/// `swing_move` exists for.
#[test]
fn a_body_adjacent_may_roll_either_move() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    let mut saw_melee = false;
    for _ in 0..40 {
        let rolled = game
            .roll_species_move_in_range(shooter, Some(1))
            .expect("Drone has moves");
        saw_melee |= !rolled.ranged;
    }
    assert!(saw_melee, "the melee half was never drawn in forty rolls");
}

/// The group model passes `None` and is untouched.
#[test]
fn the_group_model_rolls_over_every_move() {
    let mut game = game();
    let shooter = body(&mut game, "drone");
    let mut saw_melee = false;
    let mut saw_ranged = false;
    for _ in 0..40 {
        let rolled = game.roll_species_move(shooter).expect("Drone has moves");
        saw_melee |= !rolled.ranged;
        saw_ranged |= rolled.ranged;
    }
    assert!(saw_melee && saw_ranged, "the unconstrained roll narrowed");
}

/// A hostile of `species` standing beside the player, with a fight opened
/// around it. `tactical_pack` takes the *first* species in the db, so a test
/// that needs a particular one spawns its own.
fn fight_against(game: &mut Game, species: &str) -> Entity {
    let player = game.player_entity();
    let at = *game
        .world
        .get::<Position>(player)
        .expect("the player stands somewhere");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.to_string(),
            },
            Hostile,
            Position {
                x: at.x + 1,
                y: at.y,
            },
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 4,
                mitigation: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    game.open_tactical_battle(vec![wild]);
    wild
}

/// A reaching hostile swings from where it stands instead of closing, and
/// the move it swings with is the reaching half of its pair.
#[test]
fn a_reaching_hostile_swings_without_closing() {
    let mut game = game();
    let wild = fight_against(&mut game, "drone");
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
    place_bodies(&mut game, player, (0, 0), wild, (2, 0));
    {
        let battle = &mut *game.world.resource_mut::<TacticalBattle>();
        for x in 0..6 {
            for y in 0..4 {
                battle
                    .board
                    .put(x, y, crate::tactical::map::BattleCell::Open);
            }
        }
    }

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");
    // The *distance* and not the cell: holding the band is what is being
    // asserted, and a body that sidesteps to another cell two out has held
    // it exactly as well as one that stood still.
    let held = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(wild)
        .expect("the hostile left the board");
    assert_eq!(
        crate::tactical::reach::distance((0, 0), held),
        crate::tuning::TACTICAL_RANGED_MOVE_RANGE,
        "the reaching hostile closed instead of holding its band, ending at {held:?}"
    );
    assert!(
        log_texts(&game).iter().any(|l| l.contains("Recon Ping")),
        "the reaching hostile never swung its reaching move: {:?}",
        log_texts(&game)
    );
}

/// One streak per body actually swung at, from the swinger's cell to each
/// recipient's — so a reach weapon's sweep fires one at every body its shape
/// caught.
#[test]
fn a_swing_queues_one_bolt_per_body_it_lands_on() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "plasma_router");
    place_bodies(&mut game, player, (0, 0), pack[0], (3, 0));
    assert!(game.tactical_attack(pack[0]));

    let bolts = game.take_bolts();
    assert_eq!(bolts.len(), 1, "one body swung at, one streak");
    assert_eq!(bolts[0].from, (0, 0));
    assert_eq!(bolts[0].to, (3, 0));
}

/// A sweep fires one at every body its shape caught, not one at the aim.
#[test]
fn a_sweep_queues_a_bolt_per_body_it_swept() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 2);
    let player = game.player_entity();
    equip_weapon(&mut game, player, "scatter_lance");
    place_bodies(&mut game, player, (0, 0), pack[0], (2, 0));
    place_one(&mut game, pack[1], (2, 1));
    assert!(game.tactical_attack(pack[0]));

    let bolts = game.take_bolts();
    assert_eq!(bolts.len(), 2, "two bodies swept, two streaks: {bolts:?}");
    assert!(bolts.iter().all(|b| b.from == (0, 0)));
}

/// A swing on a battle map is heard off its own queue, so a fight long enough
/// to push the log past `MESSAGE_LOG_CAP` still sounds. The log is filled
/// first because that is the fight that went silent: sound read the round's
/// range by position, and a range pinned at the cap never grew.
#[test]
fn a_swing_is_heard_however_full_the_log_is() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    for i in 0..2 * crate::MESSAGE_LOG_CAP {
        game.log(format!("filler {i}"));
    }
    game.take_swing_cues();

    assert!(game.tactical_attack(pack[0]));

    let swung = game
        .message_log(crate::MESSAGE_LOG_CAP)
        .iter()
        .filter_map(|line| line.outcome)
        .collect::<Vec<_>>();
    assert_eq!(swung.len(), 1, "one swing logged");
    assert_eq!(game.take_swing_cues(), swung);
    assert!(game.take_swing_cues().is_empty(), "a drain is a drain");
}

/// The group model is heard through the reveal, which paces its cues to the
/// narration, so a swing off the board must not queue a second copy.
#[test]
fn a_swing_off_the_board_queues_no_cue() {
    let mut game = game();
    game.log_swing(
        crate::resources::MessageKind::PartyDamage,
        crate::battle::AttackOutcome::Crit { dmg: 12 },
        "you tear it clean through",
    );
    assert!(game.take_swing_cues().is_empty());
}

/// Draining is a drain — a second call comes back empty, so a frontend
/// cannot draw one streak twice.
#[test]
fn taking_the_bolts_empties_the_queue() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
    assert_eq!(game.take_bolts().len(), 1);
    assert!(game.take_bolts().is_empty());
}

/// An adjacent swing queues one too — the streak is one rule at every
/// distance, and at one cell it is the melee feedback.
#[test]
fn an_adjacent_swing_queues_a_bolt_too() {
    let mut game = game();
    let pack = ranged_fight(&mut game, 1);
    let player = game.player_entity();
    place_bodies(&mut game, player, (0, 0), pack[0], (1, 0));
    assert!(game.tactical_attack(pack[0]));
    assert_eq!(game.take_bolts().len(), 1);
}

// --- A body's own hit or heal on a battle map --------------------------
//
// `Game::apply_damage` and `Game::restore_hp` are the two doors, so the
// engine side of the whole feature is a queue push inside each — see
// `TacticalFxQueue`. The stream is searched for rather than pinned,
// `bracing_reduces_what_the_next_swing_lands`'s rule: a swing has a miss
// chance by design, so a fixed seed would read as flaky the day the combat
// tables move.

/// A landed blow cues a hit at the defender's own board cell — the same
/// door `apply_damage` always damages a creature through, reached here by
/// an ordinary melee swing.
#[test]
fn a_landed_swing_queues_a_hit_cue_at_the_defenders_cell() {
    fn attempt(seed: u64) -> (Game, Entity) {
        let mut game = game();
        let pack = ranged_fight(&mut game, 1);
        let player = game.player_entity();
        let target = pack[0];
        place_bodies(&mut game, player, (0, 0), target, (1, 0));
        crate::tests::support::reseed_rng(&mut game, seed);
        game.tactical_attack(target);
        (game, target)
    }

    let (mut game, _target) = (0..512u64)
        .map(attempt)
        .find(|(game, target)| hp_of(game, *target) < 20)
        .expect("no stream in 0..512 landed the swing");

    // The cell `place_bodies` seated the defender on, read back rather than
    // re-derived from `TacticalBattle::cell_of` after the swing: a hard
    // enough blow reaps the defender off the board entirely, and the cue's
    // own cell was captured before that could happen.
    let cues = game.take_tactical_fx();
    assert_eq!(
        cues,
        vec![crate::resources::TacticalFxCue {
            pos: (1, 0),
            kind: crate::resources::TacticalFxKind::Hit,
        }],
        "a landed blow must cue exactly one hit at the defender's cell"
    );
}

/// A miss or a zero-damage call cues nothing — `apply_damage`'s own gate on
/// `dealt > 0`, exercised directly so the case does not depend on finding an
/// unlucky stream.
#[test]
fn a_hit_with_nothing_dealt_cues_no_tactical_fx() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 20);
    let target = pack[0];
    assert_eq!(
        game.apply_damage(target, 0),
        0,
        "fixture: zero damage must land as zero"
    );
    assert!(
        game.take_tactical_fx().is_empty(),
        "zero damage must not have cued a hit"
    );
}

/// A heal on a body standing on the board cues a `+` at its cell — full
/// friendly fire's own fixture, since `mirror_restore` is what proved
/// `restore_hp` reaches a hostile standing inside a player-centred patch.
#[test]
fn a_heal_queues_a_heal_cue_at_the_recipients_cell() {
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

    assert!(
        game.tactical_use_routine(0, at),
        "a patch aimed at the caster's own cell was refused"
    );

    let cues = game.take_tactical_fx();
    assert!(
        cues.contains(&crate::resources::TacticalFxCue {
            pos: beside,
            kind: crate::resources::TacticalFxKind::Heal,
        }),
        "the healed hostile's cell must carry a heal cue: {cues:?}"
    );
}

/// A heal that restores nothing — a full-health recipient — cues nothing,
/// `restore_hp`'s own gate on `restored > 0`.
#[test]
fn a_heal_that_restores_nothing_cues_no_tactical_fx() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let target = pack[0];
    assert_eq!(
        game.restore_hp(target, 10),
        0,
        "fixture: a full-health target must have nothing to restore"
    );
    assert!(
        game.take_tactical_fx().is_empty(),
        "a no-op heal must not have cued anything"
    );
}

/// Seating a forked program on a battle map: beside the invoker, behind the
/// cursor, and gone whichever way the fight ends.
mod summons {
    use super::*;
    use crate::components::Summoned;

    /// A fight with `count` hostiles and one fork already seated beside the
    /// player.
    fn fight_with_a_fork(game: &mut Game, count: usize) -> Entity {
        tactical_fight(game, count, 40);
        let player = game.player_entity();
        let body = game.fork_programs(player, 1, 0)[0];
        assert!(
            game.seat_summon_on_board(player, body),
            "there is room beside the player"
        );
        body
    }

    #[test]
    fn a_fork_lands_on_a_free_walkable_cell_beside_its_invoker() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 2);
        let battle = game.world.resource::<TacticalBattle>();
        let player = battle
            .cell_of(game.player_entity())
            .expect("the player stands somewhere");
        let at = battle.cell_of(body).expect("the fork was placed");
        assert!(battle.board.walkable(at.0, at.1));
        assert_eq!(
            battle.bodies().filter(|&(_, cell)| cell == at).count(),
            1,
            "and on nobody else's cell"
        );
        assert!(
            (at.0 - player.0).abs() <= 1 && (at.1 - player.1).abs() <= 1,
            "beside the invoker: {at:?} against {player:?}"
        );
    }

    /// *A count of turns taken is conserved under a cursor shift and would
    /// pass against the bug*, so this compares the order **by identity**.
    #[test]
    fn splicing_a_fork_costs_nobody_else_a_turn() {
        let mut game = game();
        tactical_fight(&mut game, 3, 40);
        let before: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();
        // Somewhere in the middle of the order, so an insertion ahead of the
        // cursor would have somewhere to go wrong.
        game.tactical_end_turn();
        let cursor = game
            .world
            .resource::<TacticalBattle>()
            .actor()
            .expect("somebody is acting");

        let player = game.player_entity();
        let body = game.fork_programs(player, 1, 0)[0];
        assert!(game.seat_summon_on_board(player, body));

        let after: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();
        let spliced: Vec<Entity> = after.iter().copied().filter(|&e| e != body).collect();
        assert_eq!(spliced, before, "nobody else moved in the order");
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(cursor),
            "and the cursor still names the body that was acting"
        );
        let at = after.iter().position(|&e| e == body).expect("it is in");
        let on = after.iter().position(|&e| e == cursor).expect("so is it");
        assert_eq!(at, on + 1, "immediately behind the cursor: it acts next");
    }

    /// A fork drives itself, which is the first exception to "every party
    /// body is the player's to command".
    #[test]
    fn a_fork_takes_its_own_turn_without_waiting_for_input() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 2);
        assert!(wait_for_turn(&mut game, body));
        assert!(
            !game.tactical_awaits_input(),
            "the fight would hang waiting for a key nobody may press"
        );
        assert!(game.tactical_ai_turn(), "and the AI drove it");
    }

    /// The two endings only a battle map can reach. Task 1's sweep covers
    /// the group model's teardown, but a jack-out and a walk off the edge
    /// have never run it.
    #[test]
    fn no_fork_survives_a_jack_out() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 2);
        let player = game.player_entity();
        assert!(wait_for_turn(&mut game, player));
        let edge = western_edge(&game);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, edge)
        );

        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);
        assert!(game.world.get_resource::<TacticalBattle>().is_none());
        assert!(
            game.world.get::<Stats>(body).is_none(),
            "a jack-out is one of the five endings, and the sweep covers it"
        );
    }

    #[test]
    fn a_fork_that_walks_off_the_edge_is_still_swept_at_the_end() {
        let mut game = game();
        let body = fight_with_a_fork(&mut game, 1);
        assert!(wait_for_turn(&mut game, body));
        let edge = western_edge(&game);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(body, edge)
        );
        assert_eq!(game.tactical_step((-1, 0)), StepOutcome::Departed);
        assert!(
            game.world.get::<Stats>(body).is_some(),
            "breaking off is not dying"
        );
        assert!(
            game.world.get::<Summoned>(body).is_some(),
            "and it is still a fork"
        );

        // Now the player walks out too, so the fight tears down around a
        // body that is no longer on the board at all.
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
            game.world.get::<Stats>(body).is_none(),
            "the sweep is over every holder, not over the board"
        );
    }

    /// Every refusal before anything is spent — `commit_caravan_basket`'s
    /// rule, which `tactical_use_routine` already states for its six.
    #[test]
    fn a_fork_with_nowhere_to_stand_is_refused_and_spends_nothing() {
        let mut game = game();
        tactical_fight(&mut game, 2, 40);
        let player = game.player_entity();
        // Fill the board, so `nearest_free` has nothing to answer with.
        let cells: Vec<(i32, i32)> = {
            let battle = game.world.resource::<TacticalBattle>();
            (0..battle.board.side)
                .flat_map(|x| (0..battle.board.side).map(move |y| (x, y)))
                .filter(|&(x, y)| battle.board.walkable(x, y))
                .filter(|&at| battle.occupant(at).is_none())
                .collect()
        };
        for at in cells {
            let filler = game.world.spawn(()).id();
            game.world
                .resource_mut::<TacticalBattle>()
                .place(filler, at);
        }

        let body = game.fork_programs(player, 1, 0)[0];
        assert!(
            !game.seat_summon_on_board(player, body),
            "there is nowhere for it to stand"
        );
        assert!(
            game.world
                .resource::<TacticalBattle>()
                .cell_of(body)
                .is_none()
        );
    }
}

/// Walking into a hostile is a swing, not a refusal.
///
/// The board's own bump, `move_player`'s ladder one space over: an occupied
/// cell used to answer `Refused`, so an arrow key pressed at the body the
/// whole turn was spent closing on did nothing at all and the player had to
/// find `a` to finish the approach.
///
/// Every stream must spend the action; a landing blow is *searched for*
/// rather than owed, on `bracing_reduces_what_the_next_swing_lands`' rule —
/// `force_the_next_attack_to_land` cannot pin a tactical swing, because the
/// move is rolled first and eats the forced roll, and every matchup has a
/// miss chance by design.
#[test]
fn walking_into_a_hostile_swings_at_it() {
    // The fight built fresh and bumped on `stream`: what the step answered,
    // what the blow cost, and whether the action went with it.
    let bump = |stream: u64| -> (StepOutcome, i32, bool) {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        let player = game.player_entity();
        assert!(wait_for_turn(&mut game, player), "the fight ended early");
        let at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(player)
            .expect("the player stands on the board");
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(pack[0], (at.0 + 1, at.1)),
            "the cell beside the player is taken"
        );
        let before = game
            .world
            .get::<Stats>(pack[0])
            .expect("the hostile is alive")
            .hp;

        crate::tests::support::reseed_rng(&mut game, stream);
        let outcome = game.tactical_step((1, 0));
        // Read as "the turn is no longer the player's" rather than off
        // `acted`: the action *ends* the turn, so `tactical_attack` hands it
        // on and `acted` is then answering about whoever came next. A fight
        // that ended inside the blow took the resource with it, which spent
        // the turn as surely.
        let spent = game.tactical_actor() != Some(player);
        let after = game.world.get::<Stats>(pack[0]).map(|s| s.hp).unwrap_or(0);
        (outcome, before - after, spent)
    };

    let (outcome, _, spent) = bump(0);
    assert_eq!(
        outcome,
        StepOutcome::Struck,
        "a step into a hostile was not a swing"
    );
    assert!(spent, "the bump spent no action");

    let cost = (0..512u64)
        .map(bump)
        .find(|&(_, cost, _)| cost > 0)
        .map(|(_, cost, _)| cost)
        .expect("no stream in 0..512 landed the bump");
    assert!(cost > 0, "the bump landed no blow");
}

/// ...and the player does not move onto the cell it swung at.
///
/// A bump that both swung and stepped would put two bodies on one cell,
/// which `reach::movement_field`'s occupancy rule has no way to express.
#[test]
fn a_bump_spends_the_action_and_not_the_step() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player stands on the board");
    game.world
        .resource_mut::<TacticalBattle>()
        .move_to(pack[0], (at.0 + 1, at.1));

    assert_eq!(game.tactical_step((1, 0)), StepOutcome::Struck);
    assert_eq!(
        game.world.resource::<TacticalBattle>().cell_of(player),
        Some(at),
        "the player walked onto the body it swung at"
    );
}

/// Walking into one of your own is still refused.
///
/// Friendly fire is full and legal through the aim cursor — that is what a
/// shape is worth aiming for — but an arrow key is not an aim, and a bump
/// that attacked whatever was in the way would make crossing your own line
/// a coin flip. The gate is `Hostile` and nothing else.
#[test]
fn walking_into_a_companion_is_refused_rather_than_a_swing() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(player)
        .expect("the player stands on the board");

    // A body of the player's own, stood next to them.
    let friend = body(&mut game, "nothing-in-particular");
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .place(friend, (at.0 + 1, at.1)),
        "the cell beside the player is taken"
    );
    let before = game
        .world
        .get::<Stats>(friend)
        .expect("the companion is alive")
        .hp;

    assert_eq!(game.tactical_step((1, 0)), StepOutcome::Refused);
    assert_eq!(
        game.world.get::<Stats>(friend).map(|s| s.hp),
        Some(before),
        "a bump into one of your own landed a blow"
    );
    assert!(
        game.world.resource::<TacticalBattle>().actions_left() > 0,
        "a refused bump spent the turn's action"
    );
}

/// A hostile's own approach is never turned into a swing by a step.
///
/// `Game::tactical_step` is the door the AI's walk goes through — the whole
/// point of that seam — so a bump that fired for anybody would let a hostile
/// spend its action part-way along a path it planned, and spend it on
/// whatever of its own side happened to be standing in the way.
/// `tactical_awaits_input` is the gate for that reason and not a new
/// predicate: it is false for exactly the bodies the beat loop drives.
///
/// Two hostiles, deliberately. A hostile stepping into the *player* is
/// refused by the `Hostile` gate alone, so a test written that way passes
/// with this one deleted.
#[test]
fn a_hostile_mid_walk_does_not_bump() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 40);
    assert!(wait_for_turn(&mut game, pack[0]), "the fight ended early");
    let at = game
        .world
        .resource::<TacticalBattle>()
        .cell_of(pack[0])
        .expect("the hostile stands on the board");
    let beside = (at.0 + 1, at.1);
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(pack[1], beside),
        "the cell beside the hostile is taken"
    );
    let before = game
        .world
        .get::<Stats>(pack[1])
        .expect("the second hostile is alive")
        .hp;

    assert_eq!(game.tactical_step((1, 0)), StepOutcome::Refused);
    assert_eq!(
        game.world.get::<Stats>(pack[1]).map(|s| s.hp),
        Some(before),
        "a hostile swung at its own side by walking into it"
    );
}

/// The door auto-attack drives the player's own side through, and the one
/// thing it has to be that `tactical_ai_beat` is not: willing to spend the
/// turn of a body the player commands.
///
/// Both halves, because "the auto door drove something" passes against a
/// door that is merely the AI's under a second name.
#[test]
fn the_auto_door_drives_a_party_body_the_ai_door_declines() {
    use crate::tactical::ai::AiBeat;

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    assert!(wait_for_turn(&mut game, player), "the fight ended early");

    assert_eq!(
        game.tactical_ai_beat(),
        AiBeat::Idle,
        "the AI door spent a turn that was the player's"
    );
    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_auto_beat() == AiBeat::Acted {
            break;
        }
    }
    assert_ne!(
        game.tactical_actor(),
        Some(player),
        "the auto door spent the turn without handing it on"
    );
}

/// A fight nobody touches resolves, and it resolves by the hostile being
/// beaten rather than by a party body wandering off the edge.
///
/// **The departure is the failure this rules out.** A step off the board is a
/// jack-out, so a driven party body that ever chose an edge cell would end
/// the fight with the hostile still standing — and the player's run would
/// leave a fight it was winning because nobody pressed a key.
#[test]
fn an_auto_driven_fight_is_won_rather_than_walked_out_of() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 10);

    for _ in 0..4000 {
        if !game.has_active_battle() {
            break;
        }
        game.tactical_auto_beat();
    }

    assert!(!game.has_active_battle(), "the auto fight never resolved");
    assert!(
        game.world.get::<Stats>(pack[0]).is_none_or(|s| s.hp <= 0),
        "the fight ended with the hostile still up, so somebody walked out"
    );
}

/// Auto-attack is basic attacks and nothing else, and the cooldown is how
/// that is visible: `run_tactical_routine` arms one, a swing arms nothing,
/// and the AI's routine picker bypasses `ability_unavailable` — so a party
/// body driven through that branch would invoke, for free, whatever it was
/// carrying.
///
/// Power is deliberately not the instrument: a round on a battle map spends a
/// world tick, and the tick drains Power by itself.
#[test]
fn an_auto_driven_party_body_swings_and_never_invokes() {
    use crate::components::AbilityCooldowns;
    use crate::tactical::ai::AiBeat;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut game = game();
    tactical_fight(&mut game, 1, 200);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    assert!(
        game.wild_routine_ready(player).is_some(),
        "the fixture left nothing to invoke, so nothing is being tested"
    );

    for _ in 0..=TACTICAL_MOVE_MAX {
        if game.tactical_auto_beat() == AiBeat::Acted {
            break;
        }
    }

    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
        "an auto-driven turn armed a routine's cooldown, so it invoked rather than swung"
    );
}

/// A board with one cell of cover between the two halves, and the two bodies
/// placed either side of it.
///
/// Hand-written rather than generated: where the cover falls is what is being
/// tested, and `map::generate` puts it wherever the seed says.
fn across_cover(game: &mut Game, wild: Entity) -> ((i32, i32), (i32, i32)) {
    use crate::tactical::map::Board;

    let player = game.player_entity();
    let mut battle = game.world.resource_mut::<TacticalBattle>();
    battle.board = Board::from_rows(&[
        ".......", ".......", ".......", "...#...", ".......", ".......", ".......",
    ]);
    assert!(battle.move_to(player, (3, 1)), "the player would not stand");
    assert!(battle.move_to(wild, (3, 5)), "the hostile would not stand");
    ((3, 1), (3, 5))
}

/// **A routine may not be thrown at what the thrower cannot see.** The swing
/// has always checked sight; a routine checked only range, and since every
/// shipped area routine derives a six-cell throw, that meant every blast in
/// the game landed in full through a solid wall.
///
/// The same routine at a visible cell is run immediately afterwards, from the
/// same fight and the same turn — so the refusal cannot be passing on Power,
/// a cooldown or a range the fixture got wrong.
#[test]
fn a_blast_may_not_be_thrown_through_cover() {
    use crate::components::AbilityCooldowns;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut game = game();
    let wild = tactical_fight(&mut game, 1, 200)[0];
    let (_, behind) = across_cover(&mut game, wild);
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);
    assert!(wait_for_turn(&mut game, player), "the fight ended early");
    let hp = hp_of(&game, wild);

    assert!(
        !game.tactical_use_routine(0, behind),
        "a blast was thrown through cover"
    );

    assert_eq!(hp_of(&game, wild), hp, "the refused blast still landed");
    assert!(
        game.world
            .get::<AbilityCooldowns>(player)
            .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
        "the refused blast armed its cooldown"
    );
    assert_eq!(
        game.tactical_actor(),
        Some(player),
        "the refused blast spent the turn"
    );
    assert!(
        game.tactical_use_routine(0, (3, 2)),
        "the same blast was refused at a cell in plain view, so the fixture \
         proves nothing about sight"
    );
}

/// The hostile side is held to the same rule, and the AI's aim is where that
/// lands: `best_aim` walks every cell in range and scores who the shape would
/// cover, so without the gate it picks the party's own cell through a wall.
///
/// A `Single` shape at range, because the blast half is already closed by
/// `shape_cells` — a `Radius` thrown through cover now covers nobody, so it
/// scores zero and is skipped whether or not the aim itself is gated. A shot
/// is the case that needs the gate.
///
/// The instrument is the **cooldown** and not the player's Integrity: a shot
/// that fired and missed leaves Integrity untouched too, so a test reading
/// damage would pass against a hostile firing through walls all day. The
/// hostile is then moved into plain sight and driven again, so a refusal that
/// was really the fixture failing to arm anything cannot pass either.
#[test]
fn a_hostile_will_not_shoot_through_cover() {
    use crate::abilities::{AbilityDb, AbilityRange, AbilityShape};
    use crate::components::AbilityCooldowns;

    let mut game = game();
    let wild = tactical_fight(&mut game, 1, 200)[0];
    across_cover(&mut game, wild);

    // A shipped single-target attack given the reach of a thrown one, and
    // stripped of its trigger: the shipped file authors no `range:` and so
    // derives arm's length, where no cell can lie between the two bodies and
    // sight can never be blocked at all.
    let mut def = game
        .world
        .resource::<AbilityDb>()
        .get("siphon_cycles")
        .expect("the shipped routine is loaded")
        .clone();
    def.id = "reaching_shot".to_string();
    def.shape = Some(AbilityShape::Single);
    def.range = Some(AbilityRange { min: 0, max: 6 });
    def.triggers = None;
    game.world.resource_mut::<AbilityDb>().insert(def);
    only_routine(&mut game, wild, "reaching_shot");

    let fired = |game: &Game| {
        game.world
            .get::<AbilityCooldowns>(wild)
            .is_some_and(|c| c.0.contains_key("reaching_shot"))
    };
    let drive = |game: &mut Game| {
        assert!(wait_for_turn(game, wild), "the fight ended early");
        // Pinned where it stands: left to walk it would step around the cover
        // and shoot from somewhere it can see, which is the rule working
        // rather than being skipped — and not what this measures.
        game.world
            .resource_mut::<TacticalBattle>()
            .commit_walk(Vec::new());
        game.tactical_ai_beat()
    };

    drive(&mut game);
    assert!(!fired(&game), "the hostile shot the player through a wall");

    // In plain view of the player, two cells short of the cover.
    assert!(
        game.world
            .resource_mut::<TacticalBattle>()
            .move_to(wild, (3, 2)),
        "the hostile would not stand in the open"
    );
    drive(&mut game);
    assert!(
        fired(&game),
        "the hostile would not shoot from a cell in plain view either, so the \
         refusal above says nothing about sight"
    );
}

/// A nine-cell open board with the player in the middle and the pack stood
/// where the test says, the player on enough Integrity to outlast every turn
/// a test drives.
pub(super) fn open_ground(game: &mut Game, pack: &[Entity], cells: &[(i32, i32)]) -> (i32, i32) {
    use crate::tactical::map::Board;

    let player = game.player_entity();
    if let Some(mut stats) = game.world.get_mut::<Stats>(player) {
        stats.hp = 10_000;
        stats.max_hp = 10_000;
    }
    let centre = (4, 4);
    let mut battle = game.world.resource_mut::<TacticalBattle>();
    battle.board = Board::from_rows(&[
        ".........",
        ".........",
        ".........",
        ".........",
        ".........",
        ".........",
        ".........",
        ".........",
        ".........",
    ]);
    // Parked out of the way first, so no body is refused a cell another has
    // not left yet.
    for (i, &body) in pack.iter().enumerate() {
        assert!(battle.move_to(body, (i as i32, 8)), "a body would not park");
    }
    assert!(battle.move_to(player, centre), "the player would not stand");
    for (&body, &cell) in pack.iter().zip(cells) {
        assert!(battle.move_to(body, cell), "{cell:?} would not take a body");
    }
    centre
}

/// **Staying put is the default.** A hostile that can already swing at the
/// player from where it stands spends its turn swinging, not shuffling to
/// another cell that swings exactly as well — which is what it did while
/// every cell beside the player tied and the softmax drew among all of them.
///
/// A packmate stands beside it on purpose: crowding is the term that would
/// otherwise talk it into a sidestep, and crowding alone is not a reason to
/// move. Several turns at the shipped temperature, because one draw landing
/// back on its own cell would pass a single turn.
#[test]
fn a_hostile_already_in_reach_holds_its_cell() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 10_000);
    let wild = pack[0];
    open_ground(&mut game, &pack, &[(3, 4), (3, 3)]);

    for turn in 0..6 {
        assert!(wait_for_turn(&mut game, wild), "the fight ended early");
        assert!(game.tactical_ai_turn(), "the hostile's turn was not run");
        assert_eq!(
            cell_of(&game, wild),
            Some((3, 4)),
            "turn {turn}: a hostile already in reach walked anyway"
        );
    }
}

/// Holding is not freezing: a hostile out of reach still closes.
#[test]
fn a_hostile_out_of_reach_still_closes() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 10_000);
    let wild = pack[0];
    let player = open_ground(&mut game, &pack, &[(0, 4)]);
    assert!(wait_for_turn(&mut game, wild), "the fight ended early");
    let before = crate::tactical::reach::distance(player, (0, 4));

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    let after = crate::tactical::reach::distance(
        player,
        cell_of(&game, wild).expect("the hostile left the board"),
    );
    assert!(
        after < before,
        "the hostile stood {before} cells off and stayed at {after}"
    );
}

/// In range is not the same as able to act: a reaching hostile in band but
/// behind cover has a reason to move, and moves to a cell it can see from.
#[test]
fn a_hostile_in_range_but_blind_steps_into_sight() {
    let mut game = game();
    let wild = fight_against(&mut game, "drone");
    let player = open_ground(&mut game, &[wild], &[(4, 2)]);
    block_cell(&mut game, (4, 3));
    assert_eq!(
        game.swing_range(wild),
        2,
        "the fixture stands the drone at exactly its reach"
    );
    assert!(wait_for_turn(&mut game, wild), "the fight ended early");

    assert!(game.tactical_ai_turn(), "the hostile's turn was not run");

    let stood = cell_of(&game, wild).expect("the hostile left the board");
    let battle = game.world.resource::<TacticalBattle>();
    assert_ne!(
        stood,
        (4, 2),
        "a hostile that could not see its target held"
    );
    assert!(
        crate::tactical::reach::line_of_sight(&battle.board, stood, player),
        "the hostile moved to {stood:?}, which cannot see the player either"
    );
}

/// The outline this feeds `render/tactical.rs` cannot disagree with the
/// refusal `tactical_use_routine` gives at the keyboard: a cell out of
/// range and a cell out of sight are both excluded, and a cell the action
/// accepts is included.
///
/// `bus_fault` (`AllEnemies`) derives a six-cell range and a two-cell
/// radius — wide enough that a hand-written board can hold an out-of-range
/// corner, a cell behind cover, and a cell in plain view all inside one
/// fixture.
#[test]
fn placeable_cells_match_what_the_routine_accepts_and_refuses() {
    use crate::tactical::map::Board;
    use crate::tests::support::HOSTILE_SWEEP;

    let mut game = game();
    let wild = tactical_fight(&mut game, 1, 200)[0];
    let player = game.player_entity();
    only_routine(&mut game, player, HOSTILE_SWEEP);

    {
        let mut battle = game.world.resource_mut::<TacticalBattle>();
        battle.board = Board::from_rows(&[
            ".........",
            ".........",
            ".........",
            ".#.......",
            ".........",
            ".........",
            ".........",
            ".........",
            ".........",
        ]);
        assert!(battle.move_to(player, (1, 1)), "the player would not stand");
        assert!(battle.move_to(wild, (1, 6)), "the hostile would not stand");
    }
    assert!(wait_for_turn(&mut game, player), "the fight ended early");

    let out_of_range = (8, 8); // Chebyshev 7 from (1,1); the range caps at 6.
    let behind_cover = (1, 6); // straight down column 1, past the `#` at (1,3).
    let in_the_open = (1, 2); // one cell down, nothing between.

    let placeable = game.tactical_placeable_cells(0);
    assert!(
        !placeable.is_empty(),
        "an empty set would prove nothing about the ones excluded below"
    );

    assert!(
        !placeable.contains(&out_of_range),
        "an out-of-range cell was offered as a legal centre"
    );
    assert!(
        !game.tactical_use_routine(0, out_of_range),
        "the action accepted a cell the outline would have refused"
    );

    assert!(
        !placeable.contains(&behind_cover),
        "a cell behind cover was offered as a legal centre"
    );
    assert!(
        !game.tactical_use_routine(0, behind_cover),
        "the action accepted a cell the outline would have refused"
    );

    assert!(
        placeable.contains(&in_the_open),
        "a cell in range and in plain view was left off the outline"
    );
    assert!(
        game.tactical_use_routine(0, in_the_open),
        "the outline offered a cell the action then refused"
    );
}

/// A `Single` shape resolves onto whoever is standing at the aimed cell
/// rather than a centre the player chooses freely, and a `Line`/`Cone` is
/// aimed as a direction — neither has a "legal centre" to outline, so both
/// are left alone rather than forced into a cell set that would read as
/// every cell in range.
#[test]
fn placeable_cells_are_empty_for_a_single_target_routine() {
    let mut game = game();
    tactical_fight(&mut game, 1, 40);
    let player = game.player_entity();
    only_routine(&mut game, player, "priority_boost");
    assert!(wait_for_turn(&mut game, player), "the fight ended early");

    assert!(
        game.tactical_placeable_cells(0).is_empty(),
        "a Single shape has no centre to outline"
    );
}

// --- Partial cover -----------------------------------------------------
//
// `reach::cover_between` needs no `Game` at all: it is a pure function of a
// board and two cells, so these are bare `Board`s and direct calls.

/// Attacker at (3,1), defender at (3,5), and whatever `rows` says between
/// them.
fn cover_board(rows: &[&str]) -> crate::tactical::map::Board {
    crate::tactical::map::Board::from_rows(rows)
}

#[test]
fn a_boulder_in_the_arc_gives_cover() {
    let board = cover_board(&[
        ".......", ".......", ".......", ".......", "..#....", ".......", ".......",
    ]);
    assert!(
        crate::tactical::reach::cover_between(&board, (3, 1), (3, 5)),
        "a boulder on the attacker's side of the defender is cover"
    );
}

#[test]
fn a_boulder_behind_the_defender_is_not_cover() {
    let board = cover_board(&[
        ".......", ".......", ".......", ".......", ".......", ".......", "..#....",
    ]);
    assert!(
        !crate::tactical::reach::cover_between(&board, (3, 1), (3, 5)),
        "a boulder on the far side shields nothing"
    );
}

/// The dot product is **strictly** positive, and this is the test that says
/// so: a boulder exactly abeam of the defender scores zero, and a `>= 0`
/// comparison would hand out cover for standing next to a rock.
#[test]
fn a_boulder_exactly_abeam_is_not_cover() {
    let board = cover_board(&[
        ".......", ".......", ".......", ".......", ".......", "..#....", ".......",
    ]);
    assert!(
        !crate::tactical::reach::cover_between(&board, (3, 1), (3, 5)),
        "ninety degrees off the bearing is beside you, not between you and the shot"
    );
}

#[test]
fn cover_does_nothing_at_melee_range() {
    let board = cover_board(&[
        ".......", ".......", ".......", ".......", "..#....", ".......", ".......",
    ]);
    assert_eq!(
        crate::tactical::reach::distance((3, 4), (3, 5)),
        crate::tuning::TACTICAL_MELEE_RANGE,
        "the fixture is meant to sit exactly on the melee band"
    );
    assert!(
        !crate::tactical::reach::cover_between(&board, (3, 4), (3, 5)),
        "a boulder is no help against someone standing on top of you"
    );
}

#[test]
fn a_defender_out_of_sight_has_no_cover() {
    let board = cover_board(&[
        ".......", ".......", ".......", "...#...", "..#....", ".......", ".......",
    ]);
    assert!(
        !crate::tactical::reach::cover_between(&board, (3, 1), (3, 5)),
        "a shot that cannot be taken needs no modifier"
    );
}

/// **Cover that no generated board produces is a feature that ships green
/// and dead**, which this repo has shipped before. Measured over full
/// enumeration on 2026-09-17, as the share of ordered walkable pairs beyond
/// melee range *with line of sight* whose defender has cover:
///
/// | biome | `Cover` weight | share |
/// |---|---:|---:|
/// | OpenGrid | 4 | 12.7% |
/// | Deadlock | 7 | 22.3% |
/// | NullSector | 8 | 22.8% |
/// | Backplane | 20 | 43.2% |
///
/// Sighted pairs is the right denominator: a pair with no line of sight has
/// no attack to modify. Note Backplane has the *lowest* share of all pairs
/// and the highest of sighted ones — dense cover blocks most long sightlines
/// outright, so the shots that remain are mostly covered ones.
///
/// The sweep here samples attackers rather than exhausting them, to stay
/// cheap; the floor is well below every measured figure.
#[test]
fn cover_is_reachable_on_every_biome_a_fight_opens_on() {
    use crate::tactical::map::{BattleSpec, generate};
    use crate::tactical::reach::{cover_between, distance, line_of_sight};
    use crate::tuning::TACTICAL_MELEE_RANGE;
    use crate::world::Biome;

    for biome in [
        Biome::OpenGrid,
        Biome::Deadlock,
        Biome::NullSector,
        Biome::Backplane,
    ] {
        let (mut sighted, mut covered) = (0u32, 0u32);
        for seed in 1..=3u32 {
            for bodies in [2u32, 5, 8] {
                let board = generate(BattleSpec {
                    world_seed: seed,
                    site: (seed as i32, 0),
                    tick: u64::from(seed) * 17,
                    zone: 1,
                    biome,
                    bodies,
                });
                let walkable: Vec<(i32, i32)> = board
                    .cells()
                    .filter(|(_, kind)| kind.walkable())
                    .map(|(cell, _)| cell)
                    .collect();
                let step = (walkable.len() / 60).max(1);
                for &attacker in walkable.iter().step_by(step) {
                    for &defender in &walkable {
                        if distance(attacker, defender) <= TACTICAL_MELEE_RANGE
                            || !line_of_sight(&board, attacker, defender)
                        {
                            continue;
                        }
                        sighted += 1;
                        if cover_between(&board, attacker, defender) {
                            covered += 1;
                        }
                    }
                }
            }
        }
        assert!(sighted > 0, "{biome:?} produced no shots at all");
        let share = f64::from(covered) / f64::from(sighted);
        assert!(
            share >= 0.10,
            "{biome:?}: only {:.1}% of takeable shots are at a defender in cover, \
             so the feature is close to unreachable there",
            share * 100.0
        );
    }
}

/// Which attacks cover applies to. The fixture is `combat_status`'s, since
/// these are questions about a swing rather than about a board.
mod cover_by_shape {
    use super::*;
    use crate::abilities::AbilityShape;
    use crate::battle;

    fn shaped_swing(game: &Game, body: Entity, shape: AbilityShape) -> battle::Swing {
        battle::Swing {
            cover_ignored: shape.ignores_cover(),
            ..battle::Swing::plain(game.natural_range_of(body))
        }
    }

    fn covered(game: &Game, attacker: Entity, defender: Entity, shape: AbilityShape) -> bool {
        let swing = shaped_swing(game, defender, shape);
        game.defender_profile_against(attacker, defender, swing)
            .evasion
            > game.combatant_profile(defender, swing).evasion
    }

    /// A blast flushes a body out from behind its boulder.
    #[test]
    fn a_blast_ignores_cover() {
        let (game, player, wild) = crate::tests::combat_status::cover_fight(
            904,
            &crate::tests::combat_status::COVERED_BOARD,
            (4, 1),
            (4, 4),
        );
        assert!(!covered(
            &game,
            player,
            wild,
            AbilityShape::Radius { radius: 1 }
        ));
    }

    /// Everything that is a shot rather than a blast is refused by cover.
    #[test]
    fn every_other_shape_is_penalised_by_cover() {
        let (game, player, wild) = crate::tests::combat_status::cover_fight(
            904,
            &crate::tests::combat_status::COVERED_BOARD,
            (4, 1),
            (4, 4),
        );
        for shape in [
            AbilityShape::Single,
            AbilityShape::Line { length: 4 },
            AbilityShape::Cone {
                length: 4,
                degrees: 90,
            },
        ] {
            assert!(
                covered(&game, player, wild, shape),
                "{shape:?} should be refused by cover"
            );
        }
    }

    /// **The polarity regression.** `false` is "cover applies", so a
    /// `Swing::default()` written later cannot switch the feature off.
    #[test]
    fn a_default_swing_still_honours_cover() {
        assert!(!battle::Swing::default().cover_ignored);
    }
}

/// What the screen is told about cover.
mod cover_telegraph {
    use super::*;
    use crate::tests::combat_status::{COVERED_BOARD, cover_fight};

    /// **The test the spec asks for by name, and the one that must not be
    /// weakened.** The mark and the roll are two calls into one rule; over
    /// every ordered pair on a board, they answer the same thing.
    #[test]
    fn the_telegraph_agrees_with_the_roll() {
        let (game, player, wild) = cover_fight(905, &COVERED_BOARD, (4, 1), (4, 4));
        let mut agreed = 0;
        for (attacker, defender) in [(player, wild), (wild, player)] {
            let swing = crate::battle::Swing::plain(game.natural_range_of(defender));
            let raised = game
                .defender_profile_against(attacker, defender, swing)
                .evasion
                > game.combatant_profile(defender, swing).evasion;
            assert_eq!(
                game.body_in_cover(attacker, defender),
                raised,
                "the mark and the roll disagree"
            );
            agreed += u32::from(raised);
        }
        assert_eq!(
            agreed, 1,
            "the fixture should shelter exactly one of the two, or it proves nothing"
        );
    }

    /// The standing mark is a hostile's turn only — on the player's own,
    /// nothing has been aimed yet.
    #[test]
    fn the_standing_mark_is_a_hostiles_turn_only() {
        let (mut game, player, wild) = cover_fight(906, &COVERED_BOARD, (4, 4), (4, 1));
        assert!(wait_for_turn(&mut game, player), "the fight ended early");
        let view = game.tactical_view().expect("a fight is open");
        assert!(
            view.bodies.iter().all(|b| !b.in_cover),
            "the player's own turn lit a standing mark"
        );
        assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
        let view = game.tactical_view().expect("a fight is open");
        let marked = view
            .bodies
            .iter()
            .find(|b| b.entity == player)
            .expect("the player is on the board");
        assert!(
            marked.in_cover,
            "the boulder shelters the player from the hostile whose turn it is"
        );
        assert_eq!(
            marked.in_cover,
            game.body_in_cover(wild, player),
            "the mark disagrees with the door it is a call into"
        );
    }

    /// A reachable cell that would shelter the acting body is washed, and
    /// one that would not is left alone.
    #[test]
    fn a_covered_destination_is_marked() {
        let (mut game, _, wild) = cover_fight(907, &COVERED_BOARD, (4, 1), (4, 6));
        assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
        let view = game.tactical_view().expect("a fight is open");
        assert!(
            !view.covered.is_empty(),
            "nothing on this board sheltered the hostile from the player"
        );
        for cell in &view.covered {
            assert!(
                view.reachable.contains(cell),
                "a covered cell that cannot be walked to was marked"
            );
        }
    }

    /// A finished fight is a result screen, not a resumed one.
    #[test]
    fn a_finished_fight_marks_nothing() {
        let (mut game, _, wild) = cover_fight(908, &COVERED_BOARD, (4, 1), (4, 6));
        assert!(wait_for_turn(&mut game, wild), "the hostile never acted");
        let frozen = game.tactical_view().expect("a fight is open").frozen();
        assert!(frozen.covered.is_empty());
        assert!(frozen.bodies.iter().all(|b| !b.in_cover));
    }
}

/// Task 3: the `Squad` arm inside the existing `Game::effective_atk` door —
/// see `docs/superpowers/plans/2026-09-17-tactical-squads.md`. A bare
/// fixture rather than a real `Game::spawn_squad` (task 4's), since this
/// arm only ever reads the entity's own `Squad`-presence and `Stats`.
mod squad_effective_atk {
    use super::*;

    fn squad_body(game: &mut Game, hp: i32, max_hp: i32, atk: i32) -> Entity {
        game.world
            .spawn((
                Creature {
                    species: generic_species().id,
                },
                Hostile,
                Stats {
                    hp,
                    max_hp,
                    atk,
                    mitigation: 0,
                },
                Squad {
                    members: Vec::new(),
                    formation: 0,
                },
            ))
            .id()
    }

    #[test]
    fn a_squads_attack_falls_with_its_own_integrity() {
        let mut game = game();
        let full = squad_body(&mut game, 100, 100, 40);
        assert_eq!(
            game.effective_atk(full),
            40,
            "a squad at full Integrity should hit for its raw atk"
        );

        let half = squad_body(&mut game, 50, 100, 40);
        assert_eq!(
            game.effective_atk(half),
            20,
            "a squad at half Integrity should hit for half its raw atk"
        );

        let empty = squad_body(&mut game, 0, 100, 40);
        assert_eq!(
            game.effective_atk(empty),
            0,
            "a squad at zero Integrity should hit for nothing"
        );
    }

    /// The door's existing behaviour for anything without a `Squad` — a
    /// wild body's `effective_atk` is untouched by this arm.
    #[test]
    fn a_lone_body_is_untouched_by_the_squad_scale() {
        let mut game = game();
        let lone = body(&mut game, &generic_species().id);
        assert_eq!(game.effective_atk(lone), 3);
    }
}

/// Task 4: squads actually form and fight — see
/// `docs/superpowers/plans/2026-09-17-tactical-squads.md`.
mod squads {
    use super::*;

    /// Nine of a species (`tactical_pack`'s own baseline: one shared
    /// species, `atk: 1`, `mitigation: 0`) fold into one squad body and
    /// four singles, with the stat block the spec's table describes.
    #[test]
    fn nine_of_a_species_fold_into_a_squad_with_the_summed_stat_block() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 9, 10);

        let board_bodies: Vec<Entity> = {
            let battle = game.world.resource::<TacticalBattle>();
            battle.bodies().map(|(e, _)| e).collect()
        };
        let squads: Vec<Entity> = board_bodies
            .iter()
            .copied()
            .filter(|&e| game.world.get::<Squad>(e).is_some())
            .collect();
        assert_eq!(squads.len(), 1, "9 of a kind should seat exactly one squad");
        let squad = squads[0];

        let stats = *game.world.get::<Stats>(squad).unwrap();
        assert_eq!(stats.max_hp, 50, "summed max_hp over 5 members at 10 each");
        assert_eq!(stats.hp, 50);
        let formation = &crate::tuning::FORMATIONS[0];
        let expected_atk = ((5 * 1) as f32 * formation.swing_share).round() as i32;
        assert_eq!(stats.atk, expected_atk);
        assert_eq!(stats.mitigation, 0, "the members' highest, all zero here");

        {
            let battle = game.world.resource::<TacticalBattle>();
            assert_eq!(battle.footprint_of(squad), formation.footprint);
            assert_eq!(
                battle.cells_of(squad).len(),
                (formation.footprint as usize).pow(2)
            );
        }
        assert_eq!(game.actions_per_turn(squad), formation.actions);

        // The board holds the player, one squad and four leftover singles —
        // nine wild bodies never became six board occupants by accident.
        assert_eq!(board_bodies.len(), 1 + 1 + 4);
        let squad_members = game.world.get::<Squad>(squad).unwrap().members.clone();
        assert_eq!(squad_members.len(), 5);
        for &member in &squad_members {
            assert!(
                pack.contains(&member),
                "a squad's members must come from the pack it formed out of"
            );
            assert!(
                !board_bodies.contains(&member),
                "a squad's members must not also be placed on the board"
            );
        }
    }

    /// Four of a species is under the formation's threshold, so nothing
    /// folds — the ordinary one-cell, one-action case.
    #[test]
    fn four_of_a_species_never_folds_on_a_real_board() {
        let mut game = game();
        tactical_fight(&mut game, 4, 10);
        let battle = game.world.resource::<TacticalBattle>();
        assert!(
            battle
                .bodies()
                .all(|(e, _)| game.world.get::<Squad>(e).is_none())
        );
    }

    /// A squad gets its formation's two actions before the turn is handed
    /// on; an ordinary body still gets exactly one. `tactical_defend`
    /// rather than a swing, since it needs no range or target — only
    /// whether an action was spent.
    #[test]
    fn a_squad_spends_two_actions_before_the_turn_moves_on() {
        let mut game = game();
        let pack = tactical_pack(&mut game, 9, 40);
        game.open_tactical_battle(pack);
        let squad = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| game.world.get::<Squad>(e).is_some())
                .expect("9 of a kind must seat a squad")
        };
        assert!(
            wait_for_turn(&mut game, squad),
            "the squad never got a turn"
        );
        assert_eq!(game.world.resource::<TacticalBattle>().actions_left(), 2);

        assert!(game.tactical_defend(), "the first brace was refused");
        assert_eq!(
            game.tactical_actor(),
            Some(squad),
            "one of two actions spent must not hand the turn on"
        );

        assert!(game.tactical_defend(), "the second brace was refused");
        assert_ne!(
            game.tactical_actor(),
            Some(squad),
            "both actions spent must hand the turn on"
        );
    }

    /// A single body still gets exactly one action — the pre-squad
    /// behaviour must survive squads existing at all.
    #[test]
    fn a_single_body_still_gets_one_action() {
        let mut game = game();
        let pack = tactical_fight(&mut game, 1, 40);
        assert!(
            wait_for_turn(&mut game, pack[0]),
            "the lone body never got a turn"
        );
        assert_eq!(game.world.resource::<TacticalBattle>().actions_left(), 1);
    }
}
