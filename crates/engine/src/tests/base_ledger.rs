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
