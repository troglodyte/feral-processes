//! Founding an outpost — `crate::outposts` holds the record and the pure
//! def/growth math; this is `&mut Game` work, `game/route.rs`'s split.

use crate::Game;
use crate::outposts::{Outpost, OutpostDb};
use crate::resources::Outposts;
use crate::tuning::{
    MAX_OUTPOSTS, OUTPOST_MAX_INTEGRITY, OUTPOST_MIN_ANCHOR_DISTANCE, OUTPOST_MIN_SPACING,
};
use crate::world::{Biome, WorldMap};

impl Game {
    /// Founds an outpost at `tile` — design spec §9's refusal ladder.
    /// `Game::use_item` is the one caller, and does not take a unit out of
    /// `Inventory` until this returns `Ok`.
    pub fn found_outpost(&mut self, tile: (i32, i32)) -> Result<(), String> {
        self.require_surface()?;
        if self.world.resource::<OutpostDb>().def().is_none() {
            return Err("No outpost design is known.".into());
        }
        let (x, y) = tile;
        let map_tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        if !map_tile.walkable {
            return Err("Nothing would hold an outpost there.".into());
        }
        if map_tile.biome == Biome::Platform {
            return Err("The base's own ground holds no outpost.".into());
        }
        let biome = map_tile.biome;
        if let Some((ax, ay)) = self.anchor_position()
            && (x - ax).abs().max((y - ay).abs()) < OUTPOST_MIN_ANCHOR_DISTANCE
        {
            return Err("That's still within reach of the base. Walk further out.".into());
        }
        // The Stack link, nest and settlement checks reuse the existing
        // occupancy queries `place_trap` already draws on, rather than
        // restating what "something stands here" means a fourth time.
        if self.find_nest_at(x, y).is_some()
            || self.find_surface_link_at(x, y).is_some()
            || self.find_settlement_at(x, y).is_some()
        {
            return Err("Something already stands there.".into());
        }
        if self
            .world
            .resource::<Outposts>()
            .0
            .keys()
            .any(|&(ox, oy)| (x - ox).abs().max((y - oy).abs()) < OUTPOST_MIN_SPACING)
        {
            return Err("Another outpost stands too close to build here.".into());
        }
        if self.world.resource::<Outposts>().0.len() >= MAX_OUTPOSTS {
            return Err(format!(
                "You already have {MAX_OUTPOSTS} outposts standing. That's the limit."
            ));
        }
        self.world
            .resource_mut::<Outposts>()
            .0
            .insert(tile, Outpost::new(biome, OUTPOST_MAX_INTEGRITY));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DifficultyMode;
    use crate::components::{Inventory, Position};
    use crate::tests::support::test_assets_dir;

    fn game(seed: u32) -> Game {
        Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
    }

    fn kit_id(game: &Game) -> crate::items::ItemId {
        game.world
            .resource::<OutpostDb>()
            .def()
            .expect("assets/outposts/outpost.ron ships a def")
            .kit
            .clone()
    }

    /// Deterministically finds a legal founding tile at least `min_distance`
    /// Chebyshev tiles from the anchor and at least `avoid_dist` from every
    /// tile in `avoid` — walking outward ring by ring rather than trusting a
    /// single hardcoded offset to be walkable, unoccupied ground for every
    /// seed a test picks. Panics if none turns up within a generous search
    /// radius, which would mean the constants really do make the ground
    /// unreachable (`a-gated-consequence-can-be-green-and-unreachable`).
    fn open_tile_at_least(
        game: &mut Game,
        min_distance: i32,
        avoid: &[(i32, i32)],
        avoid_dist: i32,
    ) -> (i32, i32) {
        let (ax, ay) = game.anchor_position().unwrap();
        for radius in min_distance..min_distance + 500 {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let (x, y) = (ax + dx, ay + dy);
                    let tile = game.world.resource_mut::<WorldMap>().tile(x, y);
                    if !tile.walkable || tile.biome == Biome::Platform {
                        continue;
                    }
                    if game.find_nest_at(x, y).is_some()
                        || game.find_surface_link_at(x, y).is_some()
                        || game.find_settlement_at(x, y).is_some()
                    {
                        continue;
                    }
                    if avoid
                        .iter()
                        .any(|&(ox, oy)| (x - ox).abs().max((y - oy).abs()) < avoid_dist)
                    {
                        continue;
                    }
                    return (x, y);
                }
            }
        }
        panic!("no open founding tile found within the search radius");
    }

    /// The shipped def loads — every test below relies on it, so a broken
    /// `assets/outposts/outpost.ron` fails loudly here rather than as a
    /// mysterious refusal in every other test.
    #[test]
    fn the_shipped_outpost_def_loads() {
        let game = game(1);
        assert!(game.world.resource::<OutpostDb>().def().is_some());
    }

    #[test]
    fn founding_far_from_the_anchor_on_open_ground_succeeds() {
        let mut game = game(2);
        // `OUTPOST_MIN_ANCHOR_DISTANCE` must be reachable at all — a
        // constant that refused every tile in a real, generated world would
        // be green and useless (`a-gated-consequence-can-be-green-and-
        // unreachable`).
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(tile).unwrap();
        assert!(game.world.resource::<Outposts>().0.contains_key(&tile));
    }

    #[test]
    fn founded_outpost_reads_the_biome_at_the_tile() {
        let mut game = game(2);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        let expected_biome = game
            .world
            .resource_mut::<WorldMap>()
            .tile(tile.0, tile.1)
            .biome;
        game.found_outpost(tile).unwrap();
        let record = &game.world.resource::<Outposts>().0[&tile];
        assert_eq!(record.biome, expected_biome);
        assert_eq!(record.growth, 0);
        assert_eq!(record.integrity, OUTPOST_MAX_INTEGRITY);
        assert!(record.stock.is_empty());
        assert_eq!(record.stale_ticks, 0);
        assert_eq!(record.cycle_progress, 0);
        assert!(record.announced.is_none());
    }

    #[test]
    fn refuses_underground() {
        let mut game = game(3);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        // `enter_stack` drops the party down wherever they are standing —
        // no real link entity is needed to exercise `require_surface`.
        let player = game.player_entity();
        let pos = *game.world.get::<Position>(player).unwrap();
        game.enter_stack(pos.x, pos.y);
        assert!(game.is_underground());
        assert!(game.found_outpost(tile).is_err());
        // Deleted-fix check: without `require_surface`, this same call
        // would succeed underground, which is exactly the bug this guards.
    }

    #[test]
    fn refuses_when_the_kit_is_still_in_the_pack_on_every_refusal() {
        let mut game = game(4);
        // The player starts standing at the anchor itself, well inside
        // `OUTPOST_MIN_ANCHOR_DISTANCE` — that refusal is enough on its own.
        let id = kit_id(&game);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(id.clone(), 1);
        // Too close to the anchor — a refusal that leaves the kit untouched.
        game.use_item(&id);
        assert_eq!(
            game.world.get::<Inventory>(player).unwrap().count(&id),
            1,
            "a refused founding must not spend the kit"
        );
    }

    #[test]
    fn using_the_kit_with_none_in_the_pack_refuses_without_founding() {
        let mut game = game(5);
        // Walk the player somewhere `found_outpost` would otherwise accept,
        // so the only thing left to refuse this is the empty pack —
        // founding at the anchor itself would refuse for a different
        // reason and leave this guard untested.
        let target = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        let player = game.player_entity();
        {
            let mut pos = game.world.get_mut::<Position>(player).unwrap();
            pos.x = target.0;
            pos.y = target.1;
        }
        let id = kit_id(&game);
        // The pack starts empty of the kit — nothing to spend.
        game.use_item(&id);
        assert!(game.world.resource::<Outposts>().0.is_empty());
    }

    #[test]
    fn successful_use_spends_one_kit_and_founds_the_outpost() {
        let mut game = game(6);
        let target = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        let id = kit_id(&game);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(id.clone(), 2);
        // The player starts standing on the anchor itself, which is too
        // close to found on — walk them out first.
        {
            let mut pos = game.world.get_mut::<Position>(player).unwrap();
            pos.x = target.0;
            pos.y = target.1;
        }
        game.use_item(&id);
        assert_eq!(game.world.get::<Inventory>(player).unwrap().count(&id), 1);
        assert!(game.world.resource::<Outposts>().0.contains_key(&target));
    }

    #[test]
    fn refuses_within_min_anchor_distance() {
        let mut game = game(7);
        // The anchor's own tile is guaranteed open ground (the party
        // started there) and is distance 0 from itself — well inside the
        // minimum, with no dependence on this seed's generated terrain
        // elsewhere. Deleted-fix check: without the distance guard this
        // same call succeeds, since nothing else about the tile refuses it.
        let tile = game.anchor_position().unwrap();
        assert!(game.found_outpost(tile).is_err());
    }

    #[test]
    fn refuses_on_platform_biome() {
        let mut game = game(8);
        let (ax, ay) = game.anchor_position().unwrap();
        let tile = (ax + OUTPOST_MIN_ANCHOR_DISTANCE, ay);
        game.world.resource_mut::<WorldMap>().set_override(
            tile.0,
            tile.1,
            crate::world::Tile {
                biome: Biome::Platform,
                walkable: true,
                rock_shade: None,
            },
        );
        assert_eq!(
            game.found_outpost(tile),
            Err("The base's own ground holds no outpost.".to_string())
        );
    }

    #[test]
    fn refuses_when_a_settlement_stands_on_the_tile() {
        let mut game = game(13);
        let tile = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        let key = crate::settlements::SettlementKey { rx: 0, ry: 0 };
        game.world.spawn((
            crate::components::Settlement { key },
            Position {
                x: tile.0,
                y: tile.1,
            },
        ));
        assert!(game.found_outpost(tile).is_err());
        // Deleted-fix check: without the occupancy guard this same call
        // would succeed on top of the settlement, since nothing else about
        // an ordinary open tile refuses it.
    }

    #[test]
    fn refuses_when_the_outpost_cap_is_reached() {
        let mut game = game(9);
        let mut founded = Vec::new();
        for _ in 0..MAX_OUTPOSTS {
            let tile = open_tile_at_least(
                &mut game,
                OUTPOST_MIN_ANCHOR_DISTANCE,
                &founded,
                OUTPOST_MIN_SPACING,
            );
            game.found_outpost(tile).unwrap();
            founded.push(tile);
        }
        // A tile that clears every other check — reachable, unoccupied and
        // properly spaced — still refuses once the cap is reached.
        let one_more = open_tile_at_least(
            &mut game,
            OUTPOST_MIN_ANCHOR_DISTANCE,
            &founded,
            OUTPOST_MIN_SPACING,
        );
        assert!(game.found_outpost(one_more).is_err());
    }

    #[test]
    fn refuses_within_min_spacing_of_another_outpost() {
        let mut game = game(10);
        let first = open_tile_at_least(&mut game, OUTPOST_MIN_ANCHOR_DISTANCE, &[], 0);
        game.found_outpost(first).unwrap();
        // The same tile again is distance 0 from `first` — guaranteed to
        // trip the spacing refusal alone, since it already passed every
        // other check once.
        assert!(game.found_outpost(first).is_err());
        // Reachability: a tile spaced far enough away must still succeed.
        let far_enough = open_tile_at_least(
            &mut game,
            OUTPOST_MIN_ANCHOR_DISTANCE,
            &[first],
            OUTPOST_MIN_SPACING,
        );
        game.found_outpost(far_enough).unwrap();
    }

    #[test]
    fn refuses_without_a_loaded_outpost_def() {
        let mut game = game(11);
        // Simulates a deleted `assets/outposts/` — see `OutpostDb`'s doc.
        game.world.insert_resource(OutpostDb::default());
        let (ax, ay) = game.anchor_position().unwrap();
        let tile = (ax + OUTPOST_MIN_ANCHOR_DISTANCE, ay);
        assert_eq!(
            game.found_outpost(tile),
            Err("No outpost design is known.".to_string())
        );
    }
}
