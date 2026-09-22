//! The siege clock — `resources::SiegePressure`'s accrual, its three holds,
//! the approach warning and its wall-clock floor, and (Task 5) the dev
//! console's two rows.
//!
//! `raids.rs`'s clock tests are the model throughout: a siege's meter is
//! `RaidPressure`'s shape, built the same way, for the reasons recorded
//! there.

use super::support::*;
use crate::tuning::{
    BASE_ESTABLISHED_STAFF, BASE_ESTABLISHED_STRUCTURES, SIEGE_MIN_ZONE,
    SIEGE_PRESSURE_JITTER_PERCENT, SIEGE_PRESSURE_PER_ZONE, SIEGE_PRESSURE_THRESHOLD,
    SIEGE_PRESSURE_WARN_PERCENT, SIEGE_WARN_FLOOR_TICKS,
};
use crate::*;

/// Establishes `base_is_established()` with the minimum staff and
/// structures, and (unless `raidable` is false for every one of them)
/// leaves exactly the first structure carrying `Durability` — a legal
/// siege target — so a test can isolate the "nothing standing" hold by
/// passing `false`.
fn establish_base(game: &mut Game, raidable: bool) {
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
fn ticks_to_fire(game: &mut Game, max: u32) -> Option<u32> {
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
fn latest_possible_tick(zone: u32) -> u32 {
    let accrual = SIEGE_PRESSURE_PER_ZONE * zone;
    (SIEGE_PRESSURE_THRESHOLD * (100 + SIEGE_PRESSURE_JITTER_PERCENT) / 100).div_ceil(accrual)
}

/// Ticks `game` until it has both warned and fired, and says the tick each
/// happened on — the raw material for measuring the warning window in
/// ticks rather than in pressure. Requires an established, raidable,
/// fight-free base, or the fire never comes and this panics.
fn warn_and_fire_ticks(game: &mut Game, max: u32) -> (u32, u32) {
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

/// The opening sector's clock never starts, and it starts from zero on the
/// breach into sector 2 rather than arriving with banked pressure —
/// `the_opening_sector_builds_no_raid_pressure`'s exact shape.
#[test]
fn the_opening_sector_builds_no_siege_pressure() {
    assert_eq!(
        SIEGE_MIN_ZONE, 2,
        "sieges are exempt in the opening sector and nowhere else"
    );

    let mut game = Game::new(900, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 1);
    for _ in 0..5_000 {
        game.siege_check();
    }
    assert_eq!(
        game.siege_pressure(),
        0,
        "the opening sector's siege clock never starts"
    );

    set_zone(&mut game, 2);
    game.siege_check();
    assert_eq!(
        game.siege_pressure(),
        SIEGE_PRESSURE_PER_ZONE * 2,
        "and it starts from zero on the breach into sector 2"
    );
}

/// The first hold: no base. A tick that decides a siege happens but has no
/// base to besiege holds its pressure rather than spending it, and the very
/// next tick a base exists is the tick the held siege fires.
#[test]
fn a_tick_with_no_base_holds_the_clock_until_one_exists() {
    let mut game = Game::new(901, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);

    for _ in 0..=latest_possible_tick(2) {
        game.siege_check();
    }
    assert!(
        game.siege_pressure() > SIEGE_PRESSURE_THRESHOLD,
        "a run with no base holds its clock rather than spending it, but the clock read {}",
        game.siege_pressure()
    );

    establish_base(&mut game, true);
    game.siege_check();
    assert_eq!(
        game.siege_pressure(),
        0,
        "the tick the base is established is the tick the held siege fires"
    );
}

/// The second hold: nothing standing to besiege. An established base with
/// every structure stripped of `Durability` holds its clock exactly as a
/// base that does not exist yet does, and the tick something raidable
/// stands is the tick the held siege fires.
#[test]
fn a_tick_with_nothing_to_besiege_holds_the_clock() {
    let mut game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);
    establish_base(&mut game, false);

    for _ in 0..=latest_possible_tick(2) {
        game.siege_check();
    }
    assert!(
        game.siege_pressure() > SIEGE_PRESSURE_THRESHOLD,
        "an established base with nothing raidable standing holds its clock, but the clock read {}",
        game.siege_pressure()
    );

    let target: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Structure>>();
        query.iter(&game.world).collect()
    };
    game.world
        .entity_mut(target[0])
        .insert(Durability { hp: 30, max_hp: 30 });
    game.siege_check();
    assert_eq!(
        game.siege_pressure(),
        0,
        "the tick something is standing to besiege is the tick the held siege fires"
    );
}

/// The third hold: another fight already running. A siege must not open —
/// or, before Task 8, resolve — on top of one, and it fires the tick the
/// other fight ends.
#[test]
fn a_tick_with_another_fight_running_holds_the_clock() {
    let mut game = Game::new(903, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);
    establish_base(&mut game, true);

    let player = game.player_entity();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                mitigation: 1,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    for _ in 0..=latest_possible_tick(2) {
        game.siege_check();
    }
    assert!(
        game.siege_pressure() > SIEGE_PRESSURE_THRESHOLD,
        "a siege must not fire on top of another fight, but the clock read {}",
        game.siege_pressure()
    );

    game.world.remove_resource::<BattleState>();
    game.siege_check();
    assert_eq!(
        game.siege_pressure(),
        0,
        "the tick the other fight ends is the tick the held siege fires"
    );
}

/// The interval is drawn once, not once a tick.
///
/// A stable `next_at` across two ticks would pass equally well against an
/// implementation that redraws the same interval and discards the roll, so
/// this compares the shared `GameRng` stream rather than only the field:
/// the drawn value must be the stream's *first* draw, and two ticks must
/// leave the stream exactly one draw further along.
#[test]
fn the_interval_is_drawn_once_not_once_a_tick() {
    let seed = 904;
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);

    game.siege_check();
    let after_first = game
        .world
        .resource::<crate::resources::SiegePressure>()
        .next_at;
    game.siege_check();
    let after_second = game
        .world
        .resource::<crate::resources::SiegePressure>()
        .next_at;
    assert_eq!(
        after_first, after_second,
        "the interval must not be redrawn on the second tick"
    );
    let target = after_first.expect("the first tick must draw an interval");

    // A reference game built from the same seed, asked to make exactly one
    // draw of the same shape `draw_siege_interval` makes. If `game` drew
    // more than once, or drew and discarded, the two streams diverge here.
    let mut reference = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let low = SIEGE_PRESSURE_THRESHOLD * (100 - SIEGE_PRESSURE_JITTER_PERCENT) / 100;
    let high = SIEGE_PRESSURE_THRESHOLD * (100 + SIEGE_PRESSURE_JITTER_PERCENT) / 100;
    let one_draw: u32 = reference
        .world
        .resource_mut::<GameRng>()
        .0
        .random_range(low..=high);
    assert_eq!(
        one_draw, target,
        "the clock's one draw must be the stream's first"
    );

    let after_game: u64 = game.world.resource_mut::<GameRng>().0.random();
    let after_reference: u64 = reference.world.resource_mut::<GameRng>().0.random();
    assert_eq!(
        after_game, after_reference,
        "two ticks must cost exactly the one draw the reference also made"
    );
}

/// A siege in sector 6 comes at three times the rate of one in sector 2 —
/// asserted on the tick counts each takes to fire, not on the constants,
/// since the two games share a seed and so draw the identical target.
///
/// `ticks2 = ceil(target / 2)` and `ticks6 = ceil(target / 6)` can disagree
/// with an exact `3 * ticks6` by the rounding each `ceil` introduces; both
/// lie in `[target / 2, target / 2 + 3)`, which bounds the gap between them
/// at 2 ticks whatever the drawn target turns out to be.
#[test]
fn a_deeper_sector_sieges_three_times_as_fast() {
    let seed = 905;
    let max = latest_possible_tick(2) + 1;

    let mut zone2 = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut zone2, 2);
    establish_base(&mut zone2, true);
    let ticks2 = ticks_to_fire(&mut zone2, max).expect("sector 2 must fire within its own bound");

    let mut zone6 = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut zone6, 6);
    establish_base(&mut zone6, true);
    let ticks6 =
        ticks_to_fire(&mut zone6, max).expect("sector 6 must fire well within sector 2's bound");

    assert!(
        ticks2.abs_diff(3 * ticks6) <= 2,
        "sector 6 should fire in about a third of sector 2's ticks: {ticks2} vs 3x{ticks6}"
    );
}

/// The dev console's trigger fires the real thing — `stage_siege` — and
/// leaves the clock untouched, `dev_force_raid`'s shape exactly.
#[test]
fn forcing_a_siege_stages_one_without_touching_the_clock() {
    let mut game = Game::new(906, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "test_structure".to_string(),
            },
            Position { x: 1, y: 1 },
            Durability { hp: 30, max_hp: 30 },
        ))
        .id();
    let before = *game.world.resource::<crate::resources::SiegePressure>();

    game.dev_force_siege();

    assert_eq!(
        *game.world.resource::<crate::resources::SiegePressure>(),
        before,
        "forcing a siege must not touch the clock, matching the sweep's own dev trigger"
    );
    assert!(
        game.world
            .get::<Durability>(structure)
            .map(|d| d.hp < 30)
            .unwrap_or(true),
        "the forced fire must be the real thing — an undefended structure takes damage"
    );
}

/// Winding the clock lands exactly on the drawn interval's own warn point
/// and leaves `warned` clear — `winding_the_clock_reaches_the_warning_and_not_the_sweep`'s
/// shape, one press working whatever the jitter rolled.
#[test]
fn winding_the_siege_clock_reaches_the_warn_point() {
    let mut game = Game::new(907, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);

    game.dev_wind_siege_clock();

    let pressure = *game.world.resource::<crate::resources::SiegePressure>();
    let target = pressure
        .next_at
        .expect("winding draws the interval it winds to");
    assert_eq!(
        pressure.level,
        target * SIEGE_PRESSURE_WARN_PERCENT / 100,
        "the clock must land on the drawn interval's own warn point"
    );
    assert!(!pressure.warned, "and leave the line for the next cycle");

    game.siege_check();
    assert!(game.siege_warned(), "the very next tick must warn");
}

/// At sector 2, `SIEGE_PRESSURE_WARN_PERCENT`'s share of the interval is
/// smaller than the wall-clock floor's converted pressure across the whole
/// jitter range, so the share governs — and the window it buys is longer
/// than the floor.
#[test]
fn a_shallow_sector_warns_by_share_and_its_window_beats_the_floor() {
    let mut game = Game::new(908, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);
    establish_base(&mut game, true);

    let (warned_at, fired_at) = warn_and_fire_ticks(&mut game, latest_possible_tick(2) + 1);
    let window = fired_at - warned_at;

    assert!(
        window > SIEGE_WARN_FLOOR_TICKS,
        "sector 2's share-governed window ({window} ticks) must beat the floor \
         ({SIEGE_WARN_FLOOR_TICKS} ticks)"
    );
}

/// At sector 6, the wall-clock floor governs across the whole jitter range,
/// and it converts to *exactly* `SIEGE_WARN_FLOOR_TICKS` ticks of notice —
/// `by_floor = target - SIEGE_WARN_FLOOR_TICKS * accrual` cancels cleanly
/// against `accrual` on the way back through ticks, for any drawn target.
#[test]
fn a_deep_sector_warns_by_the_floor_at_exactly_its_own_value() {
    let mut game = Game::new(909, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 6);
    establish_base(&mut game, true);

    let (warned_at, fired_at) = warn_and_fire_ticks(&mut game, latest_possible_tick(6) + 1);
    let window = fired_at - warned_at;

    assert_eq!(
        window, SIEGE_WARN_FLOOR_TICKS,
        "sector 6's floor-governed window must be exactly the floor's own value"
    );
}

/// The warning fires once an interval, not once a tick — `raids.rs`'s
/// `the_approach_warning_fires_once_an_interval_and_not_once_a_tick` shape.
/// No structure is spawned raidable, so the clock holds past its target and
/// the latch is never cleared by a reset, which is what makes "exactly one"
/// a claim about the latch and not about the interval's length. Counted off
/// the raw log (`message_log`), which does not condense, unlike
/// `message_history` — a line said many times would inflate this count if
/// the latch were broken.
#[test]
fn the_approach_warning_fires_once_an_interval_and_not_once_a_tick() {
    let mut game = Game::new(910, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);
    establish_base(&mut game, false);

    for _ in 0..=latest_possible_tick(2) {
        game.siege_check();
    }

    let warnings = game
        .message_log(usize::MAX)
        .into_iter()
        .filter(|e| e.text.contains("siege is forming"))
        .count();
    assert_eq!(
        warnings, 1,
        "the warning must latch for the whole interval, not repeat every tick"
    );
}

/// The warning reaches `Game::attention`, present from the warning until the
/// siege lands and absent both before and after.
#[test]
fn an_approaching_siege_asks_for_the_players_attention() {
    let mut game = Game::new(911, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);
    establish_base(&mut game, true);

    fn row_present(game: &mut Game) -> bool {
        game.attention()
            .iter()
            .any(|row| row.kind == AttentionKind::SiegeIncoming)
    }

    assert!(!row_present(&mut game), "a quiet clock asks for nothing");

    let max = latest_possible_tick(2) + 1;
    let mut warned = false;
    for _ in 1..=max {
        game.siege_check();
        if !warned && row_present(&mut game) {
            warned = true;
            let row = game
                .attention()
                .into_iter()
                .find(|row| row.kind == AttentionKind::SiegeIncoming)
                .expect("just confirmed present");
            assert!(row.threat, "an approaching siege reads as a threat");
        }
        if game.siege_pressure() == 0 {
            break;
        }
    }
    assert!(warned, "the fixture must warn before firing");
    assert!(
        !row_present(&mut game),
        "the row must clear once the siege has landed"
    );
}

#[cfg(test)]
mod board {
    use super::*;
    use crate::base_grid::BaseGrid;
    use crate::game::base_space::BASE_EXIT_CELL;
    use crate::game::siege::board::build;
    use crate::tactical::map::BattleCell;

    /// Cuts `cells` into an otherwise solid `BaseGrid`, floored — a base
    /// with a known, deliberate shape for the flood fill to walk.
    fn dig(game: &mut Game, cells: &[(i32, i32)]) {
        let mut grid = game.world.resource_mut::<BaseGrid>();
        for &(x, y) in cells {
            grid.lay_floor(x, y);
        }
    }

    #[test]
    fn no_base_at_all_yields_no_board() {
        let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(build(&mut game).is_none());
    }

    /// Rock reads `Blocked` and laid floor `Open`, on a base with a known
    /// cut.
    #[test]
    fn rock_is_blocked_and_laid_floor_is_open() {
        let mut game = Game::new(941, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        dig(&mut game, &[(0, 0), (1, 0), (2, 0)]);

        let siege_board = build(&mut game).expect("the door is walkable");

        let door = siege_board.to_board(BASE_EXIT_CELL).unwrap();
        assert_eq!(siege_board.board.cell(door.0, door.1), BattleCell::Open);
        let dug = siege_board.to_board((1, 0)).unwrap();
        assert_eq!(siege_board.board.cell(dug.0, dug.1), BattleCell::Open);

        // A cell just off the cut corridor was never dug, so it is not in
        // the fill and reads `Blocked` — whether or not it lies inside the
        // board's own bounding box.
        if let Some(off) = siege_board.to_board((1, 1)) {
            assert_eq!(siege_board.board.cell(off.0, off.1), BattleCell::Blocked);
        }
    }

    /// A pocket of floor with no walkable path to the door is absent from
    /// the board by construction — never visited by the fill, never opened.
    #[test]
    fn a_sealed_pocket_is_excluded_from_the_board() {
        let mut game = Game::new(942, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // A corridor out from the door, and a pocket at (5, 0) with no
        // walkable neighbour connecting it back — solid rock all around it.
        dig(&mut game, &[(0, 0), (1, 0), (2, 0)]);
        dig(&mut game, &[(5, 0)]);

        let siege_board = build(&mut game).expect("the door is walkable");

        // The pocket may or may not fall inside the bounding box the
        // connected corridor produced; either way it must never read
        // `Open`, since the fill never reached it.
        if let Some(pocket) = siege_board.to_board((5, 0)) {
            assert_eq!(
                siege_board.board.cell(pocket.0, pocket.1),
                BattleCell::Blocked,
                "a sealed pocket must not be reachable on the board"
            );
        }
    }

    /// `to_board`/`to_base` round-trip for every cell of a fill, and
    /// `to_board` answers `None` outside the box.
    #[test]
    fn to_board_and_to_base_round_trip() {
        let mut game = Game::new(943, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        dig(&mut game, &[(0, 0), (1, 0), (0, 1), (-1, 0)]);

        let siege_board = build(&mut game).expect("the door is walkable");

        for base_cell in [(0, 0), (1, 0), (0, 1), (-1, 0)] {
            let cell = siege_board
                .to_board(base_cell)
                .expect("a dug cell must be on the board");
            assert_eq!(siege_board.to_base(cell), base_cell);
        }

        assert_eq!(siege_board.to_board((1000, 1000)), None);
    }

    /// The door is in bounds and `Open`.
    #[test]
    fn the_door_is_in_bounds_and_open() {
        let mut game = Game::new(944, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.lay_starting_pocket();
        let siege_board = build(&mut game).expect("the starting pocket floors the door");
        assert!(
            siege_board
                .board
                .in_bounds(siege_board.door.0, siege_board.door.1)
        );
        assert_eq!(
            siege_board
                .board
                .cell(siege_board.door.0, siege_board.door.1),
            BattleCell::Open
        );
        assert_eq!(siege_board.to_board(BASE_EXIT_CELL), Some(siege_board.door));
    }

    /// A single-cell base yields a 1x1 board rather than `None` or a panic.
    #[test]
    fn a_single_cell_base_yields_a_1x1_board() {
        let mut game = Game::new(945, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // A brand-new base already floors `BASE_EXIT_CELL` — see
        // `Game::lay_starting_pocket` — so an entirely rock-walled cell
        // does not occur in play. This pins the fill's own behaviour when
        // it genuinely finds nothing beyond the door: a 1x1 board, not a
        // crash or a `None`.
        let mut grid = crate::base_grid::BaseGrid::default();
        grid.set_seed(1);
        grid.lay_floor(BASE_EXIT_CELL.0, BASE_EXIT_CELL.1);
        game.world.insert_resource(grid);

        let siege_board = build(&mut game).expect("the door alone is still walkable");
        assert_eq!(siege_board.board.side, 1);
        assert_eq!(siege_board.origin, BASE_EXIT_CELL);
        assert_eq!(siege_board.door, (0, 0));
    }
}

mod opening {
    use super::*;
    use crate::components::Hostile;
    use crate::game::siege::board;
    use crate::game::siege::offscreen::pack_size;
    use crate::resources::Party;
    use crate::tactical::TacticalBattle;

    /// A floored pocket and the party standing in it — the minimum a siege
    /// can open on.
    fn ready_base(game: &mut Game) {
        game.lay_starting_pocket();
        stand_in_base_at(game, 0, 0);
    }

    /// With `Profile::tactical_battles` **off**, a siege still opens a
    /// `TacticalBattle` — §5 of the design: a player who has turned battle
    /// maps off still fights this one on a map, and `fights_tactically` is
    /// never consulted to get there.
    #[test]
    fn a_siege_opens_a_tactical_board_even_with_the_toggle_off() {
        let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);
        let mut profile = game.profile().clone();
        profile.tactical_battles = false;
        game.install_profile(profile);

        assert!(game.open_siege());
        assert!(game.in_tactical_battle());
    }

    /// A posted program standing at its machine is on the board at that
    /// cell — nobody is deployed, the base's own arrangement is the opening
    /// position.
    #[test]
    fn a_posted_program_is_seated_at_the_cell_it_already_stands_on() {
        let mut game = Game::new(951, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);
        let worker = spawn_tamed(&mut game, 10, 3);
        {
            let mut pos = game.world.get_mut::<Position>(worker).unwrap();
            pos.x = 2;
            pos.y = 0;
        }
        let expected = board::build(&mut game)
            .unwrap()
            .to_board((2, 0))
            .expect("(2, 0) must be inside the starting pocket");

        assert!(game.open_siege());

        let battle = game.world.resource::<TacticalBattle>();
        assert_eq!(
            battle.cell_of(worker),
            Some(expected),
            "the worker must be seated exactly where it already stood"
        );
    }

    /// The player is on the board and their party with them.
    #[test]
    fn the_player_and_their_party_are_seated() {
        let mut game = Game::new(952, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);
        let companion = spawn_tamed(&mut game, 10, 3);
        game.world.resource_mut::<Party>().0.push(companion);

        assert!(game.open_siege());

        let player = game.player_entity();
        let battle = game.world.resource::<TacticalBattle>();
        assert!(
            battle.cell_of(player).is_some(),
            "the player must be seated"
        );
        assert!(
            battle.cell_of(companion).is_some(),
            "the party must be seated with the player"
        );
    }

    /// Raiders are all at or adjacent to the door at round 1, and their
    /// count is exactly `siege::pack_size(zone)` — the same call the
    /// off-screen resolution prices a shortfall with, so the siege you
    /// fight and the siege you miss field the same pack.
    #[test]
    fn raiders_are_seated_at_the_door_and_number_pack_size() {
        let mut game = Game::new(953, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);
        let door = board::build(&mut game).unwrap().door;

        assert!(game.open_siege());

        let bodies: Vec<(Entity, (i32, i32))> =
            game.world.resource::<TacticalBattle>().bodies().collect();
        let raiders: Vec<(Entity, (i32, i32))> = bodies
            .into_iter()
            .filter(|&(e, _)| game.world.get::<Hostile>(e).is_some())
            .collect();

        assert_eq!(
            raiders.len() as u32,
            pack_size(2),
            "the seated pack must be exactly what pack_size(zone) prices"
        );
        for &(_, cell) in &raiders {
            let dist = (cell.0 - door.0).abs().max((cell.1 - door.1).abs());
            assert!(
                dist <= 2,
                "a raider at {cell:?} strayed far from the door at {door:?}"
            );
        }
    }

    /// A siege does not open while another fight is running — the hold,
    /// asserted through `siege_check` rather than `open_siege` directly,
    /// with the player standing in base space so the fire would otherwise
    /// take the home branch.
    #[test]
    fn a_siege_does_not_open_on_top_of_another_fight() {
        let mut game = Game::new(954, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);
        // `siege_check`'s own "no base" and "nothing to besiege" holds sit
        // above the one this test targets — establishing the base for real
        // is what isolates the "another fight is running" hold alone.
        establish_base(&mut game, true);

        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 3, y: 3 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 0,
                    mitigation: 1,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        game.dev_wind_siege_clock();
        game.world
            .resource_mut::<crate::resources::SiegePressure>()
            .level = crate::tuning::SIEGE_PRESSURE_THRESHOLD * 2;
        game.siege_check();

        assert!(
            !game.in_tactical_battle(),
            "a siege must not open on top of another fight"
        );

        game.world.remove_resource::<BattleState>();
        game.siege_check();
        assert!(
            game.in_tactical_battle(),
            "the tick the other fight ends is the tick the held siege opens"
        );
    }
}

/// Structures seated as bodies on a siege board — `Game::tactical_attack`'s
/// structure branch (Task 10).
mod board_combat {
    use super::*;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::{BattleCell, BattleSpec, Board};
    use crate::world::Biome;

    /// A wide-open board, `player` at `(0, 0)`, `structure` at `(1, 0)` —
    /// adjacent, melee range — and a hostile far off so `settle_tactical`'s
    /// "hostiles > 0" does not end the fight the moment a round boundary
    /// (which a single-body initiative order crosses on its very first
    /// hand-on) is crossed.
    fn structure_within_reach(game: &mut Game, hp: u32) -> Entity {
        let side = 10;
        let mut board = Board::solid(side);
        for y in 0..side {
            for x in 0..side {
                board.set(x, y, BattleCell::Open);
            }
        }
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 1,
        };
        game.world
            .insert_resource(TacticalBattle::open(spec, board));

        let player = game.player_entity();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Durability { hp, max_hp: hp },
            ))
            .id();
        let distant_hostile = game
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
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(player, (0, 0));
            battle.place(structure, (1, 0));
            battle.place(distant_hostile, (9, 9));
            battle.set_initiative(vec![player]);
        }
        structure
    }

    /// A swing at a machine lowers its `Durability` and does not panic on
    /// the missing `Stats` — the structure survives the one swing, so it
    /// is still seated and still holds its cell.
    #[test]
    fn a_swing_lowers_durability_and_the_structure_stays_seated() {
        let mut game = Game::new(960, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = structure_within_reach(&mut game, 10_000);

        assert!(game.tactical_attack(structure));

        let hp = game.world.get::<Durability>(structure).unwrap().hp;
        assert!(hp < 10_000, "an unhit structure has taken no damage");
        let battle = game.world.resource::<TacticalBattle>();
        assert_eq!(
            battle.cell_of(structure),
            Some((1, 0)),
            "a structure that survived the swing must still be seated"
        );
    }

    /// A machine destroyed on the board is destroyed the way any other one
    /// is — the same log line and the same teardown `damage_structure`'s
    /// other callers get — and its cells become walkable, the fight
    /// continuing around it.
    #[test]
    fn a_destroyed_structure_is_torn_down_and_its_cell_frees() {
        let mut game = Game::new(961, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = structure_within_reach(&mut game, 1);

        assert!(game.tactical_attack(structure));

        assert!(
            game.world.get::<Structure>(structure).is_none(),
            "a destroyed structure is despawned, damage_structure's own teardown"
        );
        let destroyed = game
            .message_log(usize::MAX)
            .into_iter()
            .any(|e| e.text.contains("destroyed in a siege!"));
        assert!(destroyed, "the destruction must log the same as any other");

        let battle = game.world.resource::<TacticalBattle>();
        assert_eq!(
            battle.cell_of(structure),
            None,
            "a destroyed structure must leave the board"
        );
        assert_eq!(
            battle.occupant((1, 0)),
            None,
            "its cell must be free for anyone to stand on"
        );
        assert!(
            game.world.get_resource::<TacticalBattle>().is_some(),
            "the fight must continue after the structure falls"
        );
    }

    /// Nothing about the initiative order moved when a non-combatant
    /// structure died — it held no slot to begin with.
    #[test]
    fn destroying_a_structure_does_not_touch_initiative() {
        let mut game = Game::new(962, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = structure_within_reach(&mut game, 1);
        let companion = game
            .world
            .spawn(Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                mitigation: 0,
            })
            .id();
        let player = game.player_entity();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(companion, (5, 5));
            battle.set_initiative(vec![player, companion]);
        }
        let before: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();

        assert!(game.tactical_attack(structure));

        let after: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();
        assert_eq!(before, after, "a structure's death must not move the order");
    }
}

mod turrets {
    use super::*;
    use crate::game::siege::turrets::turret_defense;

    /// A base with no turret contributes nothing.
    #[test]
    fn a_base_with_no_turret_contributes_no_turret_defense() {
        let game = Game::new(920, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(turret_defense(&game), 0);
    }

    /// A deployed turret contributes exactly its def's `damage` — matching
    /// `shield_structure_loads_with_no_work_and_a_raid_defense_bonus`'s
    /// read-the-def-rather-than-assume-a-literal style, so a retune of
    /// `turret.ron` cannot silently desync this test from the content.
    #[test]
    fn a_deployed_turret_contributes_its_defs_damage() {
        let mut game = Game::new(921, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let damage = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "turret")
            .expect("turret.ron should load as a structure")
            .turret
            .expect("turret.ron should declare a turret")
            .damage;

        game.world.spawn((
            Structure {
                kind: "turret".to_string(),
            },
            Position { x: 1, y: 1 },
        ));

        assert_eq!(turret_defense(&game), damage);
    }

    /// Two turrets stack, `total_raid_defense`'s own additive rule applied
    /// to the same structure list.
    #[test]
    fn two_deployed_turrets_sum_their_damage() {
        let mut game = Game::new(922, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let damage = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "turret")
            .expect("turret.ron should load as a structure")
            .turret
            .expect("turret.ron should declare a turret")
            .damage;

        for i in 0..2 {
            game.world.spawn((
                Structure {
                    kind: "turret".to_string(),
                },
                Position { x: i, y: 1 },
            ));
        }

        assert_eq!(turret_defense(&game), damage * 2);
    }

    /// An ordinary structure with no `turret:` field contributes nothing,
    /// even though it may still contribute `raid_defense` — the two are
    /// independent fields on the same def.
    #[test]
    fn a_non_turret_structure_contributes_no_turret_defense() {
        let mut game = Game::new(923, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.spawn((
            Structure {
                kind: "shield".to_string(),
            },
            Position { x: 1, y: 1 },
        ));
        assert_eq!(turret_defense(&game), 0);
    }
}

mod offscreen {
    use super::*;
    use crate::components::Downed;
    use crate::game::siege::offscreen::pack_size;
    use crate::tuning::{SIEGE_PACK_BASE, SIEGE_PACK_MAX, SIEGE_PACK_PER_ZONE};

    /// `pack_size` grows with the sector and stops at `SIEGE_PACK_MAX`.
    #[test]
    fn pack_size_grows_with_the_sector_and_caps() {
        assert_eq!(pack_size(1), SIEGE_PACK_BASE + SIEGE_PACK_PER_ZONE);
        assert_eq!(pack_size(2), SIEGE_PACK_BASE + SIEGE_PACK_PER_ZONE * 2);
        assert!(pack_size(2) > pack_size(1), "a deeper sector fields more");

        let uncapped = SIEGE_PACK_BASE + SIEGE_PACK_PER_ZONE * 50;
        assert!(
            uncapped > SIEGE_PACK_MAX,
            "the fixture must actually threaten the cap"
        );
        assert_eq!(pack_size(50), SIEGE_PACK_MAX);
    }

    /// A shortfall of zero costs nothing and logs nothing — an established,
    /// well-staffed base facing the smallest pack the game fields.
    #[test]
    fn a_covered_shortfall_costs_and_logs_nothing() {
        let mut game = Game::new(930, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut game, 1);
        for _ in 0..10 {
            spawn_tamed(&mut game, 10, 3);
        }
        let mut stock = Stock::new(100);
        stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), 20);
        game.world.spawn((
            Structure {
                kind: "test_structure".to_string(),
            },
            Position { x: 1, y: 1 },
            Durability { hp: 30, max_hp: 30 },
            stock,
        ));
        let before_len = game.message_log(usize::MAX).len();

        let fired = game.resolve_siege_offscreen();

        assert!(fired, "resolving an off-screen siege always reports true");
        assert_eq!(
            game.message_log(usize::MAX).len(),
            before_len,
            "a fully-covered pack must log nothing"
        );
    }

    /// Stores stolen: an undefended, stocked base loses units off its
    /// shelves.
    #[test]
    fn an_undefended_siege_steals_from_the_shelves() {
        let mut game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut game, 6);
        let item = ItemId::from(ids::CORE_FRAGMENT);
        let mut stock = Stock::new(1000);
        stock.output.insert(item.clone(), 500);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                stock,
            ))
            .id();

        game.resolve_siege_offscreen();

        let remaining = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&item)
            .copied()
            .unwrap_or(0);
        assert!(remaining < 500, "an undefended shelf must lose units");
    }

    /// Structures damaged: a machine with more Durability than the pack can
    /// spend survives, wounded.
    #[test]
    fn an_undefended_siege_damages_a_sturdy_machine() {
        let mut game = Game::new(932, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut game, 2);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                Durability {
                    hp: 10_000,
                    max_hp: 10_000,
                },
            ))
            .id();

        game.resolve_siege_offscreen();

        let hp = game.world.get::<Durability>(structure).unwrap().hp;
        assert!(hp < 10_000, "an undefended machine must take damage");
    }

    /// Structures damaged: a machine with less Durability than the pack can
    /// spend is destroyed outright, and the remainder spreads to a second
    /// machine.
    #[test]
    fn an_undefended_siege_destroys_a_frail_machine_and_spreads_the_rest() {
        let mut game = Game::new(933, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut game, 6);
        let frail = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure_a".to_string(),
                },
                Position { x: 1, y: 1 },
                Durability { hp: 1, max_hp: 1 },
            ))
            .id();
        let sturdy = game
            .world
            .spawn((
                Structure {
                    kind: "test_structure_b".to_string(),
                },
                Position { x: 2, y: 1 },
                Durability {
                    hp: 10_000,
                    max_hp: 10_000,
                },
            ))
            .id();

        game.resolve_siege_offscreen();

        assert!(
            game.world.get::<Durability>(frail).is_none(),
            "a machine outmatched by the whole shortfall must be destroyed outright"
        );
        assert!(
            game.world.get::<Durability>(sturdy).unwrap().hp < 10_000,
            "the shortfall left over from destroying the frail machine must spread to the next one"
        );
    }

    /// Staff killed: a big enough shortfall benches a defender.
    #[test]
    fn an_undefended_siege_benches_a_staff_body() {
        let mut game = Game::new(934, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Zone 8: `pack_size` is already at `SIEGE_PACK_MAX` (12), and one
        // staff body's own defence (2) leaves a shortfall of 10 — exactly
        // `SIEGE_POINTS_PER_CASUALTY`, so one casualty is guaranteed without
        // outrunning the pack cap.
        set_zone(&mut game, 8);
        let staff = spawn_tamed(&mut game, 10, 3);

        game.resolve_siege_offscreen();

        assert!(
            game.world.get::<Downed>(staff).is_some(),
            "a large enough shortfall must bench a defending staff body"
        );
    }

    /// A defended base loses strictly less than an undefended one facing the
    /// same pack — asserted as a comparison between two runs, since a
    /// constant moves with a retune and the ordering must not.
    #[test]
    fn a_defended_base_loses_less_than_an_undefended_one() {
        let item = ItemId::from(ids::CORE_FRAGMENT);

        let mut undefended = Game::new(935, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut undefended, 6);
        let mut stock = Stock::new(1000);
        stock.output.insert(item.clone(), 500);
        let poor_shelf = undefended
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                stock.clone(),
            ))
            .id();
        undefended.resolve_siege_offscreen();
        let undefended_loss = 500
            - undefended
                .world
                .get::<Stock>(poor_shelf)
                .unwrap()
                .output
                .get(&item)
                .copied()
                .unwrap_or(0);

        let mut defended = Game::new(935, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut defended, 6);
        for _ in 0..2 {
            spawn_tamed(&mut defended, 10, 3);
        }
        let rich_shelf = defended
            .world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                stock,
            ))
            .id();
        defended.resolve_siege_offscreen();
        let defended_loss = 500
            - defended
                .world
                .get::<Stock>(rich_shelf)
                .unwrap()
                .output
                .get(&item)
                .copied()
                .unwrap_or(0);

        assert!(
            defended_loss < undefended_loss,
            "a defended base ({defended_loss} lost) must lose strictly less than an \
             undefended one ({undefended_loss} lost) facing the same pack"
        );
    }

    /// The whole resolution spends no `GameRng` draw — a deterministic
    /// payout is what lets Task 8's on-screen fight be the only place a
    /// siege rolls.
    #[test]
    fn resolving_offscreen_spends_no_rng_draw() {
        let seed = 936;
        let mut touched = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let mut untouched = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        set_zone(&mut touched, 6);
        set_zone(&mut untouched, 6);
        for game in [&mut touched, &mut untouched] {
            let item = ItemId::from(ids::CORE_FRAGMENT);
            let mut stock = Stock::new(1000);
            stock.output.insert(item, 500);
            game.world.spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Position { x: 1, y: 1 },
                Durability {
                    hp: 10_000,
                    max_hp: 10_000,
                },
                stock,
            ));
        }
        spawn_tamed(&mut touched, 10, 3);
        spawn_tamed(&mut untouched, 10, 3);

        touched.resolve_siege_offscreen();

        let after_touched: u64 = touched.world.resource_mut::<GameRng>().0.random();
        let after_untouched: u64 = untouched.world.resource_mut::<GameRng>().0.random();
        assert_eq!(
            after_touched, after_untouched,
            "an off-screen siege must draw nothing from GameRng"
        );
    }
}

/// `components::Besieger`'s citizenship — the third kind of body carrying a
/// base-space `Position`, after a posted program and a `DigSite` (Task 11).
mod raiders_citizenship {
    use super::*;
    use crate::components::{Besieger, Glyph, GlyphColor};

    fn spawn_besieger_at(game: &mut Game, x: i32, y: i32) -> Entity {
        game.world
            .spawn((
                Position { x, y },
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 5,
                    max_hp: 5,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
            ))
            .id()
    }

    /// A besieger's `Position` is a base-space cell, and the two coordinate
    /// spaces alias onto each other by design — `components::Besieger`'s own
    /// doc. Without `Game::stands_in_base_space`'s arm this reads as a wild
    /// `Creature` standing on the zone surface at the same numbers.
    #[test]
    fn a_besieger_draws_on_the_base_map_and_not_the_surface() {
        let mut game = Game::new(970, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.lay_starting_pocket();
        let besieger = spawn_besieger_at(&mut game, 2, 0);

        let surface: Vec<Entity> = game
            .view_entities(20, 20)
            .into_iter()
            .map(|v| v.entity)
            .collect();
        assert!(
            !surface.contains(&besieger),
            "a besieger must not draw on the zone surface"
        );

        stand_in_base_at(&mut game, 0, 0);
        let base: Vec<Entity> = game
            .view_entities(20, 20)
            .into_iter()
            .map(|v| v.entity)
            .collect();
        assert!(
            base.contains(&besieger),
            "a besieger must draw on the base map"
        );
    }

    /// The wild-population systems must not count or cull a besieger —
    /// `local_hostile_count` (read by both `maybe_spawn_wild_creature` and
    /// `populate_chunk`/`ensure_local_population`) and `cull_to_cap`.
    #[test]
    fn a_besieger_does_not_count_toward_local_wild_population() {
        let mut game = Game::new(971, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let (px, py) = {
            let pos = game.world.get::<Position>(game.player_entity()).unwrap();
            (pos.x, pos.y)
        };
        // The ambient wild spawn around a fresh game is not this test's
        // concern — only the *delta* a siege's pack makes to it is.
        game.clear_local_wild();
        let before = game.local_hostile_count(px, py);
        for _ in 0..3 {
            spawn_besieger_at(&mut game, px, py);
        }
        assert_eq!(
            game.local_hostile_count(px, py),
            before,
            "a besieger must not be counted as local wild population"
        );

        game.cull_to_cap(0);
        assert_eq!(
            game.world
                .query_filtered::<Entity, With<Besieger>>()
                .iter(&game.world)
                .count(),
            3,
            "cull_to_cap must never evict a besieger"
        );
    }

    /// `position_is_honest` reads `true` for anything that isn't `Tamed`,
    /// which a besieger never is — still passing is the whole assertion.
    #[test]
    fn a_besiegers_position_is_still_honest() {
        let mut game = Game::new(972, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let besieger = spawn_besieger_at(&mut game, 5, 5);
        assert!(game.position_is_honest(besieger));
    }
}
