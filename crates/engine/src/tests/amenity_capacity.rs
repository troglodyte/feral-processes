//! An amenity serves at most `tuning::AMENITY_CAPACITY` programs a tick,
//! fewer when it is boxed in, and the rest wait in reach still draining.

use super::needs::{
    a_base_with_an_amenity, coherence, drift, reserve, set_reserve, set_tick, threshold,
};
use super::support::*;
use crate::base_grid::BaseGrid;
use crate::components::Position;
use crate::game::base::offshift::{effective_places, in_reach, open_tiles};
use crate::needs::NeedDb;
use crate::*;

/// The Defrag Bay `a_base_with_an_amenity` builds, with `n` staff run down to
/// just under critical and each standing in reach of it.
fn crowded_bay(seed: u32, n: usize) -> (Game, Position, Vec<Entity>) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let staff = a_base_with_an_amenity(&mut game, n);
    let bay = find_structure_by_kind(&mut game, "defrag_bay").unwrap();
    let site = *game.world.get::<Position>(bay).unwrap();
    let (critical, _) = threshold(&game, &coherence());
    let around = [(0, 1), (-1, 0), (0, -1), (1, 1), (-1, 1)];
    for (i, &who) in staff.iter().enumerate() {
        let (dx, dy) = around[i % around.len()];
        put(&mut game, who, site.x + dx, site.y + dy);
        set_reserve(&mut game, who, &coherence(), critical - 1.0);
    }
    (game, site, staff)
}

fn put(game: &mut Game, who: Entity, x: i32, y: i32) {
    let mut pos = game.world.get_mut::<Position>(who).unwrap();
    pos.x = x;
    pos.y = y;
}

fn drain() -> f32 {
    let game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource::<NeedDb>()
        .get(&coherence())
        .unwrap()
        .drain_per_tick
}

/// Which of `staff` gained reserve over `before` (the drain alone only loses).
fn refilled(game: &Game, staff: &[Entity], before: f32) -> Vec<Entity> {
    staff
        .iter()
        .copied()
        .filter(|&w| reserve(game, w) > before)
        .collect()
}

#[test]
fn places_are_capped_by_capacity_and_by_open_tiles() {
    assert_eq!(effective_places(0), 0);
    assert_eq!(effective_places(1), 1);
    assert_eq!(effective_places(2), crate::tuning::AMENITY_CAPACITY);
    assert_eq!(effective_places(8), crate::tuning::AMENITY_CAPACITY);
}

/// Three programs critical on one bay: today all three refill.
#[test]
fn a_bay_refills_two_of_three_programs_and_the_third_waits() {
    let (mut game, _, staff) = crowded_bay(70, 3);
    let before = reserve(&game, staff[0]);

    game.tick();

    let served = refilled(&game, &staff, before);
    assert_eq!(served.len(), 2, "capacity is two, got {served:?}");
}

/// A program waiting in reach is not frozen: its reserve keeps falling.
#[test]
fn a_waiting_program_still_drains() {
    let (mut game, _, staff) = crowded_bay(71, 3);
    let before = reserve(&game, staff[0]);

    game.tick();

    let waiting: Vec<Entity> = staff
        .iter()
        .copied()
        .filter(|&w| reserve(&game, w) < before)
        .collect();
    assert_eq!(waiting.len(), 1);
    assert!(
        (reserve(&game, waiting[0]) - (before - drain())).abs() < 1e-4,
        "it lost exactly the drain, got {}",
        reserve(&game, waiting[0])
    );
}

/// Whoever holds a place keeps it: a newcomer who would win the tie on entity
/// order does not displace an incumbent.
#[test]
fn an_incumbent_is_not_evicted_by_a_newcomer() {
    let (mut game, site, staff) = crowded_bay(72, 3);
    // staff is sorted, so staff[0] would win every tie. Start it far away.
    put(&mut game, staff[0], site.x - 6, site.y);
    let before = reserve(&game, staff[1]);
    game.tick();
    assert_eq!(refilled(&game, &staff, before), vec![staff[1], staff[2]]);

    put(&mut game, staff[0], site.x + 1, site.y);
    let before = reserve(&game, staff[0]);
    game.tick();

    assert!(
        reserve(&game, staff[0]) < before,
        "the newcomer waits: {before} -> {}",
        reserve(&game, staff[0])
    );
    assert!(reserve(&game, staff[1]) > before - 1.0);
    assert_eq!(refilled(&game, &staff[1..], 0.0).len(), 2);
}

/// Boxed in so only one standing tile is open, a bay serves one program.
#[test]
fn a_boxed_in_bay_serves_one() {
    let (mut game, site, staff) = crowded_bay(73, 2);
    let open = |game: &mut Game| {
        let taken = game.structure_tiles();
        let grid = game.world.resource::<BaseGrid>();
        open_tiles(grid, site, 1, &taken)
    };
    // Fill every open reach tile but one with a Depot.
    let mut keep = None;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (x, y) = (site.x + dx, site.y + dy);
            if (dx, dy) == (0, 0) || !game.world.resource::<BaseGrid>().walkable(x, y) {
                continue;
            }
            if keep.is_none() {
                keep = Some((x, y));
            } else {
                spawn_structure_at(&mut game, "depot", x, y);
            }
        }
    }
    assert_eq!(open(&mut game), 1, "one tile left open");
    let (kx, ky) = keep.unwrap();
    for &who in &staff {
        put(&mut game, who, kx, ky);
    }
    let before = reserve(&game, staff[0]);

    game.tick();

    assert_eq!(refilled(&game, &staff, before).len(), 1);
}

/// With one bay full, a third program heads for the free one.
#[test]
fn an_off_shift_program_routes_to_the_bay_with_a_free_place() {
    let (mut game, site, staff) = crowded_bay(74, 3);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "defrag_bay", 2, 4).expect("a second bay");
    let second = game
        .world
        .iter_entities()
        .filter_map(|e| e.get::<Position>().copied().filter(|p| *p != site))
        .find(|p| (p.x, p.y) == (site.x, site.y + 4))
        .expect("the second bay stands");
    // staff[0] starts nearer the first bay.
    put(&mut game, staff[0], site.x + 3, site.y + 1);
    game.tick();
    let (critical, _) = threshold(&game, &coherence());
    set_reserve(&mut game, staff[0], &coherence(), critical - 1.0);

    for _ in 0..60 {
        let next = game.current_tick() + crate::tuning::IDLE_STAFF_STEP_TICKS;
        set_tick(&mut game, next);
        drift(&mut game, &staff);
    }

    let at = *game.world.get::<Position>(staff[0]).unwrap();
    assert!(in_reach(at, second, 1), "it went to the free bay: {at:?}");
    assert!(!in_reach(at, site, 1));
}

/// `ServicedAt` is not saved; the first tick after a load re-claims.
#[test]
fn capacity_holds_across_a_save_and_load() {
    let (mut game, _, staff) = crowded_bay(75, 3);
    game.tick();
    let dir = scratch_assets_dir("amenity_capacity_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();

    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let levels = |g: &mut Game| -> Vec<f32> {
        let mut v: Vec<(Entity, f32)> = g
            .world
            .query::<(Entity, &crate::components::Needs)>()
            .iter(&g.world)
            .filter_map(|(e, n)| n.get(&coherence()).map(|c| (e, c)))
            .collect();
        v.sort_by_key(|(e, _)| *e);
        v.into_iter().map(|(_, c)| c).collect()
    };
    let _ = &staff;
    let before = levels(&mut loaded);
    loaded.tick();
    let after = levels(&mut loaded);
    let rose = before.iter().zip(&after).filter(|(b, a)| a > b).count();
    assert_eq!(rose, 2, "{before:?} -> {after:?}");
}
