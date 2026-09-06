//! Phase 7b — a Hostile town's patrol.
//!
//! The tether itself and the chase it shares with a nest guardian are
//! `tests/zone.rs`' business; what is here is the half that is a town's:
//! who fields a patrol, where its members stand, what makes them notice the
//! party, and what standing them down costs.

use super::support::*;
use crate::components::{Hostile, Position, TownPatrol};
use crate::settlements::SettlementKey;
use crate::tuning::*;
use crate::world::{Biome, Tile, WorldMap};
use crate::*;

use bevy_ecs::prelude::Entity;

/// Open ground for `radius` around `(x, y)`, so a spawn's habitat and
/// standing-room questions both have an answer whatever the seed rolled.
fn carve_open(game: &mut Game, (x, y): (i32, i32), radius: i32) {
    let mut map = game.world.resource_mut::<WorldMap>();
    for dx in -radius..=radius {
        for dy in -radius..=radius {
            map.set_override(
                x + dx,
                y + dy,
                Tile {
                    biome: Biome::OpenGrid,
                    walkable: true,
                    rock_shade: None,
                },
            );
        }
    }
}

/// Puts a town `dx` east of the party at `standing`, on carved-open ground,
/// and hands back its key and its entity.
fn town_near_player(
    game: &mut Game,
    dx: i32,
    standing: i32,
) -> (SettlementKey, Entity, (i32, i32)) {
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let tile = (ppos.x + dx, ppos.y);
    carve_open(game, tile, SETTLEMENT_PATROL_RING_MAX + 2);
    let key = SettlementKey { rx: 1, ry: 0 };
    let town = place_settlement(game, key, tile.0, tile.1);
    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .standing = standing;
    (key, town, tile)
}

/// Every patrol member `town` has standing, and where each of them is.
fn patrol_of(game: &mut Game, town: Entity) -> Vec<(Entity, Position)> {
    let mut query = game.world.query::<(Entity, &TownPatrol, &Position)>();
    query
        .iter(&game.world)
        .filter(|(_, patrol, _)| patrol.town == town)
        .map(|(entity, _, pos)| (entity, *pos))
        .collect()
}

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

// ---------------------------------------------------------------------------
// Who fields one
// ---------------------------------------------------------------------------

#[test]
fn a_hostile_town_fields_a_patrol() {
    let mut game = game(9101);
    let (_, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);

    game.field_patrol();

    assert_eq!(
        patrol_of(&mut game, town).len(),
        1,
        "a Hostile town within range fielded nobody"
    );
}

/// Four tests rather than one over a table, for Phase 7a §8's reason: a
/// single test that walks every band passes as soon as *some* band refuses,
/// and the one it is really asserting — that only the bottom band fields
/// anyone — is exactly what a table hides.
#[test]
fn a_town_that_is_not_hostile_fields_nobody() {
    for standing in [
        0,
        SETTLEMENT_ALLIED_STANDING,
        SETTLEMENT_HOSTILE_STANDING + 1,
    ] {
        let mut game = game(9102);
        let (_, town, _) = town_near_player(&mut game, 4, standing);

        game.field_patrol();

        assert!(
            patrol_of(&mut game, town).is_empty(),
            "a town at standing {standing} fielded a patrol"
        );
    }
}

#[test]
fn a_town_out_of_range_fields_nobody() {
    let mut game = game(9103);
    let (_, town, _) = town_near_player(
        &mut game,
        SETTLEMENT_PATROL_RANGE + 1,
        SETTLEMENT_HOSTILE_STANDING,
    );

    game.field_patrol();

    assert!(
        patrol_of(&mut game, town).is_empty(),
        "a Hostile town past SETTLEMENT_PATROL_RANGE still fielded a patrol"
    );
}

#[test]
fn a_town_fields_no_more_than_the_patrol_size() {
    let mut game = game(9104);
    let (_, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);

    for _ in 0..SETTLEMENT_PATROL_SIZE * 4 {
        game.field_patrol();
    }

    assert_eq!(
        patrol_of(&mut game, town).len(),
        SETTLEMENT_PATROL_SIZE as usize,
        "a town fielded past SETTLEMENT_PATROL_SIZE"
    );
}

/// The ring's minimum band is the whole of this rule — a settlement tile
/// admits nobody, so a member placed on it is a program standing where
/// `move_player` would have refused to let the player stand.
#[test]
fn a_patrol_member_never_stands_on_its_towns_own_tile() {
    for seed in 0..40u32 {
        let mut game = game(seed);
        let (_, town, tile) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);

        for _ in 0..SETTLEMENT_PATROL_SIZE {
            game.field_patrol();
        }

        for (_, pos) in patrol_of(&mut game, town) {
            let band = (pos.x - tile.0).abs().max((pos.y - tile.1).abs());
            assert!(
                band >= SETTLEMENT_PATROL_RING_MIN,
                "seed {seed}: a patrol member stood at band {band}, inside \
                 SETTLEMENT_PATROL_RING_MIN"
            );
        }
    }
}

/// A patrol member is a nest guardian minus the nest, and the components
/// are where that has to be true: everything downstream — `pursuit_tick`,
/// `wander_ai_system`, every combat path — reads them and not the tether.
#[test]
fn a_patrol_member_is_ordinary_hostile_wildlife() {
    let mut game = game(9105);
    let (_, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);

    game.field_patrol();
    let member = patrol_of(&mut game, town)[0].0;

    assert!(
        game.world.get::<Hostile>(member).is_some(),
        "a patrol member must be Hostile"
    );
    assert!(
        game.world
            .get::<crate::components::WanderAi>(member)
            .is_some(),
        "a patrol member must wander until it notices the player"
    );
    assert!(
        game.world.get::<crate::components::Boss>(member).is_none(),
        "a patrol is an ordinary encounter and must never field a boss"
    );
}

// ---------------------------------------------------------------------------
// Noticing the party, and standing down
// ---------------------------------------------------------------------------

/// Fields one member and stands it `away` tiles east of the party.
fn member_at_range(game: &mut Game, town: Entity, away: i32) -> Entity {
    game.field_patrol();
    let member = patrol_of(game, town)[0].0;
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let mut pos = game.world.get_mut::<Position>(member).unwrap();
    pos.x = ppos.x + away;
    pos.y = ppos.y;
    member
}

fn is_pursuing(game: &Game, member: Entity) -> bool {
    game.world
        .get::<crate::components::Pursuing>(member)
        .is_some()
}

#[test]
fn a_patrol_member_notices_the_party_at_its_aggro_radius() {
    let mut game = game(9106);
    let (_, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);
    let member = member_at_range(&mut game, town, SETTLEMENT_PATROL_AGGRO_RADIUS);

    game.patrol_aggro_tick();

    assert!(
        is_pursuing(&game, member),
        "a patrol member inside SETTLEMENT_PATROL_AGGRO_RADIUS must give chase — proximity is \
         the whole provocation, and an attack is a nest's rule"
    );
}

#[test]
fn a_patrol_member_out_of_its_aggro_radius_stays_put() {
    let mut game = game(9107);
    let (_, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);
    let member = member_at_range(&mut game, town, SETTLEMENT_PATROL_AGGRO_RADIUS + 1);

    game.patrol_aggro_tick();

    assert!(
        !is_pursuing(&game, member),
        "a patrol member one tile past its aggro radius noticed the party anyway"
    );
}

/// The radius is inside `EXAMINE_RANGE_TILES` on purpose: a threat only ever
/// discovered by already being in a fight is not one the player can play
/// around. Asserted against the constant rather than described in a comment,
/// so raising either one fails the build.
#[test]
fn the_party_can_see_a_patrol_before_it_notices_them() {
    let notices = SETTLEMENT_PATROL_AGGRO_RADIUS;
    let sees = EXAMINE_RANGE_TILES;
    assert!(
        notices < sees,
        "a patrol notices the party at {notices} and the party can examine one at {sees} — a \
         threat only ever discovered by already being in a fight is not one to play around"
    );
}

/// The way back out of `Hostile`, made visible. This is why the band is
/// re-read every tick rather than cached at spawn.
#[test]
fn repairing_standing_stands_the_patrol_down() {
    let mut game = game(9108);
    let (key, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);
    let member = member_at_range(&mut game, town, 1);
    game.patrol_aggro_tick();
    assert!(is_pursuing(&game, member), "fixture assumes a live chase");

    game.world
        .resource_mut::<crate::resources::Standings>()
        .0
        .entry(key)
        .or_default()
        .standing = 0;
    game.patrol_aggro_tick();

    assert!(
        game.world.get::<TownPatrol>(member).is_none(),
        "a town that is no longer Hostile must give up its tether"
    );
    assert!(
        !is_pursuing(&game, member),
        "a stood-down patrol member must drop Pursuing with its tether — an untethered \
         Pursuing has no leash and nothing can ever clear it"
    );
    assert!(
        game.world.get::<Hostile>(member).is_some(),
        "a stood-down member reverts to ordinary untethered wildlife, not to nothing"
    );
}

// ---------------------------------------------------------------------------
// What killing one costs
// ---------------------------------------------------------------------------

#[test]
fn killing_a_patrol_member_costs_standing_with_its_own_town() {
    let mut game = game(9109);
    let (key, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);
    game.field_patrol();
    let member = patrol_of(&mut game, town)[0].0;
    let before = game.standing(key);

    let player = game.player_entity();
    game.start_battle(vec![member]);
    game.finish_member(0, 0, player);

    assert_eq!(
        game.standing(key) - before,
        SETTLEMENT_PATROL_KILL_STANDING,
        "killing a town's guard is news to that town"
    );
}

/// The neighbours have no view on whose guards died — which is why this
/// goes through `adjust_standing` by key rather than through
/// `credit_nearby_settlements`, the radius mover every *other* consequence
/// of a fight uses.
#[test]
fn killing_a_patrol_member_moves_no_other_towns_standing() {
    let mut game = game(9110);
    let (_, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);
    let neighbour = SettlementKey { rx: 2, ry: 0 };
    let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
    place_settlement(&mut game, neighbour, ppos.x + 6, ppos.y);
    let before = game.standing(neighbour);

    game.field_patrol();
    let member = patrol_of(&mut game, town)[0].0;
    let player = game.player_entity();
    game.start_battle(vec![member]);
    game.finish_member(0, 0, player);

    assert_eq!(
        game.standing(neighbour),
        before,
        "a town two tiles away must have no opinion on a fight that was not its own"
    );
}

/// Taking a program into the roster is not killing it, so it costs nothing
/// — and the mechanism is `combat_rewards`' strip tuple, which `TownPatrol`
/// joins rather than being handled beside.
#[test]
fn taming_a_patrol_member_costs_nothing() {
    let mut game = game(9111);
    let (key, town, _) = town_near_player(&mut game, 4, SETTLEMENT_HOSTILE_STANDING);
    game.field_patrol();
    let member = patrol_of(&mut game, town)[0].0;
    let before = game.standing(key);

    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(member).unwrap();
        stats.hp = 1;
    }
    insert_battle(&mut game, player, vec![member]);
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::ICE_BREAKER), 50);
    game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;
    for _ in 0..50 {
        if game.world.get::<Tamed>(member).is_some() {
            break;
        }
        player_decompiles(&mut game);
    }
    assert!(
        game.world.get::<Tamed>(member).is_some(),
        "fixture assumes the capture lands"
    );

    assert_eq!(
        game.standing(key),
        before,
        "taking a program into the roster is not killing it"
    );
    assert!(
        game.world.get::<TownPatrol>(member).is_none(),
        "a tamed member must lose its tether with the rest of its wild disposition"
    );
}
