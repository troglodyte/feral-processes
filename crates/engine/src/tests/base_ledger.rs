//! The ledger's wiring: that it is registered, that it survives a reload,
//! and that a save written before it existed still loads.
//!
//! The fold itself is unit-tested in `crate::base_ledger`. What is here is
//! everything that touches the `World` and the save file, because
//! `a_save_survives_a_round_trip_through_ron_unchanged` **cannot** catch a
//! field that never reaches the disk — a `#[serde(skip)]` leaves it green.

use super::support::*;
use crate::base_ledger::{BaseLedger, Event};
use crate::items::ItemId;
use crate::*;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn item(id: &str) -> ItemId {
    ItemId(id.to_string())
}

#[test]
fn a_fresh_run_carries_an_empty_ledger() {
    let game = game(4021);
    let ledger = game.world.resource::<BaseLedger>();
    assert!(ledger.lifetime.is_empty());
    assert!(ledger.buckets.is_empty());
}

#[test]
fn what_the_base_produced_survives_a_real_save_and_load() {
    let mut game = game(4022);
    game.world.resource_mut::<BaseLedger>().fold(
        0,
        1,
        &Event::Extract {
            item: item("core_fragment"),
            rolled: 7,
            landed: 5,
            ok: true,
        },
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_base_ledger_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let totals = loaded.world.resource::<BaseLedger>().lifetime[&item("core_fragment")];
    assert_eq!(totals.mined, 5, "the lifetime total came back");
    assert_eq!(totals.lost, 2, "and so did what the clog ate");
    assert_eq!(
        loaded.world.resource::<BaseLedger>().buckets.len(),
        1,
        "the bucketed history came back too, not just the lifetime fold"
    );
}

/// The field is additive behind `#[serde(default)]`, so it costs no
/// `SAVE_FORMAT_VERSION` bump — a save written before it existed loads with
/// an empty ledger, which is exactly what that run had recorded.
#[test]
fn a_save_written_before_the_ledger_existed_still_loads() {
    let mut game = game(4023);
    let path = std::env::temp_dir().join(format!(
        "feral_processes_base_ledger_legacy_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let (version, ron) = text.split_once('\n').unwrap();
    let stripped = without_field(ron, "base_ledger");
    assert!(
        !stripped.contains("base_ledger"),
        "the fixture must actually remove the field to be a real test"
    );
    std::fs::write(&path, format!("{version}\n{stripped}")).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).expect("a pre-ledger save still loads");
    let _ = std::fs::remove_file(&path);
    assert!(loaded.world.resource::<BaseLedger>().lifetime.is_empty());
}

/// Cuts one top-level field and its whole value out of pretty RON, matching
/// brackets so a nested struct goes with it. Dropping the `field:` line
/// alone leaves the value's body behind, which parses as trailing
/// characters rather than as the older file this is imitating.
fn without_field(ron: &str, field: &str) -> String {
    let start = ron
        .find(&format!("    {field}: "))
        .expect("the field is written at the top level");
    let mut depth = 0i32;
    let mut end = start;
    for (offset, ch) in ron[start..].char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth > 0 => depth -= 1,
            ')' if depth == 0 => break,
            ',' if depth == 0 => {
                end = start + offset + 1;
                break;
            }
            _ => {}
        }
    }
    assert!(end > start, "the field's value must be delimited");
    format!("{}{}", &ron[..start], &ron[end..])
}

/// The seams, end to end. These are what separate "the ledger compiles" from
/// "the ledger is fed": every test above this point would stay green with
/// every `emit` call deleted.
mod seams {
    use super::*;
    use crate::components::{MachineStatus, Position, ResourceNode, Structure, Task, TaskKind};
    use crate::resources::BattleTelemetry;

    /// A node with `level: None` skips the reliability roll entirely, so this
    /// is a payout every time rather than a seeded one — the fizzle arm has
    /// its own unit test and does not need luck here.
    /// A `mining_node` authors `power_draw: 1`, so a hand-spawned one with
    /// nothing supplying the grid is dark and produces nothing at all —
    /// which reads exactly like a broken seam. The Home's free 4 is the
    /// bootstrap the real game uses, so it is what these fixtures use.
    fn light_the_grid(game: &mut Game) {
        game.world.spawn((
            Structure {
                kind: "home".to_string(),
            },
            Position { x: 0, y: 0 },
        ));
    }

    #[test]
    fn a_worked_node_reaches_the_ledger() {
        let mut game = game(4101);
        light_the_grid(&mut game);
        let worker = spawn_tamed(&mut game, 10, 3);
        let node = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::from(ids::CORE_FRAGMENT),
                    level: None,
                },
                work_node_parts(),
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: node,
            progress: 0,
            required: 1,
        });

        for _ in 0..3 {
            game.tick();
        }

        let mined =
            game.world.resource::<BaseLedger>().lifetime[&ItemId::from(ids::CORE_FRAGMENT)].mined;
        assert!(
            mined > 0,
            "a posted worker's cycles must reach the ledger, got {mined}"
        );
    }

    /// The denominator. Without it every rate in the log is an absolute,
    /// and B7 asks for rate per *posted program*.
    #[test]
    fn a_snapshot_heads_each_window_and_counts_the_base() {
        let mut game = game(4110);
        game.world.resource_mut::<BattleTelemetry>().on = true;
        light_the_grid(&mut game);
        game.world.spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
            work_node_parts(),
        ));

        // Two windows' worth, so the "once per window" half is measured
        // rather than assumed — a per-tick snapshot passes any test that
        // only looks for one.
        for _ in 0..(crate::base_ledger::BUCKET_TICKS * 2) {
            game.tick();
        }

        let snapshots: Vec<(u64, u32, u32)> = game
            .world
            .resource::<BattleTelemetry>()
            .records
            .iter()
            .filter_map(|r| match r {
                crate::telemetry::Record::BaseSnapshot {
                    tick,
                    machines,
                    depots,
                    ..
                } => Some((*tick, *machines, *depots)),
                _ => None,
            })
            .collect();

        assert_eq!(
            snapshots.iter().map(|s| s.0).collect::<Vec<_>>(),
            vec![0, crate::base_ledger::BUCKET_TICKS],
            "one snapshot per window, stamped with the tick that opens it"
        );
        assert_eq!(
            snapshots[0].1, 1,
            "the Mining Node runs a job and the Home does not"
        );
        assert_eq!(snapshots[0].2, 0, "and neither of them stores");
    }

    /// The base spending its own shelves — the dig crew's tile, a sortie's
    /// outfitting. Reported inside `spend_from_base` rather than at its
    /// callers, because a partial take is the case a caller-side figure
    /// would get wrong.
    #[test]
    fn what_leaves_the_shelves_is_counted_as_consumed() {
        let mut game = game(4108);
        let shelf = game
            .world
            .spawn((
                Structure {
                    kind: "depot".to_string(),
                },
                Position { x: 1, y: 1 },
                crate::components::Stock::default(),
            ))
            .id();
        game.world
            .get_mut::<crate::components::Stock>(shelf)
            .unwrap()
            .output
            .insert(ItemId::from(ids::CORE_FRAGMENT), 6);

        let taken = crate::game::base::stock::spend_from_base(
            &mut game,
            &ItemId::from(ids::CORE_FRAGMENT),
            4,
            crate::base_ledger::ConsumeSource::Base,
        );

        assert_eq!(taken, 4);
        assert_eq!(
            game.world.resource::<BaseLedger>().lifetime[&ItemId::from(ids::CORE_FRAGMENT)]
                .consumed,
            4,
            "what left the shelf must reach the ledger's sink side"
        );
    }

    /// The breach destroys the two currencies outright, and nothing else in
    /// the ledger could see it: a run's fragments would otherwise read as
    /// produced and never accounted for.
    #[test]
    fn a_breach_counts_what_it_destroys() {
        let mut game = game(4109);
        game.grant_loot(
            ItemId::from(ids::CORE_FRAGMENT),
            7,
            crate::base_ledger::LootSource::Kill,
        );
        // Read rather than assumed: a fresh run starts with a few of these
        // in the pack, and the wipe takes the whole holding.
        let player = game.player_entity();
        let held = game
            .world
            .get::<crate::components::Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT));
        assert!(
            held > 7,
            "the fixture must hold more than it was just given"
        );

        game.enter_next_zone();

        assert_eq!(
            game.world.resource::<BaseLedger>().lifetime[&ItemId::from(ids::CORE_FRAGMENT)]
                .consumed,
            held,
            "the wipe is a sink and has to be counted as one"
        );
    }

    /// **Loot is recorded and never folded.** The ledger and the page it
    /// feeds are about what the *base* made; a kill's Core Fragments
    /// counted there would read on the screen as a machine's work, which is
    /// the one thing the MINED/COMPILED split exists to keep honest. What
    /// the record answers is B5, and that lives in the analysis.
    #[test]
    fn loot_carries_its_source_and_never_reaches_the_ledger() {
        let mut game = game(4107);
        game.world.resource_mut::<BattleTelemetry>().on = true;
        game.grant_loot(
            ItemId::from(ids::CORE_FRAGMENT),
            4,
            crate::base_ledger::LootSource::Kill,
        );

        assert!(
            game.world.resource::<BaseLedger>().lifetime.is_empty(),
            "a kill's drop was counted as something the base produced"
        );
        match &game.world.resource::<BattleTelemetry>().records[..] {
            [
                crate::telemetry::Record::Acquire {
                    item, qty, source, ..
                },
            ] => {
                assert_eq!(
                    (item.as_str(), *qty, source.as_str()),
                    ("core_fragment", 4, "kill")
                );
            }
            other => panic!("expected one acquire record, got {other:?}"),
        }
    }

    /// A machine changing status is news for the log and moves no units, so
    /// it rides `record_in_system` rather than the ledger's `emit` — and it
    /// hangs on `set_machine_status`, which already speaks **only on
    /// transition**. That is what makes this one record rather than one per
    /// tick for the rest of the run.
    #[test]
    fn a_stall_is_recorded_once_per_transition_and_not_once_per_tick() {
        let mut game = game(4105);
        light_the_grid(&mut game);
        game.world.resource_mut::<BattleTelemetry>().on = true;
        // Nobody posted to it, which is what `idle_machine_system` reports.
        game.world.spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
            work_node_parts(),
        ));

        for _ in 0..5 {
            game.tick();
        }

        let stalls: Vec<_> = game
            .world
            .resource::<BattleTelemetry>()
            .records
            .iter()
            .filter_map(|r| match r {
                crate::telemetry::Record::MachineStall {
                    machine,
                    kind,
                    status,
                    ..
                } if *machine == (3, 4) => Some((kind.clone(), status.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            stalls,
            vec![("mining_node".to_string(), "idle".to_string())],
            "five ticks in one status must be one record, and it names the \
             def id rather than the display name"
        );
    }

    /// The other half of the discipline: the record must not be *built* when
    /// no dev log is armed. Nothing in the compiler holds this at a bevy
    /// seam — there is no `&Game` to hand a closure to — so it is asserted
    /// rather than constructed.
    #[test]
    fn a_stall_builds_nothing_when_the_log_is_disarmed() {
        let mut game = game(4106);
        light_the_grid(&mut game);
        game.world.spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                level: None,
            },
            work_node_parts(),
        ));

        for _ in 0..5 {
            game.tick();
        }

        assert!(
            game.world.resource::<BattleTelemetry>().records.is_empty(),
            "a disarmed log recorded a stall"
        );
    }

    /// The player cranking the handle themselves. The design spec named only
    /// `task_progress_system`; this is the seam it missed, and a run where
    /// the player works their own nodes would show an empty screen without
    /// it.
    #[test]
    fn the_player_working_a_node_reaches_the_ledger_too() {
        let mut game = game(4102);
        light_the_grid(&mut game);
        let player = game.player_entity();
        let node = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 3 },
                ResourceNode {
                    resource: ItemId::from(ids::CORE_FRAGMENT),
                    level: None,
                },
                work_node_parts(),
            ))
            .id();
        let at = *game.world.get::<Position>(node).unwrap();
        game.world.get_mut::<Position>(player).unwrap().x = at.x;
        game.world.get_mut::<Position>(player).unwrap().y = at.y;
        game.world.entity_mut(player).insert(Task {
            kind: TaskKind::GatherResource,
            target: node,
            progress: 0,
            required: 1,
        });

        for _ in 0..3 {
            game.tick();
        }

        let mined =
            game.world.resource::<BaseLedger>().lifetime[&ItemId::from(ids::CORE_FRAGMENT)].mined;
        assert!(
            mined > 0,
            "the player's own cycles must reach the ledger, got {mined}"
        );
    }

    #[test]
    fn a_hand_compile_reaches_the_ledger_and_is_not_counted_as_a_machine() {
        let mut game = game(4103);
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            inv.add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST);
        }

        game.craft(&ItemId::from(ids::ICE_BREAKER), 1, false)
            .unwrap();

        let ledger = game.world.resource::<BaseLedger>();
        let totals = ledger.lifetime[&ItemId::from(ids::ICE_BREAKER)];
        assert_eq!(totals.hand, 1);
        assert_eq!(
            totals.compiled, 0,
            "a hand-compile must not read as a machine's work — that split is the whole of B2"
        );
    }

    /// `Game::craft` is `begin_hand_craft` plus the timed loop drained to
    /// completion, so one seam covers both paths. This is what would catch
    /// them being separated again.
    #[test]
    fn the_timed_compile_and_the_headless_one_count_the_same() {
        fn hand_total(seed: u32, timed: bool) -> u32 {
            let mut game = game(seed);
            let player = game.player_entity();
            {
                let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
                inv.items.clear();
                inv.add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST);
            }
            let item = ItemId::from(ids::ICE_BREAKER);
            if timed {
                game.begin_hand_craft(&item, 1, false).unwrap();
                while let Some(progress) = game.advance_hand_craft() {
                    if progress.finished {
                        break;
                    }
                }
            } else {
                game.craft(&item, 1, false).unwrap();
            }
            game.world.resource::<BaseLedger>().lifetime[&item].hand
        }

        assert_eq!(hand_total(4104, true), hand_total(4104, false));
        assert_eq!(hand_total(4104, true), 1);
    }
}

/// The page's one derivation. Everything the screen shows comes from here,
/// so a renderer cannot invent a figure or disagree with the ledger.
mod report {
    use super::*;

    fn fed(seed: u32) -> Game {
        let mut game = game(seed);
        let mut ledger = crate::base_ledger::BaseLedger::default();
        // A mined item, an assembled one, and one only ever made by hand.
        ledger.fold(
            0,
            1,
            &Event::Extract {
                item: item(ids::CORE_FRAGMENT),
                rolled: 10,
                landed: 8,
                ok: true,
            },
        );
        ledger.fold(
            0,
            1,
            &Event::Assemble {
                product: item(ids::POWER_CELL),
                inputs: vec![(item(ids::CORE_FRAGMENT), 2)],
            },
        );
        ledger.fold(
            0,
            1,
            &Event::HandCraft {
                item: item(ids::POWER_CELL),
                qty: 3,
            },
        );
        *game.world.resource_mut::<crate::base_ledger::BaseLedger>() = ledger;
        game
    }

    /// A Power Cell is the case that decides the rule: a structure's `work`
    /// produces it *and* it is hand-compilable, so sectioning on the def
    /// files it under MINED however many the player pressed out by hand.
    #[test]
    fn sections_follow_how_a_thing_was_actually_made() {
        let mut game = fed(4201);
        let report = game.base_output_report();

        assert!(
            report
                .mined
                .iter()
                .any(|r| r.item == item(ids::CORE_FRAGMENT)),
            "units that came out of an extractor belong under MINED"
        );
        assert!(
            report
                .compiled
                .iter()
                .any(|r| r.item == item(ids::POWER_CELL)),
            "an assembled item belongs under COMPILED"
        );
        assert!(
            !report.mined.iter().any(|r| r.item == item(ids::POWER_CELL)),
            "and must not appear in both"
        );
    }

    /// The split the screen exists to show. A combined figure would hide
    /// what hand-compiling contributes, which is the whole of B2.
    #[test]
    fn a_compiled_row_separates_the_machine_from_the_hands() {
        let mut game = fed(4202);
        let report = game.base_output_report();
        let row = report
            .compiled
            .iter()
            .find(|r| r.item == item(ids::POWER_CELL))
            .expect("the assembled item has a row");

        assert_eq!(row.machine, 1);
        assert_eq!(row.hand, 3);
        assert_eq!(row.run, 4, "the run column is everything that landed");
    }

    #[test]
    fn the_sector_column_reads_only_this_sector() {
        let mut game = game(4203);
        {
            let mut ledger = game.world.resource_mut::<crate::base_ledger::BaseLedger>();
            ledger.fold(
                0,
                1,
                &Event::HandCraft {
                    item: item(ids::POWER_CELL),
                    qty: 5,
                },
            );
            ledger.fold(
                crate::base_ledger::BUCKET_TICKS,
                2,
                &Event::HandCraft {
                    item: item(ids::POWER_CELL),
                    qty: 2,
                },
            );
        }
        game.world.resource_mut::<crate::resources::ZoneLevel>().0 = 2;

        let report = game.base_output_report();
        let row = &report.compiled[0];
        assert_eq!(report.zone, 2);
        assert_eq!(row.sector, 2, "only the buckets stamped with sector 2");
        assert_eq!(row.run, 7, "the run column still counts everything");
    }

    /// A base that has produced nothing shows no rows rather than rows of
    /// zeroes — the renderer says so in one line, which is the honest
    /// reading of a base that has genuinely done nothing yet.
    #[test]
    fn a_fresh_base_reports_nothing_rather_than_zeroes() {
        let mut game = game(4204);
        let report = game.base_output_report();
        assert!(report.mined.is_empty());
        assert!(report.compiled.is_empty());
    }

    /// The spec's rule: the page must **call** `Game::attention`, never
    /// compute its own answer. A fourth surface deriving it separately is
    /// exactly the drift that seam exists to prevent.
    #[test]
    fn what_needs_attention_is_the_shared_derivation() {
        let mut game = fed(4205);
        let direct = game.attention();
        let report = game.base_output_report();
        assert_eq!(
            report.attention.len(),
            direct.len(),
            "the page reports exactly what `Game::attention` says"
        );
    }
}
