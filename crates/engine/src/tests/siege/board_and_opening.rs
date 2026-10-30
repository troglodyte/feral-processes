use super::*;

#[cfg(test)]
mod board {
    use super::*;
    use crate::base_grid::BaseGrid;
    use crate::game::siege::board::{build, siege_spawn_cells};
    use crate::tactical::map::BattleCell;

    fn set_tile(game: &mut Game, (x, y): (i32, i32), walkable: bool) {
        let biome = if walkable {
            Biome::OpenGrid
        } else {
            Biome::DataVoid
        };
        game.world.resource_mut::<WorldMap>().set_override(
            x,
            y,
            Tile {
                biome,
                walkable,
                rock_shade: None,
            },
        );
    }

    /// Every cell within `r` of `center` set walkable (`true`) or blocked,
    /// so a test's ground never depends on the seed's terrain.
    fn fill(game: &mut Game, center: (i32, i32), r: i32, walkable: bool) {
        for y in center.1 - r..=center.1 + r {
            for x in center.0 - r..=center.0 + r {
                set_tile(game, (x, y), walkable);
            }
        }
    }

    fn floor(game: &mut Game, cells: &[(i32, i32)]) {
        let mut grid = game.world.resource_mut::<BaseGrid>();
        for &(x, y) in cells {
            grid.lay_floor(x, y);
        }
    }

    fn spawn_cells(game: &Game) -> Vec<(i32, i32)> {
        siege_spawn_cells(
            game.world.resource::<BaseGrid>(),
            game.world.resource::<WorldMap>(),
        )
    }

    #[test]
    fn no_floor_at_all_yields_no_board() {
        let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(build(&mut game).is_none());
    }

    /// Siege spawn cells are the walkable field cells orthogonally next to
    /// the floor's edge — never floor, never diagonal-only — sorted by
    /// coordinate, and the same answer every time it is asked.
    #[test]
    fn siege_spawn_cells_lie_on_the_edge_and_are_deterministic() {
        let mut game = Game::new(941, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill(&mut game, (0, 0), 4, true);
        floor(&mut game, &[(0, 0), (1, 0), (2, 0)]);
        // One edge cell is rock, so the answer is not just "every neighbour".
        set_tile(&mut game, (1, 1), false);

        let cells = spawn_cells(&game);
        let expected = vec![(-1, 0), (0, -1), (0, 1), (1, -1), (2, -1), (2, 1), (3, 0)];
        assert_eq!(cells, expected);
        assert_eq!(spawn_cells(&game), cells, "asked twice, answered the same");
    }

    /// A sealed base — every cell touching the floor is rock — falls back to
    /// the nearest walkable ring outside it, found by walking out through the
    /// rock, rather than calling the siege off.
    #[test]
    fn a_sealed_base_spawns_at_the_nearest_walkable_ring() {
        let mut game = Game::new(942, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill(&mut game, (0, 0), 6, true);
        fill(&mut game, (0, 0), 2, false);
        floor(&mut game, &[(0, 0)]);

        let cells = spawn_cells(&game);
        assert!(
            !cells.is_empty(),
            "a sealed base still has somewhere to come from"
        );
        let nearest = cells.iter().map(|&(x, y)| x.abs() + y.abs()).min().unwrap();
        assert!(
            cells.iter().all(|&(x, y)| x.abs() + y.abs() == nearest),
            "only the first ring that holds walkable ground: {cells:?}"
        );
        assert_eq!(
            nearest, 3,
            "the rock is two cells deep, so ring three is open"
        );
        let mut sorted = cells.clone();
        sorted.sort();
        assert_eq!(cells, sorted);
    }

    /// Nothing walkable within the search: no board, so the clock resolves
    /// the siege off-screen instead.
    #[test]
    fn a_base_buried_past_the_search_yields_no_board() {
        let mut game = Game::new(943, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill(
            &mut game,
            (0, 0),
            crate::tuning::SIEGE_SPAWN_SEARCH_TILES + 2,
            false,
        );
        floor(&mut game, &[(0, 0)]);
        assert!(spawn_cells(&game).is_empty());
        assert!(build(&mut game).is_none());
    }

    /// Floor and spawn cells read `Open`; everything else in the box reads
    /// `Blocked`.
    #[test]
    fn floor_and_spawn_cells_are_open_and_the_rest_blocked() {
        let mut game = Game::new(944, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill(&mut game, (0, 0), 4, true);
        floor(&mut game, &[(0, 0), (1, 0), (2, 0)]);

        let siege_board = build(&mut game).expect("the floor has an edge");
        for cell in [(0, 0), (1, 0), (2, 0), (-1, 0), (1, 1)] {
            let at = siege_board.to_board(cell).unwrap();
            assert_eq!(
                siege_board.board.cell(at.0, at.1),
                BattleCell::Open,
                "{cell:?}"
            );
        }
        let corner = siege_board.to_board((-1, -1)).unwrap();
        assert_eq!(
            siege_board.board.cell(corner.0, corner.1),
            BattleCell::Blocked,
            "a diagonal-only neighbour is not a spawn cell"
        );
        for &spawn in &siege_board.spawns {
            assert_eq!(siege_board.board.cell(spawn.0, spawn.1), BattleCell::Open);
        }
    }

    /// `to_board`/`to_field` round-trip for every cell, and `to_board`
    /// answers `None` outside the box.
    #[test]
    fn to_board_and_to_field_round_trip() {
        let mut game = Game::new(945, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill(&mut game, (0, 0), 4, true);
        floor(&mut game, &[(0, 0), (1, 0), (0, 1), (-1, 0)]);

        let siege_board = build(&mut game).expect("the floor has an edge");
        for field_cell in [(0, 0), (1, 0), (0, 1), (-1, 0)] {
            let cell = siege_board
                .to_board(field_cell)
                .expect("a floor cell must be on the board");
            assert_eq!(siege_board.to_field(cell), field_cell);
        }
        assert_eq!(siege_board.to_board((1000, 1000)), None);
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
        lay_siege_pocket(game);
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

    /// A siege board is the base's own floor, so it reads `Platform`
    /// whatever surface tile the base sits under.
    #[test]
    fn a_siege_boards_ground_is_platform_not_the_surface_biome() {
        let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);

        assert!(game.open_siege());
        let spec_biome = game.world.resource::<TacticalBattle>().spec.biome;
        assert_ne!(
            spec_biome,
            crate::world::Biome::Platform,
            "the fixture must prove the override"
        );
        assert_eq!(
            game.tactical_view().unwrap().ground,
            crate::world::Biome::Platform
        );
    }

    /// The pack is drawn from ground a species lives on. The first spawn
    /// cell in coordinate order is often mined ground (`Biome::Excavated`,
    /// cut and not yet floored), which fields nobody; drawing there left the
    /// pack empty and sent a siege the player was standing in off-screen.
    #[test]
    fn a_mined_first_spawn_cell_still_fields_a_pack() {
        let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);
        let first = board::siege_spawn_cells(
            game.world.resource::<crate::base_grid::BaseGrid>(),
            game.world.resource::<WorldMap>(),
        )[0];
        game.world.resource_mut::<WorldMap>().set_override(
            first.0,
            first.1,
            Tile {
                biome: Biome::Excavated,
                walkable: true,
                rock_shade: None,
            },
        );
        assert!(
            game.habitat_pools(first.0, first.1, None, 0).is_none(),
            "test premise: mined ground fields nobody"
        );

        assert!(game.open_siege(), "the siege should open on the board");
        assert!(game.world.resource::<TacticalBattle>().siege_pack > 0);
    }

    /// A sealed base: the fixture pocket ringed by rock two cells deep, open
    /// habitat ground beyond, so the siege board's spawn cells are the
    /// breadth-first fallback ring with no walk to the floor.
    fn sealed_base(game: &mut Game) {
        lay_siege_pocket(game);
        let (w, h) = (TEST_POCKET_HALF_WIDTH, TEST_POCKET_HALF_HEIGHT);
        for y in -(h + 6)..=(h + 6) {
            for x in -(w + 6)..=(w + 6) {
                if game
                    .world
                    .resource::<crate::base_grid::BaseGrid>()
                    .is_floor(x, y)
                {
                    continue;
                }
                let rock = x.abs() <= w + 2 && y.abs() <= h + 2;
                game.world.resource_mut::<WorldMap>().set_override(
                    x,
                    y,
                    Tile {
                        biome: if rock {
                            Biome::DataVoid
                        } else {
                            Biome::OpenGrid
                        },
                        walkable: !rock,
                        rock_shade: None,
                    },
                );
            }
        }
        stand_in_base_at(game, 0, 0);
    }

    /// Besiegers that cannot reach the floor hold for
    /// `SIEGE_SEALED_HOLD_ROUNDS` and withdraw: the fight ends, nobody of
    /// theirs is left on the map, and nothing was taken or dropped.
    #[test]
    fn a_sealed_bases_siege_ends_in_a_withdrawal_after_the_hold() {
        use crate::tuning::SIEGE_SEALED_HOLD_ROUNDS;
        let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        sealed_base(&mut game);
        set_zone(&mut game, 2);
        let held = game
            .world
            .resource::<ItemDb>()
            .all()
            .find(|d| d.equipment.is_none())
            .map(|d| d.id.clone())
            .expect("the shipped items hold a plain item");
        game.dev_give_item(&held, 3).unwrap();
        let stock_before = game.base_stock();
        let pack_items_before = game
            .world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .items
            .clone();
        assert!(
            !pack_items_before.is_empty(),
            "the check needs something to lose"
        );
        let pack_before: Vec<Entity> = {
            assert!(game.open_siege(), "a sealed base still opens its siege");
            game.world
                .resource::<TacticalBattle>()
                .bodies()
                .map(|(e, _)| e)
                .filter(|&e| game.world.get::<Hostile>(e).is_some())
                .collect()
        };
        assert!(!pack_before.is_empty());

        let mut last_round = 0;
        for _ in 0..10_000 {
            let Some(battle) = game.world.get_resource::<TacticalBattle>() else {
                break;
            };
            last_round = battle.round;
            if !game.tactical_ai_turn() {
                game.tactical_end_turn();
            }
        }

        assert!(
            game.world.get_resource::<TacticalBattle>().is_none(),
            "a sealed siege must end, not hold forever"
        );
        assert_eq!(
            last_round, SIEGE_SEALED_HOLD_ROUNDS,
            "the besiegers hold for exactly the hold, then go"
        );
        for raider in pack_before {
            assert!(
                game.world.get_entity(raider).is_err(),
                "a withdrawn besieger left a body on the map"
            );
        }
        assert!(
            game.message_history(200)
                .iter()
                .any(|row| row.text.contains("no way in")),
            "the withdrawal should be announced"
        );
        assert_eq!(
            game.base_stock(),
            stock_before,
            "a withdrawal took base stock"
        );
        assert_eq!(
            game.world
                .get::<Inventory>(game.player_entity())
                .unwrap()
                .items,
            pack_items_before,
            "a withdrawal changed the party's inventory"
        );
    }

    /// A besieger with a walk to the floor holds the whole pack in the
    /// fight; only when none has one is the base sealed. Terrain alone: the
    /// one open cell between them is enough.
    #[test]
    fn one_open_path_to_the_floor_means_the_base_is_not_sealed() {
        use crate::game::siege::raiders::any_reaches_floor;
        use crate::tactical::map::{BattleCell, Board};
        let mut board = Board::solid(7);
        // Floor at (1, 1); a besieger at (5, 1) behind a wall at x = 3.
        for x in 1..=5 {
            board.set(x, 1, BattleCell::Open);
        }
        board.set(3, 1, BattleCell::Blocked);
        let is_floor = |c: (i32, i32)| c == (1, 1);
        assert!(!any_reaches_floor(&board, &[(5, 1)], is_floor));

        board.set(3, 1, BattleCell::Open);
        assert!(any_reaches_floor(&board, &[(5, 1)], is_floor));
        assert!(
            any_reaches_floor(&board, &[(5, 1), (6, 6)], is_floor),
            "one besieger with a path is enough"
        );
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

    /// Raiders are all seated on the floor's edge at round 1, and their
    /// count is exactly `siege::pack_size(zone)` — the same call the
    /// off-screen resolution prices a shortfall with, so the siege you
    /// fight and the siege you miss field the same pack.
    #[test]
    fn raiders_are_seated_on_the_floors_edge_and_number_pack_size() {
        let mut game = Game::new(953, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        ready_base(&mut game);
        set_zone(&mut game, 2);
        let spawns = crate::game::siege::board::siege_spawn_cells(
            game.world.resource::<crate::base_grid::BaseGrid>(),
            game.world.resource::<WorldMap>(),
        );

        assert!(game.open_siege());
        let origin = game.world.resource::<TacticalBattle>().siege_origin;

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
            let field = (cell.0 + origin.0, cell.1 + origin.1);
            assert!(
                spawns.contains(&field),
                "a raider at {field:?} was not seated on the floor's edge"
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
            .find(|d| !d.is_hybrid())
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
        assert_eq!(
            battle.fallen(),
            [crate::tactical::Fallen {
                cell: (1, 0),
                footprint: 1,
                raise: None,
            }],
            "a destroyed structure must leave its mark on the board"
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
        charge_defences(&mut game);

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
        charge_defences(&mut game);

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
