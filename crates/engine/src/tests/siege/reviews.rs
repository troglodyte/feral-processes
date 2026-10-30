use super::*;

/// Review findings (2026-09-22 `feat/siege` review) fixed against real
/// doors: `Game::open_siege` on a base with a real Home spawned via
/// `place_home` at `TEST_HOME_CELL`, `Game::tactical_ai_beat` for AI turns,
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
    /// `TEST_HOME_CELL` — with the player standing at `player_at` rather
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
        let home = first_structure(&mut game, "home");
        let home_cell = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(home)
            .expect("the Home is seated on its own tile");
        let besieger = any_besieger(&game);
        let beside = free_neighbour(game.world.resource::<TacticalBattle>(), home_cell);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(besieger, beside),
            "the fixture must actually free the cell beside the Home"
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
            Some(home_cell),
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
            grid.lay_floor(-TEST_POCKET_HALF_WIDTH - 1, 0);
        }
        let rebuilt_door = crate::game::siege::board::build(&mut game).unwrap().spawns[0];
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

        no_habitat_around_the_pocket(&mut game);

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
        charge_defences(&mut game);

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
                .bodies()
                .map(|(e, _)| e)
                .find(|&e| {
                    game.world
                        .get::<Structure>(e)
                        .is_some_and(|s| s.kind == "home")
                })
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
    use crate::base_grid::BaseGrid;
    use crate::components::{Besieger, Carrying, StolenFrom, Tamed, Task, TaskKind};
    use crate::game::siege::board;
    use crate::items::ids;
    use crate::tactical::TacticalBattle;
    use crate::tactical::ai::AiBeat;
    use crate::tactical::map::{BattleSpec, Board};
    use crate::tactical::reach;
    use crate::tuning::HAUL_CARRY_CAPACITY;
    use crate::world::Biome;

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
        // A source still holding a `Stock`, so the capture has somewhere
        // to put the goods back — a kill would, and so must a capture.
        let stale_source = game.world.spawn(crate::components::Stock::default()).id();
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
        assert_eq!(
            game.world
                .get::<crate::components::Stock>(stale_source)
                .unwrap()
                .output
                .get(&core_fragment())
                .copied(),
            Some(1),
            "a captured carrier must drop its cargo back, as a kill does"
        );
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

    // ---- C1 (walk stalls): a carrier reaches the door across an L-shaped
    // corridor whose north leg holds a near-constant Chebyshev distance to
    // the door for its whole length.

    #[test]
    fn c1_a_carrier_at_the_far_end_of_an_l_corridor_still_reaches_the_door() {
        let mut game = Game::new(210_004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        // A single-file corridor grafted onto the fixture pocket's own edge —
        // `(9, 0)` is the pocket's last floored cell along this row. East along `y == 0` to `x == 12`, then north up the
        // fixed `x == 12` column: every cell on that north leg is
        // Chebyshev distance exactly 12 from the door at every `y`, which
        // is the shape that stalled the old closing-distance walk.
        {
            let mut grid = game.world.resource_mut::<BaseGrid>();
            for x in 5..=12 {
                grid.lay_floor(x, 0);
            }
            for y in -10..=-1 {
                grid.lay_floor(12, y);
            }
        }
        // Rock all round, with one opening west of the pocket: the siege's
        // spawn cells, and so its door, are as far from the corridor's far
        // end as the base allows.
        for y in -12..=6 {
            for x in -12..=14 {
                let floored = game.world.resource::<BaseGrid>().is_floor(x, y);
                if !floored {
                    set_rock(&mut game, (x, y));
                }
            }
        }
        for y in -2..=2 {
            game.world.resource_mut::<WorldMap>().set_override(
                -TEST_POCKET_HALF_WIDTH - 1,
                y,
                Tile {
                    biome: Biome::OpenGrid,
                    walkable: true,
                    rock_shade: None,
                },
            );
        }
        // `move_to` and `siege_door` both take *board* coordinates, not the
        // field ones the fixture just floored — resolved through the same
        // `board::build` a real `Game::open_siege` calls, rather than
        // hand-computing the flood fill's own bounding box corner.
        let far_end = board::build(&mut game)
            .unwrap()
            .to_board((12, -10))
            .expect("the corridor's far end must be inside the flood fill");
        stand_in_base_at(&mut game, 2, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());

        let besieger = any_besieger(&game);
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(besieger, far_end),
            "the fixture must actually seat the carrier at the corridor's \
             own far end"
        );
        let door = game.world.resource::<TacticalBattle>().siege_door;
        assert!(
            reach::distance(far_end, door) > game.movement_allowance(besieger),
            "the fixture must need more than one turn's movement to close, \
             or this proves nothing about the walk stalling across rounds"
        );
        // A source to steal from is not this test's concern; `Carrying`
        // alone is what `besieger_turn` reads to make for the door.
        game.world.entity_mut(besieger).insert((
            Carrying {
                item: core_fragment(),
                qty: 1,
            },
            StolenFrom(besieger),
        ));
        // Unkillable, and the only body with a turn — `c1_home_stands_on_
        // the_door...`'s own fixture shape, so the only way this entity can
        // vanish is `besieger_leaves`'s own despawn.
        game.world.get_mut::<Stats>(besieger).unwrap().hp = 1_000_000;
        game.world.get_mut::<Stats>(besieger).unwrap().max_hp = 1_000_000;
        let player = game.player_entity();
        game.world.get_mut::<Stats>(player).unwrap().hp = 1_000_000;
        game.world.get_mut::<Stats>(player).unwrap().max_hp = 1_000_000;
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![besieger]);

        let mut left = false;
        for _ in 0..500 {
            if game.world.get::<Besieger>(besieger).is_none() {
                left = true;
                break;
            }
            game.tactical_ai_beat();
        }
        assert!(
            left,
            "a carrier at the far end of a long L-corridor must eventually \
             reach and leave through the door, across as many rounds as it \
             takes"
        );
    }

    // ---- C4-m: the reap's bench/dissolve matches `Game::run_raid`'s
    // defender precedent — a log line, and `Task` stripped before it.

    #[test]
    fn c4m_a_staff_death_on_the_board_logs_and_strips_task_like_the_raid_precedent() {
        let mut game = Game::new(210_006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        let staff = spawn_tamed(&mut game, 10, 3);
        {
            let mut pos = game.world.get_mut::<Position>(staff).unwrap();
            pos.x = 2;
            pos.y = 0;
        }
        // A `Task` to strip — anything but `Guard`, which
        // `party::walks_the_base` excludes from `base_bodies()` and so
        // from the siege board entirely.
        let dummy_target = game.world.spawn_empty().id();
        game.world.entity_mut(staff).insert(Task {
            kind: TaskKind::Excavate,
            target: dummy_target,
            progress: 0,
            required: 1,
        });
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
        let player = game.player_entity();
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![player]);
        game.tactical_end_turn();

        assert!(
            game.world.get::<Task>(staff).is_none(),
            "the raid precedent strips Task before the dissolve, not by \
             it, or a posted worker's own detachment line lands twice"
        );
        let logged = game
            .message_history(20)
            .iter()
            .any(|entry| entry.text.contains("falls in the siege"));
        assert!(
            logged,
            "a staff body lost on the board must log a line naming it, \
             the raid precedent's own rule"
        );
    }

    #[test]
    fn a_jailed_prisoner_is_not_seated_on_the_siege_board() {
        let mut game = Game::new(210_007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        let prisoner = spawn_tamed(&mut game, 10, 3);
        {
            let mut pos = game.world.get_mut::<Position>(prisoner).unwrap();
            pos.x = 2;
            pos.y = 0;
        }
        let cell = game.world.spawn_empty().id();
        game.world
            .entity_mut(prisoner)
            .insert(crate::components::Jailed {
                cell,
                attempts: 0,
                progress: 0,
                record: None,
            });
        stand_in_base_at(&mut game, 3, 0);
        set_zone(&mut game, 2);
        assert!(game.open_siege());
        assert!(
            game.world
                .resource::<TacticalBattle>()
                .cell_of(prisoner)
                .is_none(),
            "a prisoner behind bars is no defender"
        );
    }

    // ---- M2-m: an unseatable pack falls back to an off-screen resolution
    // instead of holding the pressure and re-drawing `GameRng` forever.

    #[test]
    fn m2m_an_unseatable_pack_falls_back_to_offscreen_resolution() {
        let mut game = Game::new(210_007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game);
        establish_base(&mut game, true);
        stand_in_base_at(&mut game, 2, 0);
        set_zone(&mut game, 2);

        // `open_siege`'s own empty-pack repro (`m2_an_empty_pack_does_not_
        // open_a_siege`): no habitat at any spawn cell, so
        // `spawn_siege_pack` seats nobody, every tick, forever.
        no_habitat_around_the_pocket(&mut game);

        // Wound to the fire point directly rather than ticked there —
        // thousands of ticks through the real clock would let a *first*
        // successful off-screen resolution consume this fixture's own
        // small pool of staff and structures well before the loop ends,
        // which would trip `nothing_to_besiege`'s hold for a real and
        // unrelated reason and read as this fix having failed.
        {
            let mut pressure = game.world.resource_mut::<crate::resources::SiegePressure>();
            pressure.next_at = Some(SIEGE_PRESSURE_THRESHOLD);
            pressure.level = SIEGE_PRESSURE_THRESHOLD;
        }

        // Through the real clock, one tick — `resolve_siege_offscreen` is
        // documented `Always true` once `nothing_to_besiege` has already
        // passed, so a single tick at the fire point must resolve.
        game.siege_check();

        assert_eq!(
            game.siege_pressure(),
            0,
            "an unseatable pack must still resolve — off-screen — rather \
             than hold its pressure and retry forever"
        );
        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "nothing could seat, so the on-screen board must never have \
             opened"
        );
    }

    // ---- M4-m / M5: a besieger leaving mid-round (not on the last rung)
    // still skips a disengaged staff body onto the cursor, and ages that
    // body's own tamper on the way past it.

    /// A hand-built board — `m4_a_besieger_leaving_on_the_last_rung_...`'s
    /// own shape — with `leaving` first in the order, `staff` disengaged
    /// far from everything, and `raider2` engaged with the player. Removing
    /// `leaving` (first in a three-body order) does not wrap the round, so
    /// nothing but `besieger_leaves` itself ever calls
    /// `skip_disengaged_turns` on this path.
    fn m4_fixture(seed: u32) -> (Game, Entity, Entity, Entity) {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let side = 20;
        let mut board = Board::solid(side);
        for y in 0..side {
            for x in 0..side {
                board.set(x, y, crate::tactical::map::BattleCell::Open);
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
        battle.siege_pack = 3;

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
        let raider2 = game
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
        let staff = game
            .world
            .spawn(Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                mitigation: 0,
            })
            .id();

        battle.place(player, (9, 9));
        battle.place(raider2, (9, 8));
        battle.place(staff, (19, 19));
        battle.place(leaving, (1, 0));
        battle.set_initiative(vec![leaving, staff, raider2]);

        game.world.insert_resource(battle);
        game.world.entity_mut(leaving).insert((
            Carrying {
                item: core_fragment(),
                qty: 1,
            },
            StolenFrom(leaving),
        ));

        (game, leaving, staff, raider2)
    }

    #[test]
    fn m4m_a_besieger_leaving_mid_round_still_skips_a_disengaged_staff_body() {
        let (mut game, leaving, staff, raider2) = m4_fixture(210_008);
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(leaving),
            "the fixture must put the leaving besieger first in the order"
        );

        assert_ne!(
            game.tactical_ai_beat(),
            AiBeat::Idle,
            "the leaving besieger must actually get a beat"
        );
        assert!(
            game.world.get::<Besieger>(leaving).is_none(),
            "the fixture must actually leave through the door for this \
             test to be about the mid-round case at all"
        );

        assert_eq!(
            game.world.resource::<TacticalBattle>().round,
            1,
            "leaving first in a three-body order must not wrap the round \
             — the fixture the M4-m finding is about"
        );
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(raider2),
            "a disengaged staff body handed the cursor by the besieger's \
             own departure must be skipped immediately, not left as the \
             current actor for the next AI beat to drive toward the fight"
        );
        let _ = staff;
    }

    /// The skip `besieger_leaves` runs can itself wrap the round — here the
    /// only body after the departing besieger is a disengaged staff body,
    /// so skipping it wraps — and that wrap still owes the round's upkeep.
    /// Read before the skip, the wrap went unseen and the round's tick
    /// (and turret fire) was lost.
    #[test]
    fn a_wrap_made_by_the_skip_after_a_departure_still_runs_the_upkeep() {
        let (mut game, leaving, staff, raider2) = m4_fixture(210_010);
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![raider2, leaving, staff]);
        game.world.resource_mut::<TacticalBattle>().end_turn();
        assert_eq!(
            game.world.resource::<TacticalBattle>().actor(),
            Some(leaving)
        );
        let tick_before = game.world.resource::<GameClock>().tick;

        assert_ne!(game.tactical_ai_beat(), AiBeat::Idle);
        assert!(
            game.world.get::<Besieger>(leaving).is_none(),
            "the besieger must have left through the door"
        );

        assert_eq!(
            game.world.resource::<TacticalBattle>().round,
            2,
            "skipping the disengaged staff body must wrap the round"
        );
        assert!(
            game.world.resource::<GameClock>().tick > tick_before,
            "the wrap the skip made must still run the round's upkeep"
        );
    }

    #[test]
    fn m5_a_skipped_bodys_own_tamper_ages_past_it() {
        use crate::abilities::TamperKind;
        use crate::components::Tampered;

        let (mut game, _leaving, staff, _raider2) = m4_fixture(210_009);
        let mut tampered = Tampered::default();
        tampered.apply(TamperKind::Profiled, 1, false);
        game.world.entity_mut(staff).insert(tampered);

        game.tactical_ai_beat();

        assert!(
            game.world.get::<Tampered>(staff).is_none(),
            "a one-turn tamper on a body `skip_disengaged_turns` passes \
             over must still age and expire, not freeze for skipping \
             never running it through `hand_on_turn`"
        );
    }
}

/// Opus review of todo #103 (2026-09-26): `tactical_intent`'s `Invoke` arm
/// excluded only `Hostile`, `Summoned` and taken-over, so a body that is
/// none of those but also not the player's own party — siege staff chief
/// among them, `tactical_skippable` and seated on the board whenever
/// something is in its reach — fell through to it too, and a routine it
/// could not afford sent it into `Game::tactical_use_item`, which refuses
/// anyone outside the player's party. `Game::in_party` is the fix.
mod auto_attack_party_arm_review {
    use super::*;
    use crate::tactical::TacticalBattle;
    use crate::tactical::ai::AiBeat;
    use crate::tactical::map::BattleCell;
    use crate::tests::support::HOSTILE_SWEEP;
    use crate::tests::tactical::{only_routine, tactical_fight};
    use crate::tuning::TACTICAL_MOVE_MAX;

    /// A free, open neighbour of `near` — `review_findings::free_neighbour`'s
    /// own reason, one module over.
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
            .expect("the hostile must have at least one free neighbour")
    }

    /// A hostile and a body outside the player's party (`spawn_tamed`'s full
    /// roster parts, not a hand-built stand-in — a fixture short a component
    /// reads as the feature being broken rather than as the fixture being
    /// wrong), seated beside each other in a real tactical fight.
    fn fixture(seed: u32) -> (Game, Entity, Entity) {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pack = tactical_fight(&mut game, 1, 200);
        let hostile = pack[0];
        let staff = spawn_tamed(&mut game, 200, 5);

        let beside = {
            let battle = game.world.resource::<TacticalBattle>();
            let hostile_cell = battle.cell_of(hostile).expect("the hostile must be seated");
            free_neighbour(battle, hostile_cell)
        };
        assert!(
            game.world
                .resource_mut::<TacticalBattle>()
                .place(staff, beside),
            "no free cell beside the hostile to seat the staff body"
        );
        game.world
            .resource_mut::<TacticalBattle>()
            .set_initiative(vec![staff]);
        only_routine(&mut game, staff, HOSTILE_SWEEP);
        (game, staff, hostile)
    }

    #[test]
    fn a_body_outside_the_party_swings_under_auto_attack_instead_of_invoking() {
        let (mut game, staff, _hostile) = fixture(220_001);

        for _ in 0..=TACTICAL_MOVE_MAX {
            if game.tactical_auto_beat() == AiBeat::Acted {
                break;
            }
        }

        assert!(
            game.world
                .get::<AbilityCooldowns>(staff)
                .is_none_or(|c| !c.0.contains_key(HOSTILE_SWEEP)),
            "a body outside the player's party invoked a routine under \
             auto-attack, which is the party's own to spend"
        );
    }
}
