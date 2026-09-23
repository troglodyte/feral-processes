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
