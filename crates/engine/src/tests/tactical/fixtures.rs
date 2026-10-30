use super::*;

use bevy_ecs::prelude::Entity;

pub(crate) fn game() -> Game {
    Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A body of `species`, standing nowhere in particular. Only its species
/// and its speed matter here.
pub(crate) fn body(game: &mut Game, species: &str) -> Entity {
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

pub(crate) fn log_texts(game: &Game) -> Vec<String> {
    game.message_log(crate::MESSAGE_LOG_CAP)
        .into_iter()
        .map(|l| l.text)
        .collect()
}

/// A tactical fight opened around `count` hostiles standing next to the
/// player, each on `hp`.
pub(crate) fn tactical_fight(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
    let pack = tactical_pack(game, count, hp);
    game.open_tactical_battle(pack.clone());
    // Pinned: the fixture seats whatever the roll gives, and a test about
    // something else must not depend on the seed happening to roll `Rush`.
    game.set_pack_tactic(crate::tactical::pack::PackTactic::Rush);
    pack
}

/// `count` hostiles standing next to the player, with no fight opened around
/// them yet — so a test may open one on a bearing of its own.
pub(crate) fn tactical_pack(game: &mut Game, count: usize, hp: i32) -> Vec<Entity> {
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
pub(crate) fn wait_for_turn(game: &mut Game, who: Entity) -> bool {
    for _ in 0..64 {
        match game.tactical_actor() {
            None => return false,
            Some(actor) if actor == who => return true,
            Some(_) => game.tactical_end_turn(),
        }
    }
    panic!("the turn never came back round");
}

/// A free walkable cell on the board's western edge.
pub(crate) fn western_edge(game: &Game) -> (i32, i32) {
    let battle = game.world.resource::<TacticalBattle>();
    (0..battle.board.side)
        .map(|y| (0, y))
        .find(|&at| battle.board.walkable(at.0, at.1) && battle.occupant(at).is_none())
        .expect("a walkable cell on the western edge")
}

/// A free walkable cell next to `cell`, for a test that needs two bodies
/// standing beside each other rather than wherever deployment put them.
pub(crate) fn free_neighbour(game: &Game, cell: (i32, i32)) -> (i32, i32) {
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

pub(super) fn hp_of(game: &Game, body: Entity) -> i32 {
    game.world
        .get::<Stats>(body)
        .expect("a body with no stats")
        .hp
}

/// Installs `routine` as the acting body's only one, so its index is zero.
pub(crate) fn only_routine(game: &mut Game, body: Entity, routine: &str) {
    game.world
        .entity_mut(body)
        .insert(crate::components::Routines(vec![routine.to_string()]));
}

/// A fight with its one hostile marooned in the far corner, where no
/// allowance closes on anybody, so its turn is a walk and nothing else —
/// and, with the corner open on more than one side, more than one candidate
/// cell scores.
pub(crate) fn marooned() -> (Game, Entity) {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 40);
    let wild = pack[0];
    // The hostile acts first, so its turn does not close the round: closing
    // it ages cooldowns and runs a world tick, and where a hostile lands in
    // the rolled order is seed luck.
    let player = game.player_entity();
    game.world
        .resource_mut::<TacticalBattle>()
        .set_initiative(vec![wild, player]);
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
pub(crate) fn next_draw(game: &mut Game) -> u64 {
    use rand::RngExt;
    game.world
        .resource_mut::<crate::resources::GameRng>()
        .0
        .random::<u64>()
}

/// Fights a one-body tactical pack to its end, the player walking into reach
/// of it every turn.
pub(crate) fn win_a_lone_tactical_fight(game: &mut Game) {
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

pub(super) fn catalysts_held(game: &Game) -> u32 {
    game.world
        .get::<crate::components::Inventory>(game.player_entity())
        .expect("the player carries nothing")
        .count(&crate::items::ItemId::from(crate::items::ids::ICE_BREAKER))
}

/// Stands `player` and `other` on chosen cells inside an open fight.
///
/// `move_to` rather than `place` — both are already seated by the time a
/// fight is open, and `place` refuses a body that is on the board.
pub(super) fn place_bodies(
    game: &mut Game,
    player: Entity,
    at: (i32, i32),
    other: Entity,
    theirs: (i32, i32),
) {
    place_one(game, player, at);
    place_one(game, other, theirs);
}

pub(crate) fn place_one(game: &mut Game, body: Entity, cell: (i32, i32)) {
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
pub(super) fn block_cell(game: &mut Game, cell: (i32, i32)) {
    game.world.resource_mut::<TacticalBattle>().board.put(
        cell.0,
        cell.1,
        crate::tactical::map::BattleCell::Cover,
    );
}

/// Opens a fight and hands the turn to the player, with the board's own
/// furniture cleared off the cells a test is about to use.
pub(super) fn ranged_fight(game: &mut Game, count: usize) -> Vec<Entity> {
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

/// A nine-cell open board with the player in the middle and the pack stood
/// where the test says, the player on enough Integrity to outlast every turn
/// a test drives.
pub(crate) fn open_ground(game: &mut Game, pack: &[Entity], cells: &[(i32, i32)]) -> (i32, i32) {
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
