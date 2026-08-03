//! Raids against the base — damage, shields, guards, effects, and regeneration.

use super::support::*;
use crate::tuning::{NEST_DURABILITY, RAID_DAMAGE, RAID_DEFENDER_DAMAGE, STRUCTURE_REGEN_INTERVAL};
use crate::*;

#[test]
fn raid_check_never_targets_a_nest_even_as_the_only_durability_holder() {
    let mut game = Game::new(600, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Strip every other Durability holder so a Nest would be the only
    // possible target if it weren't explicitly excluded.
    let existing: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Durability>>();
        query.iter(&game.world).collect()
    };
    for e in existing {
        game.world.despawn(e);
    }
    let nest = game
        .world
        .spawn((
            Nest {
                species: "scrapper".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 10, y: 10 },
            Glyph {
                ch: 'N',
                color: GlyphColor::Red,
            },
            Durability {
                hp: NEST_DURABILITY,
                max_hp: NEST_DURABILITY,
            },
        ))
        .id();

    for _ in 0..500 {
        game.raid_check();
    }

    assert_eq!(
        game.world.get::<Durability>(nest).unwrap().hp,
        NEST_DURABILITY,
        "a Nest must never take raid damage, even when it's the only Durability holder"
    );
}

#[test]
fn damage_structure_destroys_it_and_clears_its_cronjob_at_zero_durability() {
    let mut game = Game::new(100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 10, max_hp: 30 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    game.damage_structure(structure, 10, "Mining Node");

    assert!(
        game.world.get::<Structure>(structure).is_none(),
        "0 durability should destroy the structure"
    );
    assert!(
        game.world.get::<Task>(worker).is_none(),
        "the destroyed structure's cronjob should be cleared"
    );
}

#[test]
fn damage_structure_just_reduces_durability_when_it_survives() {
    let mut game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 20, max_hp: 30 },
        ))
        .id();

    game.damage_structure(structure, 10, "Mining Node");

    assert_eq!(game.world.get::<Durability>(structure).unwrap().hp, 10);
    assert!(
        game.world.get::<Structure>(structure).is_some(),
        "a structure with remaining durability should survive"
    );
}

#[test]
fn home_loads_as_non_raidable_and_other_structures_default_to_raidable() {
    let game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let defs = game.structure_defs();

    let home = defs
        .iter()
        .find(|d| d.id == "home")
        .expect("home should load");
    assert!(!home.raidable, "home.ron must set raidable: false");

    let mining = defs
        .iter()
        .find(|d| d.id == "mining_node")
        .expect("mining_node should load");
    assert!(
        mining.raidable,
        "a structure file that omits `raidable` must default to raidable"
    );
}

#[test]
fn deploying_home_gives_it_no_durability_pool() {
    let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    let home = find_home(&mut game).expect("place_home should have spawned a Home");

    assert!(
        game.world.get::<Durability>(home).is_none(),
        "a non-raidable structure must not carry a Durability pool at all"
    );
}

#[test]
fn deploying_a_raidable_structure_still_gives_it_a_durability_pool() {
    // Seed 300 is known to have walkable terrain at both offsets — it's
    // the seed `place_structure_rejects_anything_but_home_until_a_home_exists`
    // already places two structures on.
    let mut game = Game::new(300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, -1, 0);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 20);
    game.place_structure("mining_node", 1, 0).unwrap();

    let node = {
        let mut query = game.world.query::<(Entity, &Structure)>();
        query
            .iter(&game.world)
            .find(|(_, s)| s.kind == "mining_node")
            .map(|(e, _)| e)
            .expect("the mining node should have been deployed")
    };

    let durability = game
        .world
        .get::<Durability>(node)
        .expect("a raidable structure must still get its Durability pool");
    assert_eq!(durability.hp, durability.max_hp);
    assert!(durability.max_hp > 0);
}

#[test]
fn raid_check_never_targets_home_even_as_the_only_structure() {
    let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Strip every pre-existing Durability holder (habitat nests and
    // anything else the world seeded) so a raid has no legal target left
    // at all if Home genuinely isn't one.
    let existing: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Durability>>();
        query.iter(&game.world).collect()
    };
    for e in existing {
        game.world.despawn(e);
    }
    place_home(&mut game, -1, 0);

    for _ in 0..500 {
        game.raid_check();
    }

    let home_still_standing = {
        let mut query = game.world.query::<&Structure>();
        query.iter(&game.world).any(|s| s.kind == HOME_STRUCTURE_ID)
    };
    assert!(
        home_still_standing,
        "Home must survive every raid roll — it can't be a raid target at all"
    );
    let home = find_home(&mut game).expect("checked above: Home is standing");
    assert!(
        game.world.get::<Durability>(home).is_none(),
        "Home must still have no Durability pool after the raid rolls"
    );
}

#[test]
fn home_survives_save_and_load_without_gaining_a_durability_pool() {
    let assets = test_assets_dir();
    let mut game = Game::new(703, DifficultyMode::Forgiving, &assets).unwrap();
    place_home(&mut game, -1, 0);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_home_raidable_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let home = find_home(&mut loaded).expect("Home should survive a save/load round trip");
    assert!(
        loaded.world.get::<Durability>(home).is_none(),
        "the load path must not re-attach Durability to a non-raidable structure"
    );
}

#[test]
fn raid_check_can_damage_an_undefended_structure() {
    for seed in 0..300u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();

        for _ in 0..RAID_ATTEMPTS_PER_SEED {
            game.raid_check();

            let Some(durability) = game.world.get::<Durability>(structure) else {
                // Destroyed outright — tolerate rather than assume it can't happen.
                return;
            };
            if durability.hp < 30 {
                return;
            }
        }
    }
    panic!("raid_check never damaged the structure across 300 seeds — the raid roll may be broken");
}

#[test]
fn raid_damage_message_is_tagged_message_kind_raid() {
    for seed in 0..300u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 30, max_hp: 30 },
        ));

        for _ in 0..RAID_ATTEMPTS_PER_SEED {
            game.raid_check();

            let tagged = game
                .message_log(10)
                .into_iter()
                .any(|(kind, _)| kind == MessageKind::Raid);
            if tagged {
                return;
            }
        }
    }
    panic!(
        "raid_check never logged a MessageKind::Raid line across 300 seeds — the raid roll may be broken"
    );
}

#[test]
fn shield_structure_loads_with_no_work_and_a_raid_defense_bonus() {
    let game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "shield")
        .expect("shield.ron should load as a structure");
    assert!(
        def.work.is_none(),
        "a shield defends passively, not via cronjob work"
    );
    assert!(
        def.raid_defense > 0,
        "a shield should contribute a nonzero raid_defense bonus"
    );
}

#[test]
fn deployed_shields_reduce_raid_damage_to_an_undefended_structure() {
    for seed in 0..300u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let shield_defense = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "shield")
            .unwrap()
            .raid_defense;
        game.world.spawn((
            Structure {
                kind: "shield".to_string(),
            },
            Position { x: 1, y: 1 },
        ));
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();

        for _ in 0..RAID_ATTEMPTS_PER_SEED {
            game.raid_check();

            let Some(durability) = game.world.get::<Durability>(structure) else {
                return;
            };
            if durability.hp < 30 {
                assert_eq!(
                    durability.hp,
                    30 - (RAID_DAMAGE - shield_defense),
                    "a raid on an undefended structure should be reduced by the deployed shield's raid_defense"
                );
                return;
            }
        }
    }
    panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
}

#[test]
fn damaging_a_structure_queues_a_hit_effect_at_its_position() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 30, max_hp: 30 },
        ))
        .id();

    game.damage_structure(structure, 5, "Mining Node");

    let effects = game.take_effects();
    assert_eq!(effects.len(), 1, "one hit should queue one effect");
    assert_eq!(effects[0].kind, EffectKind::Hit);
    assert_eq!(effects[0].pos, (5, 5));
}

#[test]
fn destroying_a_structure_queues_a_destroyed_effect() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 2, y: 3 },
            Durability { hp: 4, max_hp: 30 },
        ))
        .id();

    game.damage_structure(structure, 10, "Mining Node");

    let effects = game.take_effects();
    assert_eq!(effects.len(), 1);
    assert_eq!(
        effects[0].kind,
        EffectKind::Destroyed,
        "a killing blow should queue Destroyed, not Hit"
    );
    assert_eq!(effects[0].pos, (2, 3));
}

#[test]
fn damaging_a_structure_with_no_position_queues_nothing() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Durability { hp: 30, max_hp: 30 },
        ))
        .id();

    game.damage_structure(structure, 5, "Mining Node");

    assert!(
        game.take_effects().is_empty(),
        "a flash with no known tile is worse than no flash"
    );
}

#[test]
fn a_raid_fully_absorbed_by_the_shield_network_queues_a_deflected_effect() {
    for seed in 0..300u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Enough shields that RAID_DAMAGE is reduced to zero.
        let shield_defense = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "shield")
            .unwrap()
            .raid_defense
            .max(1);
        let needed = RAID_DAMAGE.div_ceil(shield_defense);
        for _ in 0..needed {
            game.world.spawn((
                Structure {
                    kind: "shield".to_string(),
                },
                Position { x: 1, y: 1 },
            ));
        }
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();

        for _ in 0..RAID_ATTEMPTS_PER_SEED {
            game.raid_check();

            let effects = game.take_effects();
            if effects.is_empty() {
                continue;
            }
            let target = effects
                .iter()
                .find(|e| e.pos == (5, 5))
                .expect("the raid should have targeted the only durable structure");
            assert_eq!(
                target.kind,
                EffectKind::Deflected,
                "a raid the shield network zeroes out should deflect, not hit"
            );
            assert_eq!(
                game.world.get::<Durability>(structure).unwrap().hp,
                30,
                "a deflected raid should leave durability untouched"
            );
            return;
        }
    }
    panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
}

#[test]
fn a_raid_fended_off_by_a_cronjob_worker_queues_a_deflected_effect() {
    for seed in 0..300u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();
        // Defense far above RAID_DAMAGE, so the worker fully mitigates.
        game.world.spawn((
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 1,
                def: 500,
            },
            Position { x: 5, y: 5 },
            Task {
                kind: TaskKind::Guard,
                target: structure,
                progress: 0,
                required: 10,
            },
        ));

        for _ in 0..RAID_ATTEMPTS_PER_SEED {
            game.raid_check();

            let effects = game.take_effects();
            if effects.is_empty() {
                continue;
            }
            assert_eq!(effects[0].kind, EffectKind::Deflected);
            assert_eq!(effects[0].pos, (5, 5));
            assert_eq!(
                game.world.get::<Durability>(structure).unwrap().hp,
                30,
                "a fully mitigated raid should leave durability untouched"
            );
            return;
        }
    }
    panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
}

#[test]
fn take_effects_drains_the_queue() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 30, max_hp: 30 },
        ))
        .id();

    game.damage_structure(structure, 1, "Mining Node");

    assert_eq!(game.take_effects().len(), 1);
    assert!(
        game.take_effects().is_empty(),
        "a second drain should come back empty"
    );
}

#[test]
fn the_effect_queue_drops_the_oldest_effects_past_its_cap() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability {
                hp: 10_000,
                max_hp: 10_000,
            },
        ))
        .id();

    for _ in 0..(resources::EFFECT_QUEUE_CAP + 10) {
        game.damage_structure(structure, 1, "Mining Node");
    }

    assert_eq!(
        game.take_effects().len(),
        resources::EFFECT_QUEUE_CAP,
        "a frontend that never drains must not grow the queue without bound"
    );
}

#[test]
fn raid_defense_active_tracks_whether_any_shield_is_standing() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.raid_defense_active());
    game.world.spawn((
        Structure {
            kind: "shield".to_string(),
        },
        Position { x: 1, y: 1 },
    ));
    assert!(game.raid_defense_active());
}

#[test]
fn assign_guard_refuses_a_structure_that_cant_be_raided() {
    let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let home = game
        .world
        .spawn((
            Structure {
                kind: "home".to_string(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 50, 3);

    let err = game
        .assign_guard(worker, home)
        .expect_err("guarding a non-raidable structure should be refused");
    assert!(err.contains("can't be raided"), "unexpected error: {err}");
    assert!(
        game.world.get::<Task>(worker).is_none(),
        "a refused guard must not leave a Task behind"
    );
}

#[test]
fn assign_guard_defends_a_structure_with_no_work_recipe() {
    let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // A Recharger Node, not Home: Home is non-raidable now, so it is the one
    // structure a guard is refused on.
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "recharger_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 30, max_hp: 30 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 50, 3);

    game.assign_guard(worker, structure).unwrap();

    let task = game
        .world
        .get::<Task>(worker)
        .expect("guarding should assign a Task");
    assert_eq!(task.kind, TaskKind::Guard);
    assert_eq!(task.target, structure);
}

#[test]
fn a_guard_task_never_produces_resources_even_on_a_workable_node() {
    let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(ids::CORE_FRAGMENT),
                amount: 5,
                capacity: 5,
                level: None,
            },
        ))
        .id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::Guard,
        target: structure,
        progress: 0,
        required: 1,
    });

    for _ in 0..10 {
        game.tick();
    }

    assert_eq!(
        game.world.get::<ResourceNode>(structure).unwrap().amount,
        5,
        "a guard shouldn't advance the node's gather cycle at all"
    );
}

#[test]
fn guard_assignment_on_a_non_resource_structure_survives_save_and_load() {
    let mut game = Game::new(6, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // A Recharger Node, not Home: Home is non-raidable, so guarding it is refused.
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "recharger_node".to_string(),
            },
            Position { x: 3, y: 3 },
            Durability { hp: 30, max_hp: 30 },
        ))
        .id();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_guard(worker, structure).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_guard_test_{}_{}.bin",
        std::process::id(),
        6
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut query = loaded.world.query::<&Task>();
    let task = query
        .iter(&loaded.world)
        .next()
        .expect("restored creature should still have its guard assignment");
    assert_eq!(task.kind, TaskKind::Guard);
    let target_pos = loaded
        .world
        .get::<Position>(task.target)
        .expect("guard task target should resolve to the structure entity");
    assert_eq!((target_pos.x, target_pos.y), (3, 3));
}

#[test]
fn raid_check_defended_by_a_worker_reduces_structure_damage_and_hurts_the_worker() {
    for seed in 0..300u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();
        let worker = spawn_tamed(&mut game, 50, 3);
        game.world.get_mut::<Stats>(worker).unwrap().def = 100; // fully mitigates RAID_DAMAGE
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 5,
        });

        for _ in 0..RAID_ATTEMPTS_PER_SEED {
            game.raid_check();

            let worker_hp = game.world.get::<Stats>(worker).unwrap().hp;
            if worker_hp < 50 {
                // The raid rolled this attempt: the structure should be
                // untouched (fully mitigated) and the worker should have
                // taken the defender's cost.
                assert_eq!(
                    game.world.get::<Durability>(structure).unwrap().hp,
                    30,
                    "a worker with overwhelming Defense should fully mitigate the raid"
                );
                assert_eq!(worker_hp, 50 - RAID_DEFENDER_DAMAGE);
                return;
            }
        }
    }
    panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
}

/// Raids should be survivable attrition, not a countdown. Eight hits to
/// destroy a default-durability structure is the property; the exact
/// constants are free to move underneath it.
#[test]
fn a_structure_survives_seven_raids_worth_of_damage() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let durability = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "mining_node")
        .expect("mining_node.ron should load")
        .durability;
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability {
                hp: durability,
                max_hp: durability,
            },
        ))
        .id();

    for _ in 0..7 {
        game.damage_structure(structure, RAID_DAMAGE, "Mining Node");
    }

    assert!(
        game.world.get::<Durability>(structure).is_some(),
        "seven raids should not destroy a structure at full durability"
    );
}

/// Raid damage is permanent until the player builds something that repairs
/// it. There is no free healing: a base with no repairer never recovers a
/// point. If this ever fails, the Patch Node has been designed out from
/// under itself.
#[test]
fn raid_damage_is_permanent_without_a_repairer() {
    let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 30, max_hp: 30 },
        ))
        .id();

    game.damage_structure(structure, RAID_DAMAGE, "Mining Node");
    let damaged = game.world.get::<Durability>(structure).unwrap().hp;
    assert_eq!(
        damaged,
        30 - RAID_DAMAGE,
        "the raid should have landed before regen is tested"
    );

    for interval in 1..=5 {
        game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL * interval;
        game.structure_regen();
    }

    assert_eq!(
        game.world.get::<Durability>(structure).unwrap().hp,
        damaged,
        "with nothing repairing it, a damaged structure stays damaged forever"
    );
}

/// The rate every deployed Patch Node contributes, read off its own
/// definition so the test moves with the `.ron` rather than pinning a
/// number the modder is free to change.
fn patch_node_per_tier(game: &Game) -> u32 {
    game.structure_defs()
        .into_iter()
        .find(|d| d.id == "patch_node")
        .expect("patch_node.ron should load")
        .repair
        .expect("patch_node.ron should declare a repair rate")
        .per_tier
}

/// Spawns a deployed Patch Node at `tier`, the way `place_structure` and
/// `upgrade_structure` between them leave one standing.
fn spawn_patch_node(game: &mut Game, tier: u32, hp: u32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: "patch_node".to_string(),
            },
            Position { x: 7, y: 7 },
            Durability { hp, max_hp: 30 },
            StructureTier(tier),
        ))
        .id()
}

#[test]
fn a_patch_node_adds_its_tier_to_every_structures_regen() {
    let mut game = Game::new(140, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let per_tier = patch_node_per_tier(&game);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 10, max_hp: 30 },
        ))
        .id();
    spawn_patch_node(&mut game, 1, 30);
    game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

    game.structure_regen();

    assert_eq!(
        game.world.get::<Durability>(structure).unwrap().hp,
        10 + per_tier
    );
}

#[test]
fn patch_node_repair_scales_with_its_upgrade_tier() {
    let mut game = Game::new(141, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let per_tier = patch_node_per_tier(&game);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 1, max_hp: 60 },
        ))
        .id();
    spawn_patch_node(&mut game, 3, 30);
    game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

    game.structure_regen();

    assert_eq!(
        game.world.get::<Durability>(structure).unwrap().hp,
        1 + per_tier * 3,
        "a tier-3 Patch Node should repair three times a tier-1 one"
    );
}

/// Additive across nodes, the same way `raid_defense` and `pet_slot_bonus`
/// stack — building a second one is a real answer to a bigger base.
#[test]
fn patch_node_repair_stacks_across_several_nodes() {
    let mut game = Game::new(142, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let per_tier = patch_node_per_tier(&game);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 1, max_hp: 60 },
        ))
        .id();
    spawn_patch_node(&mut game, 1, 30);
    spawn_patch_node(&mut game, 2, 30);
    game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

    game.structure_regen();

    assert_eq!(
        game.world.get::<Durability>(structure).unwrap().hp,
        1 + per_tier * 3,
        "a tier-1 and a tier-2 node should contribute three tiers between them"
    );
}

#[test]
fn a_patch_node_repairs_itself() {
    let mut game = Game::new(143, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let per_tier = patch_node_per_tier(&game);
    let node = spawn_patch_node(&mut game, 1, 10);
    game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

    game.structure_regen();

    assert_eq!(
        game.world.get::<Durability>(node).unwrap().hp,
        10 + per_tier,
        "a Patch Node is not carved out of its own repair pass"
    );
}

/// A nest carries `Durability` like a structure does, and the regen pass
/// used to heal every `Durability` holder indiscriminately — so chipping a
/// nest down with bump-attacks was quietly racing its own healing. Nothing
/// the player builds maintains what spawns the raiders: a nest's Durability
/// is only ever spent, never restored.
#[test]
fn nests_do_not_regenerate_at_all() {
    let mut game = Game::new(144, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let nest = game
        .world
        .spawn((
            Nest {
                species: "glitch".to_string(),
                pending_respawns: Vec::new(),
            },
            Position { x: 20, y: 20 },
            Durability {
                hp: 5,
                max_hp: NEST_DURABILITY,
            },
        ))
        .id();
    spawn_patch_node(&mut game, 5, 30);
    game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

    game.structure_regen();

    assert_eq!(
        game.world.get::<Durability>(nest).unwrap().hp,
        5,
        "a nest gets neither the baseline trickle nor a Patch Node's repair"
    );
}

/// The Patch Node is the first structure that is upgradeable *without*
/// being cronjob-workable, so it is the first to take `upgrade_structure`'s
/// `ResourceNode` branch with no `ResourceNode` to update. Deploy, upgrade,
/// and confirm the repair rate actually followed the tier.
#[test]
fn a_deployed_patch_node_upgrades_and_repairs_harder_for_it() {
    let mut game = Game::new(146, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    unlock_research_chain(&mut game, "fortification");
    place_home(&mut game, 0, 1);
    {
        let mut inv = game
            .world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap();
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 100);
        inv.add(ItemId::from(ids::POWER_CELL), 10);
    }
    game.place_structure("patch_node", 1, 1).unwrap();
    let node = find_structure_by_kind(&mut game, "patch_node").unwrap();
    let per_tier = patch_node_per_tier(&game);

    assert_eq!(
        game.world.get::<StructureTier>(node).unwrap().0,
        1,
        "a Patch Node deploys at Mk1 even with no work recipe"
    );
    assert_eq!(game.total_repair_rate(), per_tier);

    game.upgrade_structure(node)
        .expect("a non-workable structure with an upgrade path should still upgrade");

    assert_eq!(game.world.get::<StructureTier>(node).unwrap().0, 2);
    assert_eq!(
        game.total_repair_rate(),
        per_tier * 2,
        "upgrading should raise what the node repairs, not just its Mk number"
    );
}

/// The Patch Node has to be earned, not available from turn one — it is
/// the payoff of the same node that unlocks the Shield.
#[test]
fn the_patch_node_is_gated_behind_fortification_research() {
    let mut game = Game::new(145, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        !game
            .buildable_structure_defs()
            .iter()
            .any(|d| d.id == "patch_node"),
        "the Patch Node should not be buildable before Fortification is researched"
    );

    unlock_research_chain(&mut game, "fortification");

    assert!(
        game.buildable_structure_defs()
            .iter()
            .any(|d| d.id == "patch_node"),
        "Fortification should unlock the Patch Node alongside the Shield"
    );
}

/// The shield network should ramp, not cliff: the first Shield has to
/// leave damage on the table, or `raid_defense` has drifted into
/// granting total immunity for one build.
#[test]
fn a_single_shield_reduces_raid_damage_without_erasing_it() {
    let game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let shield_defense = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "shield")
        .expect("shield.ron should load")
        .raid_defense;

    assert!(
        shield_defense > 0,
        "a Shield that reduces nothing is not a Shield"
    );
    assert!(
        shield_defense < RAID_DAMAGE,
        "one Shield must not fully absorb a raid — the network should ramp, not cliff"
    );
}

/// A repairer overshooting a nearly-full structure must clamp, not wrap.
/// Tier 5 restores far more than the 1 point missing here.
#[test]
fn structure_regen_does_not_exceed_max_durability() {
    let mut game = Game::new(103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability { hp: 29, max_hp: 30 },
        ))
        .id();
    spawn_patch_node(&mut game, 5, 30);
    game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

    game.structure_regen();

    assert_eq!(game.world.get::<Durability>(structure).unwrap().hp, 30);
}

#[test]
fn structures_survive_save_and_load_with_their_durability() {
    let assets = test_assets_dir();
    let mut game = Game::new(104, DifficultyMode::Forgiving, &assets).unwrap();
    let structure_def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "mining_node")
        .unwrap();
    game.world.spawn((
        Structure {
            kind: structure_def.id.clone(),
        },
        Position { x: 5, y: 5 },
        Durability {
            hp: 12,
            max_hp: structure_def.durability,
        },
    ));

    let path = std::env::temp_dir().join(format!(
        "feral_processes_structure_durability_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut query = loaded.world.query::<&Durability>();
    let durability = query
        .iter(&loaded.world)
        .next()
        .expect("the structure should survive a save/load round trip");
    assert_eq!(durability.hp, 12);
    assert_eq!(durability.max_hp, structure_def.durability);
}

/// Programs have no passive regen and the player is not present, so raid
/// chip damage is pure attrition: a worker left on a cronjob long enough
/// dies unattended. That is the intended cost, not an oversight.
#[test]
fn a_raid_defender_brought_to_zero_is_destroyed_rather_than_standing_down() {
    let mut game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let existing: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Durability>>();
        query.iter(&game.world).collect()
    };
    for e in existing {
        game.world.despawn(e);
    }
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 5, y: 5 },
            Durability {
                hp: 1000,
                max_hp: 1000,
            },
        ))
        .id();
    // Exactly one raid's worth of defender damage, so the first raid that
    // lands kills it and the test never depends on how many fire.
    let worker = spawn_tamed(&mut game, RAID_DEFENDER_DAMAGE, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    for _ in 0..2000 {
        game.raid_check();
        if game.world.get::<Stats>(worker).is_none() {
            break;
        }
    }

    assert!(
        game.world.get::<Stats>(worker).is_none(),
        "a defender knocked to 0 HP is destroyed, not stood down"
    );
    assert!(
        game.message_log(200)
            .iter()
            .any(|(k, l)| *k == MessageKind::Raid && l.contains("destroyed defending")),
        "the loss is reported as a Raid line, since the player wasn't there to see it"
    );
}
