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

    /// Every raider `open_siege` seats is a `Besieger` — the marker steal,
    /// wreck, withdraw and the morale count all gate on. The behaviour
    /// tests hand-insert it, so only a pack spawned through the real door
    /// can catch it missing, and a pack without it reads as already
    /// broken from the first turn.
    #[test]
    fn the_pack_open_siege_seats_is_marked_besieger() {
        let mut game = Game::new(953, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);

        assert!(game.open_siege());

        let raiders: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .filter(|&e| game.world.get::<Hostile>(e).is_some())
            .collect();
        assert!(!raiders.is_empty());
        for e in raiders {
            assert!(
                game.world.get::<crate::components::Besieger>(e).is_some(),
                "a seated raider must carry Besieger"
            );
        }
        assert!(
            !game.siege_morale_broken(),
            "a fresh pack must not read as broken"
        );
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

/// `Game::besieger_turn`'s steal arm (Task 12).
mod raiders_steal {
    use super::*;
    use crate::components::{Besieger, Carrying, Glyph, GlyphColor, StolenFrom, Structure};
    use crate::game::siege::board;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::BattleSpec;
    use crate::tuning::HAUL_CARRY_CAPACITY;
    use crate::world::Biome;

    /// A real base pocket, opened as a `TacticalBattle` through the exact
    /// board a siege would build. `besieger_turn` re-derives the door the
    /// same way (`board::build`), so this fixture and the code under test
    /// always agree about where it is. Returns the door's board cell.
    fn open_pocket_battle(game: &mut Game) -> (i32, i32) {
        game.lay_starting_pocket();
        let siege_board = board::build(game).unwrap();
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 1,
        };
        let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
        // `Game::besieger_turn` reads `battle.siege_door` rather than
        // rebuilding the board every beat (a dig finishing mid-siege must
        // not move where the door is read as standing) — this fixture has
        // to carry the same value `Game::open_siege` would have written, or
        // every besieger under test reads a phantom door at `(0, 0)`.
        battle.siege_door = siege_board.door;
        // Seated at the door so `Game::settle_tactical` does not read the
        // fight as a jack-out the moment a besieger's own turn wraps the
        // round (`reap_tactical_dead`'s unconditional `settle_tactical`
        // call) — a fixture that never seats the player at all reads
        // `battle.cell_of(player)` as `None`, which is the same "gone" a
        // real jack-out leaves.
        battle.place(game.player_entity(), siege_board.door);
        game.world.insert_resource(battle);
        siege_board.door
    }

    fn spawn_besieger(game: &mut Game) -> Entity {
        game.world
            .spawn((
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
            ))
            .id()
    }

    fn spawn_stocked_structure(game: &mut Game, qty: u32) -> Entity {
        let mut stock = Stock::new(1000);
        stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), qty);
        game.world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                stock,
            ))
            .id()
    }

    fn core_fragment() -> ItemId {
        ItemId::from(ids::CORE_FRAGMENT)
    }

    /// Adjacent to a stocked structure, a besieger steals and makes for the
    /// door, over as many beats as the whole trip takes — and once through
    /// it, the roster is empty and the fight reads as won, the same
    /// `settle_tactical` answer a wipe gives.
    #[test]
    fn a_raider_beside_a_stocked_structure_steals_and_departs() {
        let mut game = Game::new(980, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_stocked_structure(&mut game, 3);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (2, 0));
            battle.place(besieger, (3, 0));
            battle.set_initiative(vec![besieger]);
        }

        assert!(
            game.besieger_turn(besieger),
            "adjacent to a stocked structure, a besieger must steal"
        );
        assert!(
            game.world.get::<Carrying>(besieger).is_some(),
            "it must now be carrying"
        );
        let after_steal = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert!(after_steal < 3, "stealing must take units off the shelf");

        for _ in 0..20 {
            if game.world.get_resource::<TacticalBattle>().is_none() {
                break;
            }
            game.besieger_turn(besieger);
        }

        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "a lone besieger that reached the door and left empties the \
             roster, which the fight reads as won"
        );
    }

    /// A raider killed while carrying drops what it held back into the
    /// structure it came from, if that still stands — the base's total
    /// stock unchanged across the whole episode.
    #[test]
    fn a_raider_killed_one_cell_short_of_the_door_drops_what_it_held() {
        let mut game = Game::new(981, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let door = open_pocket_battle(&mut game);
        let structure = spawn_stocked_structure(&mut game, 2);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (2, 0));
            // One cell short of the door, already carrying — the steal
            // itself is Task 12's other test's concern.
            battle.place(besieger, (door.0 + 1, door.1));
            battle.set_initiative(vec![besieger]);
        }
        game.world.entity_mut(besieger).insert((
            Carrying {
                item: core_fragment(),
                qty: 3,
            },
            StolenFrom(structure),
        ));
        let total_before = 2 /* on the shelf */ + 3 /* carried */;

        game.world.get_mut::<Stats>(besieger).unwrap().hp = 0;
        let round_before = game.world.resource::<TacticalBattle>().round;
        game.world
            .resource_mut::<TacticalBattle>()
            .set_actions_left(0);
        game.hand_on_turn(besieger, round_before);

        let total_after = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert_eq!(
            total_after, total_before,
            "the base's total stock must be unchanged across the whole episode"
        );
    }

    /// A raider with nothing in reach to take does not stand still — it
    /// falls through to the generic AI.
    #[test]
    fn a_raider_with_nothing_to_take_falls_through_to_the_generic_ai() {
        let mut game = Game::new(982, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(besieger, (1, 1));
            battle.set_initiative(vec![besieger]);
        }

        assert!(
            !game.besieger_turn(besieger),
            "nothing adjacent to take must fall through rather than act"
        );
    }

    /// The hook actually reaches real gameplay's pacing door,
    /// `Game::tactical_ai_beat` — not just `Game::besieger_turn` called
    /// directly, which every other test in this module does. Without the
    /// `run_tactical_beat` hook a besieger would fight like an ordinary
    /// hostile in real play and only steal under a test that drives its
    /// turn by hand.
    #[test]
    fn the_pacing_driver_reaches_the_steal_hook() {
        let mut game = Game::new(984, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_stocked_structure(&mut game, 3);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (2, 0));
            battle.place(besieger, (3, 0));
            battle.set_initiative(vec![besieger]);
        }

        use crate::tactical::ai::AiBeat;
        // `Stepped`, not `Acted`: this besieger is the fight's only
        // initiative slot, so spending its one action wraps the round
        // straight back onto itself (`TacticalBattle::end_turn`'s `wrap`)
        // rather than handing off to anybody else — `battle.actor()` still
        // names it once the beat returns.
        assert_eq!(game.tactical_ai_beat(), AiBeat::Stepped);
        assert!(
            game.world.get::<Carrying>(besieger).is_some(),
            "the real per-frame pacing door must reach the steal hook too"
        );
    }

    /// The carry cap bounds one raider's haul, against the shared constant
    /// rather than a literal.
    #[test]
    fn the_carry_cap_bounds_one_raiders_haul() {
        let mut game = Game::new(983, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let plenty = HAUL_CARRY_CAPACITY + 5;
        let structure = spawn_stocked_structure(&mut game, plenty);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (2, 0));
            battle.place(besieger, (3, 0));
            battle.set_initiative(vec![besieger]);
        }

        assert!(game.besieger_turn(besieger));

        let carrying = game.world.get::<Carrying>(besieger).unwrap();
        assert_eq!(carrying.qty, HAUL_CARRY_CAPACITY);
        let remaining = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert_eq!(remaining, plenty - HAUL_CARRY_CAPACITY);
    }
}

/// `Game::besieger_turn`'s wreck arm, below the steal arm (Task 13).
mod raiders_wreck {
    use super::*;
    use crate::components::{Besieger, Carrying, Durability, Glyph, GlyphColor, Structure};
    use crate::game::siege::board;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::BattleSpec;
    use crate::world::Biome;

    fn open_pocket_battle(game: &mut Game) -> (i32, i32) {
        game.lay_starting_pocket();
        let siege_board = board::build(game).unwrap();
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 1,
        };
        let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
        // `Game::besieger_turn` reads `battle.siege_door` rather than
        // rebuilding the board every beat (a dig finishing mid-siege must
        // not move where the door is read as standing) — this fixture has
        // to carry the same value `Game::open_siege` would have written, or
        // every besieger under test reads a phantom door at `(0, 0)`.
        battle.siege_door = siege_board.door;
        // Seated at the door so `Game::settle_tactical` does not read the
        // fight as a jack-out the moment a besieger's own turn wraps the
        // round (`reap_tactical_dead`'s unconditional `settle_tactical`
        // call) — a fixture that never seats the player at all reads
        // `battle.cell_of(player)` as `None`, which is the same "gone" a
        // real jack-out leaves.
        battle.place(game.player_entity(), siege_board.door);
        game.world.insert_resource(battle);
        siege_board.door
    }

    fn spawn_besieger(game: &mut Game) -> Entity {
        game.world
            .spawn((
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 5,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
            ))
            .id()
    }

    fn spawn_structure(game: &mut Game, hp: u32, stocked: u32) -> Entity {
        let mut stock = Stock::new(1000);
        if stocked > 0 {
            stock
                .output
                .insert(ItemId::from(ids::CORE_FRAGMENT), stocked);
        }
        game.world
            .spawn((
                Structure {
                    kind: "test_structure".to_string(),
                },
                Durability { hp, max_hp: hp },
                stock,
            ))
            .id()
    }

    /// A raider beside an empty machine attacks it.
    #[test]
    fn a_raider_beside_an_empty_machine_wrecks_it() {
        let mut game = Game::new(990, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_structure(&mut game, 10_000, 0);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (2, 0));
            battle.place(besieger, (3, 0));
            battle.set_initiative(vec![besieger]);
        }

        assert!(
            game.besieger_turn(besieger),
            "beside an empty machine, a besieger must wreck it"
        );
        let hp = game.world.get::<Durability>(structure).unwrap().hp;
        assert!(hp < 10_000, "wrecking must lower the machine's Durability");
        assert!(
            game.world.get::<Carrying>(besieger).is_none(),
            "wrecking is not stealing"
        );
    }

    /// A raider beside a stocked one steals instead — steal before wreck.
    #[test]
    fn a_raider_beside_a_stocked_machine_steals_instead() {
        let mut game = Game::new(991, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_structure(&mut game, 10_000, 3);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (2, 0));
            battle.place(besieger, (3, 0));
            battle.set_initiative(vec![besieger]);
        }

        assert!(game.besieger_turn(besieger));
        assert!(
            game.world.get::<Carrying>(besieger).is_some(),
            "a stocked machine must be stolen from rather than wrecked"
        );
        assert_eq!(
            game.world.get::<Durability>(structure).unwrap().hp,
            10_000,
            "a machine that was stolen from rather than wrecked keeps its Durability"
        );
    }

    /// A base with nothing worth taking anywhere is still wrecked rather
    /// than stood in.
    #[test]
    fn a_base_with_nothing_worth_taking_is_still_wrecked_rather_than_stood_in() {
        let mut game = Game::new(992, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_pocket_battle(&mut game);
        let structure = spawn_structure(&mut game, 10_000, 0);
        let besieger = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (2, 0));
            battle.place(besieger, (3, 0));
            battle.set_initiative(vec![besieger]);
        }

        let acted = game.besieger_turn(besieger);
        assert!(
            acted,
            "with nothing to take anywhere in reach, a besieger must still act"
        );
    }
}

/// The morale break: `SIEGE_MORALE_BREAK_PERCENT` of the pack down sends
/// the rest to the door unconditionally (Task 14).
mod raiders_withdraw {
    use super::*;
    use crate::components::{Besieger, Carrying, Glyph, GlyphColor, StolenFrom, Structure};
    use crate::game::siege::board;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::BattleSpec;
    use crate::world::Biome;

    /// A real base pocket, opened as a `TacticalBattle` through the exact
    /// board a siege would build, with `TacticalBattle::siege_pack` set to
    /// `pack` — `Game::open_siege`'s own write, done by hand here so a
    /// fixture can pick a pack size independent of how many bodies it
    /// actually seats.
    fn open_pocket_battle(game: &mut Game, pack: u32) -> (i32, i32) {
        game.lay_starting_pocket();
        let siege_board = board::build(game).unwrap();
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 1,
        };
        let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
        battle.siege_pack = pack;
        // See the other `open_pocket_battle`s: `Game::besieger_turn` reads
        // `battle.siege_door` rather than rebuilding the board every beat.
        battle.siege_door = siege_board.door;
        // Seated at the door so `Game::settle_tactical` does not read the
        // fight as a jack-out the moment a besieger's own turn wraps the
        // round (`reap_tactical_dead`'s unconditional `settle_tactical`
        // call) — a fixture that never seats the player at all reads
        // `battle.cell_of(player)` as `None`, which is the same "gone" a
        // real jack-out leaves.
        battle.place(game.player_entity(), siege_board.door);
        game.world.insert_resource(battle);
        siege_board.door
    }

    fn spawn_besieger(game: &mut Game) -> Entity {
        game.world
            .spawn((
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
            ))
            .id()
    }

    fn chebyshev(a: (i32, i32), b: (i32, i32)) -> i32 {
        (a.0 - b.0).abs().max((a.1 - b.1).abs())
    }

    /// Downing half the pack sends the rest to the door — asserted on a
    /// survivor's cell moving doorward.
    #[test]
    fn downing_half_the_pack_sends_survivors_doorward() {
        let mut game = Game::new(993, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pack: u32 = 5;
        let door = open_pocket_battle(&mut game, pack);
        let besiegers: Vec<Entity> = (0..pack).map(|_| spawn_besieger(&mut game)).collect();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            for (i, &b) in besiegers.iter().enumerate() {
                // `(2, i)`, not `(3, i)`: the survivor this test reads
                // (index `down`) must start more than `TACTICAL_MELEE_RANGE`
                // from the door, or the fixed adjacency-based leave check
                // (C1) reads it as already having reached the door and
                // despawns it on this very call — which is correct behaviour
                // for that case, but leaves nothing here to assert the
                // "moves doorward" claim against.
                battle.place(b, (2, i as i32));
            }
            battle.set_initiative(besiegers.clone());
        }
        let down = pack.div_ceil(2);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            for &b in besiegers.iter().take(down as usize) {
                battle.remove(b);
            }
        }
        let survivor = besiegers[down as usize];
        let before = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(survivor)
            .unwrap();
        assert!(
            chebyshev(before, door) > 1,
            "the fixture must place the survivor away from the door"
        );

        assert!(
            game.besieger_turn(survivor),
            "a broken pack's door arm must be unconditional"
        );

        let after = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(survivor)
            .unwrap();
        assert!(
            chebyshev(after, door) < chebyshev(before, door),
            "a withdrawing survivor must move doorward: {before:?} -> {after:?}, door {door:?}"
        );
    }

    /// Below the threshold, nobody withdraws — a survivor with nothing in
    /// reach to take or wreck falls through instead of heading for the
    /// door.
    #[test]
    fn below_the_threshold_nobody_withdraws() {
        let mut game = Game::new(994, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pack: u32 = 5;
        let door = open_pocket_battle(&mut game, pack);
        let besiegers: Vec<Entity> = (0..pack).map(|_| spawn_besieger(&mut game)).collect();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            for (i, &b) in besiegers.iter().enumerate() {
                // Spread along a line inside the starting pocket, away from
                // the door and with nothing else adjacent to any of them.
                battle.place(b, (2, i as i32));
            }
            battle.set_initiative(besiegers.clone());
        }
        // Down exactly one — under half of any pack of at least 4.
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.remove(besiegers[0]);
        }
        let survivor = besiegers[1];
        let before = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(survivor)
            .unwrap();
        assert!(
            chebyshev(before, door) > 1,
            "the fixture must place the survivor away from the door"
        );

        assert!(
            !game.besieger_turn(survivor),
            "below the morale-break threshold a besieger with nothing to \
             take must fall through rather than make for the door"
        );
    }

    /// A pack that fully withdraws ends the fight as a win, through the
    /// same verdict a wipe produces — and the withdrawn raiders' cargo is
    /// gone from the base.
    #[test]
    fn a_fully_withdrawn_pack_ends_the_fight_as_a_win_and_keeps_its_plunder() {
        let mut game = Game::new(995, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = {
            let mut stock = Stock::new(1000);
            // Represents what is left on the shelf after the carrier's own
            // steal already happened — this fixture attaches `Carrying`
            // directly rather than replaying `besieger_steal`, so the 5
            // units it holds are not double-counted here.
            stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), 5);
            game.world
                .spawn((
                    Structure {
                        kind: "test_structure".to_string(),
                    },
                    stock,
                ))
                .id()
        };
        // A pack of 5 with only two seated on this board — `siege_pack` is
        // set by hand precisely so a fixture can pick a size independent of
        // what it seats, and 2 is already under half of 5, so both
        // survivors read the pack as broken from their very first beat.
        let door = open_pocket_battle(&mut game, 5);
        let carrier = spawn_besieger(&mut game);
        let bare = spawn_besieger(&mut game);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(structure, (5, 5));
            battle.place(carrier, (door.0 + 1, door.1));
            battle.place(bare, (door.0 + 2, door.1));
            battle.set_initiative(vec![carrier, bare]);
        }
        game.world.entity_mut(carrier).insert((
            Carrying {
                item: ItemId::from(ids::CORE_FRAGMENT),
                qty: 5,
            },
            StolenFrom(structure),
        ));

        for _ in 0..20 {
            if game.world.get_resource::<TacticalBattle>().is_none() {
                break;
            }
            game.besieger_turn(carrier);
            if game.world.get_resource::<TacticalBattle>().is_some() {
                game.besieger_turn(bare);
            }
        }

        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "a fully withdrawn pack empties the roster, the same verdict a wipe gives"
        );
        let remaining = game
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0);
        assert_eq!(
            remaining, 5,
            "the carrier's plunder must stay gone — it escaped, it was not killed"
        );
    }
}

/// `Game::fire_turrets` (Task 15).
mod turrets_fire {
    use super::*;
    use crate::structures::TurretDef;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::{BattleCell, BattleSpec, Board};
    use crate::world::Biome;

    fn open_fight(game: &mut Game, side: i32) {
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
    }

    fn spawn_turret(game: &mut Game) -> Entity {
        game.world
            .spawn(Structure {
                kind: "turret".to_string(),
            })
            .id()
    }

    fn spawn_hostile(game: &mut Game, hp: i32) -> Entity {
        game.world
            .spawn((
                Hostile,
                Stats {
                    hp,
                    max_hp: hp,
                    atk: 0,
                    mitigation: 0,
                },
            ))
            .id()
    }

    fn turret_def(game: &Game) -> TurretDef {
        game.structure_defs()
            .into_iter()
            .find(|d| d.id == "turret")
            .expect("turret.ron should load as a structure")
            .turret
            .expect("turret.ron should declare a turret")
    }

    /// A turret in range with line of sight damages the nearest hostile at
    /// the round's start, and the initiative order's length does not
    /// change — a turret holds no slot to begin with.
    #[test]
    fn a_turret_in_range_damages_the_nearest_hostile() {
        let mut game = Game::new(996, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let def = turret_def(&game);
        let turret = spawn_turret(&mut game);
        let hostile = spawn_hostile(&mut game, 10_000);
        let player = game.player_entity();
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(hostile, (def.range as i32, 0));
            battle.place(player, (10, 10));
            battle.set_initiative(vec![player, hostile]);
        }
        let before: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();

        game.fire_turrets();

        let hp = game.world.get::<Stats>(hostile).unwrap().hp;
        assert!(hp < 10_000, "a turret in range must damage its target");
        let after: Vec<Entity> = game
            .world
            .resource::<TacticalBattle>()
            .initiative()
            .to_vec();
        assert_eq!(
            before, after,
            "a turret holds no initiative slot and must not add one"
        );
    }

    /// A turret with no line of sight to the only hostile fires at nothing.
    #[test]
    fn a_turret_with_no_line_of_sight_fires_at_nothing() {
        let mut game = Game::new(997, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let turret = spawn_turret(&mut game);
        let hostile = spawn_hostile(&mut game, 10_000);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(hostile, (2, 0));
            // `Cover` is the only `BattleCell` kind that blocks sight —
            // `Blocked` is a chasm you can see over and cannot cross.
            battle.board.set(1, 0, BattleCell::Cover);
            battle.set_initiative(vec![hostile]);
        }

        game.fire_turrets();

        let hp = game.world.get::<Stats>(hostile).unwrap().hp;
        assert_eq!(
            hp, 10_000,
            "a turret with no line of sight must not hit blind"
        );
    }

    /// A turret out of range fires at nothing.
    #[test]
    fn a_turret_out_of_range_fires_at_nothing() {
        let mut game = Game::new(998, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let def = turret_def(&game);
        let turret = spawn_turret(&mut game);
        let hostile = spawn_hostile(&mut game, 10_000);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(hostile, (def.range as i32 + 1, 0));
            battle.set_initiative(vec![hostile]);
        }

        game.fire_turrets();

        let hp = game.world.get::<Stats>(hostile).unwrap().hp;
        assert_eq!(
            hp, 10_000,
            "a turret out of range must not reach its target"
        );
    }

    /// Two hostiles at different distances: the nearer is hit.
    #[test]
    fn the_nearer_of_two_hostiles_is_hit() {
        let mut game = Game::new(999, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let turret = spawn_turret(&mut game);
        let near = spawn_hostile(&mut game, 10_000);
        let far = spawn_hostile(&mut game, 10_000);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(near, (1, 0));
            battle.place(far, (3, 0));
            battle.set_initiative(vec![near, far]);
        }

        game.fire_turrets();

        assert!(
            game.world.get::<Stats>(near).unwrap().hp < 10_000,
            "the nearer hostile must be hit"
        );
        assert_eq!(
            game.world.get::<Stats>(far).unwrap().hp,
            10_000,
            "the farther hostile must be untouched"
        );
    }

    /// A destroyed turret stops firing.
    #[test]
    fn a_destroyed_turret_stops_firing() {
        let mut game = Game::new(1000, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_fight(&mut game, 20);
        let turret = spawn_turret(&mut game);
        let hostile = spawn_hostile(&mut game, 10_000);
        {
            let mut battle = game.world.resource_mut::<TacticalBattle>();
            battle.place(turret, (0, 0));
            battle.place(hostile, (1, 0));
            battle.set_initiative(vec![hostile]);
            // Destroyed: removed from the board and despawned, the way
            // `damage_structure`'s own teardown leaves one.
            battle.remove(turret);
        }
        game.world.despawn(turret);

        game.fire_turrets();

        assert_eq!(
            game.world.get::<Stats>(hostile).unwrap().hp,
            10_000,
            "a destroyed turret must not fire"
        );
    }
}

/// `save::SiegeSave` (Task 16) — a siege in progress persists across a real
/// save/load round trip, assembled from the board and the bodies rather
/// than by serialising `TacticalBattle` itself.
mod persist {
    use super::*;
    use crate::components::{Besieger, Carrying, Glyph, GlyphColor, StolenFrom, Structure};
    use crate::game::siege::board;
    use crate::items::ids;
    use crate::tactical::TacticalBattle;
    use crate::tactical::map::{BattleSpec, Board};
    use crate::world::Biome;

    fn core_fragment() -> ItemId {
        ItemId::from(ids::CORE_FRAGMENT)
    }

    /// The geometry a fixture's caller needs back, since a reload mints
    /// fresh entities and only the cells are still comparable.
    struct Fixture {
        board: Board,
        door_cell: (i32, i32),
        staff_cell: (i32, i32),
        structure_cell: (i32, i32),
        besieger_cell: (i32, i32),
    }

    /// Every kind of body a siege's `TacticalBattle` can hold — the player,
    /// one staff program, one real structure (a Depot, so it survives
    /// `restore_structures`' own `StructureDb` lookup rather than being
    /// dropped as an unrecognised kind), and one besieger carrying stolen
    /// goods from it — seated on the real flood-filled board rather than a
    /// hand-drawn one, so `SiegeSave::origin`/`door` have real geometry to
    /// round-trip. Leaves the fight one turn into round 5, on the
    /// besieger's own action budget, so `round`/`turn`/`actions_left` are
    /// none of them a coincidental default.
    fn open_siege_fixture(game: &mut Game) -> Fixture {
        game.lay_starting_pocket();
        stand_in_base_at(game, 0, 0);
        let siege_board = board::build(game).unwrap();

        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "depot")
            .expect("the shipped catalogue has a Depot");
        let structure = game.spawn_structure(&def, 2, 0, None);
        game.world
            .get_mut::<Stock>(structure)
            .unwrap()
            .output
            .insert(core_fragment(), 2);

        let staff = spawn_tamed(game, 10, 3);
        {
            let mut pos = game.world.get_mut::<Position>(staff).unwrap();
            pos.x = 1;
            pos.y = 0;
        }

        let besieger = game
            .world
            .spawn((
                Creature {
                    species: GENERIC_SPECIES_ID.to_string(),
                },
                Position { x: 3, y: 0 },
                Glyph {
                    ch: 'r',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
                Besieger,
                Carrying {
                    item: core_fragment(),
                    qty: 3,
                },
                StolenFrom(structure),
            ))
            .id();

        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 2,
            biome: Biome::OpenGrid,
            bodies: 4,
        };
        let mut battle = TacticalBattle::open(spec, siege_board.board.clone());
        battle.siege_origin = siege_board.origin;
        battle.siege_door = siege_board.door;

        let door_cell = siege_board.door;
        let staff_cell = siege_board.to_board((1, 0)).unwrap();
        let structure_cell = siege_board.to_board((2, 0)).unwrap();
        let besieger_cell = siege_board.to_board((3, 0)).unwrap();

        battle.place(structure, structure_cell);
        battle.place(staff, staff_cell);
        let player = game.player_entity();
        battle.place(player, door_cell);
        battle.place(besieger, besieger_cell);

        battle.siege_pack = 1;
        // Fastest to slowest: the player, then staff, then the besieger —
        // and one turn spent moves the actor onto staff, which is where
        // the round/turn/actions_left assertions below need it.
        battle.set_initiative(vec![player, staff, besieger]);
        battle.end_turn();
        battle.round = 5;
        battle.set_actions_left(2);

        let board = battle.board.clone();
        game.world.insert_resource(battle);

        Fixture {
            board,
            door_cell,
            staff_cell,
            structure_cell,
            besieger_cell,
        }
    }

    /// Finds the one entity of a marker component a fresh load must have
    /// reseeded — the round trip mints new `Entity` ids, so a body from
    /// before the reload can never be compared against one from after it.
    fn the<T: bevy_ecs::prelude::Component>(game: &mut Game) -> Entity {
        game.world
            .query_filtered::<Entity, With<T>>()
            .iter(&game.world)
            .next()
            .unwrap_or_else(|| {
                panic!(
                    "exactly one {} must have reloaded",
                    std::any::type_name::<T>()
                )
            })
    }

    /// **The main test.** A siege's board, every body's own cell, the
    /// initiative order and the round/turn/actions_left it was interrupted
    /// on all survive a real save→load round trip, and the fight can be
    /// continued afterward.
    #[test]
    fn a_siege_in_progress_survives_a_real_save_load_round_trip() {
        let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let fixture = open_siege_fixture(&mut game);

        let scratch = scratch_assets_dir("siege_persist_roundtrip");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();

        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

        assert!(
            loaded.in_tactical_battle(),
            "the siege must still be open after the load"
        );

        let player = loaded.player_entity();
        let staff = the::<Tamed>(&mut loaded);
        let structure = the::<Structure>(&mut loaded);
        let besieger = the::<Besieger>(&mut loaded);

        {
            let battle = loaded.world.resource::<TacticalBattle>();
            assert_eq!(battle.board, fixture.board, "the board's own cells");
            assert_eq!(battle.siege_pack, 1, "siege_pack");
            assert_eq!(battle.round, 5, "round");
            assert_eq!(battle.actions_left(), 2, "actions_left");

            assert_eq!(
                battle.cell_of(player),
                Some(fixture.door_cell),
                "player cell"
            );
            assert_eq!(
                battle.cell_of(staff),
                Some(fixture.staff_cell),
                "staff cell"
            );
            assert_eq!(
                battle.cell_of(structure),
                Some(fixture.structure_cell),
                "structure cell"
            );
            assert_eq!(
                battle.cell_of(besieger),
                Some(fixture.besieger_cell),
                "besieger cell"
            );

            assert_eq!(
                battle.initiative(),
                &[player, staff, besieger],
                "the initiative order, fastest to slowest"
            );
            assert_eq!(
                battle.actor(),
                Some(staff),
                "the acting body must survive along with everything else"
            );
        }

        // The fight can be continued: ending the acting body's turn moves
        // the cursor on exactly as it would have before the reload.
        loaded.tactical_end_turn();
        assert_eq!(
            loaded.world.resource::<TacticalBattle>().actor(),
            Some(besieger),
            "the fight must still take real turns after the load"
        );
    }

    /// A save written before this feature existed carries no `siege` key at
    /// all, and must load with no siege in progress rather than refusing or
    /// panicking — `#[serde(default)]`'s whole compatibility story.
    #[test]
    fn a_pre_siege_save_loads_with_no_siege_in_progress() {
        let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_siege_fixture(&mut game);

        let scratch = scratch_assets_dir("siege_persist_pre_siege");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();

        // Stripped to what a save written before sieges existed looked
        // like — the real save's own `siege` key, removed, rather than a
        // hand-built RON fixture that only proves the parser accepts an
        // absent field. Whole *block*, not the one line a naive filter
        // drops: pretty RON breaks `Some((...))` across many lines, and a
        // line filter leaves the tail behind as a parse error —
        // `settlement_boards::a_save_from_before_town_jobs_loads_its_
        // contracts_as_the_brokers`'s own depth-tracked pattern.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            raw.contains("siege: Some("),
            "the fixture must open a real siege, or this proves nothing"
        );
        let mut older = String::new();
        let mut depth = 0usize;
        for line in raw.lines() {
            if depth == 0 && !line.trim_start().starts_with("siege:") {
                older.push_str(line);
                older.push('\n');
                continue;
            }
            depth += line.matches('(').count();
            depth -= line.matches(')').count().min(depth);
        }
        assert!(!older.contains("siege:"), "the key has to actually be gone");
        let old_path = scratch.join("old.bin");
        std::fs::write(&old_path, older).unwrap();

        let loaded = Game::load(&old_path, &test_assets_dir()).unwrap();
        assert!(
            !loaded.in_tactical_battle(),
            "a pre-siege save must load with no siege in progress"
        );
    }

    /// The invariant this whole feature is built to hold —
    /// `TacticalBattle` never gains `Serialize`, so an ordinary tactical
    /// fight (one `siege_pack` reads as "not a siege") is still not saved.
    /// This test must fail the moment that changes.
    #[test]
    fn an_ordinary_tactical_fight_is_still_not_saved() {
        let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let hostile = game
            .world
            .spawn((
                Position { x: 1, y: 0 },
                Glyph {
                    ch: 'h',
                    color: GlyphColor::Red,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
                Hostile,
            ))
            .id();
        let spec = BattleSpec {
            world_seed: 1,
            site: (0, 0),
            tick: 0,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 2,
        };
        let mut battle = TacticalBattle::open(spec, Board::from_rows(&["....."; 5]));
        let player = game.player_entity();
        battle.place(player, (0, 0));
        battle.place(hostile, (1, 0));
        battle.set_initiative(vec![player, hostile]);
        assert_eq!(
            battle.siege_pack, 0,
            "an ordinary fight opens no siege pack"
        );
        game.world.insert_resource(battle);

        let scratch = scratch_assets_dir("siege_persist_ordinary_fight");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();

        let data = crate::save::load_from_file(&path).unwrap();
        assert!(
            data.siege.is_none(),
            "an ordinary tactical fight must not be assembled into a save"
        );

        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        assert!(
            loaded.world.get_resource::<TacticalBattle>().is_none(),
            "an ordinary tactical fight must not survive a save/load round trip"
        );
    }

    /// A besieger carrying cargo across a save still drops it — into the
    /// structure it came from, resolved fresh by tile after the reload —
    /// when it is killed after the load.
    #[test]
    fn a_besieger_carrying_cargo_across_a_save_still_drops_it_when_killed_after_the_load() {
        let mut game = Game::new(9004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        open_siege_fixture(&mut game);

        let scratch = scratch_assets_dir("siege_persist_drop_cargo");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();

        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let structure = the::<Structure>(&mut loaded);
        let besieger = the::<Besieger>(&mut loaded);

        assert_eq!(
            loaded.world.get::<StolenFrom>(besieger).map(|s| s.0),
            Some(structure),
            "StolenFrom must resolve to the reloaded structure, not a stale entity"
        );
        assert_eq!(
            loaded.world.get::<Carrying>(besieger).map(|c| c.qty),
            Some(3),
            "Carrying must round-trip through a wild creature too"
        );
        let before_on_shelf = loaded
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);

        // Two turns: player then staff, landing on the besieger's own turn
        // exactly as the fixture left it before the save.
        loaded.tactical_end_turn();
        assert_eq!(
            loaded.world.resource::<TacticalBattle>().actor(),
            Some(besieger)
        );
        loaded.world.get_mut::<Stats>(besieger).unwrap().hp = 0;
        loaded.tactical_end_turn();

        assert!(
            loaded.world.get_resource::<TacticalBattle>().is_none()
                || loaded
                    .world
                    .resource::<TacticalBattle>()
                    .cell_of(besieger)
                    .is_none(),
            "the reap must have cleared the dead besieger off the board"
        );
        let after_on_shelf = loaded
            .world
            .get::<Stock>(structure)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert_eq!(
            after_on_shelf,
            before_on_shelf + 3,
            "the stolen 3 units must return to the shelf they came from"
        );
    }

    /// This feature adds no schema break — every new field is additive.
    #[test]
    fn save_format_version_is_unchanged() {
        assert_eq!(
            crate::save::SAVE_FORMAT_VERSION,
            32,
            "adding a siege field is additive under field-named RON and must \
             not cost a version bump"
        );
    }
}

/// Review findings (2026-09-22 `feat/siege` review) fixed against real
/// doors: `Game::open_siege` on a base with a real Home spawned via
/// `place_home` at `BASE_EXIT_CELL`, `Game::tactical_ai_beat` for AI turns,
/// and a real `game.save()`/`Game::load()` round trip for the save fix.
mod review_findings {
    use super::*;
    use crate::base_grid::BaseGrid;
    use crate::components::{Besieger, Carrying, Downed, StolenFrom, Structure};
    use crate::items::ids;
    use crate::tactical::TacticalBattle;
    use crate::tactical::ai::AiBeat;
    use crate::tactical::map::{BattleCell, BattleSpec, Board};
    use crate::tuning::HAUL_CARRY_CAPACITY;
    use crate::world::Biome;

    fn core_fragment() -> ItemId {
        ItemId::from(ids::CORE_FRAGMENT)
    }

    /// A real base — `place_home`'s real `spawn_structure` door at
    /// `BASE_EXIT_CELL` — with the player standing at `player_at` rather
    /// than on the door, and a real `Game::open_siege` fired on it.
    fn open_real_siege(seed: u32, player_at: (i32, i32)) -> Game {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        stand_in_base_at(&mut game, player_at.0, player_at.1);
        set_zone(&mut game, 2);
        assert!(
            game.open_siege(),
            "a real base with the player home must open a real siege"
        );
        game
    }

    fn any_besieger(game: &Game) -> Entity {
        game.world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Besieger>(e).is_some())
            .expect("a real siege must seat at least one besieger")
    }

    /// A free, open neighbour of `near` — raiders seed several deep around
    /// the door (§4's "one body to a cell means they file in at and behind
    /// it"), so a hardcoded offset can land on whichever one got there
    /// first instead of the cell this test actually wants to control.
    fn free_neighbour(battle: &TacticalBattle, near: (i32, i32)) -> (i32, i32) {
        const OFFSETS: [(i32, i32); 8] = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        OFFSETS
            .into_iter()
            .map(|(dx, dy)| (near.0 + dx, near.1 + dy))
            .find(|&cell| {
                battle.board.cell(cell.0, cell.1) == BattleCell::Open
                    && battle.occupant(cell).is_none()
            })
            .expect("the door must have at least one free neighbour on a fresh siege")
    }

    // ---- C1: the Home really occupies the door, and a carrier leaves anyway.

    #[test]
    fn c1_home_stands_on_the_door_and_a_carrier_leaves_through_it_anyway() {
        let mut game = open_real_siege(200_001, (2, 0));
        let (door, home) = {
            let battle = game.world.resource::<TacticalBattle>();
            let door = battle.siege_door;
            let home = battle
                .occupant(door)
                .expect("the Home must be seated as a body on the door cell");
            (door, home)
        };
        assert!(
            game.world.get::<Structure>(home).is_some(),
            "whatever occupies the door must be the Home structure"
        );
        let _ = door;

        let besieger = any_besieger(&game);
        game.world.entity_mut(besieger).insert((
            Carrying {
                item: core_fragment(),
                qty: 1,
            },
            StolenFrom(home),
        ));
        // Unkillable, and the only body with a turn — so the only way this
        // entity can vanish inside the loop below is `besieger_leaves`'s own
        // despawn, not a stray combat death (its own, or the player's, which
        // would close the whole fight and let the C5 sweep account for it
        // instead) standing in for it.
        game.world.get_mut::<Stats>(besieger).unwrap().hp = 1_000_000;
        game.world.get_mut::<Stats>(besieger).unwrap().max_hp = 1_000_000;
        let player = game.player_entity();
        game.world.get_mut::<Stats>(player).unwrap().hp = 1_000_000;
        game.world.get_mut::<Stats>(player).unwrap().max_hp = 1_000_000;
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![besieger]);

        let mut left = false;
        for _ in 0..200 {
            if game.world.get::<Besieger>(besieger).is_none() {
                left = true;
                break;
            }
            game.tactical_ai_beat();
        }
        assert!(
            left,
            "a carrying besieger must eventually reach and leave through \
             the door even though the Home stands on it"
        );
        assert!(
            game.world.get_resource::<TacticalBattle>().is_some(),
            "other besiegers remain seated, so this must be the one leaving \
             through the door, not the whole fight closing some other way"
        );
    }

    // ---- C2: a carrier does not steal twice from the shelf it is still
    // standing beside.

    #[test]
    fn c2_a_carrier_does_not_overwrite_its_load_with_a_second_steal() {
        let mut game = Game::new(200_002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        let depot_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "depot")
            .expect("the shipped catalogue has a Depot");
        // Well clear of the door and the Home standing on it — `(-1, 0)`
        // is itself the door's own neighbour and this fixture needs a free
        // cell beside the Depot, not one the Home already occupies.
        let depot = game.spawn_structure(&depot_def, 2, 2, None);
        let hoard = 2 * HAUL_CARRY_CAPACITY + 1;
        game.world
            .get_mut::<Stock>(depot)
            .unwrap()
            .output
            .insert(core_fragment(), hoard);
        stand_in_base_at(&mut game, -2, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());

        let besieger = any_besieger(&game);
        let depot_cell = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(depot)
            .expect("the Depot must have seated as a body");
        let beside = (depot_cell.0 - 1, depot_cell.1);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(besieger, beside),
            "the fixture must actually free the cell it seats the besieger on"
        );
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![besieger]);

        assert!(
            game.besieger_turn(besieger),
            "adjacent to a stocked Depot, it must steal"
        );
        // Its own turn only steals — it never walks away on the same turn
        // it filled its hold — so it is still standing beside the shelf for
        // the beat this repro is about.
        assert_eq!(
            game.world.resource::<TacticalBattle>().cell_of(besieger),
            Some(beside),
            "stealing must not move the besieger"
        );

        game.besieger_turn(besieger);

        let remaining = game
            .world
            .get::<Stock>(depot)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert_eq!(
            remaining,
            hoard - HAUL_CARRY_CAPACITY,
            "a second steal from the same shelf must not fire while still carrying"
        );
    }

    // ---- C3: a save/load round trip seats the player at their own board
    // cell, not the door.

    #[test]
    fn c3_a_save_load_round_trip_seats_the_player_off_the_door() {
        let mut game = open_real_siege(200_003, (2, 0));
        let (expected_cell, door) = {
            let battle = game.world.resource::<TacticalBattle>();
            (
                battle
                    .cell_of(game.player_entity())
                    .expect("the player must be seated"),
                battle.siege_door,
            )
        };
        assert_ne!(
            expected_cell, door,
            "the fixture must actually stand the player off the door"
        );

        let scratch = scratch_assets_dir("siege_c3_player_cell");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();

        assert!(
            loaded.in_tactical_battle(),
            "the siege must survive the round trip"
        );
        let player = loaded.player_entity();
        let cell = loaded.world.resource::<TacticalBattle>().cell_of(player);
        assert_eq!(
            cell,
            Some(expected_cell),
            "the player must reload at their own board cell, not the door \
             or a base-space anchor coordinate"
        );
    }

    // ---- C4: a staff body killed on the board is benched, not left dead
    // in the roster.

    #[test]
    fn c4_staff_killed_on_the_board_is_benched_not_stuck_at_zero_hp() {
        let mut game = Game::new(200_004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        let staff = spawn_tamed(&mut game, 10, 3);
        {
            let mut pos = game.world.get_mut::<Position>(staff).unwrap();
            pos.x = 2;
            pos.y = 0;
        }
        stand_in_base_at(&mut game, 3, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());
        assert!(
            game.world
                .resource::<TacticalBattle>()
                .cell_of(staff)
                .is_some(),
            "the staff body must have seated on the real board"
        );

        game.world.get_mut::<Stats>(staff).unwrap().hp = 0;
        // A round wrap through the real door — `Game::tactical_end_turn` —
        // rather than a direct call into the private reap, so this proves
        // the production turn-ending path actually reaches it.
        let player = game.player_entity();
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![player]);
        game.tactical_end_turn();

        assert_eq!(
            game.world.get::<Stats>(staff).map(|s| s.hp),
            Some(1),
            "a Forgiving bench leaves the body at 1 HP, not stuck at 0"
        );
        assert!(
            game.world.get::<Downed>(staff).is_some(),
            "the body must be benched (Downed) rather than left dead in the roster"
        );
    }

    // ---- C5: a jack-out sweeps every stray besieger off the board.

    #[test]
    fn c5_a_jack_out_sweeps_the_stray_besiegers() {
        let mut game = open_real_siege(200_005, (2, 0));
        let besiegers: Vec<Entity> = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .filter(|&e| game.world.get::<Besieger>(e).is_some())
                .collect()
        };
        assert!(!besiegers.is_empty());

        // A siege is always tactical (§2 of the design), and a tactical
        // fight has no `BattleState` for `Game::battle_flee` to read — that
        // door is the abstract group model's own, gated on it. On a battle
        // map "the player's own [step off the board] is the jack-out"
        // (`Game::depart_tactical`'s doc), so this walks the player to a
        // free, open cell on the board's true edge and steps off it —
        // scanned rather than hand-picked, since the flood-filled pocket's
        // exact shape (`PLATFORM_CORNER_CUT`) is not this test's to assume.
        let player = game.player_entity();
        let (edge_cell, out_dir) = {
            let battle = game.world.resource::<TacticalBattle>();
            let occupied: std::collections::HashSet<(i32, i32)> =
                battle.bodies().map(|(_, cell)| cell).collect();
            let side = battle.board.side;
            (0..side)
                .flat_map(|y| (0..side).map(move |x| (x, y)))
                .filter(|&(x, y)| battle.board.cell(x, y) == BattleCell::Open)
                .filter(|cell| !occupied.contains(cell))
                .find_map(|(x, y)| {
                    [(-1, 0), (1, 0), (0, -1), (0, 1)]
                        .into_iter()
                        .find(|&(dx, dy)| !battle.board.in_bounds(x + dx, y + dy))
                        .map(|dir| ((x, y), dir))
                })
                .expect("a flood-filled board must touch its own bounding box somewhere")
        };
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, edge_cell),
            "the fixture must be able to stand the player at the board's own edge"
        );
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![player]);

        assert_eq!(
            game.tactical_step(out_dir),
            crate::tactical::turn::StepOutcome::Departed,
            "stepping off the board's own edge must be read as the player's \
             own departure"
        );

        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "the player's own departure must close the fight"
        );
        for &b in &besiegers {
            assert!(
                game.world.get::<Besieger>(b).is_none(),
                "a besieger left standing after the player's departure must \
                 not survive as a stray Hostile wandering the base"
            );
        }
    }

    // ---- I1: base staff act under the AI during a siege.

    #[test]
    fn i1_a_staff_body_acts_under_the_ai_during_a_siege() {
        let mut game = Game::new(200_006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        let staff = spawn_tamed(&mut game, 10, 3);
        {
            let mut pos = game.world.get_mut::<Position>(staff).unwrap();
            pos.x = 2;
            pos.y = 0;
        }
        stand_in_base_at(&mut game, 3, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());
        assert!(
            game.world
                .resource::<TacticalBattle>()
                .cell_of(staff)
                .is_some(),
            "the staff body must have seated on the real board"
        );

        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![staff]);

        assert!(
            !game.tactical_awaits_input(),
            "base staff must act under the AI in a siege, not wait on a \
             key the player never gets to press"
        );
        assert_ne!(
            game.tactical_ai_beat(),
            AiBeat::Idle,
            "the AI must actually drive the staff body's own turn"
        );
    }

    // ---- I2: a swing at a Durability-less structure does not remove it.

    #[test]
    fn i2_a_swing_at_the_home_does_not_delete_it_from_the_board() {
        let mut game = open_real_siege(200_007, (2, 0));
        let (door, home) = {
            let battle = game.world.resource::<TacticalBattle>();
            (
                battle.siege_door,
                battle.occupant(battle.siege_door).unwrap(),
            )
        };
        let besieger = any_besieger(&game);
        let beside = free_neighbour(game.world.resource::<TacticalBattle>(), door);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(besieger, beside),
            "the fixture must actually free the cell beside the door"
        );
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![besieger]);

        assert!(
            game.tactical_attack(home),
            "a besieger adjacent to the Home must be able to swing at it"
        );

        assert!(
            game.world.get::<Structure>(home).is_some(),
            "the Home itself must still exist — it has no Durability to lose"
        );
        assert_eq!(
            game.world.resource::<TacticalBattle>().cell_of(home),
            Some(door),
            "one swing at a Durability-less structure must not remove it \
             from the board"
        );
    }

    // ---- M1: `besieger_turn` reads the frozen `siege_door`, not a fresh
    // rebuild that a mid-siege dig could move.

    #[test]
    fn m1_besieger_turn_reads_the_frozen_door_not_a_live_rebuild() {
        let mut game = open_real_siege(200_008, (2, 0));
        let frozen_door = game.world.resource::<TacticalBattle>().siege_door;

        // Extends the flood fill's bounding box, shifting the origin (and
        // so the board coordinate) a *fresh* `board::build` would compute
        // for the same physical door — the board this fight already opened
        // on does not move with it.
        {
            let mut grid = game.world.resource_mut::<BaseGrid>();
            grid.lay_floor(-5, 0);
        }
        let rebuilt_door = crate::game::siege::board::build(&mut game).unwrap().door;
        assert_ne!(
            rebuilt_door, frozen_door,
            "the fixture must actually shift the origin for this test to mean anything"
        );

        let besieger = any_besieger(&game);
        game.world.entity_mut(besieger).insert((
            Carrying {
                item: core_fragment(),
                qty: 1,
            },
            StolenFrom(besieger),
        ));
        let beside_frozen = free_neighbour(game.world.resource::<TacticalBattle>(), frozen_door);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(besieger, beside_frozen),
            "the fixture must actually free the cell beside the frozen door"
        );
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![besieger]);

        game.besieger_turn(besieger);

        assert!(
            game.world.get::<Besieger>(besieger).is_none(),
            "adjacent to the frozen door while carrying, it must leave — \
             a fresh rebuild would have named a different door and missed it"
        );
    }

    // ---- M2: an empty pack does not open a siege at all.

    #[test]
    fn m2_an_empty_pack_does_not_open_a_siege() {
        let mut game = Game::new(200_009, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        stand_in_base_at(&mut game, 2, 0);
        set_zone(&mut game, 2);

        let (ax, ay) = game
            .anchor_position()
            .expect("Game::new always spawns the one anchor");
        {
            let mut wm = game.world.resource_mut::<WorldMap>();
            let mut tile = wm.tile(ax, ay);
            tile.walkable = false;
            wm.set_override(ax, ay, tile);
        }

        assert!(
            !game.open_siege(),
            "a pack with nowhere to spawn must not open a siege with no \
             hostiles in it"
        );
        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "no board should have opened at all"
        );
    }

    // ---- M4: a besieger leaving on the round's last rung still buys that
    // round's turret fire.

    #[test]
    fn m4_a_besieger_leaving_on_the_last_rung_still_buys_the_rounds_turret_fire() {
        let mut game = Game::new(200_010, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
            bodies: 4,
        };
        let mut battle = TacticalBattle::open(spec, board);
        let door = (0, 0);
        battle.siege_door = door;
        battle.siege_pack = 2;

        let player = game.player_entity();
        let leaving = game
            .world
            .spawn((
                Hostile,
                Besieger,
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    mitigation: 0,
                },
            ))
            .id();
        let target = game
            .world
            .spawn((
                Hostile,
                Besieger,
                Stats {
                    hp: 10_000,
                    max_hp: 10_000,
                    atk: 0,
                    mitigation: 0,
                },
            ))
            .id();
        let turret = game
            .world
            .spawn(Structure {
                kind: "turret".to_string(),
            })
            .id();

        battle.place(player, (9, 9));
        battle.place(leaving, (1, 0));
        battle.place(turret, (2, 2));
        battle.place(target, (2, 7));
        // `target`, then `leaving` on the last rung.
        battle.set_initiative(vec![target, leaving]);
        game.world.insert_resource(battle);
        game.world.entity_mut(leaving).insert((
            Carrying {
                item: core_fragment(),
                qty: 1,
            },
            StolenFrom(leaving),
        ));

        // Advances the cursor onto `leaving` without resolving `target`'s
        // turn through combat — a raw `TacticalBattle` API call to set up
        // the scene, not the mechanism this test is about.
        game.world.resource_mut::<TacticalBattle>().end_turn();
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(leaving),
            "the fixture must put the leaving besieger on the last rung"
        );

        assert!(
            game.besieger_turn(leaving),
            "adjacent to the door while carrying, it must leave"
        );

        let hp = game.world.get::<Stats>(target).unwrap().hp;
        assert!(
            hp < 10_000,
            "the round the leaving besieger's own departure wraps must \
             still fire the turrets, not skip that round's upkeep entirely"
        );
    }

    // ---- M3: a dead carrier's fallback drop goes to a real Depot, never
    // to the Home or any other non-storage structure.

    #[test]
    fn m3_a_dead_carriers_fallback_drop_never_lands_in_the_home() {
        let mut game = Game::new(200_011, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        let depot_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "depot")
            .expect("the shipped catalogue has a Depot");
        let depot = game.spawn_structure(&depot_def, 2, 2, None);
        stand_in_base_at(&mut game, -2, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());

        let besieger = any_besieger(&game);
        // Its source structure is gone (a stale id nothing resolves to any
        // more), so `drop_besieger_cargo` must fall all the way through to
        // its Depot-only fallback rather than the Home, which — like every
        // structure — carries a `Stock` of its own (`Game::spawn_structure`
        // inserts one unconditionally) but is not a place a hauler, or this
        // fallback, may ever put goods.
        let stale_source = game.world.spawn_empty().id();
        game.world.despawn(stale_source);
        game.world.entity_mut(besieger).insert((
            Carrying {
                item: core_fragment(),
                qty: 3,
            },
            StolenFrom(stale_source),
        ));
        game.world.get_mut::<Stats>(besieger).unwrap().hp = 0;

        let player = game.player_entity();
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![player]);
        game.tactical_end_turn();

        let (home_stock, home) = {
            let battle = game.world.resource::<TacticalBattle>();
            let home = battle
                .occupant(battle.siege_door)
                .expect("the Home must still be seated");
            (
                game.world
                    .get::<Stock>(home)
                    .map(|s| s.output_used())
                    .unwrap_or(0),
                home,
            )
        };
        assert_eq!(
            home_stock, 0,
            "the Home's own Stock must never receive a dead carrier's fallback drop"
        );
        let _ = home;
        let depot_stock = game
            .world
            .get::<Stock>(depot)
            .unwrap()
            .output
            .get(&core_fragment())
            .copied()
            .unwrap_or(0);
        assert_eq!(
            depot_stock, 3,
            "the fallback must still land the goods somewhere real — a Depot"
        );
    }
}

/// The 2026-09-22 re-review's own findings, fixed the same way
/// `review_findings` above was: through the real doors —
/// `Game::open_siege` on a base with a real Home, `Game::besieger_turn` /
/// `Game::tactical_ai_beat` for AI turns, the real capture door
/// (`Game::tactical_use_routine` with Decompile), and a real
/// `game.save()`/`Game::load()` round trip where a save is involved.
mod rereview_findings {
    use super::*;
    use crate::components::{Besieger, Carrying, StolenFrom, Tamed};
    use crate::items::ids;
    use crate::tactical::TacticalBattle;
    use crate::tuning::HAUL_CARRY_CAPACITY;

    fn core_fragment() -> ItemId {
        ItemId::from(ids::CORE_FRAGMENT)
    }

    fn any_besieger(game: &Game) -> Entity {
        game.world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(e, _)| e)
            .find(|&e| game.world.get::<Besieger>(e).is_some())
            .expect("a real siege must seat at least one besieger")
    }

    // ---- NEW-1: a carrier does not wreck the shelf it just stole from.

    #[test]
    fn new1_a_carrier_does_not_wreck_the_shelf_it_just_stole_from() {
        let mut game = Game::new(210_001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        let depot_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "depot")
            .expect("the shipped catalogue has a Depot");
        let depot = game.spawn_structure(&depot_def, 2, 2, None);
        let hoard = 2 * HAUL_CARRY_CAPACITY + 1;
        game.world
            .get_mut::<Stock>(depot)
            .unwrap()
            .output
            .insert(core_fragment(), hoard);
        stand_in_base_at(&mut game, -2, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());

        let besieger = any_besieger(&game);
        let depot_cell = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(depot)
            .expect("the Depot must have seated as a body");
        let beside = (depot_cell.0 - 1, depot_cell.1);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(besieger, beside),
            "the fixture must actually free the cell it seats the besieger on"
        );
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![besieger]);

        let durability_before = game
            .world
            .get::<Durability>(depot)
            .expect("the shipped Depot must carry Durability")
            .hp;

        assert!(
            game.besieger_turn(besieger),
            "adjacent to a stocked Depot, it must steal"
        );
        assert!(
            game.world.get::<Carrying>(besieger).is_some(),
            "the fixture must actually leave the besieger carrying something"
        );

        // Second beat: still carrying, still standing beside the same
        // Depot, which still has plenty left on the shelf — this must
        // neither steal again (C2) nor wreck it (NEW-1's own bug).
        game.besieger_turn(besieger);

        let durability_after = game.world.get::<Durability>(depot).unwrap().hp;
        assert_eq!(
            durability_after, durability_before,
            "a carrier making for the door must not wreck the shelf it just \
             stole from"
        );
    }

    // ---- NEW-2: a besieger captured mid-siege survives the fight's end,
    // and a reload.

    #[test]
    fn new2_a_besieger_captured_mid_siege_survives_the_fights_end_and_a_reload() {
        let mut game = Game::new(210_002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        stand_in_base_at(&mut game, 2, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());

        let besieger = any_besieger(&game);
        game.world.get_mut::<Stats>(besieger).unwrap().hp = 1;
        // A besieger mid-haul when it is captured — `decompile_body` must
        // strip this too, not just `Besieger` itself, or a captured
        // companion keeps hauling a shelf's stolen goods around forever.
        let stale_source = game.world.spawn_empty().id();
        game.world.entity_mut(besieger).insert((
            Carrying {
                item: core_fragment(),
                qty: 1,
            },
            StolenFrom(stale_source),
        ));

        let player = game.player_entity();
        crate::tests::tactical::only_routine(&mut game, player, "decompile");
        game.world
            .get_mut::<crate::components::Decompiler>(player)
            .unwrap()
            .skill = 50;
        set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);

        fn tamed_count(game: &mut Game) -> usize {
            let mut q = game.world.query_filtered::<Entity, With<Tamed>>();
            q.iter(&game.world).count()
        }
        let tamed_before = tamed_count(&mut game);

        // `a_capture_on_a_battle_map_turns_the_program_it_was_aimed_at`'s
        // own loop (`tests::tactical`): close the gap on the player's own
        // turn and retry the roll, letting every other turn pass idly
        // through `wait_for_turn` — the besieger's own AI hook is never
        // invoked, so it neither walks nor steals in the meantime.
        let mut captured = false;
        for _ in 0..50 {
            if game.world.get::<Tamed>(besieger).is_some() {
                captured = true;
                break;
            }
            if !crate::tests::tactical::wait_for_turn(&mut game, player) {
                break;
            }
            let at = game
                .world
                .resource::<TacticalBattle>()
                .cell_of(besieger)
                .expect("the target left the board");
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
                if game.tactical_step(dir) != crate::tactical::turn::StepOutcome::Moved {
                    break;
                }
            }
            if !game.tactical_use_routine(0, at) {
                game.tactical_end_turn();
            }
        }
        assert!(captured, "the besieger was never captured");
        assert!(
            game.world.get::<Besieger>(besieger).is_none(),
            "decompile_body must strip Besieger on capture"
        );
        assert!(
            game.world.get::<Carrying>(besieger).is_none(),
            "decompile_body must strip Carrying on capture"
        );
        assert!(
            game.world.get::<StolenFrom>(besieger).is_none(),
            "decompile_body must strip StolenFrom on capture"
        );

        let tamed_after_capture = tamed_count(&mut game);
        assert_eq!(
            tamed_after_capture,
            tamed_before + 1,
            "the fixture must have actually gained a companion"
        );

        // A real save/load round trip with the siege still in progress.
        let scratch = scratch_assets_dir("siege_new2_captured_besieger");
        std::fs::create_dir_all(&*scratch).unwrap();
        let path = scratch.join("save.bin");
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
        assert!(
            loaded.in_tactical_battle(),
            "the siege must survive the round trip"
        );
        assert_eq!(
            tamed_count(&mut loaded),
            tamed_after_capture,
            "the captured companion must survive a reload"
        );

        // End the siege in the loaded game — kill every remaining besieger
        // and cycle turns until the round wrap's reap closes the fight —
        // and confirm the captured companion is still standing afterward:
        // `Game::finish_fight`'s stray sweep is a world-wide query, so it
        // would catch this companion at the end of *any* fight, siege or
        // not, if the `Besieger` marker were still on it.
        let remaining: Vec<Entity> = {
            let battle = loaded.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .filter(|&e| loaded.world.get::<Besieger>(e).is_some())
                .collect()
        };
        assert!(
            !remaining.is_empty(),
            "the fixture must leave other besiegers to clear"
        );
        for e in remaining {
            if let Some(mut stats) = loaded.world.get_mut::<Stats>(e) {
                stats.hp = 0;
            }
        }
        for _ in 0..40 {
            if !loaded.in_tactical_battle() {
                break;
            }
            loaded.tactical_end_turn();
        }
        assert!(
            !loaded.in_tactical_battle(),
            "the fixture must actually end the fight"
        );

        assert_eq!(
            tamed_count(&mut loaded),
            tamed_after_capture,
            "the captured companion must survive the siege's own end, not \
             be swept as a stray besieger"
        );
    }

    /// NEW-2's second half in isolation: `Game::finish_fight`'s stray sweep
    /// is a world-wide query and must never touch a `Tamed` program, even
    /// one that (through some path other than `decompile_body` — a stale
    /// save, a future writer of `Besieger` that forgets the strip)
    /// still carries the `Besieger` marker. `decompile_body`'s own cleanup
    /// (asserted above) already keeps this unreachable in practice; this
    /// pins the sweep's own guard directly, defense in depth for the same
    /// reason the stray-`StackSpawn` sweep beside it already carries one.
    #[test]
    fn new2b_the_besieger_sweep_never_despawns_a_tamed_program() {
        let mut game = Game::new(210_003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        stand_in_base_at(&mut game, 2, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());

        let owner = game.player_entity();
        let mislabelled = any_besieger(&game);
        // The state the guard exists for, built directly rather than
        // through `decompile_body` — `Tamed` and `Besieger` co-existing on
        // one body, which nothing today produces but the sweep must still
        // survive.
        game.world.entity_mut(mislabelled).remove::<Hostile>();
        game.world.entity_mut(mislabelled).insert(Tamed { owner });

        let remaining: Vec<Entity> = {
            let battle = game.world.resource::<TacticalBattle>();
            battle
                .bodies()
                .map(|(e, _)| e)
                .filter(|&e| e != mislabelled && game.world.get::<Besieger>(e).is_some())
                .collect()
        };
        for e in remaining {
            if let Some(mut stats) = game.world.get_mut::<Stats>(e) {
                stats.hp = 0;
            }
        }
        for _ in 0..40 {
            if !game.in_tactical_battle() {
                break;
            }
            game.tactical_end_turn();
        }
        assert!(
            !game.in_tactical_battle(),
            "the fixture must actually end the fight"
        );

        assert!(
            game.world.get::<Tamed>(mislabelled).is_some(),
            "the stray-besieger sweep must never touch a Tamed program, \
             whatever else is still marked Besieger on it"
        );
    }
}
