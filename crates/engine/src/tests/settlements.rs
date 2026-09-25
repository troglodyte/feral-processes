//! Bumping into a settlement: the door that never opens, and the cue it
//! leaves behind for a frontend to act on.

use super::support::*;
use crate::*;

fn game() -> Game {
    Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Materializes a settlement well clear of the player, fully syncs its
/// footprint immediately, then sets the player down one step west of its
/// nearest cell — so `move_player(1, 0)` still bumps it exactly as this
/// fixture always meant to test.
///
/// **Doing all of that here, once, rather than leaving the footprint to
/// grow from `place_settlement`'s bare centre on whatever tick the test's
/// own `move_player` happens to trigger.** `place_settlement` spawns only
/// the centre; a materializing footprint runs settlement displacement
/// (`game/settlement_footprint.rs`) over every cell it newly covers,
/// including the player's own tile if they were standing at distance 1 from
/// the centre the way the old fixture placed them — which shoved the
/// bumping player off their own tile the instant the town grew to cover it,
/// confounding this file's bump-ladder tests with an unrelated growth
/// event. Materializing at a safe distance and only then walking the player
/// up to it keeps the two apart.
fn settlement_east_of_player(game: &mut Game) -> (crate::settlements::SettlementKey, (i32, i32)) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let key = crate::settlements::SettlementKey { rx: 0, ry: 0 };
    let target = (pos.x + 5, pos.y);
    place_settlement(game, key, target.0, target.1);
    game.sync_settlement_footprint(key);
    let radius = game
        .settlement_radius(key)
        .expect("test premise: the fixture's own key is materialized");
    let bump_from = (target.0 - radius - 1, target.1);
    {
        let mut player_pos = game
            .world
            .get_mut::<Position>(game.player_entity())
            .unwrap();
        player_pos.x = bump_from.0;
        player_pos.y = bump_from.1;
    }
    (key, target)
}

#[test]
fn walking_into_a_settlement_leaves_the_players_position_unchanged() {
    let mut game = game();
    let player = game.player_entity();
    settlement_east_of_player(&mut game);
    // Read after the fixture, which itself walks the player up beside the
    // footprint before this test's own bump.
    let pos_before = *game.world.get::<Position>(player).unwrap();

    game.move_player(1, 0);

    let pos_after = *game.world.get::<Position>(player).unwrap();
    assert_eq!(
        pos_before, pos_after,
        "the tile does not admit you — it queues a visit and the player stays put, \
         exactly like the other three bump arms"
    );
}

#[test]
fn the_bump_queues_exactly_one_visit_naming_that_settlements_key() {
    let mut game = game();
    let (key, _) = settlement_east_of_player(&mut game);

    game.move_player(1, 0);

    assert_eq!(
        game.take_visit(),
        Some(crate::resources::Visit::Settlement(key)),
        "the bump must name the settlement it landed on"
    );
}

/// Written first and watched red against a non-draining read (a `fn
/// take_visit(&self) -> Option<SettlementKey> { self.world
/// .resource::<PendingVisit>().0 }`) — a plain getter passes every other
/// test in this file, since none of the others call it twice. Only this one
/// tells a getter from a drain.
#[test]
fn a_second_take_visit_answers_none() {
    let mut game = game();
    settlement_east_of_player(&mut game);
    game.move_player(1, 0);

    assert!(
        game.take_visit().is_some(),
        "the fixture's own bump must have queued a visit"
    );
    assert_eq!(
        game.take_visit(),
        None,
        "a screen reopening on the next keypress — the one spent walking away — \
         is the bug this drain exists to close"
    );
}

#[test]
fn the_pending_visit_does_not_survive_a_save_and_load() {
    let mut game = game();
    settlement_east_of_player(&mut game);
    game.move_player(1, 0);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_settlement_visit_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.take_visit(),
        None,
        "a cue about this instant must not reopen a screen the moment a save loads — \
         `resources::CurrentStack`'s reason"
    );
}

#[test]
fn walking_onto_ordinary_ground_beside_a_settlement_still_moves_you_and_queues_nothing() {
    let mut game = game();
    // The settlement stands east of the player; the step this test takes
    // goes north, so it never lands on the town's own tile.
    settlement_east_of_player(&mut game);
    let player = game.player_entity();
    let pos_before = *game.world.get::<Position>(player).unwrap();
    let (nx, ny) = (pos_before.x, pos_before.y - 1);

    let squatters: Vec<Entity> = {
        let mut q = game.world.query::<(Entity, &Position)>();
        q.iter(&game.world)
            .filter(|(e, p)| *e != player && p.x == nx && p.y == ny)
            .map(|(e, _)| e)
            .collect()
    };
    for e in squatters {
        game.world.despawn(e);
    }
    {
        let mut map = game.world.resource_mut::<WorldMap>();
        let mut tile = map.tile(nx, ny);
        tile.walkable = true;
        map.set_override(nx, ny, tile);
    }

    game.move_player(0, -1);

    let pos_after = *game.world.get::<Position>(player).unwrap();
    assert_eq!(
        (pos_after.x, pos_after.y),
        (nx, ny),
        "ordinary ground beside a settlement must still be walkable"
    );
    assert_eq!(
        game.take_visit(),
        None,
        "a step that never touched the settlement tile must queue no visit"
    );
}

#[test]
fn walking_into_a_settlement_marks_it_visited() {
    let mut game = game();
    let (key, _) = settlement_east_of_player(&mut game);

    game.move_player(1, 0);

    assert!(
        game.world.resource::<crate::resources::Settlements>().0[&key].visited,
        "the arm that queues the visit is the one that records having been there"
    );
}

#[test]
fn a_settlement_merely_materialized_nearby_is_not_visited() {
    let mut game = game();
    let (key, _) = settlement_east_of_player(&mut game);

    assert!(
        !game.world.resource::<crate::resources::Settlements>().0[&key].visited,
        "recording where a town stands is not the same as having walked to it — \
         the whole point of the compass' two tiers"
    );
}
