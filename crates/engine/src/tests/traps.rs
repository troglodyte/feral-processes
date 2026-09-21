//! Honeypots: the entity, its placement, its tick, and what walking into
//! one does.

use super::support::*;
use crate::components::{TRAP_GLYPH_ARMED, TRAP_GLYPH_SPRUNG, Trap};
use crate::*;

fn honeypot() -> ItemId {
    ItemId::from("honeypot")
}

fn player_tile(game: &Game) -> Position {
    *game.world.get::<Position>(game.player_entity()).unwrap()
}

/// A trap stood on a tile by hand, which is what every test that is not
/// about `place_trap`'s own refusals wants.
fn stand_a_trap(game: &mut Game, x: i32, y: i32, caught: Option<items::DownedProgram>) -> Entity {
    let ch = if caught.is_some() {
        TRAP_GLYPH_SPRUNG
    } else {
        TRAP_GLYPH_ARMED
    };
    game.world
        .spawn((
            Trap {
                item: honeypot(),
                next_roll: crate::tuning::TRAP_PERIOD_TICKS,
                caught,
            },
            Position { x, y },
            Glyph {
                ch,
                color: GlyphColor::Yellow,
            },
        ))
        .id()
}

fn a_caught_program() -> items::DownedProgram {
    items::DownedProgram {
        species: "shellmon".to_string(),
        level: 3,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 60,
        carried: None,
    }
}

#[test]
fn a_trap_is_named_by_its_item_and_says_whether_it_is_sprung() {
    let mut game = Game::new(7001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = player_tile(&game);

    let armed = stand_a_trap(&mut game, pos.x + 1, pos.y, None);
    let sprung = stand_a_trap(&mut game, pos.x + 2, pos.y, Some(a_caught_program()));

    let armed_label = game.entity_label(armed);
    let sprung_label = game.entity_label(sprung);
    // Asked of the db, never hand-typed, so a renamed item moves the test
    // with the asset.
    let name = game.item_name(&honeypot()).to_string();
    assert!(
        armed_label.contains(&name),
        "an armed trap should name its item, got {armed_label:?}"
    );
    assert!(
        sprung_label.contains(&name),
        "a sprung trap should name its item too, got {sprung_label:?}"
    );
    // The wording is copy; the distinction is the contract, and a line
    // reading the same for both states is the one thing `x` exists to
    // answer.
    assert_ne!(
        armed_label, sprung_label,
        "a sprung trap must not read as an armed one"
    );
}

#[test]
fn find_trap_at_answers_the_tile_it_stands_on_and_nothing_beside_it() {
    let mut game = Game::new(7002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = player_tile(&game);
    let trap = stand_a_trap(&mut game, pos.x + 1, pos.y, None);

    assert_eq!(game.find_trap_at(pos.x + 1, pos.y), Some(trap));
    assert_eq!(game.find_trap_at(pos.x + 2, pos.y), None);
}

#[test]
fn trap_count_is_zero_on_a_fresh_game_and_counts_what_stands() {
    let mut game = Game::new(7003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.trap_count(), 0, "nothing places one at Game::new");
    let pos = player_tile(&game);
    stand_a_trap(&mut game, pos.x + 1, pos.y, None);
    stand_a_trap(&mut game, pos.x + 2, pos.y, Some(a_caught_program()));
    assert_eq!(game.trap_count(), 2);
}

/// Clears the tile `(dx, dy)` from the party of anything the shipped world
/// might have put there and makes it walkable, so a placement test is about
/// the refusal it names rather than about where the seed landed.
fn clear_target(game: &mut Game, dx: i32, dy: i32) -> (i32, i32) {
    let pos = player_tile(game);
    let (nx, ny) = (pos.x + dx, pos.y + dy);
    let squatters: Vec<Entity> = {
        let mut q = game.world.query::<(Entity, &Position)>();
        q.iter(&game.world)
            .filter(|(e, p)| *e != game.player_entity() && p.x == nx && p.y == ny)
            .map(|(e, _)| e)
            .collect()
    };
    for e in squatters {
        game.world.despawn(e);
    }
    game.world.resource_mut::<WorldMap>().set_override(
        nx,
        ny,
        Tile {
            biome: Biome::Platform,
            walkable: true,
            rock_shade: None,
        },
    );
    (nx, ny)
}

/// A game holding `qty` honeypots with a clear tile to the east.
fn ready_to_place(seed: u32, qty: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    give(&mut game, &honeypot(), qty);
    clear_target(&mut game, 1, 0);
    game
}

/// Every refusal asserts the `Err` *and* that nothing moved. A refusal test
/// that only checks the `Err` is the vacuous half — five of these six paths
/// never spend anything anyway.
fn assert_refused(game: &mut Game, result: Result<(), String>, held_before: u32) {
    assert!(result.is_err(), "expected a refusal, got {result:?}");
    assert_eq!(
        held(game, &honeypot()),
        held_before,
        "a refusal must not spend the item"
    );
    assert_eq!(game.trap_count(), 0, "a refusal must not stand anything up");
}

#[test]
fn placing_a_honeypot_spends_one_and_stands_it_on_the_tile() {
    let mut game = ready_to_place(7010, 2);
    let (nx, ny) = {
        let p = player_tile(&game);
        (p.x + 1, p.y)
    };

    game.place_trap(&honeypot(), 1, 0).expect("a clear tile");

    assert_eq!(held(&game, &honeypot()), 1, "one unit is spent");
    assert_eq!(game.trap_count(), 1);
    let trap = game
        .find_trap_at(nx, ny)
        .expect("it stands where it was put");
    let placed = game.world.get::<Trap>(trap).unwrap();
    assert_eq!(placed.item, honeypot());
    assert!(placed.caught.is_none(), "a fresh trap has caught nothing");
    assert_eq!(
        placed.next_roll,
        crate::tuning::TRAP_PERIOD_TICKS - 1,
        "the countdown starts full and placement spends a tick, so nothing is \
         caught on the tick it was set"
    );
    assert_eq!(
        game.world.get::<Glyph>(trap).map(|g| g.ch),
        Some(TRAP_GLYPH_ARMED)
    );
    assert_eq!(
        player_tile(&game).x,
        nx - 1,
        "placing is not a step; the player stays put"
    );
}

#[test]
fn placing_underground_is_refused() {
    let mut game = ready_to_place(7011, 1);
    descend(&mut game);
    let result = game.place_trap(&honeypot(), 1, 0);
    assert_refused(&mut game, result, 1);
}

#[test]
fn placing_an_item_you_do_not_hold_is_refused() {
    let mut game = ready_to_place(7012, 0);
    let result = game.place_trap(&honeypot(), 1, 0);
    assert_refused(&mut game, result, 0);
}

#[test]
fn placing_an_item_with_no_trap_def_is_refused() {
    let mut game = ready_to_place(7013, 1);
    let plain = ItemId::from("ice_breaker");
    give(&mut game, &plain, 1);
    let before = held(&game, &plain);
    let result = game.place_trap(&plain, 1, 0);
    assert!(result.is_err(), "ordinary cargo is not placeable");
    assert_eq!(held(&game, &plain), before, "and nothing is spent");
    assert_eq!(game.trap_count(), 0);
}

#[test]
fn placing_at_the_cap_is_refused() {
    let mut game = ready_to_place(7014, 1);
    let pos = player_tile(&game);
    // Stood up well away from the party, so the cap is the only thing the
    // call can be refusing.
    for i in 0..crate::tuning::TRAP_PLACEMENT_CAP as i32 {
        stand_a_trap(&mut game, pos.x + 50 + i, pos.y + 50, None);
    }
    let result = game.place_trap(&honeypot(), 1, 0);
    assert!(result.is_err(), "the cap refuses");
    assert_eq!(held(&game, &honeypot()), 1, "and spends nothing");
    assert_eq!(game.trap_count(), crate::tuning::TRAP_PLACEMENT_CAP);
}

#[test]
fn placing_on_unwalkable_ground_is_refused() {
    let mut game = ready_to_place(7015, 1);
    let pos = player_tile(&game);
    game.world.resource_mut::<WorldMap>().set_override(
        pos.x + 1,
        pos.y,
        Tile {
            biome: Biome::DataVoid,
            walkable: false,
            rock_shade: None,
        },
    );
    let result = game.place_trap(&honeypot(), 1, 0);
    assert_refused(&mut game, result, 1);
}

#[test]
fn placing_onto_an_occupied_tile_is_refused() {
    let mut game = ready_to_place(7016, 1);
    let pos = player_tile(&game);
    stand_a_trap(&mut game, pos.x + 1, pos.y, None);
    let result = game.place_trap(&honeypot(), 1, 0);
    assert!(result.is_err(), "one trap to a tile");
    assert_eq!(held(&game, &honeypot()), 1);
    assert_eq!(game.trap_count(), 1, "the standing one is untouched");
}

/// Elapses one trap's period and runs the tick pass, up to `attempts`
/// times, and answers what it caught. Each call resets the countdown, so
/// the loop is over capture *rolls* rather than over ticks.
fn spring(game: &mut Game, trap: Entity, attempts: u32) -> Option<items::DownedProgram> {
    for _ in 0..attempts {
        game.world.get_mut::<Trap>(trap).unwrap().next_roll = 0;
        game.run_traps();
        if let Some(caught) = game.world.get::<Trap>(trap).and_then(|t| t.caught.clone()) {
            return Some(caught);
        }
    }
    None
}

/// The whole "existing seeded tests are unaffected" claim rests on this
/// one: a trap that is only counting down must not touch the shared stream.
#[test]
fn a_counting_down_trap_spends_no_gamerng_draw() {
    assert!(
        rng_unadvanced_by(7020, |game| {
            let pos = player_tile(game);
            stand_a_trap(game, pos.x + 1, pos.y, None);
            stand_a_trap(game, pos.x + 2, pos.y, None);
            // Fewer passes than one period, so nothing elapses.
            for _ in 0..crate::tuning::TRAP_PERIOD_TICKS - 1 {
                game.run_traps();
            }
        }),
        "a trap that has not elapsed must not move the shared GameRng stream"
    );
}

/// The draw order is part of the contract: chance, then species, then
/// rarity, then the clamp, then a condition that spends no draw at all. A
/// change here is deliberate and not a refactor.
#[test]
fn the_capture_draws_in_the_documented_order() {
    let build = |seed: u64| {
        let mut game = Game::new(7021, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pos = player_tile(&game);
        let trap = stand_a_trap(&mut game, pos.x + 1, pos.y, None);
        game.world.get_mut::<Trap>(trap).unwrap().next_roll = 0;
        reseed_rng(&mut game, seed);
        game.run_traps();
        game
    };
    let (caught, stream) = (0..512u64)
        .find_map(|seed| {
            let mut probe = build(seed);
            let mut q = probe.world.query::<&Trap>();
            let held = q.iter(&probe.world).find_map(|t| t.caught.clone());
            held.map(|c| (c, seed))
        })
        .expect("some stream in 0..512 catches something");

    // The same four steps, in the same order, driven by hand against the
    // same stream. Anything reordered inside `run_traps` — a rarity rolled
    // before a species, a boss roll not skipped — moves this.
    let mut replay = Game::new(7021, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = player_tile(&replay);
    let (x, y) = (pos.x + 1, pos.y);
    reseed_rng(&mut replay, stream);
    let hit: bool = {
        let mut rng = replay.world.resource_mut::<GameRng>();
        rng.0.random_bool(crate::tuning::TRAP_CAPTURE_CHANCE)
    };
    assert!(hit, "the seed found above is the one that catches");
    let (species, boss) = replay
        .pick_habitat_species(x, y, None, false)
        .expect("the tile has a habitat pool");
    assert!(!boss, "allow_boss: false skips the boss roll entirely");
    let def = replay.species_defs().into_iter().find(|d| d.id == species);
    let rolled = replay.roll_rarity(&def.expect("the species ships"), x, y, false);
    let rarity = rolled.min(Rarity::Silver);
    let condition = items::DownedProgram::roll_condition(rarity, false, 0.0)
        .saturating_sub(crate::tuning::TRAP_CONDITION_PENALTY);

    assert_eq!(caught.species, species, "species is drawn second");
    assert_eq!(caught.rarity, rarity, "rarity is drawn third, then clamped");
    assert_eq!(
        caught.condition, condition,
        "condition is a formula over the clamped rarity and spends no draw"
    );
}

/// A single roll proves nothing here: Silver is a common outcome. Many
/// rolls across many seeds is what makes the ceiling an assertion.
#[test]
fn a_catch_is_never_better_than_the_ceiling_and_never_a_boss() {
    let cap = Rarity::Silver;
    let mut caught_any = 0;
    for seed in 0..24u32 {
        let mut game =
            Game::new(7030 + seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pos = player_tile(&game);
        let trap = stand_a_trap(&mut game, pos.x + 1, pos.y, None);
        let Some(caught) = spring(&mut game, trap, 40) else {
            continue;
        };
        caught_any += 1;
        assert!(
            caught.rarity <= cap,
            "seed {seed} caught a {:?}, above the authored ceiling",
            caught.rarity
        );
        assert!(!caught.boss, "a trap never catches a boss");
        let apex = game
            .species_defs()
            .into_iter()
            .find(|d| d.id == caught.species)
            .map(|d| d.is_boss)
            .unwrap_or(false);
        assert!(!apex, "and never an apex species either");
        assert!(
            caught.carried.is_none(),
            "a caught program never hands over the routine it was running"
        );
        assert_eq!(
            caught.level,
            game.wild_body_level(),
            "a caught program is the zone's level"
        );
    }
    assert!(
        caught_any >= 8,
        "only {caught_any} of 24 seeds caught anything — the sweep is not measuring the ceiling"
    );
}

/// Two traps elapsing on the same tick resolve by `(x, y)`, not by bevy's
/// query order. Asserting the order directly cannot fail — it is stable
/// within one run — so the assertion is that a second run of the same seed
/// produces the same two catches in the same two places.
#[test]
fn two_traps_elapsing_together_resolve_the_same_way_twice() {
    let run = || {
        let mut game = Game::new(7060, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pos = player_tile(&game);
        let east = stand_a_trap(&mut game, pos.x + 1, pos.y, None);
        let west = stand_a_trap(&mut game, pos.x - 1, pos.y, None);
        reseed_rng(&mut game, 404);
        for _ in 0..60 {
            for t in [east, west] {
                game.world.get_mut::<Trap>(t).unwrap().next_roll = 0;
            }
            game.run_traps();
        }
        let read = |e: Entity, g: &Game| g.world.get::<Trap>(e).and_then(|t| t.caught.clone());
        (
            read(east, &game).map(|c| (c.species, c.rarity, c.condition)),
            read(west, &game).map(|c| (c.species, c.rarity, c.condition)),
        )
    };
    assert_eq!(run(), run(), "the sort is what makes this reproducible");
}

#[test]
fn springing_changes_the_glyph_and_a_second_period_does_not_overwrite_it() {
    let mut game = Game::new(7070, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = player_tile(&game);
    let trap = stand_a_trap(&mut game, pos.x + 1, pos.y, None);
    assert_eq!(
        game.world.get::<Glyph>(trap).map(|g| g.ch),
        Some(TRAP_GLYPH_ARMED)
    );

    let caught = spring(&mut game, trap, 60).expect("sixty rolls catch something");
    assert_eq!(
        game.world.get::<Glyph>(trap).map(|g| g.ch),
        Some(TRAP_GLYPH_SPRUNG),
        "a sprung trap says so through the centre glyph's own char"
    );

    // A second elapsed period must not roll again over what is held.
    for _ in 0..40 {
        game.world.get_mut::<Trap>(trap).unwrap().next_roll = 0;
        game.run_traps();
    }
    let still = game
        .world
        .get::<Trap>(trap)
        .unwrap()
        .caught
        .clone()
        .unwrap();
    assert_eq!(
        (still.species, still.rarity, still.condition),
        (caught.species, caught.rarity, caught.condition),
        "a full trap holds what it caught until it is collected"
    );
}

fn downed_count(game: &Game) -> usize {
    game.world
        .get::<DownedPrograms>(game.player_entity())
        .map(|d| d.0.len())
        .unwrap_or(0)
}

#[test]
fn walking_into_an_armed_trap_is_a_bump() {
    let mut game = Game::new(7080, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (nx, ny) = clear_target(&mut game, 1, 0);
    stand_a_trap(&mut game, nx, ny, None);
    let before = player_tile(&game);
    let clock = game.world.resource::<GameClock>().tick;

    game.move_player(1, 0);

    assert_eq!(player_tile(&game), before, "a trap blocks the player");
    assert_eq!(game.trap_count(), 1, "and is still standing");
    assert!(
        game.world.resource::<GameClock>().tick > clock,
        "a bump spends a tick, exactly as the settlement arm does"
    );
}

#[test]
fn walking_into_a_sprung_trap_collects_it() {
    let mut game = Game::new(7081, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (nx, ny) = clear_target(&mut game, 1, 0);
    stand_a_trap(&mut game, nx, ny, Some(a_caught_program()));
    let before = player_tile(&game);
    let held_before = downed_count(&game);

    game.move_player(1, 0);

    assert_eq!(downed_count(&game), held_before + 1, "the program is taken");
    assert_eq!(game.trap_count(), 0, "and the trap is gone with it");
    assert_eq!(
        player_tile(&game),
        before,
        "collecting is not a step either"
    );
}

#[test]
fn collecting_with_the_store_full_moves_nothing() {
    let mut game = Game::new(7082, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (nx, ny) = clear_target(&mut game, 1, 0);
    stand_a_trap(&mut game, nx, ny, Some(a_caught_program()));
    let player = game.player_entity();
    {
        let mut store = game.world.get_mut::<DownedPrograms>(player).unwrap();
        while store.0.len() < crate::tuning::MAX_DOWNED_PROGRAMS {
            store.0.push(a_caught_program());
        }
    }
    let before = player_tile(&game);

    game.move_player(1, 0);

    assert_eq!(
        downed_count(&game),
        crate::tuning::MAX_DOWNED_PROGRAMS,
        "a full store takes nothing more"
    );
    assert_eq!(game.trap_count(), 1, "and the trap keeps what it caught");
    assert!(
        game.find_trap_at(nx, ny)
            .and_then(|t| game.world.get::<Trap>(t))
            .is_some_and(|t| t.caught.is_some()),
        "still sprung, to come back to"
    );
    assert_eq!(player_tile(&game), before);
}

#[test]
fn destroying_a_trap_hands_back_nothing() {
    let mut game = Game::new(7083, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (nx, ny) = clear_target(&mut game, 1, 0);
    stand_a_trap(&mut game, nx, ny, None);
    let held_before = held(&game, &honeypot());

    game.destroy_trap(1, 0).expect("it is right there");

    assert_eq!(game.trap_count(), 0);
    assert_eq!(
        held(&game, &honeypot()),
        held_before,
        "no refund — the half a despawn test misses"
    );
}

#[test]
fn destroying_a_sprung_trap_loses_what_it_caught() {
    let mut game = Game::new(7084, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (nx, ny) = clear_target(&mut game, 1, 0);
    stand_a_trap(&mut game, nx, ny, Some(a_caught_program()));
    let held_before = downed_count(&game);

    game.destroy_trap(1, 0).expect("it is right there");

    assert_eq!(game.trap_count(), 0);
    assert_eq!(
        downed_count(&game),
        held_before,
        "demolishing is not a second way to collect"
    );
}

#[test]
fn destroying_empty_ground_is_refused() {
    let mut game = Game::new(7085, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    clear_target(&mut game, 1, 0);
    let pos = player_tile(&game);
    stand_a_trap(&mut game, pos.x - 1, pos.y, None);

    assert!(game.destroy_trap(1, 0).is_err(), "nothing is there");
    assert_eq!(game.trap_count(), 1, "and the one behind is untouched");
}
