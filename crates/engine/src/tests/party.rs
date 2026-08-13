//! The roster: capacity, membership, companion status, and fusing programs together.

use super::support::*;
use crate::tuning::MAX_FUSIONS;
use crate::*;

#[test]
fn pet_capacity_grows_with_each_deployed_data_cache() {
    let mut game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY);

    spawn_data_cache(&mut game, 1);
    assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY + 5);

    spawn_data_cache(&mut game, 2);
    assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY + 10, "caches stack");
}

#[test]
fn destroying_a_data_cache_shrinks_the_pet_capacity_back() {
    let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_data_cache(&mut game, 1);
    assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY + 5);

    let cache = game
        .world
        .iter_entities()
        .find(|e| e.get::<Structure>().is_some_and(|s| s.kind == "data_cache"))
        .map(|e| e.id())
        .expect("the spawned cache should be findable");
    game.world.despawn(cache);

    assert_eq!(
        game.pet_capacity(),
        BASE_PET_CAPACITY,
        "capacity is derived, so a destroyed cache needs no invalidation"
    );
}

#[test]
fn inventory_used_counts_cargo_but_not_research_data() {
    let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Starting inventory is 3 ICE Breaker + 3 Power Cell + 5 Core Fragment
    // + 2 Power Outlet.
    assert_eq!(game.inventory_used(), 13);

    grant_research_data(&mut game, 90);
    assert_eq!(
        game.inventory_used(),
        13,
        "banked research must not consume carrying capacity"
    );

    assert_eq!(game.player_status().inventory_used, 13);
}

#[test]
fn the_buffer_is_unbounded_so_cargo_actions_never_refuse_for_space() {
    let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Pile on far more cargo than the old 30-unit cap ever allowed.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 10_000);

    game.craft(&ItemId::from(ids::POWER_CELL), 1)
        .expect("compiling never runs out of Buffer space now");
    let landed = game.grant_loot(ItemId::from(ids::PORTAL_FRAGMENT), 6);
    assert_eq!(
        landed, 6,
        "every looted unit lands — the Buffer can't fill up"
    );
}

#[test]
fn set_companion_rejects_a_wild_creature() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
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
                hp: 5,
                max_hp: 5,
                atk: 1,
                def: 1,
            },
        ))
        .id();
    assert!(game.add_companion(wild).is_err());
    assert!(game.player_status().companions.is_empty());
}

#[test]
fn set_companion_clears_any_active_cronjob_task() {
    let mut game = Game::new(24, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
        ))
        .id();
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 2,
        required: 5,
    });

    game.add_companion(worker).unwrap();

    assert!(
        game.world.get::<Task>(worker).is_none(),
        "companion duty should cancel the cronjob"
    );
    assert_eq!(
        game.player_status().companions.first().map(|c| c.hp),
        Some(10)
    );
}

#[test]
fn assigning_cronjob_to_the_active_companion_clears_companion_status() {
    let assets = test_assets_dir();
    let mut game = Game::new(25, DifficultyMode::Forgiving, &assets).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.add_companion(worker).unwrap();
    assert!(!game.player_status().companions.is_empty());

    let structure_def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.work.is_some())
        .expect("at least one workable structure should exist");
    let structure = game
        .world
        .spawn((
            Structure {
                kind: structure_def.id.clone(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: structure_def.work.as_ref().unwrap().produces.clone(),
                level: None,
            },
        ))
        .id();

    game.assign_cronjob(worker, structure).unwrap();

    assert!(
        game.player_status().companions.is_empty(),
        "running a cronjob should stand the companion down"
    );
    assert!(game.world.get::<Task>(worker).is_some());
}

#[test]
fn clear_companion_reverts_to_no_companion() {
    let mut game = Game::new(26, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.add_companion(worker).unwrap();
    assert!(!game.player_status().companions.is_empty());

    game.remove_companion(worker);

    assert!(game.player_status().companions.is_empty());
}

#[test]
fn moving_a_party_member_forward_swaps_it_with_the_slot_ahead() {
    let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 11, 4);
    game.add_companion(first).unwrap();
    game.add_companion(second).unwrap();

    game.move_party_member(second, SlotShift::Forward).unwrap();

    let party = game.world.resource::<Party>().0.clone();
    assert_eq!(
        party,
        vec![second, first],
        "moving forward takes the slot ahead and pushes its occupant back"
    );

    game.move_party_member(second, SlotShift::Back).unwrap();
    assert_eq!(
        game.world.resource::<Party>().0,
        vec![first, second],
        "moving back undoes it"
    );
}

#[test]
fn a_party_member_at_either_end_cannot_move_past_it() {
    let mut game = Game::new(28, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 11, 4);
    game.add_companion(first).unwrap();
    game.add_companion(second).unwrap();

    assert!(game.move_party_member(first, SlotShift::Forward).is_err());
    assert!(game.move_party_member(second, SlotShift::Back).is_err());
    assert_eq!(
        game.world.resource::<Party>().0,
        vec![first, second],
        "a refused move leaves the order untouched"
    );
}

#[test]
fn a_program_outside_the_party_has_no_slot_to_move() {
    let mut game = Game::new(29, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let member = spawn_tamed(&mut game, 10, 3);
    let bystander = spawn_tamed(&mut game, 11, 4);
    game.add_companion(member).unwrap();

    assert!(
        game.move_party_member(bystander, SlotShift::Forward)
            .is_err()
    );
}

#[test]
fn party_order_is_frozen_during_a_battle() {
    let mut game = Game::new(30, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 11, 4);
    game.add_companion(first).unwrap();
    game.add_companion(second).unwrap();

    let player = game.player_entity();
    let enemy = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![enemy]);

    // `BattleState::planned` indexes `Party` positionally, so a mid-battle
    // swap would point two slots' planned actions at each other's actor.
    assert!(game.move_party_member(second, SlotShift::Forward).is_err());
    assert_eq!(game.world.resource::<Party>().0, vec![first, second]);
}

#[test]
fn owned_pets_lists_the_party_first_in_slot_order() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let bystander = spawn_tamed(&mut game, 9, 2);
    let first = spawn_tamed(&mut game, 10, 3);
    let second = spawn_tamed(&mut game, 11, 4);
    game.add_companion(first).unwrap();
    game.add_companion(second).unwrap();

    let slots: Vec<Option<u32>> = game.owned_pets().iter().map(|p| p.party_slot).collect();
    assert_eq!(
        slots,
        vec![Some(0), Some(1), None],
        "the roster leads with the party in slot order so a frontend's row \
         numbering matches the battle line"
    );

    game.move_party_member(second, SlotShift::Forward).unwrap();
    let order: Vec<Entity> = game.owned_pets().iter().map(|p| p.entity).collect();
    assert_eq!(order, vec![second, first, bystander]);
}

/// Slot order is mechanical — the companion screen's `<`/`>` move a member
/// along the battle line and the number beside it is the whole point of that
/// screen — so the party keeps it. Everything *behind* the party has no slot
/// to show and used to arrive in bevy query order, which is to say in no
/// order at all, across the fuse, extract, routines and manifest pickers that
/// read the same list.
#[test]
fn owned_pets_sorts_everything_behind_the_party_by_name() {
    let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let member = spawn_tamed(&mut game, 9, 2);
    let zeta = spawn_tamed(&mut game, 9, 2);
    let alpha = spawn_tamed(&mut game, 9, 2);
    for (entity, name) in [(member, "Middle"), (zeta, "Zeta"), (alpha, "Alpha")] {
        game.world
            .entity_mut(entity)
            .insert(CustomName(name.to_string()));
    }
    game.add_companion(member).unwrap();

    let names: Vec<String> = game.owned_pets().into_iter().map(|p| p.name).collect();
    assert_eq!(
        names,
        vec!["Middle", "Alpha", "Zeta"],
        "the party member leads on its slot, the rest sort by name"
    );
}

/// The roster lists programs the player never opens a gear screen for — a
/// posted worker, a bench-warmer — so "is this one kitted out" has to be
/// readable from the list itself rather than three keypresses deep.
#[test]
fn a_roster_row_marks_which_gear_slots_are_filled() {
    let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);

    let tag = |game: &mut Game| {
        game.owned_pets()
            .into_iter()
            .find(|p| p.entity == pet)
            .map(|p| p.gear)
            .unwrap()
    };
    assert_eq!(tag(&mut game), ".|.|.", "a bare program fills nothing");

    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    let module = ItemId::from(ids::NEURAL_AMPLIFIER);
    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let player = game.player_entity();
    for item in [&weapon, &module, &armor] {
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(item.clone(), 1);
    }

    game.equip(pet, &gear(&weapon, 0)).unwrap();
    game.equip(pet, &gear(&module, 0)).unwrap();
    assert_eq!(
        tag(&mut game),
        "w|.|m",
        "a gap keeps its place so the slots line up down the list"
    );

    game.equip(pet, &gear(&armor, 0)).unwrap();
    assert_eq!(tag(&mut game), "w|a|m");
}

/// The status panel lists the party too, and the two screens read the same
/// program at the same moment. One formatter, or they disagree.
#[test]
fn the_status_panel_reads_a_loadout_the_same_way_the_roster_does() {
    let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.add_companion(pet).unwrap();

    let armor = ItemId::from(ids::ABLATIVE_PLATING);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(armor.clone(), 1);
    game.equip(pet, &gear(&armor, 0)).unwrap();

    let roster = game
        .owned_pets()
        .into_iter()
        .find(|p| p.entity == pet)
        .unwrap()
        .gear;
    let panel = game.player_status().companions[0].gear.clone();
    assert_eq!(roster, ".|a|.");
    assert_eq!(panel, roster, "both screens read one loadout");
}

#[test]
fn owned_pets_reports_every_owned_creature_regardless_of_location_or_job() {
    let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    let far_worker = spawn_tamed(&mut game, 12, 4);
    game.world
        .entity_mut(far_worker)
        .insert(Position { x: 500, y: 500 });
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 500, y: 501 },
        ))
        .id();
    game.world.entity_mut(far_worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: structure,
        progress: 1,
        required: 5,
    });

    let idle = spawn_tamed(&mut game, 5, 2);
    game.world
        .entity_mut(idle)
        .insert(Position { x: 999, y: 999 });

    let pets = game.owned_pets();
    assert_eq!(
        pets.len(),
        3,
        "every owned tamed creature should be reported, wherever it is"
    );

    let companion_info = pets.iter().find(|p| p.entity == companion).unwrap();
    assert_eq!(companion_info.party_slot, Some(0));
    assert_eq!(companion_info.activity, "in party");

    let worker_info = pets.iter().find(|p| p.entity == far_worker).unwrap();
    assert_eq!(worker_info.party_slot, None);
    assert_ne!(
        worker_info.activity, "idle",
        "a far-off cronjob worker should still be reported as working"
    );
    assert_eq!(worker_info.hp, 12);
    assert_eq!(worker_info.atk, 4);

    let idle_info = pets.iter().find(|p| p.entity == idle).unwrap();
    assert_eq!(idle_info.party_slot, None);
    assert_eq!(idle_info.activity, "idle");
}

#[test]
fn fuse_companions_averages_the_parents_potential() {
    let mut game = Game::new(422, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs();
    let species_a = species[0].id.clone();
    let species_b = species[1 % species.len()].id.clone();

    let a = game
        .world
        .spawn((
            Creature { species: species_a },
            Position { x: 3, y: 3 },
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 10,
                def: 4,
            },
            Potential {
                hp_roll: 0.8,
                atk_roll: 0.8,
                def_roll: 0.8,
                growth_roll: 0.8,
            },
            Tamed { owner: player },
            Experience {
                level: 5,
                xp: 3,
                xp_to_next: 100,
            },
        ))
        .id();
    let b = game
        .world
        .spawn((
            Creature { species: species_b },
            Position { x: 4, y: 4 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 6,
                def: 2,
            },
            Potential {
                hp_roll: 1.2,
                atk_roll: 1.2,
                def_roll: 1.2,
                growth_roll: 1.2,
            },
            Tamed { owner: player },
            Experience {
                level: 2,
                xp: 1,
                xp_to_next: 40,
            },
        ))
        .id();

    game.fuse_companions(a, b, None).unwrap();

    let mut query = game.world.query::<(&Potential, &Tamed)>();
    let (potential, _) = query
        .iter(&game.world)
        .find(|(_, t)| t.owner == player)
        .expect("a fused creature should exist");
    assert_eq!(
        potential.hp_roll, 1.0,
        "fused rolls should average the two parents'"
    );
    assert_eq!(potential.growth_roll, 1.0);
}

#[test]
fn a_creatures_potential_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(423, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    let species = game.species_defs().into_iter().next().unwrap();
    game.world.spawn((
        Creature {
            species: species.id.clone(),
        },
        Position { x: 3, y: 3 },
        Stats {
            hp: 10,
            max_hp: 10,
            atk: 1,
            def: 1,
        },
        Potential {
            hp_roll: 1.15,
            atk_roll: 0.85,
            def_roll: 1.05,
            growth_roll: 1.2,
        },
        Tamed { owner: player },
        Experience::default(),
    ));

    let path = std::env::temp_dir().join(format!(
        "feral_processes_potential_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut query = loaded.world.query::<(&Potential, &Tamed)>();
    let (potential, _) = query
        .iter(&loaded.world)
        .find(|(_, t)| t.owner == player)
        .expect("restored creature should still have its Potential");
    assert_eq!(potential.hp_roll, 1.15);
    assert_eq!(potential.atk_roll, 0.85);
    assert_eq!(potential.def_roll, 1.05);
    assert_eq!(potential.growth_roll, 1.2);
}

#[test]
fn a_knocked_out_companion_stands_down_once_the_battle_ends() {
    let species_id = {
        let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.species_defs()
            .into_iter()
            .next()
            .expect("at least one species")
            .id
            .clone()
    };

    // The companion-targeting roll is 30% per call; a 1-HP companion is
    // guaranteed to hit 0 the moment it's targeted (damage is always
    // >= 1). Across 60 seeds the odds of never once rolling the
    // companion are astronomically small, so this deterministically
    // exercises the knockout path without needing to fake the RNG.
    for seed in 0..60u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 1, 1);
        game.add_companion(companion).unwrap();

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1000,
                    max_hp: 1000,
                    atk: 50,
                    def: 0,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        game.wild_retaliate(wild, 0, player);
        if game.world.get::<Stats>(companion).unwrap().hp == 0 {
            // It keeps its place while the fight runs: `planned` indexes
            // `Party` positionally, so removing it here would shift every
            // member behind it into the wrong slot.
            assert_eq!(
                game.player_status().companions.len(),
                1,
                "a downed companion holds its slot until the battle ends"
            );
            flee_until_clear(&mut game);
            assert!(
                game.player_status().companions.is_empty(),
                "ending the battle should have stood the downed companion down"
            );
            assert!(
                game.world.get::<Stats>(companion).is_none(),
                "a companion that hit 0 HP is deleted, not merely stood down"
            );
            assert!(
                !game.owned_pets().iter().any(|p| p.entity == companion),
                "and it is gone from the roster, not just the party"
            );
            return;
        }
    }
    panic!("companion was never targeted across 60 seeds — retaliation roll may be broken");
}

#[test]
fn companion_status_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(28, DifficultyMode::Forgiving, &assets).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.add_companion(worker).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_companion_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let status = loaded.player_status();
    assert!(
        !status.companions.is_empty(),
        "the active companion should survive a save/load round trip"
    );
}

/// `fuse_companions` used to hardcode `ZonePortal(1)` on the result, which
/// was harmless while that field was a display tag. It stopped being harmless
/// the moment a Recompile Kernel multiplied current stats and capped itself
/// against that field: fusing carries the parents' stats forward, so resetting
/// the tier that bounds them makes fuse → bump → fuse an unbounded stat loop.
///
/// The same argument covers `Refactors`, one level down — a fusion must not
/// launder a program that has spent all five slots back into a fresh one.
#[test]
fn fusing_two_bumped_programs_keeps_the_higher_tier() {
    let mut game = Game::new(95, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(a)
        .insert((ZonePortal(4), Refactors(5), PurchasedTiers(3)));
    game.world
        .entity_mut(b)
        .insert((ZonePortal(2), Refactors(1), PurchasedTiers(0)));

    game.fuse_companions(a, b, None).unwrap();

    let mut pets = game.world.query_filtered::<Entity, With<Tamed>>();
    let fused = pets.iter(&game.world).next().expect("one program remains");
    assert_eq!(
        game.world.get::<ZonePortal>(fused).map(|z| z.0),
        Some(4),
        "a fusion must not reset the tier its own stats were built at"
    );
    assert_eq!(
        game.world.get::<Refactors>(fused).copied(),
        Some(Refactors(5)),
        "nor hand back the upgrade slots the parents had already spent"
    );
    assert_eq!(
        game.world.get::<PurchasedTiers>(fused).copied(),
        Some(PurchasedTiers(3)),
        "nor relabel bought tiers as earned ones, which is what a trader pays for"
    );
}

/// Both halves of a refactor are permanent and both bound future ones — the
/// slot count against `MAX_COMPANION_REFACTORS`, the zone tier against the
/// player's own. A round trip that dropped either would hand back a fresh
/// budget of upgrades on every reload, which is the same free-fusions hole
/// `fusions` was persisted to close.
#[test]
fn a_refactored_companion_keeps_its_slots_and_tier_across_a_save() {
    let assets = test_assets_dir();
    let mut game = Game::new(29, DifficultyMode::Forgiving, &assets).unwrap();
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(worker)
        .insert((Refactors(3), ZonePortal(4), PurchasedTiers(2)));

    let dir = scratch_assets_dir("refactor_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();

    let mut pets = loaded.world.query_filtered::<Entity, With<Tamed>>();
    let pet = pets.iter(&loaded.world).next().expect("the program loaded");
    assert_eq!(
        loaded.world.get::<Refactors>(pet).copied(),
        Some(Refactors(3)),
        "the spent upgrade slots have to survive, or a reload refills them"
    );
    assert_eq!(
        loaded.world.get::<ZonePortal>(pet).map(|z| z.0),
        Some(4),
        "and so does the tier the bump raised it to"
    );
    assert_eq!(
        loaded.world.get::<PurchasedTiers>(pet).copied(),
        Some(PurchasedTiers(2)),
        "and which of those tiers were bought — dropping it would let a \
         save/load launder a bought-up program into an earned one"
    );
}

#[test]
fn party_accepts_up_to_max_party_size_and_rejects_beyond_that() {
    let mut game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let members: Vec<Entity> = (0..MAX_PARTY_SIZE)
        .map(|_| spawn_tamed(&mut game, 10, 3))
        .collect();
    for &m in &members {
        game.add_companion(m).unwrap();
    }
    assert_eq!(game.player_status().companions.len(), MAX_PARTY_SIZE);

    let one_too_many = spawn_tamed(&mut game, 10, 3);
    assert!(
        game.add_companion(one_too_many).is_err(),
        "adding a 4th member to a full 3-slot party should fail"
    );
    assert_eq!(game.player_status().companions.len(), MAX_PARTY_SIZE);
}

#[test]
fn pet_count_tallies_every_owned_program_regardless_of_party_membership() {
    let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(game.pet_count(), 0);
    let a = spawn_tamed(&mut game, 10, 3);
    let _b = spawn_tamed(&mut game, 10, 3);
    assert_eq!(game.pet_count(), 2, "both owned programs count as pets");
    // Adding one to the active party doesn't change the total owned.
    game.add_companion(a).unwrap();
    assert_eq!(game.pet_count(), 2, "a party member is still a pet");
}

#[test]
fn taming_is_refused_when_the_roster_is_full_and_a_data_cache_makes_room() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Fill the base roster of 3 owned pets.
    for _ in 0..BASE_PET_CAPACITY {
        spawn_tamed(&mut game, 10, 3);
    }
    assert_eq!(game.pet_count(), BASE_PET_CAPACITY);

    start_battle_with_a_wild_program(&mut game);
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);

    // A full roster greys the row, so `battle_set_action` refuses it before
    // a round can ever resolve — the refusal is this `Err`, not a logged
    // line.
    let index = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the player starts with decompile installed")
        .index;
    let err = game
        .battle_set_action(
            0,
            BattleAction::Special {
                ability: index,
                target: battle::SpecialTarget::EnemyGroup { group: 0 },
            },
        )
        .unwrap_err();
    assert!(
        err.contains("roster is full"),
        "the refusal should say the roster is full, got: {err}"
    );

    let held = |g: &Game| {
        g.world
            .get::<Inventory>(g.player_entity())
            .unwrap()
            .count(&ItemId::from(ids::ICE_BREAKER))
    };
    assert_eq!(
        held(&game),
        1,
        "a full roster must refuse before the catalyst is spent"
    );

    // A Data Cache raises the cap to 8, so the same attempt is accepted
    // rather than refused.
    spawn_data_cache(&mut game, 1);
    assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY + 5);
    game.battle_set_action(
        0,
        BattleAction::Special {
            ability: index,
            target: battle::SpecialTarget::EnemyGroup { group: 0 },
        },
    )
    .expect("with a cache deployed the roster has room, so the action must be accepted");
    // Deliberately asserts on the acceptance rather than on the catalyst
    // being spent. Whether the attempt actually resolves depends on the
    // round: the wild acts first, and a stun costs the player the turn
    // before the decompile ever runs. That made this assertion a coin flip
    // on the RNG sequence, which any unrelated change to the number of
    // rolls could — and did — flip.
    assert_eq!(
        held(&game),
        1,
        "planning alone must not spend the catalyst — that happens on resolve"
    );
}

#[test]
fn adding_the_same_companion_twice_is_rejected() {
    let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    assert!(
        game.add_companion(companion).is_err(),
        "a program already in the party can't be added again"
    );
    assert_eq!(game.player_status().companions.len(), 1);
}

#[test]
fn removing_one_party_member_leaves_the_others_active() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.add_companion(a).unwrap();
    game.add_companion(b).unwrap();

    game.remove_companion(a);

    assert_eq!(game.player_status().companions.len(), 1);
    assert!(
        game.player_status()
            .companions
            .first()
            .is_some_and(|c| c.hp == 10)
    );
    assert!(!game.world.resource::<Party>().0.contains(&a));
    assert!(game.world.resource::<Party>().0.contains(&b));
}

#[test]
fn party_members_grant_a_passive_ten_percent_atk_def_bonus_that_stacks_updates_live_and_disappears_on_removal()
 {
    let mut game = Game::new(75, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let base_atk = game.player_status().atk;
    let base_def = game.player_status().def;

    // `spawn_tamed` fixes def at 1, so 10% of it floors to 0 and should
    // clamp up to the stated minimum of 1 rather than contributing 0.
    let a = spawn_tamed(&mut game, 10, 30);
    game.add_companion(a).unwrap();
    let status = game.player_status();
    assert_eq!(status.atk, base_atk + 3, "10% of a's 30 ATK is 3");
    assert_eq!(
        status.def,
        base_def + 1,
        "10% of a's 1 DEF floors to 0, minimum 1 applies"
    );

    // A second party member's bonus stacks on top of the first's.
    let b = spawn_tamed(&mut game, 10, 50);
    game.add_companion(b).unwrap();
    let status = game.player_status();
    assert_eq!(
        status.atk,
        base_atk + 3 + 5,
        "10% of b's 50 ATK is 5, stacked with a's"
    );
    assert_eq!(status.def, base_def + 1 + 1);

    // The bonus is computed live from each companion's current Stats,
    // not baked in at add_companion time — a level-up (simulated here
    // by mutating Stats directly, same as `progression::add_xp` would)
    // should be reflected immediately with no extra bookkeeping.
    game.world.get_mut::<Stats>(a).unwrap().atk = 60;
    let status = game.player_status();
    assert_eq!(
        status.atk,
        base_atk + 6 + 5,
        "a's stronger ATK should raise its contribution"
    );

    game.remove_companion(a);
    game.remove_companion(b);
    let status = game.player_status();
    assert_eq!(
        status.atk, base_atk,
        "bonus should vanish once every companion leaves the party"
    );
    assert_eq!(status.def, base_def);
}

#[test]
fn dropping_below_half_power_weakens_the_players_attack() {
    let mut game = Game::new(76, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let full_atk = game.player_status().atk;

    // At and above the threshold, no penalty at all.
    game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
    assert_eq!(
        game.player_status().atk,
        full_atk,
        "50 power is still full strength"
    );

    // Below it, a linear falloff — checked at a couple of points rather
    // than re-deriving the formula, since `battle::power_attack_multiplier`
    // already has its own dedicated unit tests for the exact curve.
    game.world.get_mut::<Needs>(player).unwrap().hunger = 25.0;
    let quarter_power_atk = game.player_status().atk;
    assert!(
        quarter_power_atk < full_atk,
        "attack should be weaker at 25 power than at full power"
    );

    game.world.get_mut::<Needs>(player).unwrap().hunger = 0.0;
    let zero_power_atk = game.player_status().atk;
    assert!(
        zero_power_atk < quarter_power_atk,
        "attack should keep weakening as power keeps dropping"
    );
    assert_eq!(
        zero_power_atk,
        (full_atk as f32 * 0.5).round() as i32,
        "the penalty floors at half strength, even fully starved"
    );
}

#[test]
fn a_special_is_refused_for_a_program_not_in_the_party() {
    let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let not_in_party = spawn_tamed(&mut game, 10, 20);

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
            Position { x: 5, y: 5 },
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 1,
                def: 0,
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    companion_uses_special(
        &mut game,
        not_in_party,
        0,
        battle::SpecialTarget::Ally { slot: 0 },
    );

    let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
    assert_eq!(
        wild_hp, 100,
        "a program outside the active party shouldn't be able to act in battle"
    );
}

#[test]
fn fuse_companions_combines_stats_and_keeps_the_higher_level_species() {
    let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let species = game.species_defs();
    let species_a = species[0].id.clone();
    let species_b = species[1 % species.len()].id.clone();

    let a = game
        .world
        .spawn((
            Creature { species: species_a },
            Position { x: 3, y: 3 },
            Stats {
                hp: 20,
                max_hp: 20,
                atk: 10,
                def: 4,
            },
            Tamed { owner: player },
            Experience {
                level: 5,
                xp: 3,
                xp_to_next: 100,
            },
        ))
        .id();
    let b = game
        .world
        .spawn((
            Creature {
                species: species_b.clone(),
            },
            Position { x: 4, y: 4 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 6,
                def: 2,
            },
            Tamed { owner: player },
            Experience {
                level: 2,
                xp: 1,
                xp_to_next: 40,
            },
        ))
        .id();

    game.fuse_companions(a, b, None).unwrap();

    assert!(
        game.world.get::<Creature>(a).is_none(),
        "the first input should be consumed"
    );
    assert!(
        game.world.get::<Creature>(b).is_none(),
        "the second input should be consumed"
    );

    let mut query = game
        .world
        .query::<(&Creature, &Stats, &Experience, &Tamed)>();
    let (creature, stats, exp, _) = query
        .iter(&game.world)
        .find(|(_, _, _, t)| t.owner == player)
        .expect("a fused creature should exist");
    assert_eq!(
        exp.level, 5,
        "fusion should keep the higher level (ties favor `a`)"
    );
    assert_eq!(exp.xp, 0);
    assert_eq!(exp.xp_to_next, progression::xp_for_level(5));
    assert_eq!(
        stats.max_hp,
        20 + 10 / 2,
        "fused HP should be higher + lower/2"
    );
    assert_eq!(stats.atk, 10 + 6 / 2);
    assert_eq!(stats.def, 4 + 2 / 2);
    assert_ne!(
        creature.species, species_b,
        "the lower-level input's species shouldn't win the tie"
    );
}

#[test]
fn fuse_companions_applies_a_custom_name_truncated_to_the_max_length() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.fuse_companions(a, b, Some("Way Too Long A Name".to_string()))
        .unwrap();

    let fused = game.owned_pets();
    assert_eq!(
        fused.len(),
        1,
        "fusing two owned programs should leave exactly one"
    );
    // PetInfo::name is zone-tagged (every fused program gets
    // `ZonePortal(1)`, always shown per `entity_label`'s own test
    // coverage), so strip that " 1" suffix before checking the
    // truncated custom name itself.
    let base_name = fused[0]
        .name
        .strip_suffix(" 1")
        .expect("a freshly fused program should be zone-tagged");
    assert_eq!(
        base_name.chars().count(),
        MAX_CUSTOM_NAME_LEN,
        "an overlong custom name should be truncated, not rejected"
    );
    assert!(
        "Way Too Long A Name".starts_with(base_name),
        "the truncated name should be a prefix of what was typed, got {base_name:?}"
    );
}

#[test]
fn fuse_companions_with_no_name_or_blank_name_keeps_the_species_name() {
    let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // `spawn_tamed` always uses this same species, and fusing two
    // same-level, same-species programs keeps it — capturing it directly
    // here avoids having to pick the fused entity back out of a world that
    // also has 14 unrelated wild creatures in it from `Game::new`.
    let species_name = generic_species().name;
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.fuse_companions(a, b, None).unwrap();
    let no_name = game.owned_pets();
    assert_eq!(no_name.len(), 1);
    // Every fused program gets `ZonePortal(1)` (see `fuse_companions`),
    // which `creature_label`/`PetInfo::name` always zone-tags — even at
    // zone 1, per `entity_label`'s own test coverage — so the expected
    // fallback name carries that same " 1" suffix, not the bare species name.
    let expected_default_name = format!("{species_name} 1");
    assert_eq!(
        no_name[0].name, expected_default_name,
        "no name given should fall back to the (zone-tagged) species name"
    );

    let c = spawn_tamed(&mut game, 10, 3);
    let d = spawn_tamed(&mut game, 10, 3);
    game.fuse_companions(c, d, Some("   ".to_string())).unwrap();
    let pets = game.owned_pets();
    let blank_named = pets.iter().find(|p| p.entity != no_name[0].entity).unwrap();
    assert_eq!(
        blank_named.name, expected_default_name,
        "an all-whitespace name should also fall back to the species name, not become blank"
    );
}

#[test]
fn a_fused_programs_custom_name_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(92, DifficultyMode::Forgiving, &assets).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.fuse_companions(a, b, Some("Zappy".to_string()))
        .unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_fuse_name_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let pets = loaded.owned_pets();
    assert_eq!(pets.len(), 1);
    // Zone-tagged the same as any other fused program — see the
    // truncation test above for why " 1" is expected here too.
    assert_eq!(
        pets[0].name, "Zappy 1",
        "a custom name should survive a save/load round trip"
    );
}

#[test]
fn fuse_companions_rejects_fusing_a_program_with_itself() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    assert!(game.fuse_companions(a, a, None).is_err());
}

#[test]
fn fuse_companions_rejects_a_wild_creature() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let species = game.species_defs().into_iter().next().unwrap();
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 5,
                max_hp: 5,
                atk: 1,
                def: 1,
            },
        ))
        .id();
    assert!(game.fuse_companions(a, wild, None).is_err());
    assert!(
        game.world.get::<Creature>(a).is_some(),
        "a failed fusion shouldn't consume either input"
    );
    assert!(game.world.get::<Creature>(wild).is_some());
}

/// Regression for I2: `fuse_companions` derives the result's kit fresh from
/// its species, so a routine installed manually on either input (research,
/// extraction, a swap) has nowhere to land. Before this fix that vanished
/// with no message; it must now show up as a logged loss.
#[test]
fn fusing_a_program_logs_a_manually_installed_routine_as_lost() {
    let mut game = Game::new(103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, a, 4); // two slots, one free alongside the fallback
    install_routine_for_test(&mut game, a, "sandbox");

    assert_eq!(
        game.fusion_routine_losses(a, b)
            .iter()
            .filter(|def| def.id == "sandbox")
            .count(),
        1,
        "the preview should name the routine about to be lost"
    );

    game.fuse_companions(a, b, None).unwrap();

    let fused = game.owned_pets();
    assert_eq!(fused.len(), 1);
    assert!(
        game.actor_abilities(fused[0].entity)
            .iter()
            .all(|def| def.id != "sandbox"),
        "neither generic-species input declares sandbox innately, so it must not survive"
    );
    assert!(
        game.message_log(10)
            .iter()
            .any(|e| e.text.contains("Routines lost in the fusion")
                && e.text.contains("Bastion Single")),
        "the loss must be logged, not silent: {:?}",
        game.message_log(10)
    );
}

#[test]
fn fuse_companions_removes_fused_members_from_the_active_party() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.add_companion(a).unwrap();
    game.add_companion(b).unwrap();

    game.fuse_companions(a, b, None).unwrap();

    assert!(!game.world.resource::<Party>().0.contains(&a));
    assert!(!game.world.resource::<Party>().0.contains(&b));
}

#[test]
fn fusing_two_fresh_programs_gives_a_result_one_fusion_deep() {
    let mut game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    assert_eq!(game.fusion_count(a), 0, "a caught program starts unfused");

    game.fuse_companions(a, b, None).unwrap();

    let pets = game.owned_pets();
    assert_eq!(pets.len(), 1);
    assert_eq!(pets[0].fusions, 1);
}

#[test]
fn a_fusion_result_is_one_deeper_than_its_deepest_input() {
    let mut game = Game::new(102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let deep = fuse_to_depth(&mut game, 2);
    let fresh = spawn_tamed(&mut game, 10, 3);
    assert_eq!(game.fusion_count(deep), 2);

    game.fuse_companions(deep, fresh, None).unwrap();

    let result = game
        .owned_pets()
        .into_iter()
        .max_by_key(|p| p.fusions)
        .unwrap();
    assert_eq!(
        result.fusions, 3,
        "depth should follow the deeper parent, not the sum of both"
    );
}

#[test]
fn fuse_companions_rejects_a_program_already_at_the_fusion_cap() {
    let mut game = Game::new(103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let maxed = fuse_to_depth(&mut game, MAX_FUSIONS);
    assert_eq!(game.fusion_count(maxed), MAX_FUSIONS);
    let fresh = spawn_tamed(&mut game, 10, 3);
    let owned_before = game.owned_pets().len();

    assert!(
        game.fuse_companions(maxed, fresh, None).is_err(),
        "a maxed-out program shouldn't be usable as a fusion input"
    );
    // ...in either slot.
    assert!(game.fuse_companions(fresh, maxed, None).is_err());

    assert_eq!(
        game.owned_pets().len(),
        owned_before,
        "a rejected fusion shouldn't consume either input"
    );
}

#[test]
fn fusion_depth_survives_a_save_load_round_trip() {
    let mut game = Game::new(104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let maxed = fuse_to_depth(&mut game, MAX_FUSIONS);
    game.add_companion(maxed).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_processes_fusion_cap_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .owned_pets()
        .into_iter()
        .max_by_key(|p| p.fusions)
        .expect("the fused program should survive the round trip");
    assert_eq!(
        restored.fusions, MAX_FUSIONS,
        "a maxed lineage must stay maxed across a save, not reset to fusable"
    );
}

/// Nothing drops. A program's routines die with it — the only way to get a
/// routine back off a program is `extract_routine` at a bench, and that
/// destroys the program deliberately.
#[test]
fn a_companion_killed_in_battle_teaches_none_of_its_routines() {
    let assets = test_assets_dir();
    let mut game = Game::new(5150, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["priority_boost".to_string()]));
    game.add_companion(companion).unwrap();

    assert!(
        !game.knows_routine("priority_boost"),
        "the fixture must start with the routine unknown for the assert below to mean anything"
    );

    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.apply_damage(companion, 10);
    flee_until_clear(&mut game);

    assert!(
        !game.knows_routine("priority_boost"),
        "a dead program's routines die with it — only extraction teaches"
    );
}

/// The reap has to run before `retain_outcomes_since_battle`, or the
/// `Info`-kind detachment lines `dissolve_tamed_program` writes ("leaves
/// your battle party") would survive and trail the death line onto the map.
#[test]
fn only_the_outcome_death_line_follows_the_player_out_of_the_battle() {
    let assets = test_assets_dir();
    let mut game = Game::new(5151, DifficultyMode::Forgiving, &assets).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    let name = game.creature_label(companion);

    // A real `start_battle`, not `insert_battle`: only the former calls
    // `MessageLog::open_battle`, and without that mark
    // `retain_outcomes_since_battle` returns early and prunes nothing — the
    // very behaviour this test exists to pin.
    start_battle_with_a_wild_program(&mut game);
    game.apply_damage(companion, 10);
    flee_until_clear(&mut game);

    let log = game.message_log(40);
    assert!(
        log.iter()
            .any(|e| e.kind == MessageKind::Outcome && e.text.contains("deleted for good")),
        "the death line survives the end of the battle"
    );
    assert!(
        !log.iter()
            .any(|e| e.text.contains(&name) && e.text.contains("leaves your battle party")),
        "the dissolve's departure chatter must be pruned, not trail the death line"
    );
}

#[test]
fn rename_companion_sets_the_display_name() {
    let mut game = Game::new(4201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);

    game.rename_companion(pet, Some("Hexed".to_string()))
        .unwrap();

    assert_eq!(game.creature_name(pet).as_deref(), Some("Hexed"));
}

#[test]
fn renaming_with_a_blank_name_restores_the_species_name() {
    let mut game = Game::new(4202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species_name = generic_species().name;
    let pet = spawn_tamed(&mut game, 10, 3);
    game.rename_companion(pet, Some("Hexed".to_string()))
        .unwrap();

    // Blank is the only way back to the species name, so it clears rather
    // than being refused as empty input — `sanitize_custom_name` returns
    // `None` for it and this caller reads that as "drop the override".
    game.rename_companion(pet, Some("   ".to_string())).unwrap();

    assert_eq!(
        game.creature_name(pet).as_deref(),
        Some(species_name.as_str()),
        "a blank rename should fall back to the species name"
    );
}

#[test]
fn rename_companion_trims_and_truncates_the_way_fusion_does() {
    let mut game = Game::new(4203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);

    game.rename_companion(pet, Some("  Way Too Long A Name  ".to_string()))
        .unwrap();

    let name = game.creature_name(pet).expect("a tamed program has a name");
    assert_eq!(
        name.chars().count(),
        MAX_CUSTOM_NAME_LEN,
        "an overlong name should be truncated, not rejected"
    );
    assert!(
        "Way Too Long A Name".starts_with(&name),
        "leading whitespace should be trimmed before truncating, got {name:?}"
    );
}

#[test]
fn rename_companion_refuses_a_program_you_do_not_own() {
    let mut game = Game::new(4204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);

    assert!(
        game.rename_companion(wild, Some("Hexed".to_string()))
            .is_err(),
        "only a program compiled under your control can be renamed"
    );
}

#[test]
fn custom_name_reports_only_a_name_the_player_chose() {
    let mut game = Game::new(4205, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);

    assert_eq!(
        game.custom_name(pet),
        None,
        "an unnamed program reports no custom name, not its species name"
    );

    game.rename_companion(pet, Some("Hexed".to_string()))
        .unwrap();
    assert_eq!(game.custom_name(pet).as_deref(), Some("Hexed"));
}

#[test]
fn a_renamed_program_keeps_its_name_across_a_save() {
    let mut game = Game::new(4206, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.add_companion(pet).unwrap();
    game.rename_companion(pet, Some("Hexed".to_string()))
        .unwrap();

    let dir = scratch_assets_dir("rename_save");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("save.bin");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();

    let names: Vec<String> = loaded.owned_pets().iter().map(|p| p.name.clone()).collect();
    // Zone-tagged only on the far side: `spawn_tamed` grants no
    // `ZonePortal`, but `Game::load` restores one from `CreatureSave::zone`
    // for every creature. The tag is the fixture's, the name is the point.
    assert_eq!(
        names,
        vec!["Hexed 1".to_string()],
        "the rename is what `CreatureSave::custom_name` is for"
    );
}

#[test]
fn fusing_two_shinies_keeps_the_higher_rarity() {
    let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(a).insert(Rarity::Silver);
    game.world.entity_mut(b).insert(Rarity::Gold);

    game.fuse_companions(a, b, None).unwrap();

    let fused = game.owned_pets();
    assert_eq!(fused.len(), 1);
    assert_eq!(
        fused[0].rarity,
        Rarity::Gold,
        "fusing must not launder an Overclocked program into a lesser tier"
    );
}

/// The tier is a tag on a fusion, not a fresh multiplier: `fuse_stat`
/// already works from parents whose `Stats` carry their own tier, so
/// applying it again here would pay for it twice.
#[test]
fn fusing_does_not_re_apply_the_rarity_multiplier() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let plain_a = spawn_tamed(&mut game, 10, 3);
    let plain_b = spawn_tamed(&mut game, 10, 3);
    game.fuse_companions(plain_a, plain_b, None).unwrap();
    let baseline = game.owned_pets()[0].max_hp;

    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(a).insert(Rarity::Gold);
    game.world.entity_mut(b).insert(Rarity::Gold);
    game.fuse_companions(a, b, None).unwrap();

    assert_eq!(
        game.owned_pets()[0].max_hp,
        baseline,
        "two parents with identical stats must fuse to identical stats \
         whatever tier they are tagged with"
    );
}

#[test]
fn a_shiny_programs_name_carries_its_tier() {
    let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    let plain = game.owned_pets()[0].name.clone();

    game.world.entity_mut(pet).insert(Rarity::Gold);
    let shiny = game.owned_pets()[0].name.clone();

    assert_ne!(
        shiny, plain,
        "the tier has to show up somewhere in the name"
    );
    assert!(
        shiny.contains(Rarity::Gold.label().unwrap()),
        "expected the player-facing tier word in {shiny:?}"
    );
    assert!(
        shiny.ends_with(&plain),
        "the tier is a prefix, so the zone tag stays on the end: {shiny:?}"
    );
}

/// Renaming is not a way to shed the tier — a renamed Overclocked program
/// is still Overclocked, and the prefix says so.
#[test]
fn a_custom_name_still_carries_the_tier() {
    let mut game = Game::new(95, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(pet).insert(Rarity::Silver);
    game.rename_companion(pet, Some("Hexed".to_string()))
        .unwrap();

    let name = game.owned_pets()[0].name.clone();
    assert!(
        name.contains("Hexed") && name.contains(Rarity::Silver.label().unwrap()),
        "expected both the chosen name and the tier in {name:?}"
    );
}

/// `fuse_companions` does its own reap and never calls
/// `dissolve_tamed_program`, so it carries its own strip — and the ordering
/// is the whole correctness argument: after the `Stats` snapshot, the gear
/// bonus is already baked into the child.
#[test]
fn fusing_a_geared_program_returns_its_gear_and_leaves_the_child_unchanged() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let weapon = ItemId::from(ids::OVERCLOCK_CORE);
    give(&mut game, &weapon, 1);

    let tamed = |game: &mut Game| -> Vec<Entity> {
        let mut query = game.world.query_filtered::<Entity, With<Tamed>>();
        query.iter(&game.world).collect()
    };

    // The yardstick: the same two programs fused with nothing worn.
    let c = spawn_tamed(&mut game, 20, 10);
    let d = spawn_tamed(&mut game, 10, 6);
    let before = tamed(&mut game);
    game.fuse_companions(c, d, None).unwrap();
    let bare_child = *tamed(&mut game)
        .iter()
        .find(|e| !before.contains(e))
        .expect("a fused program exists");
    let expected = *game.world.get::<Stats>(bare_child).unwrap();

    let a = spawn_tamed(&mut game, 20, 10);
    let b = spawn_tamed(&mut game, 10, 6);
    game.equip(a, &gear(&weapon, 0)).unwrap();
    let before = tamed(&mut game);
    game.fuse_companions(a, b, None).unwrap();
    let geared_child = *tamed(&mut game)
        .iter()
        .find(|e| !before.contains(e))
        .expect("a fused program exists");
    let actual = *game.world.get::<Stats>(geared_child).unwrap();

    assert_eq!(
        held(&game, &weapon),
        1,
        "gear on a fused-away parent comes back to cargo"
    );
    assert_eq!(
        (actual.max_hp, actual.atk, actual.def),
        (expected.max_hp, expected.atk, expected.def),
        "a worn weapon must not be fused into the child's base stats"
    );
}
