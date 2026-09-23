//! `Game`'s four alert-board doors, `post_alert`, and the save round trip —
//! see `alerts` for the pure collapse/cap logic exercised there instead.
//! The rest of the file is Phase 2's sources, one section per row of the
//! spec's table.

use super::support::*;
use crate::alerts::{Alert, AlertKind};
use crate::*;

/// A temp path unique to the calling test — `traps.rs`'s reason: a fixed
/// path is shared with the `dev_template` loop, which deletes files mid-run
/// and produces a panic naming the load line rather than the collision.
fn save_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_alerts_{tag}_{}.bin",
        std::process::id()
    ))
}

#[test]
fn a_fresh_game_has_no_alerts() {
    let game = Game::new(19001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.alerts().is_empty());
    assert_eq!(game.unread_alerts(), 0);
}

#[test]
fn post_alert_shows_up_unread_in_the_view() {
    let mut game = Game::new(19002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    game.post_alert(AlertKind::ProgramDowned, "prog-1", "Scout is down.");
    game.post_alert(
        AlertKind::SweepIncoming,
        "sweep",
        "Sensors catch movement at the perimeter.",
    );

    let alerts = game.alerts();
    assert_eq!(alerts.len(), 2);
    assert_eq!(game.unread_alerts(), 2);
    assert!(alerts.iter().all(|a| a.unread));
    // Newest first.
    assert_eq!(alerts[0].kind, AlertKind::SweepIncoming);
    assert_eq!(alerts[1].kind, AlertKind::ProgramDowned);
}

#[test]
fn mark_alerts_read_zeroes_unread_alerts() {
    let mut game = Game::new(19003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "Scout is down.");
    game.post_alert(AlertKind::DepotsFull, "depots", "Every depot is full.");
    assert_eq!(game.unread_alerts(), 2);

    game.mark_alerts_read();

    assert_eq!(game.unread_alerts(), 0);
    assert!(game.alerts().iter().all(|a| !a.unread));
    // Marking read does not remove anything.
    assert_eq!(game.alerts().len(), 2);
}

#[test]
fn dismiss_alert_removes_exactly_the_indexed_row() {
    let mut game = Game::new(19004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "one");
    game.post_alert(AlertKind::ProgramDowned, "prog-2", "two");
    game.post_alert(AlertKind::ProgramDowned, "prog-3", "three");

    game.dismiss_alert(1);

    let alerts = game.alerts();
    assert_eq!(alerts.len(), 2);
    assert_eq!(alerts[0].text, "three");
    assert_eq!(alerts[1].text, "one");
}

#[test]
fn dismiss_alert_out_of_range_is_a_no_op() {
    let mut game = Game::new(19005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "one");

    game.dismiss_alert(5);

    assert_eq!(game.alerts().len(), 1);
}

/// A real save to a real file and a real load back. **A RON round trip
/// cannot catch a skipped field**; only a save-then-load can, and this repo
/// has shipped a `#[serde(skip)]` that left the round-trip test green.
#[test]
fn alerts_survive_a_save_and_load_with_order_count_and_unread_intact() {
    let mut game = Game::new(19006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "Scout is down.");
    game.post_alert(
        AlertKind::SweepIncoming,
        "sweep",
        "Sensors catch movement at the perimeter.",
    );
    // Re-posting collapses this one, bumping its count to 2.
    game.post_alert(AlertKind::ProgramDowned, "prog-1", "Scout is down again.");
    game.mark_alerts_read();
    // Posted after the mark, so this one alone should come back unread.
    game.post_alert(AlertKind::DepotsFull, "depots", "Every depot is full.");

    let before = game.alerts();
    assert_eq!(before.len(), 3);

    let path = save_path("roundtrip");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.alerts(),
        before,
        "order, count and unread all survive the round trip"
    );
    assert_eq!(loaded.unread_alerts(), 1);
}

/// Load re-applies the cap, so a hand-edited or `savetool`-packed save
/// cannot exceed it — see `alerts::cap`'s doc.
#[test]
fn a_save_carrying_more_than_the_cap_loads_capped() {
    let mut game = Game::new(19007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let path = save_path("cap");
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.alerts = (0..(tuning::ALERT_BOARD_CAP + 10))
        .map(|i| Alert {
            kind: AlertKind::ProgramDowned,
            subject: format!("prog-{i}"),
            text: format!("Program {i} is down."),
            count: 1,
            unread: true,
        })
        .collect();
    save::save_to_file(&path, &data).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.alerts().len(), tuning::ALERT_BOARD_CAP);
}

// ---------------------------------------------------------------------------
// Task 3: machine stalls (`systems::set_machine_status`).
// ---------------------------------------------------------------------------

/// A bare supplier, `power_supply: 4` and nothing else — no `power_upkeep`,
/// so `game::base::power::is_fuelled` reads it as fuelled unconditionally and
/// it needs no `PowerFuel` component or charge management.
const ALERT_SUPPLY: &str = r#"(
    id: "test_alert_supply",
    name: "Alert Supply",
    glyph: 'S',
    color: Orange,
    build_cost: [],
    work: None,
    power_supply: 4,
)"#;

/// A machine that draws exactly what `ALERT_SUPPLY` provides. Placed at a
/// tile sorting before the mining node under test (both carry no grid-fuel
/// rung, so ties break on `(tile, entity)`), it exhausts the whole budget
/// and is what pushes that node dark.
const ALERT_HOG: &str = r#"(
    id: "test_alert_hog",
    name: "Alert Hog",
    glyph: 'h',
    color: Brown,
    build_cost: [],
    work: Some((produces: "core_fragment", ticks_per_unit: 999)),
    power_draw: 4,
)"#;

/// A one-tick, no-`level` gather cycle — `resolve_gather_cycle` rolls no
/// fizzle without a `level`, so this node's clog-then-resume is deterministic
/// and draws no `GameRng`, unlike the shipped Mining Node used elsewhere in
/// this section.
const ALERT_NODE: &str = r#"(
    id: "test_alert_node",
    name: "Alert Node",
    glyph: 'n',
    color: Brown,
    build_cost: [],
    work: Some((produces: "core_fragment", ticks_per_unit: 1)),
    capacity: 1,
    power_draw: 1,
)"#;

fn alert_power_assets(tag: &str) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    copy_shipped_assets(&dir, &[]);
    std::fs::write(
        dir.join("structures").join("test_alert_supply.ron"),
        ALERT_SUPPLY,
    )
    .unwrap();
    std::fs::write(dir.join("structures").join("test_alert_hog.ron"), ALERT_HOG).unwrap();
    std::fs::write(
        dir.join("structures").join("test_alert_node.ron"),
        ALERT_NODE,
    )
    .unwrap();
    dir
}

/// A worked Mining Node at `(x, y)`, powered by a freshly spawned
/// `test_alert_supply`, capacity 1 so a handful of ticks clogs it.
fn alert_mining_node_at(game: &mut Game, x: i32, y: i32) -> Entity {
    game.world.spawn((
        Structure {
            kind: "test_alert_supply".to_string(),
        },
        Position { x: x - 5, y },
    ));
    let worker = spawn_tamed(game, 10, 3);
    let node = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x, y },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
            Stock::new(1),
            MachineStatus::default(),
        ))
        .id();
    stand_player_at_post(game, node);
    game.assign_cronjob(worker, node).unwrap();
    node
}

/// The deterministic sibling of `alert_mining_node_at` — `test_alert_node`
/// rather than the shipped Mining Node, for the one test that needs to
/// observe the transition back to `Running` on an exact tick rather than
/// tolerate a fizzle.
fn alert_deterministic_node_at(game: &mut Game, x: i32, y: i32) -> Entity {
    game.world.spawn((
        Structure {
            kind: "test_alert_supply".to_string(),
        },
        Position { x: x - 5, y },
    ));
    let worker = spawn_tamed(game, 10, 3);
    let node = game
        .world
        .spawn((
            Structure {
                kind: "test_alert_node".to_string(),
            },
            Position { x, y },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
            Stock::new(1),
            MachineStatus::default(),
        ))
        .id();
    stand_player_at_post(game, node);
    game.assign_cronjob(worker, node).unwrap();
    node
}

fn alert_kinds(game: &Game) -> Vec<AlertKind> {
    game.alerts().into_iter().map(|a| a.kind).collect()
}

/// Entering a stall posts once; staying in it — many more ticks past the
/// transition — posts nothing more. The deleted-fix check for this whole
/// section is the same: remove the `alerts::post` call inside
/// `set_machine_status` and every test below fails.
#[test]
fn a_clogged_machine_posts_once_and_stays_at_count_one() {
    let dir = alert_power_assets("alerts_clog_once");
    let mut game = Game::new(19101, DifficultyMode::Forgiving, &dir).unwrap();
    stand_in_base(&mut game);
    let node = alert_mining_node_at(&mut game, 3, 4);

    for _ in 0..60 {
        game.tick();
    }
    assert_eq!(
        game.world.get::<MachineStatus>(node).copied(),
        Some(MachineStatus::Clogged),
        "the fixture has to actually clog, or the rest of this proves nothing"
    );
    assert_eq!(
        alert_kinds(&game),
        vec![AlertKind::MachineStalled(MachineStatus::Clogged)]
    );
    let count_after_first_clog = game.alerts()[0].count;

    for _ in 0..60 {
        game.tick();
    }
    assert_eq!(game.alerts().len(), 1, "still clogged, not a second row");
    assert_eq!(
        game.alerts()[0].count,
        count_after_first_clog,
        "staying in the state does not bump count"
    );
}

/// Going Clogged, then Unpowered, on the *same* machine — the identity
/// includes the status, so this is two rows rather than a collapse.
#[test]
fn a_clogged_machine_that_then_goes_dark_gets_a_second_alert() {
    let dir = alert_power_assets("alerts_clog_then_dark");
    let mut game = Game::new(19102, DifficultyMode::Forgiving, &dir).unwrap();
    stand_in_base(&mut game);
    let node = alert_mining_node_at(&mut game, 3, 4);

    for _ in 0..60 {
        game.tick();
    }
    assert_eq!(
        game.world.get::<MachineStatus>(node).copied(),
        Some(MachineStatus::Clogged)
    );
    assert_eq!(game.alerts().len(), 1);

    // Sorts before the node (smaller tile) and draws the whole supply, so
    // the node's own turn finds nothing left and goes dark.
    game.world.spawn((
        Structure {
            kind: "test_alert_hog".to_string(),
        },
        Position { x: 0, y: 0 },
    ));
    game.tick();

    assert_eq!(
        game.world.get::<MachineStatus>(node).copied(),
        Some(MachineStatus::Unpowered),
        "the hog has to actually darken the node, or this proves nothing"
    );
    let kinds = alert_kinds(&game);
    assert_eq!(
        kinds.len(),
        2,
        "different stall states are different alerts"
    );
    assert!(kinds.contains(&AlertKind::MachineStalled(MachineStatus::Clogged)));
    assert!(kinds.contains(&AlertKind::MachineStalled(MachineStatus::Unpowered)));
}

/// Resolving a stall does not remove its alert — only a dismiss does.
#[test]
fn a_resolved_stall_leaves_its_alert_on_the_board() {
    let dir = alert_power_assets("alerts_clog_resolves");
    let mut game = Game::new(19103, DifficultyMode::Forgiving, &dir).unwrap();
    stand_in_base(&mut game);
    let node = alert_deterministic_node_at(&mut game, 3, 4);

    for _ in 0..5 {
        game.tick();
    }
    assert_eq!(
        game.world.get::<MachineStatus>(node).copied(),
        Some(MachineStatus::Clogged),
        "the fixture has to actually clog, or the rest of this proves nothing"
    );
    assert_eq!(game.alerts().len(), 1);
    let count_before = game.alerts()[0].count;

    // `take_everything_adjacent` (`Game::transfer_items`) ticks once itself
    // on a non-empty take, and `ALERT_NODE`'s one-tick, no-`level` cycle
    // means that single tick is exactly enough to resume and pay out again
    // deterministically — no extra `game.tick()` needed or wanted, since a
    // second one would immediately re-clog a capacity-1 buffer.
    take_everything_adjacent(&mut game);

    assert_eq!(
        game.world.get::<MachineStatus>(node).copied(),
        Some(MachineStatus::Running),
        "the fixture has to actually resume, or this proves nothing"
    );
    assert_eq!(
        game.alerts().len(),
        1,
        "resolving is not the same as dismissing"
    );
    assert_eq!(
        game.alerts()[0].kind,
        AlertKind::MachineStalled(MachineStatus::Clogged)
    );
    assert_eq!(
        game.alerts()[0].count,
        count_before,
        "resuming does not touch the existing alert's count"
    );
}

// ---------------------------------------------------------------------------
// Task 4: program downed (`Game::bench_or_dissolve`).
// ---------------------------------------------------------------------------

/// Stands a companion in a fight and kills it, mirroring
/// `combat_rewards.rs`'s `a_companion_killed_in_battle`: teardown is what is
/// under test, not `resolve_attack`, so the kill is a direct HP write.
fn alert_companion_killed_in_battle(game: &mut Game) -> Entity {
    let player = game.player_entity();
    let companion = spawn_tamed(game, 10, 3);
    enlist(game, companion);
    let enemy = spawn_wild_on_player_tile(game);
    insert_battle(game, player, vec![enemy]);
    game.world.get_mut::<Stats>(companion).unwrap().hp = 0;
    companion
}

/// A Forgiving bench, driven end to end through `end_battle` rather than
/// calling `bench_or_dissolve` directly — the site asking the door is the
/// point (correction 1: every caller of it agrees on the alert, not just
/// the door in isolation).
#[test]
fn a_forgiving_bench_posts_a_downed_alert() {
    let mut game = Game::new(19201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = alert_companion_killed_in_battle(&mut game);

    game.end_battle(player, None);

    assert!(
        game.world.get::<components::Downed>(companion).is_some(),
        "the fixture has to actually bench it, or this proves nothing"
    );
    let alerts = game.alerts();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, AlertKind::ProgramDowned);
    assert!(alerts[0].text.ends_with("is down."), "{}", alerts[0].text);
}

/// A Permadeath dissolve, same end-to-end shape, worded as a loss rather
/// than a bench.
#[test]
fn a_permadeath_dissolve_posts_a_downed_alert() {
    let mut game = Game::new(19202, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = alert_companion_killed_in_battle(&mut game);

    game.end_battle(player, None);

    assert!(
        game.world.get::<Stats>(companion).is_none(),
        "the fixture has to actually destroy it, or this proves nothing"
    );
    let alerts = game.alerts();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, AlertKind::ProgramDowned);
    assert!(alerts[0].text.ends_with("was lost."), "{}", alerts[0].text);
}

/// Downing the same program twice collapses into one row with `count: 2` —
/// `bench_or_dissolve` called directly (it is `pub(crate)`) since the point
/// under test is the alert board's collapse, not a second teardown site.
#[test]
fn downing_the_same_program_twice_collapses() {
    let mut game = Game::new(19203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);

    game.bench_or_dissolve(companion);
    game.bench_or_dissolve(companion);

    let alerts = game.alerts();
    assert_eq!(alerts.len(), 1, "same program, same subject, one row");
    assert_eq!(alerts[0].count, 2);
}

// ---------------------------------------------------------------------------
// Task 5: sweep and siege.
// ---------------------------------------------------------------------------

fn alert_kind_count(game: &Game, kind: &AlertKind) -> usize {
    game.alerts().iter().filter(|a| &a.kind == kind).count()
}

/// The sweep's approach warning latches — `raid_check`'s own `warned` flag —
/// so posting it across repeated checks still leaves one row.
#[test]
fn sweep_incoming_warns_once_across_repeated_checks() {
    let mut game = Game::new(19301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);
    spawn_min_raid_staff(&mut game);

    game.dev_wind_raid_clock();
    game.raid_check();
    assert!(
        game.world
            .resource::<crate::resources::RaidPressure>()
            .warned,
        "the fixture has to actually warn, or this proves nothing"
    );
    game.raid_check();

    assert_eq!(alert_kind_count(&game, &AlertKind::SweepIncoming), 1);
    let alert = game
        .alerts()
        .into_iter()
        .find(|a| a.kind == AlertKind::SweepIncoming)
        .unwrap();
    assert_eq!(
        alert.count, 1,
        "the latch means a second check posts nothing more"
    );
}

/// `run_raid` posts once per sweep, whichever branch it lands in.
#[test]
fn a_sweep_posts_one_sweep_hit_alert() {
    let mut game = Game::new(19302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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

    game.dev_force_raid();

    assert!(
        game.world
            .get::<Durability>(structure)
            .is_some_and(|d| d.hp < 30),
        "the fixture has to actually sweep, or this proves nothing"
    );
    assert_eq!(alert_kind_count(&game, &AlertKind::SweepHit), 1);
}

/// The siege's approach warning, `SiegePressure`'s own latch — `raid_check`'s
/// pattern exactly.
#[test]
fn siege_incoming_warns_once_across_repeated_checks() {
    let mut game = Game::new(19303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_zone(&mut game, 2);

    game.dev_wind_siege_clock();
    game.siege_check();
    assert!(
        game.siege_warned(),
        "the fixture has to actually warn, or this proves nothing"
    );
    game.siege_check();

    assert_eq!(alert_kind_count(&game, &AlertKind::SiegeIncoming), 1);
}

/// A staged siege posts `SiegeBegun` — `open_siege`'s own line, next to the
/// "Besiegers pour in" text it reuses as the alert's own.
#[test]
fn opening_a_siege_posts_a_siege_begun_alert() {
    let mut game = Game::new(19304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.lay_starting_pocket();
    stand_in_base_at(&mut game, 0, 0);
    set_zone(&mut game, 2);

    assert!(
        game.open_siege(),
        "the fixture has to actually open one, or this proves nothing"
    );
    assert_eq!(alert_kind_count(&game, &AlertKind::SiegeBegun), 1);
}

/// An off-screen siege posts `SiegeBegun` even when the shortfall comes out
/// to zero — correction 2's rule, that a siege fully held off while the
/// player was away is still worth seeing on the board.
#[test]
fn an_off_screen_siege_with_no_shortfall_still_posts_siege_begun() {
    let mut game = Game::new(19305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // `SIEGE_PACK_BASE` (4) + one sector's worth at zone 1 is 5; three idle
    // staff at `SIEGE_STAFF_DEFENSE` (2) each covers it with room to spare.
    for _ in 0..3 {
        spawn_tamed(&mut game, 10, 3);
    }
    assert_eq!(
        game.base_staff().len(),
        3,
        "the fixture has to actually stand as staff, or the shortfall math proves nothing"
    );

    let fired = game.resolve_siege_offscreen();

    assert!(fired);
    assert!(
        !game
            .base_staff()
            .iter()
            .any(|&e| game.world.get::<crate::components::Downed>(e).is_some()),
        "a zero shortfall must take no casualty, or this wasn't the zero-shortfall case"
    );
    assert_eq!(alert_kind_count(&game, &AlertKind::SiegeBegun), 1);
}

// ---------------------------------------------------------------------------
// Task 6: sites cut off and depots full.
// ---------------------------------------------------------------------------

/// A Home laid, the party standing at its own exit cell, and `n` idle staff
/// — `base_space.rs`'s `base_with_a_crew`, reconstructed here since that one
/// is private to its own file.
fn alert_base_with_crew(seed: u32, n: usize) -> (Game, Vec<Entity>) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 20);
    place_now(&mut game, "home", 1, 0).unwrap();
    stand_in_base_at(&mut game, 0, 0);
    let mut staff = Vec::new();
    for _ in 0..n {
        staff.push(spawn_tamed(&mut game, 10, 3));
    }
    (game, staff)
}

/// Open ground far enough out that nothing walkable touches it — a station
/// exists and no route to it does, `base_space.rs`'s `STRANDED_STATION`/
/// `STRANDED_CELL` fixture.
const ALERT_STRANDED_STATION: (i32, i32) = (20, 0);
const ALERT_STRANDED_CELL: (i32, i32) = (21, 0);

#[test]
fn an_unreachable_dig_site_posts_a_site_cut_off_alert_once() {
    let (mut game, _staff) = alert_base_with_crew(19401, 1);
    // A cut the base could pay to floor, so the route is what stalls it
    // rather than a dry base dropping the want before the route matters.
    give(&mut game, &ItemId::from(ids::BLANK_SUBSTRATE), 1);
    let tick = game.current_tick();
    game.world.resource_mut::<base_grid::BaseGrid>().open(
        ALERT_STRANDED_STATION.0,
        ALERT_STRANDED_STATION.1,
        tick,
    );
    game.toggle_mark_box(ALERT_STRANDED_CELL, ALERT_STRANDED_CELL, None);

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(alert_kind_count(&game, &AlertKind::SiteCutOff), 1);
    let alert = game
        .alerts()
        .into_iter()
        .find(|a| a.kind == AlertKind::SiteCutOff)
        .unwrap();
    assert_eq!(
        alert.count, 1,
        "repeated ticks past the first stall post nothing more"
    );
}

#[test]
fn an_unreachable_build_site_posts_a_site_cut_off_alert_once() {
    let mut game = Game::new(19402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "home", 1, 0).unwrap();
    stand_in_base_at(&mut game, 0, 0);
    // A build needs laid `Floor`, unlike a dig site's `Open` station — the
    // island still has no route back to the pocket, since nothing between
    // the two is carved at all.
    game.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(ALERT_STRANDED_STATION.0, ALERT_STRANDED_STATION.1);

    file_build(
        &mut game,
        "mining_node",
        ALERT_STRANDED_STATION.0,
        ALERT_STRANDED_STATION.1,
    )
    .unwrap();
    // `file_build`'s own program is spent as the build's cost (a build costs
    // a tamed program exactly when the structure runs a job) and its spare
    // lands in the party, not on staff — so a body actually free to be sent
    // (and refused) has to be spawned on top.
    spawn_tamed(&mut game, 10, 3);

    for _ in 0..20 {
        game.tick();
    }

    assert_eq!(alert_kind_count(&game, &AlertKind::SiteCutOff), 1);
}

/// A worker whose load has nowhere to land — `hauling.rs`'s
/// `a_load_with_nowhere_to_land_goes_back_and_re_clogs_the_machine` fixture,
/// reconstructed for the same private-to-its-file reason as the crew above.
fn alert_base_for_hauling(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    game
}

/// Finds the structure standing at `(x, y)` — `deploy`'s own lookup, since
/// `place_now` reports only success.
fn alert_structure_at(game: &mut Game, x: i32, y: i32) -> Entity {
    game.world
        .query::<(Entity, &Position, &Structure)>()
        .iter(&game.world)
        .find(|(_, p, _)| p.x == x && p.y == y)
        .map(|(e, ..)| e)
        .expect("the structure was just deployed")
}

#[test]
fn depots_full_posts_once_across_repeated_failed_hauls() {
    let mut game = alert_base_for_hauling(19403);
    let (bx, by) = game.base_pos().unwrap();
    place_now(&mut game, "mining_node", 1, 0).unwrap();
    let node = alert_structure_at(&mut game, bx + 1, by);
    place_now(&mut game, "depot", 4, 0).unwrap();
    let depot = alert_structure_at(&mut game, bx + 4, by);
    let worker = spawn_tamed(&mut game, 500, 3);
    game.assign_cronjob(worker, node).unwrap();

    let cap = game.world.get::<Stock>(node).unwrap().capacity;
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), cap);

    for _ in 0..200 {
        if game.world.get::<Carrying>(worker).is_some() {
            break;
        }
        game.tick();
    }
    assert!(
        game.world.get::<Carrying>(worker).is_some(),
        "precondition: the worker has to actually pick up a load"
    );

    // Brim-full with something a Mining Node never makes, so the only reason
    // the load cannot land is room.
    let depot_cap = game.world.get::<Stock>(depot).unwrap().capacity;
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from(ids::POWER_CELL), depot_cap);

    for _ in 0..300 {
        if game.world.get::<Carrying>(worker).is_none() {
            break;
        }
        game.tick();
    }
    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "precondition: the load has to actually come back, or the branch never runs"
    );

    assert_eq!(
        alert_kind_count(&game, &AlertKind::DepotsFull),
        1,
        "the first failure has to latch"
    );

    // Several more failed round trips off the same clog, all under the one
    // latch.
    for _ in 0..300 {
        game.tick();
    }

    assert_eq!(
        alert_kind_count(&game, &AlertKind::DepotsFull),
        1,
        "repeated failures under the same latch post nothing more"
    );
    assert_eq!(
        game.alerts()
            .into_iter()
            .find(|a| a.kind == AlertKind::DepotsFull)
            .unwrap()
            .count,
        1
    );
}

/// After a successful deposit clears the latch, the next failure posts again
/// and collapses into `×2`.
#[test]
fn depots_full_posts_again_after_a_successful_deposit_clears_the_latch() {
    let mut game = alert_base_for_hauling(19404);
    let (bx, by) = game.base_pos().unwrap();
    place_now(&mut game, "mining_node", 1, 0).unwrap();
    let node = alert_structure_at(&mut game, bx + 1, by);
    place_now(&mut game, "depot", 4, 0).unwrap();
    let depot = alert_structure_at(&mut game, bx + 4, by);
    let worker = spawn_tamed(&mut game, 500, 3);
    game.assign_cronjob(worker, node).unwrap();

    // The depot must still have room when the worker *picks up* the load —
    // `Errand::Tend` refuses to start an errand at all while `depots` (every
    // depot with room) is empty, `with_no_depot_a_clogged_machine_just_
    // stays_clogged`'s rule. So the clog and the full depot are staged in
    // that order, with a pickup wait between them, on both round trips.
    let clog_node = |game: &mut Game| {
        let cap = game.world.get::<Stock>(node).unwrap().capacity;
        game.world
            .get_mut::<Stock>(node)
            .unwrap()
            .output
            .insert(ItemId::from(ids::CORE_FRAGMENT), cap);
    };
    let fill_depot = |game: &mut Game| {
        let depot_cap = game.world.get::<Stock>(depot).unwrap().capacity;
        game.world
            .get_mut::<Stock>(depot)
            .unwrap()
            .output
            .insert(ItemId::from(ids::POWER_CELL), depot_cap);
    };
    let wait_for_pickup = |game: &mut Game| {
        for _ in 0..200 {
            if game.world.get::<Carrying>(worker).is_some() {
                return;
            }
            game.tick();
        }
    };
    let wait_for_return = |game: &mut Game| {
        for _ in 0..300 {
            if game.world.get::<Carrying>(worker).is_none() {
                return;
            }
            game.tick();
        }
    };

    clog_node(&mut game);
    wait_for_pickup(&mut game);
    assert!(
        game.world.get::<Carrying>(worker).is_some(),
        "precondition: the worker has to actually pick up a load"
    );
    fill_depot(&mut game);
    wait_for_return(&mut game);
    assert_eq!(
        alert_kind_count(&game, &AlertKind::DepotsFull),
        1,
        "precondition: the first failure has to actually latch"
    );

    // Empty the depot, so the next attempt actually lands.
    game.world.get_mut::<Stock>(depot).unwrap().output.clear();

    for _ in 0..300 {
        if game.world.get::<Stock>(node).unwrap().output_used() == 0 {
            break;
        }
        game.tick();
    }
    assert_eq!(
        game.world.get::<Stock>(node).unwrap().output_used(),
        0,
        "precondition: the load has to actually land this time"
    );

    // Re-clog the machine to force another failed round trip.
    clog_node(&mut game);
    wait_for_pickup(&mut game);
    assert!(
        game.world.get::<Carrying>(worker).is_some(),
        "precondition: the second round trip has to actually start"
    );
    fill_depot(&mut game);
    wait_for_return(&mut game);

    let alert = game
        .alerts()
        .into_iter()
        .find(|a| a.kind == AlertKind::DepotsFull)
        .unwrap();
    assert_eq!(
        alert.count, 2,
        "the latch cleared and re-armed, collapsing into the same row"
    );
}
