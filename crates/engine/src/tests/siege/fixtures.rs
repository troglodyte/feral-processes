use super::*;

/// The fixture pocket with walkable field all around it, so its siege board
/// is the pocket plus one ring of spawn cells and nothing about it depends on
/// the seed's terrain. With a full ring the board's origin sits one cell
/// up and left of the pocket's bounding box.
pub(crate) fn lay_siege_pocket(game: &mut Game) {
    game.lay_test_pocket();
    let (w, h) = (TEST_POCKET_HALF_WIDTH + 1, TEST_POCKET_HALF_HEIGHT + 1);
    for y in -h..=h {
        for x in -w..=w {
            let floored = game
                .world
                .resource::<crate::base_grid::BaseGrid>()
                .is_floor(x, y);
            if !floored {
                game.world.resource_mut::<WorldMap>().set_override(
                    x,
                    y,
                    Tile {
                        biome: Biome::OpenGrid,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }
}

/// Every walkable cell a besieger could come in over, around the fixture
/// pocket, is ground no species lives on (`Biome::Platform`), so a siege's
/// pack draws nobody.
pub(crate) fn no_habitat_around_the_pocket(game: &mut Game) {
    let (w, h) = (TEST_POCKET_HALF_WIDTH + 1, TEST_POCKET_HALF_HEIGHT + 1);
    for y in -h..=h {
        for x in -w..=w {
            let floored = game
                .world
                .resource::<crate::base_grid::BaseGrid>()
                .is_floor(x, y);
            if !floored {
                game.world.resource_mut::<WorldMap>().set_override(
                    x,
                    y,
                    Tile {
                        biome: Biome::Platform,
                        walkable: true,
                        rock_shade: None,
                    },
                );
            }
        }
    }
}

/// Establishes `base_is_established()` with the minimum staff and
/// structures, and (unless `raidable` is false for every one of them)
/// leaves exactly the first structure carrying `Durability` — a legal
/// siege target — so a test can isolate the "nothing standing" hold by
/// passing `false`.
pub(super) fn establish_base(game: &mut Game, raidable: bool) {
    for _ in 0..BASE_ESTABLISHED_STAFF {
        spawn_tamed(game, 10, 3);
    }
    for i in 0..BASE_ESTABLISHED_STRUCTURES {
        let mut entity = game.world.spawn((
            Structure {
                kind: "test_structure".to_string(),
            },
            Position {
                x: 10 + i as i32,
                y: 10,
            },
        ));
        if raidable && i == 0 {
            entity.insert(Durability { hp: 30, max_hp: 30 });
        }
    }
}

/// Ticks `game.siege_check()` until the pressure resets to zero (a siege
/// fired) or `max` ticks have passed, and says which.
pub(super) fn ticks_to_fire(game: &mut Game, max: u32) -> Option<u32> {
    for t in 1..=max {
        game.siege_check();
        if game.siege_pressure() == 0 {
            return Some(t);
        }
    }
    None
}

/// The widest number of ticks the jitter can make an interval take to fire,
/// at `zone`'s accrual rate — the bound every hold test ticks past to prove
/// the clock has certainly reached its target.
pub(super) fn latest_possible_tick(zone: u32) -> u32 {
    let accrual = SIEGE_PRESSURE_PER_ZONE * zone;
    (SIEGE_PRESSURE_THRESHOLD * (100 + SIEGE_PRESSURE_JITTER_PERCENT) / 100).div_ceil(accrual)
}

/// Ticks `game` until it has both warned and fired, and says the tick each
/// happened on — the raw material for measuring the warning window in
/// ticks rather than in pressure. Requires an established, raidable,
/// fight-free base, or the fire never comes and this panics.
pub(super) fn warn_and_fire_ticks(game: &mut Game, max: u32) -> (u32, u32) {
    let mut warned_at = None;
    for t in 1..=max {
        game.siege_check();
        if warned_at.is_none() && game.siege_warned() {
            warned_at = Some(t);
        }
        if game.siege_pressure() == 0 {
            return (warned_at.expect("a siege must warn before it fires"), t);
        }
    }
    panic!("siege did not fire within {max} ticks");
}
