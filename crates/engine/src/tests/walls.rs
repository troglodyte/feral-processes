//! The Wall — `StructureDef::barrier` and `StructureDef::swept`.
//!
//! A barrier refuses the player's step in base space and screens sight on a
//! siege board for as long as it stands; an unswept structure is left out
//! of the GC Entropy Sweep's target pool. The Wall is the one shipped def
//! that sets either.

use super::support::*;
use crate::components::Durability;
use crate::game::siege::board;
use crate::items::ItemId;
use crate::resources::GameClock;
use crate::structures::StructureDef;
use crate::tactical::TacticalBattle;
use crate::tactical::map::BattleSpec;
use crate::*;

fn def(game: &Game, id: &str) -> StructureDef {
    game.structure_defs()
        .into_iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("{id}.ron should load as a structure"))
}

fn ready_base(game: &mut Game) {
    game.lay_starting_pocket();
    stand_in_base_at(game, 0, 0);
}

#[test]
fn the_wall_is_buildable_without_research_and_costs_core_fragments() {
    let game = Game::new(4100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wall = def(&game, "wall");

    assert_eq!(
        wall.build_cost,
        vec![(ItemId::from("core_fragment"), 5)],
        "a wall costs five Core Fragments and nothing else"
    );
    assert!(wall.barrier, "a wall is a barrier");
    assert!(!wall.swept, "a sweep passes over a wall");
    assert!(wall.raidable, "a besieger can still break a wall");
    assert!(
        game.buildable_structure_defs()
            .iter()
            .any(|d| d.id == "wall"),
        "no research gates the wall"
    );
}

#[test]
fn the_player_cannot_walk_into_a_wall_and_the_bump_is_free() {
    let mut game = Game::new(4101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    ready_base(&mut game);
    let wall = def(&game, "wall");
    game.spawn_structure(&wall, 1, 0, None);
    let before = game.world.resource::<GameClock>().tick;

    game.move_player(1, 0);

    assert_eq!(game.base_pos(), Some((0, 0)), "the wall must stop the step");
    assert_eq!(
        game.world.resource::<GameClock>().tick,
        before,
        "a refused bump spends no tick"
    );
}

/// The gate is the flag, not "any structure" — every other structure is
/// still walked over, as it always was.
#[test]
fn a_structure_that_is_not_a_barrier_is_still_walked_over() {
    let mut game = Game::new(4102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    ready_base(&mut game);
    let depot = def(&game, "depot");
    assert!(!depot.barrier);
    game.spawn_structure(&depot, 1, 0, None);

    game.move_player(1, 0);

    assert_eq!(game.base_pos(), Some((1, 0)));
}

#[test]
fn a_sweep_never_lands_on_a_wall() {
    for seed in 0..20 {
        let mut game =
            Game::new(4200 + seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        let wall_def = def(&game, "wall");
        let node_def = def(&game, "mining_node");
        let wall = game.spawn_structure(&wall_def, 1, 0, None);
        let node = game.spawn_structure(&node_def, 2, 1, None);
        let wall_hp = game.world.get::<Durability>(wall).unwrap().hp;
        let node_hp = game.world.get::<Durability>(node).unwrap().hp;

        game.dev_force_raid();

        assert_eq!(
            game.world.get::<Durability>(wall).map(|d| d.hp),
            Some(wall_hp),
            "seed {seed}: a sweep must pass over a wall"
        );
        assert!(
            game.world.get::<Durability>(node).map(|d| d.hp) != Some(node_hp),
            "seed {seed}: the sweep lands on the one structure it may"
        );
    }
}

#[test]
fn a_standing_wall_blocks_sight_on_the_siege_board() {
    let mut game = Game::new(4300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    ready_base(&mut game);
    set_zone(&mut game, 2);
    let wall_def = def(&game, "wall");
    let depot_def = def(&game, "depot");
    let wall = game.spawn_structure(&wall_def, 2, 0, None);
    let depot = game.spawn_structure(&depot_def, 2, 1, None);

    assert!(game.open_siege());

    let battle = game.world.resource::<TacticalBattle>();
    let wall_cell = battle.cell_of(wall).expect("the wall is seated");
    let depot_cell = battle.cell_of(depot).expect("the depot is seated");
    assert!(battle.board.blocks_sight(wall_cell.0, wall_cell.1));
    assert!(
        !battle.board.blocks_sight(depot_cell.0, depot_cell.1),
        "only a barrier screens sight"
    );
}

/// Sight is re-derived on load rather than saved: the board's cells are
/// what `SiegeSave` carries, and a wall's cell is still `Open` in them.
#[test]
fn a_wall_still_blocks_sight_after_a_save_load_round_trip() {
    let mut game = Game::new(4301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    ready_base(&mut game);
    set_zone(&mut game, 2);
    let wall_def = def(&game, "wall");
    game.spawn_structure(&wall_def, 2, 0, None);
    assert!(game.open_siege());

    let scratch = scratch_assets_dir("wall_sight_roundtrip");
    std::fs::create_dir_all(&*scratch).unwrap();
    let path = scratch.join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let wall = loaded
        .world
        .query::<(Entity, &Structure)>()
        .iter(&loaded.world)
        .find(|(_, s)| s.kind == "wall")
        .map(|(e, _)| e)
        .expect("the wall survives the load");
    let battle = loaded.world.resource::<TacticalBattle>();
    let cell = battle.cell_of(wall).expect("the wall is re-seated on load");
    assert!(battle.board.blocks_sight(cell.0, cell.1));
}

/// A wall broken on the board stops screening — through the same
/// `TacticalBattle::remove` every destroyed structure already leaves by.
#[test]
fn a_broken_wall_stops_blocking_sight() {
    let mut game = Game::new(4302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    ready_base(&mut game);
    let wall_def = def(&game, "wall");
    let wall = game.spawn_structure(&wall_def, 1, 0, None);
    game.world.get_mut::<Durability>(wall).unwrap().hp = 1;

    let siege_board = board::build(&mut game).unwrap();
    let structures = game.structure_footprints();
    let spec = BattleSpec {
        world_seed: 1,
        site: (0, 0),
        tick: 0,
        zone: 2,
        biome: Biome::OpenGrid,
        bodies: 2,
    };
    let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
    board::seat_structures(&mut battle, &siege_board, structures, |e| {
        game.is_barrier(e)
    });
    let wall_cell = battle.cell_of(wall).unwrap();
    let player = game.player_entity();
    battle.place(player, (wall_cell.0 - 1, wall_cell.1));
    // Something hostile left standing, or the swing that breaks the wall
    // ends the fight and takes the board with it.
    let hostile = game
        .world
        .spawn((
            Hostile,
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                mitigation: 0,
            },
        ))
        .id();
    let far = battle
        .board
        .cells()
        .filter(|&(c, k)| k.walkable() && c != wall_cell && (c.0 - wall_cell.0).abs() > 2)
        .map(|(c, _)| c)
        .find(|&c| battle.occupant(c).is_none())
        .expect("the pocket has room for a distant hostile");
    battle.place(hostile, far);
    battle.set_initiative(vec![player]);
    assert!(battle.board.blocks_sight(wall_cell.0, wall_cell.1));
    game.world.insert_resource(battle);

    assert!(game.tactical_attack(wall));

    assert!(
        game.world.get::<Structure>(wall).is_none(),
        "the wall broke"
    );
    let battle = game.world.resource::<TacticalBattle>();
    assert!(
        !battle.board.blocks_sight(wall_cell.0, wall_cell.1),
        "a broken wall must stop blocking sight"
    );
}
